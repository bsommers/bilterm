//! TRON & ZenTerm/REX Liquid Glass Master Control Program (`MCP`) Interactive Console (`MCP-01..03`).
//!
//! Renders the floating (`Overlay`) or docked (`Push`) MCP control deck with interactive
//! cursor selection, mouse hit-target rows, and either classic TRON crimson/cyan/gold cyber-grid
//! chrome or a modern ZenTerm/REX Liquid Glass multi-row control deck.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::config::BiltermConfig;
use crate::daemon::DaemonServerState;
use crate::mcp_console::{McpConsoleState, McpRowAction};
use crate::ui::chrome::render_drop_shadow_styled;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{DrawerEdge, DrawerLayoutMode, WindowManager};

/// Computes `(header_rows_inside_border, footer_rows_inside_border)` for the MCP console.
pub fn mcp_console_layout_metrics(cfg: &BiltermConfig, outer_height: u16) -> (u16, u16) {
    if cfg.is_modern() && outer_height >= 8 {
        (2, 1)
    } else {
        (0, 0)
    }
}

/// Renders the MCP overlay or push-out panel on top of `workspace_area` (`MCP-01..03`).
pub fn render_mcp_console_overlay(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    let Some(mcp_rect) = wm.mcp_console().panel_rect(workspace_area) else {
        return;
    };
    if wm.mcp_console().layout_mode == DrawerLayoutMode::Overlay {
        if cfg.liquid_glass {
            render_liquid_ambient_halo(buf, mcp_rect, workspace_area, palette);
        } else if cfg.drop_shadows {
            render_drop_shadow_styled(buf, mcp_rect, workspace_area, cfg.shadow_style, palette);
        }
    }
    occlude_rect(buf, mcp_rect);
    if cfg.is_modern() {
        render_mcp_console_modern(
            buf,
            mcp_rect,
            wm.mcp_console(),
            wm.daemon_state(),
            cfg,
            palette,
        );
    } else {
        render_mcp_console(buf, mcp_rect, wm.mcp_console(), wm.daemon_state());
    }
}

fn render_mcp_console_modern(
    buf: &mut Buffer,
    area: Rect,
    state: &McpConsoleState,
    daemon: &DaemonServerState,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
) {
    let bounds = area.intersection(*buf.area());
    if bounds.width < 4 || bounds.height < 3 {
        return;
    }

    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, bounds, palette.window_frame_bg, palette);
    }

    let bg_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg);
    let border_style = Style::default()
        .fg(palette.active_border_fg)
        .bg(palette.window_frame_bg);
    let header_pill_style = Style::default()
        .fg(palette.prompt_prefix_fg)
        .bg(palette.oia_bg)
        .add_modifier(Modifier::BOLD);
    let section_style = Style::default()
        .fg(palette.prompt_prefix_fg)
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    let selected_style = Style::default()
        .fg(palette.title_fg)
        .bg(if cfg.liquid_glass {
            Color::Rgb(24, 46, 78)
        } else {
            Color::Rgb(30, 41, 59)
        })
        .add_modifier(Modifier::BOLD);

    if !cfg.is_glass() {
        for y in bounds.y..bounds.y + bounds.height {
            for x in bounds.x..bounds.x + bounds.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    // Rounded modern hairline border
    let right = bounds.x + bounds.width - 1;
    let bottom = bounds.y + bounds.height - 1;
    for x in bounds.x..=right {
        if let Some(top_c) = buf.cell_mut((x, bounds.y)) {
            top_c.set_symbol("─");
            top_c.set_style(border_style);
        }
        if let Some(bot_c) = buf.cell_mut((x, bottom)) {
            bot_c.set_symbol("─");
            bot_c.set_style(border_style);
        }
    }
    for y in bounds.y..=bottom {
        if let Some(left_c) = buf.cell_mut((bounds.x, y)) {
            left_c.set_symbol("│");
            left_c.set_style(border_style);
        }
        if let Some(right_c) = buf.cell_mut((right, y)) {
            right_c.set_symbol("│");
            right_c.set_style(border_style);
        }
    }
    if let Some(c) = buf.cell_mut((bounds.x, bounds.y)) {
        c.set_symbol("╭");
    }
    if let Some(c) = buf.cell_mut((right, bounds.y)) {
        c.set_symbol("╮");
    }
    if let Some(c) = buf.cell_mut((bounds.x, bottom)) {
        c.set_symbol("╰");
    }
    if let Some(c) = buf.cell_mut((right, bottom)) {
        c.set_symbol("╯");
    }

    let mode_tag = match state.layout_mode {
        DrawerLayoutMode::Overlay => "Floating",
        DrawerLayoutMode::Push => state.edge.label(),
    };
    let badge = if cfg.liquid_glass { "💧" } else { "◢" };
    let title = format!(" ● ● ●  {badge} Control Deck · {mode_tag} ");
    write_clipped(
        buf,
        bounds.x + 2,
        bounds.y,
        right.saturating_sub(5),
        &title,
        Style::default()
            .fg(palette.title_fg)
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    );
    for (lx, lcol) in [
        (bounds.x + 3, Color::Rgb(255, 95, 87)),
        (bounds.x + 5, Color::Rgb(254, 188, 46)),
        (bounds.x + 7, Color::Rgb(40, 200, 64)),
    ] {
        if lx < right.saturating_sub(1)
            && let Some(c) = buf.cell_mut((lx, bounds.y))
        {
            c.set_fg(lcol);
        }
    }
    if right >= bounds.x + 12 {
        write_clipped(
            buf,
            right.saturating_sub(4),
            bounds.y,
            right,
            " × ",
            Style::default()
                .fg(Color::Rgb(255, 95, 87))
                .bg(palette.window_frame_bg)
                .add_modifier(Modifier::BOLD),
        );
    }

    let (hdr_rows, ftr_rows) = mcp_console_layout_metrics(cfg, bounds.height);
    if hdr_rows >= 2 {
        for x in bounds.x + 1..right {
            if let Some(c) = buf.cell_mut((x, bounds.y + 1)) {
                c.set_symbol(" ");
                c.set_style(header_pill_style);
            }
            if let Some(c) = buf.cell_mut((x, bounds.y + 2)) {
                c.set_symbol("─");
                c.set_style(Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg));
            }
        }
        let status_dot = if daemon.running { "● ONLINE" } else { "○ LOCAL" };
        let sub_header = format!(
            "  {status_dot}   ·   ⧉ Layout: {} (m)   ·   ◧ Edge: {} (d)   ·   ↻ Refresh (r)   ·   ✕ Close (Esc)",
            state.layout_mode.label(),
            state.edge.label()
        );
        write_clipped(
            buf,
            bounds.x + 1,
            bounds.y + 1,
            right,
            &sub_header,
            header_pill_style,
        );
    }

    if ftr_rows >= 1 {
        let sep_y = bottom.saturating_sub(1);
        for x in bounds.x + 1..right {
            if let Some(c) = buf.cell_mut((x, sep_y)) {
                c.set_symbol("─");
                c.set_style(Style::default().fg(palette.inactive_border_fg).bg(palette.window_frame_bg));
            }
        }
    }

    let footer = "  ↑↓ Select  ·  ⏎ Activate  ·  r Refresh  ·  f Freeze  ·  n Branch  ·  m Float/Push  ·  d Edge  ·  Esc Close  ";
    write_clipped(
        buf,
        bounds.x + 2,
        bottom,
        right.saturating_sub(1),
        footer,
        header_pill_style,
    );

    let mut row = bounds.y + 1 + hdr_rows;
    let max_row = bottom.saturating_sub(ftr_rows);
    let inner_left = bounds.x + 2;
    let inner_right = right.saturating_sub(1);

    if !state.view_lines.is_empty() {
        for (idx, line) in state.view_lines.iter().skip(state.scroll_offset).enumerate() {
            if row >= max_row {
                break;
            }
            let row_idx = state.scroll_offset + idx;
            let is_selected = row_idx == state.selected_row;
            let is_interactive = state
                .row_actions
                .get(row_idx)
                .is_some_and(|a| *a != McpRowAction::None);

            if is_selected {
                for x in bounds.x + 1..right {
                    if let Some(cell) = buf.cell_mut((x, row)) {
                        cell.set_symbol(" ");
                        cell.set_style(selected_style);
                    }
                }
                let display_line = if is_interactive {
                    format!("▎ ► {line}")
                } else {
                    format!("▎ {line}")
                };
                write_clipped(buf, inner_left, row, inner_right, &display_line, selected_style);
            } else {
                let st = if line.starts_with('◈')
                    || line.starts_with("DAEMON")
                    || line.starts_with("WORKSPACE")
                {
                    section_style
                } else {
                    bg_style
                };
                let padded = format!("  {line}");
                write_clipped(buf, inner_left, row, inner_right, &padded, st);
            }
            row += 1;
        }
    } else if row < max_row {
        let header = format!(
            "DAEMON: {} | SESSIONS: {} | ATTACHED CLIENTS: {} | BANNER: {}",
            if daemon.running { "ONLINE" } else { "OFFLINE" },
            daemon.session_tree.nodes.len(),
            daemon.clients.len(),
            state.last_status_banner
        );
        write_clipped(buf, inner_left, row, inner_right, &header, section_style);
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(buf, bounds, palette.window_frame_bg, palette, true);
    }
}

/// Renders the TRON-inspired Master Control Program (`MCP`) root dashboard into `buf` (`MCP-01..03`).
///
/// Safe for any `area` dimensions (including `0×0` or tiny viewports).
pub fn render_mcp_console(
    buf: &mut Buffer,
    area: Rect,
    state: &McpConsoleState,
    daemon: &DaemonServerState,
) {
    let bounds = area.intersection(*buf.area());
    if bounds.width < 4 || bounds.height < 3 {
        return;
    }

    let bg_style = Style::default()
        .fg(Color::Rgb(0x00, 0xF0, 0xFF))
        .bg(Color::Rgb(0x08, 0x04, 0x10));
    let border_style = Style::default()
        .fg(Color::Rgb(0xFF, 0x2A, 0x2A))
        .bg(Color::Rgb(0x08, 0x04, 0x10))
        .add_modifier(Modifier::BOLD);
    let gold_style = Style::default()
        .fg(Color::Rgb(0xFF, 0xCC, 0x00))
        .bg(Color::Rgb(0x08, 0x04, 0x10))
        .add_modifier(Modifier::BOLD);
    let selected_style = Style::default()
        .fg(Color::Rgb(255, 255, 120))
        .bg(Color::Rgb(0, 90, 130))
        .add_modifier(Modifier::BOLD);

    for y in bounds.y..bounds.y + bounds.height {
        for x in bounds.x..bounds.x + bounds.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(bg_style);
            }
        }
    }

    // Double-line TRON crimson cyber-grid border
    let right = bounds.x + bounds.width - 1;
    let bottom = bounds.y + bounds.height - 1;
    for x in bounds.x..=right {
        if let Some(top_c) = buf.cell_mut((x, bounds.y)) {
            top_c.set_symbol("═");
            top_c.set_style(border_style);
        }
        if let Some(bot_c) = buf.cell_mut((x, bottom)) {
            bot_c.set_symbol("═");
            bot_c.set_style(border_style);
        }
    }
    for y in bounds.y..=bottom {
        if let Some(left_c) = buf.cell_mut((bounds.x, y)) {
            left_c.set_symbol("║");
            left_c.set_style(border_style);
        }
        if let Some(right_c) = buf.cell_mut((right, y)) {
            right_c.set_symbol("║");
            right_c.set_style(border_style);
        }
    }
    if let Some(c) = buf.cell_mut((bounds.x, bounds.y)) {
        c.set_symbol("╔");
    }
    if let Some(c) = buf.cell_mut((right, bounds.y)) {
        c.set_symbol("╗");
    }
    if let Some(c) = buf.cell_mut((bounds.x, bottom)) {
        c.set_symbol("╚");
    }
    if let Some(c) = buf.cell_mut((right, bottom)) {
        c.set_symbol("╝");
    }

    let mode_tag = match state.layout_mode {
        DrawerLayoutMode::Overlay => "[m:FLOAT]",
        DrawerLayoutMode::Push => match state.edge {
            DrawerEdge::Top => "[m:PUSH-TOP][d:Edge]",
            DrawerEdge::Bottom => "[m:PUSH-BOT][d:Edge]",
            DrawerEdge::Left => "[m:PUSH-LEFT][d:Edge]",
            DrawerEdge::Right => "[m:PUSH-RIGHT][d:Edge]",
        },
    };
    let title = format!(" ◈ {} {mode_tag} [X] ", state.title);
    write_clipped(buf, bounds.x + 2, bounds.y, right.saturating_sub(1), &title, gold_style);

    let footer = " ↑/↓:Select │ Enter/Click:Activate │ r:Refresh │ f:Freeze │ n:Branch │ m:Float/Push │ d:Edge │ Esc:Close ";
    write_clipped(buf, bounds.x + 2, bottom, right.saturating_sub(1), footer, gold_style);

    let mut row = bounds.y + 1;
    let max_row = bottom;
    let inner_left = bounds.x + 2;
    let inner_right = right.saturating_sub(1);

    if !state.view_lines.is_empty() {
        for (idx, line) in state.view_lines.iter().skip(state.scroll_offset).enumerate() {
            if row >= max_row {
                break;
            }
            let row_idx = state.scroll_offset + idx;
            let is_selected = row_idx == state.selected_row;
            let is_interactive = state
                .row_actions
                .get(row_idx)
                .is_some_and(|a| *a != McpRowAction::None);

            if is_selected {
                for x in bounds.x + 1..right {
                    if let Some(cell) = buf.cell_mut((x, row)) {
                        cell.set_symbol(" ");
                        cell.set_style(selected_style);
                    }
                }
                let display_line = if is_interactive {
                    format!("► {line}")
                } else {
                    line.clone()
                };
                write_clipped(buf, inner_left, row, inner_right, &display_line, selected_style);
            } else {
                let st = if line.starts_with('◈')
                    || line.starts_with("DAEMON")
                    || line.starts_with("WORKSPACE")
                {
                    gold_style
                } else {
                    bg_style
                };
                write_clipped(buf, inner_left, row, inner_right, line, st);
            }
            row += 1;
        }
        return;
    }

    if row < max_row {
        let header = format!(
            "DAEMON: {} | SESSIONS: {} | ATTACHED CLIENTS: {} | BANNER: {}",
            if daemon.running { "ONLINE" } else { "OFFLINE" },
            daemon.session_tree.nodes.len(),
            daemon.clients.len(),
            state.last_status_banner
        );
        write_clipped(buf, inner_left, row, inner_right, &header, gold_style);
        row += 1;
    }

    for tree_line in daemon.session_tree.render_herdr_tree_lines("main") {
        if row >= max_row {
            break;
        }
        write_clipped(buf, inner_left, row, inner_right, &tree_line, bg_style);
        row += 1;
    }

    let tail_count = (max_row.saturating_sub(row)) as usize;
    let start_idx = state.log_lines.len().saturating_sub(tail_count);
    for log_line in &state.log_lines[start_idx..] {
        if row >= max_row {
            break;
        }
        write_clipped(buf, inner_left, row, inner_right, log_line, border_style);
        row += 1;
    }
}

fn write_clipped(
    buf: &mut Buffer,
    start_x: u16,
    y: u16,
    max_x: u16,
    text: &str,
    style: Style,
) {
    let area = *buf.area();
    if y < area.y || y >= area.y + area.height || start_x >= area.x + area.width || max_x <= start_x {
        return;
    }
    let avail = max_x.min(area.x + area.width).saturating_sub(start_x);
    buf.set_stringn(start_x, y, text, usize::from(avail), style);
}
