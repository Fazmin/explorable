use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::{Material, MaterialHit};
use crate::repo;
use crate::state::AppState;
use crate::text;

const TEXT_EXTENSIONS: &[&str] = &["txt", "md", "markdown", "csv", "tex", "rst", "html", "htm"];
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialContent {
    pub title: String,
    pub content: String,
}

#[tauri::command]
pub async fn list_materials(state: State<'_, AppState>, course_id: String) -> AppResult<Vec<Material>> {
    state.db.with(|c| repo::materials::list(c, &course_id))
}

#[tauri::command]
pub async fn get_material(state: State<'_, AppState>, id: String) -> AppResult<MaterialContent> {
    let (title, content) = state.db.with(|c| repo::materials::content(c, &id))?;
    Ok(MaterialContent { title, content })
}

#[tauri::command]
pub async fn add_material_text(state: State<'_, AppState>, course_id: String, title: String, content: String) -> AppResult<Material> {
    let title = if title.trim().is_empty() { text::derive_title(&content, "Pasted text") } else { title };
    state.db.with(|c| repo::materials::insert(c, &course_id, &title, "Pasted text", &content))
}

/// Imports a text, Markdown, HTML, or PDF file chosen in the open dialog.
#[tauri::command]
pub async fn import_material(state: State<'_, AppState>, course_id: String, path: String) -> AppResult<Material> {
    let path = PathBuf::from(path);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("material")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if ext != "pdf" && !TEXT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::invalid(
            "That file type isn't supported. Import PDF, plain text, Markdown, LaTeX, or HTML files.",
        ));
    }
    let size = std::fs::metadata(&path)?.len();
    if size > MAX_FILE_BYTES {
        return Err(AppError::invalid("That file is over 50 MB. Split it into smaller files first."));
    }
    let read_path = path.clone();
    let ext_for_read = ext.clone();
    let body = tokio::task::spawn_blocking(move || -> AppResult<String> {
        if ext_for_read == "pdf" {
            let bytes = std::fs::read(&read_path)?;
            // The PDF parser can panic on malformed files; report that as a normal error.
            match std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes)) {
                Ok(Ok(text)) => Ok(text),
                Ok(Err(e)) => Err(AppError::invalid(format!(
                    "Couldn't read text from that PDF ({e}). Scanned PDFs need OCR before import."
                ))),
                Err(_) => Err(AppError::invalid(
                    "That PDF uses a structure the reader can't handle. Try exporting it again or saving it as text.",
                )),
            }
        } else {
            let bytes = std::fs::read(&read_path)?;
            let raw = String::from_utf8_lossy(&bytes).to_string();
            Ok(if ext_for_read == "html" || ext_for_read == "htm" { strip_html(&raw) } else { raw })
        }
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;

    let body = normalize_whitespace(&body);
    if body.trim().is_empty() {
        return Err(AppError::invalid(
            "No readable text was found in that file. If it's a scanned PDF, run OCR on it first.",
        ));
    }
    let title = text::derive_title(&body, &file_name);
    state.db.with(|c| repo::materials::insert(c, &course_id, &title, &file_name, &body))
}

#[tauri::command]
pub async fn delete_material(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.with(|c| repo::materials::delete(c, &id))
}

#[tauri::command]
pub async fn search_materials(state: State<'_, AppState>, course_id: String, query: String) -> AppResult<Vec<MaterialHit>> {
    state.db.with(|c| repo::materials::search(c, &course_id, &query, 8))
}

/// Collapses runs of blank lines and trailing spaces that PDF extraction leaves behind.
fn normalize_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// A small tag stripper for saved web pages: drops scripts and styles, keeps text.
fn strip_html(html: &str) -> String {
    // ASCII lowercasing keeps byte offsets identical between the two strings.
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let skipped = ["<script", "<style"].iter().find(|s| lower[i..].starts_with(**s));
            if let Some(skip) = skipped {
                let close = if *skip == "<script" { "</script>" } else { "</style>" };
                i = lower[i..].find(close).map(|p| i + p + close.len()).unwrap_or(bytes.len());
                continue;
            }
            let tag_end = html[i..].find('>').map(|p| i + p + 1).unwrap_or(bytes.len());
            let tag = &lower[i..tag_end];
            if ["<p", "<br", "<div", "<h1", "<h2", "<h3", "<h4", "<li", "<tr", "</p", "</div", "</h"].iter().any(|t| tag.starts_with(t)) {
                out.push('\n');
            }
            i = tag_end;
        } else {
            let next = html[i..].find('<').map(|p| i + p).unwrap_or(bytes.len());
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html_keeping_text() {
        let html = "<html><head><style>p{}</style><script>alert(1)</script></head><body><h1>Title</h1><p>One &amp; two</p></body></html>";
        let text = strip_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("One & two"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("p{}"));
    }

    #[test]
    fn whitespace_is_normalized() {
        assert_eq!(normalize_whitespace("a  \n\n\n\nb\n"), "a\n\nb\n");
    }
}
