//! Converts rendered items into files instructors and students can take
//! elsewhere: Markdown, Moodle GIFT, CSV, and Anki-compatible TSV.

use serde_json::Value;

use crate::error::{AppError, AppResult};

pub struct Exported {
    pub contents: String,
    pub extension: &'static str,
}

pub fn export(kind: &str, format: &str, payload: &Value) -> AppResult<Exported> {
    let out = match (kind, format) {
        ("quiz", "markdown") => md(quiz_markdown(payload, false)),
        ("quiz", "markdown_key") => md(quiz_markdown(payload, true)),
        ("quiz", "gift") => Exported { contents: quiz_gift(payload), extension: "gift.txt" },
        ("rubric", "markdown") => md(rubric_markdown(payload)),
        ("rubric", "csv") => Exported { contents: rubric_csv(payload), extension: "csv" },
        ("lesson_plan", "markdown") => md(lesson_markdown(payload)),
        ("worked_example", "markdown") => md(worked_markdown(payload)),
        ("table", "csv") => Exported { contents: table_csv(payload), extension: "csv" },
        ("table", "markdown") => md(table_markdown(payload)),
        ("flashcards", "tsv") => Exported { contents: cards_tsv(payload), extension: "tsv" },
        _ => return Err(AppError::invalid(format!("Can't export {kind} as {format}."))),
    };
    Ok(out)
}

fn md(contents: String) -> Exported {
    Exported { contents, extension: "md" }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn num(v: &Value) -> String {
    match v.as_f64() {
        Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", n as i64),
        Some(n) => format!("{n}"),
        None => String::new(),
    }
}

fn quiz_markdown(q: &Value, with_key: bool) -> String {
    let mut out = format!("# {}\n\n", s(q, "title"));
    if !s(q, "topic").is_empty() {
        out.push_str(&format!("Topic: {}\n\n", s(q, "topic")));
    }
    let letters = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
    for (i, item) in arr(q, "questions").iter().enumerate() {
        out.push_str(&format!("**{}.** {}\n\n", i + 1, s(item, "prompt")));
        match s(item, "type") {
            "multiple_choice" => {
                for (j, opt) in arr(item, "options").iter().enumerate() {
                    out.push_str(&format!("- {}. {}\n", letters.get(j).copied().unwrap_or('?'), opt.as_str().unwrap_or_default()));
                }
                out.push('\n');
            }
            "true_false" => out.push_str("- True\n- False\n\n"),
            "numeric" => out.push_str(&format!("Answer: ________ {}\n\n", s(item, "unit"))),
            _ => out.push_str("Answer: ______________________\n\n"),
        }
    }
    if with_key {
        out.push_str("---\n\n## Answer key\n\n");
        for (i, item) in arr(q, "questions").iter().enumerate() {
            let answer = match s(item, "type") {
                "multiple_choice" => {
                    let idx = item["answer_index"].as_u64().unwrap_or(0) as usize;
                    let text = arr(item, "options").get(idx).and_then(Value::as_str).unwrap_or_default();
                    format!("{}. {}", letters.get(idx).copied().unwrap_or('?'), text)
                }
                "true_false" => if item["answer_bool"].as_bool().unwrap_or(false) { "True".into() } else { "False".into() },
                "numeric" => {
                    let tol = item["tolerance"].as_f64().unwrap_or(0.0);
                    let mut a = format!("{} {}", num(&item["answer_number"]), s(item, "unit"));
                    if tol > 0.0 {
                        a.push_str(&format!(" (± {})", num(&item["tolerance"])));
                    }
                    a.trim().to_string()
                }
                _ => arr(item, "accepted_answers").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" / "),
            };
            out.push_str(&format!("{}. **{}**", i + 1, answer));
            if !s(item, "bloom").is_empty() {
                out.push_str(&format!(" (Bloom: {})", s(item, "bloom")));
            }
            out.push_str(&format!("\n   {}\n", s(item, "explanation")));
        }
    }
    out
}

/// Escapes GIFT's special characters.
fn gift_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '~' | '=' | '#' | '{' | '}' | ':') {
            out.push('\\');
        }
        out.push(c);
    }
    out.replace('\n', " ")
}

/// Moodle GIFT format, importable into Moodle and several other LMSs.
fn quiz_gift(q: &Value) -> String {
    let mut out = format!("// {}\n// Exported from Explorable\n\n", s(q, "title").replace('\n', " "));
    if !s(q, "topic").is_empty() {
        out.push_str(&format!("$CATEGORY: {}\n\n", gift_escape(s(q, "topic"))));
    }
    for (i, item) in arr(q, "questions").iter().enumerate() {
        let name = format!("Q{}", i + 1);
        let prompt = gift_escape(s(item, "prompt"));
        let feedback = gift_escape(s(item, "explanation"));
        match s(item, "type") {
            "multiple_choice" => {
                let idx = item["answer_index"].as_u64().unwrap_or(0) as usize;
                let fb = arr(item, "option_feedback");
                out.push_str(&format!("::{name}:: {prompt} {{\n"));
                for (j, opt) in arr(item, "options").iter().enumerate() {
                    let mark = if j == idx { '=' } else { '~' };
                    out.push_str(&format!("  {mark}{}", gift_escape(opt.as_str().unwrap_or_default())));
                    if let Some(f) = fb.get(j).and_then(Value::as_str) {
                        out.push_str(&format!("#{}", gift_escape(f)));
                    }
                    out.push('\n');
                }
                out.push_str(&format!("  ####{feedback}\n}}\n\n"));
            }
            "true_false" => {
                let t = if item["answer_bool"].as_bool().unwrap_or(false) { "TRUE" } else { "FALSE" };
                out.push_str(&format!("::{name}:: {prompt} {{{t}####{feedback}}}\n\n"));
            }
            "numeric" => {
                let tol = item["tolerance"].as_f64().filter(|t| *t > 0.0).map(|_| num(&item["tolerance"])).unwrap_or_else(|| "0".into());
                out.push_str(&format!("::{name}:: {prompt} {{#{}:{tol}####{feedback}}}\n\n", num(&item["answer_number"])));
            }
            _ => {
                let answers: Vec<String> = arr(item, "accepted_answers")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|a| format!("={}", gift_escape(a)))
                    .collect();
                out.push_str(&format!("::{name}:: {prompt} {{{} ####{feedback}}}\n\n", answers.join(" ")));
            }
        }
    }
    out
}

fn csv_cell(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn rubric_markdown(r: &Value) -> String {
    let levels = arr(r, "levels");
    let mut out = format!("# {}\n\n", s(r, "title"));
    if !s(r, "assignment").is_empty() {
        out.push_str(&format!("Assignment: {}\n\n", s(r, "assignment")));
    }
    out.push_str("| Criterion |");
    for l in levels {
        out.push_str(&format!(" {} ({}) |", s(l, "label"), num(&l["points"])));
    }
    out.push_str("\n|---|");
    out.push_str(&"---|".repeat(levels.len()));
    out.push('\n');
    for c in arr(r, "criteria") {
        let mut name = s(c, "name").to_string();
        if c["weight_percent"].is_number() {
            name.push_str(&format!(" ({}%)", num(&c["weight_percent"])));
        }
        out.push_str(&format!("| **{}** |", name.replace('|', "\\|")));
        for d in arr(c, "descriptors") {
            out.push_str(&format!(" {} |", d.as_str().unwrap_or_default().replace('|', "\\|")));
        }
        out.push('\n');
    }
    if !s(r, "notes").is_empty() {
        out.push_str(&format!("\n{}\n", s(r, "notes")));
    }
    out
}

fn rubric_csv(r: &Value) -> String {
    let levels = arr(r, "levels");
    let mut header = vec!["Criterion".to_string(), "Weight (%)".to_string()];
    header.extend(levels.iter().map(|l| format!("{} ({})", s(l, "label"), num(&l["points"]))));
    let mut out = header.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(",") + "\n";
    for c in arr(r, "criteria") {
        let mut row = vec![csv_cell(s(c, "name")), num(&c["weight_percent"])];
        row.extend(arr(c, "descriptors").iter().map(|d| csv_cell(d.as_str().unwrap_or_default())));
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

fn bullet_list(items: &[Value]) -> String {
    items.iter().filter_map(Value::as_str).map(|i| format!("- {i}\n")).collect()
}

fn lesson_markdown(l: &Value) -> String {
    let mut out = format!("# {}\n\n", s(l, "title"));
    let mut facts = Vec::new();
    if !s(l, "level").is_empty() {
        facts.push(format!("Level: {}", s(l, "level")));
    }
    facts.push(format!("Duration: {} minutes", num(&l["duration_minutes"])));
    out.push_str(&facts.join("  \n"));
    out.push_str("\n\n");
    if !s(l, "overview").is_empty() {
        out.push_str(&format!("{}\n\n", s(l, "overview")));
    }
    out.push_str("## Learning objectives\n\nBy the end of the session, students will be able to:\n\n");
    for o in arr(l, "objectives") {
        let bloom = s(o, "bloom");
        out.push_str(&format!("- {}{}\n", s(o, "text"), if bloom.is_empty() { String::new() } else { format!(" *({bloom})*") }));
    }
    for (title, key) in [("Prerequisites", "prerequisites"), ("Materials", "materials")] {
        if !arr(l, key).is_empty() {
            out.push_str(&format!("\n## {title}\n\n{}", bullet_list(arr(l, key))));
        }
    }
    out.push_str("\n## Session outline\n\n");
    let mut clock = 0.0;
    for sec in arr(l, "sections") {
        let mins = sec["minutes"].as_f64().unwrap_or(0.0);
        out.push_str(&format!(
            "### {}–{} min · {}{}\n\n{}\n",
            clock as i64,
            (clock + mins) as i64,
            s(sec, "title"),
            if s(sec, "kind").is_empty() { String::new() } else { format!(" ({})", s(sec, "kind")) },
            s(sec, "description")
        ));
        if !s(sec, "instructor_notes").is_empty() {
            out.push_str(&format!("\n> Instructor note: {}\n", s(sec, "instructor_notes")));
        }
        out.push('\n');
        clock += mins;
    }
    for (title, key) in [("Assessment", "assessment"), ("Differentiation", "differentiation"), ("Follow-up", "follow_up")] {
        if !s(l, key).is_empty() {
            out.push_str(&format!("## {title}\n\n{}\n\n", s(l, key)));
        }
    }
    out
}

fn worked_markdown(w: &Value) -> String {
    let mut out = format!("# {}\n\n**Problem.** {}\n\n", s(w, "title"), s(w, "problem"));
    for (i, step) in arr(w, "steps").iter().enumerate() {
        out.push_str(&format!("**Step {}: {}**\n\n{}\n\n", i + 1, s(step, "title"), s(step, "detail")));
    }
    out.push_str(&format!("**Answer.** {}\n", s(w, "answer")));
    if w["practice_problem"].is_object() {
        let p = &w["practice_problem"];
        out.push_str(&format!("\n## Try one\n\n{}\n\n<details><summary>Answer</summary>\n\n{}\n\n</details>\n", s(p, "prompt"), s(p, "answer")));
    }
    out
}

fn table_csv(t: &Value) -> String {
    let mut out = arr(t, "columns").iter().map(|c| csv_cell(&cell_text(c))).collect::<Vec<_>>().join(",") + "\n";
    for row in arr(t, "rows") {
        let cells: Vec<String> = row.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(|c| csv_cell(&cell_text(c))).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

fn table_markdown(t: &Value) -> String {
    let cols = arr(t, "columns");
    let esc = |v: &Value| cell_text(v).replace('|', "\\|").replace('\n', " ");
    let mut out = format!("# {}\n\n| {} |\n|{}\n", s(t, "title"), cols.iter().map(esc).collect::<Vec<_>>().join(" | "), "---|".repeat(cols.len()));
    for row in arr(t, "rows") {
        let cells: Vec<String> = row.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(esc).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.push_str(&format!("\nSource: {}\n", s(t, "source")));
    out
}

/// Tab-separated front/back, the format Anki's importer accepts directly.
fn cards_tsv(d: &Value) -> String {
    let clean = |t: &str| t.replace(['\t', '\n', '\r'], " ");
    let mut out = String::from("#separator:tab\n#html:false\n");
    for c in arr(d, "cards") {
        let mut back = clean(s(c, "back"));
        if !s(c, "hint").is_empty() {
            back.push_str(&format!(" (Hint: {})", clean(s(c, "hint"))));
        }
        out.push_str(&format!("{}\t{}\n", clean(s(c, "front")), back));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn quiz() -> Value {
        json!({"title": "Q", "topic": "Cells: basics", "questions": [
            {"type": "multiple_choice", "prompt": "Pick {one}", "options": ["a=1", "b"], "answer_index": 1, "option_feedback": ["no", "yes"], "explanation": "Because #reasons"},
            {"type": "true_false", "prompt": "T?", "answer_bool": true, "explanation": "e"},
            {"type": "numeric", "prompt": "N?", "answer_number": 2.5, "tolerance": 0.1, "unit": "m", "explanation": "e"},
            {"type": "short_answer", "prompt": "S?", "accepted_answers": ["oxygen", "O2"], "explanation": "e"}
        ]})
    }

    #[test]
    fn gift_escapes_and_marks_answers() {
        let g = quiz_gift(&quiz());
        assert!(g.contains("$CATEGORY: Cells\\: basics"));
        assert!(g.contains("::Q1:: Pick \\{one\\} {"));
        assert!(g.contains("~a\\=1#no"));
        assert!(g.contains("=b#yes"));
        assert!(g.contains("####Because \\#reasons"));
        assert!(g.contains("{TRUE####e}"));
        assert!(g.contains("{#2.5:0.1####e}"));
        assert!(g.contains("{=oxygen =O2 ####e}"));
    }

    #[test]
    fn quiz_markdown_key_is_optional() {
        let plain = quiz_markdown(&quiz(), false);
        assert!(!plain.contains("Answer key"));
        let keyed = quiz_markdown(&quiz(), true);
        assert!(keyed.contains("1. **B. b**"));
        assert!(keyed.contains("2.5 m (± 0.1)"));
    }

    #[test]
    fn csv_quotes_special_cells() {
        let t = json!({"title": "T", "columns": ["a", "b"], "rows": [["x, y", "say \"hi\""], [1, true]], "source": "s"});
        assert_eq!(table_csv(&t), "a,b\n\"x, y\",\"say \"\"hi\"\"\"\n1,true\n");
    }

    #[test]
    fn rubric_and_lesson_render() {
        let r = json!({"title": "R", "levels": [{"label": "High", "points": 4}, {"label": "Low", "points": 1}],
            "criteria": [{"name": "Thesis", "weight_percent": 60, "descriptors": ["clear", "missing"]}]});
        assert!(rubric_markdown(&r).contains("| **Thesis (60%)** | clear | missing |"));
        assert!(rubric_csv(&r).starts_with("Criterion,Weight (%),High (4),Low (1)\nThesis,60,clear,missing"));
        let l = json!({"title": "L", "duration_minutes": 20, "objectives": [{"text": "Explain X", "bloom": "understand"}],
            "sections": [{"title": "Intro", "minutes": 5, "description": "d"}, {"title": "Work", "minutes": 15, "kind": "activity", "description": "d2"}],
            "assessment": "Exit ticket"});
        let md = lesson_markdown(&l);
        assert!(md.contains("### 5–20 min · Work (activity)"));
        assert!(md.contains("## Assessment"));
    }

    #[test]
    fn tsv_has_anki_headers() {
        let d = json!({"cards": [{"front": "a\tb", "back": "c", "hint": "h"}]});
        assert_eq!(cards_tsv(&d), "#separator:tab\n#html:false\na b\tc (Hint: h)\n");
        assert!(export("quiz", "pdf", &quiz()).is_err());
    }
}
