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

    /// All 14 roles as `(css-name, color)` pairs in declaration order.
    pub fn css_roles(&self) -> [(&'static str, Color); 14] {
        [
            ("canvas", self.canvas),
            ("surface", self.surface),
            ("surface-raised", self.surface_raised),
            ("overlay", self.overlay),
            ("border-idle", self.border_idle),
            ("border-focus", self.border_focus),
            ("text-primary", self.text_primary),
            ("text-muted", self.text_muted),
            ("accent", self.accent),
            ("accent-on", self.accent_on),
            ("success", self.success),
            ("warning", self.warning),
            ("danger", self.danger),
            ("scrim", self.scrim),
        ]
    }

    /// CSS custom properties (`:root { --color-canvas: #rrggbb; ... }`) for web surfaces (`WEB-FE-02`).
    /// `scrim` is emitted as translucent `rgba(...)`; non-RGB colours map to their nearest CSS value.
    pub fn to_css_variables(&self) -> String {
        let mut out = String::from(":root {\n");
        for (name, color) in self.css_roles() {
            let value = if name == "scrim" {
                let (r, g, b) = css_rgb(color);
                format!("rgba({r}, {g}, {b}, 0.55)")
            } else {
                css_hex(color)
            };
            out.push_str(&format!("  --color-{name}: {value};\n"));
        }
        out.push_str("}\n");
        out
    }
}

/// `(r, g, b)` for any colour: RGB as-is, named ANSI colours via the xterm default palette.
fn css_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black | Color::Reset => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (127, 127, 127),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Indexed(i) => indexed_rgb(i),
    }
}

/// xterm 256-colour palette entry `i`.
fn indexed_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => {
            const BASE: [(u8, u8, u8); 16] = [
                (0, 0, 0), (205, 0, 0), (0, 205, 0), (205, 205, 0), (0, 0, 238), (205, 0, 205),
                (0, 205, 205), (229, 229, 229), (127, 127, 127), (255, 0, 0), (0, 255, 0),
                (255, 255, 0), (92, 92, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255),
            ];
            BASE[usize::from(i)]
        }
        16..=231 => {
            let n = i - 16;
            let lvl = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (lvl(n / 36), lvl((n / 6) % 6), lvl(n % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}

fn css_hex(c: Color) -> String {
    let (r, g, b) = css_rgb(c);
    format!("#{r:02x}{g:02x}{b:02x}")
}

