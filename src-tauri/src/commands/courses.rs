use tauri::State;

use crate::error::AppResult;
use crate::models::{Course, CourseInput};
use crate::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_courses(state: State<'_, AppState>) -> AppResult<Vec<Course>> {
    state.db.with(repo::courses::list)
}

#[tauri::command]
pub async fn create_course(state: State<'_, AppState>, input: CourseInput) -> AppResult<Course> {
    state.db.with(|c| repo::courses::create(c, &input))
}

#[tauri::command]
pub async fn update_course(state: State<'_, AppState>, id: String, input: CourseInput) -> AppResult<Course> {
    state.db.with(|c| repo::courses::update(c, &id, &input))
}

#[tauri::command]
pub async fn delete_course(state: State<'_, AppState>, id: String) -> AppResult<()> {
    // Stop any answers streaming into this course's chats first.
    let conversation_ids: Vec<String> = state.db.with(|c| {
        let mut stmt = c.prepare("SELECT id FROM conversations WHERE course_id = ?1")?;
        let ids = stmt.query_map([&id], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    })?;
    for cid in &conversation_ids {
        state.runs.cancel(cid);
    }
    state.db.with(|c| repo::courses::delete(c, &id))
}
