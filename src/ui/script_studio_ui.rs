//! SCRIPT STUDIO floating window renderer (Plan 51.2-03, WIN-03).
//!
//! Renders a Borland frame titled `SCRIPT STUDIO [<name>] [● REC] [EDIT]/[NAV]`,
//! with an interactive toolbar row, numbered script editor pane, live run log,
//! and hotkey footer bar.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;

pub const TOOLBAR_TEXT: &str = " [▶ Run(F5)] [⏭ Step(F10)] [↺ Reset(r)] [● Rec(R)] [✎ Edit(i)] [💾 Save(s)] ";
pub const FOOTER_TEXT: &str = "F5/r:Run  F10/n:Step  R:Rec  i:Edit  dd:DelLine  o:NewLine  *:Pin  Esc:Nav/Close";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptStudioButton {
    Run,
    Step,
    Reset,
    Record,
    Edit,
    Save,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptStudioHit {
    Outside,
    Title,
    PinBadge,
    DockBadge,
    CloseBadge,
    Button(ScriptStudioButton),
    Line(usize),
    Border(WindowResizeEdge),
    Interior,
}

/// Hit-tests mouse coordinate `(col, row)` against the Script Studio window.
pub fn script_studio_hit(wm: &WindowManager, ws: Rect, col: u16, row: u16) -> ScriptStudioHit {
    let Some(outer) = wm.script_studio_rect(ws) else {
        return ScriptStudioHit::Outside;
    };
    if col < outer.x || col >= outer.x + outer.width || row < outer.y || row >= outer.y + outer.height {
        return ScriptStudioHit::Outside;
    }

    // Top title bar
    if row == outer.y {
        if col >= outer.x + outer.width.saturating_sub(3) {
            return ScriptStudioHit::CloseBadge;
        }
        if col >= outer.x + outer.width.saturating_sub(6) {
            return ScriptStudioHit::DockBadge;
        }
        if col >= outer.x + outer.width.saturating_sub(9) {
            return ScriptStudioHit::PinBadge;
        }
        return ScriptStudioHit::Title;
    }

    // Bottom border
    if row == outer.y + outer.height.saturating_sub(1) {
        if col == outer.x {
            return ScriptStudioHit::Border(WindowResizeEdge::BottomLeft);
        }
        if col == outer.x + outer.width.saturating_sub(1) {
            return ScriptStudioHit::Border(WindowResizeEdge::BottomRight);
        }
        return ScriptStudioHit::Border(WindowResizeEdge::Bottom);
    }

    // Left or right border
    if col == outer.x {
        return ScriptStudioHit::Border(WindowResizeEdge::Left);
    }
    if col == outer.x + outer.width.saturating_sub(1) {
        return ScriptStudioHit::Border(WindowResizeEdge::Right);
    }

    // Toolbar row (row == outer.y + 1)
    if row == outer.y + 1 {
        let rel_x = col.saturating_sub(outer.x + 1);
        if (1..12).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Run);
        }
        if (13..26).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Step);
        }
        if (27..39).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Reset);
        }
        if (40..50).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Record);
        }
        if (51..62).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Edit);
        }
        if (63..74).contains(&rel_x) {
            return ScriptStudioHit::Button(ScriptStudioButton::Save);
        }
        return ScriptStudioHit::Interior;
    }

    // Separator row (outer.y + 2)
    if row == outer.y + 2 {
        return ScriptStudioHit::Interior;
    }

    // Editor lines area
    let editor_start_y = outer.y + 3;
    let log_rows: u16 = 5; // 1 separator + 4 log lines
    let footer_row = outer.y + outer.height.saturating_sub(2);
    let editor_end_y = footer_row.saturating_sub(log_rows);

    if row >= editor_start_y && row < editor_end_y {
        let line_offset = (row - editor_start_y) as usize;
        let line_idx = wm.script_studio().scroll + line_offset;
        if line_idx < wm.script_studio().lines.len() {
            return ScriptStudioHit::Line(line_idx);
        }
    }

    ScriptStudioHit::Interior
}

/// Writes `text` from `(x, y)` clipped to `clip`.
fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 || x < clip.x || x.saturating_add(w) > end || y < clip.y || y >= clip.y.saturating_add(clip.height) {
            if x >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        x += w;
    }
}

/// Fills cells in `r` with blanks in `style`.
fn fill(buf: &mut Buffer, clip: Rect, r: Rect, style: Style) {
    let r = r.intersection(clip);
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the SCRIPT STUDIO window when active.
pub fn render_script_studio(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(outer) = wm.script_studio_rect(workspace_area) else {
        return;
    };
    let rect = outer.intersection(buf.area);
    if rect.width < 28 || rect.height < 10 {
        return;
    }

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);

    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    fill(buf, rect, rect, bg);

    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }

    let state = wm.script_studio();
    let mode_str = if state.edit_mode { "[EDIT]" } else { "[NAV]" };
    let rec_str = if state.recording { " [● REC]" } else { "" };
    let title = format!("SCRIPT STUDIO [{}] {}{}", state.script_name, mode_str, rec_str);

    render_borland_frame_styled(
        rect,
        &title,
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    // Pin indicator before top-right window buttons
    let pin_x = rect.x + rect.width.saturating_sub(8);
    let pin_glyph = if state.pinned { "[•]" } else { "[o]" };
    let pin_style = if state.pinned {
        Style::default().fg(palette.title_fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.controls_fg.unwrap_or(palette.title_fg)).bg(palette.window_frame_bg)
    };
    put(buf, rect, pin_x, rect.y, pin_glyph, pin_style);

    // Row 1: Interactive Toolbar row
    let toolbar_y = rect.y + 1;
    let tb_style = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD);
    put(buf, rect, rect.x + 1, toolbar_y, TOOLBAR_TEXT, tb_style);

    // Colorize specific toolbar items
    if state.recording {
        let rec_pos = rect.x + 41; // "[● Rec(R)]"
        put(buf, rect, rec_pos, toolbar_y, "[● Rec(R)]", Style::default().fg(Color::LightRed).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD));
    }
    if state.edit_mode {
        let edit_pos = rect.x + 52; // "[✎ Edit(i)]"
        put(buf, rect, edit_pos, toolbar_y, "[✎ Edit(i)]", Style::default().fg(Color::LightGreen).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD));
    }

    // Row 2: Separator
    let sep_y = rect.y + 2;
    let sep_line = "─".repeat(rect.width.saturating_sub(2) as usize);
    put(buf, rect, rect.x + 1, sep_y, &sep_line, Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg));

    // Editor Area & Run Log geometry
    let editor_start_y = rect.y + 3;
    let footer_y = rect.y + rect.height.saturating_sub(2);
    let log_sep_y = footer_y.saturating_sub(5);
    let editor_height = log_sep_y.saturating_sub(editor_start_y);

    // Render Editor lines
    let visible_lines = editor_height as usize;
    for i in 0..visible_lines {
        let line_idx = state.scroll + i;
        let line_y = editor_start_y + i as u16;
        if line_y >= log_sep_y {
            break;
        }

        if let Some(line) = state.lines.get(line_idx) {
            let is_cursor = line_idx == state.cursor_line;
            let is_pc = line_idx == state.step_pc;
            let pc_marker = if is_pc { "▶" } else { " " };

            let row_style = if is_cursor {
                bg.add_modifier(Modifier::REVERSED)
            } else if line.trim().starts_with('#') {
                Style::default().fg(tokens.text_muted).bg(palette.window_frame_bg)
            } else {
                bg
            };

            // Full line background fill for cursor row
            if is_cursor {
                fill(buf, rect, Rect::new(rect.x + 1, line_y, rect.width.saturating_sub(2), 1), row_style);
            }

            let num_prefix = format!("{:02} {} ", line_idx + 1, pc_marker);
            put(buf, rect, rect.x + 2, line_y, &num_prefix, row_style.add_modifier(Modifier::BOLD));

            let content_x = rect.x + 2 + num_prefix.len() as u16;
            put(buf, rect, content_x, line_y, line, row_style);

            // Draw edit cursor if in edit mode
            if is_cursor && state.edit_mode {
                let cur_x = content_x + state.cursor_col as u16;
                if cur_x < rect.x + rect.width.saturating_sub(1)
                    && let Some(cell) = buf.cell_mut((cur_x, line_y))
                {
                    cell.set_style(Style::default().fg(Color::Yellow).bg(Color::Blue).add_modifier(Modifier::RAPID_BLINK | Modifier::UNDERLINED));
                }
            }
        }
    }

    // Run Log Separator
    let log_sep_title = "─ RUN LOG ";
    let fill_len = (rect.width.saturating_sub(2) as usize).saturating_sub(log_sep_title.len());
    let log_sep = format!("{}{}", log_sep_title, "─".repeat(fill_len));
    put(buf, rect, rect.x + 1, log_sep_y, &log_sep, Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg));

    // Bottom 4 rows: Run log lines
    let log_rows = 4usize;
    let recent_logs = state.run_log.iter().rev().take(log_rows).collect::<Vec<_>>();
    for (i, log) in recent_logs.into_iter().rev().enumerate() {
        let ly = log_sep_y + 1 + i as u16;
        if ly >= footer_y {
            break;
        }
        let status_mark = if log.ok { "✓" } else { "✗" };
        let mark_color = if log.ok { Color::LightGreen } else { Color::LightRed };
        put(buf, rect, rect.x + 2, ly, status_mark, Style::default().fg(mark_color).bg(palette.window_frame_bg));

        let log_text = format!(" L{:02} {} -> {}", log.line_no, log.source, log.output);
        put(buf, rect, rect.x + 3, ly, &log_text, Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg));
    }

    // Footer bar
    let footer_style = Style::default().fg(tokens.text_muted).bg(palette.window_frame_bg);
    put(buf, rect, rect.x + 2, footer_y, FOOTER_TEXT, footer_style);
}
