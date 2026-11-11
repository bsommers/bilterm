//! COMMAND PALETTE utility window renderer (WIN-02, Phase 51.2-02).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::WindowManager;

pub const COMMAND_WINDOW_TITLE: &str = " COMMAND PALETTE ";
pub const FOOTER_HINT: &str = " Enter:Run  Alt+P:Pin  Ctrl+L:ClearLog  Esc:Back ";

fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut cur_x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 || cur_x < clip.x || cur_x.saturating_add(w) > end || y < clip.y || y >= clip.y.saturating_add(clip.height) {
            if cur_x >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((cur_x, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        cur_x += w;
    }
}

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

/// Renders the first-class Command Palette window when active.
pub fn render_command_window(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(outer) = wm.command_window_rect(workspace_area) else {
        return;
    };
    let rect = outer.intersection(buf.area);
    if rect.width < 20 || rect.height < 6 {
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

    let state = wm.command_window();
    let title = if state.docked {
        " COMMAND PALETTE [docked] "
    } else {
        COMMAND_WINDOW_TITLE
    };

    render_borland_frame_styled(
        rect,
        title,
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    // Title badges: [•]/[o] (pin), [□] (dock), [×] (close)
    let badge_style = Style::default()
        .fg(palette.controls_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);

    let pin_glyph = if state.pinned { "[•]" } else { "[o]" };
    let dock_glyph = "[□]";
    let close_glyph = "[×]";

    if rect.width >= 32 {
        let close_x = rect.x + rect.width.saturating_sub(4);
        let dock_x = rect.x + rect.width.saturating_sub(8);
        let pin_x = rect.x + rect.width.saturating_sub(12);

        put(buf, rect, pin_x, rect.y, pin_glyph, if state.pinned { badge_style.fg(palette.title_fg) } else { badge_style });
        put(buf, rect, dock_x, rect.y, dock_glyph, badge_style);
        put(buf, rect, close_x, rect.y, close_glyph, badge_style);
    }

    let inner = Rect::new(
        rect.x.saturating_add(1),
        rect.y.saturating_add(1),
        rect.width.saturating_sub(2),
        rect.height.saturating_sub(2),
    );

    if inner.width < 4 || inner.height < 3 {
        return;
    }

    // Row 0: REPL / Query Input
    let input_line = format!(" > {}█", state.query);
    let input_style = Style::default()
        .fg(tokens.accent)
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    put(buf, inner, inner.x, inner.y, &input_line, input_style);

    // Footer row: bottom row of inner
    let footer_y = inner.y + inner.height - 1;
    let footer_style = Style::default()
        .fg(tokens.text_muted)
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::DIM);
    put(buf, inner, inner.x, footer_y, FOOTER_HINT, footer_style);

    // History and commands partition
    let (cmd_rows_height, has_history) = if inner.height >= 10 {
        (inner.height.saturating_sub(6), true)
    } else {
        (inner.height.saturating_sub(2), false)
    };

    // Render commands list
    let items = wm.command_window_items();
    let cmd_y_start = inner.y + 1;
    for row_idx in 0..cmd_rows_height {
        let item_idx = state.scroll + row_idx as usize;
        let y = cmd_y_start + row_idx;
        if let Some((_, item)) = items.get(item_idx) {
            let is_sel = item_idx == state.selected;
            let cursor = if is_sel { "▶ " } else { "  " };
            let cat_str = format!("[{}] ", item.category);
            let cmd_badge = format!(" {}", item.colon_command);

            let row_style = if is_sel {
                Style::default()
                    .fg(tokens.text_primary)
                    .bg(tokens.surface_raised)
                    .add_modifier(Modifier::BOLD)
            } else {
                bg
            };

            // Clear row background
            fill(buf, inner, Rect::new(inner.x, y, inner.width, 1), row_style);

            let lead = format!("{}{}", cursor, cat_str);
            put(buf, inner, inner.x, y, &lead, row_style.fg(if is_sel { tokens.accent } else { tokens.text_muted }));

            let lead_w = UnicodeWidthStr::width(lead.as_str()) as u16;
            let badge_w = UnicodeWidthStr::width(cmd_badge.as_str()) as u16;
            let title_w = inner.width.saturating_sub(lead_w + badge_w + 1);

            let title_fit: String = item.title.chars().take(title_w as usize).collect();
            put(buf, inner, inner.x + lead_w, y, &title_fit, row_style);

            let badge_x = (inner.x + inner.width).saturating_sub(badge_w);
            put(buf, inner, badge_x, y, &cmd_badge, row_style.fg(tokens.accent));
        }
    }

    // Render thin horizontal rule and history log
    if has_history {
        let div_y = cmd_y_start + cmd_rows_height;
        let rule_label = "─ OUTPUT / HISTORY ";
        let rule_len = UnicodeWidthStr::width(rule_label);
        let rem_dashes = (inner.width as usize).saturating_sub(rule_len);
        let rule_full = format!("{}{}", rule_label, "─".repeat(rem_dashes));
        put(buf, inner, inner.x, div_y, &rule_full, Style::default().fg(tokens.border_idle).bg(palette.window_frame_bg));

        let hist_y_start = div_y + 1;
        let hist_len = state.history_log.len();
        let hist_visible = (footer_y.saturating_sub(hist_y_start)) as usize;
        let start_hist_idx = hist_len.saturating_sub(hist_visible);

        for (i, entry) in state.history_log[start_hist_idx..].iter().enumerate() {
            let y = hist_y_start + i as u16;
            if y >= footer_y {
                break;
            }
            let (sym, sym_fg) = if entry.ok {
                ("✓ ", Color::Green)
            } else {
                ("✗ ", Color::Red)
            };
            put(buf, inner, inner.x, y, sym, Style::default().fg(sym_fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD));
            let text = format!("{:<20} -> {}", entry.command, entry.status);
            put(buf, inner, inner.x + 2, y, &text, bg);
        }
    }
}
