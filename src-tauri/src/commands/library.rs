use tauri::State;

use crate::error::AppResult;
use crate::models::{Artifact, ArtifactInput};
use crate::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_artifacts(
    state: State<'_, AppState>,
    course_id: Option<String>,
    kind: Option<String>,
    query: Option<String>,
) -> AppResult<Vec<Artifact>> {
    state
        .db
        .with(|c| repo::artifacts::list(c, course_id.as_deref(), kind.as_deref(), query.as_deref()))
}

#[tauri::command]
pub async fn get_artifact(state: State<'_, AppState>, id: String) -> AppResult<Artifact> {
    state.db.with(|c| repo::artifacts::get(c, &id))
}

#[tauri::command]
pub async fn save_artifact(state: State<'_, AppState>, input: ArtifactInput) -> AppResult<Artifact> {
    state.db.with(|c| repo::artifacts::create(c, &input))
}

#[tauri::command]
pub async fn rename_artifact(state: State<'_, AppState>, id: String, title: String) -> AppResult<()> {
    state.db.with(|c| repo::artifacts::rename(c, &id, &title))
}

#[tauri::command]
pub async fn delete_artifact(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.with(|c| repo::artifacts::delete(c, &id))
}
