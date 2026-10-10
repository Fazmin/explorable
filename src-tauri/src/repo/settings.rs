use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::Settings;

const KEY: &str = "settings";

pub fn load(conn: &Connection) -> AppResult<Settings> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![KEY], |r| r.get(0))
        .optional()?;
    Ok(match raw {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => Settings::default(),
    })
}

pub fn save(conn: &Connection, settings: &Settings) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, serde_json::to_string(settings)?],
    )?;
    Ok(())
}

/// Merges a partial JSON object into the stored settings and validates the result.
pub fn patch(conn: &Connection, patch: &Value) -> AppResult<Settings> {
    let current = load(conn)?;
    let mut merged = serde_json::to_value(&current)?;
    let (Some(target), Some(changes)) = (merged.as_object_mut(), patch.as_object()) else {
        return Err(AppError::invalid("Settings update must be an object."));
    };
    for (k, v) in changes {
        if target.contains_key(k) {
            target.insert(k.clone(), v.clone());
        }
    }
    let next: Settings = serde_json::from_value(merged)
        .map_err(|e| AppError::invalid(format!("Invalid setting value: {e}")))?;
    validate(&next)?;
    save(conn, &next)?;
    Ok(next)
}

fn validate(s: &Settings) -> AppResult<()> {
    if !["light", "dark", "system"].contains(&s.theme.as_str()) {
        return Err(AppError::invalid("Theme must be light, dark, or system."));
    }
    if !["anthropic", "openai", "gemini", "demo"].contains(&s.provider.as_str()) {
        return Err(AppError::invalid("Provider must be anthropic, openai, gemini, or demo."));
    }
    if !["low", "medium", "high", "xhigh", "max"].contains(&s.effort.as_str()) {
        return Err(AppError::invalid("Effort must be low, medium, or high."));
    }
    let claude_ok = |m: &str| m.starts_with("claude-") && model_id_ok(m);
    if !claude_ok(&s.model) || !claude_ok(&s.router_model) {
        return Err(AppError::invalid("Claude model IDs look like claude-…"));
    }
    for m in [&s.openai_model, &s.openai_router_model, &s.gemini_model, &s.gemini_router_model] {
        if !model_id_ok(m) {
            return Err(AppError::invalid(format!("“{m}” isn't a valid model ID.")));
        }
    }
    if !(s.text_scale.is_finite() && (1.0..=1.5).contains(&s.text_scale)) {
        return Err(AppError::invalid("Text size must be between 100% and 150%."));
    }
    if !["hyperlegible", "serif", "system"].contains(&s.reading_font.as_str()) {
        return Err(AppError::invalid("Reading font must be hyperlegible, serif, or system."));
    }
    for url in [&s.openai_base_url, &s.gemini_base_url] {
        if !base_url_ok(url) {
            return Err(AppError::invalid("API base URLs must start with https:// (or http://localhost for a local gateway)."));
        }
    }
    Ok(())
}

/// Model IDs travel in URLs (Gemini) and JSON; keep them to a safe alphabet.
fn model_id_ok(m: &str) -> bool {
    !m.is_empty() && m.len() <= 100 && m.chars().all(|c| c.is_ascii_alphanumeric() || "-._:".contains(c))
}

fn base_url_ok(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() {
        return true;
    }
    match url::Url::parse(url) {
        Ok(u) if u.scheme() == "https" => u.host_str().is_some(),
        Ok(u) if u.scheme() == "http" => matches!(u.host_str(), Some("localhost") | Some("127.0.0.1")),
        _ => false,
    }
}
