// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! A small, dependency-free JSON reader and writer.
//!
//! Volt Lapse keeps its replay metadata, configuration profiles, camera project
//! files and render queue in JSON. Horizon's own workspaces do not guarantee a
//! JSON crate in `Cargo.lock`, and a bundle crate may only depend on packages
//! that are already locked, so the engine carries its own implementation
//! instead of refusing to build on an older lock file.
//!
//! The reader is strict (RFC 8259) and the writer is deterministic: object keys
//! keep insertion order, so two equal configurations always serialise to the
//! same bytes and a profile diff is meaningful.

use std::fmt;

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A number from an integer, so `tick`-like values never gain a `.0`.
    pub fn int(value: i64) -> Json {
        Json::Number(value as f64)
    }

    /// A one-key object, for building nested trees without a macro.
    pub fn obj(key: &str, value: Json) -> Json {
        Json::Object(vec![(key.to_owned(), value)])
    }

    /// The value at `key`, when this is an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value at a `/`-separated path, e.g. `record/tick_rate`.
    pub fn path(&self, path: &str) -> Option<&Json> {
        let mut node = self;
        for part in path.split('/') {
            node = node.get(part)?;
        }
        Some(node)
    }

    /// Insert or replace a key, keeping the original position when the key exists.
    pub fn set(&mut self, key: &str, value: Json) {
        if let Json::Object(entries) = self {
            if let Some(slot) = entries.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                entries.push((key.to_owned(), value));
            }
        } else {
            *self = Json::obj(key, value);
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().map(|n| n as i64)
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_f64().filter(|n| *n >= 0.0).map(|n| n as u64)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// Compact serialisation, as a `.vrc`/`metaData.json` entry stores it.
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, false);
        out
    }

    /// Indented serialisation, as a human edits a config profile.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, true);
        out
    }

    fn write(&self, out: &mut String, depth: usize, pretty: bool) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(n) => out.push_str(&format_number(*n)),
            Json::String(text) => write_string(out, text),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        if !pretty {
                            out.push(' ');
                        }
                    }
                    if pretty {
                        out.push('\n');
                        indent(out, depth + 1);
                    }
                    item.write(out, depth + 1, pretty);
                }
                if pretty && !items.is_empty() {
                    out.push('\n');
                    indent(out, depth);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                        if !pretty {
                            out.push(' ');
                        }
                    }
                    if pretty {
                        out.push('\n');
                        indent(out, depth + 1);
                    }
                    write_string(out, key);
                    out.push(':');
                    if pretty {
                        out.push(' ');
                    }
                    value.write(out, depth + 1, pretty);
                }
                if pretty && !entries.is_empty() {
                    out.push('\n');
                    indent(out, depth);
                }
                out.push('}');
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// `1` for whole numbers, shortest round-trip form otherwise; JSON has no
/// integer type, so this only affects readability.
fn format_number(n: f64) -> String {
    if !n.is_finite() {
        // JSON cannot hold NaN or infinities; the config layer clamps before
        // writing, so this is a last-resort safety net rather than a policy.
        return "0".to_owned();
    }
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        return format!("{}", n as i64);
    }
    let mut text = format!("{n}");
    if text.contains('e') && !text.contains('.') {
        // `format!` produces `1e-7`; JSON allows it, but `1.0e-7` reads better.
        text = text.replacen('e', ".0e", 1);
    }
    text
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Why a document could not be read, with the byte offset of the problem.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for JsonError {}

type Result<T> = std::result::Result<T, JsonError>;

/// Parse a document. Trailing non-whitespace is an error.
pub fn parse(text: &str) -> Result<Json> {
    let mut parser = Parser { bytes: text.as_bytes(), pos: 0 };
    parser.skip_ws();
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("trailing characters"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> JsonError {
        JsonError { message: message.to_owned(), offset: self.pos }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", byte as char)))
        }
    }

    fn value(&mut self) -> Result<Json> {
        match self.peek().ok_or_else(|| self.error("unexpected end of input"))? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Ok(Json::String(self.string()?)),
            b't' => self.literal("true", Json::Bool(true)),
            b'f' => self.literal("false", Json::Bool(false)),
            b'n' => self.literal("null", Json::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("invalid literal"))
        }
    }

    fn object(&mut self) -> Result<Json> {
        self.expect(b'{')?;
        let mut entries: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.value()?;
            // A duplicate key is a real error: silently keeping the last one
            // hides a hand-edited config that would never behave as written.
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(self.error("duplicate object key"));
            }
            entries.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => return Ok(Json::Object(entries)),
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self) -> Result<Json> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => return Ok(Json::Array(items)),
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn string(&mut self) -> Result<String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.bump().ok_or_else(|| self.error("unterminated string"))? {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = self.bump().ok_or_else(|| self.error("unterminated escape"))?;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            // Surrogate pairs: a lone surrogate is invalid UTF-8
                            // input for Rust strings, so pair them or refuse.
                            if (0xD800..0xDC00).contains(&code) {
                                if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                    return Err(self.error("lone high surrogate"));
                                }
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err(self.error("invalid low surrogate"));
                                }
                                code = 0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            } else if (0xDC00..0xE000).contains(&code) {
                                return Err(self.error("lone low surrogate"));
                            }
                            out.push(char::from_u32(code).ok_or_else(|| self.error("invalid code point"))?);
                        }
                        _ => return Err(self.error("unknown escape")),
                    }
                }
                byte if byte < 0x20 => return Err(self.error("control character in string")),
                byte => {
                    // Re-decode UTF-8 for multi-byte characters.
                    let start = self.pos - 1;
                    let len = utf8_len(byte);
                    if start + len > self.bytes.len() {
                        return Err(self.error("truncated UTF-8"));
                    }
                    let slice = &self.bytes[start..start + len];
                    let text = std::str::from_utf8(slice).map_err(|_| self.error("invalid UTF-8"))?;
                    out.push_str(text);
                    self.pos = start + len;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let mut code = 0u32;
        for _ in 0..4 {
            let byte = self.bump().ok_or_else(|| self.error("truncated \\u escape"))?;
            let digit = (byte as char).to_digit(16).ok_or_else(|| self.error("bad hex digit"))?;
            code = code * 16 + digit;
        }
        Ok(code)
    }

    fn number(&mut self) -> Result<Json> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let digits = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if digits == self.pos {
                return Err(self.error("expected digits after '.'"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let digits = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if digits == self.pos {
                return Err(self.error("expected digits in exponent"));
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("ASCII number");
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| self.error("invalid number"))
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_nested_document() {
        let text = r#"{"id":"voltlapse","version":1,"ticks":[1,2,3],"camera":{"fov":70.5,"roll":null},"ok":true}"#;
        let value = parse(text).expect("parses");
        assert_eq!(value.get("id").and_then(Json::as_str), Some("voltlapse"));
        assert_eq!(value.path("camera/fov").and_then(Json::as_f64), Some(70.5));
        assert_eq!(value.path("camera/roll"), Some(&Json::Null));
        assert_eq!(value.get("ticks").and_then(Json::as_array).map(<[Json]>::len), Some(3));
        assert_eq!(parse(&value.to_compact()).expect("reparses"), value);
        assert_eq!(parse(&value.to_pretty()).expect("reparses pretty"), value);
    }

    #[test]
    fn writes_whole_numbers_without_a_fraction() {
        assert_eq!(Json::int(20).to_compact(), "20");
        assert_eq!(Json::Number(-0.5).to_compact(), "-0.5");
        assert_eq!(Json::Number(1e-7).to_compact(), "0.0000001");
    }

    #[test]
    fn escapes_and_unicode_round_trip() {
        let value = Json::String("a\"b\\c\n\t\u{1}\u{1F600} Zürich".to_owned());
        let text = value.to_compact();
        assert_eq!(parse(&text).expect("parses"), value);
        assert_eq!(parse(r#""\u00e9\u0041""#).expect("parses"), Json::String("éA".to_owned()));
        assert_eq!(
            parse(r#""\ud83d\ude00""#).expect("parses surrogate pair"),
            Json::String("😀".to_owned())
        );
    }

    #[test]
    fn refuses_malformed_documents() {
        for bad in [
            "{",
            "{\"a\":1,}",
            "{\"a\":1,\"a\":2}",
            "[1 2]",
            "\"\\ud83d\"",
            "nul",
            "1 2",
            "\"a\u{1}b\"",
        ] {
            assert!(parse(bad).is_err(), "should refuse {bad:?}");
        }
    }

    #[test]
    fn set_keeps_key_order() {
        let mut value = Json::Object(vec![("a".into(), Json::int(1)), ("b".into(), Json::int(2))]);
        value.set("a", Json::int(9));
        value.set("c", Json::int(3));
        assert_eq!(value.to_compact(), r#"{"a": 9, "b": 2, "c": 3}"#);
    }
}
