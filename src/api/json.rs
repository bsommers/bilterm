//! Lightweight, zero-external-dependency RFC 8259 JSON AST and parser.

use std::collections::BTreeMap;

/// Lightweight, zero-external-dependency RFC 8259 JSON AST used by the JSON-RPC 2.0 and MCP servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
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
        b'-' | b'0'..=b'9' => {
            let start = *pos;
            if bytes[*pos] == b'-' {
                *pos += 1;
            }
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
                *pos += 1;
            }
            let int_end = *pos;
            while *pos < bytes.len()
                && matches!(bytes[*pos], b'.' | b'e' | b'E' | b'+' | b'-' | b'0'..=b'9')
            {
                *pos += 1;
            }
            let num_str = std::str::from_utf8(&bytes[start..int_end]).ok()?;
            let n = num_str.parse::<i64>().ok()?;
            Some(JsonValue::Number(n))
        }
        _ => None,
    }
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
                        if *pos + 4 > bytes.len() {
                            return None;
                        }
                        let hex = std::str::from_utf8(&bytes[*pos..*pos + 4]).ok()?;
                        *pos += 4;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        out.push(char::from_u32(code)?);
                    }
                    _ => out.push(esc as char),
                }
            }
            _ => {
                let start = *pos - 1;
                let rest = std::str::from_utf8(&bytes[start..]).ok()?;
                let ch = rest.chars().next()?;
                *pos = start + ch.len_utf8();
                out.push(ch);
            }
        }
    }
    None
}
