//! Semantic colour roles derived from a `ThemePalette`, so themes drive the whole app.

use ratatui::style::Color;

use crate::ui::theme::ThemePalette;

/// Named colour roles. Use these instead of `Color::Rgb` literals in UI code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorRole {
    Canvas,
    Surface,
    SurfaceRaised,
    Overlay,
    BorderIdle,
    BorderFocus,
    TextPrimary,
    TextMuted,
    Accent,
    AccentOn,
    Success,
    Warning,
    Danger,
    Scrim,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignTokens {
    pub canvas: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub overlay: Color,
    pub border_idle: Color,
    pub border_focus: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_on: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub scrim: Color,
}

/// `(r, g, b)` of an RGB colour, or `None` for named/indexed colours.
fn rgb(c: Color) -> Option<(u8, u8, u8)> {
    match c {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

/// Blends `a` toward `b` by `pct` percent; non-RGB colours are returned unchanged (`a`).
pub fn blend(a: Color, b: Color, pct: u8) -> Color {
    let (Some(x), Some(y)) = (rgb(a), rgb(b)) else {
        return a;
    };
    let t = u32::from(pct.min(100));
    let mix = |p: u8, q: u8| ((u32::from(p) * (100 - t) + u32::from(q) * t) / 100) as u8;
    Color::Rgb(mix(x.0, y.0), mix(x.1, y.1), mix(x.2, y.2))
}

impl DesignTokens {
    pub fn for_profile(profile: super::AppearanceProfile) -> Self {
        use super::AppearanceProfile as P;
        let pal = match profile {
            P::Legacy => crate::ui::theme::ThemePreset::TurboBlue.palette(),
            P::Obsidian => crate::ui::visual_style::modern_obsidian_palette(false),
            P::Liquid => crate::ui::visual_style::modern_obsidian_palette(true),
            other => crate::ui::visual_style::profile_palette(other)
                .unwrap_or_else(|| crate::ui::visual_style::modern_obsidian_palette(false)),
        };
        Self::from_palette(&pal)
    }

    pub fn from_palette(p: &ThemePalette) -> Self {
        let text = p.text_fg.unwrap_or(p.title_fg);
        Self {
            canvas: p.window_frame_bg,
            surface: p.window_frame_bg,
            surface_raised: blend(p.window_frame_bg, p.prompt_surface_bg, 100),
            overlay: blend(p.prompt_surface_bg, text, 6),
            border_idle: p.inactive_border_fg,
            border_focus: p.active_border_fg,
            text_primary: text,
            text_muted: blend(text, p.window_frame_bg, 45),
            accent: p.prompt_prefix_fg,
            accent_on: p.oia_badge_fg,
            success: Color::Rgb(74, 222, 128),
            warning: Color::Rgb(250, 204, 21),
            danger: Color::Rgb(248, 113, 113),
            scrim: blend(p.window_frame_bg, Color::Rgb(0, 0, 0), 55),
        }
    }

    pub fn get(&self, role: ColorRole) -> Color {
        match role {
            ColorRole::Canvas => self.canvas,
            ColorRole::Surface => self.surface,
            ColorRole::SurfaceRaised => self.surface_raised,
            ColorRole::Overlay => self.overlay,
            ColorRole::BorderIdle => self.border_idle,
            ColorRole::BorderFocus => self.border_focus,
            ColorRole::TextPrimary => self.text_primary,
            ColorRole::TextMuted => self.text_muted,
            ColorRole::Accent => self.accent,
            ColorRole::AccentOn => self.accent_on,
            ColorRole::Success => self.success,
            ColorRole::Warning => self.warning,
            ColorRole::Danger => self.danger,
            ColorRole::Scrim => self.scrim,
        }
    }
}
