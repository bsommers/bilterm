//! Automatic WCAG 2.1 High-Contrast Highlight & Text Legibility Engine (`src/ui/contrast.rs`).
//!
//! Ensures highlighted text, diff blocks, inline code badges, tool output, and dimmed/muted ANSI
//! sequences emitted by AI CLI agents (`agy`, `claude`, `gemini`, `codex`) and TUI tools remain
//! effortlessly readable regardless of the child process's hardcoded ANSI/256-color/TrueColor choices.

use ratatui::style::{Color, Modifier, Style};

use crate::ui::theme::{ThemePalette, ThemePreset};
use crate::vt::snapshot::{CellColor, CellView};

/// Converts an XTerm 256-color index (`0..=255`) into `(r, g, b)` using the active `ThemePreset`
/// for ANSI `0..=15`, the standard $6 \times 6 \times 6$ color cube for `16..=231`, and the
/// 24-step grayscale ramp for `232..=255`.
pub fn indexed_color_to_rgb(idx: u8, theme: ThemePreset) -> (u8, u8, u8) {
    match idx {
        0..=15 => {
            let ansi = theme_ansi_rgb_table(theme);
            ansi[usize::from(idx)]
        }
        16..=231 => {
            let n = idx - 16;
            let r_idx = n / 36;
            let g_idx = (n % 36) / 6;
            let b_idx = n % 6;
            const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
            (
                CUBE[usize::from(r_idx)],
                CUBE[usize::from(g_idx)],
                CUBE[usize::from(b_idx)],
            )
        }
        232..=255 => {
            let v = 8u8.saturating_add((idx - 232).saturating_mul(10));
            (v, v, v)
        }
    }
}

/// High-legibility 16-color ANSI RGB table tuned per [`ThemePreset`].
pub const fn theme_ansi_rgb_table(theme: ThemePreset) -> [(u8, u8, u8); 16] {
    match theme {
        ThemePreset::Ibm3278Green => [
            (8, 24, 8),       // 0 Black
            (255, 95, 95),    // 1 Red
            (0, 255, 102),    // 2 Green
            (255, 230, 90),   // 3 Yellow
            (90, 200, 255),   // 4 Blue (lifted for dark CRT legibility)
            (230, 130, 255),  // 5 Magenta
            (85, 255, 220),   // 6 Cyan
            (210, 255, 210),  // 7 White
            (110, 175, 110),  // 8 Bright Black (Gray)
            (255, 130, 130),  // 9 Bright Red
            (120, 255, 150),  // 10 Bright Green
            (255, 255, 130),  // 11 Bright Yellow
            (135, 220, 255),  // 12 Bright Blue
            (245, 165, 255),  // 13 Bright Magenta
            (140, 255, 240),  // 14 Bright Cyan
            (240, 255, 240),  // 15 Bright White
        ],
        ThemePreset::Ibm3279Amber => [
            (24, 12, 0),      // 0 Black
            (255, 105, 80),   // 1 Red
            (170, 240, 90),   // 2 Green
            (255, 200, 50),   // 3 Yellow
            (130, 195, 255),  // 4 Blue
            (245, 145, 220),  // 5 Magenta
            (120, 235, 220),  // 6 Cyan
            (255, 225, 175),  // 7 White
            (175, 130, 75),   // 8 Bright Black (Gray)
            (255, 135, 110),  // 9 Bright Red
            (195, 255, 120),  // 10 Bright Green
            (255, 230, 95),   // 11 Bright Yellow
            (165, 215, 255),  // 12 Bright Blue
            (255, 175, 235),  // 13 Bright Magenta
            (160, 250, 240),  // 14 Bright Cyan
            (255, 248, 230),  // 15 Bright White
        ],
        _ => [
            (20, 24, 34),     // 0 Black
            (244, 92, 92),    // 1 Red (high-contrast coral red)
            (74, 222, 128),   // 2 Green (crisp mint green)
            (250, 204, 21),   // 3 Yellow (warm gold)
            (96, 165, 250),   // 4 Blue (lifted sky blue — never unreadable dark navy!)
            (216, 114, 255),  // 5 Magenta (lifted orchid)
            (56, 229, 245),   // 6 Cyan (bright cyan)
            (226, 232, 240),  // 7 White (crisp slate white)
            (148, 163, 184),  // 8 Bright Black / Gray (readable slate gray — never invisible!)
            (255, 122, 122),  // 9 Bright Red
            (110, 242, 154),  // 10 Bright Green
            (254, 240, 138),  // 11 Bright Yellow
            (147, 197, 253),  // 12 Bright Blue
            (233, 168, 255),  // 13 Bright Magenta
            (125, 243, 255),  // 14 Bright Cyan
            (248, 250, 252),  // 15 Bright White
        ],
    }
}

/// Converts any `ratatui::style::Color` into `(r, g, b)` using `theme` for indexed colors.
pub fn resolve_ratatui_color_rgb(c: Color, theme: ThemePreset, default_rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(idx) => indexed_color_to_rgb(idx, theme),
        Color::Black => (20, 24, 34),
        Color::Red => (244, 92, 92),
        Color::Green => (74, 222, 128),
        Color::Yellow => (250, 204, 21),
        Color::Blue => (96, 165, 250),
        Color::Magenta => (216, 114, 255),
        Color::Cyan => (56, 229, 245),
        Color::Gray => (216, 222, 233),
        Color::DarkGray => (148, 163, 184),
        Color::LightRed => (255, 122, 122),
        Color::LightGreen => (110, 242, 154),
        Color::LightYellow => (254, 240, 138),
        Color::LightBlue => (147, 197, 253),
        Color::LightMagenta => (233, 168, 255),
        Color::LightCyan => (125, 243, 255),
        Color::White => (248, 250, 252),
        Color::Reset => default_rgb,
    }
}

/// WCAG 2.1 relative luminance (`0.0..=1.0`) for an `(r, g, b)` triplet.
#[inline]
pub fn rgb_luminance(rgb: (u8, u8, u8)) -> f32 {
    fn chan(v: u8) -> f32 {
        let s = f32::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(rgb.0) + 0.7152 * chan(rgb.1) + 0.0722 * chan(rgb.2)
}

/// WCAG 2.1 contrast ratio (`1.0..=21.0`) between two `(r, g, b)` colors.
#[inline]
pub fn rgb_contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let la = rgb_luminance(a);
    let lb = rgb_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[inline]
fn mix_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let w = t.clamp(0.0, 1.0);
    let inv = 1.0 - w;
    (
        (f32::from(a.0) * inv + f32::from(b.0) * w).round() as u8,
        (f32::from(a.1) * inv + f32::from(b.1) * w).round() as u8,
        (f32::from(a.2) * inv + f32::from(b.2) * w).round() as u8,
    )
}

/// Adjusts `fg` against `bg` so that `rgb_contrast_ratio(fg, bg) >= min_ratio`, preserving `fg`'s
/// hue tint when possible and snapping to crisp white/near-black when needed.
pub fn boost_fg_contrast(fg: (u8, u8, u8), bg: (u8, u8, u8), min_ratio: f32) -> (u8, u8, u8) {
    if rgb_contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }
    let bg_lum = rgb_luminance(bg);
    // Exact WCAG 2.1 equal-contrast crossover luminance (~0.185):
    // below 0.185 white (#F8FAFC) yields >= 4.5:1; at or above 0.185 near-black (#0C1018) yields >= 4.5:1.
    let pole = if bg_lum < 0.185 {
        (248, 250, 252)
    } else {
        (12, 16, 24)
    };
    for step in 1..=8 {
        let candidate = mix_rgb(fg, pole, step as f32 * 0.125);
        if rgb_contrast_ratio(candidate, bg) >= min_ratio {
            return candidate;
        }
    }
    pole
}

/// Converts a [`CellView`] into a high-legibility `ratatui::style::Style` on the `OutputCanvas`:
/// - Resolves `inverse` (`SGR 7`) explicitly into concrete `(fg, bg)` RGB colors so reversed blocks
///   never rely on ambiguous `Color::Reset` host inversion.
/// - Enforces WCAG AA (`>= 4.5:1`) contrast on all highlighted cells (`bg != Default` or `inverse`)
///   so light-on-light or dark-on-dark highlighted agent text (`agy`, `claude`, `gemini`, `git diff`)
///   is always sharp and readable.
/// - Lifts low-contrast dark/dim foreground colors (`Indexed(4)` navy, `Indexed(8)` / `232..244` dark
///   grays, dark RGB) against the canvas background (`>= 3.8:1`) and strips `Modifier::DIM` when it
///   would reduce readability.
pub fn readable_canvas_cell_style(
    cell: &CellView<'_>,
    theme: ThemePreset,
    palette: &ThemePalette,
) -> Style {
    let default_bg = resolve_ratatui_color_rgb(palette.window_frame_bg, theme, (14, 18, 28));
    let default_fg = (232, 238, 245);

    let has_explicit_bg = !matches!(cell.bg, CellColor::Default);
    let has_explicit_fg = !matches!(cell.fg, CellColor::Default);
    let is_highlighted = has_explicit_bg || cell.inverse;

    let mut style = Style::default();
    let mut mods = Modifier::empty();
    if cell.bold {
        mods |= Modifier::BOLD;
    }
    if cell.italic {
        mods |= Modifier::ITALIC;
    }
    if cell.underline.is_underlined() {
        mods |= Modifier::UNDERLINED;
    }

    if is_highlighted {
        let raw_fg = match cell.fg {
            CellColor::Default => default_fg,
            CellColor::Indexed(idx) => indexed_color_to_rgb(idx, theme),
            CellColor::Rgb(r, g, b) => (r, g, b),
        };
        let raw_bg = match cell.bg {
            CellColor::Default => default_bg,
            CellColor::Indexed(idx) => indexed_color_to_rgb(idx, theme),
            CellColor::Rgb(r, g, b) => (r, g, b),
        };
        let (eff_fg, eff_bg) = if cell.inverse {
            (raw_bg, raw_fg)
        } else {
            (raw_fg, raw_bg)
        };

        // If the highlight background is extremely close to the default dark canvas background,
        // subtly lift/tint it so the highlight box is visually distinct, then enforce >= 5.2:1 fg contrast.
        let adj_bg = if !cell.inverse && rgb_contrast_ratio(eff_bg, default_bg) < 1.18 {
            mix_rgb(eff_bg, (58, 76, 110), 0.38)
        } else {
            eff_bg
        };
        let adj_fg = if !has_explicit_fg && !cell.inverse {
            let bg_lum = rgb_luminance(adj_bg);
            if bg_lum < 0.185 {
                (248, 250, 252)
            } else {
                (12, 16, 24)
            }
        } else {
            boost_fg_contrast(eff_fg, adj_bg, 5.2)
        };

        style = style
            .fg(Color::Rgb(adj_fg.0, adj_fg.1, adj_fg.2))
            .bg(Color::Rgb(adj_bg.0, adj_bg.1, adj_bg.2));
    } else if has_explicit_fg {
        let raw_fg = match cell.fg {
            CellColor::Indexed(idx) => indexed_color_to_rgb(idx, theme),
            CellColor::Rgb(r, g, b) => (r, g, b),
            CellColor::Default => default_fg,
        };
        // Lift dark/muted foreground colors so they have at least 4.5:1 WCAG AA contrast against the canvas
        let adj_fg = boost_fg_contrast(raw_fg, default_bg, 4.5);
        style = style.fg(Color::Rgb(adj_fg.0, adj_fg.1, adj_fg.2));
    }

    style.add_modifier(mods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vt::snapshot::UnderlineKind;

    fn sample_cell<'a>(
        fg: CellColor,
        bg: CellColor,
        inverse: bool,
    ) -> CellView<'a> {
        CellView {
            grapheme: "A",
            width: 1,
            fg,
            bg,
            bold: false,
            italic: false,
            underline: UnderlineKind::None,
            inverse,
        }
    }

    #[test]
    fn light_highlight_background_with_default_or_light_fg_enforces_dark_text_contrast() {
        let palette = ThemePreset::TurboBlue.palette();
        // Default fg on bright yellow highlight background (ANSI 11)
        let cell = sample_cell(CellColor::Default, CellColor::Indexed(11), false);
        let style = readable_canvas_cell_style(&cell, ThemePreset::TurboBlue, &palette);
        let fg = resolve_ratatui_color_rgb(style.fg.unwrap(), ThemePreset::TurboBlue, (255, 255, 255));
        let bg = resolve_ratatui_color_rgb(style.bg.unwrap(), ThemePreset::TurboBlue, (0, 0, 0));
        assert!(
            rgb_contrast_ratio(fg, bg) >= 5.0,
            "expected >= 5.0 contrast on light highlight, got {:.2} (fg={fg:?}, bg={bg:?})",
            rgb_contrast_ratio(fg, bg)
        );
    }

    #[test]
    fn dark_highlight_background_with_dark_fg_lifts_foreground_contrast() {
        let palette = ThemePreset::TokyoNight.palette();
        // Dark gray/blue fg on dark inline-code highlight background (Indexed 236)
        let cell = sample_cell(CellColor::Indexed(238), CellColor::Indexed(236), false);
        let style = readable_canvas_cell_style(&cell, ThemePreset::TokyoNight, &palette);
        let fg = resolve_ratatui_color_rgb(style.fg.unwrap(), ThemePreset::TokyoNight, (0, 0, 0));
        let bg = resolve_ratatui_color_rgb(style.bg.unwrap(), ThemePreset::TokyoNight, (0, 0, 0));
        assert!(
            rgb_contrast_ratio(fg, bg) >= 5.0,
            "expected >= 5.0 contrast on dark highlight, got {:.2} (fg={fg:?}, bg={bg:?})",
            rgb_contrast_ratio(fg, bg)
        );
    }

    #[test]
    fn inverse_sgr7_highlight_resolves_concrete_high_contrast_rgb() {
        let palette = ThemePreset::Dracula.palette();
        let cell = sample_cell(CellColor::Indexed(8), CellColor::Default, true);
        let style = readable_canvas_cell_style(&cell, ThemePreset::Dracula, &palette);
        assert!(!style.add_modifier.contains(Modifier::REVERSED));
        let fg = resolve_ratatui_color_rgb(style.fg.unwrap(), ThemePreset::Dracula, (0, 0, 0));
        let bg = resolve_ratatui_color_rgb(style.bg.unwrap(), ThemePreset::Dracula, (0, 0, 0));
        assert!(
            rgb_contrast_ratio(fg, bg) >= 5.0,
            "expected >= 5.0 contrast on SGR 7 inverse cell, got {:.2}",
            rgb_contrast_ratio(fg, bg)
        );
    }

    #[test]
    fn muted_dark_foreground_on_canvas_is_lifted_to_wcag_aa() {
        let palette = ThemePreset::TurboBlue.palette();
        let default_bg = resolve_ratatui_color_rgb(palette.window_frame_bg, ThemePreset::TurboBlue, (14, 18, 28));
        // Dark gray 235 (38, 38, 38) on dark canvas
        let cell = sample_cell(CellColor::Indexed(235), CellColor::Default, false);
        let style = readable_canvas_cell_style(&cell, ThemePreset::TurboBlue, &palette);
        let fg = resolve_ratatui_color_rgb(style.fg.unwrap(), ThemePreset::TurboBlue, (0, 0, 0));
        assert!(
            rgb_contrast_ratio(fg, default_bg) >= 4.5,
            "expected >= 4.5 contrast on dark canvas, got {:.2}",
            rgb_contrast_ratio(fg, default_bg)
        );
    }
}
