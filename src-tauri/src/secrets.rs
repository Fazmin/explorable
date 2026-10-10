//! Provider API keys live in the operating system's credential store
//! (macOS Keychain, Windows Credential Manager, Secret Service on Linux),
//! never in SQLite. Environment variables are a fallback.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.explorable.desktop";
pub const PROVIDERS: &[&str] = &["anthropic", "openai", "gemini"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Keychain,
    Env,
}

impl KeySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeySource::Keychain => "keychain",
            KeySource::Env => "env",
        }
    }
}

fn account(provider: &str) -> AppResult<&'static str> {
    match provider {
        "anthropic" => Ok("anthropic-api-key"),
        "openai" => Ok("openai-api-key"),
        "gemini" => Ok("gemini-api-key"),
        other => Err(AppError::invalid(format!("Unknown provider: {other}"))),
    }
}

fn env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "anthropic" => &["ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "gemini" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        _ => &[],
    }
}

type Found = Option<(String, KeySource)>;

/// Caches each key after the first read so the keychain is not queried (and
/// the user not prompted) on every request.
#[derive(Default)]
pub struct Secrets {
    cache: Mutex<HashMap<String, Found>>,
}

fn entry(provider: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, account(provider)?)
        .map_err(|e| AppError::Keychain(format!("The system keychain is unavailable: {e}")))
}

impl Secrets {
    /// Keys supplied directly, bypassing the keychain (tests).
    #[cfg(test)]
    pub fn with_keys(keys: &[(&str, &str)]) -> Self {
        let mut cache = HashMap::new();
        for p in PROVIDERS {
            cache.insert(p.to_string(), None);
        }
        for (p, k) in keys {
            cache.insert(p.to_string(), Some((k.to_string(), KeySource::Env)));
        }
        Self { cache: Mutex::new(cache) }
    }

    pub async fn get(&self, provider: &str) -> Option<(String, KeySource)> {
        if account(provider).is_err() {
            return None;
        }
        if let Ok(guard) = self.cache.lock() {
            if let Some(cached) = guard.get(provider) {
                return cached.clone();
            }
        }
        let p = provider.to_string();
        let found = tokio::task::spawn_blocking(move || {
            let from_keychain = entry(&p).ok().and_then(|e| e.get_password().ok()).filter(|k| !k.trim().is_empty());
            match from_keychain {
                Some(k) => Some((k, KeySource::Keychain)),
                None => env_vars(&p).iter().find_map(|var| {
                    std::env::var(var)
                        .ok()
                        .map(|k| k.trim().to_string())
                        .filter(|k| !k.is_empty())
                        .map(|k| (k, KeySource::Env))
                }),
            }
        })
        .await
        .ok()
        .flatten();
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(provider.to_string(), found.clone());
        }
        found
    }

    pub async fn set(&self, provider: &str, key: &str) -> AppResult<()> {
        let key = key.trim().to_string();
        validate_key(provider, &key)?;
        let (p, stored) = (provider.to_string(), key.clone());
        tokio::task::spawn_blocking(move || {
            entry(&p)?
                .set_password(&stored)
                .map_err(|e| AppError::Keychain(format!("Couldn't save the key to the system keychain: {e}")))
        })
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(provider.to_string(), Some((key, KeySource::Keychain)));
        }
        Ok(())
    }

    pub async fn clear(&self, provider: &str) -> AppResult<()> {
        let p = provider.to_string();
        tokio::task::spawn_blocking(move || match entry(&p)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keychain(format!("Couldn't remove the key from the keychain: {e}"))),
        })
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;
        if let Ok(mut guard) = self.cache.lock() {
            // Re-read next time so an environment key can take over.
            guard.remove(provider);
        }
        Ok(())
    }
}

pub fn validate_key(provider: &str, key: &str) -> AppResult<()> {
    if key.is_empty() {
        return Err(AppError::invalid("Paste an API key first."));
    }
    if key.len() > 400 || key.chars().any(char::is_whitespace) {
        return Err(AppError::invalid("That doesn't look like an API key. Copy it again from your provider's console."));
    }
    match provider {
        "anthropic" if !key.starts_with("sk-ant-") => Err(AppError::invalid(
            "Anthropic API keys start with “sk-ant-”. Copy the key again from the Anthropic Console.",
        )),
        "openai" if !key.starts_with("sk-") => Err(AppError::invalid(
            "OpenAI API keys start with “sk-”. Copy the key again from the OpenAI dashboard.",
        )),
        "gemini" if key.len() < 20 || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') => Err(
            AppError::invalid("That doesn't look like a Gemini API key. Copy it again from Google AI Studio."),
        ),
        "anthropic" | "openai" | "gemini" => Ok(()),
        other => Err(AppError::invalid(format!("Unknown provider: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_format_is_checked_per_provider() {
        assert!(validate_key("anthropic", "").is_err());
        assert!(validate_key("anthropic", "sk-ant- spaced").is_err());
        assert!(validate_key("anthropic", "sk-other").is_err());
        assert!(validate_key("anthropic", "sk-ant-api03-abcdef").is_ok());
        assert!(validate_key("openai", "sk-proj-abc123").is_ok());
        assert!(validate_key("openai", "AIzaSyExample").is_err());
        assert!(validate_key("gemini", "AIzaSyA1b2c3d4e5f6g7h8i9j0").is_ok());
        assert!(validate_key("gemini", "short").is_err());
        assert!(validate_key("nope", "sk-ant-x").is_err());
    }

    #[tokio::test]
    async fn fixed_keys_bypass_the_keychain() {
        let s = Secrets::with_keys(&[("openai", "sk-test")]);
        assert_eq!(s.get("openai").await.unwrap().0, "sk-test");
        assert!(s.get("gemini").await.is_none());
    }
}
