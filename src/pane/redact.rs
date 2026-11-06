//! Secret / exclusion classifier for captured commands (Phase 50.1, D-U2).
//!
//! `classify` only decides; it never stores, returns or logs the command text. The rules are
//! the hand-written list in `50.1-CONTRACT.md` section 9 (no regex crate). Every matcher is a
//! single pass over the words or bytes of the input, so the whole classification is linear in
//! the command length. Matching is ASCII case-insensitive without allocating a lowercase copy.
//! Pane layer: imports nothing outside `std`.

/// Why a command is not recorded or indexed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
    /// Blank command.
    Empty,
    /// A bilterm `:colon` command (never a shell command).
    Colon,
    /// Looks like it carries a credential.
    Secret,
}

/// Classifies `command`; `None` means it may be stored and indexed.
///
/// `Secret` wins over `Colon` when the colon text itself holds a secret shape.
pub fn classify(command: &str) -> Option<Exclusion> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Some(Exclusion::Empty);
    }
    if looks_secret(trimmed) {
        return Some(Exclusion::Secret);
    }
    if trimmed.starts_with(':') {
        return Some(Exclusion::Colon);
    }
    None
}

const QUOTES: [char; 3] = ['"', '\'', '`'];

const SECRET_NAMES: [&str; 10] = [
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "api-key",
    "credential",
    "private_key",
    "access_key",
];

const SECRET_FLAGS: [&str; 10] = [
    "--password",
    "--passwd",
    "--token",
    "--secret",
    "--api-key",
    "--apikey",
    "--auth-token",
    "--client-secret",
    "--access-key",
    "--passphrase",
];

const MYSQL_FAMILY: [&str; 4] = ["mysql", "mariadb", "mysqldump", "mysqladmin"];
const HEADER_NEEDLES: [&str; 2] = ["authorization:", "x-api-key:"];
const BEARER_MIN_TOKEN: usize = 8;

#[cfg(test)]
thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts scanned characters in tests so linearity can be asserted without timing.
#[cfg(test)]
fn tick(n: usize) {
    STEPS.with(|s| s.set(s.get() + n));
}

#[cfg(not(test))]
#[inline(always)]
fn tick(_n: usize) {}

fn looks_secret(text: &str) -> bool {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();
    tick(text.len());
    token_shape(text)
        || assignment_secret(&words)
        || flag_secret(&words)
        || short_password_flag(&words)
        || header_secret(&words)
        || url_userinfo(&words)
        || tool_specific(&words)
}

fn unquote(s: &str) -> &str {
    s.trim_matches(QUOTES)
}

fn has_value(word: Option<&&str>) -> bool {
    word.is_some_and(|w| !unquote(w).is_empty())
}

fn starts_with_ci(hay: &str, needle: &str) -> bool {
    hay.len() >= needle.len() && hay.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

/// Byte offset of the first ASCII-case-insensitive match of the ASCII `needle`.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    h.windows(n.len()).position(|w| w.eq_ignore_ascii_case(n))
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// (a) `NAME=value` with a secret-ish NAME and a non-empty value.
fn assignment_secret(words: &[&str]) -> bool {
    words.iter().any(|w| {
        let w = unquote(w);
        let Some(eq) = w.find('=') else { return false };
        tick(w.len());
        let (name, value) = (&w[..eq], &w[eq + 1..]);
        if name.starts_with('-') || unquote(value).is_empty() {
            return false; // flags belong to `flag_secret`
        }
        let tail = name
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
            .next()
            .unwrap_or("");
        SECRET_NAMES.iter().any(|k| find_ci(tail, k).is_some())
    })
}

/// (b) `--password value`, `--token=value`, ...
fn flag_secret(words: &[&str]) -> bool {
    words.iter().enumerate().any(|(i, w)| {
        let w = unquote(w);
        tick(w.len().min(16));
        SECRET_FLAGS.iter().any(|f| {
            if !starts_with_ci(w, f) {
                return false;
            }
            let rest = &w[f.len()..];
            if rest.is_empty() {
                words
                    .get(i + 1)
                    .is_some_and(|n| !unquote(n).is_empty() && !n.starts_with('-'))
            } else {
                rest.strip_prefix('=').is_some_and(|v| !unquote(v).is_empty())
            }
        })
    })
}

/// (c) `mysql -pVALUE` family and `sshpass -p VALUE`.
fn short_password_flag(words: &[&str]) -> bool {
    let mut family = false;
    let mut sshpass = false;
    for (i, w) in words.iter().enumerate() {
        tick(w.len());
        let w = unquote(w);
        let base = basename(w);
        if MYSQL_FAMILY.iter().any(|c| base.eq_ignore_ascii_case(c)) {
            family = true;
            continue;
        }
        if base.eq_ignore_ascii_case("sshpass") {
            sshpass = true;
            continue;
        }
        if (family || sshpass) && w.starts_with("-p") && w.len() > 2 {
            return true; // attached value
        }
        if sshpass && w == "-p" && has_value(words.get(i + 1)) {
            return true;
        }
    }
    false
}

/// (d) `Authorization:`, `Proxy-Authorization:`, `X-Api-Key:` headers and `Bearer <token>`.
fn header_secret(words: &[&str]) -> bool {
    words.iter().enumerate().any(|(i, w)| {
        tick(w.len());
        for needle in HEADER_NEEDLES {
            if let Some(pos) = find_ci(w, needle) {
                let rest = unquote(&w[pos + needle.len()..]);
                if !rest.is_empty() || has_value(words.get(i + 1)) {
                    return true;
                }
            }
        }
        unquote(w).eq_ignore_ascii_case("bearer")
            && words.get(i + 1).is_some_and(|n| unquote(n).len() >= BEARER_MIN_TOKEN)
    })
}

/// (e) `scheme://user:password@host`.
fn url_userinfo(words: &[&str]) -> bool {
    words.iter().any(|w| {
        tick(w.len());
        let mut from = 0;
        while let Some(p) = w[from..].find("://") {
            let start = from + p + 3;
            let rest = &w[start..];
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let authority = &rest[..end];
            if let Some(at) = authority.rfind('@')
                && let Some((_, pass)) = authority[..at].split_once(':')
                && !pass.is_empty()
            {
                return true;
            }
            from = start;
        }
        false
    })
}

/// (f) Known token shapes, found by one walk over the bytes.
fn token_shape(text: &str) -> bool {
    pem_private_key(text) || scan_token_prefixes(text.as_bytes())
}

fn pem_private_key(text: &str) -> bool {
    const BEGIN: &str = "-----begin ";
    find_ci(text, BEGIN).is_some_and(|p| find_ci(&text[p + BEGIN.len()..], "private key-----").is_some())
}

fn is_word_us(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_token_dash(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// True when `b[at..]` starts with `min` consecutive bytes accepted by `pred` (scan stops at `min`).
fn run_of(b: &[u8], at: usize, min: usize, pred: fn(u8) -> bool) -> bool {
    b.get(at..at + min).is_some_and(|s| s.iter().all(|&c| pred(c)))
}

fn scan_token_prefixes(b: &[u8]) -> bool {
    let mut i = 0;
    while i < b.len() {
        tick(1);
        let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if boundary {
            let rest = &b[i..];
            let hit = match b[i] {
                b'g' => {
                    ([b"ghp_", b"gho_", b"ghs_", b"ghu_", b"ghr_"]
                        .iter()
                        .any(|p| rest.starts_with(*p))
                        && run_of(b, i + 4, 20, is_word_us))
                        || (rest.starts_with(b"github_pat_") && run_of(b, i + 11, 20, is_word_us))
                }
                b's' => rest.starts_with(b"sk-") && run_of(b, i + 3, 20, is_token_dash),
                b'A' => {
                    (rest.starts_with(b"AKIA") && run_of(b, i + 4, 16, |c| c.is_ascii_digit() || c.is_ascii_uppercase()))
                        || (rest.starts_with(b"AIza") && run_of(b, i + 4, 35, is_token_dash))
                }
                b'x' => {
                    rest.starts_with(b"xox")
                        && rest.get(3).is_some_and(|c| b"abprs".contains(c))
                        && rest.get(4) == Some(&b'-')
                        && run_of(b, i + 5, 8, is_token_dash)
                }
                b'e' if rest.starts_with(b"eyJ") => match jwt_end(b, i) {
                    Ok(()) => true,
                    Err(skip_to) => {
                        i = skip_to.max(i + 1);
                        continue;
                    }
                },
                _ => false,
            };
            if hit {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// `eyJ<b64>.eyJ<b64>.` starting at `i`; on failure returns where the scan may resume (end of the
/// first run, since any start inside it would fail identically and so stay linear).
fn jwt_end(b: &[u8], i: usize) -> Result<(), usize> {
    let run_end = |from: usize| {
        let mut j = from;
        while j < b.len() && is_token_dash(b[j]) {
            j += 1;
        }
        tick(j - from);
        j
    };
    let j = run_end(i + 3);
    if b.get(j) != Some(&b'.') || !b[j + 1..].starts_with(b"eyJ") {
        return Err(j);
    }
    let k = run_end(j + 4);
    if b.get(k) == Some(&b'.') { Ok(()) } else { Err(j) }
}

/// (g) openssl `-passin/-passout pass:`, `chpasswd`, `htpasswd -b`, `echo|printf ... | sudo -S`.
fn tool_specific(words: &[&str]) -> bool {
    let (mut htpasswd, mut echo, mut pipe, mut sudo) = (false, false, false, false);
    for (i, w) in words.iter().enumerate() {
        tick(w.len());
        let w = unquote(w);
        if (w.eq_ignore_ascii_case("-passin") || w.eq_ignore_ascii_case("-passout"))
            && words.get(i + 1).is_some_and(|n| starts_with_ci(unquote(n), "pass:") && unquote(n).len() > 5)
        {
            return true;
        }
        if htpasswd && w.starts_with('-') && !w.starts_with("--") && w.contains('b') {
            return true;
        }
        for (idx, part) in w.split('|').enumerate() {
            if idx > 0 && echo {
                pipe = true;
            }
            let part = part.trim_matches(QUOTES);
            if part.eq_ignore_ascii_case("chpasswd") {
                return true;
            }
            htpasswd |= basename(part).eq_ignore_ascii_case("htpasswd");
            echo |= part == "echo" || part == "printf";
            sudo |= pipe && part == "sudo";
            if sudo && part.starts_with('-') && !part.starts_with("--") && part.contains('S') {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_colon_are_excluded_plain_commands_are_not() {
        assert_eq!(classify(""), Some(Exclusion::Empty));
        assert_eq!(classify("  \t "), Some(Exclusion::Empty));
        assert_eq!(classify(":find agent"), Some(Exclusion::Colon));
        assert_eq!(classify("  :q"), Some(Exclusion::Colon));
        assert_eq!(classify("ls -la"), None);
    }

    /// The API returns only the kind of exclusion, never any part of the command text.
    #[test]
    fn masked_text_never_returned() {
        let _: fn(&str) -> Option<Exclusion> = classify;
        let hit = classify("export API_TOKEN=hunter2-super-secret");
        assert_eq!(hit, Some(Exclusion::Secret));
        assert!(!format!("{hit:?}").contains("hunter2"));
    }
}

#[cfg(test)]
mod linear_tests {
    use super::*;

    fn steps_for(input: &str) -> usize {
        STEPS.with(|s| s.set(0));
        let _ = classify(input);
        STEPS.with(|s| s.get())
    }

    fn repeated(unit: &str, bytes: usize) -> String {
        unit.repeat(bytes / unit.len())
    }

    #[test]
    fn classify_is_linear() {
        let (small, large) = (64 * 1024, 1024 * 1024);
        for unit in ["a", "a=", "-p", "-p-", "eyJ-", "a://", "a ", "x|", "ghp_"] {
            let steps_64k = steps_for(&repeated(unit, small));
            let steps_1m = steps_for(&repeated(unit, large));
            assert!(steps_64k > 0, "pattern {unit:?} was never scanned");
            assert!(
                steps_1m <= 20 * steps_64k,
                "pattern {unit:?}: {steps_1m} steps at 1 MiB vs {steps_64k} at 64 KiB"
            );
        }
        // Pathological: a long assignment-looking name, and a mysql family command with endless -p words.
        let name = format!("{}=v", "a".repeat(small));
        let name_big = format!("{}=v", "a".repeat(large));
        assert!(steps_for(&name_big) <= 20 * steps_for(&name));
        let mysql = format!("mysql {}", "-p ".repeat(small / 3));
        let mysql_big = format!("mysql {}", "-p ".repeat(large / 3));
        assert!(steps_for(&mysql_big) <= 20 * steps_for(&mysql));
    }
}
