use tauri::State;

use crate::error::AppResult;
use crate::models::{Progress, QuizAttempt, QuizAttemptInput};
use crate::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn get_progress(state: State<'_, AppState>, course_id: Option<String>) -> AppResult<Progress> {
    state.db.with(|c| repo::progress::compute(c, course_id.as_deref()))
}

#[tauri::command]
pub async fn record_quiz_attempt(state: State<'_, AppState>, input: QuizAttemptInput) -> AppResult<QuizAttempt> {
    state.db.with(|c| repo::progress::record_attempt(c, &input))
}

#[tauri::command]
pub async fn quiz_attempts(state: State<'_, AppState>, quiz_key: String) -> AppResult<Vec<QuizAttempt>> {
    state.db.with(|c| repo::progress::attempts_for(c, &quiz_key))
}
