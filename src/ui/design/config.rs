//! `[appearance]` configuration section (all fields optional; defaults preserve legacy behaviour).

use serde::{Deserialize, Serialize};

use super::AppearanceProfile;

/// Row/column breathing room used by shell and panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

impl Density {
    /// Cells of inner padding for panels/drawers.
    pub const fn padding(self) -> u16 {
        match self {
            Self::Compact => 0,
            Self::Comfortable => 1,
            Self::Spacious => 2,
        }
    }
    /// Cells between adjacent panes.
    pub const fn gap(self) -> u16 {
        match self {
            Self::Compact | Self::Comfortable => 0,
            Self::Spacious => 1,
        }
    }
}

/// Visual weight of pane borders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BorderWeight {
    None,
    #[default]
    Hairline,
    Bold,
}

/// How much application chrome is shown (consumed by Phase 46 shell).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    /// Top bar, pane titles, status bar.
    #[default]
    Standard,
    /// Top bar only on focus; minimal pane chrome. Also spelled `minimal`.
    #[serde(alias = "minimal")]
    Focus,
    /// All chrome hidden except a one-line input row.
    Zen,
}

impl DisplayMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "standard" | "std" => Some(Self::Standard),
            "minimal" | "focus" => Some(Self::Focus),
            "zen" => Some(Self::Zen),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Focus => "minimal",
            Self::Zen => "zen",
        }
    }
}

/// Which pane frames show a title (non-Legacy only; `None` in config keeps the display-mode default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitlePolicy {
    #[default]
    All,
    Focused,
    None,
}

impl TitlePolicy {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "focused" | "focus" => Some(Self::Focused),
            "none" | "off" => Some(Self::None),
            _ => Option::None,
        }
    }

    /// Whether a pane with focus state `active` shows its title.
    pub const fn shows(self, active: bool) -> bool {
        match self {
            Self::All => true,
            Self::Focused => active,
            Self::None => false,
        }
    }
}

/// `[appearance]` section. Percent fields are `0..=100`; `None` means "use the profile default".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Explicit profile. `None` derives it from the legacy style flags (see `AppearanceProfile::resolve`).
    pub profile: Option<AppearanceProfile>,
    /// Surface opacity override (`0` fully clear .. `100` opaque).
    pub opacity_pct: Option<u8>,
    /// Halo/glow strength override (`0..=100`).
    pub glow_pct: Option<u8>,
    /// Corner radius override in pixels for the GUI renderer.
    pub corner_radius_px: Option<u8>,
    pub border: BorderWeight,
    pub density: Density,
    pub display_mode: DisplayMode,
    /// Pane title policy override. `None` keeps the display-mode default (Standard: all, Minimal: none).
    pub title: Option<TitlePolicy>,
    /// Accent-colored border on the focused pane (non-Legacy only).
    pub focus_ring: bool,
    /// Optional bundled/UI font family for chrome text (GUI only; `None` keeps `font_family`).
    pub ui_font: Option<String>,
}

impl AppearanceConfig {
    /// Clamps user-supplied percentages into range.
    pub fn sanitized(mut self) -> Self {
        self.opacity_pct = self.opacity_pct.map(|v| v.min(100));
        self.glow_pct = self.glow_pct.map(|v| v.min(100));
        self.corner_radius_px = self.corner_radius_px.map(|v| v.min(24));
        self
    }
}
