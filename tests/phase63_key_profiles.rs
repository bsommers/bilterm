//! Phase 63-01: platform-aware key model (`KEY-02`, `KEY-03`).
//!
//! Covers the `KeyEnv`/`Primary` resolution, the per-OS profiles, the Alt+M replacement, the
//! compose/AltGr/normalizer defects, the frozen `Action::{WindowMode, MenuOpen, Keys}` surface and
//! the profile integrity checks. Everything is headless and runs every OS path from one machine.

use bilterm::action::{Action, KeysOp, WinSub, WindowModeOp};
use bilterm::colon_command::parse_colon_command;
use bilterm::config::keys::{KeyProfileName, KeysConfig, LegacyAlt};
use bilterm::input::chord::KeyChord;
use bilterm::input::keyenv::{HostOs, KeyEnv, Surface, primary_mods};
use bilterm::input::keymap::{
    DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS, normalize_macos_option_key,
};
use bilterm::input::keyprofile::{EffectiveKeymap, KeyProfile, hint_for_action};
use bilterm::input::keyprofile_tables::ALLOWED_SHADOWS;
use bilterm::input::handle_wm_key_event;
use bilterm::pty::line_discipline::InputRoutingMode;
use bilterm::wm::{KeyMode, WindowManager, WorkspaceLayoutMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const CS: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT);

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ch(c: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), mods)
}

const PROFILES: [KeyProfileName; 5] = [
    KeyProfileName::Mac,
    KeyProfileName::Windows,
    KeyProfileName::Linux,
    KeyProfileName::Portable,
    KeyProfileName::Legacy,
];

fn all_envs() -> Vec<KeyEnv> {
    let mut v = Vec::new();
    for os in [HostOs::MacOs, HostOs::Windows, HostOs::Linux] {
        for surface in [Surface::Tui, Surface::Gui] {
            for kitty in [false, true] {
                v.push(KeyEnv::new(os, surface).with_kitty(kitty));
            }
        }
    }
    v
}

fn keymap(profile: KeyProfileName, env: KeyEnv, legacy_alt: LegacyAlt) -> EffectiveKeymap {
    KeyProfile::resolve(&KeysConfig { profile, legacy_alt, ..KeysConfig::default() }, &env)
}

fn is_alt_printable(mods: KeyModifiers, code: KeyCode) -> bool {
    mods.contains(KeyModifiers::ALT)
        && !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
        && matches!(code, KeyCode::Char(c) if !c.is_control())
}

fn wm_with(profile: KeyProfileName, env: KeyEnv) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_key_env(env);
    wm.set_key_profile(profile);
    wm
}

fn force_raw(wm: &mut WindowManager) {
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    wm.active_pane_mut().refresh_line_discipline();
    assert_eq!(wm.active_pane().effective_routing_mode(), InputRoutingMode::RawPtyPassthrough);
}

#[test]
fn primary_resolves_per_env() {
    let rows = [
        (KeyEnv::new(HostOs::MacOs, Surface::Gui), Some(KeyModifiers::SUPER)),
        (KeyEnv::new(HostOs::Windows, Surface::Gui), Some(CS)),
        (KeyEnv::new(HostOs::Linux, Surface::Gui), Some(CS)),
        (KeyEnv::new(HostOs::Linux, Surface::Tui).with_kitty(true), Some(CS)),
        (KeyEnv::new(HostOs::MacOs, Surface::Tui).with_kitty(true), Some(CS)),
        (KeyEnv::new(HostOs::Linux, Surface::Tui), None),
    ];
    for (env, want) in rows {
        assert_eq!(primary_mods(&env), want, "{env:?}");
    }
}

#[test]
fn mac_profile_has_no_alt_printable_wm_chord() {
    for env in all_envs().into_iter().filter(|e| e.os == HostOs::MacOs) {
        let km = keymap(KeyProfileName::Mac, env, LegacyAlt::Auto);
        for (m, c, a) in km.global_rows().chain(km.pane_rows()) {
            assert!(!is_alt_printable(m, c), "mac profile binds Alt+{c:?} ({m:?}) -> {a:?} in {env:?}");
        }
        assert!(km.global_action(ch('m', KeyModifiers::ALT), false).is_none());
        assert!(km.global_action(ch('M', KeyModifiers::ALT | KeyModifiers::SHIFT), false).is_none());
        assert!(km.pane_action(ch('m', KeyModifiers::ALT)).is_none());
        // Cmd+Option+Arrows replace the Alt+Arrow focus aliases.
        assert_eq!(
            km.global_action(ev(KeyCode::Left, KeyModifiers::SUPER | KeyModifiers::ALT), false),
            Some(Action::PaneFocusDirection { dx: -1, dy: 0 })
        );
        assert_eq!(km.focus_arrow_mods(), Some(KeyModifiers::SUPER | KeyModifiers::ALT));
    }
}

#[test]
fn alt_m_reaches_no_binding_on_mac_but_layout_cycle_is_reachable_in_every_profile() {
    for env in all_envs() {
        for profile in PROFILES {
            let km = keymap(profile, env, LegacyAlt::Off);
            // Leader M reaches LayoutCycle everywhere (the universal path).
            assert_eq!(km.leader_action(ch('M', KeyModifiers::NONE)), Some(Action::LayoutCycle), "{profile:?} {env:?}");
            // Whenever Primary exists, Primary+L does too.
            if km.primary_enabled() {
                let mods = primary_mods(&env).unwrap();
                let code = if mods == KeyModifiers::SUPER { 'l' } else { 'L' };
                assert_eq!(km.primary_action(ch(code, mods)), Some(Action::LayoutCycle), "{profile:?} {env:?}");
            }
            // The accelerator hint is never an Alt+<printable> chord.
            let hint = km.hint_for_action(&Action::LayoutCycle).expect("hint");
            assert!(!hint.contains("Alt+"), "{profile:?} {env:?}: {hint}");
            if profile == KeyProfileName::Mac {
                assert!(km.global_action(ch('m', KeyModifiers::ALT), false).is_none(), "Alt+M on mac {env:?}");
            }
        }
    }
    // Legacy Alt+M alias survives on legacy, and on windows/linux while legacy_alt is on.
    let env = KeyEnv::new(HostOs::Linux, Surface::Tui);
    assert_eq!(keymap(KeyProfileName::Legacy, env, LegacyAlt::Off).global_action(ch('m', KeyModifiers::ALT), false), Some(Action::LayoutCycle));
    assert_eq!(keymap(KeyProfileName::Linux, env, LegacyAlt::Auto).global_action(ch('m', KeyModifiers::ALT), false), Some(Action::LayoutCycle));
    assert_eq!(keymap(KeyProfileName::Windows, env, LegacyAlt::On).global_action(ch('m', KeyModifiers::ALT), false), Some(Action::LayoutCycle));
    assert_eq!(keymap(KeyProfileName::Linux, env, LegacyAlt::Off).global_action(ch('m', KeyModifiers::ALT), false), None);
    // Platform labels.
    assert_eq!(hint_for_action(&Action::LayoutCycle, &KeyEnv::new(HostOs::MacOs, Surface::Gui)).as_deref(), Some("⌘L"));
    assert_eq!(hint_for_action(&Action::LayoutCycle, &KeyEnv::new(HostOs::Windows, Surface::Gui)).as_deref(), Some("Ctrl+Shift+L"));
    assert_eq!(hint_for_action(&Action::LayoutCycle, &KeyEnv::new(HostOs::Linux, Surface::Tui)).as_deref(), Some("Ctrl+X M"));
}

#[test]
fn legacy_profile_matches_current_tables() {
    let km = EffectiveKeymap::legacy_default();
    let global: Vec<_> = km.global_rows().map(|(m, c, a)| (m, c, a.clone())).collect();
    assert_eq!(global, DEFAULT_GLOBAL_BINDINGS.to_vec());
    let pane: Vec<_> = km.pane_rows().map(|(m, c, a)| (m, c, a.clone())).collect();
    assert_eq!(pane, DEFAULT_PANE_BINDINGS.to_vec());
    assert_eq!(km.leader_rows(), DEFAULT_LEADER_BINDINGS.to_vec());
    // Sentinels against accidental edits of the shipped legacy tables.
    assert_eq!(DEFAULT_GLOBAL_BINDINGS.len(), 103, "legacy global table row count");
    for env in all_envs() {
        let legacy = keymap(KeyProfileName::Legacy, env, LegacyAlt::Off);
        assert_eq!(legacy.global_rows().count(), DEFAULT_GLOBAL_BINDINGS.len(), "{env:?}");
        assert!(!legacy.primary_enabled(), "{env:?}");
        assert_eq!(legacy.focus_arrow_mods(), Some(KeyModifiers::ALT));
    }
    // Headless workspaces start on the legacy profile with a deterministic env.
    let wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.keymap().profile(), KeyProfileName::Legacy);
    assert_eq!(wm.key_env(), KeyEnv::headless());
    assert_eq!(wm.key_mode(), KeyMode::Normal);
}

#[test]
fn no_duplicate_chords_after_shift_fold_except_allowlist() {
    let mut exercised = std::collections::BTreeSet::new();
    for env in all_envs() {
        for profile in PROFILES {
            for legacy_alt in [LegacyAlt::On, LegacyAlt::Off] {
                let km = keymap(profile, env, legacy_alt);
                let dups = km.unexpected_duplicates();
                assert!(dups.is_empty(), "{profile:?} {env:?} {legacy_alt:?}: {dups:?}");
            }
        }
    }
    // Every allow-listed shadow has a reason and still describes a real duplicate on `legacy`.
    for (chord, reason) in ALLOWED_SHADOWS {
        assert!(reason.len() > 12, "{chord}: allow-listed shadow needs a reason");
        KeyChord::parse(chord).unwrap_or_else(|e| panic!("{chord}: {e}"));
    }
    let km = EffectiveKeymap::legacy_default();
    let mut seen: Vec<(KeyChord, Action)> = Vec::new();
    for (m, c, a) in km.global_rows().chain(km.pane_rows()) {
        let f = KeyChord::new(m, c).folded();
        if seen.iter().any(|(o, oa)| *o == f && oa != a) {
            exercised.insert(f.format(&KeyEnv::headless()).to_ascii_uppercase());
        }
        seen.push((f, a.clone()));
    }
    for (chord, _) in ALLOWED_SHADOWS {
        assert!(exercised.contains(&chord.to_ascii_uppercase()), "stale ALLOWED_SHADOWS entry {chord}; real duplicates: {exercised:?}");
    }
}

#[test]
fn normalizer_is_identity_off_mac_tui() {
    let composed = [ch('ø', KeyModifiers::NONE), ch('ß', KeyModifiers::NONE), ch('å', KeyModifiers::NONE), ch('µ', KeyModifiers::NONE), ch('π', KeyModifiers::NONE)];
    for env in all_envs() {
        for key in composed {
            let out = normalize_macos_option_key(key, true, &env);
            if env.os == HostOs::MacOs && env.surface == Surface::Tui {
                assert_eq!(out.modifiers, KeyModifiers::ALT, "{env:?} {key:?}");
            } else {
                assert_eq!(out, key, "normalizer must be identity for {env:?}");
            }
        }
    }
    let mac_tui = KeyEnv::new(HostOs::MacOs, Surface::Tui);
    assert_eq!(normalize_macos_option_key(ch('µ', KeyModifiers::NONE), true, &mac_tui), ch('m', KeyModifiers::ALT));
    assert_eq!(normalize_macos_option_key(ch('µ', KeyModifiers::NONE), false, &mac_tui), ch('µ', KeyModifiers::NONE));
}

#[test]
fn raw_passthrough_does_not_rewrite_composed_letters() {
    for env in [
        KeyEnv::new(HostOs::Linux, Surface::Tui),
        KeyEnv::new(HostOs::Windows, Surface::Tui),
        KeyEnv::new(HostOs::Linux, Surface::Gui),
        KeyEnv::new(HostOs::MacOs, Surface::Gui),
    ] {
        for profile in [KeyProfileName::Legacy, KeyProfileName::Linux, KeyProfileName::Mac] {
            let mut wm = wm_with(profile, env);
            force_raw(&mut wm);
            handle_wm_key_event(&mut wm, ch('ø', KeyModifiers::NONE)).unwrap();
            assert_eq!(wm.active_pane().last_pty_write(), "ø".as_bytes(), "{profile:?} {env:?}");
            handle_wm_key_event(&mut wm, ch('µ', KeyModifiers::NONE)).unwrap();
            assert_eq!(wm.active_pane().last_pty_write(), "µ".as_bytes(), "{profile:?} {env:?}");
        }
    }
    // Symbols that real EU layouts type with AltGr/Option (`£ § º ª ¡`) were normalized to
    // Alt+<digit> and swallowed by the WM focus chords even in raw passthrough on Linux/Windows.
    for c in ['£', '§', 'º', 'ª', '¡'] {
        let mut wm = wm_with(KeyProfileName::Legacy, KeyEnv::new(HostOs::Linux, Surface::Tui));
        force_raw(&mut wm);
        handle_wm_key_event(&mut wm, ch(c, KeyModifiers::NONE)).unwrap();
        assert_eq!(wm.active_pane().last_pty_write(), c.to_string().as_bytes(), "{c} must reach the child");
    }
    // On a macOS TUI the normalizer still applies (Option-as-Meta): Option+3 is Alt+3, a WM focus chord.
    let mut wm = wm_with(KeyProfileName::Legacy, KeyEnv::new(HostOs::MacOs, Surface::Tui));
    force_raw(&mut wm);
    handle_wm_key_event(&mut wm, ch('£', KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().last_pty_write().is_empty(), "consumed as Alt+3 on a macOS TUI");
}

#[test]
fn leader_w_enters_window_mode_and_w_upper_still_closes() {
    let leader = ch('x', KeyModifiers::CONTROL);
    // Non-legacy: leader w enters Window mode, leader W closes the pane.
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    wm.split_focused(bilterm::wm::SplitDirection::Vertical).unwrap();
    assert_eq!(wm.pane_count(), 2);
    handle_wm_key_event(&mut wm, leader).unwrap();
    assert!(wm.is_leader_pending() && wm.key_prefix_active());
    handle_wm_key_event(&mut wm, ch('w', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Focus));
    assert!(!wm.is_leader_pending() && wm.key_prefix_active());
    assert_eq!(wm.pane_count(), 2);
    wm.apply(Action::WindowMode(WindowModeOp::Exit)).unwrap();
    assert_eq!(wm.key_mode(), KeyMode::Normal);
    handle_wm_key_event(&mut wm, leader).unwrap();
    handle_wm_key_event(&mut wm, ch('W', KeyModifiers::SHIFT)).unwrap();
    assert_eq!(wm.pane_count(), 1, "leader W still closes the pane");

    // Legacy: leader w is ClosePane.
    let mut wm = wm_with(KeyProfileName::Legacy, KeyEnv::headless());
    wm.split_focused(bilterm::wm::SplitDirection::Vertical).unwrap();
    handle_wm_key_event(&mut wm, leader).unwrap();
    handle_wm_key_event(&mut wm, ch('w', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.key_mode(), KeyMode::Normal);
}

#[test]
fn leader_m_opens_menubar_and_colon_key_apply_agree() {
    let leader = ch('x', KeyModifiers::CONTROL);
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    handle_wm_key_event(&mut wm, leader).unwrap();
    handle_wm_key_event(&mut wm, ch('m', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.menubar_state(), Some((0, 0)));

    let mut wm2 = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    let parsed = parse_colon_command(":menu open").unwrap().unwrap();
    assert_eq!(parsed, Action::MenuOpen { category: None });
    wm2.apply(parsed).unwrap();
    assert_eq!(wm2.menubar_state(), Some((0, 0)));

    let mut wm3 = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    wm3.apply(Action::MenuOpen { category: Some(3) }).unwrap();
    assert_eq!(wm3.menubar_open_category(), Some(3));
    // Legacy leader m stays PromptModeSet.
    let mut wm4 = wm_with(KeyProfileName::Legacy, KeyEnv::headless());
    handle_wm_key_event(&mut wm4, leader).unwrap();
    handle_wm_key_event(&mut wm4, ch('m', KeyModifiers::NONE)).unwrap();
    assert_eq!(wm4.menubar_state(), None);
}

#[test]
fn primary_chords_route_through_apply_even_over_raw_passthrough() {
    let gui = KeyEnv::new(HostOs::Linux, Surface::Gui);
    let mut wm = wm_with(KeyProfileName::Auto, gui);
    assert_eq!(wm.keymap().profile(), KeyProfileName::Linux);
    wm.split_focused(bilterm::wm::SplitDirection::Vertical).unwrap();
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    // X11/Win32 report Ctrl+Shift+L as 'L' with CONTROL|SHIFT.
    handle_wm_key_event(&mut wm, ch('L', CS)).unwrap();
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    handle_wm_key_event(&mut wm, ch('l', CS)).unwrap();
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    // Ctrl+Shift+2 arrives as '@', Ctrl+Shift+1 as '!'.
    handle_wm_key_event(&mut wm, ch('@', CS)).unwrap();
    assert_eq!(wm.active_pane_id(), 2);
    handle_wm_key_event(&mut wm, ch('!', CS)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    // Primary+O enters Window mode; Primary+; opens the menu bar.
    handle_wm_key_event(&mut wm, ch('O', CS)).unwrap();
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Focus));
    wm.apply(Action::WindowMode(WindowModeOp::Exit)).unwrap();
    handle_wm_key_event(&mut wm, ch(':', CS)).unwrap();
    assert_eq!(wm.menubar_state(), Some((0, 0)));
    wm.set_menubar_open_category(None);

    // macOS GUI: Cmd+L is SUPER+l, also while an alt-screen app owns the pane.
    let mut mac = wm_with(KeyProfileName::Auto, KeyEnv::new(HostOs::MacOs, Surface::Gui));
    force_raw(&mut mac);
    handle_wm_key_event(&mut mac, ch('l', KeyModifiers::SUPER)).unwrap();
    assert_eq!(mac.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert!(mac.active_pane().last_pty_write().is_empty(), "nothing reaches the child");

    // A legacy TUI has no Primary: Ctrl+L reaches the child untouched.
    let mut tui = wm_with(KeyProfileName::Linux, KeyEnv::new(HostOs::Linux, Surface::Tui));
    force_raw(&mut tui);
    handle_wm_key_event(&mut tui, ch('L', CS)).unwrap();
    assert_eq!(tui.layout_mode(), WorkspaceLayoutMode::Tiled);
}

#[test]
fn alt_layer_follows_profile_for_wm_and_pane_hotkeys() {
    let alt_h = ch('h', KeyModifiers::ALT);
    // legacy / linux(legacy_alt on): Alt+h splits.
    for profile in [KeyProfileName::Legacy, KeyProfileName::Linux] {
        let mut wm = wm_with(profile, KeyEnv::headless());
        handle_wm_key_event(&mut wm, alt_h).unwrap();
        assert_eq!(wm.pane_count(), 2, "{profile:?}");
    }
    // mac / portable, or linux with legacy_alt = off: Alt+h is not a WM chord.
    for profile in [KeyProfileName::Mac, KeyProfileName::Portable] {
        let mut wm = wm_with(profile, KeyEnv::new(HostOs::MacOs, Surface::Gui));
        handle_wm_key_event(&mut wm, alt_h).unwrap();
        assert_eq!(wm.pane_count(), 1, "{profile:?}");
    }
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    wm.config_mut().keys.legacy_alt = LegacyAlt::Off;
    handle_wm_key_event(&mut wm, alt_h).unwrap();
    assert_eq!(wm.pane_count(), 1);
    // Pane-table Alt+t (PromptTop) follows the same switch.
    let mut mac = wm_with(KeyProfileName::Mac, KeyEnv::new(HostOs::MacOs, Surface::Gui));
    assert!(mac.keymap().pane_action(ch('t', KeyModifiers::ALT)).is_none());
    handle_wm_key_event(&mut mac, ch('t', KeyModifiers::ALT)).unwrap();
}

#[test]
fn ctrl_g_raw_toggle_is_command_prompt_only_outside_legacy() {
    // Legacy: Ctrl+G toggles raw even from raw passthrough (unchanged behaviour).
    let mut wm = wm_with(KeyProfileName::Legacy, KeyEnv::headless());
    force_raw(&mut wm);
    handle_wm_key_event(&mut wm, ch('g', KeyModifiers::CONTROL)).unwrap();
    assert!(wm.active_pane().last_pty_write().is_empty());
    // Linux profile: Ctrl+G reaches the child (emacs C-g / readline abort).
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    force_raw(&mut wm);
    handle_wm_key_event(&mut wm, ch('g', KeyModifiers::CONTROL)).unwrap();
    assert_eq!(wm.active_pane().last_pty_write(), &[0x07]);
    // ...but still toggles at the command prompt, and F12 still leaves raw.
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    assert_eq!(wm.active_pane().effective_routing_mode(), InputRoutingMode::CommandPrompt);
    handle_wm_key_event(&mut wm, ch('g', KeyModifiers::CONTROL)).unwrap();
    assert_eq!(wm.active_pane().effective_routing_mode(), InputRoutingMode::RawPtyPassthrough);
    // Ctrl+_ is no longer FontZoomOut outside legacy.
    let km = keymap(KeyProfileName::Linux, KeyEnv::headless(), LegacyAlt::Auto);
    assert_eq!(km.global_action(ch('_', KeyModifiers::CONTROL), false), None);
    assert_eq!(km.global_action(ch('-', KeyModifiers::CONTROL), false), Some(Action::FontZoomOut));
    let legacy = keymap(KeyProfileName::Legacy, KeyEnv::headless(), LegacyAlt::Auto);
    assert_eq!(legacy.global_action(ch('_', KeyModifiers::CONTROL), false), Some(Action::FontZoomOut));
}

#[test]
fn frozen_actions_have_colon_forms_that_round_trip() {
    for action in [
        Action::WindowMode(WindowModeOp::Enter),
        Action::WindowMode(WindowModeOp::Exit),
        Action::WindowMode(WindowModeOp::Toggle),
        Action::WindowMode(WindowModeOp::Sub(WinSub::Focus)),
        Action::WindowMode(WindowModeOp::Sub(WinSub::Resize)),
        Action::WindowMode(WindowModeOp::Sub(WinSub::Move)),
        Action::MenuOpen { category: None },
        Action::MenuOpen { category: Some(4) },
        Action::Keys(KeysOp::List),
        Action::Keys(KeysOp::Check),
        Action::Keys(KeysOp::Explain("Primary+L".into())),
        Action::Keys(KeysOp::Profile(None)),
        Action::Keys(KeysOp::Profile(Some("mac".into()))),
    ] {
        let line = action.colon_form_or_summary();
        assert_eq!(parse_colon_command(&line).unwrap().unwrap(), action, "{line}");
    }
    // :wmode sub-modes through apply
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(parse_colon_command(":wmode resize").unwrap().unwrap()).unwrap();
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Resize));
    wm.set_window_sub(WinSub::Move);
    assert_eq!(wm.key_mode().window_sub(), Some(WinSub::Move));
    wm.apply(parse_colon_command(":wmode").unwrap().unwrap()).unwrap();
    assert_eq!(wm.key_mode(), KeyMode::Normal);
    wm.enter_window_mode();
    wm.set_leader_pending(false);
    assert!(wm.key_mode().is_window(), "cancelling a leader never clobbers Window mode");
    wm.exit_window_mode();
    assert!(!wm.key_prefix_active());
}

#[test]
fn keys_ops_list_check_explain_and_profile_switch() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_key_env(KeyEnv::new(HostOs::Linux, Surface::Gui));
    wm.apply(Action::Keys(KeysOp::Profile(Some("linux".into())))).unwrap();
    assert_eq!(wm.keymap().profile(), KeyProfileName::Linux);
    wm.apply(Action::Keys(KeysOp::Check)).unwrap();
    assert!(wm.active_pane().status_message().unwrap().contains("no internal conflicts"));
    wm.apply(Action::Keys(KeysOp::Explain("Primary+L".into()))).unwrap();
    let msg = wm.active_pane().status_message().unwrap().to_string();
    assert!(msg.contains("Ctrl+Shift+L") && msg.contains("LayoutCycle"), "{msg}");
    wm.apply(Action::Keys(KeysOp::List)).unwrap();
    let data = wm.take_command_data().expect("structured list");
    assert!(data.contains("\"profile\":\"linux\"") && data.contains("primary"), "{data}");
    wm.apply(Action::Keys(KeysOp::Profile(Some("nope".into())))).unwrap();
    assert!(wm.active_pane().status_message().unwrap().contains("unknown key profile"));
    assert_eq!(wm.keymap().profile(), KeyProfileName::Linux);
}

#[test]
fn user_bind_accepts_primary_tokens_and_unbind_removes_shipped_chords() {
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::new(HostOs::Linux, Surface::Gui));
    wm.config_mut().keys.bind.insert("Primary+K".into(), ":split v".into());
    handle_wm_key_event(&mut wm, ch('K', CS)).unwrap();
    assert_eq!(wm.pane_count(), 2);
    // [keybindings] can now bind arrows, and keys.bind wins over it.
    let mut wm = wm_with(KeyProfileName::Legacy, KeyEnv::headless());
    wm.config_mut().keybindings.insert("Ctrl+Left".into(), ":split h".into());
    handle_wm_key_event(&mut wm, ev(KeyCode::Left, KeyModifiers::CONTROL)).unwrap();
    assert_eq!(wm.pane_count(), 2);
    wm.config_mut().keys.bind.insert("Ctrl+Left".into(), ":close".into());
    handle_wm_key_event(&mut wm, ev(KeyCode::Left, KeyModifiers::CONTROL)).unwrap();
    assert_eq!(wm.pane_count(), 1);
    // unbind
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    wm.config_mut().keys.unbind.push("Alt+V".into());
    handle_wm_key_event(&mut wm, ch('v', KeyModifiers::ALT)).unwrap();
    assert_eq!(wm.pane_count(), 1);
    handle_wm_key_event(&mut wm, ch('h', KeyModifiers::ALT)).unwrap();
    assert_eq!(wm.pane_count(), 2);
}

#[test]
fn which_key_stub_and_leader_status_come_from_the_live_table() {
    let mut wm = wm_with(KeyProfileName::Linux, KeyEnv::headless());
    handle_wm_key_event(&mut wm, ch('x', KeyModifiers::CONTROL)).unwrap();
    let line = wm.active_pane().status_message().unwrap().to_string();
    assert!(line.contains("w=Window") && line.contains("m=Menu"), "{line}");
    let mut legacy = wm_with(KeyProfileName::Legacy, KeyEnv::headless());
    handle_wm_key_event(&mut legacy, ch('x', KeyModifiers::CONTROL)).unwrap();
    let line = legacy.active_pane().status_message().unwrap().to_string();
    assert!(!line.contains("w=Window"), "{line}");
    let rows = bilterm::input::keyprofile::which_key_rows(&KeyMode::LeaderPending, &KeyEnv::headless());
    assert!(rows.iter().any(|r| r.chord == "w" && r.label == "Window"));
    assert!(bilterm::input::keyprofile::which_key_rows(&KeyMode::Normal, &KeyEnv::headless()).is_empty());
}
