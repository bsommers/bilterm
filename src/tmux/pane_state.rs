//! Screen state of a remote tmux pane, so a hydrate restores more than text: alternate screen,
//! cursor position and visibility, application cursor keys and mouse reporting.
//!
//! `capture-pane` only returns cells. Without the state a re-attach while `vim` runs leaves the
//! local pane on the primary screen with the cursor after the last line, and the application's
//! later redraws land in the wrong place.

use std::collections::BTreeMap;

use super::link::{HYDRATE_HISTORY_LINES, MAPPED_PANES_CAP};

/// What `display-message -p` reports for one pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaneState {
    pub alternate: bool,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub width: u16,
    pub height: u16,
    pub cursor_visible: bool,
    pub app_cursor_keys: bool,
    pub mouse_standard: bool,
    pub mouse_button: bool,
    pub mouse_any: bool,
    pub mouse_sgr: bool,
}

const FIELDS: [&str; 11] = [
    "alternate_on",
    "cursor_x",
    "cursor_y",
    "pane_width",
    "pane_height",
    "cursor_flag",
    "keypad_cursor_flag",
    "mouse_standard_flag",
    "mouse_button_flag",
    "mouse_any_flag",
    "mouse_sgr_flag",
];

/// Default first word of the state reply line (the link adds a per-attach nonce, see
/// [`state_marker`]). The reply identifies itself by this word instead of by its position among
/// the reply blocks.
pub const STATE_MARKER: &str = "bilterm-state";

/// Marker for one attach: screen text captured from a pane cannot be mistaken for a state reply.
pub fn state_marker(nonce: u64) -> String {
    format!("{STATE_MARKER}-{nonce:x}")
}

/// One `display-message -p` for every pane of the session: `MARKER [%id f1 .. f11][%id ..]`.
/// It names no target, so it cannot fail because a pane is gone.
pub fn pane_states_command(marker: &str) -> String {
    let fmt: Vec<String> = FIELDS.iter().map(|f| format!("#{{{f}}}")).collect();
    format!("display-message -p \"{marker} #{{W:#{{P:[#{{pane_id}} {}]}}}}\"\n", fmt.join(" "))
}

fn parse_fields(text: &str) -> Option<PaneState> {
    let nums: Vec<u16> = text
        .split_whitespace()
        .map(|t| t.parse::<u16>().ok())
        .collect::<Option<Vec<u16>>>()?;
    if nums.len() != FIELDS.len() {
        return None;
    }
    Some(PaneState {
        alternate: nums[0] != 0,
        cursor_x: nums[1],
        cursor_y: nums[2],
        width: nums[3],
        height: nums[4],
        cursor_visible: nums[5] != 0,
        app_cursor_keys: nums[6] != 0,
        mouse_standard: nums[7] != 0,
        mouse_button: nums[8] != 0,
        mouse_any: nums[9] != 0,
        mouse_sgr: nums[10] != 0,
    })
}

/// Parses a state reply line; malformed groups are skipped and the count is capped (the line is
/// remote-controlled). `None` when the line does not start with `marker`.
pub fn parse_pane_states(marker: &str, line: &str) -> Option<BTreeMap<usize, PaneState>> {
    let rest = line.strip_prefix(marker)?.strip_prefix(' ').unwrap_or("");
    let mut out = BTreeMap::new();
    for group in rest.split('[').skip(1).take(4 * MAPPED_PANES_CAP) {
        let Some((body, _)) = group.split_once(']') else { continue };
        let Some((id, fields)) = body.split_once(' ') else { continue };
        if let (Some(id), Some(st)) = (id.strip_prefix('%').and_then(|n| n.parse().ok()), parse_fields(fields))
            && out.len() < MAPPED_PANES_CAP
        {
            out.insert(id, st);
        }
    }
    Some(out)
}

/// Bytes that rebuild a pane from a capture reply plus its state: full reset, alternate screen if
/// the remote is on it, every captured line (the last `height` lines are the visible screen, so
/// trailing blank lines are kept), then the cursor and the input modes.
///
/// Lines beyond history plus one screen are dropped from the front (malformed or oversized reply).
pub fn render_capture_with_state(lines: &[String], st: &PaneState) -> Vec<u8> {
    let keep = HYDRATE_HISTORY_LINES + usize::from(st.height.max(1));
    let lines = &lines[lines.len().saturating_sub(keep)..];
    let mut out = Vec::with_capacity(lines.iter().map(|l| l.len() + 2).sum::<usize>() + 64);
    out.extend_from_slice(b"\x1bc\x1b[3J");
    if st.alternate {
        out.extend_from_slice(b"\x1b[?1049h\x1b[H\x1b[2J");
    }
    out.extend_from_slice(lines.join("\r\n").as_bytes());
    out.extend_from_slice(b"\x1b[0m");
    out.extend_from_slice(
        format!("\x1b[{};{}H", u32::from(st.cursor_y) + 1, u32::from(st.cursor_x) + 1).as_bytes(),
    );
    for (on, seq) in [
        (!st.cursor_visible, "\x1b[?25l"),
        (st.app_cursor_keys, "\x1b[?1h"),
        (st.mouse_standard, "\x1b[?1000h"),
        (st.mouse_button, "\x1b[?1002h"),
        (st.mouse_any, "\x1b[?1003h"),
        (st.mouse_sgr, "\x1b[?1006h"),
    ] {
        if on {
            out.extend_from_slice(seq.as_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_line_round_trips_and_rejects_garbage() {
        let m = state_marker(0xabc);
        assert_eq!(m, "bilterm-state-abc");
        let cmd = pane_states_command(&m);
        assert!(cmd.starts_with("display-message -p \"bilterm-state-abc #{W:#{P:[#{pane_id} #{alternate_on} #{cursor_x} "), "{cmd}");
        assert!(cmd.ends_with("#{mouse_sgr_flag}]}}\"\n"), "{cmd}");
        let got = parse_pane_states(&m, "bilterm-state-abc [%0 1 8 0 78 42 1 0 0 0 0 1][%7 0 0 3 80 24 0 1 0 0 0 0]").unwrap();
        assert_eq!(got.len(), 2);
        let st = got[&0];
        assert!(st.alternate && st.cursor_visible && st.mouse_sgr && !st.mouse_any);
        assert_eq!((st.cursor_x, st.cursor_y, st.width, st.height), (8, 0, 78, 42));
        assert!(got[&7].app_cursor_keys && !got[&7].cursor_visible);
        // Wrong marker (for example screen text of another attach) is not a state line.
        assert!(parse_pane_states(&m, "bilterm-state [%0 1 8 0 78 42 1 0 0 0 0 1]").is_none());
        for bad in ["", " [", " [%x 1 2]", " [%1 1 8 0 78 42 1 0 0 0 0]", " [%1 99999999 0 0 0 0 0 0 0 0 0 0]", " %1 1 8 0 78 42 1 0 0 0 0 1"] {
            assert_eq!(parse_pane_states(&m, &format!("{m}{bad}")).unwrap().len(), 0, "{bad}");
        }
    }

    #[test]
    fn render_restores_alt_screen_cursor_and_modes() {
        let st = PaneState { alternate: true, cursor_x: 8, cursor_y: 0, width: 20, height: 2, mouse_sgr: true, ..Default::default() };
        let out = String::from_utf8(render_capture_with_state(&["HELLO".into(), "~".into()], &st)).unwrap();
        assert!(out.starts_with("\x1bc\x1b[3J\x1b[?1049h"));
        assert!(out.contains("HELLO\r\n~\x1b[0m\x1b[1;9H"));
        assert!(out.ends_with("\x1b[?25l\x1b[?1006h"));
    }
}
