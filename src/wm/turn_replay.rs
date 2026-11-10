//! Per-turn time-travel replay log (`HUD-02`, Phase 70).
//!
//! Records prompt -> working -> idle turns with a screen snapshot and touched files.
//! Read-only by construction (`T-REP-01`): this module holds no PTY handle or writer, only
//! owned text. All collections are capped.

use std::collections::VecDeque;

/// Maximum retained turns (oldest evicted).
pub const MAX_TURNS: usize = 64;
/// Maximum snapshot lines kept per turn (tail kept).
pub const MAX_SNAPSHOT_LINES: usize = 2000;
/// Maximum characters kept per line.
pub const MAX_LINE_CHARS: usize = 512;
/// Maximum characters kept of a prompt.
pub const MAX_PROMPT_CHARS: usize = 4096;
/// Maximum distinct files per turn.
pub const MAX_FILES_PER_TURN: usize = 64;

/// One recorded turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub turn_id: u64,
    pub prompt: String,
    pub start_ts: u64,
    pub end_ts: Option<u64>,
    pub working_ts: Option<u64>,
    pub snapshot: Vec<String>,
    pub files: Vec<String>,
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn cap_tail(v: &mut Vec<String>) {
    if v.len() > MAX_SNAPSHOT_LINES {
        v.drain(..v.len() - MAX_SNAPSHOT_LINES);
    }
}

/// Replay log with a cursor over finished turns.
#[derive(Debug, Default)]
pub struct TurnReplayLog {
    turns: VecDeque<Turn>,
    open: Option<Turn>,
    next_id: u64,
    cursor: Option<u64>,
    scroll: usize,
}

impl TurnReplayLog {
    pub fn new() -> Self {
        Self::default()
    }
    /// Finished turns.
    pub fn len(&self) -> usize {
        self.turns.len()
    }
    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }
    pub fn turns(&self) -> impl Iterator<Item = &Turn> {
        self.turns.iter()
    }
    pub fn in_turn(&self) -> bool {
        self.open.is_some()
    }

    /// Starts a turn; an already-open turn is closed first (without a snapshot).
    pub fn begin_turn(&mut self, prompt: &str, ts: u64) -> u64 {
        if self.open.is_some() {
            self.end_turn(ts, Vec::new());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.open = Some(Turn {
            turn_id: id,
            prompt: clip(prompt, MAX_PROMPT_CHARS),
            start_ts: ts,
            end_ts: None,
            working_ts: None,
            snapshot: Vec::new(),
            files: Vec::new(),
        });
        id
    }
    pub fn mark_working(&mut self, ts: u64) {
        if let Some(t) = self.open.as_mut() {
            t.working_ts.get_or_insert(ts);
        }
    }
    /// Appends one output line to the open turn's running snapshot.
    pub fn record_output(&mut self, line: &str) {
        if let Some(t) = self.open.as_mut() {
            t.snapshot.push(clip(line, MAX_LINE_CHARS));
            cap_tail(&mut t.snapshot);
        }
    }
    /// Records a touched file (deduplicated, capped).
    pub fn touch_file(&mut self, path: &str) {
        if let Some(t) = self.open.as_mut() {
            let p = clip(path, MAX_LINE_CHARS);
            if !t.files.contains(&p) && t.files.len() < MAX_FILES_PER_TURN {
                t.files.push(p);
            }
        }
    }
    /// Finishes the open turn. A non-empty `snapshot_lines` replaces the running snapshot.
    pub fn end_turn(&mut self, ts: u64, snapshot_lines: Vec<String>) -> Option<u64> {
        let mut t = self.open.take()?;
        if !snapshot_lines.is_empty() {
            t.snapshot = snapshot_lines
                .iter()
                .map(|l| clip(l, MAX_LINE_CHARS))
                .collect();
            cap_tail(&mut t.snapshot);
        }
        t.end_ts = Some(ts);
        let id = t.turn_id;
        let follow = self.cursor.is_none() || self.cursor == self.turns.back().map(|x| x.turn_id);
        self.turns.push_back(t);
        while self.turns.len() > MAX_TURNS {
            self.turns.pop_front();
        }
        if follow
            || self
                .cursor
                .is_some_and(|c| !self.turns.iter().any(|t| t.turn_id == c))
        {
            self.cursor = Some(if follow { id } else { self.turns[0].turn_id });
            self.scroll = 0;
        }
        Some(id)
    }

    // --- cursor ---
    pub fn current(&self) -> Option<&Turn> {
        let c = self.cursor?;
        self.turns.iter().find(|t| t.turn_id == c)
    }
    fn index(&self) -> Option<usize> {
        let c = self.cursor?;
        self.turns.iter().position(|t| t.turn_id == c)
    }
    fn set_index(&mut self, i: usize) {
        self.cursor = self.turns.get(i).map(|t| t.turn_id);
        self.scroll = 0;
    }
    /// Steps to the previous turn; false at the first turn.
    pub fn prev(&mut self) -> bool {
        match self.index() {
            Some(i) if i > 0 => {
                self.set_index(i - 1);
                true
            }
            _ => false,
        }
    }
    /// Steps to the next turn; false at the last turn.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        match self.index() {
            Some(i) if i + 1 < self.turns.len() => {
                self.set_index(i + 1);
                true
            }
            _ => false,
        }
    }
    pub fn goto(&mut self, turn_id: u64) -> bool {
        match self.turns.iter().position(|t| t.turn_id == turn_id) {
            Some(i) => {
                self.set_index(i);
                true
            }
            None => false,
        }
    }
    pub fn first(&mut self) -> bool {
        if self.turns.is_empty() {
            return false;
        }
        self.set_index(0);
        true
    }
    pub fn last(&mut self) -> bool {
        if self.turns.is_empty() {
            return false;
        }
        self.set_index(self.turns.len() - 1);
        true
    }

    // --- in-turn scroll (top-line offset) ---
    pub fn scroll(&self) -> usize {
        self.scroll
    }
    fn max_scroll(&self, viewport: usize) -> usize {
        self.current()
            .map_or(0, |t| t.snapshot.len().saturating_sub(viewport))
    }
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }
    pub fn scroll_down(&mut self, n: usize, viewport: usize) {
        self.scroll = self.scroll.saturating_add(n).min(self.max_scroll(viewport));
    }
    /// Snapshot lines visible for a viewport height at the current scroll.
    pub fn visible_lines(&self, viewport: usize) -> &[String] {
        let Some(t) = self.current() else { return &[] };
        let s = self.scroll.min(self.max_scroll(viewport));
        let e = (s + viewport).min(t.snapshot.len());
        &t.snapshot[s..e]
    }
}
