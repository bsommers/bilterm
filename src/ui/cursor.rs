use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use serde::{Deserialize, Serialize};

/// Selectable software caret styles for the floating prompt and active PTY canvas (`UI-05`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CaretStyle {
    /// Classic inverted block cursor (`█` / `Modifier::REVERSED`).
    #[default]
    SolidBlock,
    /// IBM 3270 hollow block cursor (`▯`).
    BlinkingHollowBlock,
    /// Classic DOS underscore caret (`_`).
    Underscore,
    /// Vertical bar caret (`▏`).
    VerticalBar,
}

impl CaretStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "block" | "solid" | "solid-block" => Some(Self::SolidBlock),
            "hollow" | "hollow-block" | "blinking-hollow-block" => Some(Self::BlinkingHollowBlock),
            "underscore" | "underline" => Some(Self::Underscore),
            "bar" | "beam" | "vertical-bar" => Some(Self::VerticalBar),
            _ => None,
        }
    }
}

/// Renders the non-destructive IBM 3270 full-pane cross-hair cursor ruler (`UI-05`) by tinting
/// the background of all cells in `pane_rect` sharing `cursor_x` (vertical hair) or `cursor_y`
/// (horizontal hair) with `tint_bg`.
pub fn render_crosshair_cursor(
    buf: &mut Buffer,
    pane_rect: Rect,
    cursor_x: u16,
    cursor_y: u16,
    tint_bg: Color,
) {
    let clamped = pane_rect.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    // Horizontal hair across `cursor_y`
    if cursor_y >= clamped.y && cursor_y < clamped.y + clamped.height {
        for x in clamped.x..clamped.x + clamped.width {
            if let Some(cell) = buf.cell_mut((x, cursor_y)) {
                cell.set_style(cell.style().bg(tint_bg));
            }
        }
    }

    // Vertical hair down `cursor_x`
    if cursor_x >= clamped.x && cursor_x < clamped.x + clamped.width {
        for y in clamped.y..clamped.y + clamped.height {
            if let Some(cell) = buf.cell_mut((cursor_x, y)) {
                cell.set_style(cell.style().bg(tint_bg));
            }
        }
    }
}

/// Renders the software caret at `(cursor_x, cursor_y)` according to `caret_style` (`UI-05`).
pub fn render_caret(
    buf: &mut Buffer,
    cursor_x: u16,
    cursor_y: u16,
    caret_style: CaretStyle,
    accent_fg: Color,
) {
    let Some(cell) = buf.cell_mut((cursor_x, cursor_y)) else {
        return;
    };
    match caret_style {
        CaretStyle::SolidBlock => {
            cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
        }
        CaretStyle::BlinkingHollowBlock => {
            if cell.symbol() == " " {
                cell.set_symbol("▯");
            }
            cell.set_style(
                cell.style()
                    .fg(accent_fg)
                    .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            );
        }
        CaretStyle::Underscore => {
            if cell.symbol() == " " {
                cell.set_symbol("_");
            }
            cell.set_style(
                cell.style()
                    .fg(accent_fg)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
            );
        }
        CaretStyle::VerticalBar => {
            if cell.symbol() == " " {
                cell.set_symbol("▏");
            }
            cell.set_style(cell.style().fg(accent_fg).add_modifier(Modifier::BOLD));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosshair_cursor_tints_full_row_and_column_non_destructively() {
        let area = Rect::new(0, 0, 10, 6);
        let mut buf = Buffer::empty(area);
        buf.cell_mut((2, 3)).unwrap().set_symbol("A");

        let tint = Color::Rgb(0, 51, 0);
        render_crosshair_cursor(&mut buf, area, 5, 3, tint);
        // Cell (2, 3) keeps its "A" glyph while receiving the horizontal hair tint
        assert_eq!(buf.cell((2, 3)).unwrap().symbol(), "A");
        assert_eq!(buf.cell((2, 3)).unwrap().style().bg, Some(tint));
        // Cell (5, 1) receives the vertical hair tint
        assert_eq!(buf.cell((5, 1)).unwrap().style().bg, Some(tint));
    }
}
