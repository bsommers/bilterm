//! Left-side attention indicator rendering (`src/ui/attention_ui.rs`).
//!
//! Renders a calm, slow-pulsing (`900 ms` half-period, ~0.55 Hz) attention beacon on the left:
//! 1. **Inside the slide-out drawer (`Navigator` and `Classic` styles)**: pulses the leftmost
//!    marker cell (`inner.x, y`) and adjacent left frame rail (`drawer_rect.x, y`) of any
//!    built-in session row or window/pane row that needs attention.
//! 2. **On the left edge of the workspace when the drawer is closed**: renders a compact 1-col x
//!    3-row left-edge beacon tab at `(workspace_area.x, workspace_area.y + 1)` whenever any
//!    built-in session (including parked background sessions) or non-focused window needs attention.
//!    Clicking the left-edge tab opens the drawer and jumps to the attention window/session.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::config::BiltermConfig;
use crate::ui::glyphs::GlyphPack;
use crate::wm::WindowManager;

/// Lit-phase foreground color: warm 1970s TRON amber-orange (`#FF9600`).
pub const ATTENTION_LIT_FG: Color = Color::Rgb(255, 150, 0);
/// Lit-phase subtle background tint for the closed-drawer left-edge beacon.
pub const ATTENTION_LIT_BG: Color = Color::Rgb(48, 24, 0);
/// Dim-phase foreground color: muted amber (`#965500`) so the indicator breathes instead of vanishing jarring.
pub const ATTENTION_DIM_FG: Color = Color::Rgb(150, 85, 0);

/// Marker glyph for a drawer row needing attention (`"◆"` in Unicode, `"!"` in ASCII/CP437 when lit;
/// `"·"` / `"."` when in the dim half-period).
pub fn row_attention_glyph(cfg: &BiltermConfig, lit: bool) -> &'static str {
    let ascii = matches!(cfg.glyphs, GlyphPack::Ascii | GlyphPack::Cp437);
    match (ascii, lit) {
        (false, true) => "◆",
        (false, false) => "·",
        (true, true) => "!",
        (true, false) => ".",
    }
}

/// Paints the leftmost row marker cell `(inner_x, y)` and the adjacent left border rail
/// `(border_x, y)` for a drawer row that needs attention.
pub fn paint_drawer_row_left_attention(
    buf: &mut Buffer,
    border_x: u16,
    inner_x: u16,
    y: u16,
    cfg: &BiltermConfig,
    lit: bool,
) {
    let glyph = row_attention_glyph(cfg, lit);
    let fg = if lit { ATTENTION_LIT_FG } else { ATTENTION_DIM_FG };
    let mod_flag = if lit { Modifier::BOLD } else { Modifier::empty() };

    if let Some(cell) = buf.cell_mut((inner_x, y)) {
        let st = cell.style().fg(fg).add_modifier(mod_flag);
        cell.set_symbol(glyph).set_style(st);
    }
    if border_x < inner_x
        && let Some(rail) = buf.cell_mut((border_x, y))
    {
        let st = rail.style().fg(fg).add_modifier(mod_flag);
        rail.set_style(st);
    }
}

/// Bounding rect of the compact 1-col x 3-row left-edge attention beacon shown when the drawer is
/// closed and at least one built-in session or window needs attention.
pub fn left_attention_beacon_rect(wm: &WindowManager, workspace_area: Rect) -> Option<Rect> {
    if wm.is_drawer_effectively_visible()
        || !wm.any_attention_needed()
        || workspace_area.width < 4
        || workspace_area.height < 5
    {
        return None;
    }
    Some(Rect::new(workspace_area.x, workspace_area.y + 1, 1, 3))
}

/// Renders the compact 1-col x 3-row left-edge attention beacon when the drawer is closed and a
/// built-in session or window needs attention.
pub fn render_left_attention_beacon(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    buf: &mut Buffer,
) {
    let Some(rect) = left_attention_beacon_rect(wm, workspace_area) else {
        return;
    };
    let lit = wm.attention_pulse_lit(Instant::now());
    let ascii = matches!(cfg.glyphs, GlyphPack::Ascii | GlyphPack::Cp437);
    let cap = if ascii { "|" } else { "▐" };
    let mid = row_attention_glyph(cfg, lit);
    let style = if lit {
        Style::default()
            .fg(ATTENTION_LIT_FG)
            .bg(ATTENTION_LIT_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ATTENTION_DIM_FG)
    };
    for (dy, sym) in [(0u16, cap), (1u16, mid), (2u16, cap)] {
        if let Some(cell) = buf.cell_mut((rect.x, rect.y + dy)) {
            cell.set_symbol(sym).set_style(style);
        }
    }
}
