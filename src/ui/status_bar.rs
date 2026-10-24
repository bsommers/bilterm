//! DOS function-key bar, IBM 3270 Operator Information Area (OIA) status bar, and floating command prompt renderers.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::highlight::tokenize_shell_line;
use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::chrome::OutlineStyle;
use crate::ui::cursor::{
    SpotlightMode, render_caret, render_crosshair_cursor, render_focal_zoom_cursor,
    render_spotlight_cursor,
};
use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;
use crate::vt::Osc9Progress;

/// Renders the classic Borland Turbo Vision / DOS IDE bottom Function-Key status bar (`UI-03`).
pub fn render_dos_fkey_bar(rect: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let items = [
        ("F1", "Help "),
        ("F2", "Split "),
        ("F5", "Zoom "),
        ("F6", "Next "),
        ("F10", "Quake "),
        ("F12", "Raw "),
    ];

    let mut spans = Vec::new();
    for (key, label) in items {
        spans.push(Span::styled(
            format!(" {key}"),
            Style::default()
                .fg(palette.fkey_num_fg)
                .bg(palette.fkey_bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {label}"),
            Style::default()
                .fg(palette.fkey_bar_fg)
                .bg(palette.fkey_bar_bg),
        ));
    }

    spans.push(Span::styled(
        format!("[{}]", palette.name),
        Style::default()
            .fg(palette.fkey_num_fg)
            .bg(palette.fkey_bar_bg)
            .add_modifier(Modifier::BOLD),
    ));

    Block::default()
        .style(
            Style::default()
                .fg(palette.fkey_bar_fg)
                .bg(palette.fkey_bar_bg),
        )
        .render(rect, buf);
    Paragraph::new(Line::from(spans)).render(rect, buf);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_floating_prompt(
    pane: &CompositePane,
    active: bool,
    prompt_rect: Rect,
    canvas_rect: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let effective_outline = if !active && cfg.prompt_outline == OutlineStyle::Double {
        OutlineStyle::Solid
    } else {
        cfg.prompt_outline
    };
    let border_fg = if active {
        palette.active_border_fg
    } else {
        palette.inactive_border_fg
    };
    let surface_bg = palette.prompt_surface_bg;

    let anchor_badge = match pane.prompt().anchor {
        PromptAnchor::Top => "TOP",
        PromptAnchor::Bottom => "BOT",
        PromptAnchor::SplitTop => "SPL-T",
        PromptAnchor::SplitBottom => "SPL-B",
        PromptAnchor::Floating { y_offset } => {
            if y_offset == 0 {
                "TOP"
            } else {
                "FLT"
            }
        }
    };

    let title = format!("╣ BILTERM 3270 // CMD [{anchor_badge}] ╠");
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(palette.title_fg)
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_set(effective_outline.border_set())
        .border_style(Style::default().fg(border_fg).bg(surface_bg))
        .style(Style::default().fg(Color::White).bg(surface_bg));

    let inner = block.inner(prompt_rect);
    block.render(prompt_rect, buf);

    // Render clickable [▲][▼] pin badges on the top-right border when width >= 44
    if prompt_rect.width >= 44 {
        let right_edge = prompt_rect.x + prompt_rect.width;
        let pin_x = right_edge.saturating_sub(8);
        let pin_str = glyphs.pin_top_bottom;
        for (idx, ch) in pin_str.chars().enumerate() {
            let x = pin_x + idx as u16;
            if x < right_edge - 1
                && let Some(cell) = buf.cell_mut((x, prompt_rect.y))
            {
                let mut sym_buf = [0u8; 4];
                cell.set_symbol(ch.encode_utf8(&mut sym_buf));
                cell.set_style(
                    Style::default()
                        .fg(Color::Rgb(85, 255, 255))
                        .bg(surface_bg)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }
    }

    // Render bottom-right resize corner handle when custom width is active
    if pane.prompt().width_cols.is_some() && prompt_rect.width >= 16 && prompt_rect.height >= 3 {
        let rx = prompt_rect.x + prompt_rect.width - 1;
        let ry = prompt_rect.y + prompt_rect.height - 1;
        if let Some(cell) = buf.cell_mut((rx, ry)) {
            cell.set_symbol(glyphs.resize_corner);
            cell.set_style(
                Style::default()
                    .fg(palette.title_fg)
                    .bg(surface_bg)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if pane.prompt().fuzzy_search.active {
        render_fuzzy_search_contents(pane, inner, surface_bg, buf);
        return;
    }

    // Determine lines to render (handling legacy direct mutation of `pane.prompt.input` in tests)
    let fallback_lines;
    let lines_slice: &[String] = if pane.prompt().lines.len() == 1
        && pane.prompt().lines[0].is_empty()
        && !pane.prompt().input.is_empty()
    {
        fallback_lines = vec![pane.prompt().input.clone()];
        &fallback_lines
    } else {
        &pane.prompt().lines
    };

    let has_running_banner = pane.prompt().running_command.is_some() && inner.height >= 2;
    let has_completion_bar = pane.prompt().completion.is_active()
        && inner.height >= (2 + u16::from(has_running_banner));
    let reserved_rows = usize::from(has_running_banner) + usize::from(has_completion_bar);
    let editor_rows = usize::from(inner.height).saturating_sub(reserved_rows).max(1);

    let auto_start_row = pane
        .prompt()
        .cursor_row
        .saturating_sub(editor_rows.saturating_sub(1));
    let max_start_row = lines_slice.len().saturating_sub(editor_rows);
    let start_row = if pane.prompt().scroll_offset > 0 {
        pane.prompt().scroll_offset.min(max_start_row)
    } else {
        auto_start_row
    };

    // Horizontal scrolling when a single line exceeds inner.width
    let prefix_len = 4usize;
    let avail_text_cols = usize::from(inner.width).saturating_sub(prefix_len).max(1);
    let h_scroll = if pane.prompt().cursor_col >= avail_text_cols {
        pane.prompt().cursor_col + 1 - avail_text_cols
    } else {
        pane.prompt().h_scroll_offset
    };

    let mut rendered_lines = Vec::with_capacity(usize::from(inner.height));
    if let Some(run_cmd) = &pane.prompt().running_command
        && has_running_banner
    {
        let status_tag = if pane.has_foreground_subcommand() {
            format!("{} RUNNING: ", glyphs.status_run_icon)
        } else {
            format!("{} LAST CMD: ", glyphs.status_ready_icon)
        };
        let mut banner_spans = vec![
            Span::styled(
                status_tag,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(255, 255, 85))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {run_cmd} "),
                Style::default()
                    .fg(Color::Rgb(85, 255, 255))
                    .bg(surface_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if let Some(exit_code) = pane.vt().last_exit_code() {
            let dur_str = pane
                .vt()
                .last_duration_ms()
                .map(|ms| format!(" [{ms}ms]"))
                .unwrap_or_default();
            let (badge_txt, badge_fg) = if exit_code == 0 {
                (format!(" ✓ 0{dur_str} "), Color::Rgb(0, 255, 102))
            } else {
                (format!(" ✗ {exit_code}{dur_str} "), Color::Rgb(255, 85, 85))
            };
            banner_spans.push(Span::styled(
                badge_txt,
                Style::default()
                    .fg(badge_fg)
                    .bg(surface_bg)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        rendered_lines.push(Line::from(banner_spans));
    }

    for (rel_idx, line_str) in lines_slice
        .iter()
        .enumerate()
        .skip(start_row)
        .take(editor_rows)
    {
        let prefix = if rel_idx == 0 {
            glyphs.prompt_prefix
        } else {
            glyphs.prompt_continuation
        };
        let visible_sub: String = if h_scroll > 0 && rel_idx == pane.prompt().cursor_row {
            line_str.chars().skip(h_scroll).collect()
        } else {
            line_str.clone()
        };
        let mut spans = Vec::new();
        spans.push(Span::styled(
            prefix,
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD),
        ));
        for tok in tokenize_shell_line(&visible_sub) {
            spans.push(Span::styled(tok.text, tok.kind.style().bg(surface_bg)));
        }
        rendered_lines.push(Line::from(spans));
    }

    if has_completion_bar {
        rendered_lines.push(build_completion_bar_line(pane, inner.width, surface_bg));
    }

    Paragraph::new(rendered_lines).render(inner, buf);

    // Draw vertical scrollbar indicator on right border when lines_slice.len() > editor_rows
    if lines_slice.len() > editor_rows && inner.height >= 1 {
        let sb_x = prompt_rect.x + prompt_rect.width.saturating_sub(1);
        let thumb_rel = (start_row * usize::from(inner.height)) / lines_slice.len().max(1);
        for r in 0..inner.height {
            let y = inner.y + r;
            let sym = if usize::from(r) == thumb_rel { "█" } else { "░" };
            if let Some(cell) = buf.cell_mut((sb_x, y)) {
                cell.set_symbol(sym);
                cell.set_style(Style::default().fg(palette.title_fg).bg(surface_bg));
            }
        }
    }

    // Software cursor + optional IBM 3270 cross-hair cursor ruler
    let banner_offset = u16::from(has_running_banner);
    let cursor_vis_row = pane.prompt().cursor_row.saturating_sub(start_row) as u16;
    let vis_col = pane.prompt().cursor_col.saturating_sub(h_scroll);
    let cursor_col = (prefix_len + vis_col) as u16;
    let max_editor_row = (editor_rows as u16).saturating_sub(1);
    let clamped_row = banner_offset + cursor_vis_row.min(max_editor_row);
    let clamped_col = cursor_col.min(inner.width.saturating_sub(1));
    let cx = inner.x + clamped_col;
    let cy = inner.y + clamped_row;

    if active && cfg.focal_zoom_mode != SpotlightMode::Off && cfg.focal_zoom_pct > 100 {
        let target_rect = if cfg.focal_zoom_fullscreen {
            buf.area
        } else {
            canvas_rect
        };
        render_focal_zoom_cursor(
            buf,
            target_rect,
            cx,
            cy,
            cfg.focal_zoom_mode,
            cfg.focal_zoom_pct,
            cfg.focal_zoom_rx,
            cfg.focal_zoom_ry,
            cfg.focal_zoom_feather_pct,
            cfg.focal_zoom_pixelated,
            palette.crosshair_bg,
        );
    }
    if active && cfg.spotlight_mode != SpotlightMode::Off {
        let target_rect = if cfg.spotlight_fullscreen {
            buf.area
        } else {
            canvas_rect
        };
        render_spotlight_cursor(
            buf,
            target_rect,
            cx,
            cy,
            cfg.spotlight_mode,
            cfg.spotlight_rx,
            cfg.spotlight_ry,
            cfg.spotlight_feather_pct,
            cfg.spotlight_pixelated,
            palette.crosshair_bg,
        );
    }
    if active && cfg.crosshair_cursor {
        let target_rect = if cfg.crosshair_fullscreen {
            buf.area
        } else {
            canvas_rect
        };
        render_crosshair_cursor(buf, target_rect, cx, cy, palette.crosshair_bg);
    }
    render_caret(buf, cx, cy, cfg.cursor_style, palette.active_border_fg);
}

fn build_completion_bar_line<'a>(
    pane: &'a CompositePane,
    max_width: u16,
    surface_bg: Color,
) -> Line<'a> {
    let mut spans = vec![Span::styled(
        "TAB: ",
        Style::default()
            .fg(Color::Rgb(85, 255, 255))
            .bg(surface_bg)
            .add_modifier(Modifier::BOLD),
    )];

    let selected = pane.prompt().completion.selected;
    let total = pane.prompt().completion.candidates.len();
    let start_idx = selected.unwrap_or(0).saturating_sub(2);
    let mut used_cols = 5usize;

    for (idx, cand) in pane
        .prompt()
        .completion
        .candidates
        .iter()
        .enumerate()
        .skip(start_idx)
    {
        let label = format!(" {} ", cand.display);
        let label_w = label.chars().count();
        if used_cols + label_w + 1 > usize::from(max_width) && idx > start_idx {
            break;
        }
        let style = if selected == Some(idx) {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 255, 85))
                .add_modifier(Modifier::BOLD)
        } else if cand.is_dir {
            Style::default()
                .fg(Color::Rgb(85, 255, 255))
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).bg(surface_bg)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::styled(" ", Style::default().bg(surface_bg)));
        used_cols += label_w + 1;
        if idx + 1 >= total {
            break;
        }
    }

    Line::from(spans)
}

fn render_fuzzy_search_contents(
    pane: &CompositePane,
    inner: Rect,
    surface_bg: Color,
    buf: &mut Buffer,
) {
    let mut lines = Vec::new();
    let header = format!("(reverse-i-search)`{}': ", pane.prompt().fuzzy_search.query);
    lines.push(Line::from(vec![Span::styled(
        header,
        Style::default()
            .fg(Color::Rgb(255, 255, 85))
            .bg(surface_bg)
            .add_modifier(Modifier::BOLD),
    )]));

    let max_matches = usize::from(inner.height.saturating_sub(1));
    for (idx, item) in pane
        .prompt()
        .fuzzy_search
        .matches
        .iter()
        .take(max_matches)
        .enumerate()
    {
        let is_selected = idx == pane.prompt().fuzzy_search.selected;
        let row_bg = if is_selected {
            Color::Rgb(0, 80, 120)
        } else {
            surface_bg
        };
        let marker = if is_selected { "> " } else { "  " };
        let mut spans = vec![Span::styled(
            marker,
            Style::default()
                .fg(Color::Rgb(0, 255, 102))
                .bg(row_bg)
                .add_modifier(Modifier::BOLD),
        )];

        for (char_idx, ch) in item.command.chars().enumerate() {
            let matched = item.matched_indices.contains(&(char_idx as u32));
            let style = if matched {
                Style::default()
                    .fg(Color::Rgb(255, 255, 85))
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::White).bg(row_bg)
            };
            spans.push(Span::styled(ch.to_string(), style));
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines).render(inner, buf);
}

pub fn render_oia_status_bar(pane: &CompositePane, status_rect: Rect, buf: &mut Buffer) {
    render_oia_status_bar_themed(
        pane,
        status_rect,
        &ThemePalette::default(),
        &GlyphSet::default(),
        buf,
    );
}

pub fn render_oia_status_bar_themed(
    pane: &CompositePane,
    status_rect: Rect,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if status_rect.width == 0 || status_rect.height == 0 {
        return;
    }

    let oia_bg = palette.oia_bg;
    let sep = glyphs.oia_separator;
    let system_state = if pane.is_password_prompt() {
        (format!(" {} !ECHO LOCK ", glyphs.status_lock_icon), Color::Red)
    } else if pane.vt().is_alternate_screen() {
        (format!(" {} ALT SCR ", glyphs.status_alt_icon), Color::Yellow)
    } else if pane.vt().scrollback_offset() > 0 {
        (format!(" {} SCROLL ", glyphs.status_scroll_icon), Color::Cyan)
    } else if pane.has_foreground_subcommand() {
        (
            format!(" {} RUNNING ", glyphs.status_run_icon),
            Color::Rgb(85, 255, 255),
        )
    } else {
        (
            format!(" {} READY ", glyphs.status_ready_icon),
            Color::Rgb(0, 255, 102),
        )
    };

    let routing_badge = match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => (" INPUT: CMD BOX ", Color::Rgb(0, 255, 102)),
        InputRoutingMode::RawPtyPassthrough => (" INPUT: RAW PTY ", Color::Rgb(255, 200, 0)),
    };

    let anchor_label = match pane.prompt().anchor {
        PromptAnchor::Top => "PIN:TOP",
        PromptAnchor::Bottom => "PIN:BOT",
        PromptAnchor::SplitTop => "PIN:SPL-T",
        PromptAnchor::SplitBottom => "PIN:SPL-B",
        PromptAnchor::Floating { y_offset } => {
            if y_offset == 0 {
                "PIN:TOP"
            } else {
                "PIN:FLT"
            }
        }
    };

    // Row/Col counter reflects active input focus: editor cursor in CommandPrompt mode, VT100 cursor in RawPtyPassthrough
    let (disp_r, disp_c) = match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => {
            (pane.prompt().cursor_row + 1, pane.prompt().cursor_col + 1)
        }
        InputRoutingMode::RawPtyPassthrough => {
            let (vr, vc) = pane.vt().screen().cursor_position();
            (usize::from(vr) + 1, usize::from(vc) + 1)
        }
    };
    let coord_str = format!(
        " {} R:{:02} C:{:03} ",
        glyphs.cursor_pos_icon, disp_r, disp_c
    );
    let out_lines = pane.vt().output_line_count();
    let out_bytes = pane.vt().total_bytes_processed();
    let out_bytes_label = if out_bytes >= 1_048_576 {
        format!("{:.1}M", out_bytes as f64 / 1_048_576.0)
    } else if out_bytes >= 1024 {
        format!("{:.1}K", out_bytes as f64 / 1024.0)
    } else {
        format!("{out_bytes}B")
    };
    let out_str = format!(
        " {} OUT:{out_lines}L/{out_bytes_label}",
        glyphs.output_len_icon
    );

    let mut spans = vec![
        Span::styled(
            "3270-OIA",
            Style::default()
                .fg(palette.oia_badge_fg)
                .bg(palette.oia_badge_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            system_state.0,
            Style::default()
                .fg(system_state.1)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            routing_badge.0,
            Style::default()
                .fg(routing_badge.1)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{sep} "),
            Style::default().fg(Color::White).bg(oia_bg),
        ),
        Span::styled(
            anchor_label,
            Style::default().fg(Color::White).bg(oia_bg),
        ),
    ];

    if pane.is_read_only() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " 🔒 PROT ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(remote) = pane.remote_target() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" 🌐 SSH:{remote} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(85, 255, 255))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if pane.vt().is_recording() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " ⏺ REC ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(200, 0, 0))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(notif) = pane.vt().notifications().last() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" 🔔 {}: {} ", notif.title, notif.body),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 255, 85))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(progress) = pane.vt().osc9_progress() {
        let (gauge_str, gauge_fg) = format_osc9_gauge(progress);
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" {gauge_str} "),
            Style::default()
                .fg(gauge_fg)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(block) = pane.vt().selected_command_block() {
        let total = pane.vt().command_blocks().len();
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!("BLK {}/{total}", block.index + 1),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 255, 85))
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::styled(
        format!(" {sep}"),
        Style::default().fg(Color::White).bg(oia_bg),
    ));
    spans.push(Span::styled(
        coord_str,
        Style::default().fg(Color::Rgb(255, 255, 85)).bg(oia_bg),
    ));
    spans.push(Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)));
    spans.push(Span::styled(
        out_str,
        Style::default().fg(Color::Rgb(85, 255, 255)).bg(oia_bg),
    ));

    let line = Line::from(spans);

    Block::default()
        .style(Style::default().fg(Color::White).bg(oia_bg))
        .render(status_rect, buf);
    Paragraph::new(line).render(status_rect, buf);
}

fn format_osc9_gauge(progress: Osc9Progress) -> (String, Color) {
    let build_bar = |pct: u8| -> String {
        let filled = ((u16::from(pct.min(100)) * 8) / 100) as usize;
        let empty = 8usize.saturating_sub(filled);
        format!(
            "[{}{}] {:>3}%",
            "█".repeat(filled),
            "░".repeat(empty),
            pct.min(100)
        )
    };
    match progress {
        Osc9Progress::Normal(pct) => (build_bar(pct), Color::Rgb(0, 255, 102)),
        Osc9Progress::Error(pct) => (format!("!{}", build_bar(pct)), Color::Rgb(255, 85, 85)),
        Osc9Progress::Paused(pct) => (format!("⏸{}", build_bar(pct)), Color::Rgb(255, 255, 85)),
        Osc9Progress::Indeterminate => ("[▓▓░░░░▓▓] BUSY".to_string(), Color::Rgb(85, 255, 255)),
    }
}
