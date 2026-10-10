//! Minimal Server-Sent Events decoder for the Messages API stream.

#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Buffers raw bytes (which may split UTF-8 sequences or events anywhere) and
/// yields complete events.
#[derive(Default)]
pub struct SseDecoder {
    buf: Vec<u8>,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some((end, delim_len)) = find_boundary(&self.buf) {
            let raw: Vec<u8> = self.buf.drain(..end + delim_len).take(end).collect();
            if let Some(ev) = parse_event(&String::from_utf8_lossy(&raw)) {
                events.push(ev);
            }
        }
        events
    }

    /// Flushes a trailing event that was not followed by a blank line.
    pub fn finish(&mut self) -> Option<SseEvent> {
        if self.buf.is_empty() {
            return None;
        }
        let raw = std::mem::take(&mut self.buf);
        parse_event(&String::from_utf8_lossy(&raw))
    }
}

fn find_boundary(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if i + 3 < buf.len() && &buf[i..i + 4] == b"\r\n\r\n" {
            return Some((i, 4));
        }
        i += 1;
    }
    None
}

fn parse_event(raw: &str) -> Option<SseEvent> {
    let mut event = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => event = Some(value.to_string()),
            "data" => data_lines.push(value),
            _ => {}
        }
    }
    if data_lines.is_empty() && event.is_none() {
        return None;
    }
    Some(SseEvent {
        event,
        data: data_lines.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_events_split_across_chunks() {
        let stream = "event: message_start\ndata: {\"type\":\"message_start\"}\n\n: ping comment\n\nevent: content_block_delta\ndata: {\"text\":\"héllo\"}\n\n";
        let bytes = stream.as_bytes();
        let mut dec = SseDecoder::default();
        let mut events = Vec::new();
        // Feed one byte at a time to split UTF-8 sequences and delimiters.
        for b in bytes {
            events.extend(dec.push(std::slice::from_ref(b)));
        }
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[1].data, "{\"text\":\"héllo\"}");
        assert!(dec.finish().is_none());
    }

    #[test]
    fn handles_crlf_and_multiline_data() {
        let mut dec = SseDecoder::default();
        let ev = dec.push(b"event: x\r\ndata: a\r\ndata: b\r\n\r\n");
        assert_eq!(ev, vec![SseEvent { event: Some("x".into()), data: "a\nb".into() }]);
    }

    #[test]
    fn finish_flushes_trailing_event() {
        let mut dec = SseDecoder::default();
        assert!(dec.push(b"data: {\"type\":\"message_stop\"}").is_empty());
        assert_eq!(dec.finish().unwrap().data, "{\"type\":\"message_stop\"}");
    }
}
