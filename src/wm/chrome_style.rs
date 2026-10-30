//! Per-window (per-pane) chrome overrides: theme palette, outline style, and bold borders.

use crate::config::BiltermConfig;
use crate::pane::PaneId;
use crate::ui::chrome::OutlineStyle;
use crate::ui::theme::{ThemePalette, ThemePreset};
use crate::wm::WindowManager;

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
}

impl PaneChromeStyle {
    pub fn is_default(&self) -> bool {
        self.palette.is_none() && self.outline.is_none() && self.bold.is_none()
    }

    pub fn resolve_palette(&self, cfg: &BiltermConfig) -> ThemePalette {
        self.palette.clone().unwrap_or_else(|| cfg.palette())
    }

    pub fn resolve_outline(&self, cfg: &BiltermConfig) -> OutlineStyle {
        self.outline.unwrap_or(cfg.window_outline)
    }

    pub fn resolve_bold(&self, cfg: &BiltermConfig) -> bool {
        self.bold.unwrap_or(cfg.window_outline_bold)
    }

    /// Human-readable summary (`theme=…, outline=…, bold=…`).
    pub fn describe(&self, cfg: &BiltermConfig) -> String {
        format!(
            "theme={}, outline={:?}{}, bold={}{}",
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
            let style = PaneChromeStyle {
                theme_name,
                palette,
                outline: sp.outline,
                bold: sp.bold,
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
            .filter(|l| !l.starts_with("theme ="))
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
