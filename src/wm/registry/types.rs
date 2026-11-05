//! Shared registry types and caps (Phase 50.1 contract, `50.1-CONTRACT.md` section 3).
//!
//! Frozen by plan 50.1-01; plans 02-13 fill providers against these shapes. D-U3: every
//! [`FindRow`] carries a [`ResultSource`].

use std::time::Instant;

use crate::pane::PaneId;
use crate::wm::agent_state::AgentState;
pub use crate::wm::find_cmd::FindKind;

/// Captured commands kept in memory.
pub const CMD_RING_CAP: usize = 2048;
/// Characters per stored command (`sanitize_display`); one source with the pane outbox.
pub const CMD_CAP: usize = crate::pane::capture::CMD_CAP;
/// Characters per label / title / host.
pub const NAME_CAP: usize = 64;
/// Rows returned / rendered.
pub const FIND_RESULTS_CAP: usize = 200;
/// RPC / MCP default row limit.
pub const FIND_DEFAULT_LIMIT: usize = 50;
/// Characters of a query.
pub const FIND_QUERY_CAP: usize = crate::wm::find_cmd::QUERY_CAP;
/// Newest `history.log` lines read.
pub const FIND_HISTORY_READ_CAP: usize = 20_000;
/// Tail window read from `history.log`.
pub const HISTORY_LOG_BYTES_CAP: u64 = 16 * 1024 * 1024;
/// Session TOMLs / history dirs inspected.
pub const SAVED_SESSIONS_CAP: usize = 64;
/// Bytes read per saved-session file.
pub const SAVED_FILE_BYTES_CAP: u64 = 2 * 1024 * 1024;
/// Panes inspected per saved session.
pub const PANES_PER_SESSION_CAP: usize = 64;
/// `CapturedCommand`s per pane between ticks.
pub const PANE_OUTBOX_CAP: usize = crate::pane::capture::PANE_OUTBOX_CAP;

/// D-U3 source kind of a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Live in-memory workspace state.
    Live,
    /// A saved session file on disk.
    SavedSession,
    /// A `history.log` on disk.
    HistoryLog,
}

impl Origin {
    /// Wire name: `live`, `saved-session` or `history-log`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::SavedSession => "saved-session",
            Self::HistoryLog => "history-log",
        }
    }

    /// Sort priority; lower sorts first (`Live` < `SavedSession` < `HistoryLog`, P-05).
    pub fn priority(self) -> u8 {
        match self {
            Self::Live => 0,
            Self::SavedSession => 1,
            Self::HistoryLog => 2,
        }
    }
}

/// Where a result was found (D-U3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultSource {
    pub origin: Origin,
    /// Workspace session name.
    pub name: String,
    /// Sanitised absolute path; `None` for live rows and in-memory snapshots.
    pub path: Option<String>,
}

impl ResultSource {
    /// A live source for session `name`.
    pub fn live(name: impl Into<String>) -> Self {
        Self { origin: Origin::Live, name: name.into(), path: None }
    }

    /// UI tag: `live`, `saved:<name>` or `log:<name>`.
    pub fn label(&self) -> String {
        match self.origin {
            Origin::Live => "live".to_string(),
            Origin::SavedSession => format!("saved:{}", self.name),
            Origin::HistoryLog => format!("log:{}", self.name),
        }
    }
}

/// How a pane reaches its shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transport {
    Local,
    Ssh,
    Tmux,
    SshTmux,
}

impl Transport {
    /// Wire name: `local`, `ssh`, `tmux` or `ssh-tmux`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Ssh => "ssh",
            Self::Tmux => "tmux",
            Self::SshTmux => "ssh-tmux",
        }
    }
}

/// Transport facts about one pane (`remote::transport_of`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportInfo {
    pub transport: Transport,
    pub host: Option<String>,
    pub tmux_session: Option<String>,
    /// `@2` or `@2:name`.
    pub tmux_window: Option<String>,
    pub tmux_pane: Option<usize>,
    pub tags: Vec<String>,
    pub metadata_only: bool,
    /// `false` when the tracked cwd is a local path that says nothing about a remote shell.
    pub cwd_trusted: bool,
}

impl TransportInfo {
    /// A plain local pane.
    pub fn local() -> Self {
        Self {
            transport: Transport::Local,
            host: None,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tags: Vec::new(),
            metadata_only: false,
            cwd_trusted: true,
        }
    }
}

/// When a row's subject last happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recency {
    Live(Instant),
    /// ISO timestamp from a log line.
    Logged(String),
    Saved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRow {
    pub pane: PaneId,
    pub name: String,
    pub state: AgentState,
    pub since_ms: u64,
    pub profile: Option<String>,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRow {
    pub name: String,
    pub active: bool,
    pub parent: Option<String>,
    pub pane_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneRow {
    pub pane: PaneId,
    pub title: String,
    /// Only when `cwd_trusted`.
    pub cwd: Option<String>,
    pub cwd_trusted: bool,
    pub running_command: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub seq: u64,
    pub pane: Option<PaneId>,
    pub cwd: Option<String>,
    /// Never secret-looking.
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub at: Recency,
}

/// Per-kind row payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowData {
    Agent(AgentRow),
    Session(SessionRow),
    Pane(PaneRow),
    History(HistoryRow),
}

impl RowData {
    /// `row` key of the JSON shape: `agent|session|pane|history`.
    pub fn row_name(&self) -> &'static str {
        match self {
            Self::Agent(_) => "agent",
            Self::Session(_) => "session",
            Self::Pane(_) => "pane",
            Self::History(_) => "history",
        }
    }
}

/// One search candidate / result row.
#[derive(Clone, Debug, PartialEq)]
pub struct FindRow {
    /// Which `:find` kind returns this row (`Session` returns `Session` and `Pane` rows).
    pub kind: FindKind,
    /// `agent:<session>/<pane>` | `pane:<session>/<pane>` | `session:<name>` | `history:<session>/<seq>`.
    pub id: String,
    /// Primary text, sanitised, `<= NAME_CAP` (`CMD_CAP` for history).
    pub label: String,
    /// Secondary text, sanitised.
    pub detail: String,
    /// Text the ranker matches (not serialised); contains `label` first.
    pub haystack: String,
    pub source: ResultSource,
    /// Lower-priority origins that also hold the same session name.
    pub also_in: Vec<ResultSource>,
    pub transport: Transport,
    pub host: Option<String>,
    pub tmux_session: Option<String>,
    pub tmux_window: Option<String>,
    pub tmux_pane: Option<usize>,
    pub tags: Vec<String>,
    pub metadata_only: bool,
    pub pane: Option<PaneId>,
    /// Workspace session the row belongs to.
    pub session: String,
    pub data: RowData,
    /// Higher sorts first within an origin and equal score.
    pub order: u64,
    /// Set by `rank::search`.
    pub score: u32,
    /// Char indices into `label`, set by `rank::search`.
    pub matched: Vec<u32>,
}

impl FindRow {
    /// An empty live row of `kind` for providers to fill in.
    pub fn empty(kind: FindKind) -> Self {
        let data = match kind {
            FindKind::Agent => RowData::Agent(AgentRow {
                pane: 0,
                name: String::new(),
                state: AgentState::Idle,
                since_ms: 0,
                profile: None,
                title: String::new(),
            }),
            FindKind::Session => RowData::Session(SessionRow {
                name: String::new(),
                active: false,
                parent: None,
                pane_count: 0,
            }),
            FindKind::History => RowData::History(HistoryRow {
                seq: 0,
                pane: None,
                cwd: None,
                command: String::new(),
                exit_code: None,
                duration_ms: None,
                at: Recency::Saved,
            }),
        };
        Self {
            kind,
            id: String::new(),
            label: String::new(),
            detail: String::new(),
            haystack: String::new(),
            source: ResultSource::live(""),
            also_in: Vec::new(),
            transport: Transport::Local,
            host: None,
            tmux_session: None,
            tmux_window: None,
            tmux_pane: None,
            tags: Vec::new(),
            metadata_only: false,
            pane: None,
            session: String::new(),
            data,
            order: 0,
            score: 0,
            matched: Vec::new(),
        }
    }
}

/// Everything the live workspace knows, no disk I/O.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub agents: Vec<FindRow>,
    pub panes: Vec<FindRow>,
    pub sessions: Vec<FindRow>,
    pub history: Vec<FindRow>,
    pub masked: usize,
}

/// A ranked, capped result list.
#[derive(Clone, Debug)]
pub struct FindResults {
    pub kind: FindKind,
    pub query: String,
    pub rows: Vec<FindRow>,
    pub total: usize,
    pub truncated: bool,
    pub masked: usize,
}

impl FindResults {
    /// No rows, nothing masked.
    pub fn empty(kind: FindKind, query: &str) -> Self {
        Self { kind, query: query.to_string(), rows: Vec::new(), total: 0, truncated: false, masked: 0 }
    }
}

/// Per-open cache of disk reads (saved sessions, parsed `history.log` rows, masked count).
///
/// `registry::saved` (plan 11) checks `is_loaded(kind)`, reads from disk when `false`, then calls
/// `store(kind, rows)` and `add_masked(n)`; later calls reuse `cached(kind)`. `invalidate()` drops
/// everything (a new `:find` open, or the window closing).
#[derive(Debug, Default)]
pub struct DiskCache {
    loaded: [bool; 3],
    rows: [Vec<FindRow>; 3],
    masked: usize,
}

impl DiskCache {
    /// Forgets every loaded read.
    pub fn invalidate(&mut self) {
        *self = Self::default();
    }

    /// Whether `kind` was already read from disk since the last `invalidate()`.
    pub fn is_loaded(&self, kind: FindKind) -> bool {
        self.loaded[kind.index()]
    }

    /// Rows stored for `kind` (empty until `store`).
    pub fn cached(&self, kind: FindKind) -> &[FindRow] {
        &self.rows[kind.index()]
    }

    /// Stores the rows read for `kind` and marks it loaded.
    pub fn store(&mut self, kind: FindKind, rows: Vec<FindRow>) {
        self.rows[kind.index()] = rows;
        self.loaded[kind.index()] = true;
    }

    /// Secret-looking lines masked while reading from disk.
    pub fn masked(&self) -> usize {
        self.masked
    }

    /// Adds `n` to the masked count.
    pub fn add_masked(&mut self, n: usize) {
        self.masked = self.masked.saturating_add(n);
    }
}

/// One command in the in-memory ring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedEntry {
    pub seq: u64,
    pub pane: PaneId,
    pub session: String,
    pub command: String,
    pub cwd: Option<String>,
    pub host: Option<String>,
    pub at: Instant,
    pub block_row: Option<usize>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub exit_logged: bool,
}
