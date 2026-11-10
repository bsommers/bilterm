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
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "verbose" => Some(Self::Verbose),
            "compact" => Some(Self::Compact),
            "daemon" | "daemon-tuple" => Some(Self::DaemonTuple),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
    /// Canonical name: `verbose|compact|daemon|off`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Verbose => "verbose",
            Self::Compact => "compact",
            Self::DaemonTuple => "daemon",
            Self::Off => "off",
        }
    }
    /// Badge cycle order for the menu / `:coords cycle`: Off -> Compact -> Verbose -> DaemonTuple -> Off.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Compact,
            Self::Compact => Self::Verbose,
            Self::Verbose => Self::DaemonTuple,
            Self::DaemonTuple => Self::Off,
        }
    }
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
    /// `:coords cycle` — advance the badge style (`CoordDisplayStyle::next`).
    Cycle,
}

/// Parse error for coordinate text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordParseError(pub String);

impl std::fmt::Display for CoordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for CoordParseError {}

/// Hierarchy level, ordered outermost to innermost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Daemon,
    Workspace,
    Session,
    Window,
    Pane,
}

fn err<T>(msg: impl Into<String>) -> Result<T, CoordParseError> {
    Err(CoordParseError(msg.into()))
}

/// `true` when `s` is `prefix` (either case) followed by one or more digits.
fn prefixed_digits(s: &str, prefixes: &[char]) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if prefixes.contains(&c.to_ascii_lowercase()) => {
            let rest = it.as_str();
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

/// Classifies a segment by its prefix (`d0`, `w1`, `s1`, `y2`/`#2`, `p6`/`%3`).
fn classify(seg: &str) -> Option<Level> {
    if prefixed_digits(seg, &['d']) {
        Some(Level::Daemon)
    } else if prefixed_digits(seg, &['w']) {
        Some(Level::Workspace)
    } else if prefixed_digits(seg, &['s']) {
        Some(Level::Session)
    } else if prefixed_digits(seg, &['y', '#']) {
        Some(Level::Window)
    } else if prefixed_digits(seg, &['p', '%']) {
        Some(Level::Pane)
    } else {
        None
    }
}

/// Normalizes a window token (`#2`, `y2`, `Y2`, `2`) to `y2`.
fn norm_window(v: &str) -> Result<String, CoordParseError> {
    let digits = v.strip_prefix(['#', 'y', 'Y']).unwrap_or(v);
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Ok(format!("y{digits}"))
    } else {
        err(format!("invalid window '{v}' (expected y2, #2 or 2)"))
    }
}

/// Validates a pane token (`%3` or `p6`); lowercases.
fn norm_pane(v: &str) -> Result<String, CoordParseError> {
    let l = v.to_ascii_lowercase();
    if prefixed_digits(&l, &['p', '%']) {
        Ok(l)
    } else {
        err(format!("invalid pane '{v}' (expected %3 or p6)"))
    }
}

impl TargetCoord {
    fn set(&mut self, level: Level, raw: &str) -> Result<(), CoordParseError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return err("empty coordinate component");
        }
        let val = if raw == "*" {
            None
        } else {
            Some(match level {
                Level::Window => norm_window(raw)?,
                Level::Pane => norm_pane(raw)?,
                _ => raw.to_string(),
            })
        };
        let slot = match level {
            Level::Daemon => &mut self.daemon,
            Level::Workspace => &mut self.workspace,
            Level::Session => &mut self.session,
            Level::Window => &mut self.window,
            Level::Pane => &mut self.pane,
        };
        if slot.is_some() {
            return err(format!("duplicate {level:?} component"));
        }
        *slot = val;
        Ok(())
    }

    /// Positional assignment: 4 = W,S,Y,P; 3 = W,S,Y; 2 = S,Y; 1 = Y.
    fn from_positional(segs: &[&str]) -> Result<Self, CoordParseError> {
        let levels: &[Level] = match segs.len() {
            1 => &[Level::Window],
            2 => &[Level::Session, Level::Window],
            3 => &[Level::Workspace, Level::Session, Level::Window],
            4 => &[Level::Workspace, Level::Session, Level::Window, Level::Pane],
            5 => &[Level::Daemon, Level::Workspace, Level::Session, Level::Window, Level::Pane],
            n => return err(format!("expected 1-5 coordinate components, got {n}")),
        };
        let mut c = Self::default();
        for (l, s) in levels.iter().zip(segs) {
            c.set(*l, s)?;
        }
        Ok(c)
    }

    fn from_path(s: &str) -> Result<Self, CoordParseError> {
        let segs: Vec<&str> = s.split([':', '/']).map(str::trim).collect();
        if segs.iter().any(|x| x.is_empty()) {
            return err("empty coordinate component");
        }
        if segs.len() > 5 {
            return err(format!("expected 1-5 coordinate components, got {}", segs.len()));
        }
        let classes: Vec<Option<Level>> = segs.iter().map(|x| classify(x)).collect();
        if classes.iter().all(Option::is_some) {
            let mut c = Self::default();
            let mut last: Option<Level> = None;
            for (l, seg) in classes.iter().flatten().zip(&segs) {
                if last.is_some_and(|p| p >= *l) {
                    return err(format!("component '{seg}' is out of order or duplicated"));
                }
                last = Some(*l);
                c.set(*l, seg)?;
            }
            return Ok(c);
        }
        // `d0:w1:main:y2[:%3]`: daemon prefix, then positional remainder.
        if segs.len() >= 3 && classes[0] == Some(Level::Daemon) {
            let mut c = Self::from_positional(&segs[1..])?;
            c.set(Level::Daemon, segs[0])?;
            return Ok(c);
        }
        Self::from_positional(&segs)
    }

    /// Parses the inside of a tuple (no outer parens): positional or labelled.
    fn from_tuple_body(body: &str) -> Result<Self, CoordParseError> {
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            return err("empty coordinate component");
        }
        let labelled = parts.iter().filter(|p| p.contains(':')).count();
        if labelled == 0 {
            if parts.len() < 2 || parts.len() > 4 {
                return err(format!("tuple needs 2-4 components, got {}", parts.len()));
            }
            return Self::from_positional(&parts);
        }
        if labelled != parts.len() {
            return err("cannot mix labelled and positional components");
        }
        let mut c = Self::default();
        for p in parts {
            let (label, val) = p.split_once(':').unwrap_or((p, ""));
            let level = match label.trim().to_ascii_lowercase().as_str() {
                "daemon" | "d" => Level::Daemon,
                "wksp" | "workspace" | "ws" | "w" => Level::Workspace,
                "sess" | "session" | "s" => Level::Session,
                "win" | "window" | "y" => Level::Window,
                "pane" | "p" => Level::Pane,
                other => return err(format!("unknown coordinate label '{other}'")),
            };
            c.set(level, val)?;
        }
        Ok(c)
    }

    /// Parses every accepted form (tuple, verbose tuple, daemon tuple, colon path, `#2`, `%3`).
    pub fn parse(s: &str) -> Result<Self, CoordParseError> {
        let s = s.trim();
        if s.is_empty() {
            return err("empty coordinate");
        }
        if !s.starts_with('(') && !s.ends_with(')') {
            return Self::from_path(s);
        }
        // Outer parens must be balanced and span the whole string.
        let mut depth = 0i32;
        for (i, ch) in s.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 || (depth == 0 && i + 1 != s.len()) {
                        return err("unbalanced parentheses");
                    }
                }
                _ => {}
            }
        }
        if depth != 0 || !s.starts_with('(') || !s.ends_with(')') {
            return err("unbalanced parentheses");
        }
        let inner = s[1..s.len() - 1].trim();
        // Daemon tuple: `D:(...)`.
        if let Some(open) = inner.find('(') {
            let head = inner[..open].trim();
            let Some(d) = head.strip_suffix(':') else {
                return err("expected 'D:(W,S,Y,P)'");
            };
            let rest = inner[open..].trim();
            if !rest.ends_with(')') {
                return err("unbalanced parentheses");
            }
            let body = &rest[1..rest.len() - 1];
            if body.contains(['(', ')']) {
                return err("nested parentheses");
            }
            let mut c = Self::from_tuple_body(body)?;
            c.set(Level::Daemon, d)?;
            return Ok(c);
        }
        Self::from_tuple_body(inner)
    }

    fn levels(&self) -> [&Option<String>; 5] {
        [&self.daemon, &self.workspace, &self.session, &self.window, &self.pane]
    }

    /// Formats per `style`; returns `None` for `Off`. Pane component omitted when `self.pane` is `None`.
    pub fn format(&self, style: CoordDisplayStyle) -> Option<String> {
        let star = |o: &Option<String>| o.clone().unwrap_or_else(|| "*".to_string());
        let (w, s, y) = (star(&self.workspace), star(&self.session), star(&self.window));
        let with_pane = |sep: &str, labelled: bool| {
            let mut out = if labelled {
                format!("Wksp: {w}{sep}Sess: {s}{sep}Win: {y}")
            } else {
                format!("{w}{sep}{s}{sep}{y}")
            };
            if let Some(p) = &self.pane {
                out.push_str(sep);
                if labelled {
                    out.push_str("Pane: ");
                }
                out.push_str(p);
            }
            out
        };
        match style {
            CoordDisplayStyle::Off => None,
            CoordDisplayStyle::Verbose => Some(format!("({})", with_pane(", ", true))),
            CoordDisplayStyle::Compact => Some(format!("({})", with_pane(",", false))),
            CoordDisplayStyle::DaemonTuple => {
                Some(format!("({}:({}))", star(&self.daemon), with_pane(",", false)))
            }
        }
    }

    /// Canonical colon path, e.g. `d0:w1:s1:y2:p1` (unspecified levels omitted).
    pub fn to_path(&self) -> String {
        let v: Vec<&str> = self.levels().iter().filter_map(|o| o.as_deref()).collect();
        v.join(":")
    }
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
    pub fn coord_of(&self, id: PaneId) -> Option<TargetCoord> {
        if !self.panes.contains_key(&id) {
            return None;
        }
        // No tmux `%pane_id` is exposed on `CompositePane`, so `pane` stays `None`.
        Some(TargetCoord {
            daemon: Some("d0".to_string()),
            workspace: Some("w1".to_string()),
            session: Some(self.config.history_session.clone()),
            window: Some(format!("y{id}")),
            pane: None,
        })
    }
    /// Resolves `target` (wildcards on `None` levels) to matching windows.
    ///
    /// Herdr compat: with no window and a `pN` pane, `N` is treated as the window id.
    /// A `pN` pane alongside a window matches the implicit single pane only when `N == 1`.
    pub fn resolve_coord(&self, target: &TargetCoord) -> CoordResolution {
        let eq = |want: &Option<String>, have: &Option<String>| {
            want.as_ref().is_none_or(|w| have.as_ref() == Some(w))
        };
        let (mut window, mut pane) = (target.window.clone(), target.pane.clone());
        if let (None, Some(n)) = (&window, pane.as_deref().filter(|p| p.starts_with('p'))) {
            window = Some(format!("y{}", &n[1..]));
            pane = None;
        }
        let mut out = CoordResolution { panes: Vec::new(), coords: Vec::new() };
        for &id in self.panes.keys() {
            let Some(c) = self.coord_of(id) else { continue };
            let pane_ok = match pane.as_deref() {
                None => true,
                Some(p) if p.starts_with('p') => c.pane.is_none() && p == "p1" || c.pane.as_deref() == Some(p),
                Some(_) => eq(&pane, &c.pane),
            };
            if eq(&target.daemon, &c.daemon)
                && eq(&target.workspace, &c.workspace)
                && eq(&target.session, &c.session)
                && eq(&window, &c.window)
                && pane_ok
            {
                out.panes.push(id);
                out.coords.push(c);
            }
        }
        out
    }
    /// Current title-bar badge style.
    pub fn coord_style(&self) -> CoordDisplayStyle { self.coord_style }
    /// Sets the title-bar badge style.
    pub fn set_coord_style(&mut self, style: CoordDisplayStyle) { self.coord_style = style; }
}

#[cfg(test)]
#[path = "coordinates_tests.rs"]
mod tests;
