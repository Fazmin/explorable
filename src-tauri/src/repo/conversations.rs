use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{Conversation, Mode, PendingChoice, Style};

const COLUMNS: &str = "id, course_id, mode, style, title, pending, created_at, updated_at";

fn from_row(r: &Row) -> rusqlite::Result<Conversation> {
    let mode: String = r.get(2)?;
    let style: String = r.get(3)?;
    let pending: Option<String> = r.get(5)?;
    Ok(Conversation {
        id: r.get(0)?,
        course_id: r.get(1)?,
        mode: Mode::parse(&mode),
        style: Style::parse(&style),
        title: r.get(4)?,
        pending: pending.and_then(|p| serde_json::from_str::<PendingChoice>(&p).ok()),
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

/// Conversations for one course (or the general space when `course_id` is
/// `None`) and mode, most recently active first.
pub fn list(conn: &Connection, course_id: Option<&str>, mode: Mode) -> AppResult<Vec<Conversation>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM conversations
         WHERE course_id IS ?1 AND mode = ?2
         ORDER BY updated_at DESC LIMIT 200"
    ))?;
    let rows = stmt
        .query_map(params![course_id, mode.as_str()], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Conversation> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM conversations WHERE id = ?1"), params![id], from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("Conversation"))
}

pub fn create(conn: &Connection, course_id: Option<&str>, mode: Mode, style: Style) -> AppResult<Conversation> {
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO conversations (id, course_id, mode, style, title, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'New chat', ?5, ?5)",
        params![id, course_id, mode.as_str(), style.as_str(), ts],
    )?;
    get(conn, &id)
}

pub fn rename(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::invalid("Titles can't be empty."));
    }
    let title: String = title.chars().take(120).collect();
    conn.execute("UPDATE conversations SET title = ?2 WHERE id = ?1", params![id, title])?;
    Ok(())
}

pub fn set_style(conn: &Connection, id: &str, style: Style) -> AppResult<()> {
    conn.execute("UPDATE conversations SET style = ?2 WHERE id = ?1", params![id, style.as_str()])?;
    Ok(())
}

pub fn set_pending(conn: &Connection, id: &str, pending: Option<&PendingChoice>) -> AppResult<()> {
    let json = pending.map(serde_json::to_string).transpose()?;
    conn.execute("UPDATE conversations SET pending = ?2 WHERE id = ?1", params![id, json])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    Ok(())
}
