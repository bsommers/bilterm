//! CLIPBOARD viewer utility window renderer (`CLIP-03`, Phase 57).

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
use crate::wm::clipboard_registry::ClipboardFilterScope;
use crate::wm::WindowManager;

pub const CLIPBOARD_FOOTER_CHIPS: &str =
    "[Enter:Paste] [y:Yank] [*:Pin] [x:Del] [c:Clear] [/ :Filter]";

fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut cur_x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0
            || cur_x < clip.x
            || cur_x.saturating_add(w) > end
            || y < clip.y
            || y >= clip.y.saturating_add(clip.height)
        {
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

/// Renders the first-class `CLIPBOARD` viewer window when active (`CLIP-03`).
pub fn render_clipboard_window(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(outer) = wm.clipboard_window_rect(workspace_area) else {
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
    let bg = Style::default()
        .fg(tokens.text_primary)
        .bg(palette.window_frame_bg);
    fill(buf, rect, rect, bg);

    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }

    let state = wm.clipboard_window();
    let items = wm.clipboard_window_items();
    let total = wm.clipboard_registry().len();
    let title = if state.docked {
        format!(" CLIPBOARD ({}/{total}) [docked] ", items.len())
    } else {
        format!(" CLIPBOARD ({}/{total}) ", items.len())
    };

    render_borland_frame_styled(
        rect,
        &title,
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

        put(
            buf,
            rect,
            pin_x,
            rect.y,
            pin_glyph,
            if state.pinned {
                badge_style.fg(palette.title_fg)
            } else {
                badge_style
            },
        );
        put(buf, rect, dock_x, rect.y, dock_glyph, badge_style);
        put(buf, rect, close_x, rect.y, close_glyph, badge_style);
    }

    let inner = Rect::new(
        rect.x.saturating_add(1),
        rect.y.saturating_add(1),
        rect.width.saturating_sub(2),
        rect.height.saturating_sub(2),
    );
    if inner.width < 8 || inner.height < 4 {
        return;
    }

    // Row 0: Scope tabs + Session/Window selectors
    let tab_active = Style::default()
        .fg(Color::Black)
        .bg(tokens.accent)
        .add_modifier(Modifier::BOLD);
    let tab_idle = Style::default()
        .fg(tokens.text_muted)
        .bg(palette.window_frame_bg);

    let is_all = matches!(state.scope, ClipboardFilterScope::All);
    let is_sess = matches!(state.scope, ClipboardFilterScope::Session);
    let is_win = matches!(state.scope, ClipboardFilterScope::Window);
    let is_pin = matches!(state.scope, ClipboardFilterScope::Pinned);
    let is_med = matches!(state.scope, ClipboardFilterScope::Media);

    put(buf, inner, inner.x, inner.y, "[All]", if is_all { tab_active } else { tab_idle });
    put(buf, inner, inner.x + 6, inner.y, "[Session]", if is_sess { tab_active } else { tab_idle });
    put(buf, inner, inner.x + 16, inner.y, "[Window]", if is_win { tab_active } else { tab_idle });
    put(buf, inner, inner.x + 25, inner.y, "[Pinned]", if is_pin { tab_active } else { tab_idle });
    put(buf, inner, inner.x + 34, inner.y, "[Media]", if is_med { tab_active } else { tab_idle });

    let sess_tag = match (&state.scope, &state.session_filter) {
        (_, Some(s)) => format!("[s:{s}]"),
        (ClipboardFilterScope::Session, None) => format!("[s:{}]", wm.config().history_session),
        _ => "[s:All]".to_string(),
    };
    let win_tag = match (&state.scope, state.window_filter) {
        (_, Some(w)) => format!("[w:#{w}]"),
        (ClipboardFilterScope::Window, None) => format!("[w:#{}]", wm.active_pane_id()),
        _ => "[w:All]".to_string(),
    };
    put(buf, inner, inner.x + 42, inner.y, &sess_tag, tab_idle.fg(tokens.accent));
    put(buf, inner, inner.x + 55, inner.y, &win_tag, tab_idle.fg(tokens.accent));

    // Row 1: Search / Filter bar
    let search_cursor = if state.search_focused { "█" } else { "" };
    let search_line = format!(" / Filter: {}{search_cursor}", state.search_query);
    let search_style = if state.search_focused {
        Style::default()
            .fg(tokens.accent)
            .bg(tokens.surface_raised)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(tokens.text_primary)
            .bg(palette.window_frame_bg)
    };
    fill(buf, inner, Rect::new(inner.x, inner.y + 1, inner.width, 1), search_style);
    put(buf, inner, inner.x, inner.y + 1, &search_line, search_style);

    // Footer row: bottom row of inner
    let footer_y = inner.y + inner.height - 1;
    let footer_style = Style::default()
        .fg(tokens.accent)
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    put(buf, inner, inner.x, footer_y, CLIPBOARD_FOOTER_CHIPS, footer_style);

    let (list_rows, has_preview) = crate::wm::clipboard_window::clipboard_layout_rows(inner.height);
    let list_y_start = inner.y + 2;

    if items.is_empty() {
        let empty_msg = " (No matching clipboard entries — yank, copy, or paste to populate)";
        put(
            buf,
            inner,
            inner.x,
            list_y_start,
            empty_msg,
            Style::default().fg(tokens.text_muted).bg(palette.window_frame_bg),
        );
    } else {
        for row_idx in 0..list_rows {
            let item_idx = state.scroll + row_idx as usize;
            let y = list_y_start + row_idx;
            if let Some(entry) = items.get(item_idx) {
                let is_sel = item_idx == state.selected;
                let row_style = if is_sel {
                    Style::default()
                        .fg(tokens.text_primary)
                        .bg(tokens.surface_raised)
                        .add_modifier(Modifier::BOLD)
                } else {
                    bg
                };
                fill(buf, inner, Rect::new(inner.x, y, inner.width, 1), row_style);

                let sel_mark = if is_sel { "▶" } else { " " };
                let pin_mark = if entry.pinned { "★" } else { " " };
                let secret_mark = if entry.redacted { "🔒" } else { "" };
                let win_num = entry.pane_id.unwrap_or(0);
                let meta = format!(
                    "{sel_mark}{pin_mark}#{:<2} [{}/W#{win_num}] [{:<10}] {secret_mark}",
                    entry.id,
                    entry.session_name,
                    entry.source.badge()
                );
                let meta_w = UnicodeWidthStr::width(meta.as_str()) as u16;
                put(
                    buf,
                    inner,
                    inner.x,
                    y,
                    &meta,
                    row_style.fg(if is_sel { tokens.accent } else { tokens.text_muted }),
                );

                let rem_w = inner.width.saturating_sub(meta_w + 1) as usize;
                let snippet = entry.masked_snippet(rem_w.max(8));
                put(buf, inner, inner.x + meta_w, y, &snippet, row_style);
            }
        }
    }

    if has_preview {
        let div_y = list_y_start + list_rows;
        let selected_entry = items.get(state.selected);
        let rule_label = if let Some(e) = selected_entry {
            format!(
                "─ PREVIEW #{} ({} · {} chars · {} lines) ",
                e.id,
                e.source.badge(),
                e.char_count,
                e.line_count
            )
        } else {
            "─ PREVIEW ".to_string()
        };
        let rule_len = UnicodeWidthStr::width(rule_label.as_str());
        let rem_dashes = (inner.width as usize).saturating_sub(rule_len);
        let rule_full = format!("{rule_label}{}", "─".repeat(rem_dashes));
        put(
            buf,
            inner,
            inner.x,
            div_y,
            &rule_full,
            Style::default().fg(tokens.border_idle).bg(palette.window_frame_bg),
        );

        if let Some(entry) = selected_entry {
            let preview_lines = entry.masked_preview_lines(3, inner.width.saturating_sub(2) as usize);
            for (i, line) in preview_lines.iter().enumerate() {
                let py = div_y + 1 + i as u16;
                if py >= footer_y {
                    break;
                }
                put(buf, inner, inner.x + 1, py, line, bg);
            }
        }
    }
}
