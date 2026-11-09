//! Search state and match navigation for Copy Mode (`COPY-02`, `ARCH-R04`).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDir {
    #[default]
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopySearchState {
    pub dir: SearchDir,
    pub query: String,
    pub editing: bool,
    pub origin_cursor: (u16, u16),
}

impl CopySearchState {
    pub fn new(dir: SearchDir, origin_cursor: (u16, u16)) -> Self {
        Self {
            dir,
            query: String::new(),
            editing: true,
            origin_cursor,
        }
    }
}

/// Finds the next match relative to current cursor row and column.
pub fn find_next_match(
    matches: &[(usize, u16, u16)],
    cur_abs_row: usize,
    cur_col: u16,
    dir: SearchDir,
) -> Option<(usize, u16, u16)> {
    if matches.is_empty() {
        return None;
    }

    match dir {
        SearchDir::Forward => {
            // Find first match strictly after cursor position (or wrap to beginning)
            matches
                .iter()
                .find(|&&(r, c, _)| (r > cur_abs_row) || (r == cur_abs_row && c > cur_col))
                .copied()
                .or_else(|| matches.first().copied())
        }
        SearchDir::Backward => {
            // Find last match strictly before cursor position (or wrap to end)
            matches
                .iter()
                .rev()
                .find(|&&(r, c, _)| (r < cur_abs_row) || (r == cur_abs_row && c < cur_col))
                .copied()
                .or_else(|| matches.last().copied())
        }
    }
}
