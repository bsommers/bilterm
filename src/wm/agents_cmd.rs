//! Operations of the `:agents` command family (Phase 50-03, FLT-01).
//!
//! `:agents` (plural) is the Phase 50 view command; `:agent` (singular) is reserved for the
//! Phase 51 messaging family. Later Phase 50 plans extend this enum.

/// One `:agents` operation, lowered to [`crate::action::Action::Agents`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsOp {
    /// `:agents [list]` - per-pane agent state plus a count summary (data over RPC).
    List,
    /// `:agents zoom [pane-id]` - open the AGENT TRAFFIC window, optionally selecting a pane node.
    Zoom(Option<crate::pane::PaneId>),
    /// `:agents close` - close the window unconditionally.
    Close,
    /// `:agents dismiss` (Esc) - close an unpinned window, or only release the keyboard of a pinned one.
    Dismiss,
    /// `:agents select left|right|up|down` (arrow keys) - move the node cursor.
    Select(crate::wm::agents_window::GraphDir),
    /// `:agents edge next|prev` (Tab / Shift+Tab) - cycle the selected node's incident edges.
    CycleEdge(bool),
    /// `:agents enter` (Enter) - focus the selected pane node.
    Activate,
    /// `:agents move <dx> <dy>` (Shift+arrows) - move the window by cells.
    Move { dx: i16, dy: i16 },
    /// `:agents resize <dw> <dh>` (Alt+arrows) - grow or shrink the window by cells.
    Resize { dw: i16, dh: i16 },
    /// `:agents rect <x> <y> <w> <h>` (drag motion) - set the window rect (clamped to the workspace).
    SetRect { x: u16, y: u16, w: u16, h: u16 },
    /// `:agents size <cols>x<rows>` / `:agents size auto` - set the size, or back to the default.
    Size(Option<(u16, u16)>),
    /// `:agents max` (`m`, title badge) - toggle maximize.
    Maximize,
    /// `:agents split <20..80>` (splitter drag) - graph pane share, percent.
    Split(u16),
    /// `:agents split -5` / `+5` (`<` / `>`) - step the splitter.
    SplitStep(i16),
    /// `:agents select <node-key>` (click) - select a node.
    SelectNode(crate::wm::agents_window::NodeKey),
    /// `:agents edge <a> <b>` (click) - select the edge between two nodes.
    SelectEdge(crate::wm::agents_window::NodeKey, crate::wm::agents_window::NodeKey),
    /// `:agents grab [on|off]` (`g`) - toggle, enter or leave node grab mode.
    Grab(Option<bool>),
    /// `:agents nudge <left|right|up|down>` (arrows in grab mode) - move the selected node one cell.
    NudgeNode(crate::wm::agents_window::GraphDir),
    /// `:agents node-pin <key> <x> <y>` (node drag) - pin a node at a permille position (0..=1000).
    PinNode { key: crate::wm::agents_window::NodeKey, x_pm: u16, y_pm: u16 },
    /// `:agents node-unpin [key]` (`u`, right-click) - release a pinned node (none = selected).
    UnpinNode(Option<crate::wm::agents_window::NodeKey>),
    /// `:agents pin` / `unpin` / `pin toggle` (`*`, title pin badge) - pin the window open (`None` toggles).
    Pin(Option<bool>),
    /// `:agents scale +10` / `-10` (`]` / `[`, Ctrl+wheel) - step the window's own panel scale (GUI only).
    ScaleStep(i16),
    /// `:agents focus [graph|snoop|compose]` (Ctrl+O, click) - focus a window pane; `None` cycles.
    FocusPane(Option<crate::wm::agents_window::WindowPane>),
    /// `:agents line +n` / `-n` (`j` / `k`) - move the conversation line selection.
    SnoopStep(i16),
    /// `:agents line <seq>` (click) - select a conversation line by bus sequence number.
    SnoopSelect(u64),
    /// `:agents copy` (`y`, `[copy]` badge) - copy the selected conversation line.
    Copy,
    /// `:agents scroll <±n>` / `top` (PageUp / PageDown / Home, wheel) - scroll the conversation
    /// history (`i16::MIN` = oldest); scrolling up turns follow off.
    SnoopScroll(i16),
    /// `:agents follow [on|off]` (`f`, End, follow marker) - follow new traffic (`None` toggles).
    Follow(Option<bool>),
    /// `:agents filter ...` (`a` `p` `c` `A` `P` `T`, header buttons, chips) - conversation filters.
    Filter(SnoopFilterOp),
    /// Keyword entry (`/` then typing): text-field editing; the committed form is
    /// `:agents filter text <kw>`.
    KeywordEdit(KeywordEdit),
}

/// Which conversation filter a chip removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnoopFilterKind {
    Agent,
    Pair,
    Text,
}

/// One conversation filter operation (filters combine with AND).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnoopFilterOp {
    /// Keep events sent or received by this node (label or node key).
    Agent(String),
    /// Keep events between two nodes (either direction).
    Pair(String, String),
    /// Keep events whose preview contains this text (case-insensitive, no regex).
    Text(String),
    /// Remove every filter.
    Clear,
    /// Remove one filter.
    Remove(SnoopFilterKind),
    /// Agent filter from the sender of the selected line.
    AgentFromSelection,
    /// Pair filter from the endpoints of the selected line.
    PairFromSelection,
}

/// One edit of the keyword entry line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordEdit {
    /// Open an empty entry.
    Begin,
    Push(char),
    Pop,
    /// Apply the entry as the text filter and close it.
    Commit,
    /// Close the entry, filters unchanged.
    Cancel,
}
