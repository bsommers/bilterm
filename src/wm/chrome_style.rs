//! Per-window (per-pane) chrome overrides: theme palette, individual element colors, outline style, and bold borders.

use std::collections::BTreeMap;

use ratatui::style::Color;

use crate::config::BiltermConfig;
use crate::pane::PaneId;
use crate::ui::chrome::OutlineStyle;
use crate::ui::contrast::{resolve_ratatui_color_rgb, rgb_luminance};
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
}

impl PaneChromeStyle {
    pub fn is_default(&self) -> bool {
        self.palette.is_none()
            && self.outline.is_none()
            && self.bold.is_none()
            && self.colors.is_empty()
    }

    pub fn resolve_palette(&self, cfg: &BiltermConfig) -> ThemePalette {
        let mut pal = self.palette.clone().unwrap_or_else(|| cfg.palette());
        self.apply_color_overrides(&mut pal, cfg.theme);
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
        self.outline.unwrap_or(cfg.window_outline)
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

    /// Rebuilds per-window overrides from saved session panes (unknown themes are dropped).
    pub(crate) fn restore_pane_chrome(&mut self, saved: &[crate::wm::session::SavedPaneState]) {
        self.pane_chrome.clear();
        for sp in saved {
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
                theme_name,
                palette,
                outline: sp.outline,
                bold: sp.bold,
                colors,
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
            theme_name: Some(label.clone()),
            palette: Some(pal.clone()),
            outline: Some(OutlineStyle::Dashed),
            bold: Some(true),
            colors: BTreeMap::from([("font-color".to_string(), "amber".to_string())]),
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
