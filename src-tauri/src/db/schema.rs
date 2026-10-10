use rusqlite::Connection;

use crate::error::AppResult;

/// Ordered migrations. Index + 1 is the schema version stored in
/// `PRAGMA user_version`. Never edit a shipped migration; append a new one.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE courses (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  code        TEXT NOT NULL DEFAULT '',
  level       TEXT NOT NULL DEFAULT 'undergraduate',
  discipline  TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  color       TEXT NOT NULL DEFAULT 'blue',
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE TABLE conversations (
  id         TEXT PRIMARY KEY,
  course_id  TEXT REFERENCES courses(id) ON DELETE CASCADE,
  mode       TEXT NOT NULL CHECK (mode IN ('learn', 'teach')),
  style      TEXT NOT NULL DEFAULT 'direct' CHECK (style IN ('direct', 'socratic')),
  title      TEXT NOT NULL,
  pending    TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_conversations_course ON conversations(course_id, updated_at DESC);

CREATE TABLE messages (
  id              TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  seq             INTEGER NOT NULL,
  role            TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
  content         TEXT NOT NULL,
  meta            TEXT NOT NULL DEFAULT '{}',
  created_at      TEXT NOT NULL,
  UNIQUE (conversation_id, seq)
);

CREATE TABLE artifacts (
  id              TEXT PRIMARY KEY,
  course_id       TEXT REFERENCES courses(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  kind            TEXT NOT NULL,
  title           TEXT NOT NULL,
  payload         TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX idx_artifacts_course ON artifacts(course_id, updated_at DESC);

CREATE TABLE materials (
  id          TEXT PRIMARY KEY,
  course_id   TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  title       TEXT NOT NULL,
  source_name TEXT NOT NULL DEFAULT '',
  content     TEXT NOT NULL,
  word_count  INTEGER NOT NULL DEFAULT 0,
  chunk_count INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_materials_course ON materials(course_id);

CREATE VIRTUAL TABLE material_chunks USING fts5(
  content,
  title,
  material_id UNINDEXED,
  course_id UNINDEXED,
  ordinal UNINDEXED,
  tokenize = 'porter unicode61'
);

-- FTS5 tables cannot carry foreign keys; keep chunks in step with materials,
-- including deletes cascaded from a course.
CREATE TRIGGER materials_after_delete AFTER DELETE ON materials BEGIN
  DELETE FROM material_chunks WHERE material_id = old.id;
END;

CREATE TABLE decks (
  id                     TEXT PRIMARY KEY,
  course_id              TEXT REFERENCES courses(id) ON DELETE CASCADE,
  title                  TEXT NOT NULL,
  description            TEXT NOT NULL DEFAULT '',
  source_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  created_at             TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);

CREATE TABLE cards (
  id               TEXT PRIMARY KEY,
  deck_id          TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  position         INTEGER NOT NULL DEFAULT 0,
  front            TEXT NOT NULL,
  back             TEXT NOT NULL,
  hint             TEXT NOT NULL DEFAULT '',
  ease             REAL NOT NULL DEFAULT 2.5,
  interval_days    REAL NOT NULL DEFAULT 0,
  reps             INTEGER NOT NULL DEFAULT 0,
  lapses           INTEGER NOT NULL DEFAULT 0,
  due_at           TEXT NOT NULL,
  last_reviewed_at TEXT,
  created_at       TEXT NOT NULL
);
CREATE INDEX idx_cards_deck ON cards(deck_id, position);
CREATE INDEX idx_cards_due ON cards(due_at);

CREATE TABLE reviews (
  id            TEXT PRIMARY KEY,
  card_id       TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  deck_id       TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  rating        INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 4),
  interval_days REAL NOT NULL,
  reviewed_at   TEXT NOT NULL
);
CREATE INDEX idx_reviews_time ON reviews(reviewed_at);

CREATE TABLE quiz_attempts (
  id              TEXT PRIMARY KEY,
  course_id       TEXT REFERENCES courses(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  quiz_key        TEXT NOT NULL,
  title           TEXT NOT NULL,
  topic           TEXT NOT NULL DEFAULT '',
  score           REAL NOT NULL,
  max_score       REAL NOT NULL,
  details         TEXT NOT NULL DEFAULT '[]',
  created_at      TEXT NOT NULL
);
CREATE INDEX idx_quiz_attempts_course ON quiz_attempts(course_id, created_at);
"#];

pub fn migrate(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = i as i64 + 1;
        if version <= current {
            continue;
        }
        conn.execute_batch(&format!("BEGIN;\n{sql}\nPRAGMA user_version = {version};\nCOMMIT;"))?;
    }
    Ok(())
}
