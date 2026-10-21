use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::ui::theme::ThemePalette;

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
