//! Per-pane prompt queue and blocked-prompt classifier (`BEYOND-03`/`BEYOND-04`, Phase 68).
//!
//! `PromptQueue` holds FIFO prompts per pane, with pause/resume and idle draining.
//! `classify_blocked` inspects the tail of a pane's output to detect an interactive
//! approval prompt; `approve_payload`/`deny_payload` synthesize the matching keystrokes.

use crate::pane::PaneId;
use std::collections::{HashMap, VecDeque};

/// Maximum queued prompts per pane.
pub const MAX_QUEUE_LEN: usize = 64;
/// Number of trailing non-empty lines the classifier inspects.
pub const TAIL_LINES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    Full { max: usize },
    IndexOutOfRange { index: usize, len: usize },
}

#[derive(Debug, Default)]
struct PaneQueue {
    items: VecDeque<String>,
    paused: bool,
}

/// Per-pane FIFO prompt queues.
#[derive(Debug, Default)]
pub struct PromptQueue {
    panes: HashMap<PaneId, PaneQueue>,
}

impl PromptQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a prompt; rejects when the pane queue is full.
    pub fn enqueue(&mut self, pane: PaneId, prompt: impl Into<String>) -> Result<usize, QueueError> {
        let q = self.panes.entry(pane).or_default();
        if q.items.len() >= MAX_QUEUE_LEN {
            return Err(QueueError::Full { max: MAX_QUEUE_LEN });
        }
        q.items.push_back(prompt.into());
        Ok(q.items.len())
    }

    /// Pops the front prompt regardless of pause state (explicit dequeue).
    pub fn pop_front(&mut self, pane: PaneId) -> Option<String> {
        self.panes.get_mut(&pane)?.items.pop_front()
    }

    pub fn peek(&self, pane: PaneId) -> Option<&str> {
        self.panes.get(&pane)?.items.front().map(String::as_str)
    }

    pub fn items(&self, pane: PaneId) -> Vec<&str> {
        self.panes
            .get(&pane)
            .map(|q| q.items.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Moves the prompt at `from` to position `to`.
    pub fn reorder(&mut self, pane: PaneId, from: usize, to: usize) -> Result<(), QueueError> {
        let q = self.panes.entry(pane).or_default();
        let len = q.items.len();
        for index in [from, to] {
            if index >= len {
                return Err(QueueError::IndexOutOfRange { index, len });
            }
        }
        let item = q.items.remove(from).expect("checked bound");
        q.items.insert(to, item);
        Ok(())
    }

    /// Removes the prompt at `index`.
    pub fn remove(&mut self, pane: PaneId, index: usize) -> Option<String> {
        self.panes.get_mut(&pane)?.items.remove(index)
    }

    pub fn pause(&mut self, pane: PaneId) {
        self.panes.entry(pane).or_default().paused = true;
    }

    pub fn resume(&mut self, pane: PaneId) {
        if let Some(q) = self.panes.get_mut(&pane) {
            q.paused = false;
        }
    }

    pub fn is_paused(&self, pane: PaneId) -> bool {
        self.panes.get(&pane).is_some_and(|q| q.paused)
    }

    pub fn len(&self, pane: PaneId) -> usize {
        self.panes.get(&pane).map_or(0, |q| q.items.len())
    }

    pub fn is_empty(&self, pane: PaneId) -> bool {
        self.len(pane) == 0
    }

    /// Drain-on-idle: next prompt, only when the pane is not paused.
    pub fn on_idle(&mut self, pane: PaneId) -> Option<String> {
        let q = self.panes.get_mut(&pane)?;
        if q.paused {
            return None;
        }
        q.items.pop_front()
    }

    /// Drops all state for a closed pane.
    pub fn remove_pane(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
    }
}

/// Why a pane appears blocked on user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedReason {
    YesNoConfirm,
    ToolApproval,
    FileWritePermission,
    McpPermission,
    SudoPassword,
}

/// Strips ANSI escape sequences (CSI, OSC, two-byte escapes) and carriage returns.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\u{1b}' => match it.next() {
                Some('[') => {
                    for n in it.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(n) = it.next() {
                        if n == '\u{7}' {
                            break;
                        }
                        if n == '\u{1b}' {
                            it.next(); // ST terminator `ESC \`
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Classifies the tail of a pane's output. Only the last [`TAIL_LINES`] non-empty lines
/// (after ANSI stripping) are examined, case-insensitively.
///
/// Precedence when several match: sudo > MCP > file write > tool > y/n.
pub fn classify_blocked(tail_lines: &[&str]) -> Option<BlockedReason> {
    let mut lines: Vec<String> = tail_lines
        .iter()
        .map(|l| strip_ansi(l).to_lowercase())
        .filter(|l| !l.trim().is_empty())
        .collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let lines = lines.split_off(start);
    let any = |f: &dyn Fn(&str) -> bool| lines.iter().any(|l| f(l));

    if any(&|l| l.contains("[sudo] password for") || l.trim_end().ends_with("password:")) {
        return Some(BlockedReason::SudoPassword);
    }
    if any(&|l| l.contains("mcp") && (l.contains("allow") || l.contains("permission"))) {
        return Some(BlockedReason::McpPermission);
    }
    if any(&|l| {
        l.contains("do you want to make this edit")
            || l.contains("write to file")
            || (l.contains("create file") && l.contains('?'))
    }) {
        return Some(BlockedReason::FileWritePermission);
    }
    if any(&|l| {
        l.contains("do you want to proceed")
            || l.contains("allow tool")
            || l.contains("approve this")
    }) {
        return Some(BlockedReason::ToolApproval);
    }
    if any(&|l| l.contains("[y/n]") || l.contains("(y/n)") || l.contains("continue?")) {
        return Some(BlockedReason::YesNoConfirm);
    }
    None
}

/// Keystrokes that approve the prompt.
///
/// Choices: y/n prompts take `y\r`; numbered menus (tool / file / MCP, Claude-Code style)
/// take `1\r` (option 1 is "Yes"). Sudo returns `None`: passwords are never synthesized.
pub fn approve_payload(reason: &BlockedReason) -> Option<Vec<u8>> {
    match reason {
        BlockedReason::YesNoConfirm => Some(b"y\r".to_vec()),
        BlockedReason::ToolApproval
        | BlockedReason::FileWritePermission
        | BlockedReason::McpPermission => Some(b"1\r".to_vec()),
        BlockedReason::SudoPassword => None,
    }
}

/// Keystrokes that deny the prompt: `n\r` for y/n, Esc for menus, Ctrl-C to abort sudo.
pub fn deny_payload(reason: &BlockedReason) -> Option<Vec<u8>> {
    match reason {
        BlockedReason::YesNoConfirm => Some(b"n\r".to_vec()),
        BlockedReason::ToolApproval
        | BlockedReason::FileWritePermission
        | BlockedReason::McpPermission => Some(vec![0x1b]),
        BlockedReason::SudoPassword => Some(vec![0x03]),
    }
}
