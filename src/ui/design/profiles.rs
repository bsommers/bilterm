//! Appearance profiles. `Legacy` is a no-op passthrough for the pre-Phase-45 style flags.

use serde::{Deserialize, Serialize};

use super::config::AppearanceConfig;
use super::materials::{Material, Materials};
use crate::config::BiltermConfig;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppearanceProfile {
    /// Existing Classic/Snazzy/Glass look, untouched.
    Legacy,
    /// Dark, near-opaque, crisp hairlines. Default for new installs (Phase 46 adopts it).
    #[default]
    Obsidian,
    /// Deep-blue, slightly translucent.
    Midnight,
    /// Light, airy surfaces.
    Arctic,
    /// Translucent glass with bevel and halo.
    Liquid,
    /// Warm amber CRT modern dark profile (`#120C06` canvas, `#FFB000` accent, `#FFE8B8` text).
    Amber,
    /// Deep violet-indigo synthwave profile (`#140B24` canvas, `#FF2A85` / `#05D9E8` accents).
    Synthwave,
    /// Balanced studio slate solarized profile (`#002B36` canvas, `#2AA198` / `#B58900` accents).
    Solarized,
}

impl AppearanceProfile {
    /// All 8 built-in appearance profiles (`UIM-02`).
    pub const ALL: [Self; 8] = [
        Self::Legacy,
        Self::Obsidian,
        Self::Midnight,
        Self::Arctic,
        Self::Liquid,
        Self::Amber,
        Self::Synthwave,
        Self::Solarized,
    ];

    /// Explicit `[appearance].profile` wins; otherwise legacy flags map onto profiles:
    /// `liquid_glass` -> Liquid, `modern_mode` -> Obsidian, anything else -> Legacy.
    pub fn resolve(cfg: &BiltermConfig) -> Self {
        if let Some(p) = cfg.appearance.profile {
            return p;
        }
        if cfg.liquid_glass {
            Self::Liquid
        } else if cfg.modern_mode {
            Self::Obsidian
        } else {
            Self::Legacy
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "legacy" | "classic" => Some(Self::Legacy),
            "obsidian" => Some(Self::Obsidian),
            "midnight" => Some(Self::Midnight),
            "arctic" => Some(Self::Arctic),
            "liquid" | "liquid-glass" => Some(Self::Liquid),
            "amber" | "phosphor" | "retro-amber" => Some(Self::Amber),
            "synthwave" | "outrun" | "cyber" => Some(Self::Synthwave),
            "solarized" | "paper" => Some(Self::Solarized),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Obsidian => "obsidian",
            Self::Midnight => "midnight",
            Self::Arctic => "arctic",
            Self::Liquid => "liquid",
            Self::Amber => "amber",
            Self::Synthwave => "synthwave",
            Self::Solarized => "solarized",
        }
    }

    /// Semantic design tokens for this profile.
    pub fn tokens(self) -> super::tokens::DesignTokens {
        super::tokens::DesignTokens::for_profile(self)
    }

    /// Base materials for the profile with `[appearance]` overrides applied.
    pub fn materials(self, cfg: &AppearanceConfig) -> Materials {
        let mut m = match self {
            Self::Legacy => Materials::opaque_fallback(),
            Self::Obsidian => build(96, 70, false, 0, 6),
            Self::Midnight => build(90, 60, false, 15, 8),
            Self::Arctic => build(94, 55, false, 0, 8),
            Self::Liquid => build(72, 80, true, 55, 12),
            Self::Amber => build(88, 65, false, 20, 6),
            Self::Synthwave => build(82, 75, true, 35, 10),
            Self::Solarized => build(98, 65, false, 0, 4),
        };
        if self == Self::Legacy {
            return m;
        }
        for mat in [
            &mut m.pane,
            &mut m.pane_focused,
            &mut m.bar,
            &mut m.drawer,
            &mut m.floating,
            &mut m.modal,
        ] {
            if let Some(v) = cfg.opacity_pct {
                mat.opacity_pct = v.min(100);
            }
            if let Some(v) = cfg.glow_pct {
                mat.halo_pct = v.min(100);
            }
            if let Some(v) = cfg.corner_radius_px {
                mat.radius_px = v;
            }
        }
        m
    }
}

fn build(opacity: u8, border: u8, bevel: bool, halo: u8, radius: u8) -> Materials {
    let base = Material {
        opacity_pct: opacity,
        border_pct: border,
        bevel,
        halo_pct: 0,
        radius_px: radius,
        shadow: false,
    };
    Materials {
        pane: base,
        pane_focused: Material { border_pct: 100, halo_pct: halo, ..base },
        bar: Material { opacity_pct: opacity.saturating_sub(4), ..base },
        drawer: Material { opacity_pct: opacity.saturating_sub(8), halo_pct: halo / 2, shadow: true, ..base },
        floating: Material { opacity_pct: opacity.saturating_sub(10), halo_pct: halo, shadow: true, ..base },
        modal: Material { opacity_pct: opacity.saturating_sub(6), halo_pct: halo, shadow: true, ..base },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_flags_map_to_profiles() {
        let mut c = BiltermConfig::default();
        assert_eq!(AppearanceProfile::resolve(&c), AppearanceProfile::Legacy);
        c.modern_mode = true;
        assert_eq!(AppearanceProfile::resolve(&c), AppearanceProfile::Obsidian);
        c.liquid_glass = true;
        assert_eq!(AppearanceProfile::resolve(&c), AppearanceProfile::Liquid);
        c.appearance.profile = Some(AppearanceProfile::Arctic);
        assert_eq!(AppearanceProfile::resolve(&c), AppearanceProfile::Arctic);
    }

    #[test]
    fn legacy_is_opaque_and_overrides_ignored() {
        let cfg = AppearanceConfig { opacity_pct: Some(10), ..Default::default() };
        assert_eq!(AppearanceProfile::Legacy.materials(&cfg), Materials::opaque_fallback());
        assert_eq!(AppearanceProfile::Liquid.materials(&cfg).pane.opacity_pct, 10);
    }

    #[test]
    fn old_config_without_appearance_loads() {
        let c = BiltermConfig::from_toml_str("modern_mode = true\n").unwrap();
        assert_eq!(c.appearance, AppearanceConfig::default());
        let c = BiltermConfig::from_toml_str("[appearance]\nprofile = \"liquid\"\ndensity = \"compact\"\n").unwrap();
        assert_eq!(c.appearance.profile, Some(AppearanceProfile::Liquid));
    }
}
