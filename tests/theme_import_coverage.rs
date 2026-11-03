//! Coverage for `ui::theme_import`: format detection, per-format parsing, export round-trip, file loading.

use std::path::PathBuf;

use ratatui::style::Color;

use bilterm::ui::theme::ThemePreset;
use bilterm::ui::theme_import::{
    OpenColorScheme, build_theme_catalog_lines, contrast_ratio, discover_theme_files_in_dir,
    discover_themes_in_dir, export_open_theme_toml, format_hex_color, load_open_theme_file,
    luminance, parse_itermcolors_xml, parse_open_theme_str, resolve_theme_file_or_name,
};

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bilterm-themeimport-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn plist_color(key: &str, r: &str, g: &str, b: &str) -> String {
    format!(
        "<key>{key}</key><dict><key>Red Component</key><real>{r}</real>\
         <key>Green Component</key><real>{g}</real><key>Blue Component</key><real>{b}</real></dict>"
    )
}

fn plist(body: &str) -> String {
    format!("<?xml version=\"1.0\"?><plist version=\"1.0\"><dict>{body}</dict></plist>")
}

#[test]
fn hex_formatting_covers_named_and_indexed_colors() {
    assert_eq!(format_hex_color(Color::Rgb(1, 2, 255)), "#0102FF");
    assert_eq!(format_hex_color(Color::Black), "#000000");
    assert_eq!(format_hex_color(Color::White), "#FFFFFF");
    assert_eq!(format_hex_color(Color::Indexed(7)), "#070707");
    for c in [
        Color::Red, Color::Green, Color::Yellow, Color::Blue, Color::Magenta, Color::Cyan,
        Color::Gray, Color::DarkGray, Color::LightRed, Color::LightGreen, Color::LightYellow,
        Color::LightBlue, Color::LightMagenta, Color::LightCyan, Color::Reset,
    ] {
        let h = format_hex_color(c);
        assert_eq!(h.len(), 7, "{c:?} -> {h}");
        assert!(h.starts_with('#'));
    }
}

#[test]
fn luminance_and_contrast_follow_wcag() {
    assert!((luminance(Color::Rgb(0, 0, 0))).abs() < 1e-6);
    assert!((luminance(Color::Rgb(255, 255, 255)) - 1.0).abs() < 1e-3);
    let r = contrast_ratio(Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
    assert!((r - 21.0).abs() < 0.05, "{r}");
    assert!((contrast_ratio(Color::Rgb(9, 9, 9), Color::Rgb(9, 9, 9)) - 1.0).abs() < 1e-6);
    let a = contrast_ratio(Color::Rgb(10, 10, 10), Color::Rgb(200, 200, 200));
    let b = contrast_ratio(Color::Rgb(200, 200, 200), Color::Rgb(10, 10, 10));
    assert!((a - b).abs() < 1e-6, "symmetric");
    // Low-channel branch (<= 0.03928) vs high-channel branch.
    assert!(luminance(Color::Rgb(5, 5, 5)) < luminance(Color::Rgb(50, 50, 50)));
}

#[test]
fn itermcolors_parses_real_integer_and_clamped_components() {
    let mut body = String::new();
    for i in 0..16 {
        body += &plist_color(&format!("Ansi {i} Color"), "0.5", "0.0", "1.0");
    }
    body += &plist_color("Foreground Color", "1.0", "1.0", "1.0");
    body += &plist_color("Background Color", "0.0", "0.0", "0.0");
    body += &plist_color("Bold Color", "0.1", "0.2", "0.3");
    body += &plist_color("Cursor Color", "1.0", "0.0", "0.0");
    body += &plist_color("Cursor Text Color", "0.0", "1.0", "0.0");
    body += &plist_color("Selection Color", "0.0", "0.0", "1.0");
    body += &plist_color("Selected Text Color", "1.0", "1.0", "0.0");
    body += &plist_color("Badge Color", "0.2", "0.2", "0.2");
    body += &plist_color("Link Color", "0.3", "0.3", "0.9");
    let s = parse_itermcolors_xml("full", &plist(&body)).unwrap();
    assert_eq!(s.name, "full");
    assert_eq!(s.ansi[0], Color::Rgb(128, 0, 255));
    assert_eq!(s.ansi[15], Color::Rgb(128, 0, 255));
    assert_eq!(s.foreground, Color::Rgb(255, 255, 255));
    assert_eq!(s.background, Color::Rgb(0, 0, 0));
    assert_eq!(s.bold, Some(Color::Rgb(26, 51, 77)));
    assert_eq!(s.cursor, Color::Rgb(255, 0, 0));
    assert_eq!(s.cursor_text, Color::Rgb(0, 255, 0));
    assert_eq!(s.selection_bg, Color::Rgb(0, 0, 255));
    assert_eq!(s.selection_fg, Color::Rgb(255, 255, 0));
    assert!(s.badge.is_some() && s.link.is_some());

    let xml = plist(
        "<key>Foreground Color</key><dict><key>Red Component</key><integer>300</integer>\
         <key>Green Component</key><integer>20</integer><key>Blue Component</key><real>-1</real></dict>",
    );
    let s = parse_itermcolors_xml("ints", &xml).unwrap();
    assert_eq!(s.foreground, Color::Rgb(255, 20, 0));
}

#[test]
fn itermcolors_rejects_non_plist_and_empty_and_incomplete_entries() {
    assert!(parse_itermcolors_xml("x", "hello").unwrap_err().contains("Not a valid"));
    assert!(parse_itermcolors_xml("x", &plist("")).unwrap_err().contains("No iTerm2 color"));
    // A color dict missing a component is skipped, leaving nothing found.
    let broken = plist(
        "<key>Foreground Color</key><dict><key>Red Component</key><real>1</real></dict>",
    );
    assert!(parse_itermcolors_xml("x", &broken).is_err());
    // Non-numeric component is skipped too.
    let nan = plist(
        "<key>Foreground Color</key><dict><key>Red Component</key><real>abc</real>\
         <key>Green Component</key><real>0</real><key>Blue Component</key><real>0</real></dict>",
    );
    assert!(parse_itermcolors_xml("x", &nan).is_err());
    // Component value in an unsupported tag.
    let tag = plist(
        "<key>Foreground Color</key><dict><key>Red Component</key><string>1</string>\
         <key>Green Component</key><real>0</real><key>Blue Component</key><real>0</real></dict>",
    );
    assert!(parse_itermcolors_xml("x", &tag).is_err());
}

#[test]
fn auto_detection_routes_xml_toml_yaml_and_rejects_unknown() {
    let xml = plist(&plist_color("Background Color", "1", "1", "1"));
    assert_eq!(parse_open_theme_str("n", &xml).unwrap().background, Color::Rgb(255, 255, 255));
    let e = parse_open_theme_str("n", "just some words").unwrap_err();
    assert!(e.contains("Unrecognized theme format"), "{e}");
    assert!(parse_open_theme_str("n", "").is_err());
    assert!(parse_open_theme_str("n", "   \n  ").is_err());
}

#[test]
fn alacritty_toml_sections_are_applied() {
    let t = r##"
[colors.primary]
background = "#101010"
foreground = "0xEEEEEE"
[colors.cursor]
cursor = "#FF0000"
text = "#00FF00"
[colors.selection]
background = "#0000FF"
foreground = "#FFFF00"
[colors.normal]
black = "#000001"
red = "#000002"
white = "#000008"
[colors.bright]
black = "#000011"
white = "#000018"
"##;
    let s = parse_open_theme_str("ala", t).unwrap();
    assert_eq!(s.name, "ala");
    assert_eq!(s.background, Color::Rgb(16, 16, 16));
    assert_eq!(s.foreground, Color::Rgb(0xEE, 0xEE, 0xEE));
    assert_eq!(s.cursor, Color::Rgb(255, 0, 0));
    assert_eq!(s.cursor_text, Color::Rgb(0, 255, 0));
    assert_eq!(s.selection_bg, Color::Rgb(0, 0, 255));
    assert_eq!(s.selection_fg, Color::Rgb(255, 255, 0));
    assert_eq!(s.ansi[0], Color::Rgb(0, 0, 1));
    assert_eq!(s.ansi[1], Color::Rgb(0, 0, 2));
    assert_eq!(s.ansi[7], Color::Rgb(0, 0, 8));
    assert_eq!(s.ansi[8], Color::Rgb(0, 0, 0x11));
    assert_eq!(s.ansi[15], Color::Rgb(0, 0, 0x18));
}

#[test]
fn alacritty_selection_text_key_is_preferred_and_bad_hex_is_ignored() {
    let t = "[colors.selection]\ntext = \"#010203\"\nforeground = \"#FFFFFF\"\n[colors.primary]\nbackground = \"nothex\"\nforeground = \"#12345\"\n";
    let s = parse_open_theme_str("n", t).unwrap();
    assert_eq!(s.selection_fg, Color::Rgb(1, 2, 3));
    let d = OpenColorScheme::default();
    assert_eq!(s.background, d.background);
    assert_eq!(s.foreground, d.foreground);
}

#[test]
fn open_toml_with_ansi_arrays_names_and_typography() {
    let t = r##"
name = "Mine"
foreground = "#C0C0C0"
background = "#000010"
bold = "#FFFFFF"
cursor_bg = "#111111"
cursor_fg = "#222222"
selection_bg = "#333333"
selection_fg = "#444444"
badge = "#555555"
link = "#666666"
ansi = ["#000000", "#010101", "bad", "#030303"]
brights = ["#808080", "#818181"]
[typography]
font_family = "JetBrains Mono"
font_size_pt = 13
font_scale_pct = 120
"##;
    let s = parse_open_theme_str("fallback", t).unwrap();
    assert_eq!(s.name, "Mine");
    assert_eq!(s.bold, Some(Color::Rgb(255, 255, 255)));
    assert_eq!(s.cursor, Color::Rgb(0x11, 0x11, 0x11));
    assert_eq!(s.cursor_text, Color::Rgb(0x22, 0x22, 0x22));
    assert_eq!(s.selection_bg, Color::Rgb(0x33, 0x33, 0x33));
    assert_eq!(s.selection_fg, Color::Rgb(0x44, 0x44, 0x44));
    assert_eq!(s.badge, Some(Color::Rgb(0x55, 0x55, 0x55)));
    assert_eq!(s.link, Some(Color::Rgb(0x66, 0x66, 0x66)));
    assert_eq!(s.ansi[1], Color::Rgb(1, 1, 1));
    assert_eq!(s.ansi[2], OpenColorScheme::default().ansi[2], "bad entry keeps default");
    assert_eq!(s.ansi[3], Color::Rgb(3, 3, 3));
    assert_eq!(s.ansi[8], Color::Rgb(0x80, 0x80, 0x80));
    assert_eq!(s.ansi[9], Color::Rgb(0x81, 0x81, 0x81));
    assert_eq!(s.typography.font_family.as_deref(), Some("JetBrains Mono"));
    assert_eq!(s.typography.font_size_pt, Some(13));
    assert_eq!(s.typography.font_scale_pct, Some(120));
}

#[test]
fn open_toml_alternate_key_names_and_scheme_name_and_bad_typography() {
    let t = "scheme = \"Alt\"\ncursor = \"#010101\"\ncursor_text = \"#020202\"\n[typography]\nfont_size_pt = -4\nfont_scale_pct = 99999\n";
    let s = parse_open_theme_str("fb", t).unwrap();
    assert_eq!(s.name, "Alt");
    assert_eq!(s.cursor, Color::Rgb(1, 1, 1));
    assert_eq!(s.cursor_text, Color::Rgb(2, 2, 2));
    assert_eq!(s.typography.font_size_pt, None, "negative does not fit u16");
    assert_eq!(s.typography.font_scale_pct, None, "overflow does not fit u16");
    assert_eq!(s.typography.font_family, None);
}

#[test]
fn base16_toml_and_yaml_map_slots() {
    let toml = "scheme = \"B16\"\nbase00 = \"#181818\"\nbase01=\"#282828\"\nbase02 = \"#383838\"\nbase03=\"#585858\"\nbase04=\"#B8B8B8\"\nbase05 = \"#D8D8D8\"\nbase06 = \"#E8E8E8\"\nbase07=\"#F8F8F8\"\nbase08=\"#AB4642\"\nbase09=\"#DC9656\"\nbase0A=\"#F7CA88\"\nbase0B=\"#A1B56C\"\nbase0C=\"#86C1B9\"\nbase0D=\"#7CAFC2\"\nbase0E=\"#BA8BAF\"\nbase0F=\"#A16946\"\n";
    let s = parse_open_theme_str("x", toml).unwrap();
    assert_eq!(s.name, "B16");
    assert_eq!(s.background, Color::Rgb(0x18, 0x18, 0x18));
    assert_eq!(s.foreground, Color::Rgb(0xD8, 0xD8, 0xD8));
    assert_eq!(s.cursor, Color::Rgb(0xE8, 0xE8, 0xE8));
    assert_eq!(s.cursor_text, s.background);
    assert_eq!(s.selection_bg, Color::Rgb(0x38, 0x38, 0x38));
    assert_eq!(s.ansi[1], Color::Rgb(0xAB, 0x46, 0x42));
    assert_eq!(s.ansi[2], Color::Rgb(0xA1, 0xB5, 0x6C));
    assert_eq!(s.ansi[15], Color::Rgb(0xF8, 0xF8, 0xF8));

    let yaml = "scheme: \"Tomorrow\"   # a comment\nauthor: someone\nbase00: \"1d1f21\"\nbase05: \"c5c8c6\"\nbase08: \"cc6666\"\nbase0A: \"f0c674\"\n";
    let y = parse_open_theme_str("file", yaml).unwrap();
    assert_eq!(y.name, "Tomorrow");
    assert_eq!(y.background, Color::Rgb(0x1d, 0x1f, 0x21));
    assert_eq!(y.ansi[1], Color::Rgb(0xcc, 0x66, 0x66));
    assert_eq!(y.ansi[3], Color::Rgb(0xf0, 0xc6, 0x74), "uppercase key base0A resolves");
}

#[test]
fn base16_missing_required_slots_error() {
    let e = parse_open_theme_str("x", "scheme: s\nbase05: \"c5c8c6\"\n").unwrap_err();
    assert!(e.contains("missing base00"), "{e}");
    let e = parse_open_theme_str("x", "scheme: s\nbase00: \"1d1f21\"\n").unwrap_err();
    assert!(e.contains("missing base05"), "{e}");
    // yaml fallback name
    let y = parse_open_theme_str("stem", "palette: x\nbase00: 000000\nbase05: ffffff\n").unwrap();
    assert_eq!(y.name, "stem");
}

#[test]
fn export_then_parse_round_trips_including_optionals() {
    let mut ansi = OpenColorScheme::default().ansi;
    ansi[5] = Color::Rgb(10, 20, 30);
    ansi[12] = Color::Rgb(40, 50, 60);
    let s = OpenColorScheme {
        name: "RT \"quoted\"".into(),
        bold: Some(Color::Rgb(1, 2, 3)),
        badge: Some(Color::Rgb(4, 5, 6)),
        link: Some(Color::Rgb(7, 8, 9)),
        ansi,
        typography: bilterm::ui::ThemeTypographyProfile {
            font_family: Some("Hack".into()),
            font_size_pt: Some(11),
            font_scale_pct: Some(130),
        },
        ..OpenColorScheme::default()
    };
    let text = export_open_theme_toml(&s);
    assert!(text.contains("[typography]"));
    let back = parse_open_theme_str("zzz", &text).unwrap();
    assert_eq!(back, s);

    let plain = export_open_theme_toml(&OpenColorScheme::default());
    assert!(!plain.contains("[typography]") && !plain.contains("bold ="));
    assert!(!plain.contains("badge") && !plain.contains("link"));
    assert_eq!(parse_open_theme_str("d", &plain).unwrap().ansi, OpenColorScheme::default().ansi);
}

#[test]
fn from_preset_and_palette_conversion_are_consistent_for_every_preset() {
    for p in ThemePreset::all() {
        let s = OpenColorScheme::from_preset(*p);
        let pal = s.to_theme_palette();
        let _ = pal;
        assert!(!s.name.is_empty(), "{p:?}");
        // Exported preset must itself re-parse.
        parse_open_theme_str("p", &export_open_theme_toml(&s)).unwrap();
    }
}

#[test]
fn file_loading_uses_stem_as_name_and_reports_read_errors() {
    let d = scratch("load");
    let f = d.join("my-theme.toml");
    std::fs::write(&f, "foreground = \"#FFFFFF\"\n").unwrap();
    assert_eq!(load_open_theme_file(&f).unwrap().name, "my-theme");
    let e = load_open_theme_file(&d.join("missing.toml")).unwrap_err();
    assert!(e.contains("Failed to read theme file"), "{e}");
    let bad = d.join("bad.toml");
    std::fs::write(&bad, "nonsense").unwrap();
    assert!(load_open_theme_file(&bad).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn directory_discovery_skips_unparseable_files() {
    let d = scratch("discover");
    std::fs::write(d.join("good.toml"), "foreground = \"#FFFFFF\"\nname = \"Good\"\n").unwrap();
    std::fs::write(d.join("broken.toml"), "this is not a theme\n").unwrap();
    std::fs::write(d.join("note.txt"), "ignored").unwrap();
    let files = discover_theme_files_in_dir(&d);
    assert!(files.iter().any(|(_, s)| s.name == "Good"), "{files:?}");
    assert!(files.iter().all(|(p, _)| p.file_name().unwrap() != "note.txt"));
    assert_eq!(discover_themes_in_dir(&d).len(), files.len());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn resolve_by_explicit_path_and_empty_and_missing_queries() {
    let d = scratch("resolve");
    let f = d.join("pick-me.toml");
    std::fs::write(&f, "name = \"Picked\"\nforeground = \"#ABCDEF\"\n").unwrap();
    let (p, s) = resolve_theme_file_or_name(&format!("  \"{}\" ", f.display())).unwrap();
    assert_eq!(p, f);
    assert_eq!(s.name, "Picked");
    assert!(resolve_theme_file_or_name("").is_none());
    assert!(resolve_theme_file_or_name("   ").is_none());
    assert!(resolve_theme_file_or_name("definitely-not-a-theme-zzz-9f3a").is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn catalog_lists_presets_marks_active_and_lists_extra_dir_imports() {
    let d = scratch("catalog");
    std::fs::write(d.join("extra.toml"), "name = \"ExtraScheme\"\nforeground = \"#ABCDEF\"\n").unwrap();
    let active = ThemePreset::all()[0];
    let lines = build_theme_catalog_lines(active, false, Some(&d));
    assert_eq!(
        lines.iter().filter(|l| l.contains("[ACTIVE]")).count(),
        1,
        "{lines:#?}"
    );
    assert!(lines.iter().any(|l| l.contains(":browse")));
    assert!(lines.iter().any(|l| l.contains("ExtraScheme") && l.contains("#ABCDEF")));
    assert!(lines.iter().any(|l| l.contains("DISCOVERED")));
    assert!(lines.last().unwrap().contains(":theme import"));
    let overridden = build_theme_catalog_lines(active, true, Some(&d));
    assert!(overridden.iter().all(|l| !l.contains("[ACTIVE]")));
    let _ = std::fs::remove_dir_all(&d);
}
