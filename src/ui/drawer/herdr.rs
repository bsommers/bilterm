//! `herdr`-style drawer renderer: one buffer line per workspace > session > pane row (`FLT-01`).
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
use crate::wm::drawer::{BRAILLE_SPINNER_FRAMES, HerdrLevel};
use crate::wm::{DrawerLayoutMode, DrawerRow, DrawerRowKind, WindowManager, inset_window_frame};

/// Computes `(header_rows, footer_rows, rows_per_item)` for the herdr layout: no header, an optional
/// one-line footer and exactly one line per item.
pub fn herdr_layout_metrics(inner_height: u16) -> (u16, u16, usize) {
    (0, u16::from(inner_height >= 4), 1)
}

/// Single-width BMP symbols used by the herdr tree (no emoji; RESEARCH anti-pattern).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HerdrGlyphs {
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
}

impl HerdrGlyphs {
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
pub fn counts_segments(counts: &AgentCounts, g: &HerdrGlyphs, tick: usize) -> Vec<(AgentState, String)> {
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

/// Geometry shared by the renderer and [`herdr_dot_cells`].
struct HerdrLayout {
    inner: Rect,
    rows: Vec<DrawerRow>,
    selected: usize,
    scroll: usize,
    cap: usize,
    list_y: u16,
    list_bottom: u16,
    row_step: usize,
}

fn herdr_layout(wm: &WindowManager, drawer_rect: Rect) -> Option<HerdrLayout> {
    let inner = inset_window_frame(drawer_rect);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let rows = wm.drawer_rows();
    let selected = wm.drawer().selected_index.min(rows.len().saturating_sub(1));
    let (header_h, footer_h, row_step) = herdr_layout_metrics(inner.height);
    let list_h = usize::from(inner.height.saturating_sub(header_h + footer_h)).max(1);
    let cap = (list_h / row_step).max(1);
    let scroll = wm.drawer().effective_scroll(selected, cap, rows.len());
    Some(HerdrLayout {
        inner,
        rows,
        selected,
        scroll,
        cap,
        list_y: inner.y + header_h,
        list_bottom: inner.y + inner.height.saturating_sub(footer_h),
        row_step,
    })
}

/// Screen cell of one pane state dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HerdrDotCell {
    pub x: u16,
    pub y: u16,
    pub state: AgentState,
    /// Index into [`WindowManager::drawer_rows`].
    pub row_index: usize,
}

/// Cells of every visible pane state dot; the single source of dot positions (TUI renderer here,
/// GUI icon pass in a later plan). Empty when the drawer is not visible.
pub fn herdr_dot_cells(wm: &WindowManager, workspace_area: Rect) -> Vec<HerdrDotCell> {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return Vec::new();
    };
    let Some(lay) = herdr_layout(wm, drawer_rect) else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    for (vis_idx, (row_index, row)) in lay.rows.iter().enumerate().skip(lay.scroll).take(lay.cap).enumerate() {
        let y = lay.list_y + (vis_idx * lay.row_step) as u16;
        if y >= lay.list_bottom {
            break;
        }
        let Some(meta) = row.herdr.as_ref() else { continue };
        let Some(state) = meta.agent else { continue };
        let x = lay.inner.x + 1 + 2 * u16::from(meta.depth);
        if x < lay.inner.x + lay.inner.width {
            cells.push(HerdrDotCell { x, y, state, row_index });
        }
    }
    cells
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

/// Left text (after the marker column) and optional right-aligned rollup of one herdr row.
fn row_parts(row: &DrawerRow, g: &HerdrGlyphs) -> (String, Option<String>) {
    let meta = row.herdr.as_ref();
    let indent = "  ".repeat(meta.map_or(0, |m| usize::from(m.depth)));
    let label = match &row.kind {
        DrawerRowKind::Pane(id) => format!("{}#{id} {}", " ".repeat(DOT_COLS), row.title),
        _ => row.title.clone(),
    };
    let fold = match meta {
        Some(m) if matches!(m.level, HerdrLevel::Workspace | HerdrLevel::Session) => {
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
        .filter(|m| matches!(m.level, HerdrLevel::Workspace | HerdrLevel::Session) && (m.live || m.pane_count > 0))
        .map(|m| if m.live { format!("{}p", m.pane_count) } else { format!("{}p -", m.pane_count) });
    (format!("{indent}{fold}{label}"), rollup)
}

/// Renders the herdr-style drawer into `drawer_rect` (`FLT-01`).
pub fn render_herdr_drawer(
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

    let title = if is_modern { "◢ WORKSPACES" } else { "WORKSPACES" };
    render_borland_frame_styled(
        drawer_rect,
        title,
        drawer.focused,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    let Some(lay) = herdr_layout(wm, drawer_rect) else {
        return;
    };
    let HerdrLayout { inner, rows, selected, scroll, cap, list_y, list_bottom, row_step } = lay;
    let base_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.inactive_border_fg))
        .bg(palette.window_frame_bg);
    if !cfg.is_glass() {
        fill_rect(buf, inner, base_style);
    }

    let hg = HerdrGlyphs::for_pack(cfg.glyphs);
    let tokens = DesignTokens::from_palette(palette);
    let tick = drawer.spinner_tick();
    let footer_h = herdr_layout_metrics(inner.height).1;

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
        let width = usize::from(inner.width);
        let avail = width.saturating_sub(1);
        let segments = row
            .herdr
            .as_ref()
            .and_then(|m| m.counts.as_ref())
            .map(|c| counts_segments(c, &hg, tick))
            .unwrap_or_default();
        if segments.is_empty() {
            let rollup_w = rollup.as_ref().map_or(0, |r| r.width() + 1);
            // Keep the rollup only when it leaves room for the marker plus a few label columns.
            let rollup = rollup.filter(|_| width >= rollup_w + 6);
            let left_w = avail.saturating_sub(if rollup.is_some() { rollup_w } else { 0 });
            let line = format!("{marker}{}", fit_width(&left, left_w, hg.ellipsis));
            write_clipped_line(buf, inner.x, y, inner.width, &line, style);
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
    }

    for cell in herdr_dot_cells(wm, workspace_area) {
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
        let hint = if hg == HerdrGlyphs::ASCII { "Enter focus  e fold" } else { "⏎ focus  e fold" };
        let footer_style = Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg);
        fill_rect(buf, Rect::new(inner.x, footer_y, inner.width, 1), footer_style);
        let hint = fit_width(hint, usize::from(inner.width), hg.ellipsis);
        write_clipped_line(buf, inner.x, footer_y, inner.width, &hint, footer_style);
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(buf, drawer_rect, palette.window_frame_bg, palette, drawer.focused);
    }
}
