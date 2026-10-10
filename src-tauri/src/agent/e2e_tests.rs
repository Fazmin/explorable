//! End-to-end runs of the agent loop through a real IPC channel, using the
//! scripted demo provider. They cover routing, streaming, tool validation and
//! execution, persistence, cancellation, retry, and the choice pause.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::mpsc;

use super::*;
use crate::db::Db;
use crate::models::{Mode, Style};
use crate::repo;

fn deps(db: Arc<Db>) -> Deps {
    Deps {
        db,
        http: reqwest::Client::new(),
        secrets: Arc::new(Secrets::with_keys(&[])),
        runs: Arc::new(RunRegistry::default()),
    }
}

fn channel() -> (Channel<ChatEvent>, mpsc::UnboundedReceiver<Value>) {
    let (tx, rx) = mpsc::unbounded_channel::<Value>();
    let ch = Channel::new(move |body| {
        if let InvokeResponseBody::Json(s) = body {
            let _ = tx.send(serde_json::from_str(&s).expect("event JSON"));
        }
        Ok(())
    });
    (ch, rx)
}

async fn collect(mut rx: mpsc::UnboundedReceiver<Value>) -> Vec<Value> {
    let mut events = Vec::new();
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .expect("run timed out")
            .expect("channel closed before finishing");
        let done = ev["type"] == "finished";
        events.push(ev);
        if done {
            return events;
        }
    }
}

async fn run(d: &Deps, conversation_id: &str, input: TurnInput) -> Vec<Value> {
    let (ch, rx) = channel();
    start(d.clone(), conversation_id.to_string(), input, ch).expect("run starts");
    collect(rx).await
}

fn types(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap().to_string()).collect()
}

fn setup(mode: Mode) -> (Deps, String) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let conv = db.with(|c| repo::conversations::create(c, None, mode, Style::Direct)).unwrap();
    (deps(db), conv.id)
}

/// The stored history must alternate roles and pair every tool call with a result.
fn assert_valid_history(d: &Deps, conversation_id: &str) {
    let stored = d.db.with(|c| repo::messages::list(c, conversation_id)).unwrap();
    let api = history::to_api_messages(&stored, "anthropic");
    for pair in api.windows(2) {
        assert_ne!(pair[0]["role"], pair[1]["role"], "roles must alternate");
    }
    for (i, m) in api.iter().enumerate() {
        if m["role"] != "assistant" {
            continue;
        }
        let ids = history::tool_use_ids(&m["content"]);
        if ids.is_empty() {
            continue;
        }
        let Some(next) = api.get(i + 1) else { continue };
        for id in ids {
            assert!(
                next["content"].as_array().unwrap().iter().any(|b| b["tool_use_id"] == id.as_str()),
                "tool call {id} has no result"
            );
        }
    }
}

#[tokio::test]
async fn text_turn_streams_and_persists() {
    let (d, conv) = setup(Mode::Learn);
    let events = run(&d, &conv, TurnInput::Text("Hello there".into())).await;
    let t = types(&events);
    assert_eq!(t[0], "started");
    assert_eq!(t[1], "userMessage");
    assert!(t.contains(&"titleUpdated".to_string()));
    let route = events.iter().find(|e| e["type"] == "route").unwrap();
    assert_eq!(route["decision"]["presentation"], "text");
    assert_eq!(route["decision"]["source"], "demo");
    assert!(t.iter().filter(|x| *x == "textDelta").count() > 3);
    assert_eq!(t.last().unwrap(), "finished");
    assert_eq!(events.last().unwrap()["cancelled"], false);
    let stored = d.db.with(|c| repo::messages::list(c, &conv)).unwrap();
    assert_eq!(stored.len(), 2);
    assert!(stored[1].content[0]["text"].as_str().unwrap().contains("demo mode"));
    assert_eq!(stored[0].meta["route"]["presentation"], "text");
}

#[tokio::test]
async fn quiz_turn_renders_runs_tool_and_closes() {
    let (d, conv) = setup(Mode::Learn);
    let events = run(&d, &conv, TurnInput::Text("Quiz me on cellular respiration".into())).await;
    let t = types(&events);
    assert!(t.contains(&"toolInput".to_string()), "tool arguments stream");
    let stop = events
        .iter()
        .find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use")
        .expect("tool block completes");
    assert_eq!(stop["block"]["name"], "render_quiz");
    assert_eq!(stop["block"]["input"]["questions"].as_array().unwrap().len(), 5);
    let results = events.iter().find(|e| e["type"] == "toolResults").expect("tool results");
    assert!(results["message"]["content"][0]["is_error"].is_null(), "quiz input validates");
    assert_eq!(t.iter().filter(|x| *x == "assistantStart").count(), 2, "a closing turn follows the tool");
    assert!(!t.contains(&"error".to_string()));
    let stored = d.db.with(|c| repo::messages::list(c, &conv)).unwrap();
    assert_eq!(stored.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(), vec!["user", "assistant", "user", "assistant"]);
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn interactive_streams_css_before_html_and_scripts_last() {
    let (d, conv) = setup(Mode::Learn);
    let events = run(&d, &conv, TurnInput::Text("Visualize a Fourier series".into())).await;
    let partials: Vec<&Value> = events.iter().filter(|e| e["type"] == "toolInput").collect();
    assert!(partials.len() > 5, "many partial snapshots");
    let first_with = |key: &str| partials.iter().position(|p| p["completeKeys"].as_array().unwrap().iter().any(|k| k == key));
    let css = first_with("css").expect("css completes");
    let html = first_with("html").expect("html completes");
    assert!(css < html);
    // Partial HTML is visible before html completes.
    assert!(partials[..html].iter().any(|p| p["partial"]["html"].as_str().map(|h| h.len() > 50).unwrap_or(false)));
    let stop = events.iter().find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use").unwrap();
    assert_eq!(stop["block"]["input"]["jsExpressions"], json!(["fxInit();"]));
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn flashcards_are_saved_as_a_deck() {
    let (d, conv) = setup(Mode::Learn);
    run(&d, &conv, TurnInput::Text("Make flashcards for organic functional groups".into())).await;
    let decks = d.db.with(|c| repo::decks::list(c, None)).unwrap();
    assert_eq!(decks.len(), 1);
    assert_eq!(decks[0].card_count, 8);
    assert_eq!(decks[0].new_count, 8);
}

#[tokio::test]
async fn teach_mode_routes_lesson_plans_and_rubrics() {
    let (d, conv) = setup(Mode::Teach);
    let events = run(&d, &conv, TurnInput::Text("Plan a 50-minute lesson on Bayesian inference".into())).await;
    let stop = events.iter().find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use").unwrap();
    assert_eq!(stop["block"]["name"], "render_lesson_plan");
    let results = events.iter().find(|e| e["type"] == "toolResults").unwrap();
    assert!(results["message"]["content"][0]["content"].as_str().unwrap().starts_with("Rendered the lesson plan."));

    let events = run(&d, &conv, TurnInput::Text("Create a rubric for a research paper".into())).await;
    let stop = events.iter().find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use").unwrap();
    assert_eq!(stop["block"]["name"], "render_rubric");
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn cancel_keeps_partial_text_and_history_stays_valid() {
    let (d, conv) = setup(Mode::Learn);
    let (ch, mut rx) = channel();
    start(d.clone(), conv.clone(), TurnInput::Text("Visualize a Fourier series".into()), ch).unwrap();
    // Cancel once the tool call starts streaming.
    let mut events = Vec::new();
    loop {
        let ev = tokio::time::timeout(Duration::from_secs(20), rx.recv()).await.unwrap().unwrap();
        let is_tool = ev["type"] == "blockStart" && ev["block"]["type"] == "tool_use";
        events.push(ev);
        if is_tool {
            assert!(d.runs.cancel(&conv));
            break;
        }
    }
    let rest = collect(rx).await;
    assert_eq!(rest.last().unwrap()["cancelled"], true);
    let stored = d.db.with(|c| repo::messages::list(c, &conv)).unwrap();
    let last = stored.last().unwrap();
    assert_eq!(last.role, "assistant");
    assert!(last.content.as_array().unwrap().iter().all(|b| b["type"] == "text"), "incomplete tool call dropped");
    // The chat continues normally afterwards.
    let events = run(&d, &conv, TurnInput::Text("What is entropy?".into())).await;
    assert!(!types(&events).contains(&"error".to_string()));
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn missing_key_fails_retryably_then_retry_succeeds() {
    let (d, conv) = setup(Mode::Learn);
    d.db.with(|c| repo::settings::patch(c, &json!({ "provider": "anthropic" }))).unwrap();
    let events = run(&d, &conv, TurnInput::Text("Hello".into())).await;
    let err = events.iter().find(|e| e["type"] == "error").expect("error event");
    assert_eq!(err["retryable"], true);
    assert!(err["message"].as_str().unwrap().contains("API key"));

    d.db.with(|c| repo::settings::patch(c, &json!({ "provider": "demo" }))).unwrap();
    let events = run(&d, &conv, TurnInput::Retry).await;
    let t = types(&events);
    assert!(!t.contains(&"userMessage".to_string()), "retry does not repeat the question");
    assert!(t.contains(&"assistantDone".to_string()));
    assert!(matches!(
        start(d.clone(), conv.clone(), TurnInput::Retry, channel().0),
        Err(AppError::Invalid(_))
    ), "nothing left to retry");
}

#[tokio::test]
async fn concurrent_runs_in_one_chat_are_rejected() {
    let (d, conv) = setup(Mode::Learn);
    let (ch, rx) = channel();
    start(d.clone(), conv.clone(), TurnInput::Text("Visualize a Fourier series".into()), ch).unwrap();
    let second = start(d.clone(), conv.clone(), TurnInput::Text("again".into()), channel().0);
    assert!(second.is_err());
    collect(rx).await;
}

#[tokio::test]
async fn choice_answers_resume_the_run() {
    let (d, conv) = setup(Mode::Learn);
    // Simulate an assistant that asked a question and is waiting.
    d.db.with(|c| {
        repo::messages::append(c, "u1", &conv, "user", &json!([{ "type": "text", "text": "Quiz me" }]), &json!({}))?;
        repo::messages::append(
            c,
            "a1",
            &conv,
            "assistant",
            &json!([{ "type": "tool_use", "id": "toolu_q", "name": "ask_choice",
                      "input": { "question": "Which level?", "options": [{ "label": "Intro" }, { "label": "Advanced" }] } }]),
            &json!({}),
        )?;
        repo::conversations::set_pending(c, &conv, Some(&PendingChoice { tool_use_id: "toolu_q".into(), other_results: vec![] }))
    })
    .unwrap();
    assert!(start(d.clone(), conv.clone(), TurnInput::Choice { tool_use_id: "wrong".into(), answer: "Intro".into() }, channel().0).is_err());

    let events = run(&d, &conv, TurnInput::Choice { tool_use_id: "toolu_q".into(), answer: "Intro".into() }).await;
    let user = events.iter().find(|e| e["type"] == "userMessage").unwrap();
    assert_eq!(user["message"]["content"][0]["content"], "The user chose: Intro");
    assert!(types(&events).contains(&"assistantDone".to_string()));
    let conv_row = d.db.with(|c| repo::conversations::get(c, &conv)).unwrap();
    assert!(conv_row.pending.is_none());
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn typing_instead_of_choosing_answers_the_open_question() {
    let (d, conv) = setup(Mode::Learn);
    d.db.with(|c| {
        repo::messages::append(c, "u1", &conv, "user", &json!([{ "type": "text", "text": "Help" }]), &json!({}))?;
        repo::messages::append(c, "a1", &conv, "assistant", &json!([{ "type": "tool_use", "id": "toolu_q", "name": "ask_choice", "input": {} }]), &json!({}))?;
        repo::conversations::set_pending(c, &conv, Some(&PendingChoice { tool_use_id: "toolu_q".into(), other_results: vec![] }))
    })
    .unwrap();
    run(&d, &conv, TurnInput::Text("Actually, explain entropy".into())).await;
    assert_valid_history(&d, &conv);
}

/// Records demo runs of every starter prompt (tests/fixtures/prompts.json)
/// for the browser test harness:
/// `cargo test dump_demo_fixtures -- --ignored`
#[tokio::test]
#[ignore]
async fn dump_demo_fixtures() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures");
    let prompts: serde_json::Map<String, Value> =
        serde_json::from_str(&std::fs::read_to_string(base.join("prompts.json")).unwrap()).unwrap();
    let mut out = serde_json::Map::new();
    for (mode, list) in &prompts {
        for prompt in list.as_array().unwrap().iter().filter_map(Value::as_str) {
            let (d, conv) = setup(if mode == "teach" { Mode::Teach } else { Mode::Learn });
            let events = run(&d, &conv, TurnInput::Text(prompt.into())).await;
            out.insert(format!("{mode}:{prompt}"), Value::Array(events));
        }
    }
    std::fs::write(base.join("demo-events.json"), serde_json::to_string(&out).unwrap()).unwrap();
}

// ---- OpenAI and Gemini through a local mock server --------------------------

/// A minimal HTTP server that answers each request with the next queued
/// event-stream body and records (request line, headers, JSON body).
async fn mock_server(bodies: Vec<String>) -> (String, Arc<std::sync::Mutex<Vec<(String, String, Value)>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        for body in bodies {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let (head_end, length) = loop {
                let n = sock.read(&mut chunk).await.unwrap();
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_string();
                    let len = head
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    break (pos + 4, len);
                }
            };
            while buf.len() < head_end + length {
                let n = sock.read(&mut chunk).await.unwrap();
                buf.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let request_line = head.lines().next().unwrap_or_default().to_string();
            let json: Value = serde_json::from_slice(&buf[head_end..head_end + length]).unwrap_or(Value::Null);
            log.lock().unwrap().push((request_line, head.to_lowercase(), json));
            let content_type = if body.starts_with('{') { "application/json" } else { "text/event-stream" };
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}");
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.shutdown().await.ok();
        }
    });
    (format!("http://{addr}"), seen)
}

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|e| format!("event: {}\ndata: {}\n\n", e["type"].as_str().unwrap_or("message"), e))
        .collect()
}

fn gemini_sse(chunks: &[Value]) -> String {
    chunks.iter().map(|c| format!("data: {c}\r\n\r\n")).collect()
}

fn quiz_args() -> Value {
    json!({"title": "Quick check", "topic": "Cells", "purpose": "practice", "questions": [
        {"type": "true_false", "prompt": "Mitochondria make ATP.", "answer_bool": true, "explanation": "They host oxidative phosphorylation."}
    ]})
}

fn setup_provider(provider: &str, key: (&str, &str), base: &str) -> (Deps, String) {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let base_key = format!("{provider}BaseUrl");
    db.with(|c| repo::settings::patch(c, &json!({ "provider": provider, "routerEnabled": false, base_key.as_str(): base }))).unwrap();
    let conv = db.with(|c| repo::conversations::create(c, None, Mode::Learn, Style::Direct)).unwrap();
    let d = Deps { db, http: reqwest::Client::new(), secrets: Arc::new(Secrets::with_keys(&[key])), runs: Arc::new(RunRegistry::default()) };
    (d, conv.id)
}

#[tokio::test]
async fn openai_tool_loop_round_trips_reasoning_and_calls() {
    let args = quiz_args().to_string();
    let (a, b) = args.split_at(args.len() / 2);
    let first = sse(&[
        json!({"type": "response.created", "response": {"id": "resp_1"}}),
        json!({"type": "response.output_item.added", "output_index": 0, "item": {"type": "reasoning", "id": "rs_1", "summary": []}}),
        json!({"type": "response.reasoning_summary_text.delta", "item_id": "rs_1", "delta": "Checking what to ask."}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "enc_1"}}),
        json!({"type": "response.output_item.added", "output_index": 1, "item": {"type": "message", "id": "msg_1", "role": "assistant", "content": []}}),
        json!({"type": "response.output_text.delta", "item_id": "msg_1", "delta": "Here's a quick check."}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": {"type": "message", "id": "msg_1", "phase": "commentary"}}),
        json!({"type": "response.output_item.added", "output_index": 2, "item": {"type": "function_call", "id": "fc_1", "call_id": "call_A", "name": "render_quiz", "arguments": ""}}),
        json!({"type": "response.function_call_arguments.delta", "item_id": "fc_1", "delta": a}),
        json!({"type": "response.function_call_arguments.delta", "item_id": "fc_1", "delta": b}),
        json!({"type": "response.output_item.done", "output_index": 2, "item": {"type": "function_call", "id": "fc_1", "call_id": "call_A", "name": "render_quiz", "arguments": args}}),
        json!({"type": "response.completed", "response": {"usage": {"input_tokens": 100, "output_tokens": 50}}}),
    ]);
    let second = sse(&[
        json!({"type": "response.created", "response": {"id": "resp_2"}}),
        json!({"type": "response.output_item.added", "output_index": 0, "item": {"type": "message", "id": "msg_2", "role": "assistant", "content": []}}),
        json!({"type": "response.output_text.delta", "item_id": "msg_2", "delta": "Good luck!"}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": {"type": "message", "id": "msg_2", "phase": "final_answer"}}),
        json!({"type": "response.completed", "response": {"usage": {}}}),
    ]);
    let (base, seen) = mock_server(vec![first, second]).await;
    let (d, conv) = setup_provider("openai", ("openai", "sk-test"), &format!("{base}/v1"));
    let events = run(&d, &conv, TurnInput::Text("Quiz me on cells".into())).await;

    assert!(!types(&events).contains(&"error".to_string()), "{events:?}");
    assert!(events.iter().any(|e| e["type"] == "thinkingDelta"), "reasoning summaries stream");
    let stop = events.iter().find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use").unwrap();
    assert_eq!(stop["block"]["input"]["title"], "Quick check");
    let results = events.iter().find(|e| e["type"] == "toolResults").unwrap();
    assert!(results["message"]["content"][0]["is_error"].is_null());

    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].0.starts_with("POST /v1/responses"));
    assert!(requests[0].1.contains("authorization: bearer sk-test"));
    let input = requests[1].2["input"].as_array().unwrap().clone();
    let kinds: Vec<String> = input.iter().map(|i| i["type"].as_str().unwrap_or("message").to_string()).collect();
    assert_eq!(kinds, vec!["message", "reasoning", "message", "function_call", "function_call_output"]);
    assert_eq!(input[1]["encrypted_content"], "enc_1");
    assert_eq!(input[2]["phase"], "commentary");
    assert_eq!(input[3]["call_id"], "call_A");
    assert_eq!(input[4]["call_id"], "call_A");

    let stored = d.db.with(|c| repo::messages::list(c, &conv)).unwrap();
    assert_eq!(stored[1].meta["provider"], "openai");
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn gemini_tool_loop_round_trips_thought_signatures() {
    let first = gemini_sse(&[
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Choosing a question.", "thought": true}]}}]}),
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Here's a quick check."}]}}]}),
        json!({"candidates": [{"content": {"role": "model", "parts": [
            {"functionCall": {"id": "fc_1", "name": "render_quiz", "args": quiz_args()}, "thoughtSignature": "sig_1"}
        ]}, "finishReason": "STOP"}], "usageMetadata": {"promptTokenCount": 120, "candidatesTokenCount": 60}}),
    ]);
    let second = gemini_sse(&[
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Good luck!"}]}}]}),
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "", "thoughtSignature": "sig_2"}]}, "finishReason": "STOP"}]}),
    ]);
    let (base, seen) = mock_server(vec![first, second]).await;
    let (d, conv) = setup_provider("gemini", ("gemini", "AIzaTestKey0123456789abcdef"), &format!("{base}/v1beta"));
    let events = run(&d, &conv, TurnInput::Text("Quiz me on cells".into())).await;

    assert!(!types(&events).contains(&"error".to_string()), "{events:?}");
    let stop = events.iter().find(|e| e["type"] == "blockStop" && e["block"]["type"] == "tool_use").unwrap();
    assert_eq!(stop["block"]["id"], "fc_1");
    assert_eq!(stop["block"]["_thought_signature"], "sig_1");

    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].0.starts_with("POST /v1beta/models/gemini-3.8-flash:streamGenerateContent?alt=sse"));
    assert!(requests[0].1.contains("x-goog-api-key: aizatestkey"));
    let contents = requests[1].2["contents"].as_array().unwrap().clone();
    assert_eq!(contents.len(), 3);
    assert_eq!(contents[1]["role"], "model");
    let model_parts = contents[1]["parts"].as_array().unwrap();
    assert_eq!(model_parts.len(), 2, "text and call; the thought summary is not sent back");
    assert_eq!(model_parts[1]["functionCall"]["id"], "fc_1");
    assert_eq!(model_parts[1]["thoughtSignature"], "sig_1");
    let response = &contents[2]["parts"][0]["functionResponse"];
    assert_eq!(response["id"], "fc_1");
    assert_eq!(response["name"], "render_quiz");
    assert!(response["response"]["result"].as_str().unwrap().starts_with("Rendered the quiz"));

    // The closing answer kept its trailing signature for the next turn.
    let stored = d.db.with(|c| repo::messages::list(c, &conv)).unwrap();
    let last = stored.last().unwrap();
    assert_eq!(last.meta["provider"], "gemini");
    assert_eq!(last.content[0]["_gemini_segments"], json!([[10, null], [0, "sig_2"]]));
    assert_valid_history(&d, &conv);
}

#[tokio::test]
async fn provider_without_key_explains_what_to_do() {
    let (d, conv) = setup_provider("gemini", ("openai", "sk-x"), "");
    let events = run(&d, &conv, TurnInput::Text("Hello".into())).await;
    let err = events.iter().find(|e| e["type"] == "error").unwrap();
    assert!(err["message"].as_str().unwrap().contains("Gemini API key"));
}

#[tokio::test]
async fn router_works_with_openai_and_gemini_structured_output() {
    let decision = json!({"presentation": "quiz", "reason": "The learner asked to be tested."}).to_string();
    let openai_reply = json!({"output": [
        {"type": "reasoning", "id": "rs_1", "summary": []},
        {"type": "message", "content": [{"type": "output_text", "text": decision}]}
    ]})
    .to_string();
    let (base, seen) = mock_server(vec![openai_reply]).await;
    let provider = Provider::OpenAi(crate::llm::openai::OpenAiClient::new(reqwest::Client::new(), "sk-t".into(), Some(format!("{base}/v1"))));
    let d = router::route(&provider, "gpt-6-luna", Mode::Learn, &[("user".into(), "Quiz me".into())]).await.unwrap();
    assert_eq!(d.renderer, "render_quiz");
    let body = seen.lock().unwrap()[0].2.clone();
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(body["text"]["format"]["strict"], true);
    assert_eq!(body["model"], "gpt-6-luna");

    let gemini_reply = json!({"candidates": [{"content": {"parts": [
        {"text": "thinking", "thought": true},
        {"text": decision}
    ]}, "finishReason": "STOP"}]})
    .to_string();
    let (base, seen) = mock_server(vec![gemini_reply]).await;
    let provider = Provider::Gemini(crate::llm::gemini::GeminiClient::new(reqwest::Client::new(), "AIzaTestKey0123456789".into(), Some(format!("{base}/v1beta"))));
    let d = router::route(&provider, "gemini-3.1-flash-lite", Mode::Teach, &[("user".into(), "Quiz me".into())]).await.unwrap();
    assert_eq!(d.presentation, "quiz");
    let (line, _, body) = seen.lock().unwrap()[0].clone();
    assert!(line.starts_with("POST /v1beta/models/gemini-3.1-flash-lite:generateContent"));
    assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
    assert!(body["generationConfig"]["responseJsonSchema"]["properties"]["presentation"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("rubric")), "teach mode offers instructor formats");
}

#[tokio::test]
async fn openai_pro_model_answers_without_streaming() {
    let reply = json!({"status": "completed", "usage": {"input_tokens": 50, "output_tokens": 20}, "output": [
        {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "enc"},
        {"type": "message", "id": "msg_1", "phase": "final_answer", "content": [{"type": "output_text", "text": "Entropy measures the number of microstates.", "annotations": []}]}
    ]})
    .to_string();
    let (base, seen) = mock_server(vec![reply]).await;
    let (d, conv) = setup_provider("openai", ("openai", "sk-test"), &format!("{base}/v1"));
    d.db.with(|c| repo::settings::patch(c, &json!({ "openaiModel": "gpt-5.5-pro", "effort": "low" }))).unwrap();
    let events = run(&d, &conv, TurnInput::Text("What is entropy?".into())).await;
    assert!(!types(&events).contains(&"error".to_string()), "{events:?}");
    let done = events.iter().find(|e| e["type"] == "assistantDone").unwrap();
    assert_eq!(done["message"]["content"].as_array().unwrap().iter().find(|b| b["type"] == "text").unwrap()["text"], "Entropy measures the number of microstates.");
    let body = seen.lock().unwrap()[0].2.clone();
    assert_eq!(body["stream"], false);
    assert_eq!(body["model"], "gpt-5.5-pro");
    assert_eq!(body["reasoning"]["effort"], "medium", "pro models start at medium effort");
}
