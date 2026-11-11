//! URL policy for the browser canvas: what may be handed to Chrome (`WEB-01`).
//!
//! Only `http://`, `https://` and `about:blank` are allowed by default. `file://` needs the explicit
//! opt-in `BILTERM_BROWSER_ALLOW_FILE=1`. Everything else (`javascript:`, `data:`, `chrome:`,
//! `view-source:`, anything starting with `-`, control characters) is rejected before it reaches the
//! browser process.

use std::fmt;

/// Longest URL accepted from the user, in bytes.
pub const MAX_URL_BYTES: usize = 8 * 1024;

/// Environment opt-in that allows `file://` URLs.
pub const ALLOW_FILE_ENV: &str = "BILTERM_BROWSER_ALLOW_FILE";

/// Why a URL was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UrlRejection {
    Empty,
    TooLong,
    LeadingDash,
    ControlCharacter,
    MissingHost,
    FileNotEnabled,
    SchemeNotAllowed(String),
}

impl fmt::Display for UrlRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty URL"),
            Self::TooLong => write!(f, "URL longer than {MAX_URL_BYTES} bytes"),
            Self::LeadingDash => write!(f, "URL must not start with '-'"),
            Self::ControlCharacter => write!(f, "URL contains a control character or space"),
            Self::MissingHost => write!(f, "URL has no host"),
            Self::FileNotEnabled => write!(f, "file:// URLs need {ALLOW_FILE_ENV}=1"),
            Self::SchemeNotAllowed(s) => write!(f, "scheme '{s}:' is not allowed (http, https, about:blank only)"),
        }
    }
}

/// `true` when the user opted in to `file://` URLs through the environment.
pub fn file_urls_enabled() -> bool {
    matches!(std::env::var(ALLOW_FILE_ENV).as_deref(), Ok("1") | Ok("true") | Ok("yes"))
}

/// Splits `scheme:rest` when `text` starts with a URL scheme. `host:8080` is not a scheme.
fn split_scheme(text: &str) -> Option<(&str, &str)> {
    let colon = text.find(':')?;
    let (scheme, rest) = (&text[..colon], &text[colon + 1..]);
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    if rest.starts_with("//") {
        return Some((scheme, rest));
    }
    // `localhost:8080/path` is host:port. A dotted or `localhost` prefix followed by digits is a port.
    let port_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let after_port = &rest[port_len..];
    if port_len > 0 && (after_port.is_empty() || after_port.starts_with('/') || after_port.starts_with('?')) {
        return None;
    }
    Some((scheme, rest))
}

/// Validates a user-supplied URL and returns the exact string to navigate to.
///
/// A bare host (`example.com`, `localhost:8080/x`) gets the `https://` default.
pub fn validate_url(input: &str, allow_file: bool) -> Result<String, UrlRejection> {
    let text = input.trim();
    if text.is_empty() {
        return Err(UrlRejection::Empty);
    }
    if text.len() > MAX_URL_BYTES {
        return Err(UrlRejection::TooLong);
    }
    if text.starts_with('-') {
        return Err(UrlRejection::LeadingDash);
    }
    if text.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(UrlRejection::ControlCharacter);
    }
    let Some((scheme, rest)) = split_scheme(text) else {
        let host = text.split(['/', '?', '#']).next().unwrap_or("");
        if host.is_empty() {
            return Err(UrlRejection::MissingHost);
        }
        return Ok(format!("https://{text}"));
    };
    let scheme_lc = scheme.to_ascii_lowercase();
    match scheme_lc.as_str() {
        "http" | "https" => {
            let after = rest.strip_prefix("//").ok_or(UrlRejection::MissingHost)?;
            let authority = after.split(['/', '?', '#']).next().unwrap_or("");
            let host = authority.rsplit('@').next().unwrap_or("");
            if host.is_empty() || host.starts_with(':') {
                return Err(UrlRejection::MissingHost);
            }
            Ok(format!("{scheme_lc}:{rest}"))
        }
        "about" if rest.eq_ignore_ascii_case("blank") => Ok("about:blank".to_string()),
        "file" => {
            if !allow_file {
                return Err(UrlRejection::FileNotEnabled);
            }
            if !rest.starts_with("//") {
                return Err(UrlRejection::MissingHost);
            }
            Ok(format!("file:{rest}"))
        }
        _ => Err(UrlRejection::SchemeNotAllowed(scheme_lc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_http_https_and_about_blank() {
        assert_eq!(validate_url("https://example.com/a?b#c", false).unwrap(), "https://example.com/a?b#c");
        assert_eq!(validate_url("HTTP://Example.com", false).unwrap(), "http://Example.com");
        assert_eq!(validate_url("  about:blank ", false).unwrap(), "about:blank");
        assert_eq!(validate_url("http://127.0.0.1:8080/x", false).unwrap(), "http://127.0.0.1:8080/x");
        assert_eq!(validate_url("https://user@host.example/", false).unwrap(), "https://user@host.example/");
    }

    #[test]
    fn bare_hosts_default_to_https() {
        assert_eq!(validate_url("example.com", false).unwrap(), "https://example.com");
        assert_eq!(validate_url("localhost:8080/x", false).unwrap(), "https://localhost:8080/x");
        assert_eq!(validate_url("rust-lang.org/learn", false).unwrap(), "https://rust-lang.org/learn");
    }

    #[test]
    fn rejects_dangerous_schemes() {
        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<h1>x</h1>",
            "chrome://settings",
            "chrome:version",
            "view-source:https://example.com",
            "about:config",
            "blob:https://example.com/1",
            "ftp://example.com",
            "vbscript:x",
        ] {
            assert!(
                matches!(validate_url(bad, true), Err(UrlRejection::SchemeNotAllowed(_))),
                "{bad} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_flags_control_chars_and_empty() {
        assert_eq!(validate_url("--remote-debugging-port=9222", false), Err(UrlRejection::LeadingDash));
        assert_eq!(validate_url("-", false), Err(UrlRejection::LeadingDash));
        assert_eq!(validate_url("", false), Err(UrlRejection::Empty));
        assert_eq!(validate_url("   ", false), Err(UrlRejection::Empty));
        assert_eq!(validate_url("https://a.b/\u{0}", false), Err(UrlRejection::ControlCharacter));
        assert_eq!(validate_url("https://a.b/x y", false), Err(UrlRejection::ControlCharacter));
        assert_eq!(validate_url("https://", false), Err(UrlRejection::MissingHost));
        assert_eq!(validate_url("https://:80/", false), Err(UrlRejection::MissingHost));
        assert_eq!(validate_url(&format!("https://a.b/{}", "x".repeat(MAX_URL_BYTES)), false), Err(UrlRejection::TooLong));
    }

    #[test]
    fn file_urls_need_the_opt_in() {
        assert_eq!(validate_url("file:///etc/hostname", false), Err(UrlRejection::FileNotEnabled));
        assert_eq!(validate_url("file:///etc/hostname", true).unwrap(), "file:///etc/hostname");
        assert_eq!(validate_url("file:etc", true), Err(UrlRejection::MissingHost));
    }
}
