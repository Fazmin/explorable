use tauri::ipc::Channel;
use tauri::State;

use crate::agent::events::ChatEvent;
use crate::agent::{self, Deps, TurnInput};
use crate::error::{AppError, AppResult};
use crate::models::{Conversation, ConversationDetail, Mode, Style};
use crate::repo;
use crate::state::AppState;

fn deps(state: &AppState) -> Deps {
    Deps {
        db: state.db.clone(),
        http: state.http.clone(),
        secrets: state.secrets.clone(),
        runs: state.runs.clone(),
    }
}

fn parse_mode(mode: &str) -> AppResult<Mode> {
    match mode {
        "learn" => Ok(Mode::Learn),
        "teach" => Ok(Mode::Teach),
        _ => Err(AppError::invalid("Mode must be learn or teach.")),
    }
}

fn parse_style(style: &str) -> AppResult<Style> {
    match style {
        "direct" => Ok(Style::Direct),
        "socratic" => Ok(Style::Socratic),
        _ => Err(AppError::invalid("Style must be direct or socratic.")),
    }
}

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>, course_id: Option<String>, mode: String) -> AppResult<Vec<Conversation>> {
    let mode = parse_mode(&mode)?;
    state.db.with(|c| repo::conversations::list(c, course_id.as_deref(), mode))
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    course_id: Option<String>,
    mode: String,
    style: String,
) -> AppResult<Conversation> {
    let mode = parse_mode(&mode)?;
    let style = parse_style(&style)?;
    state.db.with(|c| repo::conversations::create(c, course_id.as_deref(), mode, style))
}

#[tauri::command]
pub async fn get_conversation(state: State<'_, AppState>, id: String) -> AppResult<ConversationDetail> {
    let (conversation, messages) = state.db.with(|c| {
        Ok((repo::conversations::get(c, &id)?, repo::messages::list(c, &id)?))
    })?;
    Ok(ConversationDetail { conversation, messages, running: state.runs.is_running(&id) })
}

#[tauri::command]
pub async fn rename_conversation(state: State<'_, AppState>, id: String, title: String) -> AppResult<()> {
    state.db.with(|c| repo::conversations::rename(c, &id, &title))
}

#[tauri::command]
pub async fn set_conversation_style(state: State<'_, AppState>, id: String, style: String) -> AppResult<()> {
    let style = parse_style(&style)?;
    state.db.with(|c| repo::conversations::set_style(c, &id, style))
}

#[tauri::command]
pub async fn delete_conversation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.runs.cancel(&id);
    state.db.with(|c| repo::conversations::delete(c, &id))
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    conversation_id: String,
    text: String,
    on_event: Channel<ChatEvent>,
) -> AppResult<String> {
    agent::start(deps(&state), conversation_id, TurnInput::Text(text), on_event)
}

#[tauri::command]
pub async fn answer_choice(
    state: State<'_, AppState>,
    conversation_id: String,
    tool_use_id: String,
    answer: String,
    on_event: Channel<ChatEvent>,
) -> AppResult<String> {
    agent::start(deps(&state), conversation_id, TurnInput::Choice { tool_use_id, answer }, on_event)
}

#[tauri::command]
pub async fn retry_run(state: State<'_, AppState>, conversation_id: String, on_event: Channel<ChatEvent>) -> AppResult<String> {
    agent::start(deps(&state), conversation_id, TurnInput::Retry, on_event)
}

#[tauri::command]
pub async fn cancel_run(state: State<'_, AppState>, conversation_id: String) -> AppResult<bool> {
    Ok(state.runs.cancel(&conversation_id))
}
