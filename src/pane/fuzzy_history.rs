use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A single fuzzy-matched command history item with score and matched character indices (`PANE-04`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyHistoryMatch {
    pub history_index: usize,
    pub command: String,
    pub score: u32,
    pub matched_indices: Vec<u32>,
}

/// Interactive `Ctrl+R` fuzzy history search state inside a pane's `FloatingCommandPrompt`.
#[derive(Clone, Debug, Default)]
pub struct FuzzyHistorySearchState {
    pub active: bool,
    pub query: String,
    pub selected: usize,
    pub matches: Vec<FuzzyHistoryMatch>,
}

impl FuzzyHistorySearchState {
    /// Recomputes fuzzy matches against `history` (most recent first when query is empty,
    /// or ranked by `nucleo-matcher` score with recency tie-breaking when query is non-empty).
    pub fn recompute(&mut self, history: &[String]) {
        self.matches.clear();
        if self.query.trim().is_empty() {
            for (idx, cmd) in history.iter().enumerate().rev() {
                self.matches.push(FuzzyHistoryMatch {
                    history_index: idx,
                    command: cmd.clone(),
                    score: 0,
                    matched_indices: Vec::new(),
                });
            }
        } else {
            let mut matcher = Matcher::new(Config::DEFAULT);
            let q_lower = self.query.to_lowercase();
            let pattern = Pattern::parse(&q_lower, CaseMatching::Ignore, Normalization::Smart);
            let mut char_buf = Vec::new();

            for (idx, cmd) in history.iter().enumerate() {
                char_buf.clear();
                let mut indices = Vec::new();
                let matched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let haystack = Utf32Str::new(cmd, &mut char_buf);
                    pattern.indices(haystack, &mut matcher, &mut indices)
                }))
                .ok()
                .flatten();
                if let Some(score) = matched {
                    indices.sort_unstable();
                    indices.dedup();
                    self.matches.push(FuzzyHistoryMatch {
                        history_index: idx,
                        command: cmd.clone(),
                        score,
                        matched_indices: indices,
                    });
                } else if cmd.to_lowercase().contains(&q_lower) {
                    self.matches.push(FuzzyHistoryMatch {
                        history_index: idx,
                        command: cmd.clone(),
                        score: 10,
                        matched_indices: Vec::new(),
                    });
                }
            }

            // Sort by descending score, then by most recent history_index
            self.matches.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| b.history_index.cmp(&a.history_index))
            });
        }

        if self.matches.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.matches.len() - 1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            self.selected = if self.selected == 0 {
                self.matches.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn current_match(&self) -> Option<&FuzzyHistoryMatch> {
        self.matches.get(self.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_history_ranks_matches_and_tracks_matched_indices() {
        let history = vec![
            "cargo check".to_string(),
            "git status -sb".to_string(),
            "cargo test --all-targets".to_string(),
        ];
        let mut search = FuzzyHistorySearchState {
            active: true,
            query: "ctall".to_string(),
            selected: 0,
            matches: Vec::new(),
        };
        search.recompute(&history);
        assert!(!search.matches.is_empty());
        assert_eq!(search.matches[0].command, "cargo test --all-targets");
        assert!(!search.matches[0].matched_indices.is_empty());
    }
}
