//! `navigator`-Style & ZenTerm/REX Liquid Glass Slide-Out Sessions / Windows / Panes & Fleet Drawer Renderer (`DRW-01`, `DRW-02`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub mod navigator;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{
    DrawerLayoutMode, DrawerRow, DrawerRowKind, WindowManager, WorkspaceLayoutMode,
    inset_window_frame,
};

/// Click target kind in the Classic drawer title/header row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicHeaderChipKind {
    DockEdge,
    LayoutMode,
    Visibility,
    NewWindow,
    NewSession,
    RowLines(u8),
}

/// Computes `(header_rows, footer_rows, rows_per_item)` for vertical drawer layout and mouse hit-testing.
pub fn drawer_vertical_layout_metrics(
    cfg: &BiltermConfig,
    inner_height: u16,
    item_count: usize,
) -> (u16, u16, usize) {
    let (header_h, footer_h, auto_two) = if cfg.is_modern() && inner_height >= 10 {
        let header_h = 2u16;
        let footer_h = 2u16;
        let avail = usize::from(inner_height.saturating_sub(header_h + footer_h)).max(1);
        let auto =
            cfg.liquid_glass && inner_height >= 14 && item_count.saturating_mul(2) <= avail;
        (header_h, footer_h, auto)
    } else {
        let footer_h = u16::from(inner_height >= 3);
        (1, footer_h, false)
    };
    let avail = usize::from(inner_height.saturating_sub(header_h + footer_h));
    let step = match cfg.drawer.classic_row_lines_mode() {
        2 if avail >= 2 => 2,
        1 => 1,
        0 if auto_two => 2,
        _ => 1,
    };
    (header_h, footer_h, step)
}

/// Computes the clickable header chips `(label, rel_x_range, kind)` for the Classic drawer
/// (`rel_x` is relative to `drawer_rect.x`; `title_row = true` for `dr.y`, `false` for `dr.y + 1`).
/// When `drawer_width` is too narrow for the `[R:n]` chip, `[R:n]` is dropped first so `[+Win]`
/// and `[+Sess]` keep their existing column regions.
pub fn classic_header_chips(
    wm: &WindowManager,
    drawer_width: u16,
    inner_height: u16,
    title_row: bool,
) -> Vec<(String, std::ops::Range<u16>, ClassicHeaderChipKind)> {
    let dcfg = wm.drawer_config();
    let is_modern = wm.config().is_modern();
    let (_, _, row_step) =
        drawer_vertical_layout_metrics(wm.config(), inner_height, wm.drawer_rows().len());
    let step_u8 = row_step as u8;
    let r_label = format!("[R:{step_u8}]");

    if title_row {
        let mut chips = vec![
            (format!("[D:{}]", dcfg.edge.label()), 0..16, ClassicHeaderChipKind::DockEdge),
            (format!("[M:{}]", dcfg.layout.label()), 16..26, ClassicHeaderChipKind::LayoutMode),
            (format!("[P:{}]", dcfg.visibility.label()), 26..42, ClassicHeaderChipKind::Visibility),
        ];
        if !is_modern && dcfg.edge.is_vertical() && drawer_width >= 58 {
            chips.push((r_label, 42..48, ClassicHeaderChipKind::RowLines(step_u8)));
        } else if let Some(last) = chips.last_mut() {
            last.1.end = drawer_width;
        }
        return chips;
    }

    let win_start = 31.min(drawer_width.saturating_sub(8));
    let mut chips = vec![
        (format!("[D:{}]", dcfg.edge.label()), 0..10, ClassicHeaderChipKind::DockEdge),
        (format!("[M:{}]", dcfg.layout.label()), 10..20, ClassicHeaderChipKind::LayoutMode),
        (format!("[P:{}]", dcfg.visibility.label()), 20..win_start, ClassicHeaderChipKind::Visibility),
        ("[+Win]".to_string(), win_start..38.min(drawer_width), ClassicHeaderChipKind::NewWindow),
    ];
    if drawer_width > 38 {
        let r_min_width = if is_modern { 64 } else { 48 };
        if dcfg.edge.is_vertical() && drawer_width >= r_min_width {
            let r_start = if is_modern { 56 } else { 46 };
            chips.push((
                "[+Sess]".to_string(),
                38..r_start,
                ClassicHeaderChipKind::NewSession,
            ));
            chips.push((
                r_label,
                r_start..drawer_width,
                ClassicHeaderChipKind::RowLines(step_u8),
            ));
        } else {
            chips.push((
                "[+Sess]".to_string(),
                38..drawer_width,
                ClassicHeaderChipKind::NewSession,
            ));
        }
    }
    chips
}

/// Writes a single-line string into `buf` at `(x, y)` clipped to `max_cols` display columns.
fn write_clipped_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    max_cols: u16,
    text: &str,
    style: Style,
) {
    let area = *buf.area();
    if y < area.y || y >= area.y + area.height || x >= area.x + area.width || max_cols == 0 {
        return;
    }
    let avail = max_cols.min((area.x + area.width).saturating_sub(x));
    buf.set_stringn(x, y, text, usize::from(avail), style);
}

/// Drops the first `h` chars of `text` (marking the cut with `‹`); `h == 0` returns the text unchanged.
fn h_clip(text: &str, h: usize) -> String {
    if h == 0 {
        text.to_string()
    } else {
        std::iter::once('‹').chain(text.chars().skip(h + 1)).collect()
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

/// Formats a spacious 2-row graphical card `(primary_line, secondary_line)` for Liquid Glass mode.
fn format_liquid_drawer_card(row: &DrawerRow, is_sel: bool, pane_count: usize) -> (String, String) {
    let bar = if is_sel { "▎" } else { " " };
    match &row.kind {
        DrawerRowKind::SessionHeader(name) => {
            let badges = row
                .mode_badges
                .iter()
                .chain(row.transport_badges.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join(" · ");
            let top = if badges.is_empty() {
                format!(" {bar} ◈ {name}")
            } else {
                format!(" {bar} ◈ {name}  ·  {badges}")
            };
            let bot = format!(" {bar}   Active Workspace · {pane_count} Panes");
            (top, bot)
        }
        DrawerRowKind::LayoutGroup(mode) => {
            let label = match mode {
                WorkspaceLayoutMode::Tiled => "Tiled Split Layout",
                WorkspaceLayoutMode::Stacked => "Stacked Z-Order Deck",
            };
            (
                format!(" {bar} ▾ {label}"),
                format!(" {bar}   {} Workspace Topology", row.connector),
            )
        }
        DrawerRowKind::Pane(pid) => {
            let dot = if row.is_focused_pane { "●" } else { "○" };
            let top = format!(
                " {bar} {dot} #{pid} · {}  {}",
                row.title, row.cmd_badge
            );
            let mut tags = Vec::new();
            for b in row.mode_badges.iter().chain(row.transport_badges.iter()) {
                tags.push(b.as_str());
            }
            let tag_str = if tags.is_empty() {
                String::from("Ready")
            } else {
                tags.join(" · ")
            };
            let bot = format!(" {bar}   {} {}  ·  {tag_str}", row.spinner, row.sparkline);
            (top, bot)
        }
        DrawerRowKind::QuickTerminal => {
            let dot = if row.is_focused_pane { "●" } else { "○" };
            let top = format!(" {bar} {dot} Quick Terminal  {}", row.cmd_badge);
            let bot = format!(" {bar}   {} {}  ·  Drop-Down", row.spinner, row.sparkline);
            (top, bot)
        }
        DrawerRowKind::FleetNode(_) => {
            let top = format!(
                " {bar} ◈ {}  {}",
                row.title, row.cmd_badge
            );
            let bot = format!(" {bar}   {}", row.transport_badges.join(" · "));
            (top, bot)
        }
    }
}

/// Renders the Navigator-style or ZenTerm/REX Liquid Glass slide-out drawer when visible (`DRW-01`, `DRW-02`).
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
    if wm.drawer_config().style == crate::wm::drawer::DrawerStyle::Navigator {
        navigator::render_navigator_drawer(wm, workspace_area, drawer_rect, cfg, palette, glyphs, buf);
        return;
    }
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

    let drawer_prefix = if cfg.liquid_glass {
        "💧 SESSIONS"
    } else if is_modern {
        "◢ SESSIONS"
    } else if cfg.snazzy_mode {
        "✨ SESSIONS"
    } else {
        "SESSIONS"
    };
    let inner_h = drawer_rect.height.saturating_sub(2);
    let show_auto_chip = !(drawer.maximized && dcfg.classic_row_lines_mode() == 0);
    let title_chips = classic_header_chips(wm, drawer_rect.width, inner_h, true);
    let title_r_chip = title_chips.iter().find_map(|(lbl, _, kind)| match kind {
        ClassicHeaderChipKind::RowLines(_) if show_auto_chip => Some(lbl.as_str()),
        _ => None,
    });
    let title = if is_modern {
        format!("● ● ●  {drawer_prefix}")
    } else if let Some(r_lbl) = title_r_chip {
        format!(
            "{drawer_prefix} [D:{}] [M:{}] [P:{}] {r_lbl}",
            dcfg.edge.label(),
            dcfg.layout.label(),
            dcfg.visibility.label()
        )
    } else {
        format!(
            "{drawer_prefix} [D:{}] [M:{}] [P:{}]",
            dcfg.edge.label(),
            dcfg.layout.label(),
            dcfg.visibility.label()
        )
    };
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
        .fg(palette.text_fg.unwrap_or(palette.inactive_border_fg))
        .bg(palette.window_frame_bg);
    if !cfg.is_glass() {
        fill_rect(buf, inner, base_style);
    }

    let rows = wm.drawer_rows();
    let selected = drawer.selected_index.min(rows.len().saturating_sub(1));

    if dcfg.edge.is_vertical() {
        let (header_h, footer_h, row_step) =
            drawer_vertical_layout_metrics(cfg, inner.height, rows.len());
        let header_style = Style::default()
            .fg(palette.title_fg)
            .bg(palette.oia_bg)
            .add_modifier(Modifier::BOLD);
        fill_rect(buf, Rect::new(inner.x, inner.y, inner.width, 1), header_style);
        let hdr_chips = classic_header_chips(wm, drawer_rect.width, inner.height, false);
        let hdr_r_chip = hdr_chips.iter().find_map(|(lbl, _, kind)| match kind {
            ClassicHeaderChipKind::RowLines(_) if show_auto_chip => Some(lbl.as_str()),
            _ => None,
        });
        let header_line = if is_modern {
            if let Some(r_lbl) = hdr_r_chip {
                format!(
                    " ◧ {}  ·  ⧉ {}  ·  ◈ {}  ·  +Win  ·  +Sess  ·  {r_lbl}",
                    dcfg.edge.label(),
                    dcfg.layout.label(),
                    dcfg.visibility.label()
                )
            } else {
                format!(
                    " ◧ {}  ·  ⧉ {}  ·  ◈ {}  ·  +Win  ·  +Sess",
                    dcfg.edge.label(),
                    dcfg.layout.label(),
                    dcfg.visibility.label()
                )
            }
        } else if let Some(r_lbl) = hdr_r_chip {
            format!(
                "[D:{}] [M:{}] [P:{}] [+Win] [+Sess] {r_lbl}",
                dcfg.edge.label(),
                dcfg.layout.label(),
                dcfg.visibility.label()
            )
        } else {
            format!(
                "[D:{}] [M:{}] [P:{}] [+Win] [+Sess]",
                dcfg.edge.label(),
                dcfg.layout.label(),
                dcfg.visibility.label()
            )
        };
        write_clipped_line(buf, inner.x, inner.y, inner.width, &header_line, header_style);

        if header_h >= 2 {
            let sub_style = Style::default()
                .fg(palette.inactive_border_fg)
                .bg(palette.window_frame_bg);
            let divider: String = "─".repeat(usize::from(inner.width));
            write_clipped_line(buf, inner.x, inner.y + 1, inner.width, &divider, sub_style);
        }

        let list_y = inner.y + header_h;
        let list_rows_px = inner.height.saturating_sub(header_h + footer_h).max(1) as usize;
        let list_items_cap = (list_rows_px / row_step).max(1);
        let scroll = drawer.effective_scroll(selected, list_items_cap, rows.len());
        let show_scrollbar = rows.len() > list_items_cap && inner.width >= 8;
        let row_text_w = if show_scrollbar {
            inner.width.saturating_sub(1)
        } else {
            inner.width
        };

        let sel_card_bg = if cfg.liquid_glass {
            Color::Rgb(24, 44, 74)
        } else {
            Color::Rgb(30, 41, 59)
        };

        for (vis_idx, (row_idx, row)) in rows
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_items_cap)
            .enumerate()
        {
            let y = list_y + (vis_idx * row_step) as u16;
            if y >= inner.y + inner.height.saturating_sub(footer_h) {
                break;
            }
            let is_sel = row_idx == selected;
            let inline_override = match (&row.kind, wm.window_manage().inline_rename.as_ref()) {
                (DrawerRowKind::Pane(pid), Some(ir)) if ir.pane_id == *pid => {
                    Some(format!("[ {}█ ]", ir.current_input))
                }
                _ => None,
            };
            if row_step == 2 {
                let mut row_cow;
                let row_ref = if let Some(ref ov) = inline_override {
                    row_cow = row.clone();
                    row_cow.title = ov.clone();
                    &row_cow
                } else {
                    row
                };
                let (line1, line2) = format_liquid_drawer_card(row_ref, is_sel, wm.pane_count());
                let bg = if is_sel {
                    sel_card_bg
                } else {
                    palette.window_frame_bg
                };
                let fg1 = if is_sel || row.is_focused_pane {
                    palette.title_fg
                } else {
                    palette.text_fg.unwrap_or(palette.inactive_border_fg)
                };
                let fg2 = if is_sel {
                    palette.prompt_prefix_fg
                } else {
                    palette.controls_fg.unwrap_or(palette.inactive_border_fg)
                };
                let st1 = Style::default()
                    .fg(fg1)
                    .bg(bg)
                    .add_modifier(if is_sel || row.is_focused_pane {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
                let st2 = Style::default().fg(fg2).bg(bg);
                fill_rect(buf, Rect::new(inner.x, y, row_text_w, 2), st2);
                write_clipped_line(buf, inner.x, y, row_text_w, &h_clip(&line1, drawer.h_scroll), st1);
                if y + 1 < inner.y + inner.height.saturating_sub(footer_h) {
                    write_clipped_line(buf, inner.x, y + 1, row_text_w, &h_clip(&line2, drawer.h_scroll), st2);
                }
            } else {
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
                    if is_modern {
                        row_style = Style::default()
                            .fg(palette.title_fg)
                            .bg(sel_card_bg)
                            .add_modifier(Modifier::BOLD);
                    } else {
                        row_style = row_style.add_modifier(Modifier::REVERSED);
                    }
                    fill_rect(buf, Rect::new(inner.x, y, row_text_w, 1), row_style);
                }
                let base_line = if let Some(ref ov) = inline_override {
                    let mut row_cow = row.clone();
                    row_cow.title = ov.clone();
                    row_cow.formatted_line()
                } else {
                    row.formatted_line()
                };
                let line = if is_modern && is_sel {
                    format!("▎{base_line}")
                } else {
                    base_line
                };
                write_clipped_line(buf, inner.x, y, row_text_w, &h_clip(&line, drawer.h_scroll), row_style);
            }
        }

        if show_scrollbar {
            let sb_x = inner.x + inner.width - 1;
            let sb_style = Style::default()
                .fg(palette.active_border_fg)
                .bg(palette.window_frame_bg);
            let thumb_pos = if rows.len() <= 1 {
                0
            } else {
                (selected * list_rows_px.saturating_sub(1)) / (rows.len() - 1)
            };
            for r in 0..list_rows_px {
                let y = list_y + r as u16;
                let sym = if is_modern {
                    if r == thumb_pos { "┃" } else { "│" }
                } else if r == 0 && scroll > 0 {
                    "▲"
                } else if r + 1 == list_rows_px && scroll + list_items_cap < rows.len() {
                    "▼"
                } else if r == thumb_pos {
                    "█"
                } else {
                    "░"
                };
                if let Some(cell) = buf.cell_mut((sb_x, y)) {
                    cell.set_symbol(sym);
                    cell.set_style(sb_style);
                }
            }
        }

        if footer_h >= 1 {
            if footer_h >= 2 {
                let sep_y = inner.y + inner.height - 2;
                let sep_style = Style::default()
                    .fg(palette.inactive_border_fg)
                    .bg(palette.window_frame_bg);
                let divider: String = "─".repeat(usize::from(inner.width));
                write_clipped_line(buf, inner.x, sep_y, inner.width, &divider, sep_style);
            }
            let footer_y = inner.y + inner.height - 1;
            let footer_style = Style::default()
                .fg(palette.fkey_bar_fg)
                .bg(palette.fkey_bar_bg);
            fill_rect(
                buf,
                Rect::new(inner.x, footer_y, inner.width, 1),
                footer_style,
            );
            let footer_text = if wm.is_leader_pending() {
                " ⌨ LEADER [Ctrl+X] — d:Close  c:NewWin  N:NewSess  ?:Help "
            } else if is_modern {
                " +Win · +Sess · JK Move · <> Par · F7/^Xd"
            } else {
                "[+Win][+Sess] JK:Move <>:Par F7"
            };
            write_clipped_line(
                buf,
                inner.x,
                footer_y,
                inner.width,
                footer_text,
                footer_style,
            );
        }
    } else {
        // Horizontal ribbon/bar mode (Top / Bottom)
        let mut ribbon = String::new();
        for (idx, row) in rows.iter().enumerate() {
            if !ribbon.is_empty() {
                ribbon.push_str(if is_modern { "  ·  " } else { " │ " });
            }
            let marker = if idx == selected {
                if is_modern { "▎" } else { "»" }
            } else {
                ""
            };
            match &row.kind {
                DrawerRowKind::SessionHeader(s) => ribbon.push_str(&format!("{marker}◈ {s}")),
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
            .bg(palette.oia_bg)
            .add_modifier(Modifier::BOLD);
        fill_rect(buf, Rect::new(inner.x, inner.y, inner.width, 1), ribbon_style);
        write_clipped_line(buf, inner.x, inner.y, inner.width, &ribbon, ribbon_style);

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
                style = if is_modern {
                    Style::default()
                        .fg(palette.title_fg)
                        .bg(Color::Rgb(24, 44, 74))
                        .add_modifier(Modifier::BOLD)
                } else {
                    style.add_modifier(Modifier::REVERSED)
                };
                fill_rect(buf, Rect::new(inner.x, y, inner.width, 1), style);
            }
            write_clipped_line(buf, inner.x, y, inner.width, &row.formatted_line(), style);
        }
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(
            buf,
            drawer_rect,
            palette.window_frame_bg,
            palette,
            drawer.focused,
        );
    }
}
