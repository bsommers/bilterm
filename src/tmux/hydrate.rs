//! Reply correlation and attach hydrate for the tmux link (Phase 51.1, plan 06 fills the bodies).
//!
//! Seam state (plan 01): `BlockHeader`/`parse_block_header` are complete; everything else is a
//! compiling skeleton with neutral results so the bridge, WM and pumps can build against it.

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
}

/// One correlated reply block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub kind: ReplyKind,
    pub lines: Vec<String>,
    pub ok: bool,
    pub truncated: bool,
}

/// FIFO matcher between written commands and reply blocks (plan 06).
#[derive(Clone, Debug, Default)]
pub struct ReplyCorrelator {}

impl ReplyCorrelator {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn enqueue(&mut self, _kind: ReplyKind) {}
    pub fn on_begin(&mut self, _header: &BlockHeader) {}
    pub fn on_line(&mut self, _line: &str) {}
    pub fn on_end(&mut self, _header: &BlockHeader, _ok: bool) -> Option<Reply> {
        None
    }
    pub fn take_failed(&mut self) -> Vec<Reply> {
        Vec::new()
    }
    pub fn pending(&self) -> usize {
        0
    }
    pub fn overflowed(&self) -> bool {
        false
    }
    pub fn clear(&mut self) {}
}

/// Where the attach hydrate is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HydratePhase {
    AwaitingGreeting,
    Ready,
    Sent,
    Done,
}

/// Plans the capture batch after attach (plan 06).
#[derive(Clone, Debug, Default)]
pub struct HydratePlanner {}

impl HydratePlanner {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn note_line(&mut self, _now_ms: u64) {}
    pub fn note_block_end(&mut self) {}
    pub fn tick(&mut self, _now_ms: u64) {}
    pub fn phase(&self) -> HydratePhase {
        HydratePhase::AwaitingGreeting
    }
    /// Commands to send once `Ready`: `refresh-client` plus one capture per mapped pane.
    pub fn plan(&mut self, _panes: &[usize], _cols: u16, _rows: u16) -> Vec<(String, ReplyKind)> {
        Vec::new()
    }
    pub fn mark_sent(&mut self) {}
    /// Records a finished capture; `true` when it was the last outstanding one.
    pub fn note_capture_done(&mut self, _tmux_pane: usize) -> bool {
        false
    }
    pub fn remaining(&self) -> usize {
        0
    }
    pub fn reset(&mut self) {}
}

/// `capture-pane -p -e -J -S -2000 -t %N` command text.
pub fn capture_pane_command(_tmux_pane: usize) -> String {
    String::new()
}

/// Turns capture reply lines into bytes for `process_pty_output`.
pub fn render_capture(_lines: &[String]) -> Vec<u8> {
    Vec::new()
}
