//! Scripted provider. It emits the same event objects as the Messages API
//! stream, with realistic pacing, so demo mode exercises the real pipeline:
//! text deltas, streamed tool arguments, tool results, and a closing turn.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::demo_content::{closing_only, script_for};
use super::TurnRequest;
use crate::models::Mode;

enum Step {
    Event(Value),
    Pause(u64),
}

pub fn stream(req: TurnRequest, cancel: CancellationToken) -> mpsc::Receiver<Value> {
    let (tx, rx) = mpsc::channel::<Value>(512);
    tokio::spawn(async move {
        for step in plan(&req) {
            match step {
                Step::Pause(ms) => {
                    // Tests run the pipeline nearly at full speed.
                    let ms = if cfg!(test) { 1 } else { ms };
                    tokio::select! {
                        _ = cancel.cancelled() => return,
                        _ = tokio::time::sleep(Duration::from_millis(ms)) => {}
                    }
                }
                Step::Event(v) => {
                    if cancel.is_cancelled() || tx.send(v).await.is_err() {
                        return;
                    }
                }
            }
        }
    });
    rx
}

fn last_user_text(messages: &[Value]) -> String {
    for m in messages.iter().rev() {
        if m["role"] != "user" {
            continue;
        }
        let texts: Vec<&str> = m["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    String::new()
}

/// The name of the tool whose result the last user message carries, if any.
fn answered_tool(messages: &[Value]) -> Option<String> {
    let last = messages.last()?;
    let blocks = last["content"].as_array()?;
    let result = blocks.iter().find(|b| b["type"] == "tool_result")?;
    let id = result["tool_use_id"].as_str()?;
    messages.iter().rev().find_map(|m| {
        m["content"].as_array()?.iter().find_map(|b| {
            (b["type"] == "tool_use" && b["id"] == id).then(|| b["name"].as_str().unwrap_or_default().to_string())
        })
    })
}

fn plan(req: &TurnRequest) -> Vec<Step> {
    let text = last_user_text(&req.messages);
    let presentation = req.presentation.clone().unwrap_or_else(|| "text".into());
    let script = script_for(&presentation, &text, req.mode == Mode::Teach);
    let mut steps = vec![
        Step::Pause(350),
        Step::Event(json!({
            "type": "message_start",
            "message": { "id": format!("msg_demo_{}", uuid::Uuid::new_v4().simple()), "type": "message",
                         "role": "assistant", "model": "demo", "content": [], "usage": { "input_tokens": 0, "output_tokens": 0 } }
        })),
    ];

    if let Some(tool) = answered_tool(&req.messages) {
        let closing = if script.closing.is_empty() { closing_only(&tool) } else { script.closing };
        push_text(&mut steps, 0, closing);
        finish(&mut steps, "end_turn");
        return steps;
    }

    let mut index = 0;
    if let Some(lead) = script.lead {
        push_text(&mut steps, index, lead);
        index += 1;
    }
    match script.tool {
        Some((name, input)) => {
            push_tool(&mut steps, index, name, &input);
            finish(&mut steps, "tool_use");
        }
        None => finish(&mut steps, "end_turn"),
    }
    steps
}

fn push_text(steps: &mut Vec<Step>, index: usize, text: &str) {
    steps.push(Step::Event(json!({
        "type": "content_block_start", "index": index, "content_block": { "type": "text", "text": "" }
    })));
    // Stream a few words at a time.
    let words: Vec<&str> = text.split_inclusive(' ').collect();
    for chunk in words.chunks(3) {
        steps.push(Step::Event(json!({
            "type": "content_block_delta", "index": index,
            "delta": { "type": "text_delta", "text": chunk.concat() }
        })));
        steps.push(Step::Pause(28));
    }
    steps.push(Step::Event(json!({ "type": "content_block_stop", "index": index })));
}

fn push_tool(steps: &mut Vec<Step>, index: usize, name: &str, input: &Value) {
    // Short enough for every provider's tool-call ID limits.
    let id = format!("toolu_demo_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
    steps.push(Step::Event(json!({
        "type": "content_block_start", "index": index,
        "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} }
    })));
    steps.push(Step::Pause(200));
    let serialized = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
    let chars: Vec<char> = serialized.chars().collect();
    // Large interactive payloads stream in a few seconds; small ones quickly.
    let (size, pause) = if chars.len() > 4000 { (36, 9) } else { (18, 14) };
    for piece in chars.chunks(size) {
        steps.push(Step::Event(json!({
            "type": "content_block_delta", "index": index,
            "delta": { "type": "input_json_delta", "partial_json": piece.iter().collect::<String>() }
        })));
        steps.push(Step::Pause(pause));
    }
    steps.push(Step::Event(json!({ "type": "content_block_stop", "index": index })));
}

fn finish(steps: &mut Vec<Step>, stop_reason: &str) {
    steps.push(Step::Event(json!({
        "type": "message_delta", "delta": { "stop_reason": stop_reason }, "usage": { "output_tokens": 0 }
    })));
    steps.push(Step::Event(json!({ "type": "message_stop" })));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(messages: Vec<Value>, presentation: &str) -> TurnRequest {
        TurnRequest {
            model: "demo".into(),
            system_static: String::new(),
            system_dynamic: String::new(),
            messages,
            tools: vec![],
            max_tokens: 1000,
            effort: "low".into(),
            refusal_fallback: false,
            mode: Mode::Learn,
            presentation: Some(presentation.into()),
        }
    }

    fn events(steps: Vec<Step>) -> Vec<Value> {
        steps
            .into_iter()
            .filter_map(|s| match s {
                Step::Event(v) => Some(v),
                Step::Pause(_) => None,
            })
            .collect()
    }

    #[test]
    fn interactive_script_streams_valid_tool_json() {
        let r = req(vec![json!({"role": "user", "content": [{"type": "text", "text": "visualize a fourier series"}]})], "interactive_explainer");
        let evs = events(plan(&r));
        let json: String = evs
            .iter()
            .filter(|e| e["delta"]["type"] == "input_json_delta")
            .map(|e| e["delta"]["partial_json"].as_str().unwrap().to_string())
            .collect();
        let input: Value = serde_json::from_str(&json).expect("streamed arguments form valid JSON");
        assert_eq!(input["jsExpressions"][0], "fxInit();");
        let stop = evs.iter().find(|e| e["type"] == "message_delta").unwrap();
        assert_eq!(stop["delta"]["stop_reason"], "tool_use");
    }

    #[test]
    fn tool_result_turn_closes_with_text() {
        let r = req(
            vec![
                json!({"role": "user", "content": [{"type": "text", "text": "quiz me"}]}),
                json!({"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "render_quiz", "input": {}}]}),
                json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "ok"}]}),
            ],
            "quiz",
        );
        let evs = events(plan(&r));
        assert!(evs.iter().all(|e| e["content_block"]["type"] != "tool_use"));
        let stop = evs.iter().find(|e| e["type"] == "message_delta").unwrap();
        assert_eq!(stop["delta"]["stop_reason"], "end_turn");
    }

    #[test]
    fn every_script_is_valid_json() {
        for p in ["simulation", "interactive_explainer", "quiz", "flashcards", "worked_example", "lesson_plan", "rubric", "table", "text"] {
            let s = script_for(p, "", false);
            if let Some((name, input)) = s.tool {
                let text = serde_json::to_string(&input).unwrap();
                assert!(serde_json::from_str::<Value>(&text).is_ok(), "{name}");
            }
        }
    }
}
