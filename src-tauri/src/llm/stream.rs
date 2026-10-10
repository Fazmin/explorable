//! The streaming loop shared by every HTTP provider: send with retries, read
//! server-sent events with an idle timeout, translate each event into
//! canonical events, and forward them until done or cancelled.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::error_event;
use super::sse::{SseDecoder, SseEvent};

const MAX_RETRIES: u32 = 2;
/// Providers send keep-alives or data regularly; this long without bytes
/// means the connection has stalled (network change, sleep).
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

pub enum ErrorAction {
    /// The adapter changed its request (for example, dropped an optional
    /// feature the server rejected); send again now.
    ResendNow,
    /// Transient: back off and retry, up to the retry limit.
    Retry,
    /// Give up with this message.
    Fail { message: String, retryable: bool },
}

pub trait StreamAdapter: Send + 'static {
    fn request(&self) -> reqwest::RequestBuilder;
    fn on_http_error(&mut self, status: StatusCode, body: &str) -> ErrorAction;
    /// Canonical events for one server-sent event.
    fn translate(&mut self, event: &SseEvent) -> Vec<Value>;
    /// Events to emit when the stream ends cleanly (closing open blocks).
    fn finish(&mut self) -> Vec<Value> {
        Vec::new()
    }
    /// Canonical events for a complete JSON response, for models that don't
    /// stream. Providers that always stream keep the default.
    fn translate_body(&mut self, _body: &str) -> Vec<Value> {
        vec![error_event("The provider sent an unexpected response.", true)]
    }
}

pub fn is_transient(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() || status.as_u16() == 529
}

pub fn spawn(adapter: impl StreamAdapter, cancel: CancellationToken) -> mpsc::Receiver<Value> {
    let (tx, rx) = mpsc::channel::<Value>(512);
    tokio::spawn(run(adapter, cancel, tx));
    rx
}

async fn run(mut adapter: impl StreamAdapter, cancel: CancellationToken, tx: mpsc::Sender<Value>) {
    let mut attempt = 0u32;
    let mut resends = 0u32;
    let response = loop {
        let sent = tokio::select! {
            _ = cancel.cancelled() => return,
            r = adapter.request().send() => r,
        };
        match sent {
            Ok(resp) if resp.status().is_success() => break resp,
            Ok(resp) => {
                let status = resp.status();
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .map(|s| s.ceil().max(1.0) as u64);
                let body = resp.text().await.unwrap_or_default();
                match adapter.on_http_error(status, &body) {
                    ErrorAction::ResendNow if resends < 2 => {
                        resends += 1;
                        continue;
                    }
                    ErrorAction::Retry if attempt < MAX_RETRIES => {
                        attempt += 1;
                        let wait = retry_after.unwrap_or(1 + 3 * u64::from(attempt)).min(20);
                        tokio::select! {
                            _ = cancel.cancelled() => return,
                            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                        }
                        continue;
                    }
                    ErrorAction::Fail { message, retryable } => {
                        let _ = tx.send(error_event(message, retryable)).await;
                        return;
                    }
                    _ => {
                        let _ = tx
                            .send(error_event(format!("The provider kept rejecting the request (HTTP {}). Retry in a moment.", status.as_u16()), true))
                            .await;
                        return;
                    }
                }
            }
            Err(e) => {
                if (e.is_connect() || e.is_timeout()) && attempt < MAX_RETRIES {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2 * u64::from(attempt))).await;
                    continue;
                }
                let _ = tx.send(error_event("Couldn't reach the AI provider. Check your internet connection and retry.", true)).await;
                return;
            }
        }
    };

    let is_json = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|ct| ct.contains("application/json"));
    if is_json {
        let text = tokio::select! {
            _ = cancel.cancelled() => return,
            t = response.text() => t.unwrap_or_default(),
        };
        for out in adapter.translate_body(&text) {
            if tx.send(out).await.is_err() {
                return;
            }
        }
        return;
    }

    let mut bytes = response.bytes_stream();
    let mut decoder = SseDecoder::default();
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => return,
            n = tokio::time::timeout(IDLE_TIMEOUT, bytes.next()) => n,
        };
        let Ok(next) = next else {
            let _ = tx.send(error_event("The connection stalled while the answer was streaming. Retry to continue.", true)).await;
            return;
        };
        match next {
            None => break,
            Some(Err(_)) => {
                let _ = tx.send(error_event("The connection dropped while the answer was streaming.", true)).await;
                return;
            }
            Some(Ok(chunk)) => {
                for ev in decoder.push(&chunk) {
                    for out in adapter.translate(&ev) {
                        if tx.send(out).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    }
    let mut tail = Vec::new();
    if let Some(ev) = decoder.finish() {
        tail.extend(adapter.translate(&ev));
    }
    tail.extend(adapter.finish());
    for out in tail {
        if tx.send(out).await.is_err() {
            return;
        }
    }
}
