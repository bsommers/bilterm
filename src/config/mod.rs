use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::pane::PromptAnchor;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::CaretStyle;
use crate::ui::glyphs::{GlyphPack, GlyphSet};
use crate::ui::theme::{parse_hex_color, ThemePalette, ThemePreset};

/// Declarative Bilterm configuration loaded from `~/.config/bilterm/config.toml` (`CFG-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BiltermConfig {
    pub theme: ThemePreset,
    pub glyphs: GlyphPack,
    pub cursor_style: CaretStyle,
    /// Outline border style for multi-pane windows (`double`, `solid`, `rounded`, `thick`, `dotted`, `dashed`, `quadruple`, `ascii`).
    pub window_outline: OutlineStyle,
    /// Outline border style for the command prompt pane (`double`, `solid`, `rounded`, `thick`, `dotted`, `dashed`, `quadruple`, `ascii`).
    pub prompt_outline: OutlineStyle,
    /// DOS-style drop-shadow mode (`none`, `translucent`, `theme-tinted`, `solid-half-block`).
    pub shadow_style: ShadowStyle,
    /// Enables the IBM 3270 cross-hair cursor ruler (`UI-05`).
    pub crosshair_cursor: bool,
    /// When `true`, the IBM 3270 cross-hair cursor spans the entire terminal screen rather than just the active pane.
    pub crosshair_fullscreen: bool,
    /// Enables Borland Turbo Vision 2-col right / 1-row bottom drop shadows (`UI-01`).
    pub drop_shadows: bool,
    /// Enables the bottom 1-row Borland DOS Function-Key bar (`UI-03`).
    pub show_dos_fkey_bar: bool,
    /// Enables the top 1-row Borland Turbo Vision Menu Bar (`≡ System  Window  Prompt  View  Theme  Tmux  Help`, `ECO-03`).
    pub show_top_menubar: bool,
    /// Enables automatic command history persistence (`true` by default).
    pub history_enabled: bool,
    /// Maximum number of session history files to retain in the history directory (default `25`).
    pub history_max_files: usize,
    /// Optional custom directory path for persisted session command history files.
    pub history_dir: Option<String>,
    /// Optional custom directory path for saved workspace session layout snapshots (`ECO-02`).
    pub session_dir: Option<String>,
    /// Default command prompt anchor (`"top"`, `"bottom"`, `"floating"`, `"split-top"`, or `"split-bottom"`).
    pub default_prompt_anchor: String,
    /// Default floating command prompt height in rows (`3..=12`).
    pub default_prompt_height: u16,
    /// Optional custom INI/TOML hex overrides for `ThemePalette`.
    #[serde(skip)]
    pub custom_palette_override: Option<ThemePalette>,
}

impl Default for BiltermConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreset::TurboBlue,
            glyphs: GlyphPack::Unicode,
            cursor_style: CaretStyle::SolidBlock,
            window_outline: OutlineStyle::Double,
            prompt_outline: OutlineStyle::Double,
            shadow_style: ShadowStyle::Translucent,
            crosshair_cursor: false,
            crosshair_fullscreen: false,
            drop_shadows: false,
            show_dos_fkey_bar: false,
            show_top_menubar: false,
            history_enabled: true,
            history_max_files: 25,
            history_dir: None,
            session_dir: None,
            default_prompt_anchor: "top".to_string(),
            default_prompt_height: 3,
            custom_palette_override: None,
        }
    }
}

impl BiltermConfig {
    /// Parses a TOML configuration string into `BiltermConfig`.
    pub fn from_toml_str(toml_text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_text)
    }

    /// Loads `BiltermConfig` from `path` if it exists, or returns `BiltermConfig::default()`.
    pub fn load_or_default(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| Self::from_toml_str(&s).ok())
            .unwrap_or_default()
    }

    /// Resolves the active `ThemePalette` (applying any INI/custom override if present).
    pub fn palette(&self) -> ThemePalette {
        self.custom_palette_override
            .clone()
            .unwrap_or_else(|| self.theme.palette())
    }

    /// Resolves the active `GlyphSet`.
    pub fn glyph_set(&self) -> GlyphSet {
        self.glyphs.glyphs()
    }

    /// Resolves the configured default `PromptAnchor`.
    pub fn resolved_prompt_anchor(&self) -> PromptAnchor {
        match self.default_prompt_anchor.trim().to_ascii_lowercase().as_str() {
            "bottom" | "bot" => PromptAnchor::Bottom,
            "float" | "floating" => PromptAnchor::Floating { y_offset: 4 },
            "split-top" | "split_top" | "splittop" | "split" => PromptAnchor::SplitTop,
            "split-bottom" | "split_bottom" | "splitbottom" | "split-bot" => {
                PromptAnchor::SplitBottom
            }
            _ => PromptAnchor::Top,
        }
    }
}

/// Parses a classic `.ini` theme file (`[theme]` / `[colors]` `key = #RRGGBB`) into a `ThemePalette` (`CFG-01`).
pub fn parse_ini_theme(ini_text: &str) -> ThemePalette {
    let mut palette = ThemePreset::TurboBlue.palette();
    for raw_line in ini_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[')
        {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim().trim_matches('"');
        match key.as_str() {
            "name" => palette.name = val.to_string(),
            "prompt_surface_bg" | "surface_bg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.prompt_surface_bg = c;
                }
            }
            "window_frame_bg" | "frame_bg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.window_frame_bg = c;
                }
            }
            "active_border_fg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.active_border_fg = c;
                }
            }
            "inactive_border_fg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.inactive_border_fg = c;
                }
            }
            "title_fg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.title_fg = c;
                }
            }
            "crosshair_bg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.crosshair_bg = c;
                }
            }
            "oia_bg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.oia_bg = c;
                }
            }
            "shadow_fg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.shadow_fg = c;
                }
            }
            "shadow_bg" => {
                if let Some(c) = parse_hex_color(val) {
                    palette.shadow_bg = c;
                }
            }
            _ => {}
        }
    }
    palette
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn parses_toml_config_and_ini_theme_overrides() {
        let toml_src = r#"
            theme = "ibm-3279-amber"
            glyphs = "cp437"
            cursor_style = "blinking-hollow-block"
            crosshair_cursor = true
            drop_shadows = true
            show_dos_fkey_bar = true
            default_prompt_anchor = "bottom"
            default_prompt_height = 5
        "#;
        let cfg = BiltermConfig::from_toml_str(toml_src).expect("valid toml");
        assert_eq!(cfg.theme, ThemePreset::Ibm3279Amber);
        assert_eq!(cfg.glyphs, GlyphPack::Cp437);
        assert_eq!(cfg.cursor_style, CaretStyle::BlinkingHollowBlock);
        assert!(cfg.crosshair_cursor);
        assert!(cfg.drop_shadows);
        assert!(cfg.show_dos_fkey_bar);
        assert_eq!(cfg.resolved_prompt_anchor(), PromptAnchor::Bottom);
        assert_eq!(cfg.default_prompt_height, 5);

        let ini_src = r#"
            [colors]
            name = CUSTOM-BORLAND
            surface_bg = #112233
            active_border_fg = #AABBCC
        "#;
        let pal = parse_ini_theme(ini_src);
        assert_eq!(pal.name, "CUSTOM-BORLAND");
        assert_eq!(pal.prompt_surface_bg, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(pal.active_border_fg, Color::Rgb(0xAA, 0xBB, 0xCC));
    }
}
