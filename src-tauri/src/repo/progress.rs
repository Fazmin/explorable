use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use rusqlite::{params, Connection, Row};
use serde_json::Value;

use crate::db::{new_id, now};
use crate::error::{AppError, AppResult};
use crate::models::{DailyActivity, Progress, QuizAttempt, QuizAttemptInput, TopicMastery};

fn attempt_from_row(r: &Row) -> rusqlite::Result<QuizAttempt> {
    Ok(QuizAttempt {
        id: r.get(0)?,
        course_id: r.get(1)?,
        quiz_key: r.get(2)?,
        title: r.get(3)?,
        topic: r.get(4)?,
        score: r.get(5)?,
        max_score: r.get(6)?,
        created_at: r.get(7)?,
    })
}

pub fn record_attempt(conn: &Connection, input: &QuizAttemptInput) -> AppResult<QuizAttempt> {
    if !input.score.is_finite() || !input.max_score.is_finite() || input.max_score <= 0.0 {
        return Err(AppError::invalid("A quiz attempt needs a positive maximum score."));
    }
    if input.score < 0.0 || input.score > input.max_score {
        return Err(AppError::invalid("The score must be between 0 and the maximum."));
    }
    let id = new_id();
    let ts = now();
    let details = if input.details.is_array() { input.details.clone() } else { Value::Array(vec![]) };
    conn.execute(
        "INSERT INTO quiz_attempts (id, course_id, conversation_id, quiz_key, title, topic, score, max_score, details, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            input.course_id,
            input.conversation_id,
            input.quiz_key.chars().take(120).collect::<String>(),
            input.title.trim().chars().take(160).collect::<String>(),
            input.topic.trim().chars().take(120).collect::<String>(),
            input.score,
            input.max_score,
            serde_json::to_string(&details)?,
            ts
        ],
    )?;
    Ok(QuizAttempt {
        id,
        course_id: input.course_id.clone(),
        quiz_key: input.quiz_key.clone(),
        title: input.title.clone(),
        topic: input.topic.clone(),
        score: input.score,
        max_score: input.max_score,
        created_at: ts,
    })
}

/// Attempts for one quiz (identified by its tool-call id), newest first.
pub fn attempts_for(conn: &Connection, quiz_key: &str) -> AppResult<Vec<QuizAttempt>> {
    let mut stmt = conn.prepare(
        "SELECT id, course_id, quiz_key, title, topic, score, max_score, created_at
         FROM quiz_attempts WHERE quiz_key = ?1 ORDER BY created_at DESC LIMIT 20",
    )?;
    let rows = stmt.query_map(params![quiz_key], attempt_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn local_date(ts: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|d| d.with_timezone(&Local).date_naive())
}

pub fn compute(conn: &Connection, course_id: Option<&str>) -> AppResult<Progress> {
    let ts = now();

    let (quizzes_taken, average_score): (i64, Option<f64>) = conn.query_row(
        "SELECT COUNT(*), AVG(score / max_score) FROM quiz_attempts
         WHERE (?1 IS NULL OR course_id = ?1) AND max_score > 0",
        params![course_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let (cards_total, cards_due, cards_learned): (i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN (c.reps > 0 OR c.last_reviewed_at IS NOT NULL) AND c.due_at <= ?2 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN c.reps > 0 THEN 1 ELSE 0 END), 0)
         FROM cards c JOIN decks d ON d.id = c.deck_id
         WHERE (?1 IS NULL OR d.course_id = ?1)",
        params![course_id, ts],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    // Activity timestamps from the last ~400 days, bucketed by local date.
    let since = (Utc::now() - Duration::days(400)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut review_days: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT r.reviewed_at FROM reviews r JOIN decks d ON d.id = r.deck_id
             WHERE (?1 IS NULL OR d.course_id = ?1) AND r.reviewed_at >= ?2",
        )?;
        for row in stmt.query_map(params![course_id, since], |r| r.get::<_, String>(0))? {
            if let Some(day) = local_date(&row?) {
                *review_days.entry(day).or_default() += 1;
            }
        }
    }
    let mut question_days: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT created_at, json_array_length(details), max_score FROM quiz_attempts
             WHERE (?1 IS NULL OR course_id = ?1) AND created_at >= ?2",
        )?;
        let rows = stmt.query_map(params![course_id, since], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, f64>(2)?))
        })?;
        for row in rows {
            let (created, n, max) = row?;
            let questions = n.filter(|&n| n > 0).unwrap_or(max.round() as i64);
            if let Some(day) = local_date(&created) {
                *question_days.entry(day).or_default() += questions;
            }
        }
    }

    let today = Local::now().date_naive();
    let reviews_last7 = review_days
        .iter()
        .filter(|(d, _)| (today - **d).num_days() < 7)
        .map(|(_, n)| n)
        .sum();

    let active: HashSet<NaiveDate> = review_days.keys().chain(question_days.keys()).copied().collect();
    // A streak survives until the end of today even if today has no activity yet.
    let mut day = if active.contains(&today) { today } else { today - Duration::days(1) };
    let mut streak_days = 0;
    while active.contains(&day) {
        streak_days += 1;
        day -= Duration::days(1);
    }

    let activity = (0..30)
        .rev()
        .map(|offset| {
            let d = today - Duration::days(offset);
            DailyActivity {
                date: d.format("%Y-%m-%d").to_string(),
                reviews: review_days.get(&d).copied().unwrap_or(0),
                questions: question_days.get(&d).copied().unwrap_or(0),
            }
        })
        .collect();

    let mut stmt = conn.prepare(
        "SELECT CASE WHEN topic = '' THEN title ELSE topic END AS t,
                COUNT(*), SUM(score) / SUM(max_score), MAX(created_at)
         FROM quiz_attempts WHERE (?1 IS NULL OR course_id = ?1) AND max_score > 0
         GROUP BY t ORDER BY MAX(created_at) DESC LIMIT 24",
    )?;
    let topics = stmt
        .query_map(params![course_id], |r| {
            Ok(TopicMastery {
                topic: r.get(0)?,
                attempts: r.get(1)?,
                accuracy: r.get(2)?,
                last_attempt_at: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT id, course_id, quiz_key, title, topic, score, max_score, created_at FROM quiz_attempts
         WHERE (?1 IS NULL OR course_id = ?1) ORDER BY created_at DESC LIMIT 10",
    )?;
    let recent_attempts = stmt.query_map(params![course_id], attempt_from_row)?.collect::<Result<Vec<_>, _>>()?;

    Ok(Progress {
        quizzes_taken,
        average_score,
        cards_total,
        cards_due,
        cards_learned,
        reviews_last7,
        streak_days,
        topics,
        activity,
        recent_attempts,
    })
}

/// A compact plain-text summary for the tutor's `get_learner_progress` tool.
pub fn summary_for_model(conn: &Connection, course_id: Option<&str>, topic: Option<&str>) -> AppResult<Value> {
    let p = compute(conn, course_id)?;
    let filter = topic.map(|t| t.to_lowercase());
    let topics: Vec<Value> = p
        .topics
        .iter()
        .filter(|t| filter.as_ref().map(|f| t.topic.to_lowercase().contains(f)).unwrap_or(true))
        .map(|t| {
            serde_json::json!({
                "topic": t.topic,
                "attempts": t.attempts,
                "accuracy_percent": (t.accuracy * 100.0).round(),
                "last_attempt": t.last_attempt_at,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "source": "This learner's saved quiz attempts and flashcard reviews in this app",
        "quizzes_taken": p.quizzes_taken,
        "average_score_percent": p.average_score.map(|a| (a * 100.0).round()),
        "flashcards": { "total": p.cards_total, "due_now": p.cards_due, "learned": p.cards_learned },
        "reviews_last_7_days": p.reviews_last7,
        "streak_days": p.streak_days,
        "topics": topics,
        "note": if p.quizzes_taken == 0 && p.cards_total == 0 { "No practice recorded yet." } else { "" },
    }))
}
