//! OpenAI Responses API (`POST /v1/responses`), stateless (`store: false`).
//!
//! Recent models only support tool calling through Responses. Because the
//! app keeps its own history, every output item must be sent back as it was
//! received: reasoning items (with their encrypted content) and assistant
//! messages (with their `phase`) are kept in private `_openai_*` fields on the
//! canonical blocks and restored when the conversation continues on OpenAI.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::sse::SseEvent;
use super::stream::{self, ErrorAction, StreamAdapter};
use super::{error_event, tool_result_text, TurnRequest};
use crate::error::{AppError, AppResult};

const DEFAULT_BASE: &str = "https://api.openai.com/v1";

#[derive(Clone)]
pub struct OpenAiClient {
    http: reqwest::Client,
    key: String,
    base: String,
}

/// Pro models (e.g. gpt-5.5-pro) answer with extra compute: they don't
/// stream, can take minutes, and accept effort from medium up.
fn is_pro(model: &str) -> bool {
    model.ends_with("-pro")
}

/// Whether a model supports streamed responses.
pub fn streams(model: &str) -> bool {
    !is_pro(model)
}

/// OpenAI reasoning effort for the app's low/medium/high setting.
fn effort(model: &str, level: &str) -> &'static str {
    match level {
        "low" if is_pro(model) => "medium",
        "low" => "low",
        "high" | "xhigh" | "max" => "high",
        _ => "medium",
    }
}

impl OpenAiClient {
    pub fn new(http: reqwest::Client, key: String, base: Option<String>) -> Self {
        Self { http, key, base: base.unwrap_or_else(|| DEFAULT_BASE.to_string()) }
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.key)
            .header("content-type", "application/json")
    }

    pub fn build_body(req: &TurnRequest, with_summary: bool) -> Value {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                // The app's schemas have optional fields, which strict mode
                // does not allow; the agent validates every input itself.
                json!({
                    "type": "function",
                    "name": t["name"],
                    "description": t["description"],
                    "parameters": t["input_schema"],
                    "strict": false,
                })
            })
            .collect();
        let mut reasoning = json!({ "effort": effort(&req.model, &req.effort) });
        if with_summary {
            reasoning["summary"] = json!("auto");
        }
        let mut body = json!({
            "model": req.model,
            "stream": streams(&req.model),
            "store": false,
            "instructions": format!("{}\n\n{}", req.system_static, req.system_dynamic),
            "input": to_input(&req.messages),
            "reasoning": reasoning,
            "max_output_tokens": req.max_tokens,
            "prompt_cache_key": format!("explorable-{}", req.mode.as_str()),
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
            body["tool_choice"] = json!("auto");
        }
        body
    }

    pub fn stream(&self, req: TurnRequest, cancel: CancellationToken) -> mpsc::Receiver<Value> {
        stream::spawn(Adapter { client: self.clone(), req, with_summary: true, tr: Translator::default() }, cancel)
    }

    async fn complete(&self, body: Value) -> AppResult<String> {
        let model = body["model"].as_str().unwrap_or_default().to_string();
        let resp = self
            .post("/responses")
            .timeout(Duration::from_secs(45))
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Provider(if e.is_timeout() { "The request timed out." } else { "Couldn't reach OpenAI." }.into()))?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::Provider(friendly_error(status, &text, &model)));
        }
        let value: Value = serde_json::from_str(&text)?;
        let mut out = String::new();
        for item in value["output"].as_array().into_iter().flatten() {
            if item["type"] != "message" {
                continue;
            }
            for part in item["content"].as_array().into_iter().flatten() {
                match part["type"].as_str() {
                    Some("output_text") => out.push_str(part["text"].as_str().unwrap_or_default()),
                    Some("refusal") => return Err(AppError::Provider("The model declined this request.".into())),
                    _ => {}
                }
            }
        }
        Ok(out)
    }

    pub async fn complete_json(&self, model: &str, system: &str, user: &str, name: &str, schema: Value) -> AppResult<String> {
        self.complete(json!({
            "model": model,
            "store": false,
            "instructions": system,
            "input": [{ "role": "user", "content": user }],
            "reasoning": { "effort": "low" },
            "max_output_tokens": 4000,
            "text": { "format": { "type": "json_schema", "name": name, "strict": true, "schema": schema } }
        }))
        .await
    }

    pub async fn complete_text(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        self.complete(json!({
            "model": model,
            "store": false,
            "instructions": system,
            "input": [{ "role": "user", "content": user }],
            "reasoning": { "effort": "low" },
            "max_output_tokens": 2000,
            "text": { "verbosity": "low" }
        }))
        .await
    }

    pub async fn check_key(&self) -> AppResult<()> {
        let resp = self
            .http
            .get(format!("{}/models", self.base))
            .bearer_auth(&self.key)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| AppError::Provider("Couldn't reach OpenAI. Check your internet connection.".into()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        Err(AppError::Provider(friendly_error(status, &text, "")))
    }
}

/// Canonical messages → Responses `input` items.
pub fn to_input(messages: &[Value]) -> Vec<Value> {
    let mut items = Vec::new();
    for m in messages {
        let blocks = m["content"].as_array().cloned().unwrap_or_default();
        if m["role"] == "user" {
            let mut text = Vec::new();
            for b in &blocks {
                match b["type"].as_str() {
                    Some("tool_result") => {
                        let mut out = tool_result_text(&b["content"]);
                        if b["is_error"] == true {
                            out = format!("Error: {out}");
                        }
                        items.push(json!({ "type": "function_call_output", "call_id": b["tool_use_id"], "output": out }));
                    }
                    Some("text") => text.push(b["text"].as_str().unwrap_or_default().to_string()),
                    _ => {}
                }
            }
            if !text.is_empty() {
                items.push(json!({ "role": "user", "content": text.join("\n\n") }));
            }
            continue;
        }
        for b in &blocks {
            match b["type"].as_str() {
                Some("thinking") => {
                    // Only OpenAI's own reasoning items go back, unchanged.
                    if b["_openai_item"].is_object() {
                        items.push(b["_openai_item"].clone());
                    }
                }
                Some("text") => {
                    let text = b["text"].as_str().unwrap_or_default();
                    if text.trim().is_empty() {
                        continue;
                    }
                    match b["_openai_id"].as_str() {
                        Some(id) => {
                            let mut item = json!({
                                "type": "message",
                                "id": id,
                                "role": "assistant",
                                "status": "completed",
                                "content": [{ "type": "output_text", "text": text, "annotations": [] }]
                            });
                            if let Some(phase) = b["_openai_phase"].as_str() {
                                item["phase"] = json!(phase);
                            }
                            items.push(item);
                        }
                        None => items.push(json!({ "role": "assistant", "content": text })),
                    }
                }
                Some("tool_use") => {
                    let mut item = json!({
                        "type": "function_call",
                        "call_id": b["id"],
                        "name": b["name"],
                        "arguments": serde_json::to_string(&b["input"]).unwrap_or_else(|_| "{}".into()),
                    });
                    if let Some(id) = b["_openai_item_id"].as_str() {
                        item["id"] = json!(id);
                    }
                    items.push(item);
                }
                _ => {}
            }
        }
    }
    items
}

struct Adapter {
    client: OpenAiClient,
    req: TurnRequest,
    with_summary: bool,
    tr: Translator,
}

impl StreamAdapter for Adapter {
    fn request(&self) -> reqwest::RequestBuilder {
        self.client.post("/responses").json(&OpenAiClient::build_body(&self.req, self.with_summary))
    }

    fn on_http_error(&mut self, status: StatusCode, body: &str) -> ErrorAction {
        let (_, message, code) = parse_error_body(body);
        // Reasoning summaries can need organization verification; continue without them.
        if status == StatusCode::BAD_REQUEST && self.with_summary && (message.contains("summary") || message.contains("verif")) {
            log::warn!("reasoning summaries rejected, retrying without them: {message}");
            self.with_summary = false;
            return ErrorAction::ResendNow;
        }
        if retryable(status, &code) {
            return ErrorAction::Retry;
        }
        ErrorAction::Fail { message: friendly_error(status, body, &self.req.model), retryable: false }
    }

    fn translate(&mut self, event: &SseEvent) -> Vec<Value> {
        match serde_json::from_str::<Value>(&event.data) {
            Ok(v) => self.tr.event(&v),
            Err(_) => Vec::new(),
        }
    }

    fn finish(&mut self) -> Vec<Value> {
        self.tr.finish()
    }

    /// A non-streaming model's complete response, replayed as stream events.
    fn translate_body(&mut self, body: &str) -> Vec<Value> {
        match serde_json::from_str::<Value>(body) {
            Ok(resp) => synthesize_events(&resp).iter().flat_map(|e| self.tr.event(e)).collect(),
            Err(_) => vec![error_event("OpenAI sent an unreadable response.", true)],
        }
    }
}

/// The stream events equivalent to a complete Response object.
pub fn synthesize_events(resp: &Value) -> Vec<Value> {
    let mut events = vec![json!({ "type": "response.created", "response": {} })];
    for (i, item) in resp["output"].as_array().into_iter().flatten().enumerate() {
        let id = item["id"].clone();
        match item["type"].as_str() {
            Some("reasoning") => {
                events.push(json!({ "type": "response.output_item.added", "output_index": i, "item": item }));
                for part in item["summary"].as_array().into_iter().flatten() {
                    events.push(json!({ "type": "response.reasoning_summary_text.delta", "item_id": id, "delta": part["text"] }));
                    events.push(json!({ "type": "response.reasoning_summary_part.done", "item_id": id }));
                }
            }
            Some("message") => {
                events.push(json!({ "type": "response.output_item.added", "output_index": i, "item": item }));
                for part in item["content"].as_array().into_iter().flatten() {
                    match part["type"].as_str() {
                        Some("output_text") => events.push(json!({ "type": "response.output_text.delta", "item_id": id, "delta": part["text"] })),
                        Some("refusal") => events.push(json!({ "type": "response.refusal.delta", "item_id": id, "delta": part["refusal"] })),
                        _ => {}
                    }
                }
            }
            Some("function_call") => {
                let mut added = item.clone();
                added["arguments"] = json!("");
                events.push(json!({ "type": "response.output_item.added", "output_index": i, "item": added }));
                events.push(json!({ "type": "response.function_call_arguments.delta", "item_id": id, "delta": item["arguments"] }));
            }
            _ => continue,
        }
        events.push(json!({ "type": "response.output_item.done", "output_index": i, "item": item }));
    }
    let terminal = match resp["status"].as_str() {
        Some("incomplete") => "response.incomplete",
        Some("failed") => "response.failed",
        _ => "response.completed",
    };
    events.push(json!({ "type": terminal, "response": resp }));
    events
}

/// Responses stream events → canonical (Claude-shaped) events.
#[derive(Default)]
pub struct Translator {
    started: bool,
    next_index: usize,
    /// Response item id → canonical block index.
    blocks: HashMap<String, usize>,
    open: Vec<usize>,
    saw_tool_call: bool,
    stopped: bool,
}

impl Translator {
    fn start(&mut self, out: &mut Vec<Value>) {
        if !self.started {
            self.started = true;
            out.push(json!({ "type": "message_start", "message": { "usage": {} } }));
        }
    }

    fn open_block(&mut self, item_id: &str, block: Value, out: &mut Vec<Value>) -> usize {
        let index = self.next_index;
        self.next_index += 1;
        self.blocks.insert(item_id.to_string(), index);
        self.open.push(index);
        out.push(json!({ "type": "content_block_start", "index": index, "content_block": block }));
        index
    }

    fn close_block(&mut self, index: usize, out: &mut Vec<Value>) {
        if let Some(pos) = self.open.iter().position(|i| *i == index) {
            self.open.remove(pos);
            out.push(json!({ "type": "content_block_stop", "index": index }));
        }
    }

    fn delta(&self, item_id: &Value, delta: Value) -> Option<Value> {
        let index = *self.blocks.get(item_id.as_str()?)?;
        Some(json!({ "type": "content_block_delta", "index": index, "delta": delta }))
    }

    fn end(&mut self, stop_reason: &str, usage: &Value, out: &mut Vec<Value>) {
        for index in std::mem::take(&mut self.open) {
            out.push(json!({ "type": "content_block_stop", "index": index }));
        }
        let usage = json!({
            "input_tokens": usage["input_tokens"],
            "output_tokens": usage["output_tokens"],
            "cache_read_input_tokens": usage["input_tokens_details"]["cached_tokens"],
        });
        out.push(json!({ "type": "message_delta", "delta": { "stop_reason": stop_reason }, "usage": usage }));
        out.push(json!({ "type": "message_stop" }));
        self.stopped = true;
    }

    pub fn event(&mut self, v: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        if self.stopped {
            return out;
        }
        match v["type"].as_str().unwrap_or_default() {
            "response.created" | "response.in_progress" => self.start(&mut out),
            "response.output_item.added" => {
                self.start(&mut out);
                let item = &v["item"];
                let id = item["id"].as_str().unwrap_or_default().to_string();
                match item["type"].as_str() {
                    Some("reasoning") => {
                        self.open_block(&id, json!({ "type": "thinking", "thinking": "" }), &mut out);
                    }
                    Some("message") => {
                        self.open_block(&id, json!({ "type": "text", "text": "" }), &mut out);
                    }
                    Some("function_call") => {
                        self.saw_tool_call = true;
                        self.open_block(
                            &id,
                            json!({ "type": "tool_use", "id": item["call_id"], "name": item["name"], "input": {}, "_openai_item_id": id }),
                            &mut out,
                        );
                    }
                    _ => {}
                }
            }
            "response.output_text.delta" | "response.refusal.delta" => {
                out.extend(self.delta(&v["item_id"], json!({ "type": "text_delta", "text": v["delta"] })));
            }
            "response.reasoning_summary_text.delta" => {
                out.extend(self.delta(&v["item_id"], json!({ "type": "thinking_delta", "thinking": v["delta"] })));
            }
            "response.reasoning_summary_part.done" => {
                // Separate summary paragraphs.
                out.extend(self.delta(&v["item_id"], json!({ "type": "thinking_delta", "thinking": "\n\n" })));
            }
            "response.function_call_arguments.delta" => {
                out.extend(self.delta(&v["item_id"], json!({ "type": "input_json_delta", "partial_json": v["delta"] })));
            }
            "response.output_item.done" => {
                let item = &v["item"];
                let Some(&index) = item["id"].as_str().and_then(|id| self.blocks.get(id)) else {
                    return out;
                };
                let meta = |key: &str, value: Value| json!({ "type": "content_block_delta", "index": index, "delta": { "type": "meta_delta", "key": key, "value": value } });
                match item["type"].as_str() {
                    // The complete item (including encrypted content) arrives only here.
                    Some("reasoning") => out.push(meta("_openai_item", item.clone())),
                    Some("message") => {
                        out.push(meta("_openai_id", item["id"].clone()));
                        if item["phase"].is_string() {
                            out.push(meta("_openai_phase", item["phase"].clone()));
                        }
                    }
                    _ => {}
                }
                self.close_block(index, &mut out);
            }
            "response.completed" => {
                self.start(&mut out);
                let reason = if self.saw_tool_call { "tool_use" } else { "end_turn" };
                self.end(reason, &v["response"]["usage"], &mut out);
            }
            "response.incomplete" => {
                self.start(&mut out);
                let reason = match v["response"]["incomplete_details"]["reason"].as_str() {
                    Some("max_output_tokens") => "max_tokens",
                    Some("content_filter") => "refusal",
                    _ => "end_turn",
                };
                self.end(reason, &v["response"]["usage"], &mut out);
            }
            "response.failed" => {
                let err = &v["response"]["error"];
                let code = err["code"].as_str().unwrap_or_default();
                let message = err["message"].as_str().unwrap_or("OpenAI couldn't complete the answer.");
                let retryable = matches!(code, "server_error" | "rate_limit_exceeded");
                out.push(error_event(message.to_string(), retryable));
                self.stopped = true;
            }
            "error" => {
                let message = v["message"].as_str().unwrap_or("The OpenAI stream failed.");
                out.push(error_event(message.to_string(), true));
                self.stopped = true;
            }
            _ => {}
        }
        out
    }

    pub fn finish(&mut self) -> Vec<Value> {
        // A stream that ends without a terminal event is reported by the agent
        // ("ended unexpectedly"); nothing to add here.
        Vec::new()
    }
}

fn parse_error_body(text: &str) -> (String, String, String) {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let kind = v["error"]["type"].as_str().unwrap_or_default().to_string();
    let message = v["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| text.chars().take(300).collect());
    let code = v["error"]["code"].as_str().unwrap_or_default().to_string();
    (kind, message, code)
}

const QUOTA_CODES: &[&str] = &[
    "credit_balance_exhausted",
    "organization_spend_limit_exceeded",
    "project_spend_limit_exceeded",
    "organization_usage_limit_exceeded",
    "insufficient_quota",
];

fn retryable(status: StatusCode, code: &str) -> bool {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return !QUOTA_CODES.contains(&code);
    }
    status.is_server_error()
}

fn friendly_error(status: StatusCode, body: &str, model: &str) -> String {
    let (_, message, code) = parse_error_body(body);
    match status.as_u16() {
        401 => "OpenAI rejected the API key. Check it in Settings.".into(),
        403 => format!("This OpenAI key isn't permitted to make this request. {message}"),
        404 if !model.is_empty() => format!("The model “{model}” isn't available to this OpenAI key. Choose another model in Settings."),
        429 if QUOTA_CODES.contains(&code.as_str()) => "Your OpenAI account is out of credit or over its spending limit.".into(),
        429 => "OpenAI rate limit reached. Wait a moment, then retry.".into(),
        s if s >= 500 => "OpenAI had a server error. Retry in a moment.".into(),
        _ => format!("OpenAI rejected the request: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Mode;

    fn req() -> TurnRequest {
        TurnRequest {
            model: "gpt-6.1-sol".into(),
            system_static: "S".into(),
            system_dynamic: "D".into(),
            messages: vec![
                json!({"role": "user", "content": [{"type": "text", "text": "Quiz me"}]}),
                json!({"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "plan", "_openai_item": {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "gAAA"}},
                    {"type": "text", "text": "Here you go.", "_openai_id": "msg_1", "_openai_phase": "commentary"},
                    {"type": "tool_use", "id": "call_A", "name": "render_quiz", "input": {"title": "Q"}, "_openai_item_id": "fc_1"}
                ]}),
                json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_A", "content": "Rendered."}, {"type": "text", "text": "Next"}]}),
                json!({"role": "assistant", "content": [{"type": "text", "text": "From Claude"}, {"type": "tool_use", "id": "toolu_1", "name": "x", "input": {}}]}),
            ],
            tools: vec![json!({"name": "render_quiz", "description": "d", "input_schema": {"type": "object"}})],
            max_tokens: 64000,
            effort: "high".into(),
            refusal_fallback: false,
            mode: Mode::Learn,
            presentation: None,
        }
    }

    #[test]
    fn body_round_trips_openai_items_and_maps_others() {
        let body = OpenAiClient::build_body(&req(), true);
        assert_eq!(body["store"], false);
        assert_eq!(body["instructions"], "S\n\nD");
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["reasoning"]["summary"], "auto");
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["strict"], false);
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[0], json!({"role": "user", "content": "Quiz me"}));
        assert_eq!(input[1]["encrypted_content"], "gAAA", "reasoning item goes back unchanged");
        assert_eq!(input[2]["phase"], "commentary");
        assert_eq!(input[2]["id"], "msg_1");
        assert_eq!(input[3]["type"], "function_call");
        assert_eq!(input[3]["call_id"], "call_A");
        assert_eq!(input[3]["id"], "fc_1");
        assert_eq!(input[3]["arguments"], "{\"title\":\"Q\"}");
        assert_eq!(input[4], json!({"type": "function_call_output", "call_id": "call_A", "output": "Rendered."}));
        assert_eq!(input[5], json!({"role": "user", "content": "Next"}));
        assert_eq!(input[6], json!({"role": "assistant", "content": "From Claude"}));
        assert!(input[7].get("id").is_none());
        assert!(!OpenAiClient::build_body(&req(), false)["reasoning"].as_object().unwrap().contains_key("summary"));
    }

    #[test]
    fn stream_translates_text_reasoning_and_tool_calls() {
        let events = [
            json!({"type": "response.created", "response": {}}),
            json!({"type": "response.output_item.added", "output_index": 0, "item": {"type": "reasoning", "id": "rs_1", "summary": []}}),
            json!({"type": "response.reasoning_summary_text.delta", "item_id": "rs_1", "delta": "Planning"}),
            json!({"type": "response.output_item.done", "output_index": 0, "item": {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "gAAA"}}),
            json!({"type": "response.output_item.added", "output_index": 1, "item": {"type": "message", "id": "msg_1", "role": "assistant", "content": []}}),
            json!({"type": "response.output_text.delta", "item_id": "msg_1", "delta": "Hi"}),
            json!({"type": "response.output_item.done", "output_index": 1, "item": {"type": "message", "id": "msg_1", "phase": "commentary"}}),
            json!({"type": "response.output_item.added", "output_index": 2, "item": {"type": "function_call", "id": "fc_1", "call_id": "call_A", "name": "render_quiz", "arguments": ""}}),
            json!({"type": "response.function_call_arguments.delta", "item_id": "fc_1", "delta": "{\"title\":"}),
            json!({"type": "response.function_call_arguments.delta", "item_id": "fc_1", "delta": "\"Q\"}"}),
            json!({"type": "response.output_item.done", "output_index": 2, "item": {"type": "function_call", "id": "fc_1", "call_id": "call_A", "name": "render_quiz", "arguments": "{\"title\":\"Q\"}"}}),
            json!({"type": "response.completed", "response": {"usage": {"input_tokens": 10, "output_tokens": 5, "input_tokens_details": {"cached_tokens": 4}}}}),
        ];
        let mut tr = Translator::default();
        let out: Vec<Value> = events.iter().flat_map(|e| tr.event(e)).collect();
        let types: Vec<&str> = out.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(types[0], "message_start");
        assert_eq!(types.last().unwrap(), &"message_stop");
        let starts: Vec<&Value> = out.iter().filter(|e| e["type"] == "content_block_start").collect();
        assert_eq!(starts[0]["content_block"]["type"], "thinking");
        assert_eq!(starts[1]["content_block"]["type"], "text");
        assert_eq!(starts[2]["content_block"]["id"], "call_A");
        assert_eq!(starts[2]["index"], 2);
        let args: String = out
            .iter()
            .filter(|e| e["delta"]["type"] == "input_json_delta")
            .map(|e| e["delta"]["partial_json"].as_str().unwrap())
            .collect();
        assert_eq!(args, "{\"title\":\"Q\"}");
        assert!(out.iter().any(|e| e["delta"]["key"] == "_openai_item" && e["delta"]["value"]["encrypted_content"] == "gAAA"));
        let stop = out.iter().find(|e| e["type"] == "message_delta").unwrap();
        assert_eq!(stop["delta"]["stop_reason"], "tool_use");
        assert_eq!(stop["usage"]["cache_read_input_tokens"], 4);
        assert_eq!(out.iter().filter(|e| e["type"] == "content_block_stop").count(), 3);
    }

    #[test]
    fn pro_models_do_not_stream_and_raise_low_effort() {
        let mut r = req();
        r.model = "gpt-5.5-pro".into();
        r.effort = "low".into();
        let body = OpenAiClient::build_body(&r, true);
        assert_eq!(body["stream"], false);
        assert_eq!(body["reasoning"]["effort"], "medium");
        r.model = "gpt-5.6-terra".into();
        let body = OpenAiClient::build_body(&r, true);
        assert_eq!(body["stream"], true);
        assert_eq!(body["reasoning"]["effort"], "low");
    }

    #[test]
    fn complete_responses_replay_as_events() {
        let resp = json!({"status": "completed", "usage": {"input_tokens": 9, "output_tokens": 3}, "output": [
            {"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "Thinking it over."}], "encrypted_content": "enc"},
            {"type": "message", "id": "msg_1", "phase": "final_answer", "content": [{"type": "output_text", "text": "Answer", "annotations": []}]},
            {"type": "function_call", "id": "fc_1", "call_id": "call_A", "name": "render_quiz", "arguments": "{\"title\":\"Q\"}"}
        ]});
        let mut tr = Translator::default();
        let out: Vec<Value> = synthesize_events(&resp).iter().flat_map(|e| tr.event(e)).collect();
        assert!(out.iter().any(|e| e["delta"]["thinking"] == "Thinking it over."));
        assert!(out.iter().any(|e| e["delta"]["text"] == "Answer"));
        assert!(out.iter().any(|e| e["delta"]["partial_json"] == "{\"title\":\"Q\"}"));
        assert!(out.iter().any(|e| e["delta"]["key"] == "_openai_item" && e["delta"]["value"]["encrypted_content"] == "enc"));
        let stop = out.iter().find(|e| e["type"] == "message_delta").unwrap();
        assert_eq!(stop["delta"]["stop_reason"], "tool_use");
        assert_eq!(out.last().unwrap()["type"], "message_stop");
    }

    #[test]
    fn incomplete_and_failed_responses() {
        let mut tr = Translator::default();
        let out = tr.event(&json!({"type": "response.incomplete", "response": {"incomplete_details": {"reason": "max_output_tokens"}, "usage": {}}}));
        assert_eq!(out.iter().find(|e| e["type"] == "message_delta").unwrap()["delta"]["stop_reason"], "max_tokens");
        let mut tr = Translator::default();
        let out = tr.event(&json!({"type": "response.failed", "response": {"error": {"code": "server_error", "message": "boom"}}}));
        assert_eq!(out[0]["type"], "error");
        assert_eq!(out[0]["retryable"], true);
    }

    #[test]
    fn errors_are_classified() {
        let quota = r#"{"error":{"message":"m","type":"insufficient_quota","code":"credit_balance_exhausted"}}"#;
        assert!(!retryable(StatusCode::TOO_MANY_REQUESTS, "credit_balance_exhausted"));
        assert!(retryable(StatusCode::TOO_MANY_REQUESTS, "rate_limit_exceeded"));
        assert!(friendly_error(StatusCode::TOO_MANY_REQUESTS, quota, "m").contains("credit"));
        assert!(friendly_error(StatusCode::UNAUTHORIZED, "{}", "m").contains("API key"));
    }
}
