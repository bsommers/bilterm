use super::VtParser;

/// Explicit `OSC 8` hyperlink recorded from the terminal stream (`VAL-03`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Osc8Hyperlink {
    pub id: Option<String>,
    pub uri: String,
}

/// Detected clickable URL span on the visible `OutputCanvas` (`VAL-03`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedUrl {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub url: String,
}

/// Match location found by `VtParser::search_canvas` across visible screen and scrollback (`VAL-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSearchMatch {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
}

/// Classification of an actionable token detected on the `OutputCanvas` by QuickSelect 3270 Field Hints (`ECO-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasHintKind {
    Url,
    Path,
    IpAddr,
    GitSha,
}

/// Actionable token on the `OutputCanvas` with a short alphabetic jump label (`[a]..[z]`, `ECO-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasHint {
    pub label: String,
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
    pub text: String,
    pub kind: CanvasHintKind,
}

impl VtParser {
    /// Returns all explicit `OSC 8` hyperlinks recorded in the stream (`VAL-03`).
    pub fn osc8_links(&self) -> &[Osc8Hyperlink] {
        &self.osc8_links
    }

    /// Scans the currently visible viewport grid for `https://`, `http://`, and `file://` URLs (`VAL-03`).
    pub fn detect_visible_urls(&self) -> Vec<DetectedUrl> {
        let (rows, cols) = self.parser.screen().size();
        let mut urls = Vec::new();
        for r in 0..rows {
            let mut row_str = String::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .map(|cell| cell.contents())
                    .unwrap_or_default();
                if ch.is_empty() {
                    row_str.push(' ');
                } else {
                    row_str.push_str(ch);
                }
            }
            for scheme in ["https://", "http://", "file://"] {
                let mut search_from = 0usize;
                while let Some(rel_idx) = row_str[search_from..].find(scheme) {
                    let start_byte = search_from + rel_idx;
                    let after = &row_str[start_byte..];
                    let raw_len = after
                        .find(|ch: char| {
                            ch.is_whitespace()
                                || matches!(ch, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']')
                        })
                        .unwrap_or(after.len());
                    let trimmed =
                        after[..raw_len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                    if trimmed.len() > scheme.len() {
                        let start_col = row_str[..start_byte].chars().count() as u16;
                        let url_cols = trimmed.chars().count() as u16;
                        urls.push(DetectedUrl {
                            row: r,
                            start_col,
                            end_col: start_col.saturating_add(url_cols).saturating_sub(1),
                            url: trimmed.to_string(),
                        });
                    }
                    search_from = start_byte + raw_len.max(scheme.len());
                }
            }
        }
        urls
    }

    /// Returns the URL at viewport cell `(row, col)` if present (`VAL-03`).
    pub fn url_at(&self, row: u16, col: u16) -> Option<String> {
        self.detect_visible_urls()
            .into_iter()
            .find(|u| u.row == row && col >= u.start_col && col <= u.end_col)
            .map(|u| u.url)
    }

    /// Scans the visible `OutputCanvas` grid for actionable tokens (URLs, file paths, IPv4 addresses,
    /// and Git commit SHAs) and assigns keyboard jump labels (`a`..`z`, `aa`..`az`) (`ECO-01`).
    pub fn detect_visible_hints(&self) -> Vec<CanvasHint> {
        let (rows, cols) = self.parser.screen().size();
        let mut raw_hints: Vec<(u16, u16, u16, String, CanvasHintKind)> = Vec::new();

        for r in 0..rows {
            let mut row_str = String::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .map(|cell| cell.contents())
                    .unwrap_or_default();
                if ch.is_empty() {
                    row_str.push(' ');
                } else {
                    row_str.push_str(ch);
                }
            }

            let mut byte_idx = 0usize;
            for token in row_str.split_whitespace() {
                let Some(rel) = row_str[byte_idx..].find(token) else {
                    continue;
                };
                let start_byte = byte_idx + rel;
                byte_idx = start_byte + token.len();

                let clean = token
                    .trim_start_matches(['(', '[', '<', '"', '\''])
                    .trim_end_matches([')', ']', '>', '"', '\'', ',', ';', '.']);
                if clean.len() < 4 {
                    continue;
                }
                let trim_prefix_bytes = token.find(clean).unwrap_or(0);
                let clean_start_byte = start_byte + trim_prefix_bytes;
                let start_col = row_str[..clean_start_byte].chars().count() as u16;
                let width_cols = clean.chars().count() as u16;
                let end_col = start_col.saturating_add(width_cols).saturating_sub(1);

                let kind_opt = if clean.starts_with("https://")
                    || clean.starts_with("http://")
                    || clean.starts_with("file://")
                {
                    Some(CanvasHintKind::Url)
                } else if is_ipv4_token(clean) {
                    Some(CanvasHintKind::IpAddr)
                } else if is_path_token(clean) {
                    Some(CanvasHintKind::Path)
                } else if is_git_sha_token(clean) {
                    Some(CanvasHintKind::GitSha)
                } else {
                    None
                };

                if let Some(kind) = kind_opt {
                    raw_hints.push((r, start_col, end_col, clean.to_string(), kind));
                }
            }
        }

        raw_hints
            .into_iter()
            .enumerate()
            .map(|(idx, (row, start_col, end_col, text, kind))| CanvasHint {
                label: hint_index_to_label(idx),
                row,
                start_col,
                end_col,
                text,
                kind,
            })
            .collect()
    }

    /// Searches the currently visible viewport grid for case-insensitive occurrences of `query` (`VAL-02`).
    pub fn search_canvas(&self, query: &str) -> Vec<CanvasSearchMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_ascii_lowercase();
        let needle_cols = needle.chars().count() as u16;
        if needle_cols == 0 {
            return Vec::new();
        }
        let (rows, cols) = self.parser.screen().size();
        let mut matches = Vec::new();
        for r in 0..rows {
            let mut row_chars: Vec<char> = Vec::with_capacity(usize::from(cols));
            for c in 0..cols {
                let ch = self
                    .parser
                    .screen()
                    .cell(r, c)
                    .and_then(|cell| cell.contents().chars().next())
                    .unwrap_or(' ');
                row_chars.push(ch.to_ascii_lowercase());
            }
            let hay: String = row_chars.iter().collect();
            let mut from = 0usize;
            while let Some(pos) = hay[from..].find(&needle) {
                let byte_idx = from + pos;
                let start_col = hay[..byte_idx].chars().count() as u16;
                matches.push(CanvasSearchMatch {
                    row: r,
                    start_col,
                    end_col: start_col.saturating_add(needle_cols).saturating_sub(1),
                });
                from = byte_idx + needle.len().max(1);
            }
        }
        matches
    }
}

fn hint_index_to_label(idx: usize) -> String {
    if idx < 26 {
        ((b'a' + idx as u8) as char).to_string()
    } else {
        let first = (b'a' + ((idx / 26 - 1) % 26) as u8) as char;
        let second = (b'a' + (idx % 26) as u8) as char;
        format!("{first}{second}")
    }
}

fn is_ipv4_token(token: &str) -> bool {
    let host = token.split(':').next().unwrap_or(token);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.parse::<u8>().is_ok())
}

fn is_path_token(token: &str) -> bool {
    let path_part = token.split(':').next().unwrap_or(token);
    if path_part.starts_with('/')
        || path_part.starts_with("./")
        || path_part.starts_with("../")
        || path_part.starts_with("~/")
    {
        return path_part.len() > 2
            && path_part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~'));
    }
    if path_part.contains('/') && path_part.len() >= 4 {
        let valid_chars = path_part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'));
        let has_alpha = path_part.chars().any(|c| c.is_ascii_alphabetic());
        return valid_chars && has_alpha;
    }
    false
}

fn is_git_sha_token(token: &str) -> bool {
    if !(7..=40).contains(&token.len()) {
        return false;
    }
    let all_hex = token.chars().all(|c| c.is_ascii_hexdigit());
    let has_digit = token.chars().any(|c| c.is_ascii_digit());
    let has_hex_alpha = token
        .chars()
        .any(|c| matches!(c, 'a'..='f' | 'A'..='F'));
    all_hex && has_digit && has_hex_alpha
}
