use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pane::{ExitProtectionPolicy, PromptAnchor};
use crate::pane::history_store::{
    DEFAULT_HISTORY_LOAD_LIMIT, DEFAULT_HISTORY_MAX_ENTRIES, DEFAULT_HISTORY_SESSION,
    HistorySettings,
};
use crate::remote::RemoteHostProfile;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::{GlyphPack, GlyphSet};
use crate::ui::theme::{parse_hex_color, ThemePalette, ThemePreset};
use crate::vt::scrollback::{PaneScrollMode, ScrollbackConfig};
use crate::wm::DrawerConfig;

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
    /// Toggleable focal spotlight shape (`off`, `circle`, `ellipse`).
    pub spotlight_mode: SpotlightMode,
    /// Horizontal radius in columns for the focal spotlight (`4..=120`, default `22`).
    pub spotlight_rx: u16,
    /// Vertical radius in rows for the elliptical focal spotlight (`2..=60`, default `6`).
    pub spotlight_ry: u16,
    /// Fuzzy feather transition ring width as a percentage of radius (`5..=100`, default `45`).
    pub spotlight_feather_pct: u8,
    /// When `true`, quantizes the spotlight edge and falloff to discrete character-cell blocks (`▒`/`░`).
    pub spotlight_pixelated: bool,
    /// When `true`, casts the spotlight vignette across the entire workspace rather than just the active pane.
    pub spotlight_fullscreen: bool,
    /// Toggleable focal zoom (magnifying lens) shape (`off`, `circle`, `ellipse`).
    pub focal_zoom_mode: SpotlightMode,
    /// Peak magnification percentage for the focal zoom lens (`100..=400`, default `150` = `1.50x`).
    pub focal_zoom_pct: u16,
    /// Horizontal radius in columns for the focal zoom lens (`4..=120`, default `20`).
    pub focal_zoom_rx: u16,
    /// Vertical radius in rows for the elliptical focal zoom lens (`2..=60`, default `6`).
    pub focal_zoom_ry: u16,
    /// Fuzzy feather transition ring width for the focal zoom lens (`5..=100`, default `45`).
    pub focal_zoom_feather_pct: u8,
    /// When `true`, quantizes the focal zoom lens to discrete `2x1` character-cell blocks (`▒`/`░`).
    pub focal_zoom_pixelated: bool,
    /// When `true`, applies the focal zoom lens across the full screen area rather than clipping to the active pane.
    pub focal_zoom_fullscreen: bool,
    /// Enables Borland Turbo Vision 2-col right / 1-row bottom drop shadows (`UI-01`).
    pub drop_shadows: bool,
    /// Enables the bottom 1-row Borland DOS Function-Key bar (`UI-03`).
    pub show_dos_fkey_bar: bool,
    /// Enables the top 1-row Borland Turbo Vision Menu Bar (`≡ System  Window  Prompt  View  Theme  Tmux  Help`, `ECO-03`).
    pub show_top_menubar: bool,
    /// Displays the Bilterm version badge (`vX.Y.Z`) in the 3270 OIA status bar (`:version [on|off]`).
    pub show_version_badge: bool,
    /// Enables automatic command history persistence (`true` by default).
    pub history_enabled: bool,
    /// Maximum number of named history session directories to retain (default `25`).
    pub history_max_files: usize,
    /// Optional custom directory path for persisted session command history files.
    pub history_dir: Option<String>,
    /// Named persistent history session (`"default"`; overridden by `--session <name>`).
    pub history_session: String,
    /// Maximum entries kept in a session history file before compaction (default `100000`).
    pub history_max_entries: usize,
    /// Number of most recent session commands restored into a new pane's prompt history (default `10000`).
    pub history_load_limit: usize,
    /// Optional custom directory path for saved workspace session layout snapshots (`ECO-02`).
    pub session_dir: Option<String>,
    /// Default command prompt anchor (`"top"`, `"bottom"`, `"floating"`, `"split-top"`, or `"split-bottom"`).
    pub default_prompt_anchor: String,
    /// Default floating command prompt height in rows (`3..=12`).
    pub default_prompt_height: u16,
    /// Enables retro CRT phosphor scanline dimming (`:crt [on|off]`, `v0.6.0`).
    pub crt_scanlines: bool,
    /// Prefix/Leader key chord for Window Manager & Pane commands (`"Ctrl+X"` by default; supports `"Ctrl+A"`, `"Ctrl+B"`, etc.).
    pub leader_key: String,
    /// Automatically normalizes macOS `Option+<key>` Unicode symbols (`˙`, `√`, `∫`, `ƒ`, `†`, etc.) into `Alt+<key>` chords (`true` by default).
    pub macos_option_as_alt: bool,
    /// User-configurable key chord bindings (`"Ctrl+B" = ":split v"`, `v0.6.0`).
    pub keybindings: BTreeMap<String, String>,
    /// Case-insensitive output highlight trigger patterns (`:trigger add <pat>`, `v0.7.0`).
    pub triggers: Vec<String>,
    /// Declarative remote SSH host profiles (`[remotes.<name>]`, `v1.4.0`).
    pub remotes: BTreeMap<String, RemoteHostProfile>,
    /// Dynamic, memory-aware scrollback sizing (`[scrollback]`).
    pub scrollback: ScrollbackConfig,
    /// Default per-pane scrolling discipline (`"smooth"`, `"page"` / `"block"`, or `"instant"`, `SCRL-01..02`).
    pub scroll_mode: PaneScrollMode,
    /// Policy for accidental `exit`/`logout`/`Ctrl+D` on the final pane or root session (`"guard"`, `"respawn"`, `"detach"`, `"off"`, `SAFE-01`).
    pub exit_protection: ExitProtectionPolicy,
    /// Marks this workspace session as a protected Root Session (`SAFE-02`).
    pub is_root_session: bool,
    /// `herdr`-style slide-out Sessions / Windows / Panes & Fleet drawer configuration (`[drawer]`, `DRW-01`).
    pub drawer: DrawerConfig,
    /// Primary GUI / terminal font family or preset (`"Cascadia Mono"`, `"Consolas"`, `"JetBrains Mono"`, `"IBM 3270"`, etc., `FONT-01`).
    pub font_family: String,
    /// Optional paths to custom `.ttf` / `.otf` / `.ttc` font files loaded automatically on GUI startup (`FONT-02`).
    pub font_files: Vec<String>,
    /// Base terminal font size in points (`6..=72`, default `14`, `FONT-01`).
    pub font_size_pt: u16,
    /// Dynamic terminal font zoom scale percentage (`50..=300`, default `100`, `FONT-01`).
    pub font_scale_pct: u16,
    /// Base unscaled pixel width per character cell for `GuiWorkspaceRenderer` (`4..=32`, default `8`, `FONT-01`).
    pub cell_width_px: u16,
    /// Base unscaled pixel height per character cell for `GuiWorkspaceRenderer` (`8..=64`, default `16`, `FONT-01`).
    pub cell_height_px: u16,
    /// Emits `OSC 50` font-scaling passthrough sequences to the host terminal when `true` (`FONT-01`).
    pub host_font_passthrough: bool,
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
            spotlight_mode: SpotlightMode::Off,
            spotlight_rx: 22,
            spotlight_ry: 6,
            spotlight_feather_pct: 45,
            spotlight_pixelated: false,
            spotlight_fullscreen: true,
            focal_zoom_mode: SpotlightMode::Off,
            focal_zoom_pct: 150,
            focal_zoom_rx: 20,
            focal_zoom_ry: 6,
            focal_zoom_feather_pct: 45,
            focal_zoom_pixelated: false,
            focal_zoom_fullscreen: true,
            drop_shadows: false,
            show_dos_fkey_bar: false,
            show_top_menubar: false,
            show_version_badge: true,
            history_enabled: true,
            history_max_files: 25,
            history_dir: None,
            history_session: DEFAULT_HISTORY_SESSION.to_string(),
            history_max_entries: DEFAULT_HISTORY_MAX_ENTRIES,
            history_load_limit: DEFAULT_HISTORY_LOAD_LIMIT,
            session_dir: None,
            default_prompt_anchor: "top".to_string(),
            default_prompt_height: 3,
            crt_scanlines: false,
            leader_key: "Ctrl+X".to_string(),
            macos_option_as_alt: true,
            keybindings: BTreeMap::new(),
            triggers: Vec::new(),
            remotes: BTreeMap::new(),
            scrollback: ScrollbackConfig::default(),
            scroll_mode: PaneScrollMode::Smooth,
            exit_protection: ExitProtectionPolicy::Guard,
            is_root_session: false,
            drawer: DrawerConfig::default(),
            font_family: "Cascadia Mono".to_string(),
            font_files: Vec::new(),
            font_size_pt: 14,
            font_scale_pct: 100,
            cell_width_px: 8,
            cell_height_px: 16,
            host_font_passthrough: true,
            custom_palette_override: None,
        }
    }
}

impl BiltermConfig {
    /// Parses a TOML configuration string into `BiltermConfig`.
    pub fn from_toml_str(toml_text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_text)
    }

    /// Resolves `$XDG_CONFIG_HOME/bilterm/config.toml`, falling back to `~/.config/bilterm/config.toml`.
    pub fn default_path() -> Option<PathBuf> {
        config_path_from_env(
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }

    /// Loads `BiltermConfig` from `path`: `Ok(None)` if the file does not exist, `Err` if it
    /// cannot be read or parsed (so callers can surface the problem instead of silently ignoring it).
    pub fn load(path: &Path) -> std::result::Result<Option<Self>, String> {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        Self::from_toml_str(&raw)
            .map(Some)
            .map_err(|e| format!("invalid {}: {e}", path.display()))
    }

    /// Loads `BiltermConfig` from `path` if it exists, or returns `BiltermConfig::default()`.
    pub fn load_or_default(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| Self::from_toml_str(&s).ok())
            .unwrap_or_default()
    }

    /// Builds the per-pane history persistence settings.
    pub fn history_settings(&self) -> HistorySettings {
        HistorySettings {
            enabled: self.history_enabled,
            max_files: self.history_max_files,
            max_entries: self.history_max_entries,
            load_limit: self.history_load_limit,
            dir: self.history_dir.clone(),
            session: self.history_session.clone(),
        }
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

    /// Cycles the focal spotlight through:
    /// `Circle (Fuzzy)` -> `Ellipse (Fuzzy)` -> `Circle (Pixelated)` -> `Ellipse (Pixelated)` -> `Off`.
    pub fn cycle_spotlight(&mut self) -> &'static str {
        match (self.spotlight_mode, self.spotlight_pixelated) {
            (SpotlightMode::Off, _) => {
                self.spotlight_mode = SpotlightMode::Circle;
                self.spotlight_pixelated = false;
            }
            (SpotlightMode::Circle, false) => {
                self.spotlight_mode = SpotlightMode::Ellipse;
                self.spotlight_pixelated = false;
            }
            (SpotlightMode::Ellipse, false) => {
                self.spotlight_mode = SpotlightMode::Circle;
                self.spotlight_pixelated = true;
            }
            (SpotlightMode::Circle, true) => {
                self.spotlight_mode = SpotlightMode::Ellipse;
                self.spotlight_pixelated = true;
            }
            (SpotlightMode::Ellipse, true) => {
                self.spotlight_mode = SpotlightMode::Off;
                self.spotlight_pixelated = false;
            }
        }
        self.spotlight_mode.label(self.spotlight_pixelated)
    }

    /// Resizes the focal spotlight horizontal and vertical radii (`Alt+Shift+]` / `Alt+Shift+[` or `:spotlight +|-`).
    pub fn resize_spotlight(&mut self, delta_rx: i16, delta_ry: i16) {
        let next_rx = (i32::from(self.spotlight_rx) + i32::from(delta_rx)).clamp(4, 120) as u16;
        let next_ry = (i32::from(self.spotlight_ry) + i32::from(delta_ry)).clamp(2, 60) as u16;
        self.spotlight_rx = next_rx;
        self.spotlight_ry = next_ry;
    }

    /// Formats a human-readable status label for the current Focal Zoom state.
    pub fn focal_zoom_label(&self) -> String {
        if self.focal_zoom_mode == SpotlightMode::Off || self.focal_zoom_pct <= 100 {
            "OFF".to_string()
        } else {
            let mag = f32::from(self.focal_zoom_pct) / 100.0;
            format!(
                "{:.2}x {}",
                mag,
                self.focal_zoom_mode.label(self.focal_zoom_pixelated)
            )
        }
    }

    /// Cycles the Focal Zoom magnifying lens across smooth magnification levels:
    /// `Off` -> `1.25x` -> `1.50x` -> `2.00x` -> `3.00x` -> `Off`.
    pub fn cycle_focal_zoom(&mut self) -> String {
        if self.focal_zoom_mode == SpotlightMode::Off {
            self.focal_zoom_mode = SpotlightMode::Ellipse;
            self.focal_zoom_pct = 125;
        } else if self.focal_zoom_pct < 140 {
            self.focal_zoom_pct = 150;
        } else if self.focal_zoom_pct < 185 {
            self.focal_zoom_pct = 200;
        } else if self.focal_zoom_pct < 260 {
            self.focal_zoom_pct = 300;
        } else {
            self.focal_zoom_mode = SpotlightMode::Off;
            self.focal_zoom_pct = 150;
        }
        self.focal_zoom_label()
    }

    /// Steps the Focal Zoom magnification factor up or down (e.g. `+25%` / `-25%`).
    pub fn step_focal_zoom_level(&mut self, delta_pct: i16) {
        if self.focal_zoom_mode == SpotlightMode::Off {
            if delta_pct > 0 {
                self.focal_zoom_mode = SpotlightMode::Ellipse;
                self.focal_zoom_pct = 125;
            }
            return;
        }
        let next = i32::from(self.focal_zoom_pct) + i32::from(delta_pct);
        if next <= 100 {
            self.focal_zoom_mode = SpotlightMode::Off;
            self.focal_zoom_pct = 100;
        } else {
            self.focal_zoom_pct = next.clamp(105, 400) as u16;
        }
    }

    /// Resizes the Focal Zoom horizontal and vertical lens radii (`:focalzoom grow|shrink`).
    pub fn resize_focal_zoom(&mut self, delta_rx: i16, delta_ry: i16) {
        let next_rx = (i32::from(self.focal_zoom_rx) + i32::from(delta_rx)).clamp(4, 120) as u16;
        let next_ry = (i32::from(self.focal_zoom_ry) + i32::from(delta_ry)).clamp(2, 60) as u16;
        self.focal_zoom_rx = next_rx;
        self.focal_zoom_ry = next_ry;
    }

    /// Computes the scaled character cell pixel dimensions `(cell_width_px, cell_height_px)`
    /// clamped to `4..=64` px wide and `8..=128` px high (`FONT-01`).
    pub fn scaled_cell_dimensions_px(&self) -> (u16, u16) {
        let scale = u32::from(self.font_scale_pct.clamp(50, 300));
        let base_w = u32::from(self.cell_width_px.clamp(4, 32));
        let base_h = u32::from(self.cell_height_px.clamp(8, 64));
        let w = ((base_w * scale + 50) / 100).clamp(4, 64) as u16;
        let h = ((base_h * scale + 50) / 100).clamp(8, 128) as u16;
        (w, h)
    }

    /// Computes the effective scaled font point size (`6..=72` pt) (`FONT-01`).
    pub fn scaled_font_size_pt(&self) -> u16 {
        let scale = u32::from(self.font_scale_pct.clamp(50, 300));
        let base_pt = u32::from(self.font_size_pt.clamp(6, 72));
        ((base_pt * scale + 50) / 100).clamp(6, 72) as u16
    }

    /// Steps the font zoom scale percentage by `delta_pct` (clamped to `50..=300%`) (`FONT-01`).
    pub fn step_font_scale(&mut self, delta_pct: i16) {
        let next = (i32::from(self.font_scale_pct) + i32::from(delta_pct)).clamp(50, 300) as u16;
        self.font_scale_pct = next;
    }

    /// Resets `font_scale_pct` to `100%` (`FONT-01`).
    pub fn reset_font_scale(&mut self) {
        self.font_scale_pct = 100;
    }

    /// Interprets `val` as a point size when `val < 50` (`6..=49` pt, updating `font_scale_pct`
    /// proportionally relative to `14pt` base or updating `font_size_pt`) or as a direct percentage
    /// when `val >= 50` (`50..=300%`) (`FONT-01`).
    pub fn set_font_size_or_scale(&mut self, val: u16) {
        if val < 50 {
            let pt = val.clamp(6, 49);
            let base = self.font_size_pt.clamp(6, 72);
            self.font_scale_pct =
                ((u32::from(pt) * 100 + u32::from(base) / 2) / u32::from(base)).clamp(50, 300) as u16;
        } else {
            self.font_scale_pct = val.clamp(50, 300);
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

fn config_path_from_env(xdg_config_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|p| !p.as_os_str().is_empty()) {
        Some(xdg) => xdg,
        None => home?.join(".config"),
    };
    Some(base.join("bilterm").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn resolves_xdg_config_path_with_home_fallback() {
        assert_eq!(
            config_path_from_env(Some("/xdg".into()), Some("/home/u".into())),
            Some(std::path::PathBuf::from("/xdg/bilterm/config.toml"))
        );
        assert_eq!(
            config_path_from_env(Some("".into()), Some("/home/u".into())),
            Some(std::path::PathBuf::from("/home/u/.config/bilterm/config.toml"))
        );
        assert_eq!(config_path_from_env(None, None), None);
    }

    #[test]
    fn load_distinguishes_missing_file_from_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent.toml");
        assert!(BiltermConfig::load(&missing).unwrap().is_none());

        let valid = dir.path().join("valid.toml");
        std::fs::write(&valid, "leader_key = \"Ctrl+A\"\n").unwrap();
        assert_eq!(BiltermConfig::load(&valid).unwrap().unwrap().leader_key, "Ctrl+A");

        let invalid = dir.path().join("invalid.toml");
        std::fs::write(&invalid, "leader_key = [\n").unwrap();
        assert!(BiltermConfig::load(&invalid).is_err());
    }

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

    #[test]
    fn scrollback_table_is_optional_and_parses_when_present() {
        let old = BiltermConfig::from_toml_str("theme = \"ibm-mono\"\n").expect("old config");
        assert_eq!(old.scrollback, ScrollbackConfig::default());

        let cfg = BiltermConfig::from_toml_str(
            "[scrollback]\nmemory_budget = \"25%\"\nmax_lines = 1000000\nlow_memory_pct = 5\n",
        )
        .expect("scrollback config");
        use crate::vt::scrollback::{LineLimit, MemoryBudget};
        assert_eq!(cfg.scrollback.memory_budget, MemoryBudget::Percent(25));
        assert_eq!(cfg.scrollback.max_lines, LineLimit::Fixed(1_000_000));
        assert_eq!(cfg.scrollback.low_memory_pct, 5);
        assert_eq!(cfg.scrollback.min_lines, 2_000);
    }
}
