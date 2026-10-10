# Explorable — Plan

A desktop app for higher education, built with Rust, Tauri 2 and SQLite, that gives
*answers you can interact with*. It serves two kinds of user: students who are learning
and instructors who are teaching. For each question the agent picks how the answer is
presented.

## 1. Modes and intents

Each mode has its own intents:

| Learn mode (student) | Teach mode (instructor) |
| --- | --- |
| **Understand**: interactive explainers, simulations, diagrams | **Demonstrate**: classroom-ready interactive demos with a full-screen *Present* view |
| **Practice**: quizzes with instant feedback; flashcards with spaced repetition | **Assess**: question banks with answer keys and Bloom levels, exportable to Moodle GIFT or Markdown |
| **Work through**: worked examples revealed one step at a time | **Plan**: lesson plans with measurable objectives and timed activities; rubrics |
| **Track**: progress by topic, review streaks, cards due | **Ground**: course materials the assistant searches and cites |

Other education-specific behaviour:

- **Courses** are workspaces. Each holds chats, materials, saved items and decks, plus a level and discipline that the tutor adapts to.
- **Tutoring style.** *Socratic* (guide with questions; don't hand over answers to graded work) or *Direct*.
- **Grounding.** Imported notes, syllabi and readings are chunked into an SQLite FTS5 index. The `search_course_materials` tool returns passages and the tutor cites them by title.
- **Academic integrity and provenance.** The prompt forbids fabricated citations, DOIs or quotes, requires labels on illustrative numbers, and requires the assumptions of simplified models to be stated.
- **Math.** LaTeX in answers is rendered with KaTeX. KaTeX and math.js are added to the sandbox importmap, and math.js lets calculators avoid `eval`.
- **Durable state.** Everything lives in SQLite: conversations, saved items, decks, review history and quiz attempts.

## 2. Architecture

```
┌───────────────────────────── Tauri window (WebView) ─────────────────────────────┐
│ React + TypeScript UI                                                            │
│  Sidebar · Chat · Library · Review · Progress · Materials · Settings             │
│  Renderers: Markdown+KaTeX │ Quiz │ Flashcards │ WorkedExample │ LessonPlan │    │
│             Rubric │ Table │ ChoicePrompt (HITL) │ InteractiveFrame (sandbox)    │
│                                   ▲ Channel<ChatEvent> (ordered stream)          │
└───────────────────────────────────┼──────────────────────────────────────────────┘
                     invoke(...)    │
┌───────────────────────────────────┴───── Rust core (src-tauri) ──────────────────┐
│ commands/  courses · chats · library · decks · materials · progress · settings   │
│ agent/     run loop · router · tools · prompt · events                           │
│ llm/       Anthropic Messages API (raw HTTP + SSE) · scripted demo provider      │
│ partial_json (tolerant streaming parser) · srs (SM-2) · export (MD/GIFT/TSV)     │
│ db/        rusqlite (bundled, FTS5) · migrations via PRAGMA user_version         │
│ secrets    OS keychain (API key never stored in SQLite)                          │
└──────────────────────────────────────────────────────────────────────────────────┘
```

**Why the UI is TypeScript.** Tauri renders its UI in a webview. The generative-UI sandbox is
DOM- and iframe-heavy (morphing previews, postMessage bridges, script injection), and a
WASM Rust UI would wrap those same browser APIs through `web_sys` while adding a second
toolchain. All application logic is in Rust: the agent loop, routing, tool execution and
validation, streaming JSON parsing, persistence, retrieval, scheduling and export. The
webview only renders.

### 2.1 Agent turn (Rust)

1. Persist the user message. Any unresolved `tool_use` from an interrupted run is answered with an error `tool_result` in the same message, so the history is append-only and stays valid.
2. **Route.** A small model (`claude-haiku-4-5`, using structured output `output_config.format` with a JSON schema) picks the presentation from the last four text messages. The decision is stored with the message and shown as a chip on the answer. If the router fails, the UI says the tutor chose the format itself. The failure is never hidden.
3. **Prompt.** The cached static system block holds the identity, playbook, sandbox contract and component guidance. A dynamic block holds the mode, style, course, materials list and route decision. Tools are fixed per conversation mode, which keeps the prompt cache warm. Routing is enforced through instructions, not by changing the tool list.
4. **Stream** `POST /v1/messages` with `stream: true`, adaptive thinking (summarized display), explicit `effort`, `eager_input_streaming` on every client tool, and `fallbacks: "default"` for refusals. SSE events are forwarded to the UI. Tool input deltas pass through the partial-JSON parser and reach the UI as throttled snapshots (`partial`, `completeKeys`).
5. At `content_block_stop`, tool input is parsed strictly and deserialized into typed Rust structs. If that fails, the model receives an `is_error` result and can retry. `max_tokens` and `refusal` stop reasons are checked before any tool runs.
6. **Execute tools.** Server tools (search, progress, saving decks) touch SQLite. Render tools return "Rendered" confirmations. `ask_choice` pauses the run until the user picks.
7. Loop until no tool calls remain, with at most 8 iterations. Cancellation uses a `CancellationToken`, and partial text is kept.

### 2.2 Presentations → renderers

| Router choice | Tool / renderer |
| --- | --- |
| text | Markdown with KaTeX and code highlighting |
| table | `render_table` (accessible table with a required provenance line) |
| chart · diagram · interactive_explainer · simulation · calculator | `generate_interactive` (sandboxed streaming UI) |
| worked_example | `render_worked_example` (steps revealed one at a time, plus "try one yourself") |
| quiz | `render_quiz` (MCQ, true/false, numeric with tolerance, short answer; attempts saved) |
| flashcards | `create_flashcards` (deck saved to SQLite and scheduled with SM-2) |
| lesson_plan *(teach)* | `render_lesson_plan` |
| rubric *(teach)* | `render_rubric` |

Supporting tools: `search_course_materials`, `get_learner_progress` *(learn)*,
`ask_choice` (human-in-the-loop) and `plan_visualization` (optional).

### 2.3 Sandbox

- `<iframe sandbox="allow-scripts">` with `srcdoc`, which gives an opaque origin: no storage, no parent DOM, no Tauri IPC (the invoke key is injected into the main frame only). A per-frame CSP allows scripts only from esm.sh, jsDelivr, unpkg and cdnjs, and `connect-src` excludes the IPC endpoints.
- **Preview phase.** After `css` completes, partial `html` (with incomplete tags and entities stripped) is morphed into the preview frame with Idiomorph. When `html` completes, the final frame boots in the background and replaces the preview once it reports ready, so there is no flash.
- `jsFunctions` and each complete `jsExpressions` element run as classic scripts, in order, exactly once per tool call (keyed by `tool_use.id`).
- **Bridge** `window.edu`: `sendPrompt({text})` (requires transient user activation inside the frame; the host validates 1–4000 chars and refuses during a run), `openLink({url})` (https only, re-validated in Rust, opened in the system browser) and resize reports clamped to 60–4000 px.
- **Theme.** Tokens are scoped by `:root[data-theme]`. The host sends theme changes to every live frame, and generated code can listen for a `themechange` event.
- **Runtime errors** are reported to the host and shown with an *Ask to fix* action, which sends the error text as a follow-up.
- **Export** writes a standalone HTML file with the bridge stubbed out.

## 3. Data model (SQLite)

`settings` · `courses` · `conversations` (mode, pending HITL state) · `messages` (Anthropic content
blocks as JSON, with metadata for route, model, usage and stop reason) · `artifacts` (saved items: kind,
title, payload) · `materials` + `material_chunks` (FTS5, porter tokenizer) · `decks` · `cards`
(SM-2: ease, interval, reps, lapses, due) · `reviews` · `quiz_attempts` (score and per-question results by topic).
Deleting a course cascades to everything it owns. Migrations are applied by `PRAGMA user_version`.

## 4. Design

**Subject:** a study and teaching workspace for university courses. **Audience:** students and
instructors at a desk, often for long sessions. **Job:** ask, then explore, practise or prepare.

- **Palette, light:** canvas `#F5F6F8` (cool paper), surface `#FFFFFF`, ink `#18213A`
  (blue-black fountain-pen ink), graphite `#5B6478`, rule `#DDE2EA`, ink blue `#2D4FD6`.
  **Dark:** canvas `#10141C`, surface `#171C27`, ink `#E7EAF0`, graphite `#9BA4B6`, rule `#2A3142`,
  ink blue `#8FA6FF`.
- **Mode instruments**, the one bold element: Learn mode uses a **highlighter** (`#F2DC3D`, always
  a translucent marker stroke behind text, never text colour). Teach mode uses **chalk** on a
  chalkboard green (`#2F6B57` / `#6FC3A2`). The active nav item, the mode switch, a selected answer and
  key terms all get the marker treatment. Everything else stays quiet: hairline rules, few shadows.
- **Type:** *Atkinson Hyperlegible Next* for the interface and body text. The Braille Institute
  designed it for legibility, which suits accessible courseware. *Literata* (a screen-reading serif)
  carries the textbook voice in answer headings, titles and the welcome line. Base size 15px, ratio about 1.25,
  body line-height 1.6, measure ≤ 72ch.
- **Layout:** sidebar (264px) plus a reading column. Prose stays at textbook measure, while
  *figures* (interactives, tables, quizzes) break out wider, as figures do in a textbook.
- **Avoided defaults:** cream with terracotta, near-black with an acid accent, all-caps eyebrows,
  middle-dot meta strings, monospace micro-labels, identical card grids with one radius and one shadow.
  Radii follow hierarchy: composer 16px, figures 10px, controls 6px.

## 5. Build phases

1. Scaffold the Tauri 2 + Vite + React app, set up the token CSS, theme toggle and the app shell.
2. Rust core: DB and migrations, models and repositories, commands for courses, chats, settings and the keychain.
3. LLM layer: Anthropic SSE client, partial-JSON parser (with tests), scripted demo provider.
4. Agent: prompt, router, tools, loop, cancellation, HITL, events.
5. Chat UI: streaming store, Markdown/KaTeX, route chip, tool activity, composer, welcome starters.
6. Renderers: InteractiveFrame and sandbox, Quiz, Flashcards, WorkedExample, LessonPlan, Rubric, Table, Choice.
7. Library, Review (SM-2 session), Progress dashboard, Materials (import/search), Settings, exports.
8. Verification: `cargo test`, `cargo clippy`, `tsc`, a production `tauri build`, and browser screenshots
   of the UI against a mocked backend for design review.

## 6. Verification limits (stated up front)

No Anthropic API key is available in the build environment, so live model calls cannot be run
here. The SSE client is checked with recorded event fixtures, and the full streaming pipeline
(router, then tokens, then streamed tool arguments, then the sandbox) runs end to end through
the scripted **demo provider**, which the UI labels as scripted. A real tutoring
conversation must be tested with a real key.

## 7. Addendum: more providers and accessibility (October 10)

Added after the first build, at the user's request.

- **OpenAI and Gemini**, alongside Claude. Each provider adapter translates the canonical history into its own request format and translates its stream back into canonical events, so the agent loop, tools, and UI are unchanged.
  - **OpenAI** uses the Responses API, the only API where current models support tool calling. It runs stateless, with encrypted reasoning round-tripped.
  - **Gemini** uses `streamGenerateContent`, with thought signatures round-tripped.
  - Model IDs were checked against each provider's documentation on October 10, 2026.
- **Accessibility:** text size (whole-webview zoom), increased contrast, reduced motion (also answered to generated code's `prefers-reduced-motion` queries), relaxed spacing, reading font, and read-aloud with spoken math. A quick menu sits next to a simplified one-button light/dark toggle.
