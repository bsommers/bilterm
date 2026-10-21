use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Built-in retro color theme presets (`UI-02`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreset {
    /// Borland Turbo C / Turbo Pascal Royal Blue + Phosphor Green/Yellow/Cyan (Default).
    #[default]
    TurboBlue,
    /// Borland Turbo Vision Classic Cyan dialog palette.
    TurboCyan,
    /// IBM 3278 Model 2 Phosphor Green mainframe terminal palette.
    #[serde(rename = "ibm-3278-green", alias = "ibm3278-green", alias = "3278")]
    Ibm3278Green,
    /// IBM 3279 / 3290 Gas-Plasma Amber mainframe terminal palette.
    #[serde(rename = "ibm-3279-amber", alias = "ibm3279-amber", alias = "3279")]
    Ibm3279Amber,
    /// High-contrast monochrome mainframe palette.
    IbmMono,
}

impl ThemePreset {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "turbo-blue" | "turboblue" | "blue" | "borland" => Some(Self::TurboBlue),
            "turbo-cyan" | "turbocyan" | "cyan" => Some(Self::TurboCyan),
            "ibm-3278-green" | "3278" | "green" | "ibm-green" => Some(Self::Ibm3278Green),
            "ibm-3279-amber" | "3279" | "amber" | "ibm-amber" => Some(Self::Ibm3279Amber),
            "ibm-mono" | "mono" | "monochrome" => Some(Self::IbmMono),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TurboBlue => "TURBO-BLUE",
            Self::TurboCyan => "TURBO-CYAN",
            Self::Ibm3278Green => "IBM-3278-GRN",
            Self::Ibm3279Amber => "IBM-3279-AMB",
            Self::IbmMono => "IBM-MONO",
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::TurboBlue => ThemePalette {
                name: self.label().to_string(),
                prompt_surface_bg: Color::Rgb(0, 28, 56),
                window_frame_bg: Color::Rgb(0, 0, 80),
                active_border_fg: Color::Rgb(0, 255, 102),
                inactive_border_fg: Color::Cyan,
                title_fg: Color::Rgb(255, 255, 85),
                prompt_prefix_fg: Color::Rgb(0, 255, 102),
                oia_bg: Color::Rgb(0, 0, 128),
                oia_badge_bg: Color::Cyan,
                oia_badge_fg: Color::Black,
                crosshair_bg: Color::Rgb(0, 43, 85),
                fkey_bar_bg: Color::Rgb(170, 170, 170),
                fkey_bar_fg: Color::Black,
                fkey_num_fg: Color::Rgb(170, 0, 0),
                shadow_fg: Color::DarkGray,
                shadow_bg: Color::Black,
            },
            Self::TurboCyan => ThemePalette {
                name: self.label().to_string(),
                prompt_surface_bg: Color::Rgb(0, 96, 96),
                window_frame_bg: Color::Rgb(0, 128, 128),
                active_border_fg: Color::Rgb(255, 255, 85),
                inactive_border_fg: Color::White,
                title_fg: Color::Rgb(255, 255, 85),
                prompt_prefix_fg: Color::Rgb(255, 255, 85),
                oia_bg: Color::Rgb(0, 64, 64),
                oia_badge_bg: Color::Rgb(255, 255, 85),
                oia_badge_fg: Color::Black,
                crosshair_bg: Color::Rgb(0, 75, 75),
                fkey_bar_bg: Color::Rgb(0, 128, 128),
                fkey_bar_fg: Color::Black,
                fkey_num_fg: Color::Rgb(255, 255, 85),
                shadow_fg: Color::Rgb(0, 64, 64),
                shadow_bg: Color::Rgb(0, 16, 16),
            },
            Self::Ibm3278Green => ThemePalette {
                name: self.label().to_string(),
                prompt_surface_bg: Color::Rgb(5, 20, 5),
                window_frame_bg: Color::Rgb(0, 24, 0),
                active_border_fg: Color::Rgb(0, 255, 102),
                inactive_border_fg: Color::Rgb(0, 136, 51),
                title_fg: Color::Rgb(85, 255, 85),
                prompt_prefix_fg: Color::Rgb(0, 255, 102),
                oia_bg: Color::Rgb(0, 34, 0),
                oia_badge_bg: Color::Rgb(0, 255, 102),
                oia_badge_fg: Color::Black,
                crosshair_bg: Color::Rgb(0, 51, 0),
                fkey_bar_bg: Color::Rgb(0, 40, 0),
                fkey_bar_fg: Color::Rgb(0, 255, 102),
                fkey_num_fg: Color::Rgb(255, 255, 85),
                shadow_fg: Color::Rgb(0, 90, 30),
                shadow_bg: Color::Rgb(0, 8, 0),
            },
            Self::Ibm3279Amber => ThemePalette {
                name: self.label().to_string(),
                prompt_surface_bg: Color::Rgb(24, 12, 0),
                window_frame_bg: Color::Rgb(32, 16, 0),
                active_border_fg: Color::Rgb(255, 176, 0),
                inactive_border_fg: Color::Rgb(153, 102, 0),
                title_fg: Color::Rgb(255, 215, 0),
                prompt_prefix_fg: Color::Rgb(255, 176, 0),
                oia_bg: Color::Rgb(36, 18, 0),
                oia_badge_bg: Color::Rgb(255, 176, 0),
                oia_badge_fg: Color::Black,
                crosshair_bg: Color::Rgb(51, 25, 0),
                fkey_bar_bg: Color::Rgb(40, 20, 0),
                fkey_bar_fg: Color::Rgb(255, 176, 0),
                fkey_num_fg: Color::Rgb(255, 255, 255),
                shadow_fg: Color::Rgb(120, 68, 0),
                shadow_bg: Color::Rgb(10, 4, 0),
            },
            Self::IbmMono => ThemePalette {
                name: self.label().to_string(),
                prompt_surface_bg: Color::Rgb(16, 16, 16),
                window_frame_bg: Color::Rgb(24, 24, 24),
                active_border_fg: Color::White,
                inactive_border_fg: Color::Gray,
                title_fg: Color::White,
                prompt_prefix_fg: Color::White,
                oia_bg: Color::Rgb(26, 26, 26),
                oia_badge_bg: Color::White,
                oia_badge_fg: Color::Black,
                crosshair_bg: Color::Rgb(36, 36, 36),
                fkey_bar_bg: Color::Rgb(40, 40, 40),
                fkey_bar_fg: Color::White,
                fkey_num_fg: Color::Gray,
                shadow_fg: Color::DarkGray,
                shadow_bg: Color::Black,
            },
        }
    }
}

/// Complete resolved color palette for Bilterm's compositor (`UI-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePalette {
    pub name: String,
    pub prompt_surface_bg: Color,
    pub window_frame_bg: Color,
    pub active_border_fg: Color,
    pub inactive_border_fg: Color,
    pub title_fg: Color,
    pub prompt_prefix_fg: Color,
    pub oia_bg: Color,
    pub oia_badge_bg: Color,
    pub oia_badge_fg: Color,
    pub crosshair_bg: Color,
    pub fkey_bar_bg: Color,
    pub fkey_bar_fg: Color,
    pub fkey_num_fg: Color,
    pub shadow_fg: Color,
    pub shadow_bg: Color,
}

impl Default for ThemePalette {
    fn default() -> Self {
        ThemePreset::default().palette()
    }
}

/// Parses a `#RRGGBB` hex string into a `ratatui::style::Color::Rgb`.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let trimmed = hex.trim().strip_prefix('#')?;
    if trimmed.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_all_five_retro_theme_presets_and_hex_colors() {
        for name in ["turbo-blue", "turbo-cyan", "3278", "3279", "mono"] {
            let preset = ThemePreset::from_name(name).expect(name);
            let pal = preset.palette();
            assert!(!pal.name.is_empty());
        }
        assert_eq!(parse_hex_color("#00FF66"), Some(Color::Rgb(0, 255, 102)));
    }
}
