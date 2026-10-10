//! Claude Messages API over raw HTTP (there is no official Rust SDK). The
//! canonical event format is this API's own stream, so translation is a pass-through.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::sse::SseEvent;
use super::stream::{self, is_transient, ErrorAction, StreamAdapter};
use super::{error_event, strip_private, TurnRequest};
use crate::error::{AppError, AppResult};

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const MODELS_URL: &str = "https://api.anthropic.com/v1/models?limit=1";
const API_VERSION: &str = "2023-06-01";
/// Server-side refusal fallback in its `"default"` form.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Request features differ by model family.
#[derive(Debug, Clone, Copy)]
pub struct ModelCaps {
    pub adaptive_thinking: bool,
    pub effort: bool,
    /// Accepts `fallbacks: "default"` with the server-side fallback beta.
    pub default_fallbacks: bool,
}

pub fn caps(model: &str) -> ModelCaps {
    // Haiku 4.5 and earlier predate adaptive thinking and effort; Haiku 5.5 has both.
    let legacy = model.starts_with("claude-haiku-4") || model.starts_with("claude-3");
    let fallbacks = matches!(
        model,
        "claude-opus-5-5" | "claude-sonnet-5-5" | "claude-fable-5-1" | "claude-opus-5" | "claude-fable-5"
    );
    ModelCaps {
        adaptive_thinking: !legacy,
        effort: !legacy,
        default_fallbacks: fallbacks,
    }
}

/// Claude effort levels for the app's low/medium/high setting.
fn effort(level: &str) -> &str {
    match level {
        "low" | "medium" | "high" | "xhigh" | "max" => level,
        _ => "medium",
    }
}

#[derive(Clone)]
pub struct AnthropicClient {
    http: reqwest::Client,
    key: String,
}

impl AnthropicClient {
    pub fn new(http: reqwest::Client, key: String) -> Self {
        Self { http, key }
    }

    pub fn build_body(req: &TurnRequest, with_fallback: bool) -> Value {
        let caps = caps(&req.model);
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                let content = m["content"].as_array().map(|b| strip_private(b)).unwrap_or_default();
                json!({ "role": m["role"], "content": content })
            })
            .collect();
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "stream": true,
            "system": [
                { "type": "text", "text": req.system_static, "cache_control": { "type": "ephemeral" } },
                { "type": "text", "text": req.system_dynamic }
            ],
            "messages": messages,
            // Auto-places a second breakpoint on the last cacheable block so the
            // tool loop within a turn re-reads the conversation from cache.
            "cache_control": { "type": "ephemeral" },
        });
        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t["name"],
                        "description": t["description"],
                        "input_schema": t["input_schema"],
                        "eager_input_streaming": true,
                    })
                })
                .collect();
            body["tools"] = Value::Array(tools);
            body["tool_choice"] = json!({ "type": "auto" });
        }
        if caps.adaptive_thinking {
            body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
        }
        if caps.effort {
            body["output_config"] = json!({ "effort": effort(&req.effort) });
        }
        if with_fallback && caps.default_fallbacks {
            body["fallbacks"] = json!("default");
        }
        body
    }

    fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .post(url)
            .header("x-api-key", &self.key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
    }

    pub fn stream(&self, req: TurnRequest, cancel: CancellationToken) -> mpsc::Receiver<Value> {
        let use_fallback = req.refusal_fallback && caps(&req.model).default_fallbacks;
        stream::spawn(Adapter { client: self.clone(), req, use_fallback }, cancel)
    }

    async fn complete(&self, body: Value) -> AppResult<Value> {
        let model = body["model"].as_str().unwrap_or_default().to_string();
        let resp = self
            .post(MESSAGES_URL)
            .timeout(Duration::from_secs(30))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AppError::Provider(if e.is_timeout() { "The request timed out." } else { "Couldn't reach Anthropic." }.into())
            })?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let (kind, message) = parse_error_body(&text);
            return Err(AppError::Provider(friendly_error(status, &kind, &message, &model)));
        }
        let value: Value = serde_json::from_str(&text)?;
        if value["stop_reason"] == "refusal" {
            return Err(AppError::Provider("The model declined this request.".into()));
        }
        Ok(value)
    }

    pub async fn complete_json(&self, model: &str, system: &str, user: &str, schema: Value) -> AppResult<String> {
        let mut body = json!({
            "model": model,
            "max_tokens": 4096,
            "system": system,
            "messages": [{ "role": "user", "content": user }],
            "output_config": { "format": { "type": "json_schema", "schema": schema } }
        });
        if caps(model).effort {
            body["output_config"]["effort"] = json!("low");
        }
        Ok(response_text(&self.complete(body).await?))
    }

    pub async fn complete_text(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        let mut body = json!({
            "model": model,
            "max_tokens": 2048,
            "system": system,
            "messages": [{ "role": "user", "content": user }]
        });
        if caps(model).effort {
            body["output_config"] = json!({ "effort": "low" });
        }
        Ok(response_text(&self.complete(body).await?))
    }

    pub async fn check_key(&self) -> AppResult<()> {
        let resp = self
            .http
            .get(MODELS_URL)
            .header("x-api-key", &self.key)
            .header("anthropic-version", API_VERSION)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| AppError::Provider("Couldn't reach Anthropic. Check your internet connection.".into()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        let (kind, message) = parse_error_body(&text);
        Err(AppError::Provider(friendly_error(status, &kind, &message, "")))
    }
}

struct Adapter {
    client: AnthropicClient,
    req: TurnRequest,
    use_fallback: bool,
}

impl StreamAdapter for Adapter {
    fn request(&self) -> reqwest::RequestBuilder {
        let mut rb = self.client.post(MESSAGES_URL).json(&AnthropicClient::build_body(&self.req, self.use_fallback));
        if self.use_fallback {
            rb = rb.header("anthropic-beta", FALLBACK_BETA);
        }
        rb
    }

    fn on_http_error(&mut self, status: StatusCode, body: &str) -> ErrorAction {
        let (kind, message) = parse_error_body(body);
        // If the fallback opt-in itself is rejected, continue without it.
        if status == StatusCode::BAD_REQUEST && self.use_fallback && message.to_lowercase().contains("fallback") {
            log::warn!("refusal fallback rejected, retrying without it: {message}");
            self.use_fallback = false;
            return ErrorAction::ResendNow;
        }
        if is_transient(status) {
            return ErrorAction::Retry;
        }
        ErrorAction::Fail { message: friendly_error(status, &kind, &message, &self.req.model), retryable: false }
    }

    fn translate(&mut self, event: &SseEvent) -> Vec<Value> {
        event_value(&event.data).into_iter().collect()
    }
}

/// Returns the text of the text blocks in a non-streaming response.
pub fn response_text(value: &Value) -> String {
    value["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn event_value(data: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(data).ok()?;
    if v["type"] == "ping" {
        return None;
    }
    if v["type"] == "error" {
        let kind = v["error"]["type"].as_str().unwrap_or_default().to_string();
        let message = v["error"]["message"].as_str().unwrap_or("The model stream failed.").to_string();
        let retryable = matches!(kind.as_str(), "overloaded_error" | "api_error" | "rate_limit_error");
        let friendly = match kind.as_str() {
            "overloaded_error" => "Anthropic is overloaded right now. Retry in a moment.".to_string(),
            _ => message,
        };
        return Some(error_event(friendly, retryable));
    }
    Some(v)
}

fn parse_error_body(text: &str) -> (String, String) {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let kind = v["error"]["type"].as_str().unwrap_or_default().to_string();
    let message = v["error"]["message"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| text.chars().take(300).collect());
    (kind, message)
}

fn friendly_error(status: StatusCode, kind: &str, message: &str, model: &str) -> String {
    match status.as_u16() {
        401 => "Anthropic rejected the API key. Check it in Settings.".into(),
        403 => format!("This API key isn't permitted to make this request. {message}"),
        404 if kind == "not_found_error" && !model.is_empty() => {
            format!("The model “{model}” isn't available to this API key. Choose another model in Settings.")
        }
        413 => "This conversation is too large to send. Start a new chat to continue.".into(),
        429 => "Rate limit reached. Wait a moment, then retry.".into(),
        529 => "Anthropic is overloaded right now. Retry in a moment.".into(),
        s if s >= 500 => "Anthropic had a server error. Retry in a moment.".into(),
        _ => format!("The request was rejected: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Mode;

    fn req(model: &str) -> TurnRequest {
        TurnRequest {
            model: model.into(),
            system_static: "static".into(),
            system_dynamic: "dynamic".into(),
            messages: vec![json!({"role": "user", "content": [{"type": "text", "text": "hi", "_private": 1}]})],
            tools: vec![json!({"name": "t", "description": "d", "input_schema": {"type": "object"}})],
            max_tokens: 64000,
            effort: "medium".into(),
            refusal_fallback: true,
            mode: Mode::Learn,
            presentation: None,
        }
    }

    #[test]
    fn opus_body_has_adaptive_thinking_effort_and_fallback() {
        let body = AnthropicClient::build_body(&req("claude-opus-5-5"), true);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "medium");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["tool_choice"]["type"], "auto");
        assert_eq!(body["tools"][0]["eager_input_streaming"], true);
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert!(body["system"][1].get("cache_control").is_none());
        assert!(body["messages"][0]["content"][0].get("_private").is_none(), "private fields never reach the API");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn haiku_5_5_body_uses_adaptive_thinking_and_effort() {
        let body = AnthropicClient::build_body(&req("claude-haiku-5-5"), true);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "medium");
    }

    #[test]
    fn haiku_body_omits_unsupported_fields() {
        let body = AnthropicClient::build_body(&req("claude-haiku-4-5"), true);
        assert!(body.get("thinking").is_none());
        assert!(body.get("output_config").is_none());
        assert!(body.get("fallbacks").is_none());
    }

    #[test]
    fn fallback_can_be_disabled() {
        let body = AnthropicClient::build_body(&req("claude-opus-5-5"), false);
        assert!(body.get("fallbacks").is_none());
    }

    #[test]
    fn stream_error_events_are_normalized() {
        let v = event_value(r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#).unwrap();
        assert_eq!(v["type"], "error");
        assert_eq!(v["retryable"], true);
        assert!(event_value(r#"{"type":"ping"}"#).is_none());
        assert_eq!(event_value(r#"{"type":"message_stop"}"#).unwrap()["type"], "message_stop");
    }

    #[test]
    fn friendly_errors_and_caps() {
        assert!(friendly_error(StatusCode::UNAUTHORIZED, "authentication_error", "", "m").contains("API key"));
        assert!(friendly_error(StatusCode::NOT_FOUND, "not_found_error", "", "claude-x").contains("claude-x"));
        assert!(is_transient(StatusCode::from_u16(529).unwrap()));
        assert!(!is_transient(StatusCode::BAD_REQUEST));
        assert!(caps("claude-opus-5-5").default_fallbacks);
        let h = caps("claude-haiku-4-5");
        assert!(!h.adaptive_thinking && !h.effort && !h.default_fallbacks);
        let h55 = caps("claude-haiku-5-5");
        assert!(h55.adaptive_thinking && h55.effort);
    }
}
