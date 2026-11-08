//! `--web-dir` static SPA hosting under `SEC-R01` confinement, plus `--cors-origin` (`WEB-FE-04`).
//!
//! The root is canonicalized once. Each request path is percent-decoded, rejected if it has NUL,
//! control bytes, backslashes, `..`/`.` segments or invalid UTF-8, then joined, canonicalized and
//! required to stay under the root (so symlinks cannot escape). Extension-less misses fall back to
//! `index.html` for client-side routing; missing assets are a plain 404.

use std::path::{Path, PathBuf};

/// Largest static file served (bounded memory per request).
pub const MAX_STATIC_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Canonicalized `--web-dir` root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticRoot {
    root: PathBuf,
}

/// A resolved static response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticAsset {
    pub status: u16,
    pub content_type: &'static str,
    pub cache_control: &'static str,
    pub body: Vec<u8>,
}

impl StaticRoot {
    /// Canonicalizes `dir`; it must be an existing directory.
    pub fn new(dir: &Path) -> Result<Self, String> {
        let root = std::fs::canonicalize(dir).map_err(|e| format!("--web-dir {}: {e}", dir.display()))?;
        if !root.is_dir() {
            return Err(format!("--web-dir {} is not a directory", root.display()));
        }
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

/// MIME type for a file extension.
pub fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decodes `raw`; `None` on malformed escapes or invalid UTF-8.
fn percent_decode(raw: &str) -> Option<String> {
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let (h, l) = (hex_val(*b.get(i + 1)?)?, hex_val(*b.get(i + 2)?)?);
            out.push(h << 4 | l);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Validates a request path into safe relative segments (`SEC-R01`); `Err(403)` when hostile.
pub fn sanitize_request_path(raw_path: &str) -> Result<Vec<String>, u16> {
    let path = raw_path.split(['?', '#']).next().unwrap_or("");
    let decoded = percent_decode(path).ok_or(400u16)?;
    if decoded.bytes().any(|b| b == 0 || b < 0x20 || b == 0x7f || b == b'\\') {
        return Err(403);
    }
    let mut segs = Vec::new();
    for seg in decoded.split('/') {
        match seg {
            "" => {}
            "." | ".." => return Err(403),
            s if s.contains(':') => return Err(403),
            s => segs.push(s.to_string()),
        }
    }
    Ok(segs)
}

fn read_confined(root: &Path, candidate: &Path) -> Option<(PathBuf, Vec<u8>)> {
    let canon = std::fs::canonicalize(candidate).ok()?;
    if !canon.starts_with(root) {
        return None;
    }
    let meta = std::fs::metadata(&canon).ok()?;
    if !meta.is_file() || meta.len() > MAX_STATIC_FILE_BYTES {
        return None;
    }
    std::fs::read(&canon).ok().map(|b| (canon, b))
}

fn asset(status: u16, path: &Path, body: Vec<u8>) -> StaticAsset {
    let is_html = path.extension().is_some_and(|e| e == "html");
    let hashed = path.components().any(|c| c.as_os_str() == "assets");
    StaticAsset {
        status,
        content_type: mime_for(path),
        cache_control: if is_html {
            "no-cache"
        } else if hashed {
            "public, max-age=31536000, immutable"
        } else {
            "public, max-age=300"
        },
        body,
    }
}

/// Resolves `raw_path` under `root`: file, SPA `index.html` fallback, or 403/404.
pub fn serve_static_asset(root: &StaticRoot, raw_path: &str) -> StaticAsset {
    let text = |status: u16, msg: &str| StaticAsset {
        status,
        content_type: "text/plain; charset=utf-8",
        cache_control: "no-store",
        body: msg.as_bytes().to_vec(),
    };
    let segs = match sanitize_request_path(raw_path) {
        Ok(s) => s,
        Err(403) => return text(403, "forbidden"),
        Err(code) => return text(code, "bad request"),
    };
    let mut candidate = root.root.clone();
    for s in &segs {
        candidate.push(s);
    }
    if candidate.is_dir() {
        candidate.push("index.html");
    }
    if let Some((p, body)) = read_confined(&root.root, &candidate) {
        return asset(200, &p, body);
    }
    let has_ext = segs.last().is_some_and(|l| Path::new(l).extension().is_some());
    if !has_ext
        && let Some((p, body)) = read_confined(&root.root, &root.root.join("index.html"))
    {
        return asset(200, &p, body);
    }
    text(404, "not found")
}

/// CORS response headers for `origin` (empty when CORS is not configured).
pub fn cors_headers(cors_origin: Option<&str>) -> String {
    match cors_origin {
        Some(o) => format!(
            "Access-Control-Allow-Origin: {o}\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, Idempotency-Key\r\nAccess-Control-Max-Age: 600\r\nVary: Origin\r\n"
        ),
        None => String::new(),
    }
}

/// Normalizes and validates a `--cors-origin` value (`scheme://host[:port]`, no path).
pub fn validate_cors_origin(origin: &str) -> Result<String, String> {
    let o = origin.trim().trim_end_matches('/');
    let rest = o
        .strip_prefix("http://")
        .or_else(|| o.strip_prefix("https://"))
        .ok_or_else(|| format!("--cors-origin must start with http:// or https://: '{origin}'"))?;
    if rest.is_empty() || rest.contains(['/', '?', '#', '*', ' ', '\r', '\n']) {
        return Err(format!("--cors-origin must be scheme://host[:port]: '{origin}'"));
    }
    Ok(o.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_rejects_traversal_and_controls() {
        assert_eq!(sanitize_request_path("/a/b.js"), Ok(vec!["a".into(), "b.js".into()]));
        for bad in ["/../etc/passwd", "/%2e%2e/x", "/a/%2E%2E/%2E%2E/x", "/a\\b", "/%00", "/a/./b", "/c:/x", "/%5c"] {
            assert_eq!(sanitize_request_path(bad), Err(403), "{bad}");
        }
        assert_eq!(sanitize_request_path("/%zz"), Err(400));
        assert_eq!(sanitize_request_path("/%ff%fe"), Err(400));
    }

    #[test]
    fn cors_origin_validation() {
        assert_eq!(validate_cors_origin("http://localhost:5173/").unwrap(), "http://localhost:5173");
        assert!(validate_cors_origin("*").is_err());
        assert!(validate_cors_origin("http://a/b").is_err());
        assert!(validate_cors_origin("http://a\r\nX: y").is_err());
    }
}
