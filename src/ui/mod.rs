use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget};

use crate::pane::{CompositePane, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::vt::cell_to_ratatui_style;

/// Ratatui widget rendering a `CompositePane` in 3 immediate-mode passes:
/// 1. **Pass 1 (`OutputCanvas`)**: Renders the full-height VT100/ANSI/TrueColor screen (`area.height - 1`)
/// 2. **Pass 2 (`FloatingCommandPrompt`)**: If visible, clears `prompt_rect` (`widgets::Clear`) and draws the bordered IBM 3270 / Borland-styled floating command prompt on top
/// 3. **Pass 3 (`OiaStatusBar`)**: Renders the bottom 1-row Operator Information Area (OIA) status bar
pub struct CompositePaneWidget<'a> {
    pub pane: &'a CompositePane,
    pub active: bool,
}

impl<'a> CompositePaneWidget<'a> {
    pub fn new(pane: &'a CompositePane, active: bool) -> Self {
        Self { pane, active }
    }
}

impl Widget for CompositePaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let canvas = self.pane.canvas_rect(area);
        let status_rect = self.pane.status_bar_rect(area);

        // Pass 1: Render full-height OutputCanvas from VtParser screen
        render_output_canvas(self.pane, canvas, buf);

        // Pass 2: Render overlaid FloatingCommandPrompt if visible
        if self.pane.is_prompt_visible() && canvas.height >= 3 {
            let prompt_rect = self.pane.prompt_rect(area);
            Clear.render(prompt_rect, buf);
            render_floating_prompt(self.pane, self.active, prompt_rect, buf);
        }

        // Pass 3: Render bottom 1-row Operator Information Area (OIA) status bar
        render_oia_status_bar(self.pane, status_rect, buf);
    }
}

fn render_output_canvas(pane: &CompositePane, canvas: Rect, buf: &mut Buffer) {
    let screen = pane.vt.screen();
    let (vt_rows, vt_cols) = pane.vt.size();
    let render_rows = canvas.height.min(vt_rows);
    let render_cols = canvas.width.min(vt_cols);

    for r in 0..render_rows {
        for c in 0..render_cols {
            let Some(vt_cell) = screen.cell(r, c) else {
                continue;
            };
            let dst_x = canvas.x + c;
            let dst_y = canvas.y + r;
            if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                let contents = vt_cell.contents();
                if contents.is_empty() {
                    dst_cell.set_symbol(" ");
                } else {
                    dst_cell.set_symbol(contents);
                }
                dst_cell.set_style(cell_to_ratatui_style(vt_cell));
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
            if let Some(dst_cell) = buf.cell_mut((canvas.x + cur_c, canvas.y + cur_r)) {
                let style = dst_cell.style().add_modifier(Modifier::REVERSED);
                dst_cell.set_style(style);
            }
        }
    }
}

fn render_floating_prompt(
    pane: &CompositePane,
    active: bool,
    prompt_rect: Rect,
    buf: &mut Buffer,
) {
    let border_type = if active {
        BorderType::Double
    } else {
        BorderType::Plain
    };
    let border_fg = if active {
        Color::Rgb(0, 255, 102)
    } else {
        Color::Cyan
    };
    let surface_bg = Color::Rgb(0, 28, 56);

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
                .fg(Color::Rgb(255, 255, 85))
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(Style::default().fg(border_fg).bg(surface_bg))
        .style(Style::default().fg(Color::White).bg(surface_bg));

    let inner = block.inner(prompt_rect);
    block.render(prompt_rect, buf);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let prefix = "==> ";
    let line = Line::from(vec![
        Span::styled(
            prefix,
            Style::default()
                .fg(Color::Rgb(0, 255, 102))
                .bg(surface_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &pane.prompt.input,
            Style::default().fg(Color::White).bg(surface_bg),
        ),
    ]);
    Paragraph::new(line).render(inner, buf);

    // Software block cursor inside the floating command prompt
    let cursor_col = (prefix.len() + pane.prompt.cursor_char_idx) as u16;
    let clamped_col = cursor_col.min(inner.width.saturating_sub(1));
    if let Some(cell) = buf.cell_mut((inner.x + clamped_col, inner.y)) {
        cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
    }
}

pub fn render_oia_status_bar(pane: &CompositePane, status_rect: Rect, buf: &mut Buffer) {
    if status_rect.width == 0 || status_rect.height == 0 {
        return;
    }

    let oia_bg = Color::Rgb(0, 0, 128);
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

    let line = Line::from(vec![
        Span::styled(
            "3270-OIA",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│", Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            system_state.0,
            Style::default()
                .fg(system_state.1)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│", Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            routing_badge.0,
            Style::default()
                .fg(routing_badge.1)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            anchor_label,
            Style::default().fg(Color::White).bg(oia_bg),
        ),
        Span::styled(" │", Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            coord_str,
            Style::default().fg(Color::Rgb(255, 255, 85)).bg(oia_bg),
        ),
    ]);

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
        pane.prompt.input = "ls -la".to_string();

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        CompositePaneWidget::new(&pane, true).render(area, &mut buf);

        let rendered = buffer_to_snapshot_string(&buf);
        assert!(rendered.contains("BILTERM 3270 // CMD [TOP]"));
        assert!(rendered.contains("==> ls -la"));
        assert!(rendered.contains("LINE_3_VISIBLE_BELOW"));
        assert!(rendered.contains("INPUT: CMD BOX"));
    }
}
