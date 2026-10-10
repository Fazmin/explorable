use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::db::now;
use crate::error::AppResult;
use crate::models::StoredMessage;

const COLUMNS: &str = "id, conversation_id, seq, role, content, meta, created_at";

fn from_row(r: &Row) -> rusqlite::Result<StoredMessage> {
    let content: String = r.get(4)?;
    let meta: String = r.get(5)?;
    Ok(StoredMessage {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        seq: r.get(2)?,
        role: r.get(3)?,
        content: serde_json::from_str(&content).unwrap_or(Value::Array(vec![])),
        meta: serde_json::from_str(&meta).unwrap_or(Value::Object(Default::default())),
        created_at: r.get(6)?,
    })
}

/// Appends a message. History is append-only: the API ties thinking blocks to
/// the exact prefix that produced them, so earlier content is never edited.
pub fn append(
    conn: &Connection,
    id: &str,
    conversation_id: &str,
    role: &str,
    content: &Value,
    meta: &Value,
) -> AppResult<StoredMessage> {
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM messages WHERE conversation_id = ?1",
        params![conversation_id],
        |r| r.get(0),
    )?;
    let ts = now();
    conn.execute(
        &format!("INSERT INTO messages ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"),
        params![
            id,
            conversation_id,
            seq,
            role,
            serde_json::to_string(content)?,
            serde_json::to_string(meta)?,
            ts
        ],
    )?;
    conn.execute(
        "UPDATE conversations SET updated_at = ?2 WHERE id = ?1",
        params![conversation_id, ts],
    )?;
    Ok(StoredMessage {
        id: id.to_string(),
        conversation_id: conversation_id.to_string(),
        seq,
        role: role.to_string(),
        content: content.clone(),
        meta: meta.clone(),
        created_at: ts,
    })
}

pub fn list(conn: &Connection, conversation_id: &str) -> AppResult<Vec<StoredMessage>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM messages WHERE conversation_id = ?1 ORDER BY seq"
    ))?;
    let rows = stmt
        .query_map(params![conversation_id], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn last(conn: &Connection, conversation_id: &str) -> AppResult<Option<StoredMessage>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM messages WHERE conversation_id = ?1 ORDER BY seq DESC LIMIT 1"),
            params![conversation_id],
            from_row,
        )
        .optional()?)
}

/// Metadata (routing decision, usage) is app-side bookkeeping that is never
/// sent to the model, so updating it does not alter the conversation prefix.
pub fn set_meta(conn: &Connection, id: &str, meta: &Value) -> AppResult<()> {
    conn.execute(
        "UPDATE messages SET meta = ?2 WHERE id = ?1",
        params![id, serde_json::to_string(meta)?],
    )?;
    Ok(())
}
