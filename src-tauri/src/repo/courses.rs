use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{Course, CourseInput};

const COLUMNS: &str = "id, name, code, level, discipline, description, color, created_at, updated_at";
const LEVELS: &[&str] = &["introductory", "undergraduate", "advanced", "graduate", "professional"];

fn from_row(r: &Row) -> rusqlite::Result<Course> {
    Ok(Course {
        id: r.get(0)?,
        name: r.get(1)?,
        code: r.get(2)?,
        level: r.get(3)?,
        discipline: r.get(4)?,
        description: r.get(5)?,
        color: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

fn clean(input: &CourseInput) -> AppResult<CourseInput> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("Give the course a name."));
    }
    if name.chars().count() > 120 {
        return Err(AppError::invalid("Course names can be up to 120 characters."));
    }
    let level = if LEVELS.contains(&input.level.as_str()) {
        input.level.clone()
    } else {
        "undergraduate".into()
    };
    Ok(CourseInput {
        name: name.to_string(),
        code: input.code.trim().chars().take(24).collect(),
        level,
        discipline: input.discipline.trim().chars().take(80).collect(),
        description: input.description.trim().chars().take(4000).collect(),
        color: input.color.trim().chars().take(24).collect(),
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<Course>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM courses ORDER BY name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Course>> {
    Ok(conn
        .query_row(&format!("SELECT {COLUMNS} FROM courses WHERE id = ?1"), params![id], from_row)
        .optional()?)
}

pub fn create(conn: &Connection, input: &CourseInput) -> AppResult<Course> {
    let c = clean(input)?;
    let id = new_id();
    let ts = now();
    conn.execute(
        &format!("INSERT INTO courses ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)"),
        params![id, c.name, c.code, c.level, c.discipline, c.description, c.color, ts],
    )?;
    get(conn, &id)?.ok_or_else(|| AppError::not_found("Course"))
}

pub fn update(conn: &Connection, id: &str, input: &CourseInput) -> AppResult<Course> {
    let c = clean(input)?;
    let n = conn.execute(
        "UPDATE courses SET name = ?2, code = ?3, level = ?4, discipline = ?5, description = ?6,
         color = ?7, updated_at = ?8 WHERE id = ?1",
        params![id, c.name, c.code, c.level, c.discipline, c.description, c.color, now()],
    )?;
    if n == 0 {
        return Err(AppError::not_found("Course"));
    }
    get(conn, id)?.ok_or_else(|| AppError::not_found("Course"))
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM courses WHERE id = ?1", params![id])?;
    Ok(())
}
