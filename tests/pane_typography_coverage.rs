//! Coverage for `pane::typography`: resolution names, px-per-cell, font presets and `:font` commands.

use bilterm::pane::typography::{CanvasResolution, FontFamilyPreset, PaneFontOptions};

fn run(o: &mut PaneFontOptions, cmd: &str) -> Result<String, String> {
    o.execute_command_with_defaults(12, 100, cmd)
}

#[test]
fn resolution_names_parse_with_aliases_case_and_whitespace() {
    use CanvasResolution::*;
    for (n, want) in [
        ("auto", Auto), ("Native", Auto), (" HiDPI ", Auto), ("1x", Scale1x), ("SD", Scale1x),
        ("standard", Scale1x), ("2x", Scale2x), ("retina", Scale2x), ("4x", Scale4x), ("ultra", Scale4x),
        ("hd", Hd720p), ("720p", Hd720p), ("fhd", FullHd1080p), ("1080p", FullHd1080p),
        ("fullhd", FullHd1080p), ("4k", UltraHd4k), ("2160p", UltraHd4k), ("uhd", UltraHd4k),
    ] {
        assert_eq!(CanvasResolution::from_name(n), Some(want), "{n}");
    }
}

#[test]
fn custom_resolution_is_clamped_and_rejects_garbage() {
    assert_eq!(
        CanvasResolution::from_name("1024x768"),
        Some(CanvasResolution::Custom { width: 1024, height: 768 })
    );
    assert_eq!(
        CanvasResolution::from_name("99999x99999"),
        Some(CanvasResolution::Custom { width: 7680, height: 4320 })
    );
    for bad in ["15x100", "100x15", "axb", "10", "", "1x2x3", "-5x100"] {
        assert_eq!(CanvasResolution::from_name(bad), None, "{bad}");
    }
}

#[test]
fn fixed_resolutions_resolve_px_per_cell() {
    assert_eq!(CanvasResolution::Scale1x.resolve_px_per_cell(80, 24), (8, 16));
    assert_eq!(CanvasResolution::Scale2x.resolve_px_per_cell(80, 24), (16, 32));
    assert_eq!(CanvasResolution::Scale4x.resolve_px_per_cell(80, 24), (32, 64));
    assert_eq!(CanvasResolution::Hd720p.resolve_px_per_cell(80, 24), (16, 30));
    assert_eq!(CanvasResolution::FullHd1080p.resolve_px_per_cell(80, 24), (24, 45));
    assert_eq!(CanvasResolution::UltraHd4k.resolve_px_per_cell(80, 24), (48, 90));
    assert_eq!(
        CanvasResolution::Custom { width: 800, height: 480 }.resolve_px_per_cell(80, 24),
        (10, 20)
    );
}

#[test]
fn resolution_floors_apply_and_zero_grid_does_not_panic() {
    assert_eq!(CanvasResolution::Hd720p.resolve_px_per_cell(1000, 1000), (4, 8));
    assert_eq!(
        CanvasResolution::Custom { width: 16, height: 16 }.resolve_px_per_cell(1000, 1000),
        (2, 4)
    );
    assert_eq!(CanvasResolution::Scale1x.resolve_px_per_cell(0, 0), (8, 16));
    let (w, h) = CanvasResolution::Auto.resolve_px_per_cell(80, 24);
    assert!((8..=64).contains(&w) && (16..=128).contains(&h), "{w}x{h}");
    assert_eq!(CanvasResolution::Hd720p.resolve_px_per_cell(0, 0), (1280, 720));
}

#[test]
fn resolution_labels_are_distinct_and_descriptive() {
    use CanvasResolution::*;
    let all = [
        Auto, Scale1x, Scale2x, Scale4x, Hd720p, FullHd1080p, UltraHd4k,
        Custom { width: 5, height: 6 },
    ];
    let labels: Vec<String> = all.iter().map(|r| r.label()).collect();
    for (i, a) in labels.iter().enumerate() {
        for b in &labels[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(Custom { width: 5, height: 6 }.label(), "Custom (5x6)");
}

#[test]
fn every_preset_has_name_badge_and_candidates_and_name_round_trips() {
    let presets = [
        FontFamilyPreset::Ibm3270, FontFamilyPreset::BorlandDosVga, FontFamilyPreset::Tektronix4014,
        FontFamilyPreset::JetBrainsMono, FontFamilyPreset::FiraCode, FontFamilyPreset::IosevkaTerm,
        FontFamilyPreset::CascadiaCode, FontFamilyPreset::Consolas, FontFamilyPreset::IbmPlexMono,
        FontFamilyPreset::Hack, FontFamilyPreset::LucidaConsole, FontFamilyPreset::CourierNew,
    ];
    for p in &presets {
        assert!(!p.display_name().is_empty());
        assert!(!p.short_badge().is_empty());
        assert!(!p.candidate_os_font_faces().is_empty(), "{p:?}");
        assert_eq!(FontFamilyPreset::from_name(p.display_name()).as_ref(), Some(p), "{p:?}");
    }
    let c = FontFamilyPreset::Custom("My Font".into());
    assert_eq!(c.display_name(), "My Font");
    assert_eq!(c.short_badge(), "My Font");
    assert_eq!(FontFamilyPreset::from_name("nonsense"), None);
}

#[test]
fn next_preset_cycle_visits_every_builtin_then_wraps() {
    let mut p = FontFamilyPreset::CascadiaCode;
    let mut seen = vec![p.clone()];
    for _ in 0..10 {
        p = p.next_preset();
        seen.push(p.clone());
    }
    assert_eq!(seen.last(), Some(&FontFamilyPreset::CourierNew));
    assert_eq!(p.next_preset(), FontFamilyPreset::CascadiaCode);
    seen.sort_by_key(|s| s.display_name().to_string());
    seen.dedup();
    assert_eq!(seen.len(), 11);
    assert_eq!(FontFamilyPreset::Tektronix4014.next_preset(), FontFamilyPreset::CascadiaCode);
    assert_eq!(
        FontFamilyPreset::Custom("x".into()).next_preset(),
        FontFamilyPreset::CascadiaCode
    );
}

#[test]
fn effective_size_and_scale_apply_defaults_and_clamps() {
    let mut o = PaneFontOptions::default();
    assert!(!o.has_custom_overrides());
    assert_eq!(o.effective_size_pt_with_defaults(12, 100), 12);
    assert_eq!(o.effective_size_pt_with_defaults(12, 150), 18);
    o.size_pt = Some(1000);
    o.scale_pct = Some(1000);
    assert_eq!(o.effective_scale_pct_with_default(100), 400);
    assert_eq!(o.effective_size_pt_with_defaults(12, 100), 96);
    o.size_pt = Some(1);
    o.scale_pct = Some(1);
    assert_eq!(o.effective_scale_pct_with_default(100), 50);
    assert_eq!(o.effective_size_pt_with_defaults(12, 100), 6);
    assert!(o.has_custom_overrides());
}

#[test]
fn every_override_field_counts_as_custom() {
    let base = PaneFontOptions::default();
    for o in [
        PaneFontOptions { family: FontFamilyPreset::Hack, ..base.clone() },
        PaneFontOptions { size_pt: Some(10), ..base.clone() },
        PaneFontOptions { scale_pct: Some(100), ..base.clone() },
        PaneFontOptions { bold: true, ..base.clone() },
        PaneFontOptions { italic: true, ..base.clone() },
        PaneFontOptions { double_width: true, ..base.clone() },
    ] {
        assert!(o.has_custom_overrides(), "{o:?}");
    }
}

#[test]
fn status_and_list_and_empty_command() {
    let mut o = PaneFontOptions::default();
    let s = run(&mut o, "").unwrap();
    assert!(s.starts_with("PANE FONT: family='IBM 3270' size=12pt (100%)"), "{s}");
    assert_eq!(run(&mut o, "status").unwrap(), s);
    assert!(run(&mut o, "list").unwrap().starts_with("FONT PRESETS:"));
    assert!(run(&mut o, "presets").unwrap().contains("cascadia"));
}

#[test]
fn cycle_reset_and_scale_steps() {
    let mut o = PaneFontOptions::default();
    run(&mut o, "cycle").unwrap();
    assert_eq!(o.family, FontFamilyPreset::BorlandDosVga);
    run(&mut o, "next").unwrap();
    assert_eq!(o.family, FontFamilyPreset::LucidaConsole);
    run(&mut o, "+").unwrap();
    assert_eq!(o.scale_pct, Some(110));
    run(&mut o, "bigger").unwrap();
    assert_eq!(o.scale_pct, Some(120));
    run(&mut o, "-").unwrap();
    run(&mut o, "smaller").unwrap();
    assert_eq!(o.scale_pct, Some(100));
    for _ in 0..20 { run(&mut o, "down").unwrap(); }
    assert_eq!(o.scale_pct, Some(50));
    for _ in 0..50 { run(&mut o, "in").unwrap(); }
    assert_eq!(o.scale_pct, Some(400));
    assert!(run(&mut o, "reset").unwrap().starts_with("PANE FONT RESET"));
    assert_eq!(o, PaneFontOptions::default());
    run(&mut o, "hack").unwrap();
    run(&mut o, "default").unwrap();
    assert_eq!(o, PaneFontOptions::default());
}

#[test]
fn bold_italic_double_take_on_off_and_toggle() {
    let mut o = PaneFontOptions::default();
    for (cmd, get) in [
        ("bold", (|o: &PaneFontOptions| o.bold) as fn(&PaneFontOptions) -> bool),
        ("italic", |o| o.italic),
        ("double", |o| o.double_width),
        ("wide", |o| o.double_width),
        ("2x", |o| o.double_width),
    ] {
        let before = get(&o);
        run(&mut o, cmd).unwrap();
        assert_eq!(get(&o), !before, "{cmd} toggles");
        run(&mut o, &format!("{cmd} on")).unwrap();
        assert!(get(&o), "{cmd} on");
        run(&mut o, &format!("{cmd} off")).unwrap();
        assert!(!get(&o), "{cmd} off");
        run(&mut o, &format!("{cmd} true")).unwrap();
        assert!(get(&o));
        run(&mut o, &format!("{cmd} 0")).unwrap();
        assert!(!get(&o));
        run(&mut o, &format!("{cmd} 1")).unwrap();
        assert!(get(&o));
        run(&mut o, &format!("{cmd} false")).unwrap();
        assert!(!get(&o));
    }
}

#[test]
fn size_and_scale_commands_clamp_and_report_usage() {
    let mut o = PaneFontOptions::default();
    run(&mut o, "size 14pt").unwrap();
    assert_eq!(o.size_pt, Some(14));
    run(&mut o, "size 500").unwrap();
    assert_eq!(o.size_pt, Some(72));
    run(&mut o, "size 1").unwrap();
    assert_eq!(o.size_pt, Some(6));
    assert!(run(&mut o, "size").unwrap_err().contains("Usage: :font size"));
    assert!(run(&mut o, "size abc").unwrap_err().contains("Usage"));
    run(&mut o, "scale 150%").unwrap();
    assert_eq!(o.scale_pct, Some(150));
    run(&mut o, "scale 9999").unwrap();
    assert_eq!(o.scale_pct, Some(400));
    run(&mut o, "scale 1").unwrap();
    assert_eq!(o.scale_pct, Some(50));
    assert!(run(&mut o, "scale").unwrap_err().contains("Usage: :font scale"));
}

#[test]
fn custom_family_and_add_usage() {
    let mut o = PaneFontOptions::default();
    run(&mut o, "custom \"Comic Mono\"").unwrap();
    assert_eq!(o.family, FontFamilyPreset::Custom("Comic Mono".into()));
    run(&mut o, "family 'Fira Code'").unwrap();
    assert_eq!(o.family, FontFamilyPreset::FiraCode);
    assert!(run(&mut o, "custom").unwrap_err().contains("Usage: :font custom"));
    assert!(run(&mut o, "add").unwrap_err().contains("Usage: :font add"));
    assert!(run(&mut o, "load \"\"").unwrap_err().contains("Usage: :font add"));
    assert!(run(&mut o, "add /definitely/not/a/font.ttf").is_err());
}

#[test]
fn bare_preset_with_optional_size_or_scale_argument() {
    let mut o = PaneFontOptions::default();
    run(&mut o, "jetbrains").unwrap();
    assert_eq!(o.family, FontFamilyPreset::JetBrainsMono);
    run(&mut o, "hack 14pt").unwrap();
    assert_eq!((o.family.clone(), o.size_pt), (FontFamilyPreset::Hack, Some(14)));
    run(&mut o, "consolas 130%").unwrap();
    assert_eq!((o.family.clone(), o.scale_pct), (FontFamilyPreset::Consolas, Some(130)));
    run(&mut o, "plex 20").unwrap();
    assert_eq!(o.size_pt, Some(20));
    run(&mut o, "plex 200").unwrap();
    assert_eq!(o.scale_pct, Some(200));
    let before = o.clone();
    run(&mut o, "hack nonsense").unwrap();
    assert_eq!((o.size_pt, o.scale_pct), (before.size_pt, before.scale_pct));
}

#[test]
fn bare_numeric_forms_and_multiword_family() {
    let mut o = PaneFontOptions::default();
    run(&mut o, "16pt").unwrap();
    assert_eq!(o.size_pt, Some(16));
    run(&mut o, "120%").unwrap();
    assert_eq!(o.scale_pct, Some(120));
    run(&mut o, "30").unwrap();
    assert_eq!(o.size_pt, Some(30));
    run(&mut o, "300").unwrap();
    assert_eq!(o.scale_pct, Some(300));
    run(&mut o, "3").unwrap();
    assert_eq!(o.size_pt, Some(6));
    run(&mut o, "courier new").unwrap();
    assert_eq!(o.family, FontFamilyPreset::CourierNew);
}

#[test]
fn unknown_option_reports_usage() {
    let mut o = PaneFontOptions::default();
    let e = run(&mut o, "frobnicate").unwrap_err();
    assert!(e.contains("Unknown :font option 'frobnicate'"), "{e}");
    assert_eq!(o, PaneFontOptions::default());
}
