pub mod chrome;
pub mod cursor;
pub mod glyphs;
pub mod occlusion;
pub mod status_bar;
pub mod theme;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::highlight::tokenize_shell_line;
use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::chrome::{
    render_borland_frame_styled, render_drop_shadow_styled, OutlineStyle,
};
use crate::ui::cursor::{render_caret, render_crosshair_cursor};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::status_bar::render_dos_fkey_bar;
use crate::ui::theme::ThemePalette;
use crate::vt::cell_to_ratatui_style;
use crate::wm::{inset_window_frame, WindowManager, WorkspaceLayoutMode};

/// Ratatui widget rendering a `CompositePane` in 3 immediate-mode passes:
/// 1. **Pass 1 (`OutputCanvas`)**: Renders the full-height VT100/ANSI/TrueColor screen (`area.height - 1`)
/// 2. **Pass 2 (`FloatingCommandPrompt`)**: If visible, clears `prompt_rect` (`occlude_rect`), draws optional Borland drop shadow, and renders the floating command prompt + optional IBM 3270 cross-hair cursor on top
/// 3. **Pass 3 (`OiaStatusBar`)**: Renders the bottom 1-row Operator Information Area (OIA) status bar
pub struct CompositePaneWidget<'a> {
    pub pane: &'a CompositePane,
    pub active: bool,
    pub config: Option<&'a BiltermConfig>,
}

impl<'a> CompositePaneWidget<'a> {
    pub fn new(pane: &'a CompositePane, active: bool) -> Self {
        Self {
            pane,
            active,
            config: None,
        }
    }

    pub fn with_config(pane: &'a CompositePane, active: bool, config: &'a BiltermConfig) -> Self {
        Self {
            pane,
            active,
            config: Some(config),
        }
    }
}

impl Widget for CompositePaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let default_cfg = BiltermConfig::default();
        let cfg = self.config.unwrap_or(&default_cfg);
        let palette = cfg.palette();
        let glyphs = cfg.glyph_set();

        let canvas = self.pane.canvas_rect(area);
        let status_rect = self.pane.status_bar_rect(area);

        // Pass 1: Render full-height OutputCanvas from VtParser screen
        render_output_canvas(self.pane, canvas, cfg, &palette, self.active, buf);

        // Pass 2: Render overlaid or split FloatingCommandPrompt if visible
        if self.pane.is_prompt_visible() && canvas.height >= 2 {
            let prompt_rect = self.pane.prompt_rect(area);
            if cfg.drop_shadows && !self.pane.prompt.anchor.is_split() {
                render_drop_shadow_styled(buf, prompt_rect, canvas, cfg.shadow_style, &palette);
            }
            occlude_rect(buf, prompt_rect);
            render_floating_prompt(
                self.pane,
                self.active,
                prompt_rect,
                canvas,
                cfg,
                &palette,
                &glyphs,
                buf,
            );
        }

        // Pass 3: Render bottom 1-row Operator Information Area (OIA) status bar
        render_oia_status_bar_themed(self.pane, status_rect, &palette, &glyphs, buf);

        // Pass 4: Render optional Borland-style Command History Pop-up Window if active on this pane
        if self.pane.is_prompt_visible() && self.pane.prompt.history_popup_open && canvas.height >= 5
        {
            render_history_popup(self.pane, canvas, cfg, &palette, &glyphs, buf);
        }

        // Pass 5: Render unsafe multi-line paste confirmation modal if pending (FND-01)
        if let Some(pending_paste) = &self.pane.pending_unsafe_paste
            && canvas.height >= 5
            && canvas.width >= 24
        {
            render_unsafe_paste_modal(pending_paste, canvas, cfg, &palette, buf);
        }
    }
}

fn is_safe_terminal_cluster(s: &str, expected_width: usize) -> bool {
    if s.is_empty() {
        return false;
    }
    if unicode_width::UnicodeWidthStr::width(s) != expected_width {
        return false;
    }
    for ch in s.chars() {
        let code = ch as u32;
        if ch.is_control()
            || matches!(
                code,
                0x200B..=0x200F
                    | 0x2028..=0x202F
                    | 0x2060..=0x206F
                    | 0xFE00..=0xFE0F
                    | 0xFEFF
                    | 0xFFF9..=0xFFFB
            )
        {
            return false;
        }
    }
    true
}

fn render_output_canvas(
    pane: &CompositePane,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    active: bool,
    buf: &mut Buffer,
) {
    let screen = pane.vt.screen();
    let (vt_rows, vt_cols) = pane.vt.size();
    let render_rows = canvas.height.min(vt_rows);
    let render_cols = canvas.width.min(vt_cols);

    for r in 0..render_rows {
        let mut skip_next_wide_continuation = false;
        for c in 0..render_cols {
            let Some(vt_cell) = screen.cell(r, c) else {
                skip_next_wide_continuation = false;
                continue;
            };
            let dst_x = canvas.x + c;
            let dst_y = canvas.y + r;
            let style = cell_to_ratatui_style(vt_cell);

            if skip_next_wide_continuation {
                skip_next_wide_continuation = false;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_style(style);
                }
                continue;
            }

            if vt_cell.is_wide_continuation() {
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
                continue;
            }

            let contents = vt_cell.contents();
            if vt_cell.is_wide() {
                // Prevent a 2-column wide glyph at the rightmost column from overflowing/wrapping the host terminal
                if c + 1 < render_cols && is_safe_terminal_cluster(contents, 2) {
                    if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                        dst_cell.set_symbol(contents);
                        dst_cell.set_style(style);
                    }
                    skip_next_wide_continuation = true;
                } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
            } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                if contents.is_empty() {
                    dst_cell.set_symbol(" ");
                } else if is_safe_terminal_cluster(contents, 1) {
                    dst_cell.set_symbol(contents);
                } else {
                    dst_cell.set_symbol("·");
                }
                dst_cell.set_style(style);
            }
        }
    }

    // Highlight detected URLs with underline + bright cyan tint (VAL-03)
    for url in pane.vt.detect_visible_urls() {
        if url.row < render_rows {
            for c in url.start_col..=url.end_col.min(render_cols.saturating_sub(1)) {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + url.row)) {
                    cell.set_style(
                        cell.style()
                            .fg(Color::Rgb(85, 255, 255))
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
            }
        }
    }

    // Highlight active OutputCanvas search matches (VAL-02)
    if pane.canvas_search.active && !pane.canvas_search.matches.is_empty() {
        for r in 0..render_rows {
            for c in 0..render_cols {
                let (is_match, is_sel) = pane.canvas_search.cell_match_status(r, c);
                if is_match
                    && let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r))
                {
                    let bg = if is_sel {
                        Color::Rgb(255, 176, 0)
                    } else {
                        Color::Rgb(255, 255, 85)
                    };
                    cell.set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }

    // Highlight Vi Copy Mode selection and cursor (VAL-04)
    if pane.copy_mode.active {
        for r in 0..render_rows {
            for c in 0..render_cols {
                if pane.copy_mode.is_cell_selected(r, c)
                    && let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r))
                {
                    cell.set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(85, 255, 255))
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
        let cr = pane.copy_mode.cursor_row.min(render_rows.saturating_sub(1));
        let cc = pane.copy_mode.cursor_col.min(render_cols.saturating_sub(1));
        render_caret(
            buf,
            canvas.x + cc,
            canvas.y + cr,
            cfg.cursor_style,
            Color::Rgb(255, 255, 85),
        );
    }

    // When in RawPtyPassthrough and not scrolled back, highlight the live PTY cursor cell
    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
        && pane.vt.scrollback_offset() == 0
        && !screen.hide_cursor()
    {
        let (cur_r, cur_c) = screen.cursor_position();
        if cur_r < canvas.height && cur_c < canvas.width {
            let cx = canvas.x + cur_c;
            let cy = canvas.y + cur_r;
            if active && cfg.crosshair_cursor {
                let target_rect = if cfg.crosshair_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_crosshair_cursor(buf, target_rect, cx, cy, palette.crosshair_bg);
            }
            render_caret(buf, cx, cy, cfg.cursor_style, palette.active_border_fg);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_floating_prompt(
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

    let anchor_badge = match pane.prompt.anchor {
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
            if x < right_edge - 1 {
                if let Some(cell) = buf.cell_mut((x, prompt_rect.y)) {
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
    }

    // Render bottom-right resize corner handle when custom width is active
    if pane.prompt.width_cols.is_some() && prompt_rect.width >= 16 && prompt_rect.height >= 3 {
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

    if pane.prompt.fuzzy_search.active {
        render_fuzzy_search_contents(pane, inner, surface_bg, buf);
        return;
    }

    // Determine lines to render (handling legacy direct mutation of `pane.prompt.input` in tests)
    let fallback_lines;
    let lines_slice: &[String] = if pane.prompt.lines.len() == 1
        && pane.prompt.lines[0].is_empty()
        && !pane.prompt.input.is_empty()
    {
        fallback_lines = vec![pane.prompt.input.clone()];
        &fallback_lines
    } else {
        &pane.prompt.lines
    };

    let has_running_banner = pane.prompt.running_command.is_some() && inner.height >= 2;
    let has_completion_bar = pane.prompt.completion.is_active()
        && inner.height >= (2 + u16::from(has_running_banner));
    let reserved_rows = usize::from(has_running_banner) + usize::from(has_completion_bar);
    let editor_rows = usize::from(inner.height).saturating_sub(reserved_rows).max(1);

    let auto_start_row = pane
        .prompt
        .cursor_row
        .saturating_sub(editor_rows.saturating_sub(1));
    let max_start_row = lines_slice.len().saturating_sub(editor_rows);
    let start_row = if pane.prompt.scroll_offset > 0 {
        pane.prompt.scroll_offset.min(max_start_row)
    } else {
        auto_start_row
    };

    // Horizontal scrolling when a single line exceeds inner.width
    let prefix_len = 4usize;
    let avail_text_cols = usize::from(inner.width).saturating_sub(prefix_len).max(1);
    let h_scroll = if pane.prompt.cursor_col >= avail_text_cols {
        pane.prompt.cursor_col + 1 - avail_text_cols
    } else {
        pane.prompt.h_scroll_offset
    };

    let mut rendered_lines = Vec::with_capacity(usize::from(inner.height));
    if let Some(run_cmd) = &pane.prompt.running_command {
        if has_running_banner {
            let status_tag = if pane.line_discipline.has_foreground_subcommand() {
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
            if let Some(exit_code) = pane.vt.last_exit_code() {
                let dur_str = pane
                    .vt
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
        let visible_sub: String = if h_scroll > 0 && rel_idx == pane.prompt.cursor_row {
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
    let cursor_vis_row = pane.prompt.cursor_row.saturating_sub(start_row) as u16;
    let vis_col = pane.prompt.cursor_col.saturating_sub(h_scroll);
    let cursor_col = (prefix_len + vis_col) as u16;
    let max_editor_row = (editor_rows as u16).saturating_sub(1);
    let clamped_row = banner_offset + cursor_vis_row.min(max_editor_row);
    let clamped_col = cursor_col.min(inner.width.saturating_sub(1));
    let cx = inner.x + clamped_col;
    let cy = inner.y + clamped_row;

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

    let selected = pane.prompt.completion.selected;
    let total = pane.prompt.completion.candidates.len();
    let start_idx = selected.unwrap_or(0).saturating_sub(2);
    let mut used_cols = 5usize;

    for (idx, cand) in pane
        .prompt
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
    let header = format!("(reverse-i-search)`{}': ", pane.prompt.fuzzy_search.query);
    lines.push(Line::from(vec![Span::styled(
        header,
        Style::default()
            .fg(Color::Rgb(255, 255, 85))
            .bg(surface_bg)
            .add_modifier(Modifier::BOLD),
    )]));

    let max_matches = usize::from(inner.height.saturating_sub(1));
    for (idx, item) in pane
        .prompt
        .fuzzy_search
        .matches
        .iter()
        .take(max_matches)
        .enumerate()
    {
        let is_selected = idx == pane.prompt.fuzzy_search.selected;
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

/// Multi-pane workspace compositor (`WM-01..05`, `UI-01..05`) rendering:
/// 1. Tiled H/V splits (`SplitTree`) or overlapping z-order stacked windows (`ZStack`) in ascending z-order
/// 2. Borland Turbo Vision window frames (`╔═[1] PANE #1 ═╗` active / `┌─[2] PANE #2 ─┐` inactive) and translucent drop shadows
/// 3. Top-anchored persistent Guake/Quake-style drop-down `QuickTerminal` overlay (`[Q] QUAKE QUICK TERM [~]`)
/// 4. Optional bottom 1-row Borland DOS Function-Key bar (`F1 Help  F2 SplitH ...`)
pub struct WorkspaceWidget<'a> {
    pub wm: &'a WindowManager,
}

impl<'a> WorkspaceWidget<'a> {
    pub fn new(wm: &'a WindowManager) -> Self {
        Self { wm }
    }
}

impl Widget for WorkspaceWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let cfg = &self.wm.config;
        let palette = cfg.palette();
        let glyphs = cfg.glyph_set();

        let workspace_area = if cfg.show_dos_fkey_bar && area.height > 2 {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height - 1,
            }
        } else {
            area
        };

        let rects = self.wm.compute_workspace_rects(workspace_area);
        let framed =
            self.wm.panes.len() > 1 || self.wm.layout_mode == WorkspaceLayoutMode::Stacked;
        let top_workspace_id = self.wm.z_stack.top();
        let qt_visible = self.wm.quick_terminal.visible;

        for (id, outer) in rects {
            let Some(pane) = self.wm.panes.get(&id) else {
                continue;
            };
            let is_active = !qt_visible && top_workspace_id == Some(id);
            if framed {
                if cfg.drop_shadows && self.wm.layout_mode == WorkspaceLayoutMode::Stacked {
                    render_drop_shadow_styled(
                        buf,
                        outer,
                        workspace_area,
                        cfg.shadow_style,
                        &palette,
                    );
                }
                occlude_rect(buf, outer);
                let z_badge = self
                    .wm
                    .z_stack
                    .rank_of(id)
                    .map(|(r, total)| format!("Z:{r}/{total}"))
                    .unwrap_or_default();
                render_borland_frame_styled(
                    outer,
                    &format!("[{id}] {} ({z_badge})", pane.title),
                    is_active,
                    cfg.window_outline,
                    &palette,
                    &glyphs,
                    buf,
                );
                let inner = inset_window_frame(outer);
                CompositePaneWidget::with_config(pane, is_active, cfg).render(inner, buf);
            } else {
                CompositePaneWidget::with_config(pane, is_active, cfg).render(outer, buf);
            }
        }

        // Render persistent Guake/Quake-style drop-down quick terminal overlay on top (WM-05)
        if qt_visible {
            let qt_outer = self.wm.quick_terminal.overlay_rect(workspace_area);
            if cfg.drop_shadows {
                render_drop_shadow_styled(
                    buf,
                    qt_outer,
                    workspace_area,
                    cfg.shadow_style,
                    &palette,
                );
            }
            occlude_rect(buf, qt_outer);
            render_borland_frame_styled(
                qt_outer,
                "[Q] QUAKE QUICK TERM [~] (Alt+` to hide)",
                true,
                cfg.window_outline,
                &palette,
                &glyphs,
                buf,
            );
            let qt_inner = inset_window_frame(qt_outer);
            CompositePaneWidget::with_config(&self.wm.quick_terminal.pane, true, cfg)
                .render(qt_inner, buf);
        }

        if cfg.show_dos_fkey_bar && area.height > 2 {
            let fkey_rect = Rect {
                x: area.x,
                y: area.y + area.height - 1,
                width: area.width,
                height: 1,
            };
            render_dos_fkey_bar(fkey_rect, &palette, buf);
        }

        // Render interactive <F1> Borland Turbo Vision Help Modal on top of the workspace
        if self.wm.help_visible && workspace_area.width >= 20 && workspace_area.height >= 6 {
            render_help_modal(
                self.wm.help_scroll,
                workspace_area,
                cfg,
                &palette,
                &glyphs,
                buf,
            );
        }
    }
}

fn render_history_popup(
    pane: &CompositePane,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let pop_w = canvas.width.saturating_sub(6).clamp(24, 72).min(canvas.width);
    let pop_h = canvas.height.saturating_sub(2).clamp(5, 14).min(canvas.height);
    let pop_x = canvas.x + (canvas.width.saturating_sub(pop_w)) / 2;
    let pop_y = canvas.y + (canvas.height.saturating_sub(pop_h)) / 2;
    let pop_rect = Rect::new(pop_x, pop_y, pop_w, pop_h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, pop_rect, canvas, cfg.shadow_style, palette);
    }
    occlude_rect(buf, pop_rect);
    let persist_tag = if pane.history_store.enabled {
        "PERSIST:ON"
    } else {
        "PERSIST:OFF"
    };
    render_borland_frame_styled(
        pop_rect,
        &format!(
            "[H] COMMAND HISTORY ({}) [Enter=Select  s=Save  Esc=Close]",
            persist_tag
        ),
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(pop_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    let header = format!(
        "Session: {} | Pane #{} | Entries: {}",
        pane.history_store.session_id,
        pane.id,
        pane.prompt.history.len()
    );
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(Color::Rgb(85, 255, 255))
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    )));

    let list_rows = usize::from(inner.height.saturating_sub(1)).max(1);
    if pane.prompt.history.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No commands recorded in this pane yet)",
            Style::default()
                .fg(Color::Yellow)
                .bg(palette.window_frame_bg),
        )));
    } else {
        let sel = pane
            .prompt
            .history_popup_selected
            .min(pane.prompt.history.len() - 1);
        let start = sel.saturating_sub(list_rows.saturating_sub(1));
        for (idx, cmd) in pane
            .prompt
            .history
            .iter()
            .enumerate()
            .skip(start)
            .take(list_rows)
        {
            let is_sel = idx == sel;
            let row_bg = if is_sel {
                palette.active_border_fg
            } else {
                palette.window_frame_bg
            };
            let row_fg = if is_sel { Color::Black } else { Color::White };
            let prefix = if is_sel { "►" } else { " " };
            lines.push(Line::from(Span::styled(
                format!("{prefix} [{:03}] {}", idx + 1, cmd.replace('\n', " ; ")),
                Style::default()
                    .fg(row_fg)
                    .bg(row_bg)
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )));
        }
    }

    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

fn render_unsafe_paste_modal(
    pending_paste: &str,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    let glyphs = cfg.glyph_set();
    let w = canvas.width.saturating_sub(6).clamp(24, 68).min(canvas.width);
    let h = canvas.height.saturating_sub(2).clamp(5, 9).min(canvas.height);
    let x = canvas.x + (canvas.width.saturating_sub(w)) / 2;
    let y = canvas.y + (canvas.height.saturating_sub(h)) / 2;
    let modal = Rect::new(x, y, w, h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, modal, canvas, cfg.shadow_style, palette);
    }
    occlude_rect(buf, modal);
    render_borland_frame_styled(
        modal,
        "[!] UNSAFE PASTE GUARD [y/Enter=Confirm  n/Esc=Cancel]",
        true,
        cfg.window_outline,
        palette,
        &glyphs,
        buf,
    );
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let line_count = pending_paste.lines().count().max(1);
    let preview: String = pending_paste
        .replace(['\r', '\n'], " ↵ ")
        .chars()
        .take(usize::from(inner.width.saturating_sub(2)))
        .collect();
    let lines = vec![
        Line::from(Span::styled(
            format!(" Multi-line/raw paste ({line_count} lines) into active PTY:"),
            Style::default()
                .fg(Color::Rgb(255, 255, 85))
                .bg(palette.window_frame_bg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {preview}"),
            Style::default()
                .fg(Color::Rgb(85, 255, 255))
                .bg(palette.window_frame_bg),
        )),
        Line::from(Span::styled(
            " Press [y/Enter] to paste or [n/Esc] to abort.",
            Style::default()
                .fg(Color::White)
                .bg(palette.window_frame_bg),
        )),
    ];
    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

fn render_help_modal(
    scroll: usize,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let w = workspace_area
        .width
        .saturating_sub(4)
        .clamp(20, 78)
        .min(workspace_area.width);
    let h = workspace_area
        .height
        .saturating_sub(2)
        .clamp(6, 22)
        .min(workspace_area.height);
    let x = workspace_area.x + (workspace_area.width.saturating_sub(w)) / 2;
    let y = workspace_area.y + (workspace_area.height.saturating_sub(h)) / 2;
    let modal = Rect::new(x, y, w, h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, modal, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, modal);
    render_borland_frame_styled(
        modal,
        "[F1] BILTERM 3270 // TURBO VISION HELP (Up/Down/PgUp/PgDn, Esc/F1=Close)",
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    const HELP_LINES: &[(&str, &str)] = &[
        ("═══ FUNCTION KEYS & GLOBAL HOTKEYS ═══", ""),
        ("F1 / :help", "Toggle this interactive Help reference window"),
        ("F2 / Alt+H", "Split focused composite pane horizontally"),
        ("Alt+V / :vsplit", "Split focused composite pane vertically"),
        ("F5 / Alt+Z / :zoom", "Toggle maximize/zoom on focused pane"),
        ("F6 / Alt+Tab", "Shuffle pane deck top-to-bottom (cycle focus)"),
        ("Alt+Shift+Tab", "Shuffle pane deck bottom-to-top"),
        ("Alt+1 .. Alt+9", "Selectively raise & focus pane #1..#9"),
        ("Alt+M / :layout", "Toggle Tiled Splits vs Z-Order Stacked Windows"),
        ("F10 / Alt+` / Ctrl+Space", "Toggle persistent Guake Quick Terminal"),
        ("F12 / Ctrl+G / :raw", "Toggle Command Box vs Raw PTY Passthrough"),
        ("Alt+X / :crosshair", "Cycle IBM 3270 Cross-Hair (Pane -> Full -> Off)"),
        ("Ctrl+Q / :quit", "Cleanly exit Bilterm"),
        ("═══ COMMAND PROMPT & HISTORY ═══", ""),
        ("Alt+T / Alt+B", "Pin floating prompt to Top / Bottom of canvas"),
        ("Alt+S / :prompt split", "Toggle non-overlapping Split vs Floating prompt"),
        ("Alt+Up / Alt+Down", "Float prompt vertically row-by-row"),
        ("Alt+Plus / Alt+Minus", "Increase / decrease command prompt height"),
        ("Alt+] / Alt+[", "Increase / decrease command prompt width"),
        ("Shift+PgUp / Shift+PgDn", "Scroll multi-line command prompt content"),
        ("Tab / Shift+Tab", "Tab-complete paths, commands, $ENV, and :commands"),
        ("Ctrl+R", "Fuzzy search command history (nucleo-matcher)"),
        ("Alt+P / :history", "Open Command History Pop-up Window (s = Save tmp)"),
        (":history save [path]", "Export session commands to a temporary script"),
        (":history on | off", "Enable or disable disk history persistence"),
        ("═══ SEARCH, COPY MODE & LINKS (v0.2.0) ═══", ""),
        ("Alt+/ / :search <query>", "Search OutputCanvas & highlight matches"),
        ("Alt+n / Alt+N", "Jump to next / previous canvas search match"),
        ("Alt+C / :copy", "Toggle Vi Copy Mode (hjkl, v char, V line, b block, y yank)"),
        ("Alt+LeftDrag", "Mouse rectangular block selection on OutputCanvas"),
        ("Ctrl+Click / :links", "Yank URL under mouse or last OSC 8/visible URL via OSC 52"),
        ("═══ THEMES, BORDERS & SHADOWS ═══", ""),
        (":theme <name>", "turbo-blue | turbo-cyan | 3278 | 3279 | mono"),
        (":outline <style>", "double | solid | rounded | thick | dotted | dashed | quadruple | ascii"),
        (":shadows <style>", "on | off | translucent | tinted | solid"),
        (":glyphs <pack>", "unicode | cp437 | nerd-font | ascii"),
        (":cursor <style>", "block | hollow | underscore | bar"),
        (":crosshair full|pane|off", "Configure full-screen or pane-local cross-hairs"),
    ];

    let max_scroll = HELP_LINES.len().saturating_sub(usize::from(inner.height));
    let start = scroll.min(max_scroll);
    let mut rendered = Vec::with_capacity(usize::from(inner.height));
    for &(lhs, rhs) in HELP_LINES.iter().skip(start).take(usize::from(inner.height)) {
        if rhs.is_empty() {
            rendered.push(Line::from(Span::styled(
                format!(" {lhs}"),
                Style::default()
                    .fg(Color::Rgb(255, 255, 85))
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            rendered.push(Line::from(vec![
                Span::styled(
                    format!(" {:<24}", lhs),
                    Style::default()
                        .fg(Color::Rgb(85, 255, 255))
                        .bg(palette.window_frame_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    rhs,
                    Style::default()
                        .fg(Color::White)
                        .bg(palette.window_frame_bg),
                ),
            ]));
        }
    }

    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(rendered).render(inner, buf);
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
    let system_state = if pane.line_discipline.is_password_prompt() {
        (format!(" {} !ECHO LOCK ", glyphs.status_lock_icon), Color::Red)
    } else if pane.vt.is_alternate_screen() {
        (format!(" {} ALT SCR ", glyphs.status_alt_icon), Color::Yellow)
    } else if pane.vt.scrollback_offset() > 0 {
        (format!(" {} SCROLL ", glyphs.status_scroll_icon), Color::Cyan)
    } else if pane.line_discipline.has_foreground_subcommand() {
        (format!(" {} RUNNING ", glyphs.status_run_icon), Color::Rgb(85, 255, 255))
    } else {
        (format!(" {} READY ", glyphs.status_ready_icon), Color::Rgb(0, 255, 102))
    };

    let routing_badge = match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => (" INPUT: CMD BOX ", Color::Rgb(0, 255, 102)),
        InputRoutingMode::RawPtyPassthrough => (" INPUT: RAW PTY ", Color::Rgb(255, 200, 0)),
    };

    let anchor_label = match pane.prompt.anchor {
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
        InputRoutingMode::CommandPrompt => (
            pane.prompt.cursor_row + 1,
            pane.prompt.cursor_col + 1,
        ),
        InputRoutingMode::RawPtyPassthrough => {
            let (vr, vc) = pane.vt.screen().cursor_position();
            (usize::from(vr) + 1, usize::from(vc) + 1)
        }
    };
    let coord_str = format!(
        " {} R:{:02} C:{:03} ",
        glyphs.cursor_pos_icon, disp_r, disp_c
    );
    let out_lines = pane.vt.output_line_count();
    let out_bytes = pane.vt.total_bytes_processed();
    let out_bytes_label = if out_bytes >= 1_048_576 {
        format!("{:.1}M", out_bytes as f64 / 1_048_576.0)
    } else if out_bytes >= 1024 {
        format!("{:.1}K", out_bytes as f64 / 1024.0)
    } else {
        format!("{out_bytes}B")
    };
    let out_str = format!(" {} OUT:{out_lines}L/{out_bytes_label}", glyphs.output_len_icon);

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

    if let Some(block) = pane.vt.selected_command_block() {
        let total = pane.vt.command_blocks().len();
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

/// Converts a `ratatui::buffer::Buffer` into a deterministic newline-delimited string for snapshot assertions.
pub fn buffer_to_snapshot_string(buf: &Buffer) -> String {
    let area = buf.area;
    let mut lines = Vec::with_capacity(usize::from(area.height));
    for y in area.y..area.y + area.height {
        let mut row = String::with_capacity(usize::from(area.width));
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell((x, y)) {
                row.push_str(cell.symbol());
            }
        }
        lines.push(row.trim_end().to_string());
    }
     lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_composite_pane_with_floating_prompt_and_oia_bar() {
        let mut pane = CompositePane::new_headless(1, 10, 60);
        pane.process_pty_output(b"LINE_0_UNDER_PROMPT\r\nLINE_1_UNDER_PROMPT\r\nLINE_2_UNDER_PROMPT\r\nLINE_3_VISIBLE_BELOW\r\n");
        pane.prompt.set_text("ls -la");

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        CompositePaneWidget::new(&pane, true).render(area, &mut buf);

        let rendered = buffer_to_snapshot_string(&buf);
        assert!(rendered.contains("BILTERM 3270 // CMD [TOP]"));
        assert!(rendered.contains("==> ls -la"));
        assert!(rendered.contains("[▲][▼]"));
        assert!(rendered.contains("LINE_3_VISIBLE_BELOW"));
        assert!(rendered.contains("INPUT: CMD BOX"));
    }
}
