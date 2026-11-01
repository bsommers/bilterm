//! Per-window (per-pane) chrome overrides: theme palette, individual element colors, outline style, and bold borders.

use std::collections::BTreeMap;

use ratatui::style::Color;

use crate::config::BiltermConfig;
use crate::pane::PaneId;
use crate::ui::chrome::OutlineStyle;
use crate::ui::contrast::{resolve_ratatui_color_rgb, rgb_luminance};
use crate::ui::cursor::{CaretStyle, CursorBlinkMode};
use crate::ui::cursor_cmd::parse_cursor_color;
use crate::ui::theme::{ThemePalette, ThemePreset};
use crate::wm::WindowManager;

/// Canonical per-window theme element keys (`:window <element> <color|reset>`).
pub const WINDOW_COLOR_ELEMENTS: &[(&str, &str)] = &[
    ("font-color", "Terminal canvas & prompt text color (aliases: fg, text, font)"),
    ("bg", "Window canvas & frame background color (aliases: background, frame-bg)"),
    ("prompt-bg", "Floating command prompt surface background color (alias: cmd-bg)"),
    ("prompt-color", "Command prompt prefix (===>) color (alias: prefix-color)"),
    ("cursor-color", "Software cursor color in this window (alias: cursor)"),
    ("frame-color", "Active window border & frame color (alias: border-color)"),
    ("inactive-border", "Inactive window border color (alias: inactive-frame)"),
    ("title-color", "Window & prompt title bar text color (alias: title)"),
    ("controls-color", "Window control badges ([_][□][X], ◢, [▲][▼]) color (alias: controls)"),
    ("status-color", "Bottom 3270-OIA status bar background color (aliases: status, oia-bg)"),
    ("badge-color", "3270-OIA leading badge background color (aliases: status-badge, oia-badge)"),
    ("crosshair-color", "3270 cross-hair / spotlight tint color (alias: ruler-color)"),
    ("shadow-color", "Drop-shadow foreground tint color (alias: shadow-fg)"),
];

/// Normalizes any user-supplied element alias to its canonical key in [`WINDOW_COLOR_ELEMENTS`].
pub fn canonical_window_color_element(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "font-color" | "font" | "fg" | "foreground" | "text" | "text-color" => Some("font-color"),
        "bg" | "background" | "canvas-bg" | "window-bg" | "frame-bg" => Some("bg"),
        "prompt-bg" | "cmd-bg" | "input-bg" | "surface-bg" => Some("prompt-bg"),
        "prompt-color" | "prompt" | "prefix-color" | "prompt-fg" => Some("prompt-color"),
        "cursor-color" | "cursor" | "caret" | "caret-color" => Some("cursor-color"),
        "frame-color" | "border-color" | "active-border" | "frame-fg" | "border-fg" => {
            Some("frame-color")
        }
        "inactive-border" | "inactive-frame" | "inactive-frame-color" | "inactive-border-color" => {
            Some("inactive-border")
        }
        "title-color" | "title" | "header-color" | "title-fg" => Some("title-color"),
        "controls-color" | "controls" | "badges-color" | "buttons-color" | "controls-fg" => {
            Some("controls-color")
        }
        "status-color" | "status" | "oia" | "oia-color" | "oia-bg" | "status-bg" => {
            Some("status-color")
        }
        "badge-color" | "status-badge" | "oia-badge" | "oia-badge-bg" | "badge" => {
            Some("badge-color")
        }
        "crosshair-color" | "crosshair-bg" | "ruler-color" => Some("crosshair-color"),
        "shadow-color" | "shadow-fg" => Some("shadow-color"),
        _ => None,
    }
}

/// Optional per-window overrides layered on top of the global `BiltermConfig`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaneChromeStyle {
    /// Built-in preset pinned for this window (`None` = custom or follow global theme).
    pub preset: Option<ThemePreset>,
    /// Display name of the per-window theme (`None` = follow global theme).
    pub theme_name: Option<String>,
    /// Resolved per-window palette (`None` = follow global palette).
    pub palette: Option<ThemePalette>,
    /// Per-window outline style (`None` = follow global `window_outline`).
    pub outline: Option<OutlineStyle>,
    /// Per-window bold border flag (`None` = follow global `window_outline_bold`).
    pub bold: Option<bool>,
    /// Per-window individual element color overrides (`canonical_key -> color_spec`).
    pub colors: BTreeMap<String, String>,
    /// Per-window cursor shape override (`None` = follow global `cursor_style`).
    pub cursor_style: Option<CaretStyle>,
    /// Per-window cursor blink override (`None` = follow global `cursor_blink`).
    pub cursor_blink: Option<CursorBlinkMode>,
    /// Per-window custom cursor character (`None` = follow global `cursor_custom_char`).
    pub cursor_custom_char: Option<String>,
    /// Per-window cursor text color override (`None` = follow global, `Some(opt)` = pinned).
    pub cursor_text_color: Option<Option<String>>,
    /// Per-window cursor color override (`None` = follow global, `Some(opt)` = pinned).
    pub cursor_color_override: Option<Option<String>>,
}

impl PaneChromeStyle {
    pub fn is_default(&self) -> bool {
        self.preset.is_none()
            && self.palette.is_none()
            && self.outline.is_none()
            && self.bold.is_none()
            && self.colors.is_empty()
            && self.cursor_style.is_none()
            && self.cursor_blink.is_none()
            && self.cursor_custom_char.is_none()
            && self.cursor_text_color.is_none()
            && self.cursor_color_override.is_none()
    }

    /// Resolves a per-pane `BiltermConfig` applying any per-window cursor overrides.
    pub fn resolve_pane_config(&self, base: &BiltermConfig) -> BiltermConfig {
        let mut cfg = base.clone();
        if let Some(p) = self.preset {
            cfg.theme = p;
        }
        if let Some(st) = self.cursor_style {
            cfg.cursor_style = st;
        }
        if let Some(bl) = self.cursor_blink {
            cfg.cursor_blink = bl;
        }
        if let Some(ref ch) = self.cursor_custom_char {
            cfg.cursor_custom_char = ch.clone();
        }
        if let Some(ref tc) = self.cursor_text_color {
            cfg.cursor_text_color = tc.clone();
        }
        if let Some(ref cc) = self.cursor_color_override {
            cfg.cursor_color = cc.clone();
        }
        if let Some(c) = self.colors.get("cursor-color") {
            cfg.cursor_color = Some(c.clone());
        }
        cfg
    }

    /// Resolves both the per-window `BiltermConfig` and `ThemePalette` for rendering.
    pub fn resolve_config_and_palette(&self, base: &BiltermConfig) -> (BiltermConfig, ThemePalette) {
        let pane_cfg = self.resolve_pane_config(base);
        let pal = self.resolve_palette(&pane_cfg);
        (pane_cfg, pal)
    }

    pub fn resolve_palette(&self, cfg: &BiltermConfig) -> ThemePalette {
        let mut pal = if self.preset == Some(ThemePreset::TurboBlue)
            && cfg.is_modern()
            && cfg.custom_palette_override.is_none()
        {
            crate::ui::visual_style::modern_obsidian_palette(cfg.liquid_glass)
        } else if let Some(ref p) = self.palette {
            p.clone()
        } else if let Some(preset) = self.preset {
            preset.palette()
        } else {
            cfg.palette()
        };
        let eff_cursor_col = self
            .colors
            .get("cursor-color")
            .cloned()
            .or_else(|| match &self.cursor_color_override {
                Some(opt) => opt.clone(),
                None => cfg.cursor_color.clone(),
            });
        if let Some(col) = eff_cursor_col.as_deref().and_then(parse_cursor_color) {
            pal.cursor_fg = Some(col);
        }
        self.apply_color_overrides(&mut pal, self.preset.unwrap_or(cfg.theme));
        pal
    }

    pub fn apply_color_overrides(&self, pal: &mut ThemePalette, theme: ThemePreset) {
        let has_prompt_bg = self.colors.contains_key("prompt-bg");
        for (key, spec) in &self.colors {
            let Some(color) = parse_cursor_color(spec) else {
                continue;
            };
            match key.as_str() {
                "font-color" => {
                    pal.text_fg = Some(color);
                    pal.fkey_bar_fg = color;
                }
                "bg" => {
                    pal.window_frame_bg = color;
                    if !has_prompt_bg {
                        pal.prompt_surface_bg = color;
                    }
                }
                "prompt-bg" => pal.prompt_surface_bg = color,
                "prompt-color" => pal.prompt_prefix_fg = color,
                "cursor-color" => pal.cursor_fg = Some(color),
                "frame-color" => pal.active_border_fg = color,
                "inactive-border" => pal.inactive_border_fg = color,
                "title-color" => pal.title_fg = color,
                "controls-color" => pal.controls_fg = Some(color),
                "status-color" => pal.oia_bg = color,
                "badge-color" => {
                    pal.oia_badge_bg = color;
                    let rgb = resolve_ratatui_color_rgb(color, theme, (0, 255, 255));
                    pal.oia_badge_fg = if rgb_luminance(rgb) < 0.22 {
                        Color::White
                    } else {
                        Color::Black
                    };
                }
                "crosshair-color" => pal.crosshair_bg = color,
                "shadow-color" => pal.shadow_fg = color,
                _ => {}
            }
        }
    }

    pub fn resolve_outline(&self, cfg: &BiltermConfig) -> OutlineStyle {
        if let Some(outline) = self.outline {
            return outline;
        }
        if cfg.is_modern() && cfg.window_outline == OutlineStyle::Double {
            OutlineStyle::Rounded
        } else {
            cfg.window_outline
        }
    }

    pub fn resolve_bold(&self, cfg: &BiltermConfig) -> bool {
        self.bold.unwrap_or(cfg.window_outline_bold)
    }

    /// Human-readable summary (`theme=…, outline=…, bold=…, colors=…`).
    pub fn describe(&self, cfg: &BiltermConfig) -> String {
        let colors_desc = if self.colors.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = self
                .colors
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!(", {}", pairs.join(", "))
        };
        format!(
            "theme={}, outline={:?}{}, bold={}{}{colors_desc}",
            self.theme_name.as_deref().unwrap_or("(global)"),
            self.resolve_outline(cfg),
            if self.outline.is_none() {
                " (global)"
            } else {
                ""
            },
            if self.resolve_bold(cfg) { "on" } else { "off" },
            if self.bold.is_none() { " (global)" } else { "" },
        )
    }
}

/// Resolves a built-in preset name or imported `.itermcolors`/TOML/YAML theme name/path to a palette.
pub fn resolve_named_palette(name: &str) -> Option<(String, ThemePalette)> {
    if let Some(preset) = ThemePreset::from_name(name) {
        return Some((preset.label().to_string(), preset.palette()));
    }
    let (path, scheme) = crate::ui::theme_import::resolve_theme_file_or_name(name)?;
    let label = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string();
    Some((label, scheme.to_theme_palette()))
}

impl WindowManager {
    /// Per-window chrome overrides (theme, outline, bold border) for `id` (default if unset).
    pub fn pane_chrome(&self, id: PaneId) -> PaneChromeStyle {
        self.pane_chrome.get(&id).cloned().unwrap_or_default()
    }

    /// Mutable per-window chrome overrides for the focused pane.
    pub fn active_pane_chrome_mut(&mut self) -> &mut PaneChromeStyle {
        let id = self.active_pane_id();
        self.pane_chrome.entry(id).or_default()
    }

    /// Removes a single per-window color override key on the focused pane and prunes empty entries.
    pub fn clear_active_pane_color_override(&mut self, key: &str) {
        let id = self.active_pane_id();
        if let Some(chrome) = self.pane_chrome.get_mut(&id) {
            chrome.colors.remove(key);
            if chrome.is_default() {
                self.pane_chrome.remove(&id);
            }
        }
    }

    /// Pins the current theme palette on all panes other than `active_id` so changing the active
    /// pane's theme does not alter other open windows (`THM-01`).
    pub fn pin_other_panes_theme(&mut self, active_id: PaneId) {
        let cur_preset = self
            .config
            .custom_palette_override
            .is_none()
            .then_some(self.config.theme);
        let cur_pal = self.config.palette();
        let cur_name = self.config.theme.label().to_string();
        let pids: Vec<PaneId> = self.panes.keys().copied().collect();
        for pid in pids {
            if pid == active_id {
                continue;
            }
            let chrome = self.pane_chrome.entry(pid).or_default();
            if chrome.palette.is_none() && chrome.preset.is_none() {
                chrome.preset = cur_preset;
                chrome.palette = Some(cur_pal.clone());
                chrome.theme_name = Some(cur_name.clone());
            }
        }
    }

    /// Clears per-window theme overrides across all panes (used by `:theme global <name>`).
    pub fn clear_all_pane_theme_overrides(&mut self) {
        let pids: Vec<PaneId> = self.pane_chrome.keys().copied().collect();
        for pid in pids {
            if let Some(chrome) = self.pane_chrome.get_mut(&pid) {
                chrome.preset = None;
                chrome.palette = None;
                chrome.theme_name = None;
                if chrome.is_default() {
                    self.pane_chrome.remove(&pid);
                }
            }
        }
    }

    /// Pins the current cursor settings on all panes other than `active_id` so changing the active
    /// pane's cursor does not alter other open windows (`UI-05`).
    pub fn pin_other_panes_cursor(&mut self, active_id: PaneId) {
        let st = self.config.cursor_style;
        let bl = self.config.cursor_blink;
        let ch = self.config.cursor_custom_char.clone();
        let tc = self.config.cursor_text_color.clone();
        let cc = self.config.cursor_color.clone();
        let pids: Vec<PaneId> = self.panes.keys().copied().collect();
        for pid in pids {
            if pid == active_id {
                continue;
            }
            let chrome = self.pane_chrome.entry(pid).or_default();
            chrome.cursor_style.get_or_insert(st);
            chrome.cursor_blink.get_or_insert(bl);
            chrome.cursor_custom_char.get_or_insert_with(|| ch.clone());
            chrome.cursor_text_color.get_or_insert_with(|| tc.clone());
            if !chrome.colors.contains_key("cursor-color") {
                chrome
                    .cursor_color_override
                    .get_or_insert_with(|| cc.clone());
            }
        }
    }

    /// Copies the current `self.config` cursor settings into the focused pane's `PaneChromeStyle`.
    pub fn sync_active_pane_cursor_from_config(&mut self) {
        let st = self.config.cursor_style;
        let bl = self.config.cursor_blink;
        let ch = self.config.cursor_custom_char.clone();
        let tc = self.config.cursor_text_color.clone();
        let cc = self.config.cursor_color.clone();
        let chrome = self.active_pane_chrome_mut();
        chrome.cursor_style = Some(st);
        chrome.cursor_blink = Some(bl);
        chrome.cursor_custom_char = Some(ch);
        chrome.cursor_text_color = Some(tc);
        chrome.cursor_color_override = Some(cc.clone());
        if let Some(c) = cc {
            chrome.colors.insert("cursor-color".to_string(), c);
        } else {
            chrome.colors.remove("cursor-color");
        }
    }

    /// Clears per-window cursor overrides across all panes (used by `:cursor global ...`).
    pub fn clear_all_pane_cursor_overrides(&mut self) {
        let pids: Vec<PaneId> = self.pane_chrome.keys().copied().collect();
        for pid in pids {
            if let Some(chrome) = self.pane_chrome.get_mut(&pid) {
                chrome.cursor_style = None;
                chrome.cursor_blink = None;
                chrome.cursor_custom_char = None;
                chrome.cursor_text_color = None;
                chrome.cursor_color_override = None;
                chrome.colors.remove("cursor-color");
                if chrome.is_default() {
                    self.pane_chrome.remove(&pid);
                }
            }
        }
    }

    /// Rebuilds per-window overrides from saved session panes (unknown themes are dropped).
    pub(crate) fn restore_pane_chrome(&mut self, saved: &[crate::wm::session::SavedPaneState]) {
        self.pane_chrome.clear();
        for sp in saved {
            let preset = sp.theme.as_deref().and_then(ThemePreset::from_name);
            let (theme_name, palette) = sp
                .theme
                .as_deref()
                .and_then(resolve_named_palette)
                .map_or((None, None), |(n, p)| (Some(n), Some(p)));
            let mut colors = BTreeMap::new();
            for (k, v) in &sp.colors {
                if let Some(canon) = canonical_window_color_element(k)
                    && parse_cursor_color(v).is_some()
                {
                    colors.insert(canon.to_string(), v.clone());
                }
            }
            let style = PaneChromeStyle {
                preset,
                theme_name,
                palette,
                outline: sp.outline,
                bold: sp.bold,
                colors,
                ..Default::default()
            };
            if !style.is_default() {
                self.pane_chrome.insert(sp.id, style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_fall_back_to_global_config() {
        let cfg = BiltermConfig::default();
        let mut s = PaneChromeStyle::default();
        assert!(s.is_default());
        assert_eq!(s.resolve_outline(&cfg), cfg.window_outline);
        assert!(!s.resolve_bold(&cfg));
        let (label, pal) = resolve_named_palette("dracula").unwrap();
        s.theme_name = Some(label);
        s.palette = Some(pal.clone());
        s.outline = Some(OutlineStyle::Dashed);
        s.bold = Some(true);
        assert_eq!(s.resolve_palette(&cfg), pal);
        assert_eq!(s.resolve_outline(&cfg), OutlineStyle::Dashed);
        assert!(s.resolve_bold(&cfg));
    }

    #[test]
    fn per_window_styles_round_trip_through_session_toml() {
        let mut wm = WindowManager::new_headless(24, 80);
        let id = wm.active_pane_id();
        let (label, pal) = resolve_named_palette("solarized").unwrap();
        *wm.active_pane_chrome_mut() = PaneChromeStyle {
            preset: Some(ThemePreset::SolarizedDark),
            theme_name: Some(label.clone()),
            palette: Some(pal.clone()),
            outline: Some(OutlineStyle::Dashed),
            bold: Some(true),
            colors: BTreeMap::from([("font-color".to_string(), "amber".to_string())]),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&wm.snapshot_session("rt")).unwrap();
        assert!(toml_str.contains("outline = \"dashed\""), "{toml_str}");
        let saved: crate::wm::session::SavedWorkspaceSession = toml::from_str(&toml_str).unwrap();
        let mut wm2 = WindowManager::new_headless(24, 80);
        wm2.restore_pane_chrome(&saved.panes);
        let got = wm2.pane_chrome(id);
        assert_eq!(got.theme_name.as_deref(), Some(label.as_str()));
        assert_eq!(got.palette, Some(pal));
        assert_eq!(got.outline, Some(OutlineStyle::Dashed));
        assert_eq!(got.bold, Some(true));
        // Legacy sessions without style keys deserialize to global defaults.
        let legacy = toml_str
            .replace("outline = \"dashed\"", "")
            .replace("bold = true", "");
        let legacy = legacy
            .lines()
            .filter(|l| {
                !l.starts_with("theme =")
                    && !l.starts_with("font-color =")
                    && !l.starts_with("[panes.colors]")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let saved: crate::wm::session::SavedWorkspaceSession = toml::from_str(&legacy).unwrap();
        wm2.restore_pane_chrome(&saved.panes);
        assert!(wm2.pane_chrome(id).is_default());
    }

    #[test]
    fn every_preset_label_re_resolves() {
        for p in ThemePreset::all() {
            assert_eq!(ThemePreset::from_name(p.label()), Some(*p), "{}", p.label());
        }
    }
}
