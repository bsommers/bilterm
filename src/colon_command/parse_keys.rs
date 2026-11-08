//! Parsers for the Phase 63 keyboard commands (`KEY-01..03`, `UNI-R01`).
//!
//! Syntax: `:wmode [on|off|toggle|focus|resize|move]`, `:menu [open [category]]`,
//! `:keys [list|check|explain <chord>|profile [name]]`. Bare `:menu` / `:menu on|off` stay the
//! pre-existing menu-bar visibility toggle (`parse_visual.rs`); only `:menu open ...` is claimed here.

use super::ColonCommand;
use crate::action::{KeysOp, WinSub, WindowModeOp};

pub const WMODE_USAGE: &str = "Usage: :wmode [on|off|toggle|focus|resize|move]";
pub const MENU_USAGE: &str = "Usage: :menu open [category]";
pub const KEYS_USAGE: &str = "Usage: :keys [list | check | explain <chord> | profile [name]]";

/// Parses `:wmode ...`, `:menu ...` and `:keys ...`; `None` when `head` is none of them.
pub fn parse_keys_command(head: &str, parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    match head {
        "wmode" => Some(parse_wmode(parts)),
        "menu" if parts.get(1) == Some(&"open") => Some(parse_menu(parts)),
        "keys" => Some(parse_keys(parts)),
        _ => None,
    }
}

fn parse_wmode(parts: &[&str]) -> Result<ColonCommand, String> {
    let op = match (parts.get(1).copied(), parts.len()) {
        (None, 1) => WindowModeOp::Toggle,
        (Some("toggle"), 2) => WindowModeOp::Toggle,
        (Some("on" | "enter"), 2) => WindowModeOp::Enter,
        (Some("off" | "exit"), 2) => WindowModeOp::Exit,
        (Some("focus"), 2) => WindowModeOp::Sub(WinSub::Focus),
        (Some("resize"), 2) => WindowModeOp::Sub(WinSub::Resize),
        (Some("move"), 2) => WindowModeOp::Sub(WinSub::Move),
        _ => return Err(WMODE_USAGE.to_string()),
    };
    Ok(ColonCommand::WindowMode(op))
}

fn parse_menu(parts: &[&str]) -> Result<ColonCommand, String> {
    match (parts.get(1).copied(), parts.get(2), parts.len()) {
        (Some("open"), None, 2) => Ok(ColonCommand::MenuOpen { category: None }),
        (Some("open"), Some(n), 3) => match n.parse::<usize>() {
            Ok(c) => Ok(ColonCommand::MenuOpen { category: Some(c) }),
            Err(_) => Err(MENU_USAGE.to_string()),
        },
        _ => Err(MENU_USAGE.to_string()),
    }
}

fn parse_keys(parts: &[&str]) -> Result<ColonCommand, String> {
    let op = match (parts.get(1).copied(), parts.len()) {
        (None, 1) | (Some("list"), 2) => KeysOp::List,
        (Some("check"), 2) => KeysOp::Check,
        (Some("explain"), 3) => KeysOp::Explain(parts[2].to_string()),
        (Some("profile"), 2) => KeysOp::Profile(None),
        (Some("profile"), 3) => KeysOp::Profile(Some(parts[2].to_string())),
        _ => return Err(KEYS_USAGE.to_string()),
    };
    Ok(ColonCommand::Keys(op))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<Result<ColonCommand, String>> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        parse_keys_command(parts[0], &parts)
    }

    #[test]
    fn parses_wmode_menu_and_keys_forms() {
        use ColonCommand as C;
        assert_eq!(p("wmode"), Some(Ok(C::WindowMode(WindowModeOp::Toggle))));
        assert_eq!(p("wmode on"), Some(Ok(C::WindowMode(WindowModeOp::Enter))));
        assert_eq!(p("wmode off"), Some(Ok(C::WindowMode(WindowModeOp::Exit))));
        assert_eq!(p("wmode resize"), Some(Ok(C::WindowMode(WindowModeOp::Sub(WinSub::Resize)))));
        assert!(matches!(p("wmode bogus"), Some(Err(_))));
        assert_eq!(p("menu"), None, "bare :menu stays the menu-bar visibility toggle");
        assert_eq!(p("menu off"), None);
        assert_eq!(p("menu open"), Some(Ok(C::MenuOpen { category: None })));
        assert_eq!(p("menu open 3"), Some(Ok(C::MenuOpen { category: Some(3) })));
        assert!(matches!(p("menu open x"), Some(Err(_))));
        assert_eq!(p("keys"), Some(Ok(C::Keys(KeysOp::List))));
        assert_eq!(p("keys check"), Some(Ok(C::Keys(KeysOp::Check))));
        assert_eq!(p("keys explain Primary+L"), Some(Ok(C::Keys(KeysOp::Explain("Primary+L".into())))));
        assert_eq!(p("keys profile"), Some(Ok(C::Keys(KeysOp::Profile(None)))));
        assert_eq!(p("keys profile mac"), Some(Ok(C::Keys(KeysOp::Profile(Some("mac".into()))))));
        assert!(matches!(p("keys explain"), Some(Err(_))));
        assert_eq!(p("keysx"), None);
    }

    #[test]
    fn colon_forms_round_trip() {
        for a in [
            ColonCommand::WindowMode(WindowModeOp::Enter),
            ColonCommand::WindowMode(WindowModeOp::Exit),
            ColonCommand::WindowMode(WindowModeOp::Toggle),
            ColonCommand::WindowMode(WindowModeOp::Sub(WinSub::Move)),
            ColonCommand::MenuOpen { category: Some(2) },
            ColonCommand::Keys(KeysOp::Check),
            ColonCommand::Keys(KeysOp::Profile(Some("linux".into()))),
        ] {
            let line = a.colon_form_or_summary();
            let parsed = super::super::parse_colon_command(&line).unwrap().unwrap();
            assert_eq!(parsed, a, "{line}");
        }
    }
}
