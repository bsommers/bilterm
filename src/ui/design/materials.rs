//! Surface materials: how translucent, bordered, lit and rounded each class of surface is.
//! The GUI renderer has no true backdrop blur, so "glass" = tint + bevel + halo + radius,
//! with `opaque_fallback` for terminals/backends that cannot blend.

/// Class of surface a material applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialKind {
    Pane,
    PaneFocused,
    Bar,
    Drawer,
    Floating,
    Modal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    /// Tint opacity, `0..=100`.
    pub opacity_pct: u8,
    /// Border opacity, `0..=100`.
    pub border_pct: u8,
    /// Specular top/left edge highlight.
    pub bevel: bool,
    /// Outer glow strength, `0..=100`.
    pub halo_pct: u8,
    /// Corner radius in pixels (GUI) — TUI uses rounded box glyphs when `> 0`.
    pub radius_px: u8,
    /// Drop shadow beneath the surface.
    pub shadow: bool,
}

impl Material {
    pub const OPAQUE: Self = Self {
        opacity_pct: 100,
        border_pct: 100,
        bevel: false,
        halo_pct: 0,
        radius_px: 0,
        shadow: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Materials {
    pub pane: Material,
    pub pane_focused: Material,
    pub bar: Material,
    pub drawer: Material,
    pub floating: Material,
    pub modal: Material,
}

impl Materials {
    pub const fn get(&self, kind: MaterialKind) -> Material {
        match kind {
            MaterialKind::Pane => self.pane,
            MaterialKind::PaneFocused => self.pane_focused,
            MaterialKind::Bar => self.bar,
            MaterialKind::Drawer => self.drawer,
            MaterialKind::Floating => self.floating,
            MaterialKind::Modal => self.modal,
        }
    }

    /// All materials forced opaque (backends without alpha blending).
    pub const fn opaque_fallback() -> Self {
        Self {
            pane: Material::OPAQUE,
            pane_focused: Material::OPAQUE,
            bar: Material::OPAQUE,
            drawer: Material::OPAQUE,
            floating: Material::OPAQUE,
            modal: Material::OPAQUE,
        }
    }
}
