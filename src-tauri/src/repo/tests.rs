use chrono::Utc;
use serde_json::json;

use super::*;
use crate::db::Db;
use crate::models::{CardInput, CourseInput, Mode, QuizAttemptInput, Style};

fn course_input(name: &str) -> CourseInput {
    CourseInput {
        name: name.into(),
        code: "PHYS 201".into(),
        level: "undergraduate".into(),
        discipline: "Physics".into(),
        description: "Classical mechanics".into(),
        color: "blue".into(),
    }
}

#[test]
fn migrations_are_idempotent() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        crate::db::migrate_for_tests(c)?;
        let v: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
        assert_eq!(v, 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn settings_roundtrip_and_validation() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let s = settings::load(c)?;
        assert_eq!(s.provider, "demo");
        let s = settings::patch(c, &json!({"theme": "dark", "effort": "high", "unknown": 1}))?;
        assert_eq!(s.theme, "dark");
        assert_eq!(s.effort, "high");
        assert!(settings::patch(c, &json!({"theme": "neon"})).is_err());
        assert!(settings::patch(c, &json!({"model": "gpt-4"})).is_err());
        let s = settings::patch(c, &json!({"textScale": 1.25, "highContrast": true, "readingFont": "serif"}))?;
        assert_eq!((s.text_scale, s.high_contrast, s.reading_font.as_str()), (1.25, true, "serif"));
        assert!(settings::patch(c, &json!({"textScale": 3.0})).is_err());
        assert!(settings::patch(c, &json!({"readingFont": "comic"})).is_err());
        assert_eq!(settings::load(c)?.theme, "dark");
        Ok(())
    })
    .unwrap();
}

#[test]
fn course_delete_cascades_everything() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let course = courses::create(c, &course_input("Mechanics"))?;
        let conv = conversations::create(c, Some(&course.id), Mode::Learn, Style::Direct)?;
        messages::append(c, "m1", &conv.id, "user", &json!([{"type": "text", "text": "hi"}]), &json!({}))?;
        materials::insert(c, &course.id, "Notes", "notes.md", "Newton's second law relates force and acceleration.")?;
        decks::create(c, Some(&course.id), Some(&conv.id), "Laws", "", &[CardInput {
            front: "F = ?".into(),
            back: "ma".into(),
            hint: String::new(),
        }])?;

        courses::delete(c, &course.id)?;
        for table in ["conversations", "messages", "materials", "decks", "cards", "material_chunks"] {
            let n: i64 = c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            assert_eq!(n, 0, "{table} should be empty after cascade");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn material_search_ranks_relevant_chunks() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let course = courses::create(c, &course_input("Thermo"))?;
        let other = courses::create(c, &course_input("Biology"))?;
        materials::insert(c, &course.id, "Week 1", "w1.md", "The first law of thermodynamics is conservation of energy.\n\nHeat engines convert heat into work.")?;
        materials::insert(c, &course.id, "Week 2", "w2.md", "Entropy always increases in an isolated system. The Carnot cycle sets the efficiency limit.")?;
        materials::insert(c, &other.id, "Cells", "cells.md", "Entropy appears in biology too, in protein folding.")?;

        let hits = materials::search(c, &course.id, "What limits Carnot engine efficiency?", 5)?;
        assert!(!hits.is_empty());
        assert_eq!(hits[0].title, "Week 2");
        assert!(hits.iter().all(|h| h.title != "Cells"), "search must stay within the course");
        assert!(hits[0].snippet.contains('«'));
        assert!(materials::search(c, &course.id, "the of", 5)?.is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn deck_review_schedules_and_counts() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let cards: Vec<CardInput> = (0..3)
            .map(|i| CardInput { front: format!("Q{i}"), back: format!("A{i}"), hint: String::new() })
            .collect();
        let deck = decks::create(c, None, None, "Deck", "", &cards)?;
        assert_eq!(deck.card_count, 3);
        assert_eq!(deck.new_count, 3);
        assert_eq!(deck.due_count, 0);

        let due = decks::due(c, None, Some(&deck.id), 20, 20)?;
        assert_eq!(due.len(), 3);
        assert_eq!(due[0].previews[2], "1d");

        let reviewed = decks::review(c, &due[0].card.id, 3, Utc::now())?;
        assert_eq!(reviewed.reps, 1);
        assert!(reviewed.due_at > crate::db::now());
        assert!(decks::review(c, &due[0].card.id, 7, Utc::now()).is_err());

        let failed = decks::review(c, &due[1].card.id, 1, Utc::now())?;
        assert_eq!(failed.reps, 0);
        let deck = decks::get_deck(c, &deck.id)?;
        assert_eq!(deck.new_count, 1);

        let remaining = decks::due(c, None, Some(&deck.id), 20, 20)?;
        assert_eq!(remaining.len(), 1, "only the untouched new card is due right now");
        Ok(())
    })
    .unwrap();
}

#[test]
fn progress_aggregates_attempts_and_reviews() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let course = courses::create(c, &course_input("Stats"))?;
        for (score, topic) in [(3.0, "Bayes"), (4.0, "Bayes"), (1.0, "CLT")] {
            progress::record_attempt(c, &QuizAttemptInput {
                course_id: Some(course.id.clone()),
                conversation_id: None,
                quiz_key: "toolu_1".into(),
                title: "Quiz".into(),
                topic: topic.into(),
                score,
                max_score: 4.0,
                details: json!([{"correct": true}, {"correct": false}, {"correct": true}, {"correct": true}]),
            })?;
        }
        assert!(progress::record_attempt(c, &QuizAttemptInput {
            course_id: None,
            conversation_id: None,
            quiz_key: "k".into(),
            title: "Bad".into(),
            topic: String::new(),
            score: 5.0,
            max_score: 4.0,
            details: json!([]),
        })
        .is_err());

        let p = progress::compute(c, Some(&course.id))?;
        assert_eq!(p.quizzes_taken, 3);
        assert!((p.average_score.unwrap() - (8.0 / 12.0)).abs() < 1e-9);
        let bayes = p.topics.iter().find(|t| t.topic == "Bayes").unwrap();
        assert_eq!(bayes.attempts, 2);
        assert!((bayes.accuracy - 7.0 / 8.0).abs() < 1e-9);
        assert_eq!(p.streak_days, 1);
        assert_eq!(p.activity.len(), 30);
        assert_eq!(p.activity.last().unwrap().questions, 12);

        let summary = progress::summary_for_model(c, Some(&course.id), Some("bayes"))?;
        assert_eq!(summary["topics"].as_array().unwrap().len(), 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn messages_append_in_sequence() {
    let db = Db::open_in_memory().unwrap();
    db.with(|c| {
        let conv = conversations::create(c, None, Mode::Teach, Style::Direct)?;
        let a = messages::append(c, "a", &conv.id, "user", &json!([{"type": "text", "text": "1"}]), &json!({}))?;
        let b = messages::append(c, "b", &conv.id, "assistant", &json!([{"type": "text", "text": "2"}]), &json!({}))?;
        assert_eq!((a.seq, b.seq), (1, 2));
        messages::set_meta(c, "a", &json!({"route": {"presentation": "text"}}))?;
        let all = messages::list(c, &conv.id)?;
        assert_eq!(all[0].meta["route"]["presentation"], "text");
        assert_eq!(messages::last(c, &conv.id)?.unwrap().id, "b");
        assert_eq!(conversations::list(c, None, Mode::Teach)?.len(), 1);
        assert_eq!(conversations::list(c, None, Mode::Learn)?.len(), 0);
        Ok(())
    })
    .unwrap();
}
