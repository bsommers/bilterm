//! Parser for the keyboard-ownership forms of `:focus` (`FOCUS-01`): `:focus cycle [next|prev]`,
//! `:focus pane` and `:focus <window>`. Pane ids, directions and `next`/`prev` (deck shuffle) are
//! matched by the caller first.

use super::ColonCommand;
use crate::wm::keyboard_focus::{FocusOp, KeyboardOwner};

pub const FOCUS_USAGE: &str = "Usage: :focus <pane_id|left|right|up|down|h|j|k|l|w|a|s|d|next|prev|cycle [next|prev]|pane|agents-list|agents|sessions|clipboard|traffic|commands|script|fonts|daemon|hud|drawer|console>";

/// Parses `head` (the word after `:focus`) and the remaining arguments.
pub(super) fn parse(head: &str, rest: &[&str]) -> Result<ColonCommand, String> {
    let op = match (head, rest) {
        ("cycle", [] | ["next" | "forward"]) => FocusOp::Next,
        ("cycle", ["prev" | "previous" | "back"]) => FocusOp::Prev,
        ("cycle", _) => return Err(FOCUS_USAGE.to_string()),
        (word, []) => match word.parse::<usize>() {
            Ok(id) => return Ok(ColonCommand::FocusPane(id)),
            Err(_) => match KeyboardOwner::parse(word) {
                Some(KeyboardOwner::Pane) => FocusOp::Pane,
                Some(window) => FocusOp::Window(window),
                None => return Err(FOCUS_USAGE.to_string()),
            },
        },
        // `:focus 3 trailing words` keeps focusing pane 3, as before.
        (word, _) => match word.parse::<usize>() {
            Ok(id) => return Ok(ColonCommand::FocusPane(id)),
            Err(_) => return Err(FOCUS_USAGE.to_string()),
        },
    };
    Ok(ColonCommand::KeyboardFocus(op))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_focus_op_round_trips_through_its_colon_form() {
        let ops = [FocusOp::Next, FocusOp::Prev, FocusOp::Pane]
            .into_iter()
            .chain(KeyboardOwner::WINDOWS.into_iter().map(FocusOp::Window));
        for op in ops {
            let line = op.colon_form();
            let parsed = crate::colon_command::parse_colon_command(&line);
            assert_eq!(parsed, Some(Ok(ColonCommand::KeyboardFocus(op))), "{line}");
        }
    }

    #[test]
    fn pane_ids_and_garbage_keep_their_old_meaning() {
        assert_eq!(parse("3", &[]), Ok(ColonCommand::FocusPane(3)));
        assert!(parse("nonsense", &[]).is_err());
        assert!(parse("cycle", &["sideways"]).is_err());
    }
}
