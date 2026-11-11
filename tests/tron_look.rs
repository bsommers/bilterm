//! TRON look: `--tron`, `:tron on|off|toggle`, `:style tron`, the `F11` cycle, the `TRON` palette on a
//! rendered frame, contrast, and the config save/load round trip.

use std::sync::Once;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::config::BiltermConfig;
use bilterm::input::handle_wm_key_event;
use bilterm::ui::design::tron::{apply_cli_tron_switch, tron_active, tron_palette, TRON_STYLE_LABEL};
use bilterm::ui::design::{AppearanceProfile, DesignTokens};
use bilterm::ui::theme_import::contrast_ratio;
use bilterm::ui::visual_style::{apply_visual_style_mode, visual_style_label};
use bilterm::ui::{buffer_to_snapshot_string, WorkspaceWidget};
use bilterm::wm::{SplitDirection, WindowManager};

static ENV: Once = Once::new();

fn wm() -> WindowManager {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-tron-look-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: executed exactly once, before any test in this binary reads the environment.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
    WindowManager::new_headless(24, 100)
}

fn run(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).unwrap_or_else(|| panic!("{cmd} not parsed")).unwrap();
    execute_wm_colon_command(wm, action);
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

#[test]
fn cli_tron_switch_selects_the_tron_profile() {
    let mut cfg = BiltermConfig::default();
    let args: Vec<String> = ["--gui", "--tron"].iter().map(|s| s.to_string()).collect();
    assert!(apply_cli_tron_switch(&mut cfg, &args));
    assert_eq!(AppearanceProfile::resolve(&cfg), AppearanceProfile::Tron);
    assert_eq!(cfg.palette().name, "TRON");
    assert!(cfg.is_modern() && cfg.palette().is_modern_look());
    assert_eq!(visual_style_label(&cfg), TRON_STYLE_LABEL);

    let mut plain = BiltermConfig::default();
    assert!(!apply_cli_tron_switch(&mut plain, &["--tronx".to_string(), "tron".to_string()]));
    assert_eq!(plain, BiltermConfig::default());
}

#[test]
fn colon_tron_on_off_toggle_and_style_tron() {
    let mut w = wm();
    run(&mut w, ":tron on");
    assert!(tron_active(w.config()));
    assert!(w.active_pane().status_message().is_some_and(|m| m.contains("TRON")));
    run(&mut w, ":tron on");
    assert!(tron_active(w.config()));
    run(&mut w, ":tron off");
    assert_eq!(AppearanceProfile::resolve(w.config()), AppearanceProfile::Legacy);
    run(&mut w, ":tron");
    assert!(tron_active(w.config()));
    run(&mut w, ":tron toggle");
    assert!(!tron_active(w.config()));
    run(&mut w, ":style tron");
    assert!(tron_active(w.config()));
    run(&mut w, ":style classic");
    assert_eq!(AppearanceProfile::resolve(w.config()), AppearanceProfile::Legacy);
    run(&mut w, ":appearance tron");
    assert!(tron_active(w.config()));
    assert!(parse_colon_command(":tron sideways").unwrap().is_err());
}

#[test]
fn tron_off_restores_the_previous_look() {
    let mut w = wm();
    run(&mut w, ":liquid on");
    run(&mut w, ":tron on");
    assert!(!w.config().liquid_glass && !w.config().is_glass());
    run(&mut w, ":tron off");
    assert!(w.config().liquid_glass && w.config().modern_mode && w.config().glass_mode);
    assert_eq!(w.config().palette().name, "LIQUID-GLASS");

    run(&mut w, ":style classic");
    run(&mut w, ":appearance midnight");
    run(&mut w, ":tron");
    assert_eq!(w.config().palette().name, "TRON");
    run(&mut w, ":tron");
    assert_eq!(w.config().appearance.profile, Some(AppearanceProfile::Midnight));
}

#[test]
fn f11_cycle_order_ends_with_tron_then_wraps_to_classic() {
    let mut w = wm();
    let mut labels = vec![visual_style_label(w.config()).to_string()];
    for _ in 0..7 {
        let _ = handle_wm_key_event(&mut w, KeyEvent::new(KeyCode::F(11), KeyModifiers::NONE));
        labels.push(visual_style_label(w.config()).to_string());
    }
    let heads: Vec<&str> = labels.iter().map(|l| l.split(" (").next().unwrap()).collect();
    assert_eq!(
        heads,
        ["CLASSIC", "✨ SNAZZY", "💎 GLASS", "✨💎 SNAZZY + GLASS", "◢ MODERN", "💧 LIQUID GLASS", "◈ TRON", "CLASSIC"]
    );
    let c = w.config();
    assert!(!c.snazzy_mode && !c.glass_mode && !c.modern_mode && !c.liquid_glass);
    assert_eq!(c.appearance.profile, None);
    assert_eq!(c.palette().name, BiltermConfig::default().palette().name);

    // The cycle is a pure function of the config as well.
    let mut cfg = BiltermConfig::default();
    for _ in 0..6 {
        apply_visual_style_mode(&mut cfg, None);
    }
    assert!(tron_active(&cfg));
    assert_eq!(apply_visual_style_mode(&mut cfg, Some("cycle")), "CLASSIC (3270/Borland)");
    assert_eq!(cfg.appearance.profile, None);
}

#[test]
fn tron_frame_uses_black_canvas_cyan_active_border_and_dim_teal_idle_border() {
    let mut w = wm();
    let area = Rect::new(0, 0, 100, 24);
    w.split_focused(SplitDirection::Vertical).unwrap();
    run(&mut w, ":tron on");
    w.resize_all(area).unwrap();
    let buf = render(&w, area);
    let pal = tron_palette();
    assert_eq!(pal.window_frame_bg, Color::Rgb(0, 0, 0));
    assert_eq!(pal.active_border_fg, Color::Rgb(111, 195, 223));

    let (mut active, mut idle, mut rounded) = (0, 0, 0);
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(x, y)];
            let sym = cell.symbol();
            if matches!(sym, "│" | "─" | "╭" | "╮" | "╰" | "╯") {
                rounded += usize::from(matches!(sym, "╭" | "╮" | "╰" | "╯"));
                if cell.fg == pal.active_border_fg {
                    assert!(
                        cell.bg == pal.window_frame_bg || cell.bg == pal.prompt_surface_bg,
                        "active border bg at {x},{y}: {:?}",
                        cell.bg
                    );
                    active += 1;
                } else if cell.fg == pal.inactive_border_fg {
                    idle += 1;
                }
            }
            assert!(!matches!(sym, "╔" | "║" | "═"), "classic double-line chrome in TRON at {x},{y}");
        }
    }
    assert!(active > 20, "neon cyan active border cells: {active}");
    assert!(idle > 20, "dim teal inactive border cells: {idle}");
    assert!(rounded >= 4, "rounded light-cycle corners: {rounded}");
    let black = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|p| buf[*p].bg == pal.window_frame_bg)
        .count();
    assert!(black > usize::from(area.width * area.height) / 2, "black canvas cells: {black}");
    let text = buffer_to_snapshot_string(&buf);
    if std::env::var_os("TRON_DUMP").is_some() {
        eprintln!("{text}");
    }
    assert!(text.contains("◈ Bilterm"), "TRON OIA badge missing:\n{text}");
    assert!(!text.contains("[□][X]"), "classic window controls in TRON:\n{text}");
}

#[test]
fn tron_palette_contrast_and_tokens() {
    let t = AppearanceProfile::Tron.tokens();
    assert_eq!(t, DesignTokens::from_palette(&tron_palette()));
    assert!(contrast_ratio(t.text_primary, t.canvas) >= 7.0);
    assert!(contrast_ratio(t.text_primary, t.surface_raised) >= 7.0);
    assert!(contrast_ratio(t.border_focus, t.canvas) >= 4.5);
    assert!(contrast_ratio(t.accent, t.canvas) >= 4.5);
    assert!(contrast_ratio(t.accent_on, tron_palette().oia_badge_bg) >= 4.5);
    assert!(contrast_ratio(t.border_idle, t.canvas) >= 1.5);
    assert!(contrast_ratio(t.border_focus, t.canvas) > contrast_ratio(t.border_idle, t.canvas) * 2.0);
    assert_eq!(t.accent, Color::Rgb(255, 159, 28));
}

#[test]
fn tron_config_round_trips_through_toml_and_config_save() {
    let parsed = BiltermConfig::from_toml_str("[appearance]\nprofile = \"tron\"\n").unwrap();
    assert!(tron_active(&parsed));
    assert_eq!(parsed.palette().name, "TRON");

    let mut cfg = BiltermConfig { liquid_glass: true, ..Default::default() };
    apply_visual_style_mode(&mut cfg, Some("tron"));
    let text = toml::to_string(&cfg).unwrap();
    assert!(text.contains("profile = \"tron\""), "{text}");
    assert!(!text.contains("tron_restore"), "restore state must stay runtime-only:\n{text}");
    let back = BiltermConfig::from_toml_str(&text).unwrap();
    assert!(tron_active(&back) && !back.liquid_glass);
    assert_eq!(back.appearance.tron_restore, None);

    let dir = std::env::temp_dir().join(format!("bilterm-tron-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let _ = std::fs::remove_file(&path);
    let mut c = BiltermConfig::default();
    bilterm::config::dotted::set_key(&mut c, "appearance.profile", "tron").unwrap();
    assert_eq!(bilterm::config::dotted::get_key(&c, "appearance.profile").unwrap(), "\"tron\"");
    bilterm::config::dotted::save_changes(&c, &path, Some("appearance.")).unwrap();
    let loaded = BiltermConfig::load(&path).unwrap().unwrap();
    assert!(tron_active(&loaded));
    let _ = std::fs::remove_dir_all(&dir);
}
