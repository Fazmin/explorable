use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{Card, CardInput, Deck, DeckDetail, DueCard};
use crate::srs::{self, CardState};

const CARD_COLUMNS: &str = "id, deck_id, position, front, back, hint, ease, interval_days, reps, lapses,
     due_at, last_reviewed_at, created_at";

fn card_from_row(r: &Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        deck_id: r.get(1)?,
        position: r.get(2)?,
        front: r.get(3)?,
        back: r.get(4)?,
        hint: r.get(5)?,
        ease: r.get(6)?,
        interval_days: r.get(7)?,
        reps: r.get(8)?,
        lapses: r.get(9)?,
        due_at: r.get(10)?,
        last_reviewed_at: r.get(11)?,
        created_at: r.get(12)?,
    })
}

fn state_of(card: &Card) -> CardState {
    CardState {
        ease: card.ease,
        interval_days: card.interval_days,
        reps: card.reps,
        lapses: card.lapses,
    }
}

fn clean_card(input: &CardInput) -> AppResult<CardInput> {
    let front = input.front.trim();
    let back = input.back.trim();
    if front.is_empty() || back.is_empty() {
        return Err(AppError::invalid("Cards need text on both sides."));
    }
    Ok(CardInput {
        front: front.chars().take(2000).collect(),
        back: back.chars().take(4000).collect(),
        hint: input.hint.trim().chars().take(500).collect(),
    })
}

const DECK_SELECT: &str = "SELECT d.id, d.course_id, d.title, d.description,
        (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id),
        (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id
            AND (c.reps > 0 OR c.last_reviewed_at IS NOT NULL) AND c.due_at <= ?1),
        (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id AND c.reps = 0 AND c.last_reviewed_at IS NULL),
        d.created_at, d.updated_at
     FROM decks d";

fn deck_from_row(r: &Row) -> rusqlite::Result<Deck> {
    Ok(Deck {
        id: r.get(0)?,
        course_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        card_count: r.get(4)?,
        due_count: r.get(5)?,
        new_count: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

pub fn create(
    conn: &Connection,
    course_id: Option<&str>,
    conversation_id: Option<&str>,
    title: &str,
    description: &str,
    cards: &[CardInput],
) -> AppResult<Deck> {
    if cards.is_empty() {
        return Err(AppError::invalid("A deck needs at least one card."));
    }
    let cleaned = cards.iter().map(clean_card).collect::<AppResult<Vec<_>>>()?;
    let title = title.trim();
    let title: String = if title.is_empty() { "Untitled deck".into() } else { title.chars().take(160).collect() };
    let id = new_id();
    let ts = now();
    conn.execute(
        "INSERT INTO decks (id, course_id, title, description, source_conversation_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, course_id, title, description.trim(), conversation_id, ts],
    )?;
    let mut stmt = conn.prepare(&format!(
        "INSERT INTO cards ({CARD_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 2.5, 0, 0, 0, ?7, NULL, ?7)"
    ))?;
    for (i, c) in cleaned.iter().enumerate() {
        stmt.execute(params![new_id(), id, i as i64, c.front, c.back, c.hint, ts])?;
    }
    get_deck(conn, &id)
}

pub fn get_deck(conn: &Connection, id: &str) -> AppResult<Deck> {
    conn.query_row(&format!("{DECK_SELECT} WHERE d.id = ?2"), params![now(), id], deck_from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("Deck"))
}

pub fn list(conn: &Connection, course_id: Option<&str>) -> AppResult<Vec<Deck>> {
    let mut stmt = conn.prepare(&format!(
        "{DECK_SELECT} WHERE (?2 IS NULL OR d.course_id = ?2) ORDER BY d.updated_at DESC"
    ))?;
    let rows = stmt
        .query_map(params![now(), course_id], deck_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn detail(conn: &Connection, id: &str) -> AppResult<DeckDetail> {
    let deck = get_deck(conn, id)?;
    let mut stmt = conn.prepare(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE deck_id = ?1 ORDER BY position"))?;
    let cards = stmt.query_map(params![id], card_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(DeckDetail { deck, cards })
}

pub fn rename(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::invalid("Titles can't be empty."));
    }
    let title: String = title.chars().take(160).collect();
    conn.execute("UPDATE decks SET title = ?2, updated_at = ?3 WHERE id = ?1", params![id, title, now()])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM decks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn add_card(conn: &Connection, deck_id: &str, input: &CardInput) -> AppResult<Card> {
    let c = clean_card(input)?;
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM cards WHERE deck_id = ?1",
        params![deck_id],
        |r| r.get(0),
    )?;
    let id = new_id();
    let ts = now();
    conn.execute(
        &format!("INSERT INTO cards ({CARD_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 2.5, 0, 0, 0, ?7, NULL, ?7)"),
        params![id, deck_id, position, c.front, c.back, c.hint, ts],
    )?;
    conn.execute("UPDATE decks SET updated_at = ?2 WHERE id = ?1", params![deck_id, ts])?;
    get_card(conn, &id)
}

pub fn update_card(conn: &Connection, id: &str, input: &CardInput) -> AppResult<Card> {
    let c = clean_card(input)?;
    conn.execute(
        "UPDATE cards SET front = ?2, back = ?3, hint = ?4 WHERE id = ?1",
        params![id, c.front, c.back, c.hint],
    )?;
    get_card(conn, id)
}

pub fn delete_card(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM cards WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_card(conn: &Connection, id: &str) -> AppResult<Card> {
    conn.query_row(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = ?1"), params![id], card_from_row)
        .optional()?
        .ok_or_else(|| AppError::not_found("Card"))
}

/// Cards due now: learned cards past their due time first (most overdue
/// first), then up to `new_limit` unseen cards in deck order.
pub fn due(conn: &Connection, course_id: Option<&str>, deck_id: Option<&str>, limit: usize, new_limit: usize) -> AppResult<Vec<DueCard>> {
    let ts = now();
    let cols = CARD_COLUMNS
        .split(',')
        .map(|c| format!("c.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let base = format!(
        "SELECT {cols}, d.title FROM cards c JOIN decks d ON d.id = c.deck_id
         WHERE (?1 IS NULL OR d.course_id = ?1) AND (?2 IS NULL OR d.id = ?2)"
    );
    let map = |r: &Row| -> rusqlite::Result<(Card, String)> { Ok((card_from_row(r)?, r.get(13)?)) };

    let mut out = Vec::new();
    let mut stmt = conn.prepare(&format!(
        "{base} AND c.due_at <= ?3 AND (c.reps > 0 OR c.last_reviewed_at IS NOT NULL)
         ORDER BY c.due_at LIMIT ?4"
    ))?;
    for row in stmt.query_map(params![course_id, deck_id, ts, limit as i64], map)? {
        out.push(row?);
    }
    if out.len() < limit {
        let room = (limit - out.len()).min(new_limit);
        let mut stmt = conn.prepare(&format!(
            "{base} AND c.reps = 0 AND c.last_reviewed_at IS NULL ORDER BY d.created_at, c.position LIMIT ?3"
        ))?;
        for row in stmt.query_map(params![course_id, deck_id, room as i64], map)? {
            out.push(row?);
        }
    }
    Ok(out
        .into_iter()
        .map(|(card, deck_title)| {
            let previews = srs::previews(state_of(&card));
            DueCard { card, deck_title, previews }
        })
        .collect())
}

/// Records a rating, reschedules the card, and returns it.
pub fn review(conn: &Connection, card_id: &str, rating: i64, at: DateTime<Utc>) -> AppResult<Card> {
    if !(1..=4).contains(&rating) {
        return Err(AppError::invalid("Ratings run from 1 (again) to 4 (easy)."));
    }
    let card = get_card(conn, card_id)?;
    let next = srs::schedule(state_of(&card), rating);
    let due = srs::due_after(at, next.interval_days).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let reviewed = at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    conn.execute(
        "UPDATE cards SET ease = ?2, interval_days = ?3, reps = ?4, lapses = ?5, due_at = ?6, last_reviewed_at = ?7
         WHERE id = ?1",
        params![card_id, next.ease, next.interval_days, next.reps, next.lapses, due, reviewed],
    )?;
    conn.execute(
        "INSERT INTO reviews (id, card_id, deck_id, rating, interval_days, reviewed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![new_id(), card_id, card.deck_id, rating, next.interval_days, reviewed],
    )?;
    get_card(conn, card_id)
}
