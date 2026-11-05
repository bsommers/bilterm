//! Navigator-style drawer renderer: one buffer line per workspace > session > pane row (`FLT-01`).
//!
//! Called from [`super::render_workspace_drawer`] via a single early-return branch, so the classic
//! renderer is never touched when `drawer.style = classic`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::{fill_rect, write_clipped_line};
use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::{GlyphPack, GlyphSet};
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::agent_state::{AgentCounts, AgentState};
use crate::vt::sanitize_display;
use crate::wm::drawer::{BRAILLE_SPINNER_FRAMES, CommsIndicator, NavigatorLevel, NavigatorMenuEntry};
use crate::wm::{DrawerLayoutMode, DrawerRow, DrawerRowKind, WindowManager, inset_window_frame};

/// Computes `(header_rows, footer_rows, rows_per_item)` for the navigator layout: no header, an optional
/// one-line footer and exactly one line per item.
pub fn navigator_layout_metrics(inner_height: u16) -> (u16, u16, usize) {
    (0, u16::from(inner_height >= 4), 1)
}

/// Single-width BMP symbols used by the navigator tree (no emoji; RESEARCH anti-pattern).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigatorGlyphs {
    pub current: &'static str,
    pub fold_open: &'static str,
    pub fold_closed: &'static str,
    pub leaf: &'static str,
    pub ellipsis: &'static str,
    /// Animated braille spinner for `Working` (Unicode) or the static `working` glyph (ASCII).
    pub spinner: bool,
    pub working: &'static str,
    pub blocked: &'static str,
    pub idle: &'static str,
    pub done: &'static str,
    pub errored: &'static str,
    /// Pairing comms glyph, and its alternate frame shown on odd ticks while traffic flows.
    pub pair: &'static str,
    pub pair_alt: &'static str,
    /// Fan-out comms glyph (followed by the partner count).
    pub fan: &'static str,
}

impl NavigatorGlyphs {
    pub const UNICODE: Self = Self {
        current: "›",
        fold_open: "▾",
        fold_closed: "▸",
        leaf: "·",
        ellipsis: "…",
        spinner: true,
        working: "⠋",
        blocked: "▲",
        idle: "○",
        done: "●",
        errored: "✗",
        pair: "⇄",
        pair_alt: "⇆",
        fan: "⋔",
    };
    pub const ASCII: Self = Self {
        current: ">",
        fold_open: "-",
        fold_closed: "+",
        leaf: ".",
        ellipsis: "~",
        spinner: false,
        working: "*",
        blocked: "!",
        idle: "-",
        done: "+",
        errored: "x",
        pair: "<>",
        pair_alt: "><",
        fan: ">>",
    };

    /// State dot glyph; `Working` animates with `tick` in the Unicode pack.
    pub fn dot(&self, state: AgentState, tick: usize) -> String {
        match state {
            AgentState::Working if self.spinner => {
                BRAILLE_SPINNER_FRAMES[tick % BRAILLE_SPINNER_FRAMES.len()].to_string()
            }
            AgentState::Working => self.working.to_string(),
            AgentState::Blocked => self.blocked.to_string(),
            AgentState::Idle => self.idle.to_string(),
            AgentState::Done => self.done.to_string(),
            AgentState::Errored => self.errored.to_string(),
        }
    }

    /// Table for the configured glyph pack (ASCII pack -> ASCII table, everything else Unicode).
    pub const fn for_pack(pack: GlyphPack) -> Self {
        match pack {
            GlyphPack::Ascii => Self::ASCII,
            _ => Self::UNICODE,
        }
    }
}

/// Dot colour per state (design tokens: working=success, blocked=warning, idle=muted, done=accent,
/// errored=danger).
pub fn dot_color(state: AgentState, t: &DesignTokens) -> Color {
    match state {
        AgentState::Working => t.success,
        AgentState::Blocked => t.warning,
        AgentState::Idle => t.text_muted,
        AgentState::Done => t.accent,
        AgentState::Errored => t.danger,
    }
}

/// Non-zero states of `counts` as `<glyph><n>` segments in Blocked, Errored, Working, Done, Idle order.
pub fn counts_segments(counts: &AgentCounts, g: &NavigatorGlyphs, tick: usize) -> Vec<(AgentState, String)> {
    [
        (AgentState::Blocked, counts.blocked),
        (AgentState::Errored, counts.errored),
        (AgentState::Working, counts.working),
        (AgentState::Done, counts.done),
        (AgentState::Idle, counts.idle),
    ]
    .into_iter()
    .filter(|&(_, n)| n > 0)
    .map(|(state, n)| (state, format!("{}{n}", g.dot(state, tick))))
    .collect()
}

/// Longest prefix of `segments` that fits `space` columns (segments separated by one space).
fn fit_segments(segments: Vec<(AgentState, String)>, space: usize) -> Vec<(AgentState, String)> {
    let mut used = 0;
    let mut keep = 0;
    for (i, (_, text)) in segments.iter().enumerate() {
        let need = text.width() + usize::from(i > 0);
        if used + need > space {
            break;
        }
        used += need;
        keep = i + 1;
    }
    segments.into_iter().take(keep).collect()
}

/// Display columns reserved before a pane label for the state dot (`dot` + space).
const DOT_COLS: usize = 2;

/// Geometry shared by the renderer and [`navigator_dot_cells`].
struct NavigatorLayout {
    inner: Rect,
    rows: Vec<DrawerRow>,
    selected: usize,
    scroll: usize,
    cap: usize,
    list_y: u16,
    list_bottom: u16,
    row_step: usize,
    /// Screen row of the footer button bar, `None` when the inner height is below 4.
    footer_y: Option<u16>,
}

fn navigator_layout(wm: &WindowManager, drawer_rect: Rect) -> Option<NavigatorLayout> {
    let inner = inset_window_frame(drawer_rect);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let rows = wm.drawer_rows();
    let selected = wm.drawer().selected_index.min(rows.len().saturating_sub(1));
    let (header_h, footer_h, row_step) = navigator_layout_metrics(inner.height);
    let list_h = usize::from(inner.height.saturating_sub(header_h + footer_h)).max(1);
    let cap = (list_h / row_step).max(1);
    let scroll = wm.drawer().effective_scroll(selected, cap, rows.len());
    Some(NavigatorLayout {
        inner,
        rows,
        selected,
        scroll,
        cap,
        list_y: inner.y + header_h,
        list_bottom: inner.y + inner.height.saturating_sub(footer_h),
        row_step,
        footer_y: (footer_h >= 1).then(|| inner.y + inner.height - 1),
    })
}

/// Screen cell of one pane state dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigatorDotCell {
    pub x: u16,
    pub y: u16,
    pub state: AgentState,
    /// Index into [`WindowManager::drawer_rows`].
    pub row_index: usize,
}

/// Cells of every visible pane state dot; the single source of dot positions (TUI renderer here,
/// GUI icon pass in a later plan). Empty when the drawer is not visible.
pub fn navigator_dot_cells(wm: &WindowManager, workspace_area: Rect) -> Vec<NavigatorDotCell> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    let Some(lay) = navigator_layout(wm, drawer_rect) else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    for (vis_idx, (row_index, row)) in lay.rows.iter().enumerate().skip(lay.scroll).take(lay.cap).enumerate() {
        let y = lay.list_y + (vis_idx * lay.row_step) as u16;
        if y >= lay.list_bottom {
            break;
        }
        let Some(meta) = row.navigator.as_ref() else { continue };
        let Some(state) = meta.agent else { continue };
        let x = lay.inner.x + 1 + 2 * u16::from(meta.depth);
        if x < lay.inner.x + lay.inner.width {
            cells.push(NavigatorDotCell { x, y, state, row_index });
        }
    }
    cells
}

/// Placement of one row's comms indicator (glyph plus partner label or count), right-aligned in the row.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CommsFit {
    text: String,
    /// Display columns of `text` (the indicator occupies the last `width` columns of the row).
    width: usize,
    /// Left-text budget (columns after the marker column) that leaves one blank column before the indicator.
    left_width: usize,
}

/// Fits the comms indicator of a pane row into `width` columns. The title truncates first (down to its
/// `indent + dot + #id` prefix plus one column), then the partner label; the glyph is never dropped unless
/// the row cannot even hold it. `None` for rows without an indicator.
fn comms_fit(row: &DrawerRow, g: &NavigatorGlyphs, tick: usize, width: usize) -> Option<CommsFit> {
    let meta = row.navigator.as_ref()?;
    let (DrawerRowKind::Pane(id), Some(comms)) = (&row.kind, meta.comms.as_ref()) else { return None };
    let (glyph, label) = match comms {
        CommsIndicator::Pair { partner_label, .. } => {
            (if meta.comms_active && tick % 2 == 1 { g.pair_alt } else { g.pair }, partner_label.clone())
        }
        CommsIndicator::FanOut { count } => (g.fan, count.to_string()),
    };
    let avail = width.saturating_sub(1);
    let prefix = 2 * usize::from(meta.depth) + DOT_COLS + format!("#{id} ").width();
    let glyph_w = glyph.width();
    if avail < glyph_w {
        return None;
    }
    let label_w = label.width();
    let label = (0..=label_w)
        .rev()
        .map(|n| fit_width(&label, n, g.ellipsis))
        .find(|l| avail > glyph_w + l.width() + 1 + prefix)
        .unwrap_or_default();
    let text = format!("{glyph}{label}");
    let w = text.width();
    Some(CommsFit { text, width: w, left_width: avail.saturating_sub(w + 1) })
}

/// Screen cell span of one row's comms indicator (the whole indicator is the click target).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigatorCommsCell {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    /// Index into [`WindowManager::drawer_rows`].
    pub row_index: usize,
}

/// Cells of every visible comms indicator; the single source for the renderer and the mouse hit-test.
/// Empty when the drawer is not visible.
pub fn navigator_comms_cells(wm: &WindowManager, workspace_area: Rect) -> Vec<NavigatorCommsCell> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    let Some(lay) = navigator_layout(wm, drawer_rect) else {
        return Vec::new();
    };
    let g = NavigatorGlyphs::for_pack(wm.config().glyphs);
    let tick = wm.drawer().spinner_tick();
    let width = usize::from(lay.inner.width);
    let mut cells = Vec::new();
    for (vis_idx, (row_index, row)) in lay.rows.iter().enumerate().skip(lay.scroll).take(lay.cap).enumerate() {
        let y = lay.list_y + (vis_idx * lay.row_step) as u16;
        if y >= lay.list_bottom {
            break;
        }
        if let Some(fit) = comms_fit(row, &g, tick, width) {
            cells.push(NavigatorCommsCell {
                x: lay.inner.x + (width - fit.width) as u16,
                y,
                width: fit.width as u16,
                row_index,
            });
        }
    }
    cells
}

/// Screen cell of one fold marker (workspace or session row that has children).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigatorCell {
    pub x: u16,
    pub y: u16,
    /// Index into [`WindowManager::drawer_rows`].
    pub row_index: usize,
}

/// Cells of every visible fold marker; the single source for the renderer's glyph column and the mouse
/// hit-test. Empty when the drawer is not visible.
pub fn navigator_fold_marker_cells(wm: &WindowManager, workspace_area: Rect) -> Vec<NavigatorCell> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    let Some(lay) = navigator_layout(wm, drawer_rect) else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    for (vis_idx, (row_index, row)) in lay.rows.iter().enumerate().skip(lay.scroll).take(lay.cap).enumerate() {
        let y = lay.list_y + (vis_idx * lay.row_step) as u16;
        if y >= lay.list_bottom {
            break;
        }
        let Some(meta) = row.navigator.as_ref() else { continue };
        if !matches!(meta.level, NavigatorLevel::Workspace | NavigatorLevel::Session) || !meta.has_children {
            continue;
        }
        let x = lay.inner.x + 1 + 2 * u16::from(meta.depth);
        if x < lay.inner.x + lay.inner.width {
            cells.push(NavigatorCell { x, y, row_index });
        }
    }
    cells
}

/// Operation a footer button triggers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigatorButtonKind {
    Pin,
    Attention,
    NewSession,
    NewPane,
    Rename,
    Close,
    Zoom,
}

impl NavigatorButtonKind {
    /// Buttons in left-to-right order; later ones are dropped first when the drawer is narrow.
    pub const ALL: [Self; 7] =
        [Self::Pin, Self::Attention, Self::NewSession, Self::NewPane, Self::Rename, Self::Close, Self::Zoom];

    /// Label: the key character that triggers the same operation, in both glyph packs.
    pub const fn label(self) -> char {
        match self {
            Self::Pin => '*',
            Self::Attention => '!',
            Self::NewSession => 'N',
            Self::NewPane => '+',
            Self::Rename => 'r',
            Self::Close => 'x',
            Self::Zoom => '@',
        }
    }

    /// Wider text shown when there is room (the last button toggles the agent traffic window and says
    /// so); the button falls back to the single-cell `label` otherwise.
    pub const fn wide_text(self) -> Option<&'static str> {
        match self {
            Self::Zoom => Some("@ agents"),
            _ => None,
        }
    }
}

/// One rendered footer button: `width` cells starting at `(x, y)` (label cell plus one padding cell).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigatorButton {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub kind: NavigatorButtonKind,
}

/// Footer buttons that fit the drawer, in order; the single source for the renderer and the hit-test.
/// Empty when the drawer is not visible or has no footer row (inner height below 4).
pub fn navigator_footer_buttons(wm: &WindowManager, workspace_area: Rect) -> Vec<NavigatorButton> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    let Some(lay) = navigator_layout(wm, drawer_rect) else {
        return Vec::new();
    };
    footer_buttons_in(&lay)
}

fn footer_buttons_in(lay: &NavigatorLayout) -> Vec<NavigatorButton> {
    let Some(y) = lay.footer_y else { return Vec::new() };
    let right = lay.inner.x + lay.inner.width;
    NavigatorButtonKind::ALL
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let x = lay.inner.x + 1 + 2 * i as u16;
            let wide = kind.wide_text().map_or(2, |t| t.len() as u16 + 1);
            NavigatorButton { x, y, width: if x + wide <= right { wide } else { 2 }, kind }
        })
        .take_while(|b| b.x + b.width <= right)
        .collect()
}

/// Display columns of the `[+]` add button.
const PLUS_COLS: u16 = 3;

/// Title text of the navigator drawer frame.
fn navigator_title(is_modern: bool) -> &'static str {
    if is_modern { "◢ WORKSPACES" } else { "WORKSPACES" }
}

/// Columns (relative to the inner left edge) where the left text of a row ends and where its right-aligned
/// rollup or state counts begin, mirroring exactly what [`render_navigator_drawer`] draws.
fn row_extent(row: &DrawerRow, g: &NavigatorGlyphs, tick: usize, width: usize) -> (usize, usize) {
    let (left, rollup) = row_parts(row, g);
    let avail = width.saturating_sub(1);
    let segments = row
        .navigator
        .as_ref()
        .and_then(|m| m.counts.as_ref())
        .map(|c| counts_segments(c, g, tick))
        .unwrap_or_default();
    if segments.is_empty() {
        let rollup_w = rollup.as_ref().map_or(0, |r| r.width() + 1);
        let rollup = rollup.filter(|_| width >= rollup_w + 6);
        let left_w = avail.saturating_sub(if rollup.is_some() { rollup_w } else { 0 });
        let right = rollup.map_or(width, |r| width - r.width());
        (1 + fit_width(&left, left_w, g.ellipsis).width(), right)
    } else {
        let space = avail.saturating_sub(left.width() + 1);
        let fitted = fit_segments(segments, space);
        let total: usize = fitted.iter().map(|(_, t)| t.width()).sum::<usize>() + fitted.len().saturating_sub(1);
        (1 + fit_width(&left, avail, g.ellipsis).width(), width - total.min(width))
    }
}

/// `[+]` cells of the visible workspace and session rows: one gap column after the label, only where the
/// button fits before the row's right-aligned rollup or counts (`x` is the first of `PLUS_COLS` cells).
fn plus_cells_in(wm: &WindowManager, lay: &NavigatorLayout) -> Vec<NavigatorCell> {
    let g = NavigatorGlyphs::for_pack(wm.config().glyphs);
    let tick = wm.drawer().spinner_tick();
    let width = usize::from(lay.inner.width);
    let mut cells = Vec::new();
    for (vis_idx, (row_index, row)) in lay.rows.iter().enumerate().skip(lay.scroll).take(lay.cap).enumerate() {
        let y = lay.list_y + (vis_idx * lay.row_step) as u16;
        if y >= lay.list_bottom {
            break;
        }
        let is_session = row
            .navigator
            .as_ref()
            .is_some_and(|m| matches!(m.level, NavigatorLevel::Workspace | NavigatorLevel::Session));
        if !is_session {
            continue;
        }
        let (left_end, right_start) = row_extent(row, &g, tick, width);
        let col = left_end + 1;
        // Keep one blank column before right-aligned content (none needed when the row has none).
        let gap = usize::from(right_start < width);
        if col + usize::from(PLUS_COLS) + gap <= right_start {
            cells.push(NavigatorCell { x: lay.inner.x + col as u16, y, row_index });
        }
    }
    cells
}

/// `[+]` buttons of the visible workspace and session rows: clicking one adds a window to that session.
/// A row without room before its rollup or counts has no button.
pub fn navigator_plus_cells(wm: &WindowManager, workspace_area: Rect) -> Vec<NavigatorCell> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    navigator_layout(wm, drawer_rect).map(|lay| plus_cells_in(wm, &lay)).unwrap_or_default()
}

/// Rect (3x1) of the header `[+]` button on the title row, right after the title text: clicking it creates
/// a session. `None` when the title row has no room for it before the title-bar badges.
pub fn navigator_header_plus(wm: &WindowManager, workspace_area: Rect) -> Option<Rect> {
    let dr = wm.drawer_rect(workspace_area)?;
    let title_w = navigator_title(wm.config().is_modern()).width() as u16;
    let x = dr.x + 1 + title_w + 2 + 1;
    let limit = if dr.width >= 24 {
        let g = wm.config().glyph_set();
        let badges = (g.minimize_badge.chars().count() + g.zoom_badge.chars().count() + g.close_badge.chars().count()) as u16;
        (dr.x + dr.width).saturating_sub(badges + 2)
    } else {
        dr.x + dr.width.saturating_sub(1)
    };
    (dr.width >= 4 && x + PLUS_COLS <= limit).then(|| Rect::new(x, dr.y, PLUS_COLS, 1))
}

/// Placement of the open context menu: a title line (the selected row's name) followed by the entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigatorMenuLayout {
    pub rect: Rect,
    /// `(entry, screen row)` of every visible entry, in display order.
    pub entries: Vec<(NavigatorMenuEntry, u16)>,
}

fn menu_layout_in(wm: &WindowManager, lay: &NavigatorLayout) -> Option<NavigatorMenuLayout> {
    let menu = wm.navigator_menu()?;
    let inner = lay.inner;
    let longest = NavigatorMenuEntry::ALL.iter().map(|e| e.label().width()).max().unwrap_or(0) as u16;
    let w = (longest + 2).min(inner.width);
    let h = (NavigatorMenuEntry::ALL.len() as u16 + 1).min(inner.height);
    if w < 4 || h < 2 {
        return None;
    }
    let (ax, ay) = menu.anchor().unwrap_or_else(|| {
        let vis = lay.selected.saturating_sub(lay.scroll).min(lay.cap.saturating_sub(1));
        (inner.x + 1, lay.list_y + (vis * lay.row_step) as u16)
    });
    let x = ax.clamp(inner.x, inner.x + inner.width - w);
    let bottom = inner.y + inner.height;
    let below = ay.saturating_add(1);
    let y = if below + h <= bottom { below.max(inner.y) } else { bottom.saturating_sub(h).max(inner.y) };
    let entries = NavigatorMenuEntry::ALL
        .iter()
        .take(usize::from(h - 1))
        .enumerate()
        .map(|(i, &e)| (e, y + 1 + i as u16))
        .collect();
    Some(NavigatorMenuLayout { rect: Rect::new(x, y, w, h), entries })
}

/// Placement of the open context menu; `None` when no menu is open or the drawer is too small for it.
pub fn navigator_menu_layout(wm: &WindowManager, workspace_area: Rect) -> Option<NavigatorMenuLayout> {
    let drawer_rect = wm.drawer_rect(workspace_area)?;
    let lay = navigator_layout(wm, drawer_rect)?;
    menu_layout_in(wm, &lay)
}

/// Index into [`NavigatorMenuEntry::ALL`] of the menu entry under `(col, row)`.
pub fn navigator_menu_entry_at(wm: &WindowManager, workspace_area: Rect, col: u16, row: u16) -> Option<usize> {
    let menu = navigator_menu_layout(wm, workspace_area)?;
    if col < menu.rect.x || col >= menu.rect.x + menu.rect.width {
        return None;
    }
    menu.entries.iter().find(|&&(_, y)| y == row).and_then(|&(e, _)| NavigatorMenuEntry::ALL.iter().position(|&a| a == e))
}

/// Index into [`WindowManager::drawer_rows`] of the list row under screen cell `(col, row)`, `None` outside
/// the rendered list rows. Uses the same layout as [`render_navigator_drawer`].
pub fn navigator_row_at(wm: &WindowManager, workspace_area: Rect, col: u16, row: u16) -> Option<usize> {
    let drawer_rect = wm.drawer_rect(workspace_area)?;
    let lay = navigator_layout(wm, drawer_rect)?;
    if col < lay.inner.x || col >= lay.inner.x + lay.inner.width || row < lay.list_y || row >= lay.list_bottom {
        return None;
    }
    let vis_idx = usize::from(row - lay.list_y) / lay.row_step;
    if vis_idx >= lay.cap {
        return None;
    }
    let idx = lay.scroll + vis_idx;
    (idx < lay.rows.len()).then_some(idx)
}

/// Truncates `text` to at most `max_w` display columns, ending with `ellipsis` when cut.
fn fit_width(text: &str, max_w: usize, ellipsis: &str) -> String {
    if text.width() <= max_w {
        return text.to_string();
    }
    let budget = max_w.saturating_sub(ellipsis.width());
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(ch);
    }
    if max_w >= ellipsis.width() {
        out.push_str(ellipsis);
    }
    out
}

/// Left text (after the marker column) and optional right-aligned rollup of one navigator row.
fn row_parts(row: &DrawerRow, g: &NavigatorGlyphs) -> (String, Option<String>) {
    let meta = row.navigator.as_ref();
    let indent = "  ".repeat(meta.map_or(0, |m| usize::from(m.depth)));
    let label = match &row.kind {
        DrawerRowKind::Pane(id) => format!("{}#{id} {}", " ".repeat(DOT_COLS), row.title),
        _ => row.title.clone(),
    };
    let fold = match meta {
        Some(m) if matches!(m.level, NavigatorLevel::Workspace | NavigatorLevel::Session) => {
            let glyph = if !m.has_children {
                g.leaf
            } else if m.collapsed {
                g.fold_closed
            } else {
                g.fold_open
            };
            format!("{glyph} ")
        }
        _ => String::new(),
    };
    let rollup = meta
        .filter(|m| matches!(m.level, NavigatorLevel::Workspace | NavigatorLevel::Session) && (m.live || m.pane_count > 0))
        .map(|m| if m.live { format!("{}p", m.pane_count) } else { format!("{}p -", m.pane_count) });
    (format!("{indent}{fold}{label}"), rollup)
}

/// Renders the navigator-style drawer into `drawer_rect` (`FLT-01`).
pub fn render_navigator_drawer(
    wm: &WindowManager,
    workspace_area: Rect,
    drawer_rect: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let dcfg = wm.drawer_config();
    let drawer = wm.drawer();
    let is_modern = cfg.is_modern();

    if dcfg.layout == DrawerLayoutMode::Overlay {
        if cfg.liquid_glass {
            render_liquid_ambient_halo(buf, drawer_rect, workspace_area, palette);
        } else if cfg.drop_shadows {
            render_drop_shadow_styled(buf, drawer_rect, workspace_area, cfg.shadow_style, palette);
        }
    }
    occlude_rect(buf, drawer_rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, drawer_rect, palette.window_frame_bg, palette);
    }

    let title = navigator_title(is_modern);
    render_borland_frame_styled(
        drawer_rect,
        title,
        drawer.focused,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    let Some(lay) = navigator_layout(wm, drawer_rect) else {
        return;
    };
    let buttons = footer_buttons_in(&lay);
    let plus_cells = plus_cells_in(wm, &lay);
    let menu_layout = menu_layout_in(wm, &lay);
    let NavigatorLayout { inner, rows, selected, scroll, cap, list_y, list_bottom, row_step, .. } = lay;
    let base_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.inactive_border_fg))
        .bg(palette.window_frame_bg);
    if !cfg.is_glass() {
        fill_rect(buf, inner, base_style);
    }

    let hg = NavigatorGlyphs::for_pack(cfg.glyphs);
    let tokens = DesignTokens::from_palette(palette);
    let tick = drawer.spinner_tick();
    let footer_h = navigator_layout_metrics(inner.height).1;

    for (vis_idx, (row_idx, row)) in rows.iter().enumerate().skip(scroll).take(cap).enumerate() {
        let y = list_y + (vis_idx * row_step) as u16;
        if y >= list_bottom {
            break;
        }
        let mut style = if row.is_focused_pane {
            base_style.fg(palette.title_fg).add_modifier(Modifier::BOLD)
        } else {
            base_style
        };
        if row_idx == selected {
            style = if is_modern {
                Style::default()
                    .fg(palette.title_fg)
                    .bg(Color::Rgb(30, 41, 59))
                    .add_modifier(Modifier::BOLD)
            } else {
                style.add_modifier(Modifier::REVERSED)
            };
            fill_rect(buf, Rect::new(inner.x, y, inner.width, 1), style);
        }
        let marker = if row.is_focused_pane { hg.current } else { " " };
        let (left, rollup) = row_parts(row, &hg);
        let plus = plus_cells.iter().find(|c| c.row_index == row_idx);
        let width = usize::from(inner.width);
        let avail = width.saturating_sub(1);
        let segments = row
            .navigator
            .as_ref()
            .and_then(|m| m.counts.as_ref())
            .map(|c| counts_segments(c, &hg, tick))
            .unwrap_or_default();
        if segments.is_empty() {
            let rollup_w = rollup.as_ref().map_or(0, |r| r.width() + 1);
            // Keep the rollup only when it leaves room for the marker plus a few label columns.
            let rollup = rollup.filter(|_| width >= rollup_w + 6);
            let comms = comms_fit(row, &hg, tick, width);
            let left_w = match &comms {
                Some(c) => c.left_width,
                None => avail.saturating_sub(if rollup.is_some() { rollup_w } else { 0 }),
            };
            let line = format!("{marker}{}", fit_width(&left, left_w, hg.ellipsis));
            write_clipped_line(buf, inner.x, y, inner.width, &line, style);
            if let Some(c) = comms {
                let active = row.navigator.as_ref().is_some_and(|m| m.comms_active);
                let focused = row_idx == selected && wm.navigator_comms_focused();
                let mut cs = style;
                if active {
                    cs = cs.fg(tokens.accent);
                    if matches!(row.navigator.as_ref().and_then(|m| m.comms.as_ref()), Some(CommsIndicator::FanOut { .. }))
                        && tick.is_multiple_of(2)
                    {
                        cs = cs.add_modifier(Modifier::BOLD);
                    }
                    if row_idx == selected && !is_modern {
                        // Keep the reversed selection bar continuous behind the accent glyph.
                        cs = cs.bg(palette.text_fg.unwrap_or(palette.inactive_border_fg)).remove_modifier(Modifier::REVERSED);
                    }
                }
                if focused {
                    cs = if cs.add_modifier.contains(Modifier::REVERSED) {
                        cs.remove_modifier(Modifier::REVERSED)
                    } else {
                        cs.add_modifier(Modifier::REVERSED)
                    }
                    .add_modifier(Modifier::UNDERLINED);
                }
                write_clipped_line(buf, inner.x + (width - c.width) as u16, y, c.width as u16, &c.text, cs);
            }
            if let Some(r) = rollup {
                let rx = inner.x + inner.width - r.width() as u16;
                write_clipped_line(buf, rx, y, r.width() as u16, &r, style);
            }
        } else {
            let line = format!("{marker}{}", fit_width(&left, avail, hg.ellipsis));
            write_clipped_line(buf, inner.x, y, inner.width, &line, style);
            let space = avail.saturating_sub(left.width() + 1);
            let fitted = fit_segments(segments, space);
            let total: usize =
                fitted.iter().map(|(_, t)| t.width()).sum::<usize>() + fitted.len().saturating_sub(1);
            let mut x = inner.x + (width - total.min(width)) as u16;
            for (state, text) in &fitted {
                let seg_style = style.fg(dot_color(*state, &tokens));
                write_clipped_line(buf, x, y, text.width() as u16, text, seg_style);
                x += text.width() as u16 + 1;
            }
        }
        if let Some(c) = plus {
            write_clipped_line(buf, c.x, y, PLUS_COLS, "[+]", style);
        }
    }

    for cell in navigator_dot_cells(wm, workspace_area) {
        let on_selected = cell.row_index == selected;
        let mut st = Style::default().fg(dot_color(cell.state, &tokens));
        if on_selected && !is_modern {
            // Keep the reversed selection bar continuous behind the dot.
            st = st.bg(palette.text_fg.unwrap_or(palette.inactive_border_fg));
        }
        buf[(cell.x, cell.y)].set_symbol(&hg.dot(cell.state, tick)).set_style(st);
        if on_selected && !is_modern {
            buf[(cell.x, cell.y)].modifier.remove(Modifier::REVERSED);
        }
    }

    if footer_h >= 1 {
        let footer_y = inner.y + inner.height - 1;
        let footer_style = Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg);
        fill_rect(buf, Rect::new(inner.x, footer_y, inner.width, 1), footer_style);
        let label_style = footer_style.fg(palette.title_fg).add_modifier(Modifier::BOLD);
        for b in &buttons {
            let mut tmp = [0u8; 4];
            match b.kind.wide_text().filter(|_| b.width > 2) {
                Some(text) => write_clipped_line(buf, b.x, b.y, b.width, text, label_style),
                None => write_clipped_line(buf, b.x, b.y, 1, b.kind.label().encode_utf8(&mut tmp), label_style),
            }
        }
    }

    if let Some(r) = navigator_header_plus(wm, workspace_area) {
        let st = Style::default().fg(palette.title_fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD);
        write_clipped_line(buf, r.x, r.y, r.width, "[+]", st);
    }

    if let Some(menu) = menu_layout {
        let body = Style::default().fg(palette.window_frame_bg).bg(palette.title_fg);
        fill_rect(buf, menu.rect, body);
        let w = usize::from(menu.rect.width);
        let name = rows.get(selected).map(|r| sanitize_display(&r.title, 64)).unwrap_or_default();
        let head = fit_width(&format!(" {name}"), w, hg.ellipsis);
        write_clipped_line(buf, menu.rect.x, menu.rect.y, menu.rect.width, &head, body.add_modifier(Modifier::BOLD));
        for (entry, y) in &menu.entries {
            let line = fit_width(&format!(" {}", entry.label()), w, hg.ellipsis);
            write_clipped_line(buf, menu.rect.x, *y, menu.rect.width, &line, body);
        }
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(buf, drawer_rect, palette.window_frame_bg, palette, drawer.focused);
    }
}
