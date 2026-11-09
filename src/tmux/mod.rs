pub mod hydrate;
pub mod link;
pub mod reconnect;
pub mod list_panes;
pub mod splitter;
pub mod transport;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
    /// `%window-renamed @<window_id> <name>`
    WindowRenamed { window_id: usize, name: String },
    /// `%session-changed $<session_id> <name>`
    SessionChanged { session_id: usize, name: String },
    /// `%exit [reason]`
    Exit { reason: Option<String> },
    /// `%begin <timestamp> <cmd_num> <flags>`
    BeginBlock,
    /// `%end <timestamp> <cmd_num> <flags>`
    EndBlock,
    /// `%error <timestamp> <cmd_num> <flags>` (a reply block that failed)
    ErrorBlock,
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

    if let Some(rest) = trimmed.strip_prefix("%window-renamed ") {
        let mut parts = rest.splitn(2, ' ');
        if let Some(win_tok) = parts.next()
            && let Some(id_str) = win_tok.strip_prefix('@')
            && let Ok(window_id) = id_str.parse::<usize>()
        {
            let name = parts.next().unwrap_or("").to_string();
            return TmuxControlEvent::WindowRenamed { window_id, name };
        }
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
    if trimmed.starts_with("%error ") {
        return TmuxControlEvent::ErrorBlock;
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

/// Longest `%exit` reason kept (remote-controlled text, shown in banners and status lines).
const EXIT_REASON_CAP: usize = 160;
/// Longest badge kept for titles and `remote_target`.
const BADGE_CAP: usize = 240;
/// Most unread correlated replies kept between `take_replies` calls.
const COMPLETED_CAP: usize = link::PENDING_REPLIES_CAP;

/// Bridge synchronizing a bi-directional `tmux -CC` Control Mode stream with Bilterm's `WindowManager` (`TMUX-02`, `TMUX-03`).
///
/// Phase 51.1: starts empty (no `%0` seed), adopts the link's primary pane, reconciles ids on
/// reattach, queues closed panes for the WM, and never lets reply-body text act as an event.
#[derive(Clone, Debug, Default)]
pub struct TmuxControlBridge {
    /// Maps `tmux` `%pane_id` -> Bilterm `PaneId`.
    pub tmux_to_bilterm: BTreeMap<usize, PaneId>,
    /// Reverse map from Bilterm `PaneId` -> `tmux` `%pane_id`.
    pub bilterm_to_tmux: BTreeMap<PaneId, usize>,
    pub active_session_name: Option<String>,
    /// Maps `tmux` `%pane_id` -> `@window_id` (kept current from layout/pane/close events).
    pub window_of_pane: BTreeMap<usize, usize>,
    /// Window names learned from `%window-renamed` (windows that predate attach are unnamed, gap G10).
    pub window_names: BTreeMap<usize, String>,
    pub outbound_commands: Vec<String>,
    pub exited: bool,
    /// Link primary pane waiting to adopt the first discovered tmux pane (Phase 51.1, P-08).
    primary: Option<PaneId>,
    /// Pane bound to a guessed tmux id by `map_provisional_primary`, until `list-panes` confirms.
    provisional: Option<PaneId>,
    /// `dest/session` badge used to title adopted panes.
    badge: Option<String>,
    /// Panes mapped before the last `reset_for_attach` and not yet re-adopted, by old tmux id.
    stale: BTreeMap<usize, PaneId>,
    /// Panes whose tmux pane or window closed, waiting for the WM to remove them.
    removed: Vec<PaneId>,
    /// `%exit` reason text of the current attach (sanitised, capped).
    exit_reason: Option<String>,
    /// tmux panes whose `%output` is dropped while a capture is pending.
    held: BTreeSet<usize>,
    /// Reply blocks closed so far, greeting included.
    blocks_closed: u64,
    /// Reply block currently open: its body lines are never events.
    open_block: Option<hydrate::BlockHeader>,
    replies: hydrate::ReplyCorrelator,
    completed: Vec<hydrate::Reply>,
    /// `ReplyKind` of each entry of `outbound_commands`, in the same order.
    outbound_kinds: VecDeque<hydrate::ReplyKind>,
    first_line_seen: bool,
    cap_noted: bool,
    diag: Vec<String>,
}

impl TmuxControlBridge {
    /// An empty bridge: nothing is mapped until tmux names a pane.
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves the current mappings to the stale set and forgets the previous attach's stream state
    /// (exit, reply blocks, queued commands). A rediscovered id keeps its pane; see `ensure_tmux_pane`.
    pub fn reset_for_attach(&mut self) {
        for (&pane, &tmux) in &self.bilterm_to_tmux {
            self.stale.insert(tmux, pane);
        }
        self.tmux_to_bilterm.clear();
        self.bilterm_to_tmux.clear();
        self.window_of_pane.clear();
        self.provisional = None;
        self.exited = false;
        self.exit_reason = None;
        self.held.clear();
        self.open_block = None;
        self.replies.clear();
        self.completed.clear();
        self.outbound_commands.clear();
        self.outbound_kinds.clear();
        self.blocks_closed = 0;
        self.first_line_seen = false;
        self.cap_noted = false;
    }

    /// `(tmux pane, bilterm pane)` pairs currently mapped, ordered by tmux pane id.
    pub fn mapped_panes(&self) -> Vec<(usize, PaneId)> {
        self.tmux_to_bilterm.iter().map(|(&t, &p)| (t, p)).collect()
    }

    /// Panes mapped before the last attach that have not been re-adopted (ascending).
    pub fn stale_panes(&self) -> Vec<PaneId> {
        let set: BTreeSet<PaneId> = self.stale.values().copied().collect();
        set.into_iter().collect()
    }

    /// `%exit` reason of the current attach, when tmux gave one.
    pub fn exit_reason(&self) -> Option<&str> {
        self.exit_reason.as_deref()
    }

    /// Panes whose tmux pane or window closed since the last call (the WM removes them).
    pub fn take_removed_panes(&mut self) -> Vec<PaneId> {
        std::mem::take(&mut self.removed)
    }

    /// tmux panes known to live in `window`.
    pub fn window_panes(&self, window: usize) -> Vec<usize> {
        self.window_of_pane
            .iter()
            .filter(|(_, w)| **w == window)
            .map(|(&p, _)| p)
            .collect()
    }

    /// Queues a raw tmux command with the reply kind it will be correlated with.
    pub fn queue_command(&mut self, mut cmd: String, kind: hydrate::ReplyKind) {
        if !cmd.ends_with('\n') {
            cmd.push('\n');
        }
        self.push_outbound(cmd, kind);
    }

    fn push_outbound(&mut self, cmd: String, kind: hydrate::ReplyKind) {
        if self.outbound_commands.len() >= link::OUTBOUND_CAP {
            // Keep captures (they carry the hydrate); drop the oldest plain command first.
            let idx = self
                .outbound_kinds
                .iter()
                .position(|k| matches!(k, hydrate::ReplyKind::Ignore))
                .unwrap_or(0)
                .min(self.outbound_commands.len() - 1);
            self.outbound_commands.remove(idx);
            self.outbound_kinds.remove(idx);
        }
        self.outbound_commands.push(cmd);
        self.outbound_kinds.push_back(kind);
    }

    /// Correlated replies since the last call, failed ones included (`ok == false`).
    pub fn take_replies(&mut self) -> Vec<hydrate::Reply> {
        let mut out = std::mem::take(&mut self.completed);
        out.extend(self.replies.take_failed());
        out
    }

    /// Ends a reattach reconcile: stale panes never re-adopted are returned (orphans) and kept.
    pub fn finish_reconcile(&mut self) -> Vec<PaneId> {
        let orphans = self.stale_panes();
        self.stale.clear();
        orphans
    }

    /// Drops `%output` for `tmux_pane` until [`Self::release_output`] (capture pending).
    pub fn hold_output(&mut self, tmux_pane: usize) {
        self.held.insert(tmux_pane);
    }

    /// Resumes live output for `tmux_pane`.
    pub fn release_output(&mut self, tmux_pane: usize) {
        self.held.remove(&tmux_pane);
    }

    /// Reply blocks closed so far, greeting included.
    pub fn blocks_closed(&self) -> u64 {
        self.blocks_closed
    }

    /// Diagnostic lines recorded since the last call (pane cap reached).
    pub fn take_diag(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diag)
    }

    /// `true` exactly once per attach: the first control line of a transport.
    pub(crate) fn note_first_line(&mut self) -> bool {
        !std::mem::replace(&mut self.first_line_seen, true)
    }

    /// Marks `pane` as the link's primary pane: the first tmux pane discovered adopts it
    /// instead of creating a new pane (Phase 51.1, P-08).
    pub fn set_primary(&mut self, pane: PaneId) {
        self.primary = Some(pane);
    }

    /// The primary pane that has not adopted a tmux pane yet.
    pub fn primary(&self) -> Option<PaneId> {
        self.primary
    }

    /// `dest/session` text used to title adopted panes.
    pub fn set_badge(&mut self, badge: Option<String>) {
        self.badge = badge.map(|b| crate::vt::sanitize_display(&b, BADGE_CAP));
    }

    /// The `tmux` window id (`@N`) that holds `tmux_pane`, when known.
    pub fn window_of(&self, tmux_pane: usize) -> Option<usize> {
        self.window_of_pane.get(&tmux_pane).copied()
    }

    /// The window's name once a `%window-renamed` was seen for it.
    pub fn window_name(&self, window: usize) -> Option<&str> {
        self.window_names.get(&window).map(String::as_str)
    }

    /// Next local pane that may adopt a newly discovered tmux pane: the pending primary, then a
    /// stale pane of the same tmux id, then the lowest-numbered stale pane. Missing panes are skipped.
    fn take_adoptable(&mut self, wm: &WindowManager, tmux: usize) -> Option<PaneId> {
        let live = |p: PaneId, me: &Self| wm.pane(p).is_some() && !me.bilterm_to_tmux.contains_key(&p);
        if let Some(p) = self.primary.take()
            && live(p, self)
        {
            return Some(p);
        }
        if let Some(p) = self.stale.remove(&tmux)
            && live(p, self)
        {
            return Some(p);
        }
        while let Some((&t, &p)) = self.stale.iter().min_by_key(|(_, p)| **p) {
            self.stale.remove(&t);
            if live(p, self) {
                return Some(p);
            }
        }
        None
    }

    /// Maps `tmux_pane_id` to a Bilterm pane (adopting or creating one); `None` beyond the cap.
    fn ensure_tmux_pane(&mut self, wm: &mut WindowManager, tmux_pane_id: usize) -> Option<PaneId> {
        if let Some(&id) = self.tmux_to_bilterm.get(&tmux_pane_id) {
            return Some(id);
        }
        if self.tmux_to_bilterm.len() >= link::MAPPED_PANES_CAP {
            if !std::mem::replace(&mut self.cap_noted, true) {
                let note = format!(
                    "tmux pane limit ({}) reached; further panes are ignored",
                    link::MAPPED_PANES_CAP
                );
                tracing::debug!(target: "bilterm::link", "{note}");
                self.diag.push(note);
            }
            return None;
        }
        self.rebind_provisional(tmux_pane_id);
        let id = match self.take_adoptable(wm, tmux_pane_id) {
            Some(id) => {
                if let Some(pane) = wm.pane_mut(id) {
                    pane.process_pty_output(b"\x1bc");
                    let title = match &self.badge {
                        Some(b) => format!("TMUX-SSH:{b} %{tmux_pane_id} [#{id}]"),
                        None => format!("TMUX %{tmux_pane_id} [#{id}]"),
                    };
                    pane.set_title(title);
                }
                self.stale.retain(|_, p| *p != id);
                id
            }
            None => {
                // No `remote_target` here: the registry reads one as "label-only ssh-tmux pane"
                // (tests/phase50_1_e2e.rs keeps `%5` a plain `tmux` row); plan 51.1-08 owns that.
                wm.insert_tmux_pane(tmux_pane_id)
            }
        };
        self.tmux_to_bilterm.insert(tmux_pane_id, id);
        self.bilterm_to_tmux.insert(id, tmux_pane_id);
        Some(id)
    }

    /// Forgets a closed tmux pane and queues its Bilterm pane for removal by the WM.
    fn drop_tmux_pane(&mut self, tmux_pane_id: usize) {
        self.window_of_pane.remove(&tmux_pane_id);
        self.held.remove(&tmux_pane_id);
        if let Some(pane) = self.tmux_to_bilterm.remove(&tmux_pane_id) {
            self.bilterm_to_tmux.remove(&pane);
            if !self.removed.contains(&pane) {
                self.removed.push(pane);
            }
        }
    }

    fn close_block(&mut self, header: hydrate::BlockHeader, ok: bool) {
        self.open_block = None;
        self.blocks_closed += 1;
        if let Some(reply) = self.replies.on_end(&header, ok)
            && self.completed.len() < COMPLETED_CAP
        {
            self.completed.push(reply);
        }
    }

    /// Handles one line while a reply block is open: only the matching `%end`/`%error` (or a new
    /// `%begin`, which abandons the unterminated block) is a guard; everything else is body text.
    fn handle_block_line(&mut self, open: hydrate::BlockHeader, line: &str) -> TmuxControlEvent {
        use hydrate::BlockKind;
        match hydrate::parse_block_header(line) {
            Some(h) if h.kind != BlockKind::Begin && h.number == open.number => {
                self.close_block(open, h.kind == BlockKind::End);
                if h.kind == BlockKind::End {
                    TmuxControlEvent::EndBlock
                } else {
                    TmuxControlEvent::ErrorBlock
                }
            }
            Some(h) if h.kind == BlockKind::Begin => {
                self.close_block(open, false);
                self.open_block = Some(h);
                self.replies.on_begin(&h);
                TmuxControlEvent::BeginBlock
            }
            _ => {
                let body = line.trim_end_matches(&['\r', '\n'][..]);
                self.replies.on_line(body);
                TmuxControlEvent::Other(body.to_string())
            }
        }
    }

    /// Processes a line from `tmux -CC` stdout and updates the corresponding `CompositePane` in `wm`.
    pub fn handle_control_line(&mut self, wm: &mut WindowManager, line: &str) -> TmuxControlEvent {
        if let Some(open) = self.open_block {
            return self.handle_block_line(open, line);
        }
        let ev = parse_tmux_control_line(line);
        match &ev {
            TmuxControlEvent::Output { pane_id, data } => {
                if !self.held.contains(pane_id)
                    && let Some(id) = self.ensure_tmux_pane(wm, *pane_id)
                    && let Some(pane) = wm.pane_mut(id)
                {
                    pane.process_pty_output(data);
                }
            }
            TmuxControlEvent::LayoutChange { window_id, layout } => {
                let panes = parse_tmux_layout_panes(layout);
                if !panes.is_empty() {
                    let gone: Vec<usize> = self
                        .window_of_pane
                        .iter()
                        .filter(|(p, w)| **w == *window_id && !panes.contains(p))
                        .map(|(&p, _)| p)
                        .collect();
                    for p in gone {
                        self.drop_tmux_pane(p);
                    }
                }
                for tmux_pane_id in panes {
                    if self.ensure_tmux_pane(wm, tmux_pane_id).is_some() {
                        self.window_of_pane.insert(tmux_pane_id, *window_id);
                    }
                }
            }
            TmuxControlEvent::WindowPaneChanged { window_id, pane_id } => {
                if let Some(id) = self.ensure_tmux_pane(wm, *pane_id) {
                    self.window_of_pane.insert(*pane_id, *window_id);
                    wm.focus_and_raise(id);
                }
            }
            TmuxControlEvent::WindowClose { window_id } => {
                for p in self.window_panes(*window_id) {
                    self.drop_tmux_pane(p);
                }
                self.window_names.remove(window_id);
            }
            TmuxControlEvent::WindowRenamed { window_id, name } => {
                // Remote-influenced text: bound both the name and the number of tracked windows.
                if self.window_names.len() < 1024 || self.window_names.contains_key(window_id) {
                    let capped: String = name.chars().take(256).collect();
                    self.window_names.insert(*window_id, capped);
                }
            }
            TmuxControlEvent::SessionChanged { name, .. } => {
                self.active_session_name = Some(name.clone());
            }
            TmuxControlEvent::Exit { reason } => {
                self.exited = true;
                self.exit_reason = reason
                    .as_deref()
                    .map(|r| crate::vt::sanitize_display(r, EXIT_REASON_CAP))
                    .filter(|r| !r.is_empty());
            }
            TmuxControlEvent::BeginBlock => {
                if let Some(h) = hydrate::parse_block_header(line)
                    && h.kind == hydrate::BlockKind::Begin
                {
                    self.open_block = Some(h);
                    self.replies.on_begin(&h);
                }
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
        self.push_outbound(cmd.clone(), hydrate::ReplyKind::Ignore);
        Some(cmd)
    }

    /// Queues a `split-window` command for the mapped `tmux` `%pane_id` (`TMUX-03`).
    pub fn queue_split(&mut self, bilterm_pane_id: PaneId, horizontal: bool) -> Option<String> {
        let &tmux_id = self.bilterm_to_tmux.get(&bilterm_pane_id)?;
        let cmd = format_split_window(tmux_id, horizontal);
        self.push_outbound(cmd.clone(), hydrate::ReplyKind::Ignore);
        Some(cmd)
    }

    /// Queues a `refresh-client -C <cols>x<rows>` command (`TMUX-03`).
    pub fn queue_resize_client(&mut self, cols: u16, rows: u16) -> String {
        let cmd = format_refresh_client_size(cols, rows);
        self.push_outbound(cmd.clone(), hydrate::ReplyKind::Ignore);
        cmd
    }

    /// Drains all queued outbound `tmux -CC` commands to be written to the `tmux -CC` subprocess
    /// stdin, registering each command's `ReplyKind` with the correlator in write order.
    pub fn take_outbound_commands(&mut self) -> Vec<String> {
        let cmds = std::mem::take(&mut self.outbound_commands);
        for _ in &cmds {
            let kind = self
                .outbound_kinds
                .pop_front()
                .unwrap_or(hydrate::ReplyKind::Ignore);
            self.replies.enqueue(kind);
        }
        self.outbound_kinds.clear();
        cmds
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
    ///
    /// Mappings of an earlier attach become stale (reconciled by tmux id as panes are rediscovered).
    pub fn attach_tmux_control(&mut self, subproc: TmuxControlSubprocess) {
        self.tmux_bridge.reset_for_attach();
        self.tmux_bridge.active_session_name = Some(subproc.session_name.clone());
        self.tmux_control = Some(subproc);
        if self.tmux_link.target.is_some() {
            self.on_tmux_control_attached();
        }
    }

    /// Returns `true` if a `TmuxControlSubprocess` is currently attached (`ARCH-R05`).
    pub fn is_tmux_control_attached(&self) -> bool {
        self.tmux_control.is_some()
    }

    /// Drains all available control lines from an attached `TmuxControlSubprocess` and routes them
    /// through `TmuxControlBridge::handle_control_line` (`TMUX-03`, `ARCH-R05`).
    ///
    /// A disconnect (`%exit` or channel EOF) is reported once through `on_tmux_control_down` when a
    /// link record exists, before `tmux_control` is dropped; without a link record the legacy
    /// silent drop remains. Lines after `%exit` in the same batch are not applied.
    pub fn poll_tmux_control(&mut self) -> usize {
        let mut lines = Vec::new();
        let mut eof = false;
        let Some(subproc) = self.tmux_control.as_mut() else {
            return 0;
        };
        loop {
            match subproc.line_rx.try_recv() {
                Ok(line) => lines.push(line),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    eof = true;
                    break;
                }
            }
        }
        let count = lines.len();
        if count > 0 && self.tmux_bridge.note_first_line() {
            self.on_tmux_control_line();
        }
        let mut bridge = std::mem::take(&mut self.tmux_bridge);
        for line in &lines {
            if bridge.exited {
                break;
            }
            bridge.handle_control_line(self, line);
        }
        let exited = bridge.exited;
        let reason = bridge.exit_reason().map(ToOwned::to_owned);
        self.tmux_bridge = bridge;
        if exited || eof {
            if self.tmux_link.target.is_some() {
                let tail = self
                    .tmux_link
                    .diag_tail
                    .iter()
                    .map(|l| crate::vt::sanitize_display(l, link::DIAG_LINE_CAP))
                    .collect();
                let down = link::TmuxDisconnect {
                    cause: if exited {
                        link::DisconnectCause::ControlExit
                    } else {
                        link::DisconnectCause::Eof
                    },
                    reason,
                    code: None,
                    tail,
                    at_ms: self.link_now_ms(),
                    // Neutral initial value; plan 12 classifies when it handles the report.
                    class: link::ExitClass::Transient,
                };
                self.on_tmux_control_down(down);
            }
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

        // %output %0 with octal \033 (ESC), \015 (\r), \012 (\n) maps a NEW pane (no seed).
        bridge.handle_control_line(&mut wm, "%output %0 \\033[32mHELLO_TMUX\\033[0m\\015\\012");
        let first = bridge.tmux_to_bilterm[&0];
        assert_ne!(first, 1, "a stranger tmux id never writes into local pane 1");
        assert!(wm.pane(first).unwrap().vt().contents().contains("HELLO_TMUX"));
        assert!(!wm.pane(1).unwrap().vt().contents().contains("HELLO_TMUX"));

        // %output %1 spawns/maps a second CompositePane automatically
        bridge.handle_control_line(&mut wm, "%output %1 SECOND_TMUX_PANE\\015\\012");
        assert_eq!(wm.pane_count(), 3);
        let second = bridge.tmux_to_bilterm[&1];
        assert!(wm.pane(second).unwrap().vt().contents().contains("SECOND_TMUX_PANE"));

        let send_cmd = format_send_keys_hex(0, b"ls\n");
        assert_eq!(send_cmd, "send-keys -t %0 -H 6c 73 0a\n");
    }

    fn linked_wm_with_channel() -> (WindowManager, tokio::sync::mpsc::UnboundedSender<String>) {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        wm.attach_tmux_control(TmuxControlSubprocess::from_channels("main", cmd_tx, line_rx));
        (wm, line_tx)
    }

    #[test]
    fn percent_exit_reports_once_with_reason() {
        let (mut wm, tx) = linked_wm_with_channel();
        tx.send("%output %3 hi".into()).unwrap();
        tx.send("%exit server exited unexpectedly".into()).unwrap();
        tx.send("%output %7 after-exit".into()).unwrap();
        wm.poll_all_ptys();
        assert!(!wm.is_tmux_control_attached());
        let d = wm.tmux_link.last_exit.clone().expect("disconnect reported");
        assert_eq!(d.cause, link::DisconnectCause::ControlExit);
        assert_eq!(d.reason.as_deref(), Some("server exited unexpectedly"));
        assert!(d.tail.is_empty());
        assert!(!wm.tmux_bridge.tmux_to_bilterm.contains_key(&7), "batch stops at %exit");
        // A later poll has nothing attached and reports nothing more.
        wm.tmux_link.last_exit = None;
        wm.poll_all_ptys();
        assert!(wm.tmux_link.last_exit.is_none());
    }

    #[test]
    fn eof_without_exit_reports_eof_and_legacy_drop_stays_silent() {
        let (mut wm, tx) = linked_wm_with_channel();
        drop(tx);
        wm.poll_all_ptys();
        let d = wm.tmux_link.last_exit.clone().expect("eof reported");
        assert_eq!(d.cause, link::DisconnectCause::Eof);
        assert!(d.reason.is_none());
        assert!(!wm.is_tmux_control_attached());

        // No link record: %exit drops the control as before and records nothing.
        let mut plain = WindowManager::new_headless(24, 80);
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        plain.attach_tmux_control(TmuxControlSubprocess::from_channels("x", cmd_tx, line_rx));
        line_tx.send("%exit".into()).unwrap();
        plain.poll_all_ptys();
        assert!(!plain.is_tmux_control_attached());
        assert!(plain.tmux_link.last_exit.is_none());
    }
}
