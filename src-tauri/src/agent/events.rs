use serde::Serialize;
use serde_json::Value;

use crate::models::StoredMessage;

/// The presentation chosen for a user turn.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteDecision {
    pub presentation: String,
    /// The tool that renders it, or "text".
    pub renderer: String,
    pub reason: String,
    /// "router" (classifier model), "demo" (keyword rules), or "model" (the
    /// router was off or failed and the tutor chose itself).
    pub source: String,
}

/// Events streamed to the UI over a Tauri channel during one run.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChatEvent {
    Started { run_id: String, conversation_id: String },
    UserMessage { message: StoredMessage },
    Route { message_id: String, decision: RouteDecision },
    AssistantStart { message_id: String },
    BlockStart { message_id: String, index: usize, block: Value },
    TextDelta { message_id: String, index: usize, text: String },
    ThinkingDelta { message_id: String, index: usize, text: String },
    /// A throttled snapshot of a tool's arguments while they stream.
    ToolInput { message_id: String, index: usize, partial: Value, complete_keys: Vec<String> },
    BlockStop { message_id: String, index: usize, block: Value },
    AssistantDone { message: StoredMessage },
    ToolResults { message: StoredMessage },
    /// The run paused for the user to choose; `other_results` are results of
    /// tools called alongside the question, shown while it waits.
    Awaiting { tool_use_id: String, other_results: Vec<Value> },
    Notice { level: String, text: String },
    TitleUpdated { conversation_id: String, title: String },
    Error { message: String, retryable: bool },
    Finished { cancelled: bool },
}
