//! Reply correlation and attach hydrate for the tmux link (Phase 51.1, plan 06).
//!
//! Pure logic: no workspace types, no clock, no I/O. Callers pass time in as milliseconds.
//!
//! * [`ReplyCorrelator`] matches `%begin`..`%end|%error` reply blocks to the commands Bilterm wrote,
//!   strictly FIFO (tmux command numbers are server-wide, only the per-client reply ORDER is
//!   predictable). A block that starts while the FIFO is empty is the attach greeting (the reply to
//!   the client's own attach command) and is discarded. A second `%begin` while a block is open
//!   finishes the first as a failed reply; those are delivered through
//!   [`ReplyCorrelator::take_failed`], which the WM (plan 13) drains.
//! * [`HydratePlanner`] decides which commands to send after attaching: `refresh-client` plus one
//!   `capture-pane` per mapped pane, once the greeting has passed.
//! * [`capture_pane_command`] / [`render_capture`] build the command and turn its reply into bytes.

use std::collections::{BTreeSet, VecDeque};

use super::format_refresh_client_size;
use super::link::{
    GREETING_GRACE_MS, HYDRATE_HISTORY_LINES, HYDRATE_REPLY_LINES_CAP, PENDING_REPLIES_CAP,
};

/// Kind of a `%begin`/`%end`/`%error` guard line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Begin,
    End,
    Error,
}

/// Parsed `%begin|%end|%error TIME NUMBER FLAGS` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    pub kind: BlockKind,
    pub time: u64,
    pub number: u64,
    pub flags: u32,
}

/// Parses a control-mode block guard line; `None` for anything else (including malformed guards).
pub fn parse_block_header(line: &str) -> Option<BlockHeader> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut parts = line.split(' ');
    let kind = match parts.next()? {
        "%begin" => BlockKind::Begin,
        "%end" => BlockKind::End,
        "%error" => BlockKind::Error,
        _ => return None,
    };
    let time = parts.next()?.parse::<u64>().ok()?;
    let number = parts.next()?.parse::<u64>().ok()?;
    let flags = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(BlockHeader {
        kind,
        time,
        number,
        flags,
    })
}

/// What a queued tmux command's reply block means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyKind {
    Ignore,
    CapturePane { tmux_pane: usize },
    /// `list-panes -s -F "#{window_id} #{pane_id} #{window_name}"` pane discovery.
    ListPanes,
    /// `display-message -p` screen state of every pane, sent just before the captures. Its reply
    /// identifies itself by its marker (see [`ReplyCorrelator::set_state_marker`]) and takes no
    /// place in the FIFO.
    PaneStates,
}

/// One correlated reply block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub kind: ReplyKind,
    pub lines: Vec<String>,
    pub ok: bool,
    /// Body exceeded `HYDRATE_REPLY_LINES_CAP`; the excess was dropped.
    pub truncated: bool,
}

/// What the currently open block will become.
#[derive(Clone, Debug)]
struct OpenReply {
    /// `None` for the attach greeting (no command was queued for it).
    kind: Option<ReplyKind>,
    number: u64,
    lines: Vec<String>,
    truncated: bool,
}

impl OpenReply {
    fn finish(self, ok: bool) -> Option<Reply> {
        Some(Reply {
            kind: self.kind?,
            lines: self.lines,
            ok,
            truncated: self.truncated,
        })
    }
}

/// FIFO matcher between written commands and reply blocks.
#[derive(Clone, Debug, Default)]
pub struct ReplyCorrelator {
    queue: VecDeque<ReplyKind>,
    open: Option<OpenReply>,
    failed: Vec<Reply>,
    overflowed: bool,
    /// First word of a pane-state reply; such a block is recognised by content, not position.
    state_marker: Option<String>,
}

impl ReplyCorrelator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a command that was written; its reply is the next unclaimed block.
    /// Overflow clears the FIFO (guessing would misalign captures) and sets `overflowed()`.
    pub fn enqueue(&mut self, kind: ReplyKind) {
        if self.queue.len() >= PENDING_REPLIES_CAP {
            self.queue.clear();
            self.overflowed = true;
            return;
        }
        self.queue.push_back(kind);
    }

    /// A `%begin` guard: claims the head of the FIFO (empty FIFO means the attach greeting).
    pub fn on_begin(&mut self, header: &BlockHeader) {
        if let Some(prev) = self.open.take()
            && let Some(reply) = prev.finish(false)
        {
            self.failed.push(reply);
        }
        self.open = Some(OpenReply {
            kind: self.queue.pop_front(),
            number: header.number,
            lines: Vec::new(),
            truncated: false,
        });
    }

    /// Sets the first word that marks a pane-state reply (kept across `clear()`).
    pub fn set_state_marker(&mut self, marker: impl Into<String>) {
        self.state_marker = Some(marker.into());
    }

    /// A body line inside the open block; ignored outside a block.
    pub fn on_line(&mut self, line: &str) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        // A state reply was never queued in the FIFO: give back the entry this block claimed.
        if open.lines.is_empty()
            && open.kind != Some(ReplyKind::PaneStates)
            && self.state_marker.as_deref().is_some_and(|m| {
                line.strip_prefix(m).is_some_and(|r| r.is_empty() || r.starts_with(' '))
            })
            && let Some(claimed) = open.kind.replace(ReplyKind::PaneStates)
        {
            self.queue.push_front(claimed);
        }
        if open.lines.len() >= HYDRATE_REPLY_LINES_CAP {
            open.truncated = true;
        } else {
            open.lines.push(line.to_string());
        }
    }

    /// An `%end`/`%error` guard. `None` for the greeting, for no open block, or for a block
    /// number that does not match the open one (the block stays open).
    pub fn on_end(&mut self, header: &BlockHeader, ok: bool) -> Option<Reply> {
        if self.open.as_ref().is_none_or(|o| o.number != header.number) {
            return None;
        }
        self.open.take()?.finish(ok)
    }

    /// Replies abandoned by a second `%begin`; drained by the WM.
    pub fn take_failed(&mut self) -> Vec<Reply> {
        std::mem::take(&mut self.failed)
    }

    /// Commands written whose reply has not started yet.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// The FIFO overflowed since the last `clear()` / `take_overflowed()`.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Returns and resets the overflow flag (for log-once callers).
    pub fn take_overflowed(&mut self) -> bool {
        std::mem::take(&mut self.overflowed)
    }

    /// Forgets everything: FIFO, open block, failed replies and the overflow flag.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.open = None;
        self.failed.clear();
        self.overflowed = false;
        // `state_marker` is kept: it belongs to the bridge, not to one attach's FIFO.
    }
}

/// Where the attach hydrate is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HydratePhase {
    AwaitingGreeting,
    Ready,
    Sent,
    Done,
}

/// Plans the capture batch after attach. Fake-clock friendly: time is passed in.
#[derive(Clone, Debug)]
pub struct HydratePlanner {
    phase: HydratePhase,
    first_line_ms: Option<u64>,
    planned: BTreeSet<usize>,
    pending: BTreeSet<usize>,
}

impl Default for HydratePlanner {
    fn default() -> Self {
        Self {
            phase: HydratePhase::AwaitingGreeting,
            first_line_ms: None,
            planned: BTreeSet::new(),
            pending: BTreeSet::new(),
        }
    }
}

impl HydratePlanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the first control line seen (only the first one counts).
    pub fn note_line(&mut self, now_ms: u64) {
        if self.first_line_ms.is_none() {
            self.first_line_ms = Some(now_ms);
        }
    }

    /// A reply block ended: the greeting is over.
    pub fn note_block_end(&mut self) {
        if self.phase == HydratePhase::AwaitingGreeting {
            self.phase = HydratePhase::Ready;
        }
    }

    /// `GREETING_GRACE_MS` after the first line the greeting is assumed over.
    pub fn tick(&mut self, now_ms: u64) {
        if self.phase != HydratePhase::AwaitingGreeting {
            return;
        }
        if let Some(first) = self.first_line_ms
            && now_ms >= first.saturating_add(GREETING_GRACE_MS)
        {
            self.phase = HydratePhase::Ready;
        }
    }

    pub fn phase(&self) -> HydratePhase {
        self.phase
    }

    /// Commands to send once `Ready`: `refresh-client` (`Ignore`), one state query for all panes
    /// (`marker` is its reply's first word), then one capture per pane in ascending order. Empty outside `Ready`. The phase moves on with `mark_sent()`.
    pub fn plan_with_state(
        &mut self,
        panes: &[usize],
        cols: u16,
        rows: u16,
        marker: &str,
    ) -> Vec<(String, ReplyKind)> {
        let mut out = self.plan(panes, cols, rows);
        if !out.is_empty() {
            out.insert(1, (super::pane_state::pane_states_command(marker), ReplyKind::PaneStates));
        }
        out
    }

    /// `refresh-client` plus one capture per pane, without the state query (text only).
    pub fn plan(&mut self, panes: &[usize], cols: u16, rows: u16) -> Vec<(String, ReplyKind)> {
        if self.phase != HydratePhase::Ready {
            return Vec::new();
        }
        self.planned = panes.iter().copied().collect();
        let mut out = Vec::with_capacity(self.planned.len() + 2);
        out.push((format_refresh_client_size(cols, rows), ReplyKind::Ignore));
        for &n in &self.planned {
            out.push((
                capture_pane_command(n),
                ReplyKind::CapturePane { tmux_pane: n },
            ));
        }
        out
    }

    /// The planned commands were written. With no capture to wait for the hydrate is `Done`.
    pub fn mark_sent(&mut self) {
        if self.phase != HydratePhase::Ready {
            return;
        }
        self.pending = std::mem::take(&mut self.planned);
        self.phase = if self.pending.is_empty() {
            HydratePhase::Done
        } else {
            HydratePhase::Sent
        };
    }

    /// Records a finished capture; `true` when it was the last outstanding one.
    pub fn note_capture_done(&mut self, tmux_pane: usize) -> bool {
        if self.phase != HydratePhase::Sent || !self.pending.remove(&tmux_pane) {
            return false;
        }
        if self.pending.is_empty() {
            self.phase = HydratePhase::Done;
            return true;
        }
        false
    }

    /// Captures still outstanding.
    pub fn remaining(&self) -> usize {
        self.pending.len()
    }

    /// Back to `AwaitingGreeting` (new attach).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `capture-pane -p -e -J -S -2000 -t %N` command text (with newline).
pub fn capture_pane_command(tmux_pane: usize) -> String {
    format!("capture-pane -p -e -J -S -{HYDRATE_HISTORY_LINES} -t %{tmux_pane}\n")
}

/// Turns capture reply lines into bytes for `process_pty_output`: lines joined with `\r\n`,
/// trailing blank lines dropped, no trailing newline (the cursor ends after the last content
/// line, usually the prompt) and no cursor-addressing sequences. Escape sequences pass through.
pub fn render_capture(lines: &[String]) -> Vec<u8> {
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    lines[..end].join("\r\n").into_bytes()
}
