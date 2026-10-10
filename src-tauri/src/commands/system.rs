use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::export;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub contents: String,
    pub extension: String,
}

#[tauri::command]
pub async fn export_item(kind: String, format: String, payload: Value) -> AppResult<ExportResult> {
    let out = export::export(&kind, &format, &payload)?;
    Ok(ExportResult { contents: out.contents, extension: out.extension.to_string() })
}

const WRITABLE_EXTENSIONS: &[&str] = &["md", "txt", "csv", "tsv", "html", "json"];

/// Writes an export to a path the user picked in the save dialog. Only
/// document formats are allowed, so this can't be used to drop executables.
#[tauri::command]
pub async fn write_text_file(path: String, contents: String) -> AppResult<()> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !WRITABLE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::invalid("Exports can only be saved as .md, .txt, .csv, .tsv, .html, or .json files."));
    }
    std::fs::write(p, contents)?;
    Ok(())
}

/// Opens an https link in the system browser. Generated interactives reach
/// this through the validated sandbox bridge.
#[tauri::command]
pub async fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    let parsed = url::Url::parse(&url).map_err(|_| AppError::invalid("That link isn't a valid URL."))?;
    if parsed.scheme() != "https" {
        return Err(AppError::invalid("Only https links can be opened."));
    }
    if parsed.host_str().map(|h| h.is_empty()).unwrap_or(true) {
        return Err(AppError::invalid("That link has no host."));
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| AppError::Other(format!("Couldn't open the link: {e}")))
}
