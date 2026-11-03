//! Lightweight, zero-external-dependency RFC 8259 JSON AST and parser.

use std::collections::BTreeMap;

/// Lightweight, zero-external-dependency RFC 8259 JSON AST used by the JSON-RPC 2.0 and MCP servers.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    /// Finite non-integer (fraction/exponent) number; integer literals always parse to `Number`.
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn obj(entries: impl IntoIterator<Item = (impl Into<String>, JsonValue)>) -> Self {
        let mut map = BTreeMap::new();
        for (k, v) in entries {
            map.insert(k.into(), v);
        }
        Self::Object(map)
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Float(f) if f.is_finite() && f.fract() == 0.0 && *f >= -9.223_372_036_854_776e18 && *f < 9.223_372_036_854_776e18 => {
                Some(*f as i64)
            }
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n as f64),
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_i64().and_then(|n| usize::try_from(n).ok())
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    /// Serializes this value into a compact RFC 8259 JSON string.
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Number(n) => out.push_str(&n.to_string()),
            Self::Float(f) if f.is_finite() => out.push_str(&format!("{f:?}")),
            Self::Float(_) => out.push_str("null"),
            Self::String(s) => {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            Self::Object(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    Self::String(k.clone()).write_json(out);
                    out.push(':');
                    v.write_json(out);
                }
                out.push('}');
            }
        }
    }

    /// Parses an RFC 8259 JSON string into a `JsonValue`.
    pub fn parse(input: &str) -> Option<Self> {
        let bytes = input.as_bytes();
        let mut pos = 0;
        let val = parse_json_value(bytes, &mut pos, 0)?;
        skip_ws(bytes, &mut pos);
        if pos == bytes.len() { Some(val) } else { None }
    }
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && matches!(bytes[*pos], b' ' | b'\t' | b'\r' | b'\n') {
        *pos += 1;
    }
}

/// Maximum array/object nesting accepted by [`JsonValue::parse`]; deeper input is rejected so a
/// hostile client cannot exhaust the stack of the recursive parser.
pub(super) const MAX_JSON_DEPTH: usize = 128;

fn parse_json_value(bytes: &[u8], pos: &mut usize, depth: usize) -> Option<JsonValue> {
    if depth > MAX_JSON_DEPTH {
        return None;
    }
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return None;
    }
    match bytes[*pos] {
        b'n' if bytes[*pos..].starts_with(b"null") => {
            *pos += 4;
            Some(JsonValue::Null)
        }
        b't' if bytes[*pos..].starts_with(b"true") => {
            *pos += 4;
            Some(JsonValue::Bool(true))
        }
        b'f' if bytes[*pos..].starts_with(b"false") => {
            *pos += 5;
            Some(JsonValue::Bool(false))
        }
        b'"' => parse_json_string(bytes, pos).map(JsonValue::String),
        b'[' => {
            *pos += 1;
            skip_ws(bytes, pos);
            let mut arr = Vec::new();
            if *pos < bytes.len() && bytes[*pos] == b']' {
                *pos += 1;
                return Some(JsonValue::Array(arr));
            }
            loop {
                arr.push(parse_json_value(bytes, pos, depth + 1)?);
                skip_ws(bytes, pos);
                if *pos >= bytes.len() {
                    return None;
                }
                match bytes[*pos] {
                    b',' => *pos += 1,
                    b']' => {
                        *pos += 1;
                        return Some(JsonValue::Array(arr));
                    }
                    _ => return None,
                }
            }
        }
        b'{' => {
            *pos += 1;
            skip_ws(bytes, pos);
            let mut map = BTreeMap::new();
            if *pos < bytes.len() && bytes[*pos] == b'}' {
                *pos += 1;
                return Some(JsonValue::Object(map));
            }
            loop {
                skip_ws(bytes, pos);
                let key = parse_json_string(bytes, pos)?;
                skip_ws(bytes, pos);
                if *pos >= bytes.len() || bytes[*pos] != b':' {
                    return None;
                }
                *pos += 1;
                let val = parse_json_value(bytes, pos, depth + 1)?;
                map.insert(key, val);
                skip_ws(bytes, pos);
                if *pos >= bytes.len() {
                    return None;
                }
                match bytes[*pos] {
                    b',' => *pos += 1,
                    b'}' => {
                        *pos += 1;
                        return Some(JsonValue::Object(map));
                    }
                    _ => return None,
                }
            }
        }
        b'-' | b'0'..=b'9' => parse_json_number(bytes, pos),
        _ => None,
    }
}

/// RFC 8259 number: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`.
fn parse_json_number(bytes: &[u8], pos: &mut usize) -> Option<JsonValue> {
    let digits = |pos: &mut usize| {
        let s = *pos;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
        *pos - s
    };
    let start = *pos;
    if bytes[*pos] == b'-' {
        *pos += 1;
    }
    match bytes.get(*pos) {
        Some(b'0') => *pos += 1,
        Some(b'1'..=b'9') => {
            digits(pos);
        }
        _ => return None,
    }
    let mut is_float = false;
    if bytes.get(*pos) == Some(&b'.') {
        *pos += 1;
        is_float = true;
        if digits(pos) == 0 {
            return None;
        }
    }
    if matches!(bytes.get(*pos), Some(b'e' | b'E')) {
        *pos += 1;
        is_float = true;
        if matches!(bytes.get(*pos), Some(b'+' | b'-')) {
            *pos += 1;
        }
        if digits(pos) == 0 {
            return None;
        }
    }
    let text = std::str::from_utf8(&bytes[start..*pos]).ok()?;
    if is_float {
        let f = text.parse::<f64>().ok().filter(|f| f.is_finite())?;
        Some(JsonValue::Float(f))
    } else {
        text.parse::<i64>().ok().map(JsonValue::Number)
    }
}

fn parse_hex4(bytes: &[u8], pos: &mut usize) -> Option<u32> {
    let chunk = bytes.get(*pos..*pos + 4)?;
    if !chunk.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    *pos += 4;
    u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()
}

fn parse_json_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
    if *pos >= bytes.len() || bytes[*pos] != b'"' {
        return None;
    }
    *pos += 1;
    let mut out = String::new();
    while *pos < bytes.len() {
        let b = bytes[*pos];
        *pos += 1;
        match b {
            b'"' => return Some(out),
            b'\\' => {
                if *pos >= bytes.len() {
                    return None;
                }
                let esc = bytes[*pos];
                *pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'u' => {
                        let code = parse_hex4(bytes, pos)?;
                        let ch = match code {
                            0xD800..=0xDBFF => {
                                if bytes.get(*pos..*pos + 2) != Some(b"\\u") {
                                    return None;
                                }
                                *pos += 2;
                                let low = parse_hex4(bytes, pos)?;
                                if !(0xDC00..=0xDFFF).contains(&low) {
                                    return None;
                                }
                                char::from_u32(0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00))?
                            }
                            _ => char::from_u32(code)?, // lone low surrogate -> None
                        };
                        out.push(ch);
                    }
                    _ => return None,
                }
            }
            b if b < 0x20 => return None,
            _ => {
                let start = *pos - 1;
                let len = match b {
                    0..=0x7f => 1,
                    0xc0..=0xdf => 2,
                    0xe0..=0xef => 3,
                    _ => 4,
                };
                let rest = std::str::from_utf8(bytes.get(start..start + len)?).ok()?;
                let ch = rest.chars().next()?;
                *pos = start + ch.len_utf8();
                out.push(ch);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_and_exponents_parse() {
        assert_eq!(JsonValue::parse("1.5"), Some(JsonValue::Float(1.5)));
        assert_eq!(JsonValue::parse("-0.5"), Some(JsonValue::Float(-0.5)));
        assert_eq!(JsonValue::parse("2e3").and_then(|v| v.as_i64()), Some(2000));
        assert_eq!(JsonValue::parse("1E-2"), Some(JsonValue::Float(0.01)));
        assert_eq!(JsonValue::parse("1.5").unwrap().as_i64(), None);
        assert_eq!(JsonValue::parse("7"), Some(JsonValue::Number(7)));
        for bad in ["1.", ".5", "1e", "1e+", "1e999", "-", "--1", "+1", "1.e2"] {
            assert_eq!(JsonValue::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn float_round_trip_and_non_finite() {
        for f in [1.5, -0.5, 0.01, 1e300, 2.0] {
            let v = JsonValue::Float(f);
            let back = JsonValue::parse(&v.to_json_string()).unwrap();
            assert_eq!(back.as_f64(), Some(f));
        }
        assert_eq!(JsonValue::Float(f64::NAN).to_json_string(), "null");
        assert_eq!(JsonValue::Float(f64::INFINITY).to_json_string(), "null");
    }

    #[test]
    fn leading_zeros() {
        for bad in ["01", "-01", "00", "[01]", "01.5"] {
            assert_eq!(JsonValue::parse(bad), None, "{bad}");
        }
        for ok in ["0", "-0", "0.5", "0e1"] {
            assert!(JsonValue::parse(ok).is_some(), "{ok}");
        }
    }

    #[test]
    fn surrogates() {
        let s = |t: &str| JsonValue::parse(t).and_then(|v| v.as_str().map(String::from));
        assert_eq!(s(r#""\ud83d\ude00""#).as_deref(), Some("\u{1F600}"));
        for bad in [r#""\ud83d""#, r#""\ude00""#, r#""\ud83dx""#, r#""\ud83d\u0041""#, r#""\ud83d\ud83d""#, r#""\u+123""#, r#""\u12""#] {
            assert_eq!(s(bad), None, "{bad}");
        }
    }

    #[test]
    fn strings_strict() {
        assert_eq!(JsonValue::parse("\"a\nb\""), None, "raw control char");
        assert_eq!(JsonValue::parse(r#""\q""#), None, "unknown escape");
        assert_eq!(JsonValue::parse("\"héllo😀\"").unwrap().as_str(), Some("héllo😀"));
    }

    #[test]
    fn deep_nesting_rejected() {
        let t = "[".repeat(10_000) + &"]".repeat(10_000);
        assert_eq!(JsonValue::parse(&t), None);
        let o = "{\"a\":".repeat(10_000);
        assert_eq!(JsonValue::parse(&o), None);
        let ok = "[".repeat(100) + &"]".repeat(100);
        assert!(JsonValue::parse(&ok).is_some());
    }
}
