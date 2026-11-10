//! VTE host terminals (GNOME Terminal, Tilix, ...) swallow F10 as their menu accelerator, so the
//! default F10 -> Quake binding silently does nothing. `vte_f10_hint` explains this at startup.

use bilterm::config::BiltermConfig;
use bilterm::input::keyconflict::vte_f10_hint;
use bilterm::input::keyenv::{HostOs, KeyEnv, Surface};

fn tui() -> KeyEnv {
    KeyEnv::new(HostOs::Linux, Surface::Tui)
}

#[test]
fn vte_terminal_with_default_f10_quake_gets_hint_with_alternative() {
    let hint = vte_f10_hint(&BiltermConfig::default(), &tui(), Some("8400")).expect("hint");
    assert!(hint.contains("F10"), "{hint}");
    assert!(hint.contains("menu"), "{hint}");
    assert!(hint.contains(":quake"), "{hint}");
    assert!(!hint.contains("or F10"), "alternative must not be F10 itself: {hint}");
}

#[test]
fn non_vte_terminal_gets_no_hint() {
    assert_eq!(vte_f10_hint(&BiltermConfig::default(), &tui(), None), None);
    assert_eq!(vte_f10_hint(&BiltermConfig::default(), &tui(), Some("")), None);
}

#[test]
fn gui_surface_gets_no_hint() {
    let env = KeyEnv::new(HostOs::Linux, Surface::Gui);
    assert_eq!(vte_f10_hint(&BiltermConfig::default(), &env, Some("8400")), None);
}
