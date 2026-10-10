//! The agent loop: route the turn, stream the model, run tools, repeat.

pub mod events;
pub mod history;
pub mod prompt;
pub mod router;
pub mod tools;

#[cfg(test)]
mod e2e_tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use crate::db::{new_id, Db};
use crate::error::{AppError, AppResult};
use crate::llm::{Provider, TurnRequest};
use crate::models::{Mode, PendingChoice, Settings, StoredMessage};
use crate::partial_json::parse_partial_object;
use crate::repo;
use crate::secrets::Secrets;
use crate::text::provisional_title;
use events::{ChatEvent, RouteDecision};

const MAX_ITERATIONS: usize = 8;
const MAX_OUTPUT_TOKENS: u32 = 64_000;
const TOOL_INPUT_THROTTLE: Duration = Duration::from_millis(80);
const MAX_USER_TEXT: usize = 20_000;

/// Tracks the active run per conversation so it can be cancelled and so two
/// runs never interleave in one history.
#[derive(Default)]
pub struct RunRegistry {
    runs: Mutex<HashMap<String, CancellationToken>>,
}

impl RunRegistry {
    fn begin(&self, conversation_id: &str) -> Option<CancellationToken> {
        let mut runs = self.runs.lock().ok()?;
        if runs.contains_key(conversation_id) {
            return None;
        }
        let token = CancellationToken::new();
        runs.insert(conversation_id.to_string(), token.clone());
        Some(token)
    }

    fn end(&self, conversation_id: &str) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(conversation_id);
        }
    }

    pub fn cancel(&self, conversation_id: &str) -> bool {
        match self.runs.lock() {
            Ok(runs) => runs.get(conversation_id).map(|t| t.cancel()).is_some(),
            Err(_) => false,
        }
    }

    pub fn is_running(&self, conversation_id: &str) -> bool {
        self.runs.lock().map(|r| r.contains_key(conversation_id)).unwrap_or(false)
    }
}

pub enum TurnInput {
    Text(String),
    Choice { tool_use_id: String, answer: String },
    Retry,
}

#[derive(Clone)]
pub struct Deps {
    pub db: Arc<Db>,
    pub http: reqwest::Client,
    pub secrets: Arc<Secrets>,
    pub runs: Arc<RunRegistry>,
}

#[derive(Clone)]
struct Emitter(Channel<ChatEvent>);

impl Emitter {
    fn emit(&self, event: ChatEvent) {
        // The window may have closed; the run still completes and persists.
        let _ = self.0.send(event);
    }
}

enum RunEnd {
    Done,
    Cancelled,
    Failed { message: String, retryable: bool },
}

/// Validates and persists the user's input, then runs the agent in the
/// background, streaming events over `channel`. Returns the run ID.
pub fn start(deps: Deps, conversation_id: String, input: TurnInput, channel: Channel<ChatEvent>) -> AppResult<String> {
    let token = deps.runs.begin(&conversation_id).ok_or_else(|| {
        AppError::invalid("An answer is still in progress in this chat. Stop it or wait for it to finish.")
    })?;
    let prepared = match prepare(&deps.db, &conversation_id, input) {
        Ok(p) => p,
        Err(e) => {
            deps.runs.end(&conversation_id);
            return Err(e);
        }
    };
    let run_id = new_id();
    let emitter = Emitter(channel);
    emitter.emit(ChatEvent::Started { run_id: run_id.clone(), conversation_id: conversation_id.clone() });
    if let Some(message) = prepared.user_message {
        emitter.emit(ChatEvent::UserMessage { message });
    }
    if let Some(title) = prepared.new_title {
        emitter.emit(ChatEvent::TitleUpdated { conversation_id: conversation_id.clone(), title });
    }

    tauri::async_runtime::spawn(async move {
        let end = run(&deps, &conversation_id, &emitter, &token).await;
        let cancelled = matches!(end, Ok(RunEnd::Cancelled));
        match end {
            Ok(RunEnd::Failed { message, retryable }) => emitter.emit(ChatEvent::Error { message, retryable }),
            Err(e) => emitter.emit(ChatEvent::Error { message: e.to_string(), retryable: true }),
            _ => {}
        }
        deps.runs.end(&conversation_id);
        emitter.emit(ChatEvent::Finished { cancelled });
    });
    Ok(run_id)
}

struct Prepared {
    user_message: Option<StoredMessage>,
    new_title: Option<String>,
}

fn tool_result(id: &str, content: &str, is_error: bool) -> Value {
    let mut v = json!({ "type": "tool_result", "tool_use_id": id, "content": content });
    if is_error {
        v["is_error"] = json!(true);
    }
    v
}

fn prepare(db: &Db, conversation_id: &str, input: TurnInput) -> AppResult<Prepared> {
    db.with_tx(|tx| {
        let conv = repo::conversations::get(tx, conversation_id)?;
        let last = repo::messages::last(tx, conversation_id)?;
        match input {
            TurnInput::Text(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return Err(AppError::invalid("Type a message first."));
                }
                if text.chars().count() > MAX_USER_TEXT {
                    return Err(AppError::invalid(
                        "That message is over 20,000 characters. Import long documents as course materials instead.",
                    ));
                }
                let mut blocks: Vec<Value> = Vec::new();
                if let Some(p) = &conv.pending {
                    // Answer the open question so the history stays valid.
                    blocks.extend(p.other_results.iter().cloned());
                    blocks.push(tool_result(
                        &p.tool_use_id,
                        "The user did not pick an option and wrote the message below instead.",
                        false,
                    ));
                    repo::conversations::set_pending(tx, conversation_id, None)?;
                } else if let Some(last) = last.as_ref().filter(|m| m.role == "assistant") {
                    blocks.extend(history::interrupted_results(&history::tool_use_ids(&last.content)));
                }
                blocks.push(json!({ "type": "text", "text": text }));
                let message = repo::messages::append(tx, &new_id(), conversation_id, "user", &Value::Array(blocks), &json!({}))?;
                let new_title = if conv.title == "New chat" {
                    let t = provisional_title(text);
                    repo::conversations::rename(tx, conversation_id, &t)?;
                    Some(t)
                } else {
                    None
                };
                Ok(Prepared { user_message: Some(message), new_title })
            }
            TurnInput::Choice { tool_use_id, answer } => {
                let pending = conv
                    .pending
                    .ok_or_else(|| AppError::invalid("This question has already been answered."))?;
                if pending.tool_use_id != tool_use_id {
                    return Err(AppError::invalid("This question is no longer waiting for an answer."));
                }
                let answer = answer.trim();
                if answer.is_empty() || answer.chars().count() > 2000 {
                    return Err(AppError::invalid("Choose an option or type a short answer."));
                }
                let mut blocks = pending.other_results.clone();
                blocks.push(tool_result(&tool_use_id, &format!("The user chose: {answer}"), false));
                let message = repo::messages::append(tx, &new_id(), conversation_id, "user", &Value::Array(blocks), &json!({ "choiceAnswer": answer }))?;
                repo::conversations::set_pending(tx, conversation_id, None)?;
                Ok(Prepared { user_message: Some(message), new_title: None })
            }
            TurnInput::Retry => {
                if conv.pending.is_some() {
                    return Err(AppError::invalid("Pick an option to continue."));
                }
                match last {
                    Some(m) if m.role == "user" => Ok(Prepared { user_message: None, new_title: None }),
                    _ => Err(AppError::invalid("There's nothing to retry. Send a new message instead.")),
                }
            }
        }
    })
}

fn has_text(m: &StoredMessage) -> bool {
    m.content
        .as_array()
        .map(|a| a.iter().any(|b| b["type"] == "text"))
        .unwrap_or(false)
}

fn message_text(m: &StoredMessage) -> String {
    m.content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn auto_decision(reason: &str) -> RouteDecision {
    RouteDecision {
        presentation: "auto".into(),
        renderer: "auto".into(),
        reason: reason.into(),
        source: "model".into(),
    }
}

async fn decide(
    provider: &Provider,
    settings: &Settings,
    router_model: &str,
    mode: Mode,
    stored: &[StoredMessage],
    text: &str,
) -> (RouteDecision, Option<String>) {
    if provider.is_demo() {
        return (router::demo_route(text, mode), None);
    }
    if !settings.router_enabled {
        return (auto_decision("Format routing is turned off in Settings."), None);
    }
    match router::route(provider, router_model, mode, &history::recent_text(stored, 4)).await {
        Ok(d) => (d, None),
        Err(e) => (
            auto_decision(&e.to_string()),
            Some(format!("The format router was unavailable ({e}), so the tutor chose the format itself.")),
        ),
    }
}

async fn run(deps: &Deps, conversation_id: &str, em: &Emitter, token: &CancellationToken) -> AppResult<RunEnd> {
    let (settings, conv, course, titles, stored) = deps.db.with(|c| {
        let settings = repo::settings::load(c)?;
        let conv = repo::conversations::get(c, conversation_id)?;
        let course = match &conv.course_id {
            Some(id) => repo::courses::get(c, id)?,
            None => None,
        };
        let titles = match &conv.course_id {
            Some(id) => repo::materials::titles(c, id, 40)?,
            None => vec![],
        };
        let stored = repo::messages::list(c, conversation_id)?;
        Ok((settings, conv, course, titles, stored))
    })?;

    let provider = match Provider::from_settings(&settings, &deps.secrets, &deps.http).await {
        Ok(p) => p,
        Err(e) => return Ok(RunEnd::Failed { message: e.to_string(), retryable: true }),
    };
    let (main_model, router_model) = settings.models_for(provider.kind());

    // Route once per user turn; tool continuations and retries reuse it.
    let turn_message = stored.iter().rev().find(|m| m.role == "user" && has_text(m)).cloned();
    let existing = turn_message
        .as_ref()
        .and_then(|m| serde_json::from_value::<RouteDecision>(m.meta["route"].clone()).ok());
    let decision = match existing {
        Some(d) => d,
        None => {
            let text = turn_message.as_ref().map(message_text).unwrap_or_default();
            let (d, notice) = tokio::select! {
                _ = token.cancelled() => return Ok(RunEnd::Cancelled),
                r = decide(&provider, &settings, &router_model, conv.mode, &stored, &text) => r,
            };
            if let Some(m) = &turn_message {
                let mut meta = m.meta.clone();
                if !meta.is_object() {
                    meta = json!({});
                }
                meta["route"] = serde_json::to_value(&d)?;
                deps.db.with(|c| repo::messages::set_meta(c, &m.id, &meta))?;
                em.emit(ChatEvent::Route { message_id: m.id.clone(), decision: d.clone() });
            }
            if let Some(text) = notice {
                em.emit(ChatEvent::Notice { level: "warning".into(), text });
            }
            // Name the chat after its first question, in the background.
            let user_turns = stored.iter().filter(|m| m.role == "user" && has_text(m)).count();
            if let (false, 1, Some(m)) = (provider.is_demo(), user_turns, &turn_message) {
                if settings.router_enabled {
                    spawn_title(provider.clone(), router_model.clone(), message_text(m), deps.db.clone(), conversation_id.to_string(), em.clone());
                }
            }
            d
        }
    };

    let dynamic = prompt::dynamic_prompt(&prompt::DynamicContext {
        mode: conv.mode,
        style: conv.style,
        course: course.as_ref(),
        material_titles: &titles,
        decision: &decision,
        today: chrono::Local::now().format("%Y-%m-%d").to_string(),
    });
    let tool_defs = tools::definitions(conv.mode);
    let model = if provider.is_demo() { "demo".to_string() } else { main_model };
    let provider_kind = provider.kind();

    for _ in 0..MAX_ITERATIONS {
        let stored = deps.db.with(|c| repo::messages::list(c, conversation_id))?;
        let request = TurnRequest {
            model: model.clone(),
            system_static: prompt::STATIC_PROMPT.to_string(),
            system_dynamic: dynamic.clone(),
            messages: history::to_api_messages(&stored, provider_kind),
            tools: tool_defs.clone(),
            max_tokens: MAX_OUTPUT_TOKENS,
            effort: settings.effort.clone(),
            refusal_fallback: settings.refusal_fallback,
            mode: conv.mode,
            presentation: Some(decision.presentation.clone()),
        };

        let message_id = new_id();
        em.emit(ChatEvent::AssistantStart { message_id: message_id.clone() });
        let mut acc = Accumulator::new(message_id.clone());
        let mut rx = provider.stream(request, token.clone());
        let outcome = loop {
            let next = tokio::select! {
                _ = token.cancelled() => break StreamOutcome::Cancelled,
                ev = rx.recv() => ev,
            };
            match next {
                Some(ev) => {
                    if let Some(o) = acc.handle(&ev, em) {
                        break o;
                    }
                }
                None if acc.stop_reason.is_some() => break StreamOutcome::Completed,
                None => {
                    break StreamOutcome::Failed {
                        message: "The answer stream ended unexpectedly.".into(),
                        retryable: true,
                    }
                }
            }
        };

        match outcome {
            StreamOutcome::Cancelled => {
                let blocks = history::sanitize_partial(acc.into_blocks());
                if !blocks.is_empty() {
                    let meta = json!({ "model": model, "provider": provider_kind, "stopReason": "cancelled" });
                    let msg = deps.db.with(|c| {
                        repo::messages::append(c, &message_id, conversation_id, "assistant", &Value::Array(blocks), &meta)
                    })?;
                    em.emit(ChatEvent::AssistantDone { message: msg });
                }
                return Ok(RunEnd::Cancelled);
            }
            StreamOutcome::Failed { message, retryable } => return Ok(RunEnd::Failed { message, retryable }),
            StreamOutcome::Completed => {}
        }

        let stop_reason = acc.stop_reason.clone().unwrap_or_default();
        let invalid_json = std::mem::take(&mut acc.invalid_json);
        let usage = acc.usage.clone();
        let blocks = history::sanitize_complete(acc.into_blocks());
        if blocks.is_empty() {
            if stop_reason == "refusal" {
                return Ok(RunEnd::Failed { message: "The model declined this request.".into(), retryable: false });
            }
            return Ok(RunEnd::Failed { message: "The model returned an empty answer.".into(), retryable: true });
        }
        let meta = json!({ "model": model, "provider": provider_kind, "stopReason": stop_reason, "usage": usage });
        let assistant = deps.db.with(|c| {
            repo::messages::append(c, &message_id, conversation_id, "assistant", &Value::Array(blocks), &meta)
        })?;
        em.emit(ChatEvent::AssistantDone { message: assistant.clone() });

        if stop_reason == "refusal" {
            em.emit(ChatEvent::Notice {
                level: "warning".into(),
                text: "The model declined to continue with this request.".into(),
            });
            return Ok(RunEnd::Done);
        }
        let tool_uses: Vec<Value> = assistant
            .content
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "tool_use")
            .cloned()
            .collect();
        if tool_uses.is_empty() {
            return Ok(RunEnd::Done);
        }
        if stop_reason == "max_tokens" {
            return Ok(RunEnd::Failed {
                message: "The answer reached the length limit before it finished, so it wasn't rendered. Ask for a shorter or simpler version.".into(),
                retryable: false,
            });
        }

        let ctx = tools::ToolContext {
            db: &deps.db,
            course_id: conv.course_id.as_deref(),
            conversation_id,
        };
        let mut results: Vec<Value> = Vec::new();
        let mut choice: Option<String> = None;
        for tu in &tool_uses {
            let id = tu["id"].as_str().unwrap_or_default();
            let name = tu["name"].as_str().unwrap_or_default();
            if let Some(raw) = invalid_json.get(id) {
                results.push(tool_result(id, &json!({ "INVALID_JSON": raw }).to_string(), true));
                continue;
            }
            if name == tools::ASK_CHOICE {
                match tools::validate_choice(&tu["input"]) {
                    Ok(_) if choice.is_none() => choice = Some(id.to_string()),
                    Ok(_) => results.push(tool_result(id, "Only one question can be open at a time.", true)),
                    Err(e) => results.push(tool_result(id, &e, true)),
                }
                continue;
            }
            let out = tools::execute(&ctx, name, &tu["input"]);
            results.push(tool_result(id, &out.content, out.is_error));
        }

        if let Some(tool_use_id) = choice {
            let pending = PendingChoice { tool_use_id: tool_use_id.clone(), other_results: results.clone() };
            deps.db.with(|c| repo::conversations::set_pending(c, conversation_id, Some(&pending)))?;
            em.emit(ChatEvent::Awaiting { tool_use_id, other_results: results });
            return Ok(RunEnd::Done);
        }
        let message = deps.db.with(|c| {
            repo::messages::append(c, &new_id(), conversation_id, "user", &Value::Array(results), &json!({ "toolResults": true }))
        })?;
        em.emit(ChatEvent::ToolResults { message });
        if token.is_cancelled() {
            return Ok(RunEnd::Cancelled);
        }
    }
    em.emit(ChatEvent::Notice {
        level: "info".into(),
        text: format!("Stopped after {MAX_ITERATIONS} steps. Send a message to continue."),
    });
    Ok(RunEnd::Done)
}

fn spawn_title(provider: Provider, model: String, first: String, db: Arc<Db>, conversation_id: String, em: Emitter) {
    tauri::async_runtime::spawn(async move {
        match router::generate_title(&provider, &model, &first).await {
            Ok(title) => {
                if db.with(|c| repo::conversations::rename(c, &conversation_id, &title)).is_ok() {
                    em.emit(ChatEvent::TitleUpdated { conversation_id, title });
                }
            }
            Err(e) => log::info!("title generation skipped: {e}"),
        }
    });
}

enum StreamOutcome {
    Completed,
    Cancelled,
    Failed { message: String, retryable: bool },
}

/// Rebuilds content blocks from stream events and forwards UI events.
struct Accumulator {
    message_id: String,
    blocks: Vec<Option<Value>>,
    json_buffers: HashMap<usize, String>,
    last_tool_emit: HashMap<usize, (Instant, usize)>,
    invalid_json: HashMap<String, String>,
    stop_reason: Option<String>,
    usage: Map<String, Value>,
}

impl Accumulator {
    fn new(message_id: String) -> Self {
        Self {
            message_id,
            blocks: Vec::new(),
            json_buffers: HashMap::new(),
            last_tool_emit: HashMap::new(),
            invalid_json: HashMap::new(),
            stop_reason: None,
            usage: Map::new(),
        }
    }

    fn into_blocks(self) -> Vec<Value> {
        self.blocks.into_iter().flatten().collect()
    }

    fn merge_usage(&mut self, usage: &Value) {
        if let Some(obj) = usage.as_object() {
            for (k, v) in obj {
                if v.is_number() {
                    self.usage.insert(k.clone(), v.clone());
                }
            }
        }
    }

    fn block_mut(&mut self, index: usize) -> Option<&mut Value> {
        self.blocks.get_mut(index).and_then(|b| b.as_mut())
    }

    fn handle(&mut self, ev: &Value, em: &Emitter) -> Option<StreamOutcome> {
        let index = ev["index"].as_u64().unwrap_or(0) as usize;
        match ev["type"].as_str().unwrap_or_default() {
            "message_start" => self.merge_usage(&ev["message"]["usage"]),
            "content_block_start" => {
                let mut block = ev["content_block"].clone();
                if block["type"] == "tool_use" {
                    block["input"] = json!({});
                    self.json_buffers.insert(index, String::new());
                }
                if self.blocks.len() <= index {
                    self.blocks.resize(index + 1, None);
                }
                self.blocks[index] = Some(block.clone());
                em.emit(ChatEvent::BlockStart { message_id: self.message_id.clone(), index, block });
            }
            "content_block_delta" => {
                let delta = &ev["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        let text = delta["text"].as_str().unwrap_or_default().to_string();
                        if let Some(b) = self.block_mut(index) {
                            let cur = b["text"].as_str().unwrap_or_default().to_string();
                            b["text"] = json!(cur + &text);
                        }
                        em.emit(ChatEvent::TextDelta { message_id: self.message_id.clone(), index, text });
                    }
                    "thinking_delta" => {
                        let text = delta["thinking"].as_str().unwrap_or_default().to_string();
                        if let Some(b) = self.block_mut(index) {
                            let cur = b["thinking"].as_str().unwrap_or_default().to_string();
                            b["thinking"] = json!(cur + &text);
                        }
                        em.emit(ChatEvent::ThinkingDelta { message_id: self.message_id.clone(), index, text });
                    }
                    "signature_delta" => {
                        let sig = delta["signature"].as_str().unwrap_or_default().to_string();
                        if let Some(b) = self.block_mut(index) {
                            b["signature"] = json!(sig);
                        }
                    }
                    "citations_delta" => {
                        let citation = delta["citation"].clone();
                        if let Some(b) = self.block_mut(index) {
                            if !b["citations"].is_array() {
                                b["citations"] = json!([]);
                            }
                            if let Some(arr) = b["citations"].as_array_mut() {
                                arr.push(citation);
                            }
                        }
                    }
                    // Provider-private data that must round-trip with the block,
                    // e.g. Gemini thought signatures. Keys are always `_`-prefixed.
                    "meta_delta" => {
                        if let Some(key) = delta["key"].as_str().filter(|k| k.starts_with('_')) {
                            let value = delta["value"].clone();
                            if let Some(b) = self.block_mut(index) {
                                b[key] = value;
                            }
                        }
                    }
                    "input_json_delta" => {
                        let piece = delta["partial_json"].as_str().unwrap_or_default();
                        let buf = self.json_buffers.entry(index).or_default();
                        buf.push_str(piece);
                        self.maybe_emit_tool_input(index, em, false);
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some(raw) = self.json_buffers.remove(&index) {
                    let parsed: Result<Value, _> = if raw.trim().is_empty() { Ok(json!({})) } else { serde_json::from_str(&raw) };
                    let id = self.block_mut(index).and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
                    match parsed {
                        Ok(v) if v.is_object() => {
                            if let Some(b) = self.block_mut(index) {
                                b["input"] = v;
                            }
                        }
                        _ => {
                            // Keep a valid empty input in history and report the bad JSON to the model.
                            self.invalid_json.insert(id, raw.chars().take(2000).collect());
                        }
                    }
                }
                if let Some(block) = self.blocks.get(index).cloned().flatten() {
                    em.emit(ChatEvent::BlockStop { message_id: self.message_id.clone(), index, block });
                }
            }
            "message_delta" => {
                if let Some(r) = ev["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(r.to_string());
                }
                self.merge_usage(&ev["usage"]);
            }
            "message_stop" => return Some(StreamOutcome::Completed),
            "error" => {
                return Some(StreamOutcome::Failed {
                    message: ev["error"]["message"].as_str().unwrap_or("The model stream failed.").to_string(),
                    retryable: ev["retryable"].as_bool().unwrap_or(true),
                })
            }
            _ => {}
        }
        None
    }

    fn maybe_emit_tool_input(&mut self, index: usize, em: &Emitter, force: bool) {
        let Some(buf) = self.json_buffers.get(&index) else { return };
        let Some(partial) = parse_partial_object(buf) else { return };
        let now = Instant::now();
        let (last_at, last_keys) = self.last_tool_emit.get(&index).copied().unwrap_or((now - TOOL_INPUT_THROTTLE * 2, 0));
        let keys = partial.complete_keys.len();
        if !force && keys == last_keys && now.duration_since(last_at) < TOOL_INPUT_THROTTLE {
            return;
        }
        self.last_tool_emit.insert(index, (now, keys));
        em.emit(ChatEvent::ToolInput {
            message_id: self.message_id.clone(),
            index,
            partial: Value::Object(partial.value),
            complete_keys: partial.complete_keys,
        });
    }
}
