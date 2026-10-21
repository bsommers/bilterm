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
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::highlight::tokenize_shell_line;
use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::chrome::{render_borland_frame, render_drop_shadow};
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

        // Pass 2: Render overlaid FloatingCommandPrompt if visible
        if self.pane.is_prompt_visible() && canvas.height >= 3 {
            let prompt_rect = self.pane.prompt_rect(area);
            if cfg.drop_shadows {
                render_drop_shadow(buf, prompt_rect, canvas);
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
                render_crosshair_cursor(buf, canvas, cx, cy, palette.crosshair_bg);
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
    let border_type = if active {
        BorderType::Double
    } else {
        BorderType::Plain
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
        .border_type(border_type)
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

    let has_completion_bar = pane.prompt.completion.is_active() && inner.height >= 2;
    let editor_rows = if has_completion_bar {
        usize::from(inner.height.saturating_sub(1))
    } else {
        usize::from(inner.height)
    };
    let start_row = pane
        .prompt
        .cursor_row
        .saturating_sub(editor_rows.saturating_sub(1));

    let mut rendered_lines = Vec::with_capacity(usize::from(inner.height));
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
        let mut spans = Vec::new();
        spans.push(Span::styled(
            prefix,
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD),
        ));
        for tok in tokenize_shell_line(line_str) {
            spans.push(Span::styled(tok.text, tok.kind.style().bg(surface_bg)));
        }
        rendered_lines.push(Line::from(spans));
    }

    if has_completion_bar {
        rendered_lines.push(build_completion_bar_line(pane, inner.width, surface_bg));
    }

    Paragraph::new(rendered_lines).render(inner, buf);

    // Software cursor + optional IBM 3270 cross-hair cursor ruler
    let prefix_len = 4usize;
    let cursor_vis_row = pane.prompt.cursor_row.saturating_sub(start_row) as u16;
    let cursor_col = (prefix_len + pane.prompt.cursor_col) as u16;
    let max_editor_row = (editor_rows as u16).saturating_sub(1);
    let clamped_row = cursor_vis_row.min(max_editor_row);
    let clamped_col = cursor_col.min(inner.width.saturating_sub(1));
    let cx = inner.x + clamped_col;
    let cy = inner.y + clamped_row;

    if active && cfg.crosshair_cursor {
        render_crosshair_cursor(buf, canvas_rect, cx, cy, palette.crosshair_bg);
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
                    render_drop_shadow(buf, outer, workspace_area);
                }
                occlude_rect(buf, outer);
                let z_badge = self
                    .wm
                    .z_stack
                    .rank_of(id)
                    .map(|(r, total)| format!("Z:{r}/{total}"))
                    .unwrap_or_default();
                render_borland_frame(
                    outer,
                    &format!("[{id}] {} ({z_badge})", pane.title),
                    is_active,
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
                render_drop_shadow(buf, qt_outer, workspace_area);
            }
            occlude_rect(buf, qt_outer);
            render_borland_frame(
                qt_outer,
                "[Q] QUAKE QUICK TERM [~] (Alt+` to hide)",
                true,
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
    }
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
        (" !ECHO LOCK ", Color::Red)
    } else if pane.vt.is_alternate_screen() {
        (" ALT SCR ", Color::Yellow)
    } else if pane.vt.scrollback_offset() > 0 {
        (" SCROLL ", Color::Cyan)
    } else {
        (" READY ", Color::Rgb(0, 255, 102))
    };

    let routing_badge = match pane.effective_routing_mode() {
        InputRoutingMode::CommandPrompt => (" INPUT: CMD BOX ", Color::Rgb(0, 255, 102)),
        InputRoutingMode::RawPtyPassthrough => (" INPUT: RAW PTY ", Color::Rgb(255, 200, 0)),
    };

    let anchor_label = match pane.prompt.anchor {
        PromptAnchor::Top => "PIN:TOP",
        PromptAnchor::Bottom => "PIN:BOT",
        PromptAnchor::Floating { y_offset } => {
            if y_offset == 0 {
                "PIN:TOP"
            } else {
                "PIN:FLT"
            }
        }
    };

    let (cur_r, cur_c) = pane.vt.screen().cursor_position();
    let coord_str = format!(" R:{:02} C:{:03} ", cur_r + 1, cur_c + 1);

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
