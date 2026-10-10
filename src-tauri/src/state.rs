use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::agent::RunRegistry;
use crate::db::Db;
use crate::secrets::Secrets;

/// Shared application state, managed by Tauri.
pub struct AppState {
    pub db: Arc<Db>,
    pub http: reqwest::Client,
    pub secrets: Arc<Secrets>,
    pub runs: Arc<RunRegistry>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(db: Db, data_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("Explorable/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Self {
            db: Arc::new(db),
            http,
            secrets: Arc::new(Secrets::default()),
            runs: Arc::new(RunRegistry::default()),
            data_dir,
        }
    }
}
