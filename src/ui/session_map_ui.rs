//! TUI Ratatui buffer renderer for the non-modal `SESSION MAP` window (Phase 54, VIEW-02).

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
use crate::ui::visual_style::render_liquid_ambient_halo;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;

/// Title string of the session packing viewer.
pub const SESSION_MAP_TITLE: &str = "SESSION MAP";

/// Writes clipped text into buffer at `(x, y)`.
pub(crate) fn put_str(buf: &mut Buffer, clip: Rect, mut x: u16, y: u16, text: &str, style: Style) {
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 || x < clip.x || x.saturating_add(w) > end || y < clip.y || y >= clip.bottom() {
            if x >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        x = x.saturating_add(w);
    }
}

/// Fills area with blanks in given style.
pub(crate) fn fill_rect(buf: &mut Buffer, clip: Rect, r: Rect, style: Style) {
    let r = r.intersection(clip);
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the `SESSION MAP` window when active.
pub fn render_session_map(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(rect) = wm.session_map_rect(workspace_area) else {
        return;
    };
    let rect = rect.intersection(buf.area);
    if rect.width < 10 || rect.height < 6 {
        return;
    }

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);

    let tokens = DesignTokens::from_palette(palette);
    let bg_style = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    fill_rect(buf, rect, rect, bg_style);

    let map_state = wm.session_map();
    let inputs = wm.build_session_pack_inputs();
    let total_windows: usize = inputs.iter().map(|s| s.windows.len()).sum();

    // Title line with count
    let title = format!(" {} ({}s, {}w) ", SESSION_MAP_TITLE, inputs.len(), total_windows);

    // Outer Borland frame
    render_borland_frame_styled(
        rect,
        &title,
        map_state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    // Badges on title row: [•]/[o] Pin, [□] Dock, [×] Close
    let pin_glyph = if map_state.pinned { "[•]" } else { "[o]" };
    let badges = format!("{pin_glyph} [□] [×]");
    let badge_x = rect.right().saturating_sub(badges.len() as u16 + 2);
    let badge_style = Style::default().fg(palette.title_fg).bg(palette.window_frame_bg);
    put_str(buf, rect, badge_x, rect.y, &badges, badge_style);

    // Subheader status row
    let subheader_y = rect.y + 1;
    let target_sess = map_state.target_session.as_deref().unwrap_or("main");
    let holding_info = if let Some(pid) = map_state.holding_pane_id {
        format!("Holding: #{pid} -> '{target_sess}'")
    } else {
        format!("Target: '{target_sess}'")
    };
    let subheader = format!(" Focused: {} | {}", target_sess, holding_info);
    let sub_style = Style::default().fg(tokens.text_muted).bg(palette.window_frame_bg);
    put_str(buf, rect, rect.x + 1, subheader_y, &subheader, sub_style);

    // Footer hint bar
    let footer_y = rect.bottom().saturating_sub(1);
    let footer = " Space/m:Pickup  Tab/Arrows:Target  Enter:Drop  Esc:Cancel/Back  *:Pin ";
    let footer_style = Style::default().fg(tokens.text_muted).bg(palette.window_frame_bg);
    put_str(buf, rect, rect.x + 2, footer_y, footer, footer_style);

    // Interior area for packing layout
    let interior = Rect::new(
        rect.x + 1,
        rect.y + 2,
        rect.width.saturating_sub(2),
        rect.height.saturating_sub(3),
    );
    if interior.width < 4 || interior.height < 3 {
        return;
    }

    let layout = wm.compute_session_pack_layout(interior);
    for sess in &layout {
        if sess.outer.width < 3 || sess.outer.height < 2 {
            continue;
        }

        let is_target = map_state.target_session.as_deref() == Some(&sess.session_name);
        let border_fg = if is_target && (map_state.holding_pane_id.is_some() || map_state.drag.is_some()) {
            Color::Yellow
        } else if sess.active {
            tokens.accent
        } else {
            tokens.border_idle
        };
        let container_style = Style::default().fg(border_fg).bg(palette.window_frame_bg);

        // Container borders
        draw_container_box(buf, sess.outer, container_style);

        // Session name header
        let s_title = format!(" {} ", sanitize_display(&sess.session_name, 18));
        let header_style = if is_target {
            container_style.add_modifier(Modifier::BOLD)
        } else {
            container_style
        };
        put_str(buf, sess.outer, sess.outer.x + 1, sess.outer.y, &s_title, header_style);

        // Render packed window cards
        for win in &sess.windows {
            if win.rect.width < 2 || win.rect.height < 1 {
                continue;
            }

            let is_held = map_state.holding_pane_id == Some(win.pane_id);
            let is_selected = map_state.selected_pane_id == Some(win.pane_id);

            let (win_fg, win_bg, modif) = if is_held {
                (Color::Black, Color::Yellow, Modifier::BOLD)
            } else if is_selected {
                (palette.window_frame_bg, tokens.accent, Modifier::BOLD)
            } else if win.focused {
                (tokens.text_primary, Color::DarkGray, Modifier::BOLD)
            } else {
                (tokens.text_muted, Color::Rgb(30, 32, 44), Modifier::empty())
            };

            let win_style = Style::default().fg(win_fg).bg(win_bg).add_modifier(modif);
            fill_rect(buf, sess.inner, win.rect, win_style);

            let label = if is_held {
                format!("[HELD] #{} {}", win.pane_id, sanitize_display(&win.title, 14))
            } else if is_selected {
                format!("*#{} {}", win.pane_id, sanitize_display(&win.title, 14))
            } else {
                format!("#{} {}", win.pane_id, sanitize_display(&win.title, 14))
            };
            put_str(buf, win.rect, win.rect.x, win.rect.y, &label, win_style);
        }
    }
}

fn draw_container_box(buf: &mut Buffer, r: Rect, style: Style) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let right = r.right() - 1;
    let bottom = r.bottom() - 1;

    if let Some(cell) = buf.cell_mut((r.x, r.y)) {
        cell.set_symbol("┌");
        cell.set_style(style);
    }
    if let Some(cell) = buf.cell_mut((right, r.y)) {
        cell.set_symbol("┐");
        cell.set_style(style);
    }
    if let Some(cell) = buf.cell_mut((r.x, bottom)) {
        cell.set_symbol("└");
        cell.set_style(style);
    }
    if let Some(cell) = buf.cell_mut((right, bottom)) {
        cell.set_symbol("┘");
        cell.set_style(style);
    }
    for x in (r.x + 1)..right {
        if let Some(cell) = buf.cell_mut((x, r.y)) {
            cell.set_symbol("─");
            cell.set_style(style);
        }
        if let Some(cell) = buf.cell_mut((x, bottom)) {
            cell.set_symbol("─");
            cell.set_style(style);
        }
    }
    for y in (r.y + 1)..bottom {
        if let Some(cell) = buf.cell_mut((r.x, y)) {
            cell.set_symbol("│");
            cell.set_style(style);
        }
        if let Some(cell) = buf.cell_mut((right, y)) {
            cell.set_symbol("│");
            cell.set_style(style);
        }
    }
}
