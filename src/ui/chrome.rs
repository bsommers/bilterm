use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Widget};
use serde::{Deserialize, Serialize};

use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;

/// Selectable window and pane outline border styles (`UI-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutlineStyle {
    /// Classic Borland Turbo Vision double-line border (`╔═╗` / `║`).
    #[default]
    Double,
    /// Single-line solid box border (`┌─┐` / `│`).
    #[serde(alias = "plain", alias = "single")]
    Solid,
    /// Rounded corners with solid lines (`╭─╮` / `│`).
    Rounded,
    /// Heavy/thick box border (`┏━┓` / `┃`).
    Thick,
    /// Light dotted box border (`┌┈┐` / `┊`).
    Dotted,
    /// Light dashed box border (`┌┄┐` / `┆`).
    Dashed,
    /// Quadruple-dash box border (`┏┉┓` / `┋`).
    Quadruple,
    /// Pure 7-bit ASCII box border (`+-+` / `|`).
    Ascii,
}

pub const DOTTED_BORDER_SET: border::Set = border::Set {
    top_left: "┌",
    top_right: "┐",
    bottom_left: "└",
    bottom_right: "┘",
    vertical_left: "┊",
    vertical_right: "┊",
    horizontal_top: "┈",
    horizontal_bottom: "┈",
};

pub const DASHED_BORDER_SET: border::Set = border::Set {
    top_left: "┌",
    top_right: "┐",
    bottom_left: "└",
    bottom_right: "┘",
    vertical_left: "┆",
    vertical_right: "┆",
    horizontal_top: "┄",
    horizontal_bottom: "┄",
};

pub const ASCII_BORDER_SET: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

impl OutlineStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "double" | "dbl" | "borland" => Some(Self::Double),
            "solid" | "plain" | "single" | "line" => Some(Self::Solid),
            "rounded" | "round" => Some(Self::Rounded),
            "thick" | "heavy" | "bold" => Some(Self::Thick),
            "dotted" | "dot" | "dots" => Some(Self::Dotted),
            "dashed" | "dash" | "dashes" => Some(Self::Dashed),
            "quadruple" | "quad" => Some(Self::Quadruple),
            "ascii" => Some(Self::Ascii),
            _ => None,
        }
    }

    pub fn border_set(self) -> border::Set<'static> {
        match self {
            Self::Double => border::DOUBLE,
            Self::Solid => border::PLAIN,
            Self::Rounded => border::ROUNDED,
            Self::Thick => border::THICK,
            Self::Dotted => DOTTED_BORDER_SET,
            Self::Dashed => DASHED_BORDER_SET,
            Self::Quadruple => border::QUADRANT_OUTSIDE,
            Self::Ascii => ASCII_BORDER_SET,
        }
    }
}

/// Selectable DOS-style window/prompt drop-shadow rendering modes (`UI-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowStyle {
    /// No drop shadow.
    None,
    /// Classic Borland Turbo Vision VGA `0x08` translucent attribute shadow (`DarkGray` on `Black`)
    /// that preserves underlying characters.
    #[default]
    Translucent,
    /// Theme-tinted translucent shadow using `palette.shadow_fg` on `palette.shadow_bg` while
    /// preserving underlying characters.
    #[serde(alias = "tinted", alias = "theme")]
    ThemeTinted,
    /// Solid CP437 block/half-block shadow (`█` on right, `▀`/`█` on bottom).
    #[serde(alias = "solid", alias = "block")]
    SolidHalfBlock,
}

impl ShadowStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "none" | "off" | "false" => Some(Self::None),
            "translucent" | "on" | "true" | "borland" | "vga" => Some(Self::Translucent),
            "tinted" | "theme" | "theme-tinted" => Some(Self::ThemeTinted),
            "solid" | "block" | "solid-half-block" | "cp437" => Some(Self::SolidHalfBlock),
            _ => None,
        }
    }
}

/// Renders a classic Borland Turbo Vision translucent character-cell drop shadow (`UI-01`).
pub fn render_drop_shadow(buf: &mut Buffer, rect: Rect, bounds: Rect) {
    render_drop_shadow_styled(
        buf,
        rect,
        bounds,
        ShadowStyle::Translucent,
        &ThemePalette::default(),
    );
}

/// Renders a themeable/selectable DOS-style drop shadow (`UI-01`):
/// - **2 columns wide** along the right edge (`rect.x + rect.width .. rect.x + rect.width + 2`, `y = rect.y + 1 .. rect.y + rect.height + 1`)
/// - **1 row tall** along the bottom edge (`x = rect.x + 2 .. rect.x + rect.width + 2`, `y = rect.y + rect.height`)
pub fn render_drop_shadow_styled(
    buf: &mut Buffer,
    rect: Rect,
    bounds: Rect,
    shadow_style: ShadowStyle,
    palette: &ThemePalette,
) {
    if shadow_style == ShadowStyle::None || rect.width == 0 || rect.height == 0 {
        return;
    }
    let clamped_bounds = bounds.intersection(buf.area);

    let (fg, bg) = match shadow_style {
        ShadowStyle::None => return,
        ShadowStyle::Translucent => (Color::DarkGray, Color::Black),
        ShadowStyle::ThemeTinted | ShadowStyle::SolidHalfBlock => {
            (palette.shadow_fg, palette.shadow_bg)
        }
    };

    let cell_style = Style::default().fg(fg).bg(bg).remove_modifier(Modifier::BOLD);

    let right_x_start = rect.x.saturating_add(rect.width);
    let right_x_end = right_x_start.saturating_add(2);
    let bottom_y = rect.y.saturating_add(rect.height);

    // 2-column right shadow
    for y in rect.y.saturating_add(1)..=bottom_y {
        for x in right_x_start..right_x_end {
            if x >= clamped_bounds.x
                && x < clamped_bounds.x + clamped_bounds.width
                && y >= clamped_bounds.y
                && y < clamped_bounds.y + clamped_bounds.height
                && let Some(cell) = buf.cell_mut((x, y)) {
                    if shadow_style == ShadowStyle::SolidHalfBlock {
                        cell.set_symbol("█");
                    }
                    cell.set_style(cell_style);
                }
        }
    }

    // 1-row bottom shadow
    if bottom_y >= clamped_bounds.y && bottom_y < clamped_bounds.y + clamped_bounds.height {
        for x in rect.x.saturating_add(2)..right_x_end {
            if x >= clamped_bounds.x && x < clamped_bounds.x + clamped_bounds.width
                && let Some(cell) = buf.cell_mut((x, bottom_y)) {
                    if shadow_style == ShadowStyle::SolidHalfBlock {
                        cell.set_symbol("█");
                    }
                    cell.set_style(cell_style);
                }
        }
    }
}

/// Renders a Borland Turbo Vision window frame (`╔═╗` active / `┌─┐` inactive by default).
pub fn render_borland_frame(
    outer: Rect,
    title: &str,
    active: bool,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let style = if active {
        OutlineStyle::Double
    } else {
        OutlineStyle::Solid
    };
    render_borland_frame_styled(outer, title, active, style, palette, glyphs, buf);
}

/// Renders a Borland Turbo Vision window frame with a selectable `OutlineStyle` (`UI-01`).
pub fn render_borland_frame_styled(
    outer: Rect,
    title: &str,
    active: bool,
    outline: OutlineStyle,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if outer.width < 4 || outer.height < 3 {
        return;
    }
    let effective_outline = if !active && outline == OutlineStyle::Double {
        OutlineStyle::Solid
    } else {
        outline
    };
    let border_fg = if active {
        palette.active_border_fg
    } else {
        palette.inactive_border_fg
    };
    let frame_bg = palette.window_frame_bg;

    let block = Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(palette.title_fg)
                .bg(frame_bg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_set(effective_outline.border_set())
        .border_style(Style::default().fg(border_fg).bg(frame_bg));
    block.render(outer, buf);

    // Top-right [↕] zoom badge and [▪] close badge when window is wide enough
    if outer.width >= 24 {
        let right_edge = outer.x + outer.width;
        let badges = format!("{}{}", glyphs.zoom_badge, glyphs.close_badge);
        let start_x = right_edge.saturating_sub(badges.chars().count() as u16 + 2);
        for (i, ch) in badges.chars().enumerate() {
            let x = start_x + i as u16;
            if x < right_edge - 1
                && let Some(cell) = buf.cell_mut((x, outer.y)) {
                    let mut sym = [0u8; 4];
                    cell.set_symbol(ch.encode_utf8(&mut sym));
                    cell.set_style(
                        Style::default()
                            .fg(palette.title_fg)
                            .bg(frame_bg)
                            .add_modifier(Modifier::BOLD),
                    );
                }
        }
    }

    // Bottom-right ◢ resize corner
    let br_x = outer.x + outer.width - 1;
    let br_y = outer.y + outer.height - 1;
    if let Some(cell) = buf.cell_mut((br_x, br_y)) {
        cell.set_symbol(glyphs.resize_corner);
        cell.set_style(
            Style::default()
                .fg(palette.title_fg)
                .bg(frame_bg)
                .add_modifier(Modifier::BOLD),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_shadow_dims_2col_right_and_1row_bottom_without_erasing_text() {
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        buf.cell_mut((10, 2)).unwrap().set_symbol("S");
        buf.cell_mut((4, 5)).unwrap().set_symbol("B");

        // Window at x=2, y=1, width=8 (right edge = 10), height=4 (bottom edge = 5)
        render_drop_shadow(&mut buf, Rect::new(2, 1, 8, 4), area);

        // Right shadow at (10, 2) preserves "S" and sets DarkGray on Black
        assert_eq!(buf.cell((10, 2)).unwrap().symbol(), "S");
        assert_eq!(buf.cell((10, 2)).unwrap().style().fg, Some(Color::DarkGray));
        assert_eq!(buf.cell((10, 2)).unwrap().style().bg, Some(Color::Black));

        // Bottom shadow at (4, 5) preserves "B" and sets DarkGray on Black
        assert_eq!(buf.cell((4, 5)).unwrap().symbol(), "B");
        assert_eq!(buf.cell((4, 5)).unwrap().style().fg, Some(Color::DarkGray));
    }
}
