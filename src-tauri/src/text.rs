//! Text helpers: material chunking, FTS5 query building, titles.

/// Target chunk size in characters. Roughly 200–250 words: large enough to
/// carry a self-contained idea, small enough that several fit in a prompt.
const CHUNK_TARGET: usize = 1100;
const CHUNK_MAX: usize = 1600;

/// Splits a document into retrieval chunks along paragraph boundaries, falling
/// back to sentence boundaries for very long paragraphs.
pub fn chunk_text(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut pieces: Vec<String> = Vec::new();
    for para in normalized.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if para.chars().count() <= CHUNK_MAX {
            pieces.push(para.to_string());
        } else {
            pieces.extend(split_long(para));
        }
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for piece in pieces {
        let cur_len = current.chars().count();
        let piece_len = piece.chars().count();
        if cur_len > 0 && cur_len + piece_len + 2 > CHUNK_TARGET {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(&piece);
    }
    if !current.trim().is_empty() {
        chunks.push(current);
    }
    chunks
}

fn split_long(para: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for sentence in split_sentences(para) {
        if current.chars().count() + sentence.chars().count() > CHUNK_TARGET && !current.is_empty() {
            out.push(std::mem::take(&mut current).trim().to_string());
        }
        // A single run-on "sentence" longer than the max is hard-wrapped.
        if sentence.chars().count() > CHUNK_MAX {
            let chars: Vec<char> = sentence.chars().collect();
            for window in chars.chunks(CHUNK_TARGET) {
                out.push(window.iter().collect::<String>().trim().to_string());
            }
            continue;
        }
        current.push_str(&sentence);
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        current.push(c);
        let next_is_space = chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true);
        if matches!(c, '.' | '!' | '?') && next_is_space {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

pub fn word_count(text: &str) -> i64 {
    text.split_whitespace().count() as i64
}

/// Builds a safe FTS5 MATCH expression from free text: each word becomes a
/// quoted term (operators and punctuation are neutralized) joined with OR, so
/// BM25 ranks passages that match more of the query higher.
pub fn fts_query(input: &str) -> Option<String> {
    const STOP: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "how", "in", "is", "it",
        "of", "on", "or", "that", "the", "this", "to", "was", "what", "when", "where", "which",
        "who", "why", "with", "does", "do", "can", "explain", "about",
    ];
    let mut terms: Vec<String> = Vec::new();
    for raw in input.split(|c: char| !c.is_alphanumeric()) {
        let word = raw.to_lowercase();
        if word.len() < 2 || STOP.contains(&word.as_str()) || terms.contains(&word) {
            continue;
        }
        terms.push(word);
        if terms.len() >= 12 {
            break;
        }
    }
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

/// The first Markdown heading, or else the file stem, as a material title.
pub fn derive_title(content: &str, file_name: &str) -> String {
    for line in content.lines().take(30) {
        let trimmed = line.trim();
        if let Some(h) = trimmed.strip_prefix("# ") {
            let h = h.trim();
            if !h.is_empty() {
                return truncate_chars(h, 120);
            }
        }
    }
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled material");
    truncate_chars(&stem.replace(['_', '-'], " "), 120)
}

/// A provisional conversation title from the first message.
pub fn provisional_title(text: &str) -> String {
    let first_line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("New chat");
    let words: Vec<&str> = first_line.split_whitespace().take(8).collect();
    let mut title = words.join(" ");
    if first_line.split_whitespace().count() > 8 {
        title.push('…');
    }
    if title.is_empty() {
        "New chat".into()
    } else {
        truncate_chars(&title, 80)
    }
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_respect_paragraphs_and_size() {
        let para = "Entropy measures the number of microstates. ".repeat(10);
        let doc = format!("{para}\n\n{para}\n\n{para}\n\n{para}");
        let chunks = chunk_text(&doc);
        assert!(chunks.len() >= 2, "expected several chunks, got {}", chunks.len());
        for c in &chunks {
            assert!(c.chars().count() <= CHUNK_MAX + 2);
            assert!(!c.trim().is_empty());
        }
    }

    #[test]
    fn long_paragraph_is_split_on_sentences() {
        let long = "This is a sentence about thermodynamics. ".repeat(80);
        let chunks = chunk_text(&long);
        assert!(chunks.len() >= 3);
        assert!(chunks.iter().all(|c| c.chars().count() <= CHUNK_MAX));
    }

    #[test]
    fn fts_query_neutralizes_operators() {
        let q = fts_query("What is the \"Carnot\" cycle? NEAR(x) OR -drop").unwrap();
        assert_eq!(q, "\"carnot\" OR \"cycle\" OR \"near\" OR \"drop\"");
        assert!(fts_query("the of and").is_none());
    }

    #[test]
    fn titles() {
        assert_eq!(derive_title("intro\n# Week 3: Entropy\nbody", "notes.md"), "Week 3: Entropy");
        assert_eq!(derive_title("no heading", "lecture_notes-week2.txt"), "lecture notes week2");
        assert_eq!(provisional_title("Explain   backpropagation please"), "Explain backpropagation please");
        assert_eq!(
            provisional_title("one two three four five six seven eight nine"),
            "one two three four five six seven eight…"
        );
    }
}
