//! Turning stored messages into a valid Messages API history.

use serde_json::{json, Value};

use crate::llm::strip_private;
use crate::models::StoredMessage;

/// Which provider produced a stored assistant message.
pub fn origin(m: &StoredMessage) -> &str {
    if let Some(p) = m.meta["provider"].as_str() {
        return p;
    }
    match m.meta["model"].as_str() {
        Some(model) if model.starts_with("claude-") => "anthropic",
        Some("demo") => "demo",
        _ => "unknown",
    }
}

/// Stored messages → canonical messages for `target` provider. Reasoning
/// artifacts (thinking blocks, private signatures) only mean something to the
/// provider that produced them, so they are dropped from other providers'
/// messages. Consecutive messages with the same role are merged (for example
/// after a failed run followed by a new question), which is deterministic and
/// so keeps the request prefix stable.
pub fn to_api_messages(stored: &[StoredMessage], target: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for m in stored {
        let mut blocks = m.content.as_array().cloned().unwrap_or_default();
        if m.role == "assistant" && origin(m) != target {
            let kept: Vec<Value> = blocks
                .into_iter()
                .filter(|b| !matches!(b["type"].as_str(), Some("thinking") | Some("redacted_thinking")))
                .collect();
            blocks = strip_private(&kept);
        }
        if blocks.is_empty() {
            continue;
        }
        if out.is_empty() && m.role != "user" {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last["role"] == m.role.as_str() {
                if let Some(arr) = last["content"].as_array_mut() {
                    arr.extend(blocks);
                }
                continue;
            }
        }
        out.push(json!({ "role": m.role, "content": blocks }));
    }
    // tool_result blocks must come first in a user message.
    for msg in &mut out {
        if msg["role"] == "user" {
            if let Some(arr) = msg["content"].as_array_mut() {
                arr.sort_by_key(|b| if b["type"] == "tool_result" { 0 } else { 1 });
            }
        }
    }
    out
}

/// IDs of tool calls in an assistant message.
pub fn tool_use_ids(content: &Value) -> Vec<String> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "tool_use")
        .filter_map(|b| b["id"].as_str().map(str::to_string))
        .collect()
}

pub fn interrupted_results(ids: &[String]) -> Vec<Value> {
    ids.iter()
        .map(|id| {
            json!({
                "type": "tool_result",
                "tool_use_id": id,
                "content": "This tool call was interrupted before it finished.",
                "is_error": true
            })
        })
        .collect()
}

/// Cleans a finished assistant message before it is stored and replayed.
/// * Empty text blocks are dropped (the API rejects them).
/// * After a server-side fallback, blocks the declined model produced before
///   the last `fallback` marker are dropped, along with the markers.
pub fn sanitize_complete(blocks: Vec<Value>) -> Vec<Value> {
    let last_fallback = blocks.iter().rposition(|b| b["type"] == "fallback");
    blocks
        .into_iter()
        .enumerate()
        .filter(|(i, b)| {
            let kind = b["type"].as_str().unwrap_or_default();
            if kind == "fallback" {
                return false;
            }
            if let Some(f) = last_fallback {
                if *i < f && matches!(kind, "thinking" | "redacted_thinking" | "tool_use" | "server_tool_use") {
                    return false;
                }
            }
            // The API rejects text blocks that are empty or only whitespace.
            !(kind == "text" && b["text"].as_str().map(|t| t.trim().is_empty()).unwrap_or(true))
        })
        .map(|(_, b)| b)
        .collect()
}

/// Keeps what is safe to store from an answer that was cancelled mid-stream:
/// non-empty text and fully signed thinking. Tool calls are dropped because
/// their input may be incomplete and they never ran.
pub fn sanitize_partial(blocks: Vec<Value>) -> Vec<Value> {
    let blocks = sanitize_complete(blocks);
    let kept: Vec<Value> = blocks
        .into_iter()
        .filter(|b| match b["type"].as_str().unwrap_or_default() {
            "text" => true,
            "thinking" => b["signature"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "redacted_thinking" => true,
            _ => false,
        })
        .collect();
    // A message of only thinking blocks is not useful on its own.
    if kept.iter().any(|b| b["type"] == "text" && b["text"].as_str().is_some_and(|t| !t.trim().is_empty())) {
        kept
    } else {
        vec![]
    }
}

/// Recent (role, text) pairs for the router, oldest first.
pub fn recent_text(stored: &[StoredMessage], max: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = stored
        .iter()
        .filter_map(|m| {
            let text: Vec<&str> = m
                .content
                .as_array()?
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect();
            if text.is_empty() {
                None
            } else {
                Some((m.role.clone(), text.join("\n")))
            }
        })
        .collect();
    let start = out.len().saturating_sub(max);
    out.drain(..start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: Value) -> StoredMessage {
        StoredMessage {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: "c".into(),
            seq: 0,
            role: role.into(),
            content,
            meta: json!({}),
            created_at: String::new(),
        }
    }

    #[test]
    fn merges_consecutive_roles_and_orders_tool_results_first() {
        let stored = vec![
            msg("user", json!([{"type": "text", "text": "a"}])),
            msg("user", json!([{"type": "text", "text": "b"}, {"type": "tool_result", "tool_use_id": "t", "content": "x"}])),
            msg("assistant", json!([{"type": "text", "text": "c"}])),
        ];
        let api = to_api_messages(&stored, "anthropic");
        assert_eq!(api.len(), 2);
        assert_eq!(api[0]["content"][0]["type"], "tool_result");
        assert_eq!(api[0]["content"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn foreign_reasoning_is_dropped_when_switching_provider() {
        let mut gemini = msg("assistant", json!([
            {"type": "thinking", "thinking": "summary"},
            {"type": "tool_use", "id": "g1", "name": "n", "input": {}, "_thought_signature": "sig"}
        ]));
        gemini.meta = json!({"provider": "gemini"});
        let stored = vec![msg("user", json!([{"type": "text", "text": "q"}])), gemini];
        let for_claude = to_api_messages(&stored, "anthropic");
        assert_eq!(for_claude[1]["content"].as_array().unwrap().len(), 1);
        assert!(for_claude[1]["content"][0].get("_thought_signature").is_none());
        let for_gemini = to_api_messages(&stored, "gemini");
        assert_eq!(for_gemini[1]["content"][1]["_thought_signature"], "sig");
        let mut claude = msg("assistant", json!([{"type": "thinking", "thinking": "", "signature": "s"}, {"type": "text", "text": "a"}]));
        claude.meta = json!({"model": "claude-opus-5-5"});
        assert_eq!(origin(&claude), "anthropic");
    }

    #[test]
    fn fallback_blocks_are_cleaned() {
        let blocks = vec![
            json!({"type": "thinking", "thinking": "", "signature": "s1"}),
            json!({"type": "text", "text": "partial"}),
            json!({"type": "fallback", "from": {}, "to": {}}),
            json!({"type": "thinking", "thinking": "", "signature": "s2"}),
            json!({"type": "text", "text": ""}),
            json!({"type": "text", "text": "\n\n "}),
            json!({"type": "text", "text": "final"}),
        ];
        let clean = sanitize_complete(blocks);
        let kinds: Vec<&str> = clean.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(kinds, vec!["text", "thinking", "text"]);
        assert_eq!(clean[1]["signature"], "s2");
    }

    #[test]
    fn partial_keeps_text_and_signed_thinking_only() {
        let blocks = vec![
            json!({"type": "thinking", "thinking": "x", "signature": ""}),
            json!({"type": "text", "text": "hello"}),
            json!({"type": "tool_use", "id": "t", "name": "n", "input": {}}),
        ];
        let kept = sanitize_partial(blocks);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0]["text"], "hello");
        assert!(sanitize_partial(vec![json!({"type": "tool_use", "id": "t", "name": "n", "input": {}})]).is_empty());
        assert!(sanitize_partial(vec![json!({"type": "text", "text": "  \n"})]).is_empty());
    }

    #[test]
    fn recent_text_skips_tool_only_messages() {
        let stored = vec![
            msg("user", json!([{"type": "text", "text": "q1"}])),
            msg("assistant", json!([{"type": "tool_use", "id": "t", "name": "n", "input": {}}])),
            msg("user", json!([{"type": "tool_result", "tool_use_id": "t", "content": "r"}])),
            msg("assistant", json!([{"type": "text", "text": "a1"}])),
            msg("user", json!([{"type": "text", "text": "q2"}])),
        ];
        let r = recent_text(&stored, 2);
        assert_eq!(r, vec![("assistant".into(), "a1".into()), ("user".into(), "q2".into())]);
    }
}
