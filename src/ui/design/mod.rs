//! Bilterm design system (Phase 45): semantic tokens, materials, density and profiles.
//!
//! Additive layer over `ThemePalette`/`BiltermConfig`. Nothing here replaces the legacy style
//! flags (`snazzy_mode`, `glass_mode`, `modern_mode`, `liquid_glass`); `AppearanceProfile::resolve`
//! maps them onto profiles so every existing CLI switch and `:style` command keeps working.
//! Themes feed `DesignTokens`, so a theme change restyles the whole application.

pub mod config;
pub mod materials;
pub mod profiles;
pub mod tokens;

pub use config::{AppearanceConfig, BorderWeight, Density, DisplayMode, TitlePolicy};
pub use materials::{Material, MaterialKind, Materials};
pub use profiles::AppearanceProfile;
pub use tokens::{ColorRole, DesignTokens};

use crate::config::BiltermConfig;
use crate::ui::theme::ThemePalette;

/// Everything a renderer needs to style a surface: colours, materials and spacing.
#[derive(Clone, Debug)]
pub struct Appearance {
    pub profile: AppearanceProfile,
    pub tokens: DesignTokens,
    pub materials: Materials,
    pub density: Density,
    pub display_mode: DisplayMode,
}

impl Appearance {
    /// Resolves the effective appearance from config and the already-resolved palette.
    pub fn resolve(cfg: &BiltermConfig, palette: &ThemePalette) -> Self {
        let profile = AppearanceProfile::resolve(cfg);
        let tokens = DesignTokens::from_palette(palette);
        let materials = profile.materials(&cfg.appearance);
        Self {
            profile,
            tokens,
            materials,
            density: cfg.appearance.density,
            display_mode: cfg.appearance.display_mode,
        }
    }
}
