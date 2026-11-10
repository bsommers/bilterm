//! Parser for `:coords [verbose|compact|daemon|off|locate <coord>|copy|cycle]` (`COORD-04`, Phase 71).

use super::ColonCommand;
use crate::action::Action;
use crate::wm::coordinates::{CoordDisplayStyle, CoordsOp};

pub const COORDS_USAGE: &str = "Usage: :coords [verbose|compact|daemon|off|locate <coord>|copy|cycle]";

/// Parses the arguments after `:coords`.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        [] | ["show"] => CoordsOp::Show,
        ["copy"] => CoordsOp::Copy,
        ["cycle" | "next"] => CoordsOp::Cycle,
        // Bare `:coords locate` (palette prompt entry) parses; the empty coord errors at execute time.
        ["locate" | "goto", rest @ ..] => CoordsOp::Locate(rest.join(" ")),
        ["style", s] | [s] => match CoordDisplayStyle::from_name(s) {
            Some(style) => CoordsOp::Style(style),
            None => return Err(COORDS_USAGE.to_string()),
        },
        _ => return Err(COORDS_USAGE.to_string()),
    };
    Ok(Action::Coords(op))
}

/// Canonical colon form of a coords op.
pub fn colon_form(op: &CoordsOp) -> String {
    match op {
        CoordsOp::Show => ":coords".to_string(),
        CoordsOp::Style(s) => format!(":coords {}", s.name()),
        CoordsOp::Locate(c) => format!(":coords locate {c}"),
        CoordsOp::Copy => ":coords copy".to_string(),
        CoordsOp::Cycle => ":coords cycle".to_string(),
    }
}
