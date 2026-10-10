use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{Artifact, ArtifactInput};

const COLUMNS: &str = "id, course_id, conversation_id, kind, title, payload, created_at, updated_at";
pub const KINDS: &[&str] = &[
    "interactive",
    "quiz",
    "flashcards",
    "worked_example",
    "lesson_plan",
    "rubric",
    "table",
    "note",
];

fn from_row(r: &Row) -> rusqlite::Result<Artifact> {
    let payload: String = r.get(5)?;
    Ok(Artifact {
        id: r.get(0)?,
        course_id: r.get(1)?,
        conversation_id: r.get(2)?,
        kind: r.get(3)?,
        title: r.get(4)?,
        payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

/// Saved items, newest first. `course_id = None` lists every course.
pub fn list(
    conn: &Connection,
    course_id: Option<&str>,
    kind: Option<&str>,
    query: Option<&str>,
) -> AppResult<Vec<Artifact>> {
    let like = query
        .map(|q| q.trim())
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", q.replace('%', "\\%").replace('_', "\\_")));
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM artifacts
         WHERE (?1 IS NULL OR course_id = ?1)
           AND (?2 IS NULL OR kind = ?2)
           AND (?3 IS NULL OR title LIKE ?3 ESCAPE '\\')
         ORDER BY updated_at DESC LIMIT 500"
    ))?;
    let rows = stmt
        .query_map(params![course_id, kind, like], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Artifact> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM artifacts WHERE id = ?1"), params![id], from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("Saved item"))
}

pub fn create(conn: &Connection, input: &ArtifactInput) -> AppResult<Artifact> {
    if !KINDS.contains(&input.kind.as_str()) {
        return Err(AppError::invalid(format!("Unknown item type: {}", input.kind)));
    }
    let title = input.title.trim();
    let title: String = if title.is_empty() { "Untitled".into() } else { title.chars().take(160).collect() };
    let id = new_id();
    let ts = now();
    conn.execute(
        &format!("INSERT INTO artifacts ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)"),
        params![
            id,
            input.course_id,
            input.conversation_id,
            input.kind,
            title,
            serde_json::to_string(&input.payload)?,
            ts
        ],
    )?;
    get(conn, &id)
}

pub fn rename(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::invalid("Titles can't be empty."));
    }
    let title: String = title.chars().take(160).collect();
    conn.execute(
        "UPDATE artifacts SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, title, now()],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id])?;
    Ok(())
}
