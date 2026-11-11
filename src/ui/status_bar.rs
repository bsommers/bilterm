//! DOS function-key bar, IBM 3270 Operator Information Area (OIA) status bar, and floating command prompt renderers.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::highlight::tokenize_shell_line;
use crate::pane::{CompositePane, PromptAnchor};
pub use crate::ui::oia::{render_oia_status_bar, render_oia_status_bar_themed};
use crate::ui::chrome::OutlineStyle;
use crate::ui::cursor::{
    SpotlightMode, render_crosshair_cursor, render_focal_zoom_cursor, render_spotlight_cursor,
};
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens};
use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;

fn role(cfg: &BiltermConfig, palette: &ThemePalette, role: ColorRole, legacy: Color) -> Color {
    if AppearanceProfile::resolve(cfg) == AppearanceProfile::Legacy {
        legacy
    } else {
        DesignTokens::from_palette(palette).get(role)
    }
}

/// Renders the classic Borland Turbo Vision / DOS IDE bottom Function-Key status bar (`UI-03`),
/// or a clean modern shortcut strip when Modern / Liquid Glass palette is active.
pub fn render_dos_fkey_bar(rect: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let is_modern = palette.is_modern_look();
    let items = if is_modern {
        [
            ("F1", "Help  ·"),
            ("F2", "Split  ·"),
            ("F5", "Zoom  ·"),
            ("F7", "Drawer  ·"),
            ("F11", "Style  ·"),
            ("F12", "Raw "),
        ]
    } else {
        [
            ("F1", "Help "),
            ("F2", "Split "),
            ("F5", "Zoom "),
            ("F6", "Next "),
            ("F10", "Quake "),
            ("F12", "Raw "),
        ]
    };

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

    let badge = if is_modern {
        format!(" · {}", palette.name)
    } else {
        format!("[{}]", palette.name)
    };
    spans.push(Span::styled(
        badge,
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
    let effective_outline = if cfg.is_modern() && cfg.prompt_outline == OutlineStyle::Double {
        OutlineStyle::Rounded
    } else if !active && cfg.prompt_outline == OutlineStyle::Double {
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

    let base_text_fg = palette.text_fg.unwrap_or(Color::White);
    let pin_fg = palette
        .controls_fg
        .unwrap_or_else(|| role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)));
    let corner_fg = palette.controls_fg.unwrap_or(palette.title_fg);
    let title = if cfg.is_modern() {
        let liq = if cfg.liquid_glass { "💧 " } else { "" };
        if cfg.snazzy_mode {
            let cmd_lbl = pane
                .resolve_running_command()
                .unwrap_or_else(|| pane.shell_program().to_string());
            let icon = crate::ui::visual_style::navigator_process_icon(&cmd_lbl);
            format!(" {liq}{icon} ❯ Command · {anchor_badge} ")
        } else {
            format!(" {liq}❯ Command · {anchor_badge} ")
        }
    } else if cfg.snazzy_mode {
        let cmd_lbl = pane
            .resolve_running_command()
            .unwrap_or_else(|| pane.shell_program().to_string());
        let icon = crate::ui::visual_style::navigator_process_icon(&cmd_lbl);
        format!("╣ {icon} BILTERM 3270 // CMD [{anchor_badge}] ╠")
    } else {
        format!("╣ BILTERM 3270 // CMD [{anchor_badge}] ╠")
    };
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
        .style(Style::default().fg(base_text_fg).bg(surface_bg));

    let inner = block.inner(prompt_rect);
    block.render(prompt_rect, buf);
    if cfg.is_glass() {
        crate::ui::visual_style::polish_glass_panel_rect(buf, prompt_rect, surface_bg, palette, active);
    }

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
                        .fg(pin_fg)
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
                    .fg(corner_fg)
                    .bg(surface_bg)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if pane.prompt().fuzzy_search.active {
        render_fuzzy_search_contents(pane, inner, surface_bg, cfg, palette, buf);
        return;
    }

    let lines_slice: &[String] = &pane.prompt().lines;

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
            if cfg.is_modern() {
                format!(" {} Running ", glyphs.status_run_icon)
            } else {
                format!("{} RUNNING: ", glyphs.status_run_icon)
            }
        } else if cfg.is_modern() {
            format!(" {} Last ", glyphs.status_ready_icon)
        } else {
            format!("{} LAST CMD: ", glyphs.status_ready_icon)
        };
        let (tag_fg, tag_bg, cmd_fg) = if cfg.is_modern() {
            (
                role(cfg, palette, ColorRole::Accent, Color::Rgb(125, 211, 252)),
                role(cfg, palette, ColorRole::SurfaceRaised, Color::Rgb(24, 46, 78)),
                role(cfg, palette, ColorRole::TextPrimary, Color::Rgb(226, 232, 240)),
            )
        } else {
            (
                Color::Black,
                role(cfg, palette, ColorRole::Warning, Color::Rgb(255, 255, 85)),
                role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)),
            )
        };
        let mut banner_spans = vec![
            Span::styled(
                status_tag,
                Style::default()
                    .fg(tag_fg)
                    .bg(tag_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {run_cmd} "),
                Style::default()
                    .fg(cmd_fg)
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
                (
                    format!(" ✓ 0{dur_str} "),
                    role(cfg, palette, ColorRole::Success, Color::Rgb(0, 255, 102)),
                )
            } else {
                (
                    format!(" ✗ {exit_code}{dur_str} "),
                    role(cfg, palette, ColorRole::Danger, Color::Rgb(255, 85, 85)),
                )
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
            let mut tstyle = tok.kind.style().bg(surface_bg);
            if let Some(tfg) = palette.text_fg
                && tstyle.fg == Some(Color::White)
            {
                tstyle = tstyle.fg(tfg);
            }
            spans.push(Span::styled(tok.text, tstyle));
        }
        rendered_lines.push(Line::from(spans));
    }

    if has_completion_bar {
        rendered_lines.push(build_completion_bar_line(
            pane,
            inner.width,
            surface_bg,
            cfg,
            palette,
        ));
    }

    Paragraph::new(rendered_lines).render(inner, buf);

    // Highlight active visual selection inside the command prompt editor
    if pane.prompt().has_selection() {
        let banner_off = u16::from(has_running_banner);
        let sel_style = Style::default()
            .fg(Color::Black)
            .bg(role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)))
            .add_modifier(Modifier::BOLD);
        for (vis_r, (rel_idx, line_str)) in lines_slice
            .iter()
            .enumerate()
            .skip(start_row)
            .take(editor_rows)
            .enumerate()
        {
            let dst_y = inner.y + banner_off + vis_r as u16;
            if dst_y >= inner.y + inner.height {
                break;
            }
            let row_h_scroll = if h_scroll > 0 && rel_idx == pane.prompt().cursor_row {
                h_scroll
            } else {
                0
            };
            let line_chars = line_str.chars().count();
            for vis_c in 0..avail_text_cols {
                let char_col = row_h_scroll + vis_c;
                if char_col <= line_chars && pane.prompt().is_char_selected(rel_idx, char_col) {
                    let dst_x = inner.x + (prefix_len + vis_c) as u16;
                    if dst_x < inner.x + inner.width
                        && let Some(cell) = buf.cell_mut((dst_x, dst_y))
                    {
                        cell.set_style(sel_style);
                    }
                }
            }
        }
    }

    // Draw vertical scrollbar indicator on right border when lines_slice.len() > editor_rows
    if lines_slice.len() > editor_rows && inner.height >= 1 {
        let sb_x = prompt_rect.x + prompt_rect.width.saturating_sub(1);
        let thumb_rel = (start_row * usize::from(inner.height)) / lines_slice.len().max(1);
        for r in 0..inner.height {
            let y = inner.y + r;
            let (sym, fg_col) = if usize::from(r) == thumb_rel {
                if cfg.is_modern() {
                    ("┃", palette.active_border_fg)
                } else {
                    ("█", palette.title_fg)
                }
            } else if cfg.is_modern() {
                ("│", palette.inactive_border_fg)
            } else {
                ("░", palette.title_fg)
            };
            if let Some(cell) = buf.cell_mut((sb_x, y)) {
                cell.set_symbol(sym);
                cell.set_style(Style::default().fg(fg_col).bg(surface_bg));
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
    if active && !pane.copy_mode().active {
        crate::ui::cursor_cmd::render_caret_configured(buf, cx, cy, cfg, palette);
    }
}

fn build_completion_bar_line<'a>(
    pane: &'a CompositePane,
    max_width: u16,
    surface_bg: Color,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
) -> Line<'a> {
    let mut spans = vec![Span::styled(
        "TAB: ",
        Style::default()
            .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)))
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
                .bg(role(cfg, palette, ColorRole::Warning, Color::Rgb(255, 255, 85)))
                .add_modifier(Modifier::BOLD)
        } else if cand.is_dir {
            Style::default()
                .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)))
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
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    let mut lines = Vec::new();
    let header = format!("(reverse-i-search)`{}': ", pane.prompt().fuzzy_search.query);
    lines.push(Line::from(vec![Span::styled(
        header,
        Style::default()
            .fg(role(cfg, palette, ColorRole::Warning, Color::Rgb(255, 255, 85)))
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
            role(cfg, palette, ColorRole::SurfaceRaised, Color::Rgb(0, 80, 120))
        } else {
            surface_bg
        };
        let marker = if is_selected { "> " } else { "  " };
        let mut spans = vec![Span::styled(
            marker,
            Style::default()
                .fg(role(cfg, palette, ColorRole::Success, Color::Rgb(0, 255, 102)))
                .bg(row_bg)
                .add_modifier(Modifier::BOLD),
        )];

        for (char_idx, ch) in item.command.chars().enumerate() {
            let matched = item.matched_indices.contains(&(char_idx as u32));
            let style = if matched {
                Style::default()
                    .fg(role(cfg, palette, ColorRole::Warning, Color::Rgb(255, 255, 85)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_profile_keeps_legacy_literal_byte_identical() {
        let cfg = BiltermConfig::default();
        assert_eq!(AppearanceProfile::resolve(&cfg), AppearanceProfile::Legacy);

        let pal = ThemePalette::default();
        let lit = Color::Rgb(12, 34, 56);
        assert_eq!(role(&cfg, &pal, ColorRole::Accent, lit), lit);
        assert_eq!(role(&cfg, &pal, ColorRole::SurfaceRaised, lit), lit);
        assert_eq!(role(&cfg, &pal, ColorRole::TextPrimary, lit), lit);
        assert_eq!(role(&cfg, &pal, ColorRole::Warning, lit), lit);
        assert_eq!(role(&cfg, &pal, ColorRole::Success, lit), lit);
        assert_eq!(role(&cfg, &pal, ColorRole::Danger, lit), lit);
    }

    #[test]
    fn non_legacy_profile_routes_to_design_tokens() {
        let mut cfg = BiltermConfig::default();
        cfg.appearance.profile = Some(AppearanceProfile::Obsidian);

        let pal = ThemePalette::default();
        let tokens = DesignTokens::from_palette(&pal);
        let lit = Color::Rgb(12, 34, 56);
        assert_eq!(role(&cfg, &pal, ColorRole::Accent, lit), tokens.accent);
        assert_eq!(role(&cfg, &pal, ColorRole::SurfaceRaised, lit), tokens.surface_raised);
        assert_eq!(role(&cfg, &pal, ColorRole::TextPrimary, lit), tokens.text_primary);
        assert_eq!(role(&cfg, &pal, ColorRole::Warning, lit), tokens.warning);
        assert_eq!(role(&cfg, &pal, ColorRole::Success, lit), tokens.success);
        assert_eq!(role(&cfg, &pal, ColorRole::Danger, lit), tokens.danger);
    }
}


