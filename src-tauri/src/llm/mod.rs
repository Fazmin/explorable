//! Model providers. Every provider streams the same canonical event objects
//! (shaped like the Claude Messages API stream: `message_start`,
//! `content_block_delta`, ...) over a channel, so the agent has one code path
//! for Claude, OpenAI, Gemini, and the scripted demo. Adapters translate the
//! canonical history into each provider's request format.

pub mod anthropic;
pub mod catalog;
pub mod demo;
mod demo_content;
pub mod gemini;
pub mod openai;
pub mod sse;
pub mod stream;

use serde_json::{json, Map, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::models::{Mode, Settings};
use crate::secrets::Secrets;

/// Everything needed for one model call in an agent turn.
#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub model: String,
    /// Stable instructions; cached.
    pub system_static: String,
    /// Per-turn context (mode, course, routing decision); after the cache breakpoint.
    pub system_dynamic: String,
    /// Canonical (Claude-shaped) messages, already filtered for this provider.
    pub messages: Vec<Value>,
    /// Canonical tool definitions: name, description, input_schema.
    pub tools: Vec<Value>,
    pub max_tokens: u32,
    pub effort: String,
    pub refusal_fallback: bool,
    pub mode: Mode,
    /// The routing decision, used by the demo provider to pick a script.
    pub presentation: Option<String>,
}

#[derive(Clone)]
pub enum Provider {
    Anthropic(anthropic::AnthropicClient),
    OpenAi(openai::OpenAiClient),
    Gemini(gemini::GeminiClient),
    Demo,
}

impl Provider {
    /// Builds the provider chosen in Settings, or explains what is missing.
    pub async fn from_settings(settings: &Settings, secrets: &Secrets, http: &reqwest::Client) -> AppResult<Provider> {
        Self::build(&settings.provider, settings, secrets, http).await
    }

    pub async fn build(kind: &str, settings: &Settings, secrets: &Secrets, http: &reqwest::Client) -> AppResult<Provider> {
        if kind == "demo" {
            return Ok(Provider::Demo);
        }
        let Some((key, _)) = secrets.get(kind).await else {
            return Err(AppError::Provider(format!(
                "Add your {} API key in Settings, or switch to demo mode there.",
                catalog::provider_label(kind)
            )));
        };
        let base = settings.base_url_for(kind);
        Ok(match kind {
            "anthropic" => Provider::Anthropic(anthropic::AnthropicClient::new(http.clone(), key)),
            "openai" => Provider::OpenAi(openai::OpenAiClient::new(http.clone(), key, base)),
            "gemini" => Provider::Gemini(gemini::GeminiClient::new(http.clone(), key, base)),
            other => return Err(AppError::invalid(format!("Unknown provider: {other}"))),
        })
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Provider::Anthropic(_) => "anthropic",
            Provider::OpenAi(_) => "openai",
            Provider::Gemini(_) => "gemini",
            Provider::Demo => "demo",
        }
    }

    pub fn is_demo(&self) -> bool {
        matches!(self, Provider::Demo)
    }

    /// Starts a streaming call. Transport failures arrive as synthetic `error`
    /// events so the consumer handles every failure in one place.
    pub fn stream(&self, req: TurnRequest, cancel: CancellationToken) -> mpsc::Receiver<Value> {
        match self {
            Provider::Anthropic(c) => c.stream(req, cancel),
            Provider::OpenAi(c) => c.stream(req, cancel),
            Provider::Gemini(c) => c.stream(req, cancel),
            Provider::Demo => demo::stream(req, cancel),
        }
    }

    /// One-shot call whose reply must match `schema` (a JSON Schema object with
    /// `additionalProperties: false`). Returns the parsed JSON.
    pub async fn complete_json(&self, model: &str, system: &str, user: &str, name: &str, schema: Value) -> AppResult<Value> {
        let text = match self {
            Provider::Anthropic(c) => c.complete_json(model, system, user, schema).await?,
            Provider::OpenAi(c) => c.complete_json(model, system, user, name, schema).await?,
            Provider::Gemini(c) => c.complete_json(model, system, user, schema).await?,
            Provider::Demo => return Err(AppError::Provider("Demo mode has no model.".into())),
        };
        serde_json::from_str(text.trim()).map_err(|_| AppError::Provider("The model returned unreadable JSON.".into()))
    }

    /// One-shot plain-text call (chat titles).
    pub async fn complete_text(&self, model: &str, system: &str, user: &str) -> AppResult<String> {
        match self {
            Provider::Anthropic(c) => c.complete_text(model, system, user).await,
            Provider::OpenAi(c) => c.complete_text(model, system, user).await,
            Provider::Gemini(c) => c.complete_text(model, system, user).await,
            Provider::Demo => Err(AppError::Provider("Demo mode has no model.".into())),
        }
    }

    /// Verifies the key without spending tokens.
    pub async fn check_key(&self) -> AppResult<()> {
        match self {
            Provider::Anthropic(c) => c.check_key().await,
            Provider::OpenAi(c) => c.check_key().await,
            Provider::Gemini(c) => c.check_key().await,
            Provider::Demo => Ok(()),
        }
    }
}

pub fn error_event(message: impl Into<String>, retryable: bool) -> Value {
    json!({
        "type": "error",
        "error": { "type": "client_error", "message": message.into() },
        "retryable": retryable,
    })
}

/// Removes app-private fields (keys starting with `_`, such as Gemini thought
/// signatures) from content blocks before they go to a provider that would
/// reject unknown fields.
pub fn strip_private(blocks: &[Value]) -> Vec<Value> {
    blocks
        .iter()
        .map(|b| match b.as_object() {
            Some(obj) => Value::Object(obj.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.clone())).collect::<Map<_, _>>()),
            None => b.clone(),
        })
        .collect()
}

/// Text of a tool_result's content, which may be a string or a list of text blocks.
pub fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_fields_are_stripped() {
        let blocks = vec![json!({"type": "tool_use", "id": "t", "name": "n", "input": {}, "_thought_signature": "sig"})];
        let clean = strip_private(&blocks);
        assert!(clean[0].get("_thought_signature").is_none());
        assert_eq!(clean[0]["id"], "t");
    }

    #[test]
    fn tool_result_text_handles_shapes() {
        assert_eq!(tool_result_text(&json!("ok")), "ok");
        assert_eq!(tool_result_text(&json!([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}])), "a\nb");
    }
}
