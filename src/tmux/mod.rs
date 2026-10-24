use std::collections::BTreeMap;

use crate::pane::PaneId;
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
    /// `%window-close @<window_id>` or `%unlinked-window-close @<window_id>`
    WindowClose { window_id: usize },
    /// `%window-pane-changed @<window_id> %<pane_id>`
    WindowPaneChanged { window_id: usize, pane_id: usize },
    /// `%layout-change @<window_id> <layout>`
    LayoutChange { window_id: usize, layout: String },
    /// `%session-changed $<session_id> <name>`
    SessionChanged { session_id: usize, name: String },
    /// `%exit [reason]`
    Exit { reason: Option<String> },
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

    if let Some(rest) = trimmed.strip_prefix("%output ")
        && let Some((pane_tok, payload)) = rest.split_once(' ')
            && let Some(id_str) = pane_tok.strip_prefix('%')
                && let Ok(pane_id) = id_str.parse::<usize>() {
                    return TmuxControlEvent::Output {
                        pane_id,
                        data: unescape_tmux_output(payload),
                    };
                }

    if let Some(rest) = trimmed.strip_prefix("%window-add ")
        && let Some(id_str) = rest.trim().strip_prefix('@')
            && let Ok(window_id) = id_str.parse::<usize>() {
                return TmuxControlEvent::WindowAdd { window_id };
            }

    if let Some(rest) = trimmed
        .strip_prefix("%window-close ")
        .or_else(|| trimmed.strip_prefix("%unlinked-window-close "))
        && let Some(id_str) = rest.trim().strip_prefix('@')
            && let Ok(window_id) = id_str.parse::<usize>() {
                return TmuxControlEvent::WindowClose { window_id };
            }

    if let Some(rest) = trimmed.strip_prefix("%window-pane-changed ") {
        let mut parts = rest.split_whitespace();
        if let (Some(win_tok), Some(pane_tok)) = (parts.next(), parts.next())
            && let (Some(w_str), Some(p_str)) =
                (win_tok.strip_prefix('@'), pane_tok.strip_prefix('%'))
            && let (Ok(window_id), Ok(pane_id)) = (w_str.parse::<usize>(), p_str.parse::<usize>())
        {
            return TmuxControlEvent::WindowPaneChanged { window_id, pane_id };
        }
    }

    if let Some(rest) = trimmed.strip_prefix("%layout-change ") {
        let mut parts = rest.splitn(2, ' ');
        if let Some(win_tok) = parts.next()
            && let Some(id_str) = win_tok.strip_prefix('@')
                && let Ok(window_id) = id_str.parse::<usize>() {
                    let layout = parts.next().unwrap_or("").to_string();
                    return TmuxControlEvent::LayoutChange { window_id, layout };
                }
    }

    if let Some(rest) = trimmed.strip_prefix("%session-changed ") {
        let mut parts = rest.splitn(2, ' ');
        if let Some(sess_tok) = parts.next()
            && let Some(id_str) = sess_tok.strip_prefix('$')
                && let Ok(session_id) = id_str.parse::<usize>() {
                    let name = parts.next().unwrap_or("").to_string();
                    return TmuxControlEvent::SessionChanged { session_id, name };
                }
    }

    if trimmed.starts_with("%begin ") {
        return TmuxControlEvent::BeginBlock;
    }
    if trimmed.starts_with("%end ") {
        return TmuxControlEvent::EndBlock;
    }
    if trimmed == "%exit" || trimmed.starts_with("%exit ") {
        let reason = trimmed
            .strip_prefix("%exit")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        return TmuxControlEvent::Exit { reason };
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

/// Formats a `tmux -CC` `refresh-client -C <cols>x<rows>` size notification (`TMUX-03`).
pub fn format_refresh_client_size(cols: u16, rows: u16) -> String {
    format!("refresh-client -C {}x{}\n", cols.max(1), rows.max(1))
}

/// Extracts all `%pane_id` integers from a `tmux` layout descriptor string
/// (e.g. `b25d,160x48,0,0{80x48,0,0,0,79x48,81,0,1}` -> `[0, 1]`, `TMUX-03`).
pub fn parse_tmux_layout_panes(layout: &str) -> Vec<usize> {
    let primary = layout.split_whitespace().next().unwrap_or(layout);
    let mut panes = Vec::new();
    let tokens: Vec<&str> = primary
        .split([',', '{', '}', '[', ']'])
        .filter(|s| !s.is_empty())
        .collect();
    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i];
        if tok.contains('x') && tok.chars().all(|c| c.is_ascii_digit() || c == 'x') {
            // Next 2 tokens are X and Y, 3rd token is pane_id (unless followed by '{' or '[' container)
            if i + 3 < tokens.len()
                && let (Ok(_x), Ok(_y), Ok(pane_id)) = (
                    tokens[i + 1].parse::<u32>(),
                    tokens[i + 2].parse::<u32>(),
                    tokens[i + 3].parse::<usize>(),
                )
            {
                if !panes.contains(&pane_id) {
                    panes.push(pane_id);
                }
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    panes
}

/// Bridge synchronizing a bi-directional `tmux -CC` Control Mode stream with Bilterm's `WindowManager` (`TMUX-02`, `TMUX-03`).
#[derive(Clone, Debug, Default)]
pub struct TmuxControlBridge {
    /// Maps `tmux` `%pane_id` -> Bilterm `PaneId`.
    pub tmux_to_bilterm: BTreeMap<usize, PaneId>,
    /// Reverse map from Bilterm `PaneId` -> `tmux` `%pane_id`.
    pub bilterm_to_tmux: BTreeMap<PaneId, usize>,
    pub active_session_name: Option<String>,
    pub outbound_commands: Vec<String>,
    pub exited: bool,
}

impl TmuxControlBridge {
    pub fn new() -> Self {
        let mut t2b = BTreeMap::new();
        let mut b2t = BTreeMap::new();
        t2b.insert(0, 1);
        b2t.insert(1, 0);
        Self {
            tmux_to_bilterm: t2b,
            bilterm_to_tmux: b2t,
            active_session_name: None,
            outbound_commands: Vec::new(),
            exited: false,
        }
    }

    fn ensure_tmux_pane(&mut self, wm: &mut WindowManager, tmux_pane_id: usize) -> PaneId {
        if let Some(&id) = self.tmux_to_bilterm.get(&tmux_pane_id) {
            return id;
        }
        let new_id = wm.insert_tmux_pane(tmux_pane_id);
        self.tmux_to_bilterm.insert(tmux_pane_id, new_id);
        self.bilterm_to_tmux.insert(new_id, tmux_pane_id);
        new_id
    }

    /// Processes a line from `tmux -CC` stdout and updates the corresponding `CompositePane` in `wm`.
    pub fn handle_control_line(&mut self, wm: &mut WindowManager, line: &str) -> TmuxControlEvent {
        let ev = parse_tmux_control_line(line);
        match &ev {
            TmuxControlEvent::Output { pane_id, data } => {
                let bilterm_id = self.ensure_tmux_pane(wm, *pane_id);
                if let Some(pane) = wm.pane_mut(bilterm_id) {
                    pane.process_pty_output(data);
                }
            }
            TmuxControlEvent::LayoutChange { layout, .. } => {
                for tmux_pane_id in parse_tmux_layout_panes(layout) {
                    self.ensure_tmux_pane(wm, tmux_pane_id);
                }
            }
            TmuxControlEvent::WindowPaneChanged { pane_id, .. } => {
                let bilterm_id = self.ensure_tmux_pane(wm, *pane_id);
                wm.focus_and_raise(bilterm_id);
            }
            TmuxControlEvent::SessionChanged { name, .. } => {
                self.active_session_name = Some(name.clone());
            }
            TmuxControlEvent::Exit { .. } => {
                self.exited = true;
            }
            _ => {}
        }
        ev
    }

    /// Routes a command submitted in Bilterm's `FloatingCommandPrompt` to the mapped `tmux` `%pane_id` (`TMUX-03`).
    pub fn route_prompt_submission(
        &mut self,
        bilterm_pane_id: PaneId,
        command_line: &str,
    ) -> Option<String> {
        let &tmux_id = self.bilterm_to_tmux.get(&bilterm_pane_id)?;
        let mut payload = command_line.as_bytes().to_vec();
        if !payload.ends_with(b"\n") {
            payload.push(b'\n');
        }
        let cmd = format_send_keys_hex(tmux_id, &payload);
        self.outbound_commands.push(cmd.clone());
        Some(cmd)
    }

    /// Queues a `split-window` command for the mapped `tmux` `%pane_id` (`TMUX-03`).
    pub fn queue_split(&mut self, bilterm_pane_id: PaneId, horizontal: bool) -> Option<String> {
        let &tmux_id = self.bilterm_to_tmux.get(&bilterm_pane_id)?;
        let cmd = format_split_window(tmux_id, horizontal);
        self.outbound_commands.push(cmd.clone());
        Some(cmd)
    }

    /// Queues a `refresh-client -C <cols>x<rows>` command (`TMUX-03`).
    pub fn queue_resize_client(&mut self, cols: u16, rows: u16) -> String {
        let cmd = format_refresh_client_size(cols, rows);
        self.outbound_commands.push(cmd.clone());
        cmd
    }

    /// Drains all queued outbound `tmux -CC` commands to be written to the `tmux -CC` subprocess stdin.
    pub fn take_outbound_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outbound_commands)
    }
}

/// Live asynchronous `tmux -CC` Control Mode subprocess handle (`TMUX-03`).
pub struct TmuxControlSubprocess {
    pub session_name: String,
    pub cmd_tx: tokio::sync::mpsc::UnboundedSender<String>,
    pub line_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
}

impl TmuxControlSubprocess {
    /// Spawns `tmux -CC new-session -A -s <session_name>` with piped stdin/stdout channels.
    pub fn spawn_attach_or_new(session_name: &str) -> std::io::Result<Self> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::process::Command;

        let clean_name = if session_name.trim().is_empty() {
            "bilterm"
        } else {
            session_name.trim()
        };
        let mut child = Command::new("tmux")
            .args(["-CC", "new-session", "-A", "-s", clean_name])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().expect("tmux stdout piped");
        let mut stdin = child.stdin.take().expect("tmux stdin piped");

        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                if stdin.write_all(cmd.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line_tx.send(line).is_err() {
                    break;
                }
            }
            let _ = child.wait().await;
        });

        Ok(Self {
            session_name: clean_name.to_string(),
            cmd_tx,
            line_rx,
        })
    }

    /// Constructs a `TmuxControlSubprocess` from pre-existing channel endpoints (for deterministic testing or SSH stream bridges).
    pub fn from_channels(
        session_name: impl Into<String>,
        cmd_tx: tokio::sync::mpsc::UnboundedSender<String>,
        line_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    ) -> Self {
        Self {
            session_name: session_name.into(),
            cmd_tx,
            line_rx,
        }
    }
}

impl WindowManager {
    /// Attaches a live `tmux -CC` Control Mode subprocess (`TmuxControlSubprocess`) to this workspace (`TMUX-03`, `ARCH-R05`).
    pub fn attach_tmux_control(&mut self, subproc: TmuxControlSubprocess) {
        self.tmux_bridge.active_session_name = Some(subproc.session_name.clone());
        self.tmux_control = Some(subproc);
    }

    /// Returns `true` if a `TmuxControlSubprocess` is currently attached (`ARCH-R05`).
    pub fn is_tmux_control_attached(&self) -> bool {
        self.tmux_control.is_some()
    }

    /// Drains all available control lines from an attached `TmuxControlSubprocess` and routes them
    /// through `TmuxControlBridge::handle_control_line` (`TMUX-03`, `ARCH-R05`).
    pub fn poll_tmux_control(&mut self) -> usize {
        let mut lines = Vec::new();
        let mut disconnected = false;
        if let Some(subproc) = self.tmux_control.as_mut() {
            loop {
                match subproc.line_rx.try_recv() {
                    Ok(line) => lines.push(line),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        let count = lines.len();
        let mut bridge = std::mem::take(&mut self.tmux_bridge);
        for line in &lines {
            if matches!(
                bridge.handle_control_line(self, line),
                TmuxControlEvent::Exit { .. }
            ) {
                disconnected = true;
            }
        }
        self.tmux_bridge = bridge;
        if disconnected {
            self.tmux_control = None;
        }
        count
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
        assert!(wm.pane(1).unwrap().vt().contents().contains("HELLO_TMUX"));

        // %output %1 spawns/maps a second CompositePane automatically
        bridge.handle_control_line(&mut wm, "%output %1 SECOND_TMUX_PANE\\015\\012");
        assert_eq!(wm.pane_count(), 2);
        assert!(wm.pane(2).unwrap().vt().contents().contains("SECOND_TMUX_PANE"));

        let send_cmd = format_send_keys_hex(0, b"ls\n");
        assert_eq!(send_cmd, "send-keys -t %0 -H 6c 73 0a\n");
    }
}
