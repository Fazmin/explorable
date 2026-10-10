use rusqlite::{params, Connection, Row};

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{Material, MaterialHit};
use crate::text;

/// Imported text beyond this is rejected rather than silently truncated.
pub const MAX_MATERIAL_CHARS: usize = 2_000_000;

fn from_row(r: &Row) -> rusqlite::Result<Material> {
    Ok(Material {
        id: r.get(0)?,
        course_id: r.get(1)?,
        title: r.get(2)?,
        source_name: r.get(3)?,
        word_count: r.get(4)?,
        chunk_count: r.get(5)?,
        created_at: r.get(6)?,
    })
}

pub fn list(conn: &Connection, course_id: &str) -> AppResult<Vec<Material>> {
    let mut stmt = conn.prepare(
        "SELECT id, course_id, title, source_name, word_count, chunk_count, created_at
         FROM materials WHERE course_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![course_id], from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn titles(conn: &Connection, course_id: &str, limit: usize) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT title FROM materials WHERE course_id = ?1 ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![course_id, limit as i64], |r| r.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(rows)
}

pub fn content(conn: &Connection, id: &str) -> AppResult<(String, String)> {
    conn.query_row("SELECT title, content FROM materials WHERE id = ?1", params![id], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .map_err(|_| AppError::not_found("Material"))
}

/// Stores a document and indexes its chunks for full-text search.
pub fn insert(conn: &Connection, course_id: &str, title: &str, source_name: &str, body: &str) -> AppResult<Material> {
    let body = body.trim();
    if body.is_empty() {
        return Err(AppError::invalid("That material has no readable text."));
    }
    if body.chars().count() > MAX_MATERIAL_CHARS {
        return Err(AppError::invalid(
            "That material is over 2 million characters. Split it into smaller files and import them separately.",
        ));
    }
    let title = title.trim();
    let title: String = if title.is_empty() { "Untitled material".into() } else { title.chars().take(160).collect() };
    let chunks = text::chunk_text(body);
    let id = new_id();
    conn.execute(
        "INSERT INTO materials (id, course_id, title, source_name, content, word_count, chunk_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, course_id, title, source_name, body, text::word_count(body), chunks.len() as i64, now()],
    )?;
    let mut stmt = conn.prepare(
        "INSERT INTO material_chunks (content, title, material_id, course_id, ordinal) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (i, chunk) in chunks.iter().enumerate() {
        stmt.execute(params![chunk, title, id, course_id, i as i64])?;
    }
    let material = conn.query_row(
        "SELECT id, course_id, title, source_name, word_count, chunk_count, created_at FROM materials WHERE id = ?1",
        params![id],
        from_row,
    )?;
    Ok(material)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM materials WHERE id = ?1", params![id])?;
    Ok(())
}

/// BM25-ranked passages from one course's materials.
pub fn search(conn: &Connection, course_id: &str, query: &str, limit: usize) -> AppResult<Vec<MaterialHit>> {
    let Some(fts) = text::fts_query(query) else {
        return Ok(vec![]);
    };
    let mut stmt = conn.prepare(
        "SELECT material_id, title, ordinal, content,
                snippet(material_chunks, 0, '«', '»', '…', 28)
         FROM material_chunks
         WHERE material_chunks MATCH ?1 AND course_id = ?2
         ORDER BY bm25(material_chunks, 1.0, 0.4)
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![fts, course_id, limit.clamp(1, 12) as i64], |r| {
            Ok(MaterialHit {
                material_id: r.get(0)?,
                title: r.get(1)?,
                ordinal: r.get(2)?,
                content: r.get(3)?,
                snippet: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
