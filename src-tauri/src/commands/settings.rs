use serde_json::Value;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::llm::Provider;
use crate::models::{ApiKeySources, SettingsView};
use crate::repo;
use crate::secrets::PROVIDERS;
use crate::state::AppState;

async fn view(state: &AppState) -> AppResult<SettingsView> {
    let settings = state.db.with(repo::settings::load)?;
    let source = |found: Option<(String, crate::secrets::KeySource)>| found.map(|(_, src)| src.as_str().to_string());
    let api_keys = ApiKeySources {
        anthropic: source(state.secrets.get("anthropic").await),
        openai: source(state.secrets.get("openai").await),
        gemini: source(state.secrets.get("gemini").await),
    };
    Ok(SettingsView {
        settings,
        api_keys,
        data_dir: state.data_dir.display().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn check_provider(provider: &str) -> AppResult<()> {
    if PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(AppError::invalid(format!("Unknown provider: {provider}")))
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsView> {
    view(&state).await
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, patch: Value) -> AppResult<SettingsView> {
    state.db.with(|c| repo::settings::patch(c, &patch))?;
    view(&state).await
}

/// Saves a provider's key to the system keychain and makes it the active provider.
#[tauri::command]
pub async fn set_api_key(state: State<'_, AppState>, provider: String, key: String) -> AppResult<SettingsView> {
    check_provider(&provider)?;
    state.secrets.set(&provider, &key).await?;
    state
        .db
        .with(|c| repo::settings::patch(c, &serde_json::json!({ "provider": provider })))?;
    view(&state).await
}

#[tauri::command]
pub async fn clear_api_key(state: State<'_, AppState>, provider: String) -> AppResult<SettingsView> {
    check_provider(&provider)?;
    state.secrets.clear(&provider).await?;
    view(&state).await
}

/// Checks a provider's stored key against its API without spending tokens.
#[tauri::command]
pub async fn test_api_key(state: State<'_, AppState>, provider: String) -> AppResult<()> {
    check_provider(&provider)?;
    let settings = state.db.with(repo::settings::load)?;
    Provider::build(&provider, &settings, &state.secrets, &state.http).await?.check_key().await
}

/// Providers and models offered in Settings.
#[tauri::command]
pub async fn model_catalog() -> AppResult<Vec<crate::llm::catalog::ProviderInfo>> {
    Ok(crate::llm::catalog::catalog())
}
