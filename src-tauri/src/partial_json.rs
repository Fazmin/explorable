//! Tolerant parser for a JSON object that is still streaming in.
//!
//! Tool arguments arrive as `input_json_delta` fragments. To render a widget
//! progressively we need, at any moment, the fields parsed so far and which
//! top-level fields are finished. Rules:
//!
//! * A top-level string value that is still open is returned with the text
//!   decoded so far (so partial HTML and CSS can be previewed).
//! * Inside arrays, unfinished strings and scalars are omitted, so a consumer
//!   never executes half of a `jsExpressions` entry. Unfinished objects and
//!   arrays inside arrays are included, so a quiz can show questions as they
//!   form.
//! * Unfinished numbers and literals are omitted everywhere.
//! * A key is reported complete only once its value has fully closed.

use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Partial {
    pub value: Map<String, Value>,
    pub complete_keys: Vec<String>,
    /// The whole object closed.
    pub complete: bool,
}

pub fn parse_partial_object(src: &str) -> Option<Partial> {
    let mut p = Parser { s: src.as_bytes(), src, i: 0 };
    p.ws();
    if p.peek()? != b'{' {
        return None;
    }
    p.i += 1;
    let mut map = Map::new();
    let mut complete_keys = Vec::new();
    let complete = p.object_body(&mut map, Some(&mut complete_keys), Ctx::TopLevel);
    Some(Partial { value: map, complete_keys, complete })
}

#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    /// Values directly in the top-level object: partial strings allowed.
    TopLevel,
    /// Inside a nested object.
    Nested,
    /// Elements of an array.
    Array,
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
}

enum Parsed {
    Complete(Value),
    /// Input ended inside this value; carries a usable partial value if any.
    Partial(Option<Value>),
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\n' || c == b'\r' || c == b'\t' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    /// Parses members after `{`. Returns true if the closing brace was seen.
    fn object_body(&mut self, map: &mut Map<String, Value>, mut keys: Option<&mut Vec<String>>, ctx: Ctx) -> bool {
        let child_ctx = if ctx == Ctx::TopLevel { Ctx::TopLevel } else { Ctx::Nested };
        loop {
            self.ws();
            match self.peek() {
                None => return false,
                Some(b'}') => {
                    self.i += 1;
                    return true;
                }
                Some(b',') => {
                    self.i += 1;
                    continue;
                }
                Some(b'"') => {}
                // Malformed input: stop with what we have.
                Some(_) => return false,
            }
            self.i += 1;
            let key = match self.string_body() {
                (s, true) => s,
                (_, false) => return false,
            };
            self.ws();
            match self.peek() {
                Some(b':') => self.i += 1,
                _ => return false,
            }
            match self.value(child_ctx) {
                Parsed::Complete(v) => {
                    if let Some(k) = keys.as_deref_mut() {
                        if !k.contains(&key) {
                            k.push(key.clone());
                        }
                    }
                    map.insert(key, v);
                }
                Parsed::Partial(v) => {
                    if let Some(v) = v {
                        map.insert(key, v);
                    }
                    return false;
                }
            }
        }
    }

    fn value(&mut self, ctx: Ctx) -> Parsed {
        self.ws();
        let Some(c) = self.peek() else {
            return Parsed::Partial(None);
        };
        match c {
            b'"' => {
                self.i += 1;
                let (s, done) = self.string_body();
                if done {
                    Parsed::Complete(Value::String(s))
                } else if ctx == Ctx::TopLevel || ctx == Ctx::Nested {
                    // Partial strings show progress for object fields; array
                    // elements must be whole.
                    Parsed::Partial(Some(Value::String(s)))
                } else {
                    Parsed::Partial(None)
                }
            }
            b'{' => {
                self.i += 1;
                let mut map = Map::new();
                let done = self.object_body(&mut map, None, Ctx::Nested);
                if done {
                    Parsed::Complete(Value::Object(map))
                } else {
                    Parsed::Partial(Some(Value::Object(map)))
                }
            }
            b'[' => {
                self.i += 1;
                let (items, done) = self.array_body();
                if done {
                    Parsed::Complete(Value::Array(items))
                } else {
                    Parsed::Partial(Some(Value::Array(items)))
                }
            }
            b't' => self.literal("true", Value::Bool(true)),
            b'f' => self.literal("false", Value::Bool(false)),
            b'n' => self.literal("null", Value::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Parsed::Partial(None),
        }
    }

    fn array_body(&mut self) -> (Vec<Value>, bool) {
        let mut items = Vec::new();
        loop {
            self.ws();
            match self.peek() {
                None => return (items, false),
                Some(b']') => {
                    self.i += 1;
                    return (items, true);
                }
                Some(b',') => {
                    self.i += 1;
                    continue;
                }
                Some(_) => {}
            }
            match self.value(Ctx::Array) {
                Parsed::Complete(v) => items.push(v),
                Parsed::Partial(v) => {
                    if let Some(v) = v {
                        items.push(v);
                    }
                    return (items, false);
                }
            }
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Parsed {
        let rest = &self.s[self.i..];
        let w = word.as_bytes();
        if rest.len() >= w.len() {
            if &rest[..w.len()] == w {
                self.i += w.len();
                Parsed::Complete(v)
            } else {
                self.i = self.s.len();
                Parsed::Partial(None)
            }
        } else {
            self.i = self.s.len();
            Parsed::Partial(None)
        }
    }

    fn number(&mut self) -> Parsed {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i >= self.s.len() {
            // More digits may follow.
            return Parsed::Partial(None);
        }
        let text = &self.src[start..self.i];
        match parse_number(text) {
            Some(n) => Parsed::Complete(Value::Number(n)),
            None => Parsed::Partial(None),
        }
    }

    /// Reads string content after the opening quote. Returns the decoded text
    /// and whether the closing quote was reached. An escape cut off at the end
    /// of input is dropped.
    fn string_body(&mut self) -> (String, bool) {
        let mut out = String::new();
        let mut run_start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            match c {
                b'"' => {
                    out.push_str(&self.src[run_start..self.i]);
                    self.i += 1;
                    return (out, true);
                }
                b'\\' => {
                    out.push_str(&self.src[run_start..self.i]);
                    match self.escape() {
                        Some(ch) => out.push_str(&ch),
                        None => {
                            self.i = self.s.len();
                            return (out, false);
                        }
                    }
                    run_start = self.i;
                }
                _ => self.i += 1,
            }
        }
        out.push_str(&self.src[run_start..self.i]);
        (out, false)
    }

    /// At a backslash. Returns the decoded text, or None if input ends first.
    fn escape(&mut self) -> Option<String> {
        let kind = *self.s.get(self.i + 1)?;
        self.i += 2;
        let s = match kind {
            b'"' => "\"".to_string(),
            b'\\' => "\\".to_string(),
            b'/' => "/".to_string(),
            b'b' => "\u{0008}".to_string(),
            b'f' => "\u{000C}".to_string(),
            b'n' => "\n".to_string(),
            b'r' => "\r".to_string(),
            b't' => "\t".to_string(),
            b'u' => {
                let hi = self.hex4()?;
                if (0xD800..0xDC00).contains(&hi) {
                    // High surrogate: needs a following \uXXXX low surrogate.
                    if self.s.get(self.i) == Some(&b'\\') && self.s.get(self.i + 1) == Some(&b'u') {
                        self.i += 2;
                        let lo = self.hex4()?;
                        let code = 0x10000 + ((hi - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                        char::from_u32(code).map(String::from).unwrap_or_else(|| "\u{FFFD}".into())
                    } else if self.i + 1 >= self.s.len() {
                        return None;
                    } else {
                        "\u{FFFD}".to_string()
                    }
                } else {
                    char::from_u32(hi).map(String::from).unwrap_or_else(|| "\u{FFFD}".into())
                }
            }
            other => (other as char).to_string(),
        };
        Some(s)
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.src.get(self.i..self.i + 4)?;
        let v = u32::from_str_radix(digits, 16).ok()?;
        self.i += 4;
        Some(v)
    }
}

fn parse_number(text: &str) -> Option<Number> {
    if let Ok(i) = text.parse::<i64>() {
        return Some(Number::from(i));
    }
    text.parse::<f64>().ok().and_then(Number::from_f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(s: &str) -> Partial {
        parse_partial_object(s).expect("object")
    }

    #[test]
    fn complete_object_matches_serde() {
        let src = r#"{"title":"Waves","initialHeight":420,"placeholderMessages":["a","b"],"css":".x{color:red}","ok":true,"n":null,"f":-1.5e2}"#;
        let p = parse(src);
        assert!(p.complete);
        assert_eq!(Value::Object(p.value), serde_json::from_str::<Value>(src).unwrap());
        assert_eq!(p.complete_keys.len(), 7);
    }

    #[test]
    fn partial_top_level_string_is_exposed_but_not_complete() {
        let p = parse(r#"{"title":"Fourier","css":".a{} .b{col"#);
        assert_eq!(p.value["title"], "Fourier");
        assert_eq!(p.value["css"], ".a{} .b{col");
        assert_eq!(p.complete_keys, vec!["title"]);
        assert!(!p.complete);
    }

    #[test]
    fn unfinished_number_is_omitted() {
        let p = parse(r#"{"initialHeight":42"#);
        assert!(p.value.get("initialHeight").is_none());
        let p = parse(r#"{"initialHeight":420,"#);
        assert_eq!(p.value["initialHeight"], 420);
        assert_eq!(p.complete_keys, vec!["initialHeight"]);
    }

    #[test]
    fn array_strings_must_be_whole() {
        let p = parse(r#"{"jsExpressions":["init();","draw(1"#);
        assert_eq!(p.value["jsExpressions"], json!(["init();"]));
        assert!(p.complete_keys.is_empty());
        let p = parse(r#"{"jsExpressions":["init();","draw(1);"]}"#);
        assert_eq!(p.value["jsExpressions"], json!(["init();", "draw(1);"]));
        assert_eq!(p.complete_keys, vec!["jsExpressions"]);
    }

    #[test]
    fn partial_objects_in_arrays_show_progress() {
        let p = parse(r#"{"questions":[{"prompt":"Q1","options":["a","b"]},{"prompt":"Q2","opt"#);
        let qs = p.value["questions"].as_array().unwrap();
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[1]["prompt"], "Q2");
    }

    #[test]
    fn escapes_decode_and_cut_escapes_are_dropped() {
        let p = parse(r#"{"html":"<p class=\"x\">a\nb\u00e9\ud83d\ude00</p>"}"#);
        assert_eq!(p.value["html"], "<p class=\"x\">a\nbé😀</p>");
        let p = parse(r#"{"html":"abc\"#);
        assert_eq!(p.value["html"], "abc");
        let p = parse(r#"{"html":"abc\u00"#);
        assert_eq!(p.value["html"], "abc");
        let p = parse(r#"{"html":"x\ud83d"#);
        assert_eq!(p.value["html"], "x");
    }

    #[test]
    fn multibyte_text_is_preserved() {
        let p = parse("{\"t\":\"Ångström · λ = h/p — ∑\",\"u\":\"日本");
        assert_eq!(p.value["t"], "Ångström · λ = h/p — ∑");
        assert_eq!(p.value["u"], "日本");
    }

    #[test]
    fn partial_keys_and_literals() {
        let p = parse(r#"{"a":1,"tit"#);
        assert_eq!(p.value.len(), 1);
        let p = parse(r#"{"a":tr"#);
        assert!(p.value.is_empty());
        let p = parse(r#"{"a":true}"#);
        assert_eq!(p.value["a"], true);
    }

    #[test]
    fn every_prefix_parses_without_panicking() {
        let src = r#"{"title":"Sim","initialHeight":500,"placeholderMessages":["Setting up…","Drawing"],"css":"body{margin:0}","html":"<div id=\"app\">ünï</div>","jsFunctions":"function a(){return \"\\n\";}","jsExpressions":["a()","b({x:[1,2]})"]}"#;
        let full: Value = serde_json::from_str(src).unwrap();
        let mut last_keys = 0;
        for (idx, _) in src.char_indices() {
            if let Some(p) = parse_partial_object(&src[..idx]) {
                assert!(p.complete_keys.len() >= last_keys, "completed keys never regress");
                last_keys = p.complete_keys.len();
                for k in &p.complete_keys {
                    assert_eq!(p.value[k], full[k], "complete key {k} must equal final value");
                }
            }
        }
        let p = parse(src);
        assert_eq!(Value::Object(p.value), full);
    }

    #[test]
    fn non_objects_are_rejected() {
        assert!(parse_partial_object("").is_none());
        assert!(parse_partial_object("  ").is_none());
        assert!(parse_partial_object("[1,2]").is_none());
        assert!(parse_partial_object("  {").is_some());
    }
}
