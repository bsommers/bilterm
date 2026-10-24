//! `herdr`-Style Slide-Out Sessions / Windows / Panes & Fleet Drawer Renderer (`DRW-01`, `DRW-02`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::wm::{DrawerLayoutMode, DrawerRowKind, WindowManager, inset_window_frame};

/// Writes a single-line string into `buf` at `(x, y)` clipped to `max_cols` display columns.
fn write_clipped_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    max_cols: u16,
    text: &str,
    style: Style,
) {
    let mut col = 0u16;
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(1).max(1) as u16;
        if col + w > max_cols {
            break;
        }
        if let Some(cell) = buf.cell_mut((x + col, y)) {
            let mut sym = [0u8; 4];
            cell.set_symbol(ch.encode_utf8(&mut sym));
            cell.set_style(style);
        }
        if w == 2
            && col + 1 < max_cols
            && let Some(next_cell) = buf.cell_mut((x + col + 1, y))
        {
            next_cell.set_symbol(" ");
            next_cell.set_style(style);
        }
        col += w;
    }
}

/// Fills `rect` in `buf` with space characters and `style`.
fn fill_rect(buf: &mut Buffer, rect: Rect, style: Style) {
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the `herdr`-style slide-out Sessions / Windows / Panes & Fleet drawer when visible (`DRW-01`, `DRW-02`).
pub fn render_workspace_drawer(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(drawer_rect) = wm.drawer_rect(workspace_area) else {
        return;
    };
    let dcfg = wm.drawer_config();
    let drawer = wm.drawer();

    // In Overlay mode, cast a Borland Turbo Vision drop shadow before occluding underlying cells
    if dcfg.layout == DrawerLayoutMode::Overlay && cfg.drop_shadows {
        render_drop_shadow_styled(buf, drawer_rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, drawer_rect);

    let title = format!(
        "HERDR [D:{}] [M:{}] [P:{}]",
        dcfg.edge.label(),
        dcfg.layout.label(),
        dcfg.visibility.label()
    );
    render_borland_frame_styled(
        drawer_rect,
        &title,
        drawer.focused,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    let inner = inset_window_frame(drawer_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let base_style = Style::default()
        .fg(palette.inactive_border_fg)
        .bg(palette.window_frame_bg);
    fill_rect(buf, inner, base_style);

    let rows = wm.drawer_rows();
    let selected = drawer.selected_index.min(rows.len().saturating_sub(1));

    if dcfg.edge.is_vertical() {
        let header_line = format!(
            "[D:{}] [M:{}] [P:{}]",
            dcfg.edge.label(),
            dcfg.layout.label(),
            dcfg.visibility.label()
        );
        let header_style = Style::default()
            .fg(palette.title_fg)
            .bg(palette.oia_bg)
            .add_modifier(Modifier::BOLD);
        fill_rect(buf, Rect::new(inner.x, inner.y, inner.width, 1), header_style);
        write_clipped_line(buf, inner.x, inner.y, inner.width, &header_line, header_style);

        let list_y = inner.y + 1;
        let list_h = inner.height.saturating_sub(2).max(1) as usize;
        let scroll = if selected >= list_h {
            selected + 1 - list_h
        } else {
            0
        };

        for (vis_idx, (row_idx, row)) in rows
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_h)
            .enumerate()
        {
            let y = list_y + vis_idx as u16;
            if y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }
            let is_sel = row_idx == selected;
            let mut row_style = match row.kind {
                DrawerRowKind::SessionHeader(_) => Style::default()
                    .fg(palette.title_fg)
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
                DrawerRowKind::LayoutGroup(_) => Style::default()
                    .fg(palette.active_border_fg)
                    .bg(palette.window_frame_bg),
                _ if row.is_focused_pane => Style::default()
                    .fg(palette.title_fg)
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
                _ => base_style,
            };
            if is_sel {
                row_style = row_style.add_modifier(Modifier::REVERSED);
                fill_rect(
                    buf,
                    Rect::new(inner.x, y, inner.width, 1),
                    row_style,
                );
            }
            let line = row.formatted_line();
            write_clipped_line(buf, inner.x, y, inner.width, &line, row_style);
        }

        // Bottom 1-row hotkey hint bar inside vertical drawer
        if inner.height >= 3 {
            let footer_y = inner.y + inner.height - 1;
            let footer_style = Style::default()
                .fg(palette.fkey_bar_fg)
                .bg(palette.fkey_bar_bg);
            fill_rect(
                buf,
                Rect::new(inner.x, footer_y, inner.width, 1),
                footer_style,
            );
            write_clipped_line(
                buf,
                inner.x,
                footer_y,
                inner.width,
                "j/k:Nav Ent:Go Spc:Peek x:Close z:Zoom d:Edge m:Mode p:Pin",
                footer_style,
            );
        }
    } else {
        // Horizontal ribbon/bar mode (Top / Bottom):
        // Row 0: compact horizontal ribbon of session + pane pills
        let mut ribbon = String::new();
        for (idx, row) in rows.iter().enumerate() {
            if !ribbon.is_empty() {
                ribbon.push_str(" │ ");
            }
            let marker = if idx == selected { "»" } else { "" };
            match &row.kind {
                DrawerRowKind::SessionHeader(s) => {
                    ribbon.push_str(&format!("{marker}◈ {s}"));
                }
                DrawerRowKind::LayoutGroup(_) => {
                    ribbon.push_str(&format!("{marker}{} {}", row.connector, row.title));
                }
                DrawerRowKind::Pane(_) | DrawerRowKind::QuickTerminal => {
                    ribbon.push_str(&format!(
                        "{marker}{} {} {} {} {}",
                        row.connector,
                        row.focus_rank_badge,
                        row.spinner,
                        row.sparkline,
                        row.cmd_badge
                    ));
                }
                DrawerRowKind::FleetNode(n) => {
                    ribbon.push_str(&format!("{marker}{} @{n} {}", row.connector, row.cmd_badge));
                }
            }
        }
        let ribbon_style = Style::default()
            .fg(palette.title_fg)
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD);
        write_clipped_line(buf, inner.x, inner.y, inner.width, &ribbon, ribbon_style);

        // Remaining rows: full telemetry lines starting from selected / pane rows
        let remaining_h = inner.height.saturating_sub(1) as usize;
        for (vis_idx, (row_idx, row)) in rows
            .iter()
            .enumerate()
            .skip(selected.saturating_sub(remaining_h.saturating_sub(1)))
            .take(remaining_h)
            .enumerate()
        {
            let y = inner.y + 1 + vis_idx as u16;
            let mut style = base_style;
            if row_idx == selected {
                style = style.add_modifier(Modifier::REVERSED);
                fill_rect(buf, Rect::new(inner.x, y, inner.width, 1), style);
            }
            write_clipped_line(buf, inner.x, y, inner.width, &row.formatted_line(), style);
        }
    }
}
