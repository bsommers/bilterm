use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Built-in retro and `iterm2-color-schemes` open color theme presets (`UI-02`, `THM-01`).
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
    /// Cyberpunk Neon high-contrast magenta/cyan/violet palette.
    #[serde(rename = "cyberpunk-neon", alias = "cyberpunk", alias = "neon")]
    CyberpunkNeon,
    /// Dracula classic purple/pink/cyan dark palette (`iterm2-color-schemes`).
    Dracula,
    /// Ethan Schoonover's Solarized Dark teal/gold/cyan palette (`iterm2-color-schemes`).
    #[serde(rename = "solarized-dark", alias = "solarized")]
    SolarizedDark,
    /// Gruvbox Dark warm retro groove amber/aqua/orange palette (`iterm2-color-schemes`).
    #[serde(rename = "gruvbox-dark", alias = "gruvbox")]
    GruvboxDark,
    /// Tokyo Night deep indigo/cyan/magenta palette (`iterm2-color-schemes`).
    #[serde(rename = "tokyo-night", alias = "tokyonight")]
    TokyoNight,
    /// Catppuccin Mocha soothing pastel dark palette (`iterm2-color-schemes`).
    #[serde(rename = "catppuccin-mocha", alias = "catppuccin")]
    CatppuccinMocha,
    /// Nord arctic bluish-cyan polar night palette (`iterm2-color-schemes`).
    Nord,
    /// Monokai Remastered high-contrast charcoal/lime/magenta/gold palette (`iterm2-color-schemes`).
    #[serde(rename = "monokai-remastered", alias = "monokai")]
    MonokaiRemastered,
    /// TRON Encom / Master Control Program (MCP) cyber-grid Black/Cyan/Orange-Red/Gold palette (`THM-01`, `MCP-01`).
    #[serde(rename = "tron-grid", alias = "tron", alias = "mcp")]
    TronGrid,
}

const ALL_THEME_PRESETS: [ThemePreset; 14] = [
    ThemePreset::TurboBlue,
    ThemePreset::TurboCyan,
    ThemePreset::Ibm3278Green,
    ThemePreset::Ibm3279Amber,
    ThemePreset::IbmMono,
    ThemePreset::CyberpunkNeon,
    ThemePreset::Dracula,
    ThemePreset::SolarizedDark,
    ThemePreset::GruvboxDark,
    ThemePreset::TokyoNight,
    ThemePreset::CatppuccinMocha,
    ThemePreset::Nord,
    ThemePreset::MonokaiRemastered,
    ThemePreset::TronGrid,
];

impl ThemePreset {
    pub const fn all() -> &'static [ThemePreset] {
        &ALL_THEME_PRESETS
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "turbo-blue" | "turboblue" | "blue" | "borland" => Some(Self::TurboBlue),
            "turbo-cyan" | "turbocyan" | "cyan" => Some(Self::TurboCyan),
            "ibm-3278-green" | "ibm-3278-grn" | "ibm3278-green" | "3278" | "green" | "ibm-green" => {
                Some(Self::Ibm3278Green)
            }
            "ibm-3279-amber" | "ibm-3279-amb" | "ibm3279-amber" | "3279" | "amber" | "ibm-amber" => {
                Some(Self::Ibm3279Amber)
            }
            "ibm-mono" | "mono" | "monochrome" => Some(Self::IbmMono),
            "cyberpunk-neon" | "cyber-neon" | "cyberpunk" | "neon" => Some(Self::CyberpunkNeon),
            "dracula" => Some(Self::Dracula),
            "solarized-dark" | "solarized-dk" | "solarized_dark" | "solarized" => Some(Self::SolarizedDark),
            "gruvbox-dark" | "gruvbox-dk" | "gruvbox_dark" | "gruvbox" => Some(Self::GruvboxDark),
            "tokyo-night" | "tokyo_night" | "tokyonight" => Some(Self::TokyoNight),
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccin" => Some(Self::CatppuccinMocha),
            "nord" => Some(Self::Nord),
            "monokai-remastered" | "monokai-rm" | "monokai_remastered" | "monokai" => {
                Some(Self::MonokaiRemastered)
            }
            "tron-grid" | "tron_grid" | "tron" | "mcp" => Some(Self::TronGrid),
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
            Self::CyberpunkNeon => "CYBER-NEON",
            Self::Dracula => "DRACULA",
            Self::SolarizedDark => "SOLARIZED-DK",
            Self::GruvboxDark => "GRUVBOX-DK",
            Self::TokyoNight => "TOKYO-NIGHT",
            Self::CatppuccinMocha => "CATPPUCCIN",
            Self::Nord => "NORD",
            Self::MonokaiRemastered => "MONOKAI-RM",
            Self::TronGrid => "TRON-GRID",
        }
    }

    pub fn palette(self) -> ThemePalette {
        use Color::{Black, Cyan, DarkGray, Gray, Rgb, White};
        let n = self.label().to_string();
        match self {
            Self::TurboBlue => ThemePalette::new(
                n, Rgb(0, 28, 56), Rgb(0, 0, 80), Rgb(0, 255, 102), Cyan, Rgb(255, 255, 85),
                Rgb(0, 255, 102), Rgb(0, 0, 128), Cyan, Black, Rgb(0, 43, 85),
                Rgb(170, 170, 170), Black, Rgb(170, 0, 0), DarkGray, Black,
            ),
            Self::TurboCyan => ThemePalette::new(
                n, Rgb(0, 96, 96), Rgb(0, 128, 128), Rgb(255, 255, 85), White, Rgb(255, 255, 85),
                Rgb(255, 255, 85), Rgb(0, 64, 64), Rgb(255, 255, 85), Black, Rgb(0, 75, 75),
                Rgb(0, 128, 128), Black, Rgb(255, 255, 85), Rgb(0, 64, 64), Rgb(0, 16, 16),
            ),
            Self::Ibm3278Green => ThemePalette::new(
                n, Rgb(5, 20, 5), Rgb(0, 24, 0), Rgb(0, 255, 102), Rgb(0, 136, 51), Rgb(85, 255, 85),
                Rgb(0, 255, 102), Rgb(0, 34, 0), Rgb(0, 255, 102), Black, Rgb(0, 51, 0),
                Rgb(0, 40, 0), Rgb(0, 255, 102), Rgb(255, 255, 85), Rgb(0, 90, 30), Rgb(0, 8, 0),
            ),
            Self::Ibm3279Amber => ThemePalette::new(
                n, Rgb(24, 12, 0), Rgb(32, 16, 0), Rgb(255, 176, 0), Rgb(153, 102, 0), Rgb(255, 215, 0),
                Rgb(255, 176, 0), Rgb(36, 18, 0), Rgb(255, 176, 0), Black, Rgb(51, 25, 0),
                Rgb(40, 20, 0), Rgb(255, 176, 0), Rgb(255, 255, 255), Rgb(120, 68, 0), Rgb(10, 4, 0),
            ),
            Self::IbmMono => ThemePalette::new(
                n, Rgb(16, 16, 16), Rgb(24, 24, 24), White, Gray, White,
                White, Rgb(26, 26, 26), White, Black, Rgb(36, 36, 36),
                Rgb(40, 40, 40), White, Gray, DarkGray, Black,
            ),
            Self::CyberpunkNeon => ThemePalette::new(
                n, Rgb(18, 8, 36), Rgb(12, 4, 24), Rgb(0, 245, 212), Rgb(155, 93, 229), Rgb(255, 42, 133),
                Rgb(0, 245, 212), Rgb(26, 10, 48), Rgb(255, 42, 133), Black, Rgb(36, 14, 68),
                Rgb(24, 8, 44), Rgb(0, 245, 212), Rgb(255, 42, 133), Rgb(90, 30, 140), Rgb(6, 2, 12),
            ),
            Self::Dracula => ThemePalette::new(
                n, Rgb(33, 34, 44), Rgb(40, 42, 54), Rgb(189, 147, 249), Rgb(98, 114, 164), Rgb(255, 121, 198),
                Rgb(80, 250, 123), Rgb(68, 71, 90), Rgb(139, 233, 253), Black, Rgb(54, 57, 73),
                Rgb(33, 34, 44), Rgb(248, 248, 242), Rgb(255, 121, 198), Rgb(68, 71, 90), Rgb(21, 22, 29),
            ),
            Self::SolarizedDark => ThemePalette::new(
                n, Rgb(7, 54, 66), Rgb(0, 43, 54), Rgb(42, 161, 152), Rgb(88, 110, 117), Rgb(181, 137, 0),
                Rgb(133, 153, 0), Rgb(7, 54, 66), Rgb(38, 139, 210), Black, Rgb(12, 66, 80),
                Rgb(7, 54, 66), Rgb(147, 161, 161), Rgb(203, 75, 22), Rgb(88, 110, 117), Rgb(0, 24, 31),
            ),
            Self::GruvboxDark => ThemePalette::new(
                n, Rgb(50, 48, 47), Rgb(40, 40, 40), Rgb(250, 189, 47), Rgb(146, 131, 116), Rgb(254, 128, 25),
                Rgb(184, 187, 38), Rgb(60, 56, 54), Rgb(142, 192, 124), Black, Rgb(60, 56, 54),
                Rgb(50, 48, 47), Rgb(235, 219, 178), Rgb(251, 73, 52), Rgb(102, 92, 84), Rgb(29, 32, 33),
            ),
            Self::TokyoNight => ThemePalette::new(
                n, Rgb(22, 22, 30), Rgb(26, 27, 38), Rgb(125, 207, 255), Rgb(86, 95, 137), Rgb(187, 154, 247),
                Rgb(158, 206, 106), Rgb(36, 40, 59), Rgb(122, 162, 247), Black, Rgb(41, 46, 66),
                Rgb(22, 22, 30), Rgb(192, 202, 245), Rgb(247, 118, 142), Rgb(65, 72, 104), Rgb(15, 15, 20),
            ),
            Self::CatppuccinMocha => ThemePalette::new(
                n, Rgb(24, 24, 37), Rgb(30, 30, 46), Rgb(203, 166, 247), Rgb(108, 112, 134), Rgb(245, 194, 231),
                Rgb(166, 227, 161), Rgb(49, 50, 68), Rgb(137, 220, 235), Black, Rgb(49, 50, 68),
                Rgb(24, 24, 37), Rgb(205, 214, 244), Rgb(250, 179, 135), Rgb(88, 91, 112), Rgb(17, 17, 27),
            ),
            Self::Nord => ThemePalette::new(
                n, Rgb(59, 66, 82), Rgb(46, 52, 64), Rgb(136, 192, 208), Rgb(76, 86, 106), Rgb(235, 203, 139),
                Rgb(163, 190, 140), Rgb(67, 76, 94), Rgb(129, 161, 193), Black, Rgb(67, 76, 94),
                Rgb(59, 66, 82), Rgb(236, 239, 244), Rgb(191, 97, 106), Rgb(76, 86, 106), Rgb(36, 41, 51),
            ),
            Self::MonokaiRemastered => ThemePalette::new(
                n, Rgb(28, 29, 24), Rgb(39, 40, 34), Rgb(166, 226, 46), Rgb(117, 113, 94), Rgb(249, 38, 114),
                Rgb(166, 226, 46), Rgb(52, 53, 45), Rgb(102, 217, 239), Black, Rgb(58, 59, 50),
                Rgb(28, 29, 24), Rgb(248, 248, 242), Rgb(253, 151, 31), Rgb(90, 88, 75), Rgb(18, 19, 15),
            ),
            Self::TronGrid => ThemePalette::new(
                n, Rgb(4, 18, 28), Rgb(2, 10, 18), Rgb(110, 240, 255), Rgb(24, 110, 148), Rgb(255, 140, 0),
                Rgb(110, 240, 255), Rgb(6, 26, 40), Rgb(255, 69, 0), Black, Rgb(10, 42, 64),
                Rgb(4, 20, 32), Rgb(110, 240, 255), Rgb(255, 200, 60), Rgb(18, 84, 120), Rgb(0, 4, 8),
            ),
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

impl ThemePalette {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: String,
        prompt_surface_bg: Color,
        window_frame_bg: Color,
        active_border_fg: Color,
        inactive_border_fg: Color,
        title_fg: Color,
        prompt_prefix_fg: Color,
        oia_bg: Color,
        oia_badge_bg: Color,
        oia_badge_fg: Color,
        crosshair_bg: Color,
        fkey_bar_bg: Color,
        fkey_bar_fg: Color,
        fkey_num_fg: Color,
        shadow_fg: Color,
        shadow_bg: Color,
    ) -> Self {
        Self {
            name,
            prompt_surface_bg,
            window_frame_bg,
            active_border_fg,
            inactive_border_fg,
            title_fg,
            prompt_prefix_fg,
            oia_bg,
            oia_badge_bg,
            oia_badge_fg,
            crosshair_bg,
            fkey_bar_bg,
            fkey_bar_fg,
            fkey_num_fg,
            shadow_fg,
            shadow_bg,
        }
    }
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
    fn resolves_all_fourteen_theme_presets_and_hex_colors() {
        assert_eq!(ThemePreset::all().len(), 14);
        for name in [
            "turbo-blue",
            "turbo-cyan",
            "3278",
            "3279",
            "mono",
            "cyberpunk",
            "dracula",
            "solarized",
            "gruvbox",
            "tokyo-night",
            "catppuccin",
            "nord",
            "monokai",
            "tron-grid",
            "mcp",
        ] {
            let preset = ThemePreset::from_name(name).expect(name);
            let pal = preset.palette();
            assert!(!pal.name.is_empty());
        }
        assert_eq!(parse_hex_color("#00FF66"), Some(Color::Rgb(0, 255, 102)));
    }
}
