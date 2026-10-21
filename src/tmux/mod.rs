use std::collections::BTreeMap;

use crate::pane::{CompositePane, PaneId};
use crate::wm::WindowManager;

/// Outer `$TMUX` environment metadata (`TMUX-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TmuxEnv {
    pub socket_path: String,
    pub server_pid: Option<u32>,
    pub session_id: String,
}

impl TmuxEnv {
    /// Parses a `$TMUX` value (`<socket_path>,<pid>,<session_id>`).
    pub fn from_env_str(val: &str) -> Option<Self> {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut parts = trimmed.splitn(3, ',');
        let socket_path = parts.next()?.to_string();
        let server_pid = parts.next().and_then(|s| s.parse::<u32>().ok());
        let session_id = parts.next().unwrap_or("0").to_string();
        Some(Self {
            socket_path,
            server_pid,
            session_id,
        })
    }

    /// Detects whether Bilterm is running inside an outer `tmux` session via `$TMUX`.
    pub fn detect() -> Option<Self> {
        std::env::var("TMUX")
            .ok()
            .and_then(|v| Self::from_env_str(&v))
    }

    pub fn status_badge(&self) -> String {
        format!("TMUX:${}", self.session_id)
    }
}

/// Wraps an escape sequence (such as `OSC 52` clipboard `\x1b]52;c;...\x07`) in a `DCS tmux;`
/// passthrough envelope (`\x1bPtmux;...\x1b\\`) with doubled inner `\x1b` bytes (`TMUX-01`).
pub fn wrap_tmux_dcs_passthrough(seq: &str) -> String {
    let doubled_esc = seq.replace('\x1b', "\x1b\x1b");
    format!("\x1bPtmux;{doubled_esc}\x1b\\")
}

/// Parsed asynchronous event from a `tmux -CC` Control Mode stream (`TMUX-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TmuxControlEvent {
    /// `%output %<pane_id> <octal_escaped_bytes>`
    Output { pane_id: usize, data: Vec<u8> },
    /// `%window-add @<window_id>`
    WindowAdd { window_id: usize },
    /// `%layout-change @<window_id> <layout>`
    LayoutChange { window_id: usize, layout: String },
    /// `%session-changed $<session_id> <name>`
    SessionChanged { session_id: usize, name: String },
    /// `%begin <timestamp> <cmd_num> <flags>`
    BeginBlock,
    /// `%end <timestamp> <cmd_num> <flags>`
    EndBlock,
    /// Unrecognized line or command output line.
    Other(String),
}

/// Parses a single line emitted by `tmux -CC` Control Mode (`TMUX-02`).
pub fn parse_tmux_control_line(line: &str) -> TmuxControlEvent {
    let trimmed = line.trim_end_matches(&['\r', '\n'][..]);

    if let Some(rest) = trimmed.strip_prefix("%output ") {
        if let Some((pane_tok, payload)) = rest.split_once(' ') {
            if let Some(id_str) = pane_tok.strip_prefix('%') {
                if let Ok(pane_id) = id_str.parse::<usize>() {
                    return TmuxControlEvent::Output {
                        pane_id,
                        data: unescape_tmux_output(payload),
                    };
                }
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("%window-add ") {
        if let Some(id_str) = rest.trim().strip_prefix('@') {
            if let Ok(window_id) = id_str.parse::<usize>() {
                return TmuxControlEvent::WindowAdd { window_id };
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("%layout-change ") {
        let mut parts = rest.splitn(2, ' ');
        if let Some(win_tok) = parts.next() {
            if let Some(id_str) = win_tok.strip_prefix('@') {
                if let Ok(window_id) = id_str.parse::<usize>() {
                    let layout = parts.next().unwrap_or("").to_string();
                    return TmuxControlEvent::LayoutChange { window_id, layout };
                }
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("%session-changed ") {
        let mut parts = rest.splitn(2, ' ');
        if let Some(sess_tok) = parts.next() {
            if let Some(id_str) = sess_tok.strip_prefix('$') {
                if let Ok(session_id) = id_str.parse::<usize>() {
                    let name = parts.next().unwrap_or("").to_string();
                    return TmuxControlEvent::SessionChanged { session_id, name };
                }
            }
        }
    }

    if trimmed.starts_with("%begin ") {
        return TmuxControlEvent::BeginBlock;
    }
    if trimmed.starts_with("%end ") {
        return TmuxControlEvent::EndBlock;
    }

    TmuxControlEvent::Other(trimmed.to_string())
}

/// Unescapes a `tmux -CC` `%output` octal-escaped payload (`\ooo` where `o` is `0..=7`).
pub fn unescape_tmux_output(escaped: &str) -> Vec<u8> {
    let bytes = escaped.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && (b'0'..=b'7').contains(&bytes[i + 1])
            && (b'0'..=b'7').contains(&bytes[i + 2])
            && (b'0'..=b'7').contains(&bytes[i + 3])
        {
            let d0 = bytes[i + 1] - b'0';
            let d1 = bytes[i + 2] - b'0';
            let d2 = bytes[i + 3] - b'0';
            let val = (d0 << 6) | (d1 << 3) | d2;
            out.push(val);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// Formats a `tmux -CC` hex `send-keys` command (`send-keys -t %<id> -H <hex>`) so commands
/// submitted from Bilterm's `FloatingCommandPrompt` are sent cleanly to `tmux` (`TMUX-02`).
pub fn format_send_keys_hex(tmux_pane_id: usize, payload: &[u8]) -> String {
    let mut cmd = format!("send-keys -t %{tmux_pane_id} -H");
    for &b in payload {
        cmd.push_str(&format!(" {b:02x}"));
    }
    cmd.push('\n');
    cmd
}

/// Formats a `tmux -CC` `split-window` command (`split-window -h/-v -t %<id>`).
pub fn format_split_window(tmux_pane_id: usize, horizontal: bool) -> String {
    let flag = if horizontal { "-v" } else { "-h" };
    format!("split-window {flag} -t %{tmux_pane_id}\n")
}

/// Bridge synchronizing a `tmux -CC` Control Mode stream with Bilterm's `WindowManager` (`TMUX-02`).
#[derive(Clone, Debug, Default)]
pub struct TmuxControlBridge {
    /// Maps `tmux` `%pane_id` -> Bilterm `PaneId`.
    pub tmux_to_bilterm: BTreeMap<usize, PaneId>,
    pub active_session_name: Option<String>,
}

impl TmuxControlBridge {
    pub fn new() -> Self {
        let mut map = BTreeMap::new();
        map.insert(0, 1);
        Self {
            tmux_to_bilterm: map,
            active_session_name: None,
        }
    }

    /// Processes a line from `tmux -CC` stdout and updates the corresponding `CompositePane` in `wm`.
    pub fn handle_control_line(&mut self, wm: &mut WindowManager, line: &str) -> TmuxControlEvent {
        let ev = parse_tmux_control_line(line);
        match &ev {
            TmuxControlEvent::Output { pane_id, data } => {
                let bilterm_id = if let Some(&id) = self.tmux_to_bilterm.get(pane_id) {
                    id
                } else {
                    let new_id = wm.panes.keys().max().copied().unwrap_or(1) + 1;
                    let (rows, cols) = wm.active_pane().vt.size();
                    let mut new_pane = CompositePane::new_headless(new_id, rows + 1, cols);
                    new_pane.title = format!("TMUX %{pane_id}");
                    wm.panes.insert(new_id, new_pane);
                    wm.z_stack.push(new_id);
                    self.tmux_to_bilterm.insert(*pane_id, new_id);
                    new_id
                };
                if let Some(pane) = wm.panes.get_mut(&bilterm_id) {
                    pane.process_pty_output(data);
                }
            }
            TmuxControlEvent::SessionChanged { name, .. } => {
                self.active_session_name = Some(name.clone());
            }
            _ => {}
        }
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tmux_env_and_wraps_dcs_passthrough() {
        let env = TmuxEnv::from_env_str("/tmp/tmux-1000/default,4321,0").expect("valid $TMUX");
        assert_eq!(env.socket_path, "/tmp/tmux-1000/default");
        assert_eq!(env.server_pid, Some(4321));
        assert_eq!(env.session_id, "0");
        assert_eq!(env.status_badge(), "TMUX:$0");

        let osc52 = "\x1b]52;c;YmlsdGVybQ==\x07";
        let wrapped = wrap_tmux_dcs_passthrough(osc52);
        assert_eq!(wrapped, "\x1bPtmux;\x1b\x1b]52;c;YmlsdGVybQ==\x07\x1b\\");
    }

    #[test]
    fn unescapes_tmux_control_output_and_routes_to_composite_panes() {
        let mut wm = WindowManager::new_headless(16, 80);
        let mut bridge = TmuxControlBridge::new();

        // %output %0 with octal \033 (ESC), \015 (\r), \012 (\n)
        bridge.handle_control_line(&mut wm, "%output %0 \\033[32mHELLO_TMUX\\033[0m\\015\\012");
        assert!(wm.panes.get(&1).unwrap().vt.screen().contents().contains("HELLO_TMUX"));

        // %output %1 spawns/maps a second CompositePane automatically
        bridge.handle_control_line(&mut wm, "%output %1 SECOND_TMUX_PANE\\015\\012");
        assert_eq!(wm.panes.len(), 2);
        assert!(wm.panes.get(&2).unwrap().vt.screen().contents().contains("SECOND_TMUX_PANE"));

        let send_cmd = format_send_keys_hex(0, b"ls\n");
        assert_eq!(send_cmd, "send-keys -t %0 -H 6c 73 0a\n");
    }
}
