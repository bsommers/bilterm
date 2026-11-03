//! Coverage for `src/input/modal.rs::handle_wm_modal_key`: held exited shell, Ctrl+D / `exit`
//! interception, exit-guard modal, Help, Inspector, Palette and Menubar.

use std::sync::Once;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::pane::{ExitGuardChoice, ExitProtectionPolicy};
use bilterm::wm::{Modal, WindowManager};

static ENV: Once = Once::new();

fn isolate() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-modal-wm-cov-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: executed exactly once, before any test in this binary reads the environment.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn km(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
fn ch(c: char) -> KeyEvent {
    k(KeyCode::Char(c))
}
fn wm() -> WindowManager {
    isolate();
    WindowManager::new_headless(30, 100)
}
fn press(wm: &mut WindowManager, key: KeyEvent) -> AppAction {
    handle_wm_key_event(wm, key).unwrap()
}
fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or_default().to_string()
}

// ---------------------------------------------------------------------------------------------
// Held exited shell
// ---------------------------------------------------------------------------------------------

#[test]
fn held_shell_enter_and_r_respawn_in_place() {
    for key in [k(KeyCode::Enter), ch('r'), ch('R')] {
        let mut w = wm();
        w.active_pane_mut().hold_exited_shell(Some(0));
        assert!(w.active_pane().held_exited_shell().is_some());
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(w.active_pane().held_exited_shell().is_none(), "{key:?}");
    }
}

#[test]
fn held_shell_d_detaches_client_and_quits() {
    let mut w = wm();
    w.active_pane_mut().hold_exited_shell(Some(1));
    assert_eq!(press(&mut w, ch('d')), AppAction::Quit);
    assert!(w.is_client_detached());
    let mut w = wm();
    w.active_pane_mut().hold_exited_shell(None);
    assert_eq!(press(&mut w, ch('D')), AppAction::Quit);
}

#[test]
fn held_shell_other_keys_do_not_respawn() {
    let mut w = wm();
    w.active_pane_mut().hold_exited_shell(Some(0));
    press(&mut w, ch('z'));
    assert!(w.active_pane().held_exited_shell().is_some());
    // Modified Enter/r do not trigger respawn either.
    press(&mut w, km(KeyCode::Char('r'), KeyModifiers::ALT));
    assert!(w.active_pane().held_exited_shell().is_some());
}

#[test]
fn held_shell_ignored_while_leader_pending() {
    let mut w = wm();
    w.active_pane_mut().hold_exited_shell(Some(0));
    w.set_leader_pending(true);
    press(&mut w, ch('r'));
    assert!(w.active_pane().held_exited_shell().is_some());
}

// ---------------------------------------------------------------------------------------------
// Ctrl+D / `exit` interception (Modal::None path)
// ---------------------------------------------------------------------------------------------

#[test]
fn ctrl_d_on_empty_prompt_opens_exit_guard_for_root_session() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.config_mut().exit_protection = ExitProtectionPolicy::Guard;
    let act = press(&mut w, km(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(act, AppAction::Handled);
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
    assert!(
        w.exit_guard_state()
            .unwrap()
            .trigger_reason
            .starts_with("Ctrl+D (EOF)")
    );
}

#[test]
fn ctrl_d_with_detach_policy_detaches_and_quits() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.config_mut().exit_protection = ExitProtectionPolicy::Detach;
    let act = press(&mut w, km(KeyCode::Char('D'), KeyModifiers::CONTROL));
    assert_eq!(act, AppAction::Quit);
    assert!(w.is_client_detached());
}

#[test]
fn ctrl_d_with_respawn_policy_handled_without_modal() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.config_mut().exit_protection = ExitProtectionPolicy::Respawn;
    let act = press(&mut w, km(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_eq!(act, AppAction::Handled);
    assert!(matches!(w.modal(), Modal::None));
    assert!(!w.is_client_detached());
}

#[test]
fn typing_exit_then_enter_opens_exit_guard_and_clears_prompt() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.config_mut().exit_protection = ExitProtectionPolicy::Guard;
    w.active_pane_mut().prompt_mut().set_text("exit");
    let act = press(&mut w, k(KeyCode::Enter));
    assert_eq!(act, AppAction::Handled);
    let reason = w.exit_guard_state().expect("exit guard open").trigger_reason.clone();
    assert!(reason.contains("exit"), "reason was {reason:?}");
    assert_eq!(w.active_pane().prompt().text(), "", "submission consumed");
}

#[test]
fn typing_exit_with_detach_policy_quits_when_detached() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.config_mut().exit_protection = ExitProtectionPolicy::Detach;
    w.active_pane_mut().prompt_mut().set_text("logout");
    let act = press(&mut w, k(KeyCode::Enter));
    assert_eq!(act, AppAction::Quit);
    assert!(w.is_client_detached());
}

#[test]
fn ctrl_d_not_intercepted_when_prompt_has_text() {
    let mut w = wm();
    w.config_mut().is_root_session = true;
    w.active_pane_mut().prompt_mut().set_text("ls");
    press(&mut w, km(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(matches!(w.modal(), Modal::None));
}

// ---------------------------------------------------------------------------------------------
// Exit guard modal
// ---------------------------------------------------------------------------------------------

fn guard_wm() -> WindowManager {
    let mut w = wm();
    w.open_exit_guard("test trigger");
    w
}

#[test]
fn exit_guard_tab_and_arrows_cycle_selection() {
    let mut w = guard_wm();
    let s0 = w.exit_guard_state().unwrap().selected_index;
    assert_eq!(press(&mut w, k(KeyCode::Right)), AppAction::Handled);
    let s1 = w.exit_guard_state().unwrap().selected_index;
    assert_eq!(s1, (s0 + 1) % ExitGuardChoice::ALL.len());
    press(&mut w, k(KeyCode::Tab));
    assert_eq!(w.exit_guard_state().unwrap().selected_index, (s0 + 2) % 5);
    press(&mut w, k(KeyCode::Left));
    assert_eq!(w.exit_guard_state().unwrap().selected_index, s1);
    press(&mut w, k(KeyCode::BackTab));
    assert_eq!(w.exit_guard_state().unwrap().selected_index, s0);
    press(&mut w, k(KeyCode::BackTab));
    assert_eq!(
        w.exit_guard_state().unwrap().selected_index,
        ExitGuardChoice::ALL.len() - 1,
        "selection wraps backwards"
    );
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
}

#[test]
fn exit_guard_enter_applies_highlighted_choice() {
    // Default highlight: respawn shell -> modal closes, no quit.
    let mut w = guard_wm();
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::None));

    // Highlight Quit (index 2): Enter returns Quit.
    let mut w = guard_wm();
    press(&mut w, k(KeyCode::Right));
    press(&mut w, k(KeyCode::Right));
    assert_eq!(
        w.exit_guard_state().unwrap().selected_choice(),
        ExitGuardChoice::QuitClient
    );
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Quit);

    // Highlight Cancel (index 4): Enter dismisses with a status message.
    let mut w = guard_wm();
    for _ in 0..4 {
        press(&mut w, k(KeyCode::Right));
    }
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::None));
    assert_eq!(status(&w), "Exit cancelled — session preserved");
}

#[test]
fn exit_guard_hotkeys_map_to_choices() {
    let mut w = guard_wm();
    assert_eq!(press(&mut w, ch('q')), AppAction::Quit);
    let mut w = guard_wm();
    assert_eq!(press(&mut w, ch('K')), AppAction::Quit);
    let mut w = guard_wm();
    assert_eq!(press(&mut w, ch('d')), AppAction::Quit);
    assert!(w.is_client_detached());
    for key in [k(KeyCode::Esc), ch('c'), ch('C')] {
        let mut w = guard_wm();
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(matches!(w.modal(), Modal::None), "{key:?}");
        assert_eq!(status(&w), "Exit cancelled — session preserved");
    }
    let mut w = guard_wm();
    assert_eq!(press(&mut w, ch('r')), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::None));
}

#[test]
fn exit_guard_unknown_key_keeps_modal_open() {
    let mut w = guard_wm();
    assert_eq!(press(&mut w, ch('z')), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
    // Ctrl-modified hotkeys are rejected by ExitGuardChoice::from_key.
    assert_eq!(
        press(&mut w, km(KeyCode::Char('c'), KeyModifiers::ALT)),
        AppAction::Handled
    );
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
}

// ---------------------------------------------------------------------------------------------
// Help & Inspector
// ---------------------------------------------------------------------------------------------

fn help_wm() -> WindowManager {
    let mut w = wm();
    *w.modal_mut() = Modal::Help { scroll: 0 };
    w
}

fn help_scroll(w: &WindowManager) -> usize {
    match w.modal() {
        Modal::Help { scroll } => *scroll,
        other => panic!("expected help, got {other:?}"),
    }
}

#[test]
fn help_scroll_keys_and_bounds() {
    let mut w = help_wm();
    press(&mut w, k(KeyCode::Up));
    assert_eq!(help_scroll(&w), 0, "saturates at 0");
    press(&mut w, k(KeyCode::Down));
    assert_eq!(help_scroll(&w), 1);
    press(&mut w, ch('j'));
    assert_eq!(help_scroll(&w), 2);
    press(&mut w, ch('k'));
    assert_eq!(help_scroll(&w), 1);
    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(help_scroll(&w), 9);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(help_scroll(&w), 1);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(help_scroll(&w), 0);
    press(&mut w, k(KeyCode::End));
    assert_eq!(help_scroll(&w), 500);
    press(&mut w, k(KeyCode::Home));
    assert_eq!(help_scroll(&w), 0);
    press(&mut w, ch('x'));
    assert_eq!(help_scroll(&w), 0, "unmapped key leaves scroll");
}

#[test]
fn help_close_keys() {
    for key in [k(KeyCode::F(1)), k(KeyCode::Esc), ch('q')] {
        let mut w = help_wm();
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(matches!(w.modal(), Modal::None), "{key:?}");
    }
}

fn insp_scroll(w: &WindowManager) -> usize {
    match w.modal() {
        Modal::Inspector { scroll } => *scroll,
        other => panic!("expected inspector, got {other:?}"),
    }
}

#[test]
fn inspector_scroll_and_close_keys() {
    let mut w = wm();
    *w.modal_mut() = Modal::Inspector { scroll: 0 };
    press(&mut w, k(KeyCode::Up));
    assert_eq!(insp_scroll(&w), 0);
    press(&mut w, k(KeyCode::Down));
    press(&mut w, ch('j'));
    assert_eq!(insp_scroll(&w), 2);
    press(&mut w, ch('k'));
    assert_eq!(insp_scroll(&w), 1);
    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(insp_scroll(&w), 7);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(insp_scroll(&w), 1);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(insp_scroll(&w), 0);
    press(&mut w, ch('x'));
    assert_eq!(insp_scroll(&w), 0);
    for key in [k(KeyCode::F(9)), k(KeyCode::Esc), ch('q')] {
        let mut w = wm();
        *w.modal_mut() = Modal::Inspector { scroll: 3 };
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(matches!(w.modal(), Modal::None), "{key:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------------------------

fn palette_wm(query: &str) -> WindowManager {
    let mut w = wm();
    w.open_palette_with_query(query);
    w
}

#[test]
fn palette_typing_backspace_and_esc() {
    let mut w = palette_wm("");
    press(&mut w, ch('h'));
    press(&mut w, ch('e'));
    assert_eq!(w.palette_state(), Some(("he", 0)));
    press(&mut w, k(KeyCode::Backspace));
    assert_eq!(w.palette_state(), Some(("h", 0)));
    // Ctrl-modified chars are not typed.
    press(&mut w, km(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert_eq!(w.palette_state(), Some(("h", 0)));
    // Unmapped key ignored.
    press(&mut w, k(KeyCode::F(6)));
    assert!(w.is_palette_visible());
    assert_eq!(press(&mut w, k(KeyCode::Esc)), AppAction::Handled);
    assert!(!w.is_palette_visible());
}

#[test]
fn palette_selection_clamps_between_zero_and_last_result() {
    let mut w = palette_wm("");
    let count = w.filtered_palette_items().len();
    assert!(count > 2);
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.palette_state().unwrap().1, 0, "Up saturates at 0");
    press(&mut w, k(KeyCode::Down));
    press(&mut w, k(KeyCode::Down));
    assert_eq!(w.palette_state().unwrap().1, 2);
    for _ in 0..count + 5 {
        press(&mut w, k(KeyCode::Down));
    }
    assert_eq!(w.palette_state().unwrap().1, count - 1, "Down clamps at last result");
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.palette_state().unwrap().1, count - 2);
    // Typing resets selection to 0.
    press(&mut w, ch('s'));
    assert_eq!(w.palette_state().unwrap().1, 0);
}

#[test]
fn palette_down_with_no_results_is_noop() {
    let mut w = palette_wm("zzzzqqqqxxxx");
    assert!(w.filtered_palette_items().is_empty());
    press(&mut w, k(KeyCode::Down));
    assert_eq!(w.palette_state().unwrap().1, 0);
    assert!(w.is_palette_visible());
}

#[test]
fn palette_enter_executes_literal_colon_command_in_query() {
    let mut w = palette_wm(":quit");
    w.config_mut().exit_protection = ExitProtectionPolicy::Off;
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Quit);
    assert!(!w.is_palette_visible());
}

#[test]
fn palette_quit_on_final_pane_opens_exit_guard_instead_of_quitting() {
    let mut w = palette_wm(":quit");
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(!w.is_palette_visible());
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
}

#[test]
fn palette_enter_with_unmatched_text_runs_it_as_colon_command() {
    let mut w = palette_wm("zzzzqqqqxxxx");
    // Parses as ":zzzzqqqqxxxx" (unknown) -> palette closes, nothing quits.
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(!w.is_palette_visible());
    // And the quit spelling without a leading colon is accepted too.
    let mut w = palette_wm("");
    w.open_palette_with_query("");
    *w.modal_mut() = Modal::Palette {
        query: "zzzzqqqqxxxx".into(),
        selected: 0,
    };
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
}

#[test]
fn palette_enter_on_selected_catalog_item_closes_palette() {
    let mut w = palette_wm("");
    let first = w.filtered_palette_items()[0].0.to_string();
    assert!(!first.is_empty());
    let act = press(&mut w, k(KeyCode::Enter));
    assert!(!w.is_palette_visible());
    assert!(matches!(act, AppAction::Handled | AppAction::Quit));
}

#[test]
fn palette_enter_runs_quit_item_from_filtered_results() {
    let mut w = palette_wm("quit");
    w.config_mut().exit_protection = ExitProtectionPolicy::Off;
    let items = w.filtered_palette_items();
    let idx = items.iter().position(|i| i.0 == ":quit").expect(":quit in catalog");
    for _ in 0..idx {
        press(&mut w, k(KeyCode::Down));
    }
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Quit);
}

// ---------------------------------------------------------------------------------------------
// Menubar
// ---------------------------------------------------------------------------------------------

fn menubar_wm(cat: usize) -> WindowManager {
    let mut w = wm();
    w.set_menubar_open_category(Some(cat));
    w
}

#[test]
fn menubar_left_right_wrap_and_reset_item() {
    let mut w = menubar_wm(0);
    press(&mut w, k(KeyCode::Down));
    assert_eq!(w.menubar_state(), Some((0, 1)));
    press(&mut w, k(KeyCode::Left));
    assert_eq!(w.menubar_state(), Some((6, 0)), "Left wraps to the last category and resets item");
    press(&mut w, k(KeyCode::Right));
    assert_eq!(w.menubar_state(), Some((0, 0)), "Right wraps to the first category");
    press(&mut w, k(KeyCode::Right));
    assert_eq!(w.menubar_state(), Some((1, 0)));
}

#[test]
fn menubar_up_down_clamp_to_item_count() {
    let mut w = menubar_wm(2); // "Prompt" has 4 items
    let n = bilterm::ui::modals::menubar_category_item_count(2);
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.menubar_state(), Some((2, 0)));
    for _ in 0..n + 3 {
        press(&mut w, k(KeyCode::Down));
    }
    assert_eq!(w.menubar_state(), Some((2, n - 1)));
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.menubar_state(), Some((2, n - 2)));
    press(&mut w, ch('x'));
    assert_eq!(w.menubar_state(), Some((2, n - 2)), "unmapped keys are ignored");
}

#[test]
fn menubar_esc_closes() {
    let mut w = menubar_wm(1);
    assert_eq!(press(&mut w, k(KeyCode::Esc)), AppAction::Handled);
    assert!(w.menubar_state().is_none());
    assert!(matches!(w.modal(), Modal::None));
}

#[test]
fn menubar_enter_executes_item_and_closes_menu() {
    // System > Quit (index 6) returns Quit.
    let mut w = menubar_wm(0);
    w.config_mut().exit_protection = ExitProtectionPolicy::Off;
    for _ in 0..6 {
        press(&mut w, k(KeyCode::Down));
    }
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Quit);
    assert!(w.menubar_state().is_none());

    // With the default Guard policy the same item opens the Exit Guard modal instead.
    let mut w = menubar_wm(0);
    for _ in 0..6 {
        press(&mut w, k(KeyCode::Down));
    }
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));

    // Window > Split Horizontal creates a pane.
    let mut w = menubar_wm(1);
    let before = w.pane_count();
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert_eq!(w.pane_count(), before + 1);
    assert!(w.menubar_state().is_none());

    // Help > Help Reference opens the help modal.
    let mut w = menubar_wm(6);
    assert_eq!(press(&mut w, k(KeyCode::Enter)), AppAction::Handled);
    assert!(matches!(w.modal(), Modal::Help { .. }));
}

#[test]
fn menubar_enter_on_inspector_item_opens_inspector() {
    let mut w = menubar_wm(0);
    press(&mut w, k(KeyCode::Enter)); // item 0: 3270 Inspector
    assert!(matches!(w.modal(), Modal::Inspector { .. }));
}
