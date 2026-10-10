//! Google Gemini API (`generativelanguage.googleapis.com`, API-key auth),
//! streaming with `streamGenerateContent?alt=sse`.
//!
//! Gemini 3 attaches thought signatures to parts and requires them back in
//! the exact part they arrived on. Signatures on function calls go in a
//! private `_thought_signature` field on the tool_use block. A text block
//! records how its text was split into parts (`_gemini_segments`) so signed
//! text parts are rebuilt exactly. Function-call arguments arrive whole, not
//! streamed, so a Gemini interactive appears when it is complete.

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

const DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const MAX_OUTPUT_TOKENS: u32 = 65_536;
/// Documented value for function calls that have no signature of their own
/// (for example, calls made earlier by another provider).
const SKIP_SIGNATURE: &str = "skip_thought_signature_validator";

#[derive(Clone)]
pub struct GeminiClient {
    http: reqwest::Client,
    key: String,
    base: String,
}

/// Gemini 3 thinking level for the app's low/medium/high setting; every
/// current model accepts these three.
fn thinking_level(level: &str) -> &'static str {
    match level {
        "low" => "low",
        "high" | "xhigh" | "max" => "high",
        _ => "medium",
    }
}

impl GeminiClient {
    pub fn new(http: reqwest::Client, key: String, base: Option<String>) -> Self {
        Self { http, key, base: base.unwrap_or_else(|| DEFAULT_BASE.to_string()) }
    }

    fn post(&self, model: &str, method: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/models/{model}:{method}", self.base);
        self.http.post(url).header("x-goog-api-key", &self.key).header("content-type", "application/json")
    }

    pub fn build_body(req: &TurnRequest) -> Value {
        let declarations: Vec<Value> = req
            .tools
            .iter()
            .map(|t| json!({ "name": t["name"], "description": t["description"], "parametersJsonSchema": t["input_schema"] }))
            .collect();
        let mut body = json!({
            "systemInstruction": { "parts": [{ "text": format!("{}\n\n{}", req.system_static, req.system_dynamic) }] },
            "contents": to_contents(&req.messages),
            "generationConfig": {
                "maxOutputTokens": req.max_tokens.min(MAX_OUTPUT_TOKENS),
                "thinkingConfig": { "thinkingLevel": thinking_level(&req.effort), "includeThoughts": true }
            }
        });
        if !declarations.is_empty() {
            body["tools"] = json!([{ "functionDeclarations": declarations }]);
            body["toolConfig"] = json!({ "functionCallingConfig": { "mode": "AUTO" } });
        }
        body
    }

    pub fn stream(&self, req: TurnRequest, cancel: CancellationToken) -> mpsc::Receiver<Value> {
        stream::spawn(Adapter { client: self.clone(), req, tr: Translator::default() }, cancel)
    }

    async fn generate(&self, model: &str, body: &Value) -> Result<Value, (StatusCode, String)> {
        let resp = self
            .post(model, "generateContent")
            .timeout(Duration::from_secs(45))
            .json(body)
            .send()
            .await
            .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, if e.is_timeout() { "The request timed out." } else { "Couldn't reach Gemini." }.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err((status, text));
        }
        serde_json::from_str(&text).map_err(|_| (status, "Unreadable response".into()))
    }

    fn answer_text(value: &Value) -> AppResult<String> {
        if value["promptFeedback"]["blockReason"].is_string() {
            return Err(AppError::Provider("Gemini declined this request.".into()));
        }
        let candidate = &value["candidates"][0];
        if matches!(candidate["finishReason"].as_str(), Some("SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII" | "RECITATION")) {
            return Err(AppError::Provider("Gemini declined this request.".into()));
        }
        Ok(candidate["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["thought"] != true)
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(""))
    }

    pub async fn complete_json(&self, model: &str, system: &str, user: &str, schema: Value) -> AppResult<String> {
        let base = json!({
            "systemInstruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        });
        // Two documented spellings of JSON output; the second is the newer one.
        let mut first = base.clone();
        first["generationConfig"] = json!({
            "maxOutputTokens": 2048,
            "thinkingConfig": { "thinkingLevel": "low" },
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        });
        let value = match self.generate(model, &first).await {
            Ok(v) => v,
            Err((StatusCode::BAD_REQUEST, _)) => {
                let mut second = base;
                second["generationConfig"] = json!({
                    "maxOutputTokens": 2048,
                    "thinkingConfig": { "thinkingLevel": "low" },
                    "responseFormat": { "text": { "mimeType": "application/json", "schema": schema } }
                });
                self.generate(model, &second).await.map_err(|(s, t)| AppError::Provider(friendly_error(s, &t, model)))?
            }
            Err((s, t)) => return Err(AppError::Provider(friendly_error(s, &t, model))),
        };
        Self::answer_text(&value)
    }

    pub async fn complete_text(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        let body = json!({
            "systemInstruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user }] }],
            "generationConfig": { "maxOutputTokens": 1024, "thinkingConfig": { "thinkingLevel": "low" } }
        });
        let value = self.generate(model, &body).await.map_err(|(s, t)| AppError::Provider(friendly_error(s, &t, model)))?;
        Self::answer_text(&value)
    }

    pub async fn check_key(&self) -> AppResult<()> {
        let resp = self
            .http
            .get(format!("{}/models?pageSize=1", self.base))
            .header("x-goog-api-key", &self.key)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| AppError::Provider("Couldn't reach Gemini. Check your internet connection.".into()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        Err(AppError::Provider(friendly_error(status, &text, "")))
    }
}

/// Canonical messages → Gemini `contents`.
pub fn to_contents(messages: &[Value]) -> Vec<Value> {
    // Function responses must name their function; collect names by call id.
    let mut names: HashMap<String, String> = HashMap::new();
    for m in messages {
        for b in m["content"].as_array().into_iter().flatten() {
            if b["type"] == "tool_use" {
                if let (Some(id), Some(name)) = (b["id"].as_str(), b["name"].as_str()) {
                    names.insert(id.to_string(), name.to_string());
                }
            }
        }
    }
    let mut contents = Vec::new();
    for m in messages {
        let blocks = m["content"].as_array().cloned().unwrap_or_default();
        let mut parts: Vec<Value> = Vec::new();
        if m["role"] == "user" {
            for b in &blocks {
                match b["type"].as_str() {
                    Some("tool_result") => {
                        let id = b["tool_use_id"].as_str().unwrap_or_default();
                        let text = tool_result_text(&b["content"]);
                        // `response` must be an object; keep JSON results structured.
                        let response = if b["is_error"] == true {
                            json!({ "error": text })
                        } else {
                            match serde_json::from_str::<Value>(&text) {
                                Ok(Value::Object(obj)) => Value::Object(obj),
                                _ => json!({ "result": text }),
                            }
                        };
                        parts.push(json!({ "functionResponse": {
                            "id": id,
                            "name": names.get(id).cloned().unwrap_or_else(|| "unknown_tool".into()),
                            "response": response
                        } }));
                    }
                    Some("text") => parts.push(json!({ "text": b["text"] })),
                    _ => {}
                }
            }
            if !parts.is_empty() {
                contents.push(json!({ "role": "user", "parts": parts }));
            }
            continue;
        }
        for b in &blocks {
            match b["type"].as_str() {
                Some("text") => parts.extend(text_parts(b)),
                Some("tool_use") => {
                    let mut part = json!({ "functionCall": { "id": b["id"], "name": b["name"], "args": b["input"] } });
                    part["thoughtSignature"] = json!(b["_thought_signature"].as_str().unwrap_or(SKIP_SIGNATURE));
                    parts.push(part);
                }
                // Thought summaries are display-only; signatures carry the context.
                _ => {}
            }
        }
        if !parts.is_empty() {
            contents.push(json!({ "role": "model", "parts": parts }));
        }
    }
    contents
}

/// Rebuilds the parts a text block arrived in, with their signatures.
fn text_parts(block: &Value) -> Vec<Value> {
    let text = block["text"].as_str().unwrap_or_default();
    let Some(segments) = block["_gemini_segments"].as_array() else {
        return if text.is_empty() { vec![] } else { vec![json!({ "text": text })] };
    };
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0usize;
    let mut parts = Vec::new();
    for seg in segments {
        let len = seg[0].as_u64().unwrap_or(0) as usize;
        let end = (at + len).min(chars.len());
        let mut part = json!({ "text": chars[at..end].iter().collect::<String>() });
        if let Some(sig) = seg[1].as_str() {
            part["thoughtSignature"] = json!(sig);
        }
        at = end;
        parts.push(part);
    }
    if at < chars.len() {
        parts.push(json!({ "text": chars[at..].iter().collect::<String>() }));
    }
    parts
}

struct Adapter {
    client: GeminiClient,
    req: TurnRequest,
    tr: Translator,
}

impl StreamAdapter for Adapter {
    fn request(&self) -> reqwest::RequestBuilder {
        self.client.post(&self.req.model, "streamGenerateContent?alt=sse").json(&GeminiClient::build_body(&self.req))
    }

    fn on_http_error(&mut self, status: StatusCode, body: &str) -> ErrorAction {
        if matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504) {
            return ErrorAction::Retry;
        }
        ErrorAction::Fail { message: friendly_error(status, body, &self.req.model), retryable: false }
    }

    fn translate(&mut self, event: &SseEvent) -> Vec<Value> {
        match serde_json::from_str::<Value>(&event.data) {
            Ok(v) => self.tr.chunk(&v),
            Err(_) => Vec::new(),
        }
    }
}

/// Streamed `GenerateContentResponse` chunks → canonical events.
#[derive(Default)]
pub struct Translator {
    started: bool,
    next_index: usize,
    /// The open thinking or text block, if any: (index, kind).
    open: Option<(usize, &'static str)>,
    /// Part segmentation of the open text block: (char length, signature).
    segments: Vec<(usize, Option<String>)>,
    any_signed_segment: bool,
    saw_call: bool,
    done: bool,
}

impl Translator {
    fn start(&mut self, out: &mut Vec<Value>) {
        if !self.started {
            self.started = true;
            out.push(json!({ "type": "message_start", "message": { "usage": {} } }));
        }
    }

    fn close_open(&mut self, out: &mut Vec<Value>) {
        if let Some((index, kind)) = self.open.take() {
            if kind == "text" && self.any_signed_segment {
                let segs: Vec<Value> = self.segments.iter().map(|(len, sig)| json!([len, sig])).collect();
                out.push(json!({ "type": "content_block_delta", "index": index,
                                 "delta": { "type": "meta_delta", "key": "_gemini_segments", "value": segs } }));
            }
            out.push(json!({ "type": "content_block_stop", "index": index }));
        }
        self.segments.clear();
        self.any_signed_segment = false;
    }

    fn open_block(&mut self, kind: &'static str, out: &mut Vec<Value>) -> usize {
        if let Some((index, k)) = self.open {
            if k == kind {
                return index;
            }
        }
        self.close_open(out);
        let index = self.next_index;
        self.next_index += 1;
        let block = if kind == "thinking" { json!({ "type": "thinking", "thinking": "" }) } else { json!({ "type": "text", "text": "" }) };
        out.push(json!({ "type": "content_block_start", "index": index, "content_block": block }));
        self.open = Some((index, kind));
        index
    }

    fn text_part(&mut self, text: &str, signature: Option<&str>, out: &mut Vec<Value>) {
        let index = self.open_block("text", out);
        if !text.is_empty() {
            out.push(json!({ "type": "content_block_delta", "index": index, "delta": { "type": "text_delta", "text": text } }));
        }
        let len = text.chars().count();
        match signature {
            Some(sig) => {
                // A signed part stays its own part.
                self.segments.push((len, Some(sig.to_string())));
                self.any_signed_segment = true;
            }
            None => match self.segments.last_mut() {
                Some((l, None)) => *l += len,
                _ => self.segments.push((len, None)),
            },
        }
    }

    pub fn chunk(&mut self, v: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        if self.done {
            return out;
        }
        if v["error"].is_object() {
            let code = v["error"]["code"].as_u64().unwrap_or(0);
            let message = v["error"]["message"].as_str().unwrap_or("The Gemini stream failed.").to_string();
            out.push(error_event(message, matches!(code, 429 | 500 | 503 | 504)));
            self.done = true;
            return out;
        }
        self.start(&mut out);
        if v["promptFeedback"]["blockReason"].is_string() && v["candidates"].as_array().map(|c| c.is_empty()).unwrap_or(true) {
            self.finish_with("refusal", &v["usageMetadata"], &mut out);
            return out;
        }
        let candidate = &v["candidates"][0];
        for part in candidate["content"]["parts"].as_array().into_iter().flatten() {
            let signature = part["thoughtSignature"].as_str();
            if let Some(call) = part.get("functionCall") {
                self.close_open(&mut out);
                self.saw_call = true;
                let index = self.next_index;
                self.next_index += 1;
                let id = call["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("toolu_g_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]));
                let mut block = json!({ "type": "tool_use", "id": id, "name": call["name"], "input": {} });
                if let Some(sig) = signature {
                    block["_thought_signature"] = json!(sig);
                }
                out.push(json!({ "type": "content_block_start", "index": index, "content_block": block }));
                let args = if call["args"].is_object() { call["args"].clone() } else { json!({}) };
                out.push(json!({ "type": "content_block_delta", "index": index,
                                 "delta": { "type": "input_json_delta", "partial_json": args.to_string() } }));
                out.push(json!({ "type": "content_block_stop", "index": index }));
            } else if part["thought"] == true {
                let index = self.open_block("thinking", &mut out);
                if let Some(text) = part["text"].as_str().filter(|t| !t.is_empty()) {
                    out.push(json!({ "type": "content_block_delta", "index": index, "delta": { "type": "thinking_delta", "thinking": text } }));
                }
            } else if let Some(text) = part["text"].as_str() {
                if text.is_empty() && signature.is_none() {
                    continue;
                }
                self.text_part(text, signature, &mut out);
            }
        }
        if let Some(reason) = candidate["finishReason"].as_str() {
            let stop = match reason {
                "STOP" if self.saw_call => "tool_use",
                "STOP" => "end_turn",
                "MAX_TOKENS" => "max_tokens",
                "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII" | "RECITATION" | "IMAGE_SAFETY" => "refusal",
                "MALFORMED_FUNCTION_CALL" | "UNEXPECTED_TOOL_CALL" | "MALFORMED_RESPONSE" | "MISSING_THOUGHT_SIGNATURE" => {
                    out.push(error_event(format!("Gemini produced an invalid tool call ({reason}). Retry to try again."), true));
                    self.done = true;
                    return out;
                }
                _ if self.saw_call => "tool_use",
                _ => "end_turn",
            };
            self.finish_with(stop, &v["usageMetadata"], &mut out);
        }
        out
    }

    fn finish_with(&mut self, stop: &str, usage: &Value, out: &mut Vec<Value>) {
        self.close_open(out);
        let usage = json!({
            "input_tokens": usage["promptTokenCount"],
            "output_tokens": usage["candidatesTokenCount"],
            "cache_read_input_tokens": usage["cachedContentTokenCount"],
        });
        out.push(json!({ "type": "message_delta", "delta": { "stop_reason": stop }, "usage": usage }));
        out.push(json!({ "type": "message_stop" }));
        self.done = true;
    }
}

fn friendly_error(status: StatusCode, body: &str, model: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let message = v["error"]["message"].as_str().map(str::to_string).unwrap_or_else(|| body.chars().take(300).collect());
    let invalid_key = body.contains("API_KEY_INVALID") || message.to_lowercase().contains("api key not valid");
    match status.as_u16() {
        400 if invalid_key => "Gemini rejected the API key. Check it in Settings.".into(),
        401 => "Gemini rejected the API key. Check it in Settings.".into(),
        402 => "Your Gemini prepaid credit is used up.".into(),
        403 => format!("This Gemini key isn't permitted to make this request. {message}"),
        404 if !model.is_empty() => format!("The model “{model}” isn't available to this Gemini key. Choose another model in Settings."),
        429 => "Gemini rate limit reached. Wait a moment, then retry.".into(),
        s if s >= 500 => "Gemini had a server error. Retry in a moment.".into(),
        _ => format!("Gemini rejected the request: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Mode;

    fn req(messages: Vec<Value>) -> TurnRequest {
        TurnRequest {
            model: "gemini-3.8-flash".into(),
            system_static: "S".into(),
            system_dynamic: "D".into(),
            messages,
            tools: vec![json!({"name": "render_quiz", "description": "d", "input_schema": {"type": "object", "properties": {"title": {"type": "string"}}}})],
            max_tokens: 100_000,
            effort: "low".into(),
            refusal_fallback: false,
            mode: Mode::Learn,
            presentation: None,
        }
    }

    #[test]
    fn body_maps_history_signatures_and_results() {
        let messages = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "Quiz me"}]}),
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "plan"},
                {"type": "text", "text": "Here.", "_gemini_segments": [[5, "sigT"]]},
                {"type": "tool_use", "id": "fc_1", "name": "render_quiz", "input": {"title": "Q"}, "_thought_signature": "sigF"},
                {"type": "tool_use", "id": "fc_2", "name": "render_quiz", "input": {}}
            ]}),
            json!({"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "fc_1", "content": "Rendered."},
                {"type": "tool_result", "tool_use_id": "fc_2", "content": "{\"status\":\"saved\"}"},
                {"type": "tool_result", "tool_use_id": "fc_3", "content": "bad", "is_error": true}
            ]}),
        ];
        let body = GeminiClient::build_body(&req(messages));
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "S\n\nD");
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 65_536);
        assert_eq!(body["generationConfig"]["thinkingConfig"]["thinkingLevel"], "low");
        assert_eq!(body["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"]["type"], "object");
        let c = body["contents"].as_array().unwrap();
        assert_eq!(c[1]["role"], "model");
        let parts = c[1]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 3, "thought summaries are not sent back");
        assert_eq!(parts[0], json!({"text": "Here.", "thoughtSignature": "sigT"}));
        assert_eq!(parts[1]["thoughtSignature"], "sigF");
        assert_eq!(parts[1]["functionCall"]["args"]["title"], "Q");
        assert_eq!(parts[2]["thoughtSignature"], SKIP_SIGNATURE);
        let results = c[2]["parts"].as_array().unwrap();
        assert_eq!(results[0]["functionResponse"]["name"], "render_quiz");
        assert_eq!(results[0]["functionResponse"]["response"], json!({"result": "Rendered."}));
        assert_eq!(results[1]["functionResponse"]["response"]["status"], "saved");
        assert_eq!(results[2]["functionResponse"]["response"], json!({"error": "bad"}));
    }

    #[test]
    fn text_segments_rebuild_signed_parts() {
        let block = json!({"type": "text", "text": "Hello world", "_gemini_segments": [[6, null], [5, "sig"], [0, "sig2"]]});
        assert_eq!(text_parts(&block), vec![json!({"text": "Hello "}), json!({"text": "world", "thoughtSignature": "sig"}), json!({"text": "", "thoughtSignature": "sig2"})]);
        assert_eq!(text_parts(&json!({"type": "text", "text": "plain"})), vec![json!({"text": "plain"})]);
    }

    #[test]
    fn stream_translates_thoughts_text_calls_and_signatures() {
        let chunks = [
            json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Planning", "thought": true}]}}]}),
            json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Here's "}]}}]}),
            json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "a quiz."}]}}]}),
            json!({"candidates": [{"content": {"role": "model", "parts": [
                {"functionCall": {"id": "fc_1", "name": "render_quiz", "args": {"title": "Q"}}, "thoughtSignature": "sig"}
            ]}, "finishReason": "STOP"}], "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 4}}),
        ];
        let mut tr = Translator::default();
        let out: Vec<Value> = chunks.iter().flat_map(|c| tr.chunk(c)).collect();
        let starts: Vec<&Value> = out.iter().filter(|e| e["type"] == "content_block_start").collect();
        assert_eq!(starts.len(), 3);
        assert_eq!(starts[0]["content_block"]["type"], "thinking");
        assert_eq!(starts[1]["content_block"]["type"], "text");
        assert_eq!(starts[2]["content_block"]["_thought_signature"], "sig");
        let text: String = out.iter().filter(|e| e["delta"]["type"] == "text_delta").map(|e| e["delta"]["text"].as_str().unwrap()).collect();
        assert_eq!(text, "Here's a quiz.");
        assert!(out.iter().all(|e| e["delta"]["key"] != "_gemini_segments"), "unsigned text needs no segments");
        let args = out.iter().find(|e| e["delta"]["type"] == "input_json_delta").unwrap();
        assert_eq!(args["delta"]["partial_json"], "{\"title\":\"Q\"}");
        let stop = out.iter().find(|e| e["type"] == "message_delta").unwrap();
        assert_eq!(stop["delta"]["stop_reason"], "tool_use");
        assert_eq!(out.last().unwrap()["type"], "message_stop");
        assert_eq!(out.iter().filter(|e| e["type"] == "content_block_stop").count(), 3);
    }

    #[test]
    fn trailing_signature_on_empty_text_is_recorded() {
        let mut tr = Translator::default();
        let mut out = tr.chunk(&json!({"candidates": [{"content": {"parts": [{"text": "Done."}]}}]}));
        out.extend(tr.chunk(&json!({"candidates": [{"content": {"parts": [{"text": "", "thoughtSignature": "tail"}]}, "finishReason": "STOP"}]})));
        let seg = out.iter().find(|e| e["delta"]["key"] == "_gemini_segments").unwrap();
        assert_eq!(seg["delta"]["value"], json!([[5, null], [0, "tail"]]));
        assert_eq!(out.iter().find(|e| e["type"] == "message_delta").unwrap()["delta"]["stop_reason"], "end_turn");
    }

    #[test]
    fn blocks_and_errors() {
        let mut tr = Translator::default();
        let out = tr.chunk(&json!({"promptFeedback": {"blockReason": "SAFETY"}}));
        assert_eq!(out.iter().find(|e| e["type"] == "message_delta").unwrap()["delta"]["stop_reason"], "refusal");
        let mut tr = Translator::default();
        let out = tr.chunk(&json!({"candidates": [{"content": {"parts": []}, "finishReason": "MALFORMED_FUNCTION_CALL"}]}));
        assert!(out.iter().any(|e| e["type"] == "error"));
        let mut tr = Translator::default();
        let out = tr.chunk(&json!({"error": {"code": 503, "message": "overloaded"}}));
        assert_eq!(out[0]["retryable"], true);
        assert!(friendly_error(StatusCode::BAD_REQUEST, r#"{"error":{"message":"API key not valid","details":[{"reason":"API_KEY_INVALID"}]}}"#, "m").contains("API key"));
    }
}
