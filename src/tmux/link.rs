//! Shared types, traits and constants of the persistent SSH + tmux link (Phase 51.1, PST-01..04).
//!
//! Frozen by plan 51.1-01 (51.1-CONTRACT.md section 3): every other 51.1 plan fills bodies in
//! files it owns and builds against these definitions. Behaviour lives elsewhere:
//! `reconnect.rs` (state machine), `splitter.rs`/`transport.rs` (PTY transport), `hydrate.rs`
//! (reply correlator and attach hydrate) and `crate::wm::tmux_link`/`tmux_io` (lifecycle, pumps).

use std::collections::{BTreeSet, VecDeque};
use std::time::Instant;

use crate::pane::PaneId;
use crate::remote::RemoteHostProfile;
use crate::tmux::TmuxControlSubprocess;
use crate::tmux::hydrate::HydratePlanner;

/// Session name used when `:ssh-tmux <dest>` gives none.
pub const LINK_SESSION_DEFAULT: &str = "bilterm";
/// Last transport diagnostic lines kept for disconnect reports.
pub const DIAG_TAIL_LINES: usize = 8;
/// Characters per stored diagnostic line.
pub const DIAG_LINE_CAP: usize = 160;
/// Bytes per control line; longer lines are dropped with one diag line.
pub const CONTROL_LINE_MAX: usize = 256 * 1024;
/// Queued tmux commands; the oldest non-hydrate commands are dropped beyond this.
pub const OUTBOUND_CAP: usize = 256;
/// Bytes buffered per pane between ticks.
pub const REMOTE_INPUT_CAP: usize = 4096;
/// Payload bytes per `send-keys -H` command.
pub const SEND_KEYS_CHUNK: usize = 512;
/// Pending reply FIFO bound.
pub const PENDING_REPLIES_CAP: usize = 512;
/// Tmux panes the bridge will map; further ids are dropped with one diag line (remote-controlled
/// input must not create unbounded panes).
pub const MAPPED_PANES_CAP: usize = 64;
/// Scrollback lines requested per pane by the attach hydrate.
pub const HYDRATE_HISTORY_LINES: usize = 2000;
/// Reply lines accepted per capture before the reply is marked truncated.
pub const HYDRATE_REPLY_LINES_CAP: usize = 6000;
/// Grace after the first control line before the greeting is considered over.
pub const GREETING_GRACE_MS: u64 = 1500;

/// The remote a link connects to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkTarget {
    pub profile: RemoteHostProfile,
    pub session: String,
    pub tags: Vec<String>,
}

impl LinkTarget {
    /// SSH destination (`user@host`).
    pub fn dest(&self) -> &str {
        &self.profile.destination
    }

    /// `dest/session`, the pane badge and `remote_target` value.
    pub fn badge(&self) -> String {
        format!("{}/{}", self.profile.destination, self.session)
    }
}

/// Why a transport went away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisconnectCause {
    ControlExit,
    Eof,
    ProcessExit(Option<i32>),
    SpawnFailed(String),
    UserDetach,
}

/// How a disconnect is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitClass {
    Transient,
    Fatal,
    Ended,
    User,
}

/// A recorded transport loss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TmuxDisconnect {
    pub cause: DisconnectCause,
    /// `%exit` text, sanitised.
    pub reason: Option<String>,
    pub code: Option<i32>,
    /// Sanitised diag lines.
    pub tail: Vec<String>,
    pub at_ms: u64,
    pub class: ExitClass,
}

/// Link lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    Idle,
    Connecting { since_ms: u64, awaiting_control: bool },
    Connected { since_ms: u64 },
    Backoff { attempt: u32, retry_at_ms: u64 },
    Detached,
    Ended,
    Failed { attempts: u32 },
}

impl LinkState {
    /// `idle | connecting | connected | reconnecting | detached | ended | failed`.
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkState::Idle => "idle",
            LinkState::Connecting { .. } => "connecting",
            LinkState::Connected { .. } => "connected",
            LinkState::Backoff { .. } => "reconnecting",
            LinkState::Detached => "detached",
            LinkState::Ended => "ended",
            LinkState::Failed { .. } => "failed",
        }
    }

    /// `true` while a link owns the single tmux slot (P-01).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            LinkState::Connecting { .. } | LinkState::Connected { .. } | LinkState::Backoff { .. }
        )
    }
}

/// Reconnect schedule (P-05).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub base_ms: u64,
    pub cap_ms: u64,
    pub factor: u32,
    pub max_attempts: u32,
    pub stable_after_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            base_ms: 1000,
            cap_ms: 30_000,
            factor: 2,
            max_attempts: 20,
            stable_after_ms: 30_000,
        }
    }
}

/// Input to the link state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    Connect,
    ControlStarted,
    Down(TmuxDisconnect),
    UserDisconnect,
    UserReconnect,
}

/// Side effect requested by the link state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkCmd {
    Spawn,
    KillTransport,
    MarkConnected,
    MarkDown { retry_in_ms: u64 },
    MarkEnded,
    MarkFailed,
    MarkDetached,
}

/// Pure link state machine (`reconnect.rs` holds the transitions, plan 04). Fields are
/// crate-visible so the `impl` can live in `reconnect.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkMachine {
    pub(crate) policy: ReconnectPolicy,
    pub(crate) state: LinkState,
    pub(crate) attempt: u32,
    pub(crate) connected_since: Option<u64>,
    /// `true` once a `Spawn` was issued and no `Down`/kill has followed (a transport may exist).
    pub(crate) transport_live: bool,
}

/// What one transport poll produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DriverPoll {
    pub raw_output: Vec<u8>,
    pub diag: Vec<String>,
    pub control_started: bool,
    pub exited: Option<DriverExit>,
}

/// Child exit reported by a driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverExit {
    pub code: Option<i32>,
}

/// The ssh child behind a link (a PTY in production, a fake in tests).
pub trait LinkDriver: Send {
    /// Drains available output; control lines go to the channel, everything else is returned.
    fn pump(&mut self) -> DriverPoll;
    /// Writes raw bytes (ssh prompts) to the transport.
    fn write_raw(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    /// Resizes the transport PTY.
    fn resize(&mut self, rows: u16, cols: u16);
    /// Terminates the transport.
    fn kill(&mut self);
}

/// A connected transport: the control channels plus the optional driver behind them.
pub struct LinkTransport {
    pub control: TmuxControlSubprocess,
    pub driver: Option<Box<dyn LinkDriver>>,
}

/// Starts transports for a target.
pub trait LinkConnector: Send {
    /// `size` is `(rows, cols)`.
    fn connect(&mut self, target: &LinkTarget, size: (u16, u16)) -> std::io::Result<LinkTransport>;
}

/// Spare per-link state for later plans; they edit `tmux_link.rs`/`tmux_io.rs` only, so state
/// they need lives here (plan 12: lifecycle and banners, plan 13: pump bookkeeping).
#[derive(Clone, Debug, Default)]
pub struct LinkAux {
    /// A first control line arrived on the current transport.
    pub control_seen: bool,
    /// Last banner text pushed to the primary pane (change detection).
    pub last_banner: Option<String>,
    /// Pane the banner was last pushed to.
    pub banner_pane: Option<PaneId>,
    /// Last status text set by the lifecycle (change detection).
    pub last_status: Option<String>,
    /// A `Spawn` command is waiting for the next tick.
    pub pending_spawn: bool,
    /// Millisecond stamp of the last newly mapped tmux pane (hydrate burst wait).
    pub last_new_pane_ms: u64,
    /// Millisecond stamp the hydrate batch was sent.
    pub hydrate_sent_ms: Option<u64>,
    /// Mapped pane count seen on the previous tick (fleet refresh detection).
    pub last_mapped_count: usize,
    /// Free-form counters/flags keyed by name.
    pub notes: std::collections::BTreeMap<String, String>,
}

/// All link state held by the `WindowManager` (one link at a time, P-01).
pub struct TmuxLinkState {
    pub target: Option<LinkTarget>,
    pub machine: LinkMachine,
    pub primary_pane: Option<PaneId>,
    pub fleet_node: Option<String>,
    pub last_exit: Option<TmuxDisconnect>,
    pub diag_tail: VecDeque<String>,
    pub driver: Option<Box<dyn LinkDriver>>,
    pub connector: Option<Box<dyn LinkConnector>>,
    pub epoch: Instant,
    pub generation: u64,
    pub hydrate: HydratePlanner,
    pub orphans: BTreeSet<PaneId>,
    pub io_generation: u64,
    pub last_resize: Option<(u16, u16)>,
    pub down_reported_generation: Option<u64>,
    pub aux: LinkAux,
}

impl Default for TmuxLinkState {
    fn default() -> Self {
        Self {
            target: None,
            machine: LinkMachine::new(ReconnectPolicy::default()),
            primary_pane: None,
            fleet_node: None,
            last_exit: None,
            diag_tail: VecDeque::new(),
            driver: None,
            connector: None,
            epoch: Instant::now(),
            generation: 0,
            hydrate: HydratePlanner::default(),
            orphans: BTreeSet::new(),
            io_generation: 0,
            last_resize: None,
            down_reported_generation: None,
            aux: LinkAux::default(),
        }
    }
}

/// Snapshot of the link for `:ssh-tmux info`, `remote.link` and the drawer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkSummary {
    pub state: &'static str,
    pub dest: Option<String>,
    pub session: Option<String>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub retry_in_ms: Option<u64>,
    pub primary_pane: Option<PaneId>,
    /// `(bilterm pane, tmux pane)`.
    pub panes: Vec<(PaneId, usize)>,
    pub stale_panes: Vec<PaneId>,
    pub last_exit: Option<LinkExitSummary>,
}

impl Default for LinkSummary {
    fn default() -> Self {
        Self {
            state: "idle",
            dest: None,
            session: None,
            attempt: 0,
            max_attempts: ReconnectPolicy::default().max_attempts,
            retry_in_ms: None,
            primary_pane: None,
            panes: Vec::new(),
            stale_panes: Vec::new(),
            last_exit: None,
        }
    }
}

/// Last disconnect, flattened for reporting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkExitSummary {
    pub cause: String,
    pub class: &'static str,
    pub code: Option<i32>,
    pub reason: Option<String>,
    pub tail: Vec<String>,
    pub at_ms: u64,
}
