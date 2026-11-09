//! Pure word and character motion algorithms for Copy Mode (`COPY-02`, `ARCH-R04`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

impl CharClass {
    pub fn of(c: char) -> Self {
        if c.is_whitespace() {
            CharClass::Whitespace
        } else if c.is_alphanumeric() || c == '_' {
            CharClass::Word
        } else {
            CharClass::Punctuation
        }
    }
}

/// Finds the column index of the next word start (`w` motion in Vim).
/// If `col` is at or after the last word on this line, returns `None`.
pub fn next_word_start(text: &str, col: u16) -> Option<u16> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return None;
    }
    let cur_idx = chars
        .iter()
        .position(|&(byte_idx, _)| byte_idx as u16 >= col)
        .unwrap_or(chars.len());

    if cur_idx >= chars.len() {
        return None;
    }

    let initial_class = CharClass::of(chars[cur_idx].1);
    let mut i = cur_idx;

    // If starting on a word or punctuation, advance past this run
    if initial_class != CharClass::Whitespace {
        while i < chars.len() && CharClass::of(chars[i].1) == initial_class {
            i += 1;
        }
    }

    // Now advance past any whitespace
    while i < chars.len() && CharClass::of(chars[i].1) == CharClass::Whitespace {
        i += 1;
    }

    if i < chars.len() {
        Some(chars[i].0 as u16)
    } else {
        None
    }
}

/// Finds the column index of the previous word start (`b` motion in Vim).
pub fn prev_word_start(text: &str, col: u16) -> Option<u16> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() || col == 0 {
        return None;
    }
    let cur_idx = chars
        .iter()
        .position(|&(byte_idx, _)| byte_idx as u16 >= col)
        .unwrap_or(chars.len());

    if cur_idx == 0 {
        return None;
    }

    let mut i = cur_idx.saturating_sub(1);
    // Skip any whitespace before current position
    while i > 0 && CharClass::of(chars[i].1) == CharClass::Whitespace {
        i -= 1;
    }

    let target_class = CharClass::of(chars[i].1);
    if target_class == CharClass::Whitespace {
        return None;
    }

    // Scan backwards to the start of this class run
    while i > 0 && CharClass::of(chars[i - 1].1) == target_class {
        i -= 1;
    }

    Some(chars[i].0 as u16)
}

/// Finds the column index of the word end (`e` motion in Vim).
pub fn word_end(text: &str, col: u16) -> Option<u16> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return None;
    }
    let cur_idx = chars
        .iter()
        .position(|&(byte_idx, _)| byte_idx as u16 >= col)
        .unwrap_or(chars.len());

    if cur_idx >= chars.len() {
        return None;
    }

    let mut i = cur_idx;
    // If on whitespace, advance to non-whitespace
    while i < chars.len() && CharClass::of(chars[i].1) == CharClass::Whitespace {
        i += 1;
    }
    if i >= chars.len() {
        return None;
    }

    // If starting on the end of a word, step forward by one
    let target_class = CharClass::of(chars[i].1);
    if i == cur_idx && i + 1 < chars.len() && CharClass::of(chars[i + 1].1) != target_class {
        i += 1;
        while i < chars.len() && CharClass::of(chars[i].1) == CharClass::Whitespace {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }
    }

    let target_class = CharClass::of(chars[i].1);
    while i + 1 < chars.len() && CharClass::of(chars[i + 1].1) == target_class {
        i += 1;
    }

    Some(chars[i].0 as u16)
}

/// Finds the start and inclusive end column of the word enclosing `col`.
pub fn word_bounds_at(text: &str, col: u16) -> (u16, u16) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return (0, 0);
    }
    let cur_idx = chars
        .iter()
        .position(|&(byte_idx, _)| byte_idx as u16 >= col)
        .unwrap_or(chars.len().saturating_sub(1));

    let target_class = CharClass::of(chars[cur_idx].1);

    let mut start = cur_idx;
    while start > 0 && CharClass::of(chars[start - 1].1) == target_class {
        start -= 1;
    }

    let mut end = cur_idx;
    while end + 1 < chars.len() && CharClass::of(chars[end + 1].1) == target_class {
        end += 1;
    }

    let start_col = chars[start].0 as u16;
    let end_col = chars[end].0 as u16;
    (start_col, end_col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_motions_basic() {
        let text = "alpha beta, gamma_x delta";
        assert_eq!(next_word_start(text, 0), Some(6));
        assert_eq!(word_end(text, 0), Some(4));
        assert_eq!(word_bounds_at(text, 2), (0, 4));

        // Punctuation is separate word class
        assert_eq!(next_word_start(text, 6), Some(10)); // ','
        assert_eq!(next_word_start(text, 10), Some(12)); // 'g' in gamma_x
        assert_eq!(prev_word_start(text, 12), Some(10));
        assert_eq!(prev_word_start(text, 10), Some(6));
    }

    #[test]
    fn test_cjk_word_bounds() {
        let text = "世界 hello";
        let (s, e) = word_bounds_at(text, 0);
        assert_eq!(s, 0);
        // "世界" is 6 UTF-8 bytes (0..=3 start of second char)
        assert!(e >= 3);
    }
}
