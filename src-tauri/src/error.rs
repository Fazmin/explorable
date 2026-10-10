use serde::{Serialize, Serializer};

/// Every fallible command returns this. It serializes to a plain message so the
/// frontend can show it directly.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Malformed data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Provider(String),
    #[error("{0}")]
    Keychain(String),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        AppError::Invalid(msg.into())
    }
    pub fn not_found(what: &str) -> Self {
        AppError::NotFound(format!("{what} was not found."))
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
