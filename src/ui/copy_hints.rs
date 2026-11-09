//! State-driven status hint line for Copy Mode (`COPY-02`, `ARCH-R04`).

use crate::pane::{CopyModeState, CopyOrigin, CopySelectionKind};

/// Formats the single-line context-sensitive status hint for the active Copy Mode state.
pub fn copy_hint_line(state: &CopyModeState, scope_label: &str, width: u16) -> String {
    if let Some(ref search) = state.search_state
        && search.editing
    {
        let prefix = match search.dir {
            crate::pane::copy_search::SearchDir::Forward => "/",
            crate::pane::copy_search::SearchDir::Backward => "?",
        };
        return format!("FIND: {prefix}{}_  [Enter go | Esc cancel]", search.query);
    }

    if state.origin == CopyOrigin::Mouse {
        if width < 80 {
            return "MOUSE: [y copy | Esc clear | keys -> app]".to_string();
        }
        return format!(
            "COPY [{scope_label}]: MOUSE SELECT - [y/Enter copy | Esc clear | any other key -> app]"
        );
    }

    if width < 80 {
        if state.has_selection() {
            return "v sel  y copy  o swap  Esc clear".to_string();
        }
        return "v sel  y copy  / find  Esc exit".to_string();
    }

    match state.selection_kind {
        CopySelectionKind::None => {
            format!(
                "COPY [{scope_label}]  v char  V line  ^v block  w/b/e word  / find  y copy  x -> prompt  Esc exit"
            )
        }
        CopySelectionKind::Char => {
            format!(
                "COPY [{scope_label}: CHAR]  Arrows/hjkl extend  o swap  y copy  x -> prompt  Esc clear"
            )
        }
        CopySelectionKind::Line => {
            format!(
                "COPY [{scope_label}: LINE]  Up/Down extend  y copy  x -> prompt  Esc clear"
            )
        }
        CopySelectionKind::Block => {
            format!(
                "COPY [{scope_label}: BLOCK]  Arrows extend  y copy  x -> prompt  Esc clear"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_line_states() {
        let mut state = CopyModeState {
            active: true,
            ..Default::default()
        };

        let idle = copy_hint_line(&state, "history", 120);
        assert!(idle.contains("COPY [history]"));
        assert!(idle.contains("v char"));

        state.selection_kind = CopySelectionKind::Char;
        state.anchor = Some((1, 1));
        let char_hint = copy_hint_line(&state, "history", 120);
        assert!(char_hint.contains("CHAR"));
        assert!(char_hint.contains("o swap"));

        let narrow = copy_hint_line(&state, "history", 60);
        assert!(narrow.contains("Esc clear"));
    }
}
