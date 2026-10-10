//! Tool schemas, input validation, and execution.
//!
//! Tool inputs stream with `eager_input_streaming`, which turns off the API's
//! own validation, so every input is deserialized into a typed struct and
//! checked here before anything runs. Failures go back to the model as
//! `is_error` results so it can correct itself.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::error::AppResult;
use crate::models::{CardInput, Mode};
use crate::repo;

pub const ASK_CHOICE: &str = "ask_choice";

/// Tools whose output the frontend renders from the tool input.
pub const RENDER_TOOLS: &[&str] = &[
    "generate_interactive",
    "render_table",
    "render_quiz",
    "create_flashcards",
    "render_worked_example",
    "render_lesson_plan",
    "render_rubric",
];

pub struct ToolContext<'a> {
    pub db: &'a Db,
    pub course_id: Option<&'a str>,
    pub conversation_id: &'a str,
}

pub struct ToolOutcome {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutcome {
    fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false }
    }
    fn err(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true }
    }
}

/// Tool definitions for a conversation mode, in a fixed order so the request
/// prefix stays cacheable.
pub fn definitions(mode: Mode) -> Vec<Value> {
    let mut tools = vec![
        generate_interactive_def(),
        render_table_def(),
        render_quiz_def(),
        create_flashcards_def(),
        render_worked_example_def(),
    ];
    if mode == Mode::Teach {
        tools.push(render_lesson_plan_def());
        tools.push(render_rubric_def());
    }
    tools.push(search_def());
    if mode == Mode::Learn {
        tools.push(progress_def());
    }
    tools.push(ask_choice_def());
    tools.push(plan_def());
    tools
}

fn generate_interactive_def() -> Value {
    json!({
        "name": "generate_interactive",
        "description": "Render a custom visual or interactive answer (explainer, simulation, chart, diagram, or calculator) as HTML, CSS, and JavaScript that streams into a sandboxed frame in the chat. Emit the parameters in exactly the listed order.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Short, specific title (60 characters or fewer)." },
                "initialHeight": { "type": "integer", "description": "Estimated finished height in pixels, 200–1400." },
                "placeholderMessages": { "type": "array", "items": { "type": "string" }, "description": "2–4 short progress messages shown while the styles stream." },
                "css": { "type": "string", "description": "All widget styles. The design system is already loaded." },
                "html": { "type": "string", "description": "Body markup, readable before scripts run. No <style> blocks or inline scripts." },
                "jsFunctions": { "type": "string", "description": "Named function declarations. Classic script: no top-level await, no static import." },
                "jsExpressions": { "type": "array", "items": { "type": "string" }, "description": "Small synchronous statements that call the functions, run in order." }
            },
            "required": ["title", "initialHeight", "placeholderMessages", "css", "html", "jsFunctions", "jsExpressions"]
        }
    })
}

fn render_table_def() -> Value {
    json!({
        "name": "render_table",
        "description": "Render an accessible table of exact values or a side-by-side comparison.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "columns": { "type": "array", "items": { "type": "string" }, "description": "1–12 column headers." },
                "rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Up to 200 rows; each has exactly one cell per column." },
                "source": { "type": "string", "description": "Honest provenance: user-provided, calculated, cited source, or illustrative." }
            },
            "required": ["title", "columns", "rows", "source"]
        }
    })
}

fn render_quiz_def() -> Value {
    json!({
        "name": "render_quiz",
        "description": "Render a quiz with instant feedback. Learners' attempts are saved to their progress. In teach mode the instructor also sees the answer key.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "topic": { "type": "string", "description": "Specific topic, used to track mastery." },
                "purpose": { "type": "string", "enum": ["practice", "assessment"] },
                "questions": {
                    "type": "array",
                    "description": "1–30 questions.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["multiple_choice", "true_false", "numeric", "short_answer"] },
                            "prompt": { "type": "string", "description": "Markdown with LaTeX allowed." },
                            "options": { "type": "array", "items": { "type": "string" }, "description": "multiple_choice only: 3–5 options." },
                            "answer_index": { "type": "integer", "description": "multiple_choice only: zero-based index of the correct option." },
                            "option_feedback": { "type": "array", "items": { "type": "string" }, "description": "multiple_choice only: why each option is right or wrong, same order as options." },
                            "answer_bool": { "type": "boolean", "description": "true_false only." },
                            "answer_number": { "type": "number", "description": "numeric only." },
                            "tolerance": { "type": "number", "description": "numeric only: accepted absolute error." },
                            "unit": { "type": "string", "description": "numeric only." },
                            "accepted_answers": { "type": "array", "items": { "type": "string" }, "description": "short_answer only: accepted phrasings." },
                            "explanation": { "type": "string" },
                            "bloom": { "type": "string", "enum": ["remember", "understand", "apply", "analyze", "evaluate", "create"] }
                        },
                        "required": ["type", "prompt", "explanation"]
                    }
                }
            },
            "required": ["title", "topic", "purpose", "questions"]
        }
    })
}

fn create_flashcards_def() -> Value {
    json!({
        "name": "create_flashcards",
        "description": "Create a flashcard deck. The deck is saved and scheduled for spaced-repetition review in the Review tab.",
        "input_schema": {
            "type": "object",
            "properties": {
                "deck_title": { "type": "string" },
                "description": { "type": "string" },
                "cards": {
                    "type": "array",
                    "description": "1–60 cards.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "front": { "type": "string" },
                            "back": { "type": "string" },
                            "hint": { "type": "string" }
                        },
                        "required": ["front", "back"]
                    }
                }
            },
            "required": ["deck_title", "cards"]
        }
    })
}

fn render_worked_example_def() -> Value {
    json!({
        "name": "render_worked_example",
        "description": "Render a problem solved step by step. Learners reveal one step at a time and can try a similar practice problem.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "problem": { "type": "string", "description": "Markdown with LaTeX." },
                "steps": {
                    "type": "array",
                    "description": "1–15 steps.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "detail": { "type": "string", "description": "Markdown with LaTeX; explain the reasoning." }
                        },
                        "required": ["title", "detail"]
                    }
                },
                "answer": { "type": "string" },
                "practice_problem": {
                    "type": "object",
                    "properties": { "prompt": { "type": "string" }, "answer": { "type": "string" } },
                    "required": ["prompt", "answer"]
                }
            },
            "required": ["title", "problem", "steps", "answer"]
        }
    })
}

fn render_lesson_plan_def() -> Value {
    json!({
        "name": "render_lesson_plan",
        "description": "Render a lesson plan for an instructor.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "level": { "type": "string" },
                "duration_minutes": { "type": "integer" },
                "overview": { "type": "string" },
                "objectives": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" },
                            "bloom": { "type": "string", "enum": ["remember", "understand", "apply", "analyze", "evaluate", "create"] }
                        },
                        "required": ["text"]
                    }
                },
                "prerequisites": { "type": "array", "items": { "type": "string" } },
                "materials": { "type": "array", "items": { "type": "string" } },
                "sections": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "minutes": { "type": "integer" },
                            "kind": { "type": "string", "enum": ["warm-up", "lecture", "discussion", "activity", "demonstration", "assessment", "wrap-up", "break"] },
                            "description": { "type": "string" },
                            "instructor_notes": { "type": "string" }
                        },
                        "required": ["title", "minutes", "description"]
                    }
                },
                "assessment": { "type": "string" },
                "differentiation": { "type": "string" },
                "follow_up": { "type": "string" }
            },
            "required": ["title", "duration_minutes", "objectives", "sections", "assessment"]
        }
    })
}

fn render_rubric_def() -> Value {
    json!({
        "name": "render_rubric",
        "description": "Render an analytic grading rubric.",
        "input_schema": {
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "assignment": { "type": "string" },
                "levels": {
                    "type": "array",
                    "description": "Performance levels from highest to lowest.",
                    "items": {
                        "type": "object",
                        "properties": { "label": { "type": "string" }, "points": { "type": "number" } },
                        "required": ["label", "points"]
                    }
                },
                "criteria": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "weight_percent": { "type": "number" },
                            "descriptors": { "type": "array", "items": { "type": "string" }, "description": "One per level, same order as levels." }
                        },
                        "required": ["name", "descriptors"]
                    }
                },
                "notes": { "type": "string" }
            },
            "required": ["title", "levels", "criteria"]
        }
    })
}

fn search_def() -> Value {
    json!({
        "name": "search_course_materials",
        "description": "Full-text search over the notes, syllabi, and readings imported for the current course. Returns the most relevant passages with their material titles. Cite passages by title.",
        "input_schema": {
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Key terms to search for." },
                "limit": { "type": "integer", "description": "Passages to return, 1–8 (default 5)." }
            },
            "required": ["query"]
        }
    })
}

fn progress_def() -> Value {
    json!({
        "name": "get_learner_progress",
        "description": "The learner's saved practice record in this app for the current course: quiz scores by topic, flashcards due, and recent activity.",
        "input_schema": {
            "type": "object",
            "properties": { "topic": { "type": "string", "description": "Optional topic filter." } }
        }
    })
}

fn ask_choice_def() -> Value {
    json!({
        "name": ASK_CHOICE,
        "description": "Ask the user to pick one option when an ambiguity would substantially change the answer. The conversation pauses until they choose. Call it on its own, not alongside other tools.",
        "input_schema": {
            "type": "object",
            "properties": {
                "question": { "type": "string" },
                "options": {
                    "type": "array",
                    "description": "2–5 options.",
                    "items": {
                        "type": "object",
                        "properties": { "label": { "type": "string" }, "description": { "type": "string" } },
                        "required": ["label"]
                    }
                },
                "allow_other": { "type": "boolean", "description": "Let the user type their own answer." }
            },
            "required": ["question", "options"]
        }
    })
}

fn plan_def() -> Value {
    json!({
        "name": "plan_visualization",
        "description": "Optionally outline a complex interactive before building it. Not required before generate_interactive.",
        "input_schema": {
            "type": "object",
            "properties": {
                "approach": { "type": "string", "description": "One sentence on the strategy." },
                "technology": { "type": "string", "description": "For example inline SVG, Canvas, Chart.js, D3, Three.js." },
                "key_elements": { "type": "array", "items": { "type": "string" }, "description": "2–4 short bullet points." }
            },
            "required": ["approach", "technology", "key_elements"]
        }
    })
}

// ---- Typed inputs ---------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractiveInput {
    title: String,
    #[serde(default)]
    css: String,
    html: String,
    #[serde(default)]
    js_functions: String,
    #[serde(default)]
    js_expressions: Vec<String>,
}

#[derive(Deserialize)]
struct TableInput {
    title: String,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    source: String,
}

#[derive(Deserialize)]
struct QuizInput {
    title: String,
    #[serde(default)]
    topic: String,
    questions: Vec<QuestionInput>,
}

#[derive(Deserialize)]
struct QuestionInput {
    #[serde(rename = "type")]
    kind: String,
    prompt: String,
    #[serde(default)]
    options: Vec<String>,
    answer_index: Option<i64>,
    #[serde(default)]
    option_feedback: Vec<String>,
    answer_bool: Option<bool>,
    answer_number: Option<f64>,
    tolerance: Option<f64>,
    #[serde(default)]
    accepted_answers: Vec<String>,
    #[serde(default)]
    explanation: String,
}

#[derive(Deserialize)]
struct FlashcardsInput {
    deck_title: String,
    #[serde(default)]
    description: String,
    cards: Vec<FlashcardInput>,
}

#[derive(Deserialize)]
struct FlashcardInput {
    front: String,
    back: String,
    #[serde(default)]
    hint: String,
}

#[derive(Deserialize)]
struct WorkedExampleInput {
    problem: String,
    steps: Vec<StepInput>,
    answer: String,
}

#[derive(Deserialize)]
struct StepInput {
    title: String,
    detail: String,
}

#[derive(Deserialize)]
struct LessonPlanInput {
    duration_minutes: f64,
    objectives: Vec<ObjectiveInput>,
    sections: Vec<SectionInput>,
}

#[derive(Deserialize)]
struct ObjectiveInput {
    text: String,
}

#[derive(Deserialize)]
struct SectionInput {
    title: String,
    minutes: f64,
}

#[derive(Deserialize)]
struct RubricInput {
    levels: Vec<LevelInput>,
    criteria: Vec<CriterionInput>,
}

#[derive(Deserialize)]
struct LevelInput {
    label: String,
    points: f64,
}

#[derive(Deserialize)]
struct CriterionInput {
    name: String,
    weight_percent: Option<f64>,
    descriptors: Vec<String>,
}

#[derive(Deserialize)]
pub struct ChoiceInput {
    pub question: String,
    pub options: Vec<ChoiceOption>,
}

#[derive(Deserialize)]
pub struct ChoiceOption {
    pub label: String,
}

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ProgressInput {
    topic: Option<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(input: &Value) -> Result<T, String> {
    serde_json::from_value(input.clone()).map_err(|e| format!("The tool input did not match the schema: {e}. Call the tool again with corrected input."))
}

fn require(cond: bool, msg: &str) -> Result<(), String> {
    if cond {
        Ok(())
    } else {
        Err(format!("{msg} Call the tool again with corrected input."))
    }
}

// ---- Validation -----------------------------------------------------------

/// Validates render-tool input. Returns a confirmation for the model.
pub fn validate_render(name: &str, input: &Value) -> Result<String, String> {
    match name {
        "generate_interactive" => {
            let i: InteractiveInput = parse(input)?;
            require(!i.title.trim().is_empty(), "title is empty.")?;
            require(!i.html.trim().is_empty(), "html is empty.")?;
            require(!i.html.to_lowercase().contains("<style"), "Put styles in the css parameter, not a <style> block in html.")?;
            require(i.css.len() + i.html.len() + i.js_functions.len() < 400_000, "The interactive is too large; simplify it.")?;
            let _ = i.js_expressions.len();
            Ok("Rendered. The user can see and use this interactive now. Do not render it again; add a brief note only if it helps them use it.".into())
        }
        "render_table" => {
            let t: TableInput = parse(input)?;
            require(!t.title.trim().is_empty(), "title is empty.")?;
            require((1..=12).contains(&t.columns.len()), "A table needs 1–12 columns.")?;
            require(t.rows.len() <= 200, "A table can have at most 200 rows.")?;
            require(!t.source.trim().is_empty(), "source must state where the values come from.")?;
            for (i, row) in t.rows.iter().enumerate() {
                require(
                    row.len() == t.columns.len(),
                    &format!("Row {} has {} cells but there are {} columns.", i + 1, row.len(), t.columns.len()),
                )?;
                require(
                    row.iter().all(|c| c.is_string() || c.is_number() || c.is_boolean() || c.is_null()),
                    &format!("Row {} contains a nested value; cells must be text.", i + 1),
                )?;
            }
            Ok(format!("Rendered the table ({} rows). The user can see it now.", t.rows.len()))
        }
        "render_quiz" => {
            let q: QuizInput = parse(input)?;
            require(!q.title.trim().is_empty(), "title is empty.")?;
            require((1..=30).contains(&q.questions.len()), "A quiz needs 1–30 questions.")?;
            for (n, question) in q.questions.iter().enumerate() {
                let at = format!("Question {}:", n + 1);
                require(!question.prompt.trim().is_empty(), &format!("{at} prompt is empty."))?;
                require(!question.explanation.trim().is_empty(), &format!("{at} needs an explanation."))?;
                match question.kind.as_str() {
                    "multiple_choice" => {
                        require((2..=8).contains(&question.options.len()), &format!("{at} needs 2–8 options."))?;
                        let idx = question.answer_index.unwrap_or(-1);
                        require(
                            idx >= 0 && (idx as usize) < question.options.len(),
                            &format!("{at} answer_index must point at one of the options."),
                        )?;
                        require(
                            question.option_feedback.is_empty() || question.option_feedback.len() == question.options.len(),
                            &format!("{at} option_feedback needs one entry per option."),
                        )?;
                    }
                    "true_false" => require(question.answer_bool.is_some(), &format!("{at} needs answer_bool."))?,
                    "numeric" => {
                        require(
                            question.answer_number.map(f64::is_finite).unwrap_or(false),
                            &format!("{at} needs a finite answer_number."),
                        )?;
                        require(
                            question.tolerance.map(|t| t.is_finite() && t >= 0.0).unwrap_or(true),
                            &format!("{at} tolerance must be zero or positive."),
                        )?;
                    }
                    "short_answer" => require(
                        question.accepted_answers.iter().any(|a| !a.trim().is_empty()),
                        &format!("{at} needs at least one accepted answer."),
                    )?,
                    other => return Err(format!("{at} unknown question type \"{other}\". Call the tool again with corrected input.")),
                }
            }
            let topic = if q.topic.trim().is_empty() { "this topic".to_string() } else { q.topic.clone() };
            Ok(format!(
                "Rendered the quiz ({} questions on {topic}). The user answers it in the chat and sees feedback when they submit; attempts are saved to their progress.",
                q.questions.len()
            ))
        }
        "render_worked_example" => {
            let w: WorkedExampleInput = parse(input)?;
            require(!w.problem.trim().is_empty(), "problem is empty.")?;
            require((1..=15).contains(&w.steps.len()), "A worked example needs 1–15 steps.")?;
            require(w.steps.iter().all(|s| !s.title.trim().is_empty() && !s.detail.trim().is_empty()), "Every step needs a title and detail.")?;
            require(!w.answer.trim().is_empty(), "answer is empty.")?;
            Ok(format!("Rendered the worked example ({} steps). The user reveals the steps one at a time.", w.steps.len()))
        }
        "render_lesson_plan" => {
            let l: LessonPlanInput = parse(input)?;
            require(l.duration_minutes.is_finite() && l.duration_minutes > 0.0, "duration_minutes must be positive.")?;
            require(!l.objectives.is_empty() && l.objectives.iter().all(|o| !o.text.trim().is_empty()), "Include at least one objective.")?;
            require(!l.sections.is_empty(), "Include at least one section.")?;
            require(l.sections.iter().all(|s| !s.title.trim().is_empty() && s.minutes.is_finite() && s.minutes >= 0.0), "Every section needs a title and non-negative minutes.")?;
            let total: f64 = l.sections.iter().map(|s| s.minutes).sum();
            let note = if (total - l.duration_minutes).abs() > 0.5 {
                format!(" Note: the sections add up to {total} minutes, not {}; mention this to the instructor.", l.duration_minutes)
            } else {
                String::new()
            };
            Ok(format!("Rendered the lesson plan.{note}"))
        }
        "render_rubric" => {
            let r: RubricInput = parse(input)?;
            require((2..=6).contains(&r.levels.len()), "A rubric needs 2–6 levels.")?;
            require(r.levels.iter().all(|l| !l.label.trim().is_empty() && l.points.is_finite()), "Every level needs a label and numeric points.")?;
            require(!r.criteria.is_empty() && r.criteria.len() <= 15, "A rubric needs 1–15 criteria.")?;
            for c in &r.criteria {
                require(!c.name.trim().is_empty(), "Every criterion needs a name.")?;
                require(
                    c.descriptors.len() == r.levels.len(),
                    &format!("Criterion \"{}\" has {} descriptors but there are {} levels.", c.name, c.descriptors.len(), r.levels.len()),
                )?;
            }
            let weights: Vec<f64> = r.criteria.iter().filter_map(|c| c.weight_percent).collect();
            let note = if weights.len() == r.criteria.len() && (weights.iter().sum::<f64>() - 100.0).abs() > 0.5 {
                format!(" Note: the weights add up to {}%, not 100%.", weights.iter().sum::<f64>())
            } else {
                String::new()
            };
            Ok(format!("Rendered the rubric.{note}"))
        }
        other => Err(format!("Unknown render tool {other}.")),
    }
}

pub fn validate_choice(input: &Value) -> Result<ChoiceInput, String> {
    let c: ChoiceInput = parse(input)?;
    require(!c.question.trim().is_empty(), "question is empty.")?;
    require((2..=6).contains(&c.options.len()), "Offer 2–6 options.")?;
    require(c.options.iter().all(|o| !o.label.trim().is_empty()), "Every option needs a label.")?;
    Ok(c)
}

// ---- Execution ------------------------------------------------------------

pub fn execute(ctx: &ToolContext, name: &str, input: &Value) -> ToolOutcome {
    match run(ctx, name, input) {
        Ok(outcome) => outcome,
        Err(e) => ToolOutcome::err(format!("The tool failed: {e}")),
    }
}

fn run(ctx: &ToolContext, name: &str, input: &Value) -> AppResult<ToolOutcome> {
    if name == "create_flashcards" {
        if let Err(e) = validate_flashcards(input) {
            return Ok(ToolOutcome::err(e));
        }
        let f: FlashcardsInput = serde_json::from_value(input.clone())?;
        let cards: Vec<CardInput> = f
            .cards
            .iter()
            .map(|c| CardInput { front: c.front.clone(), back: c.back.clone(), hint: c.hint.clone() })
            .collect();
        let deck = ctx.db.with(|c| {
            repo::decks::create(c, ctx.course_id, Some(ctx.conversation_id), &f.deck_title, &f.description, &cards)
        })?;
        return Ok(ToolOutcome::ok(
            json!({
                "status": "saved",
                "deck_id": deck.id,
                "deck_title": deck.title,
                "card_count": deck.card_count,
                "message": "The deck is saved and the user can study it now in the chat or in the Review tab with spaced repetition."
            })
            .to_string(),
        ));
    }
    if RENDER_TOOLS.contains(&name) {
        return Ok(match validate_render(name, input) {
            Ok(msg) => ToolOutcome::ok(msg),
            Err(e) => ToolOutcome::err(e),
        });
    }
    match name {
        "search_course_materials" => {
            let s: SearchInput = match parse(input) {
                Ok(s) => s,
                Err(e) => return Ok(ToolOutcome::err(e)),
            };
            let Some(course_id) = ctx.course_id else {
                return Ok(ToolOutcome::ok(
                    json!({ "results": [], "note": "No course is selected, so there are no course materials to search. Answer from general knowledge and say so." }).to_string(),
                ));
            };
            let hits = ctx.db.with(|c| repo::materials::search(c, course_id, &s.query, s.limit.unwrap_or(5).clamp(1, 8)))?;
            let results: Vec<Value> = hits
                .iter()
                .map(|h| json!({ "title": h.title, "passage": h.content, "excerpt": h.snippet, "material_id": h.material_id, "section": h.ordinal + 1 }))
                .collect();
            let note = if results.is_empty() {
                "No passages matched. Tell the user the course materials do not seem to cover this, or try different key terms."
            } else {
                "Cite passages by their title. Only these passages are from the course materials."
            };
            Ok(ToolOutcome::ok(json!({ "query": s.query, "results": results, "note": note }).to_string()))
        }
        "get_learner_progress" => {
            let p: ProgressInput = parse(input).unwrap_or(ProgressInput { topic: None });
            let summary = ctx.db.with(|c| repo::progress::summary_for_model(c, ctx.course_id, p.topic.as_deref()))?;
            Ok(ToolOutcome::ok(summary.to_string()))
        }
        "plan_visualization" => Ok(ToolOutcome::ok("Plan noted. Build it now.")),
        _ => Ok(ToolOutcome::err(format!("There is no tool named {name}."))),
    }
}

fn validate_flashcards(input: &Value) -> Result<(), String> {
    let f: FlashcardsInput = parse(input)?;
    require(!f.deck_title.trim().is_empty(), "deck_title is empty.")?;
    require((1..=60).contains(&f.cards.len()), "A deck needs 1–60 cards.")?;
    require(
        f.cards.iter().all(|c| !c.front.trim().is_empty() && !c.back.trim().is_empty()),
        "Every card needs text on both sides.",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn definitions_vary_by_mode() {
        let learn: Vec<String> = definitions(Mode::Learn).iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        let teach: Vec<String> = definitions(Mode::Teach).iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert!(learn.contains(&"get_learner_progress".to_string()));
        assert!(!learn.contains(&"render_rubric".to_string()));
        assert!(teach.contains(&"render_lesson_plan".to_string()));
        assert!(definitions(Mode::Learn).iter().all(|t| t["input_schema"]["type"] == "object"));
    }

    #[test]
    fn quiz_validation_catches_bad_answers() {
        let good = json!({"title": "Q", "topic": "T", "purpose": "practice", "questions": [
            {"type": "multiple_choice", "prompt": "p", "options": ["a", "b", "c"], "answer_index": 2, "explanation": "e"},
            {"type": "numeric", "prompt": "p", "answer_number": 3.5, "tolerance": 0.1, "explanation": "e"}
        ]});
        assert!(validate_render("render_quiz", &good).is_ok());
        let bad = json!({"title": "Q", "topic": "T", "purpose": "practice", "questions": [
            {"type": "multiple_choice", "prompt": "p", "options": ["a", "b"], "answer_index": 5, "explanation": "e"}
        ]});
        let err = validate_render("render_quiz", &bad).unwrap_err();
        assert!(err.contains("answer_index"));
    }

    #[test]
    fn table_rows_must_match_columns() {
        let bad = json!({"title": "T", "columns": ["a", "b"], "rows": [["1"]], "source": "s"});
        assert!(validate_render("render_table", &bad).unwrap_err().contains("Row 1"));
        let ok = json!({"title": "T", "columns": ["a", "b"], "rows": [["1", 2]], "source": "s"});
        assert!(validate_render("render_table", &ok).is_ok());
    }

    #[test]
    fn rubric_descriptors_match_levels() {
        let bad = json!({"title": "R", "levels": [{"label": "A", "points": 2}, {"label": "B", "points": 1}],
            "criteria": [{"name": "C", "descriptors": ["only one"]}]});
        assert!(validate_render("render_rubric", &bad).is_err());
    }

    #[test]
    fn interactive_rejects_style_blocks_in_html() {
        let bad = json!({"title": "x", "initialHeight": 300, "placeholderMessages": [], "css": "", "html": "<style>p{}</style><p>x</p>", "jsFunctions": "", "jsExpressions": []});
        assert!(validate_render("generate_interactive", &bad).is_err());
    }

    #[test]
    fn flashcards_are_saved() {
        let db = Db::open_in_memory().unwrap();
        let conv = db.with(|c| repo::conversations::create(c, None, Mode::Learn, crate::models::Style::Direct)).unwrap();
        let ctx = ToolContext { db: &db, course_id: None, conversation_id: &conv.id };
        let out = execute(&ctx, "create_flashcards", &json!({"deck_title": "D", "cards": [{"front": "f", "back": "b"}]}));
        assert!(!out.is_error, "{}", out.content);
        let v: Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["card_count"], 1);
        let bad = execute(&ctx, "create_flashcards", &json!({"deck_title": "D", "cards": []}));
        assert!(bad.is_error);
    }

    #[test]
    fn search_without_course_explains_itself() {
        let db = Db::open_in_memory().unwrap();
        let ctx = ToolContext { db: &db, course_id: None, conversation_id: "x" };
        let out = execute(&ctx, "search_course_materials", &json!({"query": "entropy"}));
        assert!(!out.is_error);
        assert!(out.content.contains("No course is selected"));
    }
}
