//! Operations of the `:agents list` window verbs (Phase 51-01, UIT-03).
//!
//! Frozen by `51-CONTRACT.md` section 2.1: later plans build against these names; none edits this file.
//! The window itself is described in [`crate::wm::agents_list`].

use crate::pane::PaneId;

/// Which rows the AGENTS list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListFilter {
    /// Every tracked pane (default).
    #[default]
    All,
    /// Only panes with a detected agent (a matched profile or a reported state).
    Agents,
}

/// One `:agents list <verb>` operation, lowered to [`crate::action::Action::AgentsList`].
///
/// `None` on `Pin` / `Filter` / `Dock` means toggle. Absolute scale reuses
/// `Action::PanelScaleOf(PanelTarget::AgentsList, n)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsListOp {
    /// `:agents list open` - open the window (focused).
    Open,
    /// `:agents list close` (`q`, close badge) - close the window unconditionally.
    Close,
    /// `:agents list toggle` (`Ctrl+Alt+N`, leader `A`, menu) - close when open, else open.
    Toggle,
    /// `:agents list dismiss` (minimize badge) - close an unpinned window, or only release the keyboard of a pinned one.
    Dismiss,
    /// `:agents list blur` (Esc) - release the keyboard without closing.
    Blur,
    /// `:agents list select up|down` (Up / Down, `k` / `j`) - move the row selection.
    Step(i16),
    /// `:agents list select first` (Home) - select the first row.
    First,
    /// `:agents list select last` (End) - select the last row.
    Last,
    /// `:agents list select page-up|page-down` (PageUp / PageDown) - move the selection a page.
    Page(i16),
    /// `:agents list select <pane-id>` (click) - select the row of a pane.
    SelectPane(PaneId),
    /// `:agents list jump [pane-id]` (Enter, click) - focus the (selected) pane; the list stays open, blurred.
    Jump(Option<PaneId>),
    /// `:agents list zoom [pane-id]` (`z`, right-click, `[zoom]` chip) - open the traffic window on a pane.
    Zoom(Option<PaneId>),
    /// `:agents list pin` / `unpin` / `pin toggle` (`*`, title pin badge) - pin the window open.
    Pin(Option<bool>),
    /// `:agents list scale +10` / `-10` (`]` / `[`, Ctrl+wheel) - step the window's own panel scale.
    ScaleStep(i16),
    /// `:agents list filter all|agents|toggle` (`f`, `[filter]` chip) - choose the rows shown.
    Filter(Option<ListFilter>),
    /// `:agents list dock` / `float` / `dock toggle` (`d`, `[dock]` chip) - dock into the tiling strip or float.
    Dock(Option<bool>),
    /// `:agents list move <dx> <dy>` (Shift+arrows) - move the window by cells.
    Move { dx: i16, dy: i16 },
    /// `:agents list resize <dw> <dh>` (Alt+arrows) - grow or shrink the window by cells.
    Resize { dw: i16, dh: i16 },
    /// `:agents list rect <x> <y> <w> <h>` (drag motion) - set the window rect (clamped to the workspace).
    SetRect { x: u16, y: u16, w: u16, h: u16 },
    /// `:agents list size <cols>x<rows>` / `size auto` - set the size, or back to the default.
    Size(Option<(u16, u16)>),
    /// `:agents list scroll <±n>` (wheel) - scroll the rows.
    Scroll(i16),
}

impl AgentsListOp {
    /// The canonical colon form (the inverse of the `:agents list` parser, 51-CONTRACT.md section 3).
    pub fn colon_form(&self) -> String {
        let p = ":agents list";
        match self {
            Self::Open => format!("{p} open"),
            Self::Close => format!("{p} close"),
            Self::Toggle => format!("{p} toggle"),
            Self::Dismiss => format!("{p} dismiss"),
            Self::Blur => format!("{p} blur"),
            Self::Step(n) if *n < 0 => format!("{p} select up"),
            Self::Step(_) => format!("{p} select down"),
            Self::First => format!("{p} select first"),
            Self::Last => format!("{p} select last"),
            Self::Page(n) if *n < 0 => format!("{p} select page-up"),
            Self::Page(_) => format!("{p} select page-down"),
            Self::SelectPane(id) => format!("{p} select {id}"),
            Self::Jump(None) => format!("{p} jump"),
            Self::Jump(Some(id)) => format!("{p} jump {id}"),
            Self::Zoom(None) => format!("{p} zoom"),
            Self::Zoom(Some(id)) => format!("{p} zoom {id}"),
            Self::Pin(Some(true)) => format!("{p} pin"),
            Self::Pin(Some(false)) => format!("{p} unpin"),
            Self::Pin(None) => format!("{p} pin toggle"),
            Self::ScaleStep(n) => format!("{p} scale {n:+}"),
            Self::Filter(Some(ListFilter::All)) => format!("{p} filter all"),
            Self::Filter(Some(ListFilter::Agents)) => format!("{p} filter agents"),
            Self::Filter(None) => format!("{p} filter toggle"),
            Self::Dock(Some(true)) => format!("{p} dock"),
            Self::Dock(Some(false)) => format!("{p} float"),
            Self::Dock(None) => format!("{p} dock toggle"),
            Self::Move { dx, dy } => format!("{p} move {dx} {dy}"),
            Self::Resize { dw, dh } => format!("{p} resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!("{p} rect {x} {y} {w} {h}"),
            Self::Size(None) => format!("{p} size auto"),
            Self::Size(Some((c, r))) => format!("{p} size {c}x{r}"),
            Self::Scroll(n) => format!("{p} scroll {n:+}"),
        }
    }
}
