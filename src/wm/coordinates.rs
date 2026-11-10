//! 5-tuple hierarchical coordinate addressing `D:(W,S,Y,P)` (`COORD-01..04`, Phase 71).
//!
//! Daemon (D) -> Workspace (W) -> Session (S) -> Window (Y) -> Pane (P, optional).
//! CONTRACT STUB written by the orchestrator: type shapes and signatures are fixed;
//! bodies are owned by the coordinate implementation task.

use super::WindowManager;
use crate::pane::PaneId;

/// One fully- or partially-specified hierarchical address. `None` = wildcard / unspecified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetCoord {
    pub daemon: Option<String>,
    pub workspace: Option<String>,
    pub session: Option<String>,
    pub window: Option<String>,
    pub pane: Option<String>,
}

/// How the window-title-bar coordinate badge is rendered. Default is `Off` (keeps classic goldens).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CoordDisplayStyle {
    /// `(Wksp: W, Sess: S, Win: Y, Pane: P)`; `Pane:` omitted when single-pane.
    Verbose,
    /// `(W,S,Y,P)` or `(W,S,Y)`.
    Compact,
    /// `(D:(W,S,Y,P))` or `(D:(W,S,Y))`.
    DaemonTuple,
    #[default]
    Off,
}

impl CoordDisplayStyle {
    /// Parses `verbose|compact|daemon|daemon-tuple|off` (case-insensitive).
    pub fn from_name(_name: &str) -> Option<Self> { todo!("COORD impl") }
    /// Canonical name: `verbose|compact|daemon|off`.
    pub fn name(self) -> &'static str { todo!("COORD impl") }
}

/// `:coords` sub-operation carried by `Action::Coords(CoordsOp)` (`COORD-04`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordsOp {
    /// `:coords` — report focused window's coordinate (status bar + `command_data`).
    Show,
    /// `:coords verbose|compact|daemon|off`.
    Style(CoordDisplayStyle),
    /// `:coords locate <coord>` — resolve and focus the first match.
    Locate(String),
    /// `:coords copy` — copy focused window's canonical path to the clipboard.
    Copy,
}

/// Parse error for coordinate text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordParseError(pub String);

impl std::fmt::Display for CoordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for CoordParseError {}

impl TargetCoord {
    /// Parses every accepted form (tuple, verbose tuple, daemon tuple, colon path, `#2`, `%3`).
    pub fn parse(_s: &str) -> Result<Self, CoordParseError> { todo!("COORD impl") }
    /// Formats per `style`; returns `None` for `Off`. Pane component omitted when `self.pane` is `None`.
    pub fn format(&self, _style: CoordDisplayStyle) -> Option<String> { todo!("COORD impl") }
    /// Canonical colon path, e.g. `d0:w1:s1:y2:p1` (unspecified levels omitted).
    pub fn to_path(&self) -> String { todo!("COORD impl") }
}

/// Result of resolving a coordinate against the live workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordResolution {
    /// Matching windows, ascending.
    pub panes: Vec<PaneId>,
    /// Fully-populated coordinate of each match (same order as `panes`).
    pub coords: Vec<TargetCoord>,
}

impl WindowManager {
    /// Fully-populated coordinate of window `id` (`None` if no such pane).
    pub fn coord_of(&self, _id: PaneId) -> Option<TargetCoord> { todo!("COORD impl") }
    /// Resolves `target` (wildcards on `None` levels) to matching windows.
    pub fn resolve_coord(&self, _target: &TargetCoord) -> CoordResolution { todo!("COORD impl") }
    /// Current title-bar badge style.
    pub fn coord_style(&self) -> CoordDisplayStyle { self.coord_style }
    /// Sets the title-bar badge style.
    pub fn set_coord_style(&mut self, style: CoordDisplayStyle) { self.coord_style = style; }
}
