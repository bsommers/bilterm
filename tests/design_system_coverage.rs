//! Public-API coverage for the design system (`bilterm::ui::design`).

use bilterm::config::BiltermConfig;
use bilterm::ui::design::{
    Appearance, AppearanceConfig, AppearanceProfile, BorderWeight, ColorRole, Density, DesignTokens,
    DisplayMode, Material, MaterialKind, Materials, TitlePolicy,
};
use bilterm::ui::design::tokens::blend;
use bilterm::ui::theme::ThemePalette;
use ratatui::style::Color;

fn palette(text_fg: Option<Color>) -> ThemePalette {
    let mut p = bilterm::ui::theme::ThemePreset::TurboBlue.palette();
    p.window_frame_bg = Color::Rgb(10, 20, 30);
    p.prompt_surface_bg = Color::Rgb(110, 120, 130);
    p.active_border_fg = Color::Rgb(1, 2, 3);
    p.inactive_border_fg = Color::Rgb(4, 5, 6);
    p.title_fg = Color::Rgb(7, 8, 9);
    p.prompt_prefix_fg = Color::Rgb(11, 12, 13);
    p.oia_badge_fg = Color::Rgb(14, 15, 16);
    p.text_fg = text_fg;
    p
}

#[test]
fn title_policy_names_and_shows() {
    for (s, want) in [
        ("all", TitlePolicy::All),
        ("focused", TitlePolicy::Focused),
        ("focus", TitlePolicy::Focused),
        ("none", TitlePolicy::None),
        ("off", TitlePolicy::None),
        ("  FOCUS ", TitlePolicy::Focused),
        ("Off", TitlePolicy::None),
    ] {
        assert_eq!(TitlePolicy::from_name(s), Some(want), "{s}");
    }
    for bad in ["", "some", "yes", "focuss"] {
        assert_eq!(TitlePolicy::from_name(bad), None, "{bad}");
    }
    assert!(TitlePolicy::All.shows(true) && TitlePolicy::All.shows(false));
    assert!(TitlePolicy::Focused.shows(true) && !TitlePolicy::Focused.shows(false));
    assert!(!TitlePolicy::None.shows(true) && !TitlePolicy::None.shows(false));
    assert_eq!(TitlePolicy::default(), TitlePolicy::All);
}

#[test]
fn display_mode_names_labels_and_aliases() {
    assert_eq!(DisplayMode::from_name("standard"), Some(DisplayMode::Standard));
    assert_eq!(DisplayMode::from_name(" STD "), Some(DisplayMode::Standard));
    assert_eq!(DisplayMode::from_name("minimal"), Some(DisplayMode::Focus));
    assert_eq!(DisplayMode::from_name("focus"), Some(DisplayMode::Focus));
    assert_eq!(DisplayMode::from_name("zen"), Some(DisplayMode::Zen));
    assert_eq!(DisplayMode::from_name("loud"), None);
    assert_eq!(DisplayMode::Standard.label(), "standard");
    assert_eq!(DisplayMode::Focus.label(), "minimal");
    assert_eq!(DisplayMode::Zen.label(), "zen");
    assert_eq!(DisplayMode::from_name(DisplayMode::Focus.label()), Some(DisplayMode::Focus));
}

#[test]
fn density_values() {
    assert_eq!((Density::Compact.padding(), Density::Compact.gap()), (0, 0));
    assert_eq!((Density::Comfortable.padding(), Density::Comfortable.gap()), (1, 0));
    assert_eq!((Density::Spacious.padding(), Density::Spacious.gap()), (2, 1));
    assert_eq!(Density::default(), Density::Comfortable);
    assert_eq!(BorderWeight::default(), BorderWeight::Hairline);
}

#[test]
fn appearance_config_defaults() {
    let c = AppearanceConfig::default();
    assert_eq!(c.profile, None);
    assert_eq!(c.opacity_pct, None);
    assert_eq!(c.glow_pct, None);
    assert_eq!(c.corner_radius_px, None);
    assert_eq!(c.border, BorderWeight::Hairline);
    assert_eq!(c.density, Density::Comfortable);
    assert_eq!(c.display_mode, DisplayMode::Standard);
    assert_eq!(c.title, None);
    assert!(!c.focus_ring);
    assert_eq!(c.ui_font, None);
}

#[test]
fn appearance_config_toml_parse_and_roundtrip() {
    let toml_src = "profile = \"midnight\"\ntitle = \"focused\"\nfocus_ring = true\n\
        display_mode = \"minimal\"\nopacity_pct = 80\nborder = \"bold\"\ndensity = \"spacious\"\nui_font = \"Inter\"\n";
    let c: AppearanceConfig = toml::from_str(toml_src).unwrap();
    assert_eq!(c.profile, Some(AppearanceProfile::Midnight));
    assert_eq!(c.title, Some(TitlePolicy::Focused));
    assert!(c.focus_ring);
    assert_eq!(c.display_mode, DisplayMode::Focus);
    assert_eq!(c.opacity_pct, Some(80));
    assert_eq!(c.border, BorderWeight::Bold);
    assert_eq!(c.density, Density::Spacious);
    assert_eq!(c.ui_font.as_deref(), Some("Inter"));
    assert_eq!(c.glow_pct, None);

    let back: AppearanceConfig = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(back, c);

    // Through the full config section too.
    let full = BiltermConfig::from_toml_str(
        "[appearance]\nprofile = \"obsidian\"\ntitle = \"none\"\nfocus_ring = true\ndisplay_mode = \"zen\"\n",
    )
    .unwrap();
    assert_eq!(full.appearance.profile, Some(AppearanceProfile::Obsidian));
    assert_eq!(full.appearance.title, Some(TitlePolicy::None));
    assert!(full.appearance.focus_ring);
    assert_eq!(full.appearance.display_mode, DisplayMode::Zen);
}

#[test]
fn appearance_config_missing_fields_default_and_invalid_errors() {
    let empty: AppearanceConfig = toml::from_str("").unwrap();
    assert_eq!(empty, AppearanceConfig::default());
    let partial: AppearanceConfig = toml::from_str("focus_ring = true").unwrap();
    assert!(partial.focus_ring);
    assert_eq!(partial.profile, None);
    assert_eq!(partial.density, Density::Comfortable);

    assert!(toml::from_str::<AppearanceConfig>("profile = \"neon\"").is_err());
    assert!(toml::from_str::<AppearanceConfig>("title = \"sometimes\"").is_err());
    assert!(toml::from_str::<AppearanceConfig>("display_mode = \"huge\"").is_err());
    assert!(toml::from_str::<AppearanceConfig>("density = \"tight\"").is_err());
    assert!(toml::from_str::<AppearanceConfig>("opacity_pct = 300").is_err());
}

#[test]
fn sanitized_clamps_ranges() {
    let c = AppearanceConfig {
        opacity_pct: Some(250),
        glow_pct: Some(101),
        corner_radius_px: Some(200),
        ..Default::default()
    }
    .sanitized();
    assert_eq!((c.opacity_pct, c.glow_pct, c.corner_radius_px), (Some(100), Some(100), Some(24)));
    let ok = AppearanceConfig {
        opacity_pct: Some(40),
        glow_pct: Some(0),
        corner_radius_px: Some(24),
        ..Default::default()
    }
    .sanitized();
    assert_eq!((ok.opacity_pct, ok.glow_pct, ok.corner_radius_px), (Some(40), Some(0), Some(24)));
    assert_eq!(AppearanceConfig::default().sanitized(), AppearanceConfig::default());
}

#[test]
fn profile_names_and_labels() {
    use AppearanceProfile::*;
    for (n, p) in [("legacy", Legacy), ("classic", Legacy), ("obsidian", Obsidian), ("midnight", Midnight), ("arctic", Arctic), ("liquid", Liquid), (" LIQUID ", Liquid)] {
        assert_eq!(AppearanceProfile::from_name(n), Some(p), "{n}");
    }
    assert_eq!(AppearanceProfile::from_name("neon"), None);
    for (p, l) in [(Legacy, "legacy"), (Obsidian, "obsidian"), (Midnight, "midnight"), (Arctic, "arctic"), (Liquid, "liquid")] {
        assert_eq!(p.label(), l);
        assert_eq!(AppearanceProfile::from_name(l), Some(p));
    }
}

fn mat(op: u8, border: u8, bevel: bool, halo: u8, r: u8, shadow: bool) -> Material {
    Material { opacity_pct: op, border_pct: border, bevel, halo_pct: halo, radius_px: r, shadow }
}

#[test]
fn obsidian_and_liquid_material_tables() {
    let cfg = AppearanceConfig::default();
    let o = AppearanceProfile::Obsidian.materials(&cfg);
    assert_eq!(o.pane, mat(96, 70, false, 0, 6, false));
    assert_eq!(o.pane_focused, mat(96, 100, false, 0, 6, false));
    assert_eq!(o.bar, mat(92, 70, false, 0, 6, false));
    assert_eq!(o.drawer, mat(88, 70, false, 0, 6, true));
    assert_eq!(o.floating, mat(86, 70, false, 0, 6, true));
    assert_eq!(o.modal, mat(90, 70, false, 0, 6, true));

    let l = AppearanceProfile::Liquid.materials(&cfg);
    assert_eq!(l.pane, mat(72, 80, true, 0, 12, false));
    assert_eq!(l.pane_focused, mat(72, 100, true, 55, 12, false));
    assert_eq!(l.bar, mat(68, 80, true, 0, 12, false));
    assert_eq!(l.drawer, mat(64, 80, true, 27, 12, true));
    assert_eq!(l.floating, mat(62, 80, true, 55, 12, true));
    assert_eq!(l.modal, mat(66, 80, true, 55, 12, true));

    let m = AppearanceProfile::Midnight.materials(&cfg);
    assert_eq!(m.pane, mat(90, 60, false, 0, 8, false));
    assert_eq!(m.pane_focused.halo_pct, 15);
    assert_eq!(m.drawer.halo_pct, 7);
    let a = AppearanceProfile::Arctic.materials(&cfg);
    assert_eq!(a.pane, mat(94, 55, false, 0, 8, false));
}

#[test]
fn material_overrides_and_clamping() {
    let cfg = AppearanceConfig { opacity_pct: Some(250), glow_pct: Some(220), corner_radius_px: Some(3), ..Default::default() };
    let m = AppearanceProfile::Obsidian.materials(&cfg);
    for kind in [MaterialKind::Pane, MaterialKind::PaneFocused, MaterialKind::Bar, MaterialKind::Drawer, MaterialKind::Floating, MaterialKind::Modal] {
        let x = m.get(kind);
        assert_eq!((x.opacity_pct, x.halo_pct, x.radius_px), (100, 100, 3), "{kind:?}");
    }
    // Unset overrides leave per-kind differences intact; border untouched by overrides.
    let m = AppearanceProfile::Liquid.materials(&AppearanceConfig { opacity_pct: Some(10), ..Default::default() });
    assert_eq!(m.pane.opacity_pct, 10);
    assert_eq!(m.pane_focused.halo_pct, 55);
    assert_eq!(m.pane.border_pct, 80);
    // Legacy ignores every override.
    let all = AppearanceConfig { opacity_pct: Some(1), glow_pct: Some(1), corner_radius_px: Some(1), ..Default::default() };
    assert_eq!(AppearanceProfile::Legacy.materials(&all), Materials::opaque_fallback());
}

#[test]
fn materials_get_and_opaque_fallback() {
    assert_eq!(Material::OPAQUE, mat(100, 100, false, 0, 0, false));
    let f = Materials::opaque_fallback();
    for kind in [MaterialKind::Pane, MaterialKind::PaneFocused, MaterialKind::Bar, MaterialKind::Drawer, MaterialKind::Floating, MaterialKind::Modal] {
        assert_eq!(f.get(kind), Material::OPAQUE);
    }
    let mut m = f;
    m.pane = mat(1, 2, true, 3, 4, true);
    m.pane_focused = mat(5, 6, false, 7, 8, false);
    m.bar = mat(9, 9, false, 9, 9, false);
    m.drawer = mat(10, 10, false, 10, 10, false);
    m.floating = mat(11, 11, false, 11, 11, false);
    m.modal = mat(12, 12, false, 12, 12, false);
    assert_eq!(m.get(MaterialKind::Pane).opacity_pct, 1);
    assert_eq!(m.get(MaterialKind::PaneFocused).opacity_pct, 5);
    assert_eq!(m.get(MaterialKind::Bar).opacity_pct, 9);
    assert_eq!(m.get(MaterialKind::Drawer).opacity_pct, 10);
    assert_eq!(m.get(MaterialKind::Floating).opacity_pct, 11);
    assert_eq!(m.get(MaterialKind::Modal).opacity_pct, 12);
}

#[test]
fn blend_math() {
    let a = Color::Rgb(0, 100, 200);
    let b = Color::Rgb(100, 0, 0);
    assert_eq!(blend(a, b, 0), a);
    assert_eq!(blend(a, b, 100), b);
    assert_eq!(blend(a, b, 50), Color::Rgb(50, 50, 100));
    assert_eq!(blend(a, b, 250), b, "pct clamps to 100");
    assert_eq!(blend(Color::Red, b, 50), Color::Red, "non-rgb passthrough");
    assert_eq!(blend(a, Color::Blue, 50), a);
}

#[test]
fn tokens_from_palette_exact_values() {
    let t = DesignTokens::from_palette(&palette(Some(Color::Rgb(200, 100, 0))));
    assert_eq!(t.canvas, Color::Rgb(10, 20, 30));
    assert_eq!(t.surface, Color::Rgb(10, 20, 30));
    assert_eq!(t.surface_raised, Color::Rgb(110, 120, 130));
    assert_eq!(t.overlay, Color::Rgb(115, 118, 122));
    assert_eq!(t.border_idle, Color::Rgb(4, 5, 6));
    assert_eq!(t.border_focus, Color::Rgb(1, 2, 3));
    assert_eq!(t.text_primary, Color::Rgb(200, 100, 0));
    assert_eq!(t.text_muted, Color::Rgb(114, 64, 13));
    assert_eq!(t.accent, Color::Rgb(11, 12, 13));
    assert_eq!(t.accent_on, Color::Rgb(14, 15, 16));
    assert_eq!(t.success, Color::Rgb(74, 222, 128));
    assert_eq!(t.warning, Color::Rgb(250, 204, 21));
    assert_eq!(t.danger, Color::Rgb(248, 113, 113));
    assert_eq!(t.scrim, Color::Rgb(4, 9, 13));

    // Without text_fg the title colour is the text source.
    let t2 = DesignTokens::from_palette(&palette(None));
    assert_eq!(t2.text_primary, Color::Rgb(7, 8, 9));
}

#[test]
fn tokens_get_maps_every_role() {
    let t = DesignTokens::from_palette(&palette(Some(Color::Rgb(200, 100, 0))));
    let pairs = [
        (ColorRole::Canvas, t.canvas), (ColorRole::Surface, t.surface), (ColorRole::SurfaceRaised, t.surface_raised),
        (ColorRole::Overlay, t.overlay), (ColorRole::BorderIdle, t.border_idle), (ColorRole::BorderFocus, t.border_focus),
        (ColorRole::TextPrimary, t.text_primary), (ColorRole::TextMuted, t.text_muted), (ColorRole::Accent, t.accent),
        (ColorRole::AccentOn, t.accent_on), (ColorRole::Success, t.success), (ColorRole::Warning, t.warning),
        (ColorRole::Danger, t.danger), (ColorRole::Scrim, t.scrim),
    ];
    for (role, want) in pairs {
        assert_eq!(t.get(role), want, "{role:?}");
    }
    assert_ne!(t.get(ColorRole::Accent), t.get(ColorRole::Canvas));
    assert_ne!(t.get(ColorRole::BorderFocus), t.get(ColorRole::BorderIdle));
}

#[test]
fn appearance_resolve_wires_profile_tokens_materials() {
    let pal = palette(None);
    // Legacy default
    let cfg = BiltermConfig::default();
    let a = Appearance::resolve(&cfg, &pal);
    assert_eq!(a.profile, AppearanceProfile::Legacy);
    assert_eq!(a.materials, Materials::opaque_fallback());
    assert_eq!(a.tokens, DesignTokens::from_palette(&pal));
    assert_eq!(a.density, Density::Comfortable);
    assert_eq!(a.display_mode, DisplayMode::Standard);

    // Obsidian via modern_mode with density/display overrides
    let mut cfg = BiltermConfig::default();
    cfg.modern_mode = true;
    cfg.appearance.density = Density::Spacious;
    cfg.appearance.display_mode = DisplayMode::Zen;
    cfg.appearance.opacity_pct = Some(50);
    let a = Appearance::resolve(&cfg, &pal);
    assert_eq!(a.profile, AppearanceProfile::Obsidian);
    assert_ne!(a.materials, Materials::opaque_fallback(), "Legacy vs Obsidian differ");
    assert_eq!(a.materials.get(MaterialKind::Pane).opacity_pct, 50);
    assert_eq!(a.materials.get(MaterialKind::Pane).radius_px, 6);
    assert_eq!(a.density, Density::Spacious);
    assert_eq!(a.display_mode, DisplayMode::Zen);

    // Explicit profile beats legacy flags
    cfg.liquid_glass = true;
    cfg.appearance.profile = Some(AppearanceProfile::Arctic);
    assert_eq!(Appearance::resolve(&cfg, &pal).profile, AppearanceProfile::Arctic);
}
