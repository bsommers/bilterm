//! Phase 63-02: sticky Window mode, which-key overlay and the keyboard menu bar
//! (`KEY-01`, `KEY-02`, `KEY-03`). Everything is headless; every OS/surface path runs from one machine.

use bilterm::action::{Action, WinSub, WindowModeOp};
use bilterm::colon_command::parse_colon_command;
use bilterm::config::keys::KeyProfileName;
use bilterm::input::AppAction;
use bilterm::input::handle_wm_key_event;
use bilterm::input::keyenv::{HostOs, KeyEnv, Surface};
use bilterm::input::window_mode_tables::{WINDOW_MODE_BINDINGS, WinScope, rows_for_scope};
use bilterm::pty::line_discipline::InputRoutingMode;
use bilterm::ui::which_key_ui::{footer_line, which_key_rows};
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::{KeyMode, SplitDirection, WindowManager, WorkspaceLayoutMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const CS: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::SHIFT);

fn ch(c: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), mods)
}

fn press(wm: &mut WindowManager, code: KeyCode) -> AppAction {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap()
}

fn typ(wm: &mut WindowManager, c: char) -> AppAction {
    let mods = if c.is_ascii_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
    handle_wm_key_event(wm, ch(c, mods)).unwrap()
}

fn leader(wm: &mut WindowManager) {
    handle_wm_key_event(wm, ch('x', KeyModifiers::CONTROL)).unwrap();
}

/// A headless workspace with three side-by-side panes, the `portable` profile on a Linux TUI.
fn wm3(env: KeyEnv, profile: KeyProfileName) -> WindowManager {
    let mut wm = WindowManager::new_headless(30, 100);
    wm.set_key_env(env);
    wm.set_key_profile(profile);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, 100, 30)).unwrap();
    wm
}

fn linux_tui() -> KeyEnv {
    KeyEnv::new(HostOs::Linux, Surface::Tui)
}

fn enter_via_leader(wm: &mut WindowManager) {
    leader(wm);
    typ(wm, 'w');
    assert!(wm.key_mode().is_window(), "leader w must enter Window mode");
}

fn force_raw(wm: &mut WindowManager) {
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    wm.active_pane_mut().refresh_line_discipline();
    assert_eq!(wm.active_pane().effective_routing_mode(), InputRoutingMode::RawPtyPassthrough);
}

fn render(wm: &mut WindowManager) -> String {
    let area = Rect::new(0, 0, 100, 30);
    let ids: Vec<_> = wm.panes().keys().copied().collect();
    for id in ids {
        wm.pane_mut(id).unwrap().set_status_message(None);
    }
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buffer_to_snapshot_string(&buf)
}

/// Everything a Window-mode key may change, for parity against `wm.apply`.
fn fingerprint(wm: &mut WindowManager) -> String {
    let ids: Vec<_> = wm.panes().keys().copied().collect();
    let st = format!(
        "{ids:?} active={} layout={:?} zoom={:?} drawer={} palette={} find={} help={} agents={} copy={} mode={:?}",
        wm.active_pane_id(),
        wm.layout_mode(),
        wm.zoomed_pane(),
        wm.drawer_is_open(),
        wm.is_palette_visible(),
        wm.is_find_visible(),
        wm.is_help_visible(),
        wm.agents_list().active,
        wm.active_pane().copy_mode().active,
        wm.key_mode(),
    );
    format!("{st}\n{}", render(wm))
}

fn hands_control_away(a: &Action) -> bool {
    matches!(
        a,
        Action::CopyModeToggle | Action::PaletteToggle | Action::Find(_) | Action::DrawerFocus | Action::AgentsList(_)
    )
}

#[test]
fn window_mode_key_matrix() {
    for (scope, mods, code, action) in WINDOW_MODE_BINDINGS {
        let mut a = wm3(linux_tui(), KeyProfileName::Portable);
        enter_via_leader(&mut a);
        let mut b = wm3(linux_tui(), KeyProfileName::Portable);
        b.apply(Action::WindowMode(WindowModeOp::Enter)).unwrap();
        match scope {
            WinScope::Focus => {}
            WinScope::Resize => {
                typ(&mut a, 'r');
                b.apply(Action::WindowMode(WindowModeOp::Sub(WinSub::Resize))).unwrap();
            }
            WinScope::Move => {
                typ(&mut a, 'm');
                b.apply(Action::WindowMode(WindowModeOp::Sub(WinSub::Move))).unwrap();
            }
        }
        let event = KeyEvent::new(*code, *mods);
        handle_wm_key_event(&mut a, event).unwrap();
        // Parity: the colon form of the same action on an identical workspace.
        let colon = parse_colon_command(&action.colon_form_or_summary());
        let lowered = colon.and_then(Result::ok).unwrap_or_else(|| action.clone());
        if hands_control_away(action) {
            b.apply(Action::WindowMode(WindowModeOp::Exit)).unwrap();
        }
        b.apply(lowered).unwrap();
        assert_eq!(fingerprint(&mut a), fingerprint(&mut b), "{scope:?} {code:?} -> {action:?}");
    }
}

#[test]
fn window_mode_is_sticky_and_esc_enter_q_exit() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    enter_via_leader(&mut wm);
    for c in ['h', 'l', 'z', 'z', 'k', 'j'] {
        typ(&mut wm, c);
        assert!(wm.key_mode().is_window(), "still sticky after {c}");
    }
    press(&mut wm, KeyCode::Right);
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Focus));
    for exit in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')] {
        enter_via_leader(&mut wm);
        press(&mut wm, exit);
        assert_eq!(wm.key_mode(), KeyMode::Normal, "{exit:?} leaves Window mode");
    }
}

#[test]
fn resize_and_move_submodes_switch_and_return() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    enter_via_leader(&mut wm);
    typ(&mut wm, 'r');
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Resize));
    typ(&mut wm, 'm');
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Move));
    typ(&mut wm, 'f');
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Focus));
}

#[test]
fn unknown_key_is_swallowed_with_a_hint() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    enter_via_leader(&mut wm);
    let before = wm.active_pane_id();
    for ev in [ch('Q', KeyModifiers::SHIFT), ch('g', KeyModifiers::NONE), ch('h', KeyModifiers::CONTROL), ch('x', KeyModifiers::ALT)] {
        assert_eq!(handle_wm_key_event(&mut wm, ev).unwrap(), AppAction::Handled);
        assert!(wm.key_mode().is_window(), "{ev:?} must not leave the mode");
        let status = wm.active_pane().status_message().unwrap_or("").to_string();
        assert!(status.contains("unknown key"), "{ev:?}: {status}");
    }
    assert_eq!(wm.active_pane_id(), before);
    assert!(wm.active_pane().last_pty_write().is_empty());
}

#[test]
fn window_mode_blocks_pty_in_raw_passthrough() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    force_raw(&mut wm);
    // Leader w is a WM chord even over raw passthrough.
    enter_via_leader(&mut wm);
    for c in ['h', 'j', 'g', 'k'] {
        typ(&mut wm, c);
    }
    typ(&mut wm, 'z');
    typ(&mut wm, 'z');
    assert!(wm.key_mode().is_window());
    for id in wm.panes().keys().copied().collect::<Vec<_>>() {
        assert!(wm.pane(id).unwrap().last_pty_write().is_empty(), "pane {id} got bytes from Window mode");
    }
    press(&mut wm, KeyCode::Esc);
    assert_eq!(wm.key_mode(), KeyMode::Normal);
    force_raw(&mut wm);
    typ(&mut wm, 'h');
    assert_eq!(wm.active_pane().last_pty_write(), b"h", "outside Window mode the key reaches the child");
}

#[test]
fn window_mode_focus_while_zoomed_matches_apply() {
    let build = || {
        let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
        wm.apply(Action::FocusPane(1)).unwrap();
        wm.apply(Action::ZoomToggle).unwrap();
        wm
    };
    let mut a = build();
    enter_via_leader(&mut a);
    typ(&mut a, 'l');
    let mut b = build();
    b.apply(Action::WindowMode(WindowModeOp::Enter)).unwrap();
    b.apply(Action::PaneFocusDirection { dx: 1, dy: 0 }).unwrap();
    assert_eq!(a.active_pane_id(), b.active_pane_id());
    assert_eq!(a.zoomed_pane(), b.zoomed_pane());
    assert_eq!(fingerprint(&mut a), fingerprint(&mut b));
}

#[test]
fn primary_o_and_colon_enter_the_same_mode() {
    let gui = KeyEnv::new(HostOs::Linux, Surface::Gui);
    let mut by_key = wm3(gui, KeyProfileName::Auto);
    handle_wm_key_event(&mut by_key, ch('O', CS)).unwrap();
    let mut by_colon = wm3(gui, KeyProfileName::Auto);
    by_colon.apply(parse_colon_command(":wmode").unwrap().unwrap()).unwrap();
    let mut by_leader = wm3(gui, KeyProfileName::Auto);
    enter_via_leader(&mut by_leader);
    assert_eq!(by_key.key_mode(), KeyMode::Window(WinSub::Focus));
    assert_eq!(by_key.key_mode(), by_colon.key_mode());
    assert_eq!(by_key.key_mode(), by_leader.key_mode());
}

#[test]
fn stack_and_tile_keys_select_layouts() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    enter_via_leader(&mut wm);
    typ(&mut wm, 's');
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    typ(&mut wm, 't');
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
    press(&mut wm, KeyCode::Char(' '));
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    typ(&mut wm, 'x');
    assert_eq!(wm.pane_count(), 2);
}

#[test]
fn control_handoff_actions_leave_window_mode() {
    for (key, check) in [('c', "copy"), (':', "palette")] {
        let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
        enter_via_leader(&mut wm);
        typ(&mut wm, key);
        assert_eq!(wm.key_mode(), KeyMode::Normal, "{check}");
    }
}

#[test]
fn user_window_mode_binding_runs_through_apply() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Portable);
    wm.config_mut().keys.window_mode.insert("g".into(), ":layout stack".into());
    enter_via_leader(&mut wm);
    typ(&mut wm, 'g');
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert!(wm.key_mode().is_window());
}

#[test]
fn hint_for_stack_windows_is_the_window_mode_sequence() {
    let stack = Action::LayoutMode(WorkspaceLayoutMode::Stacked);
    let wm = wm3(linux_tui(), KeyProfileName::Auto);
    assert_eq!(wm.hint_for_action(&stack).as_deref(), Some("Ctrl+X w s"));
    // The legacy profile keeps leader w = ClosePane, so it must not advertise the sequence.
    let legacy = wm3(linux_tui(), KeyProfileName::Legacy);
    assert_eq!(legacy.hint_for_action(&stack), None);
}

// ---------------------------------------------------------------- which-key

#[test]
fn which_key_rows_match_distinct_actions_of_the_tables() {
    let env = linux_tui();
    for sub in [WinSub::Focus, WinSub::Resize, WinSub::Move] {
        let scope = WinScope::of(sub);
        let mut distinct: Vec<&Action> = Vec::new();
        for (_, _, _, a) in rows_for_scope(scope) {
            if !distinct.contains(&a) {
                distinct.push(a);
            }
        }
        let rows = which_key_rows(&KeyMode::Window(sub), &env);
        assert_eq!(rows.len(), distinct.len(), "{sub:?}: row per distinct action");
    }
    let km = bilterm::input::keyprofile::KeyProfile::resolve(&bilterm::config::keys::KeysConfig::default(), &env);
    let mut distinct: Vec<Action> = Vec::new();
    for (_, a) in km.leader_rows() {
        if !distinct.contains(&a) {
            distinct.push(a);
        }
    }
    assert_eq!(which_key_rows(&KeyMode::LeaderPending, &env).len(), distinct.len());
    assert!(which_key_rows(&KeyMode::Normal, &env).is_empty());
}

#[test]
fn leader_hint_and_window_footer_come_from_the_tables() {
    let wm = wm3(linux_tui(), KeyProfileName::Linux);
    let km = wm.keymap();
    let leader_line = footer_line(&KeyMode::LeaderPending, &km, "Ctrl+X");
    let body = leader_line.strip_prefix("⌨ LEADER [Ctrl+X]: ").unwrap_or_else(|| panic!("{leader_line}"));
    assert!(body.contains("w=Window") && body.contains("m=Menu"), "{leader_line}");
    assert_eq!(
        footer_line(&KeyMode::Window(WinSub::Focus), &km, "Ctrl+X"),
        "WINDOW [focus]  hjkl move  1-9 jump  z zoom  s stack  t tile  x close  ? help  Esc exit"
    );
    // The status bar shows exactly that line on entering the mode.
    let mut live = wm3(linux_tui(), KeyProfileName::Linux);
    leader(&mut live);
    // The leader label depends on $TMUX, so compare the generated body.
    let live_line = live.active_pane().status_message().unwrap_or("").to_string();
    assert!(live_line.starts_with("⌨ LEADER [") && live_line.ends_with(body), "{live_line}");
    typ(&mut live, 'w');
    assert_eq!(live.active_pane().status_message().unwrap_or(""), footer_line(&KeyMode::Window(WinSub::Focus), &km, "Ctrl+X"));
}

#[test]
fn question_mark_pins_the_popup_and_delay_is_honoured() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    assert!(!wm.which_key_visible(), "never visible outside a prefix mode");
    enter_via_leader(&mut wm);
    wm.config_mut().keys.which_key_delay_ms = 3_600_000;
    assert!(!wm.which_key_visible(), "not before the delay");
    typ(&mut wm, '?');
    assert!(wm.which_key_pinned() && wm.which_key_visible(), "? pins at once");
    assert!(wm.key_mode().is_window(), "? does not leave the mode");
    press(&mut wm, KeyCode::Esc);
    assert!(!wm.which_key_visible() && !wm.which_key_pinned());

    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    wm.config_mut().keys.which_key_delay_ms = 0;
    enter_via_leader(&mut wm);
    assert!(wm.which_key_visible(), "zero delay shows at once");
    wm.config_mut().keys.which_key = false;
    assert!(!wm.which_key_visible(), "which_key = false disables the timer");
    wm.pin_which_key();
    assert!(wm.which_key_visible(), "an explicit pin still works");
}

fn overlay(env: KeyEnv, profile: KeyProfileName, window: bool) -> String {
    let mut wm = wm3(env, profile);
    if window {
        wm.apply(Action::WindowMode(WindowModeOp::Enter)).unwrap();
    } else {
        wm.set_leader_pending(true);
    }
    wm.pin_which_key();
    assert!(wm.which_key_visible());
    render(&mut wm)
}

#[test]
fn snapshot_leader_overlay_linux_tui() {
    insta::assert_snapshot!("leader_overlay_linux_tui", overlay(linux_tui(), KeyProfileName::Auto, false));
}

#[test]
fn snapshot_leader_overlay_macos_gui() {
    insta::assert_snapshot!("leader_overlay_macos_gui", overlay(KeyEnv::new(HostOs::MacOs, Surface::Gui), KeyProfileName::Auto, false));
}

#[test]
fn snapshot_window_overlay_linux_tui() {
    insta::assert_snapshot!("window_overlay_linux_tui", overlay(linux_tui(), KeyProfileName::Auto, true));
}

#[test]
fn snapshot_window_overlay_macos_gui() {
    insta::assert_snapshot!("window_overlay_macos_gui", overlay(KeyEnv::new(HostOs::MacOs, Surface::Gui), KeyProfileName::Auto, true));
}

// ------------------------------------------------------------------ menu bar

use bilterm::ui::menu_model::{MENUBAR_CATEGORIES, category_by_mnemonic, item_by_mnemonic, item_hint, mnemonic_index};
use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_item_command};

fn category_index(title: &str) -> usize {
    MENUBAR_CATEGORIES.iter().position(|c| c.title.contains(title)).unwrap_or_else(|| panic!("category {title}"))
}

fn item_index(cat: usize, command: &str) -> usize {
    MENUBAR_CATEGORIES[cat].items.iter().position(|i| i.command == command).unwrap_or_else(|| panic!("item {command}"))
}

#[test]
fn menubar_all_categories_reachable_by_keyboard() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    leader(&mut wm);
    typ(&mut wm, 'm');
    assert_eq!(wm.menubar_state(), Some((0, 0)), "leader m opens the first category");
    let mut seen = vec![0usize];
    for _ in 0..MENUBAR_CATEGORIES.len() - 1 {
        press(&mut wm, KeyCode::Right);
        seen.push(wm.menubar_open_category().expect("menu stays open"));
    }
    let expected: Vec<usize> = (0..MENUBAR_CATEGORIES.len()).collect();
    assert_eq!(seen, expected, "Right visits every category in order");
    assert!(seen.contains(&category_index("Daemon")) && seen.contains(&category_index("Web")));
    press(&mut wm, KeyCode::Right);
    assert_eq!(wm.menubar_open_category(), Some(0), "wraps over the whole table (no literal 7)");
    press(&mut wm, KeyCode::Left);
    assert_eq!(wm.menubar_open_category(), Some(MENUBAR_CATEGORIES.len() - 1));
    press(&mut wm, KeyCode::Esc);
    assert_eq!(wm.menubar_state(), None);
}

#[test]
fn menubar_items_move_with_arrows_home_and_end() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    wm.apply(Action::MenuOpen { category: Some(category_index("Window")) }).unwrap();
    let cat = category_index("Window");
    let last = menubar_category_item_count(cat) - 1;
    press(&mut wm, KeyCode::End);
    assert_eq!(wm.menubar_state(), Some((cat, last)));
    press(&mut wm, KeyCode::Down);
    assert_eq!(wm.menubar_state(), Some((cat, last)), "Down stops at the last item");
    press(&mut wm, KeyCode::Home);
    press(&mut wm, KeyCode::Down);
    assert_eq!(wm.menubar_state(), Some((cat, 1)));
}

#[test]
fn menubar_mnemonics_unique_per_level() {
    let mut cats: Vec<char> = Vec::new();
    for cat in MENUBAR_CATEGORIES {
        let m = cat.mnemonic.to_ascii_uppercase();
        assert!(!cats.contains(&m), "duplicate category mnemonic {m}");
        cats.push(m);
        assert!(mnemonic_index(cat.title, cat.mnemonic).is_some(), "{} has no letter {}", cat.title, cat.mnemonic);
        let mut items: Vec<char> = Vec::new();
        for item in cat.items {
            let m = item.mnemonic.to_ascii_uppercase();
            assert!(!items.contains(&m), "duplicate item mnemonic {m} in {}", cat.title);
            items.push(m);
            assert!(mnemonic_index(item.label, item.mnemonic).is_some(), "{:?} has no letter {}", item.label, item.mnemonic);
        }
    }
    assert_eq!(category_by_mnemonic('x'), Some(category_index("Tmux")));
    assert_eq!(item_by_mnemonic(category_index("Window"), 's'), Some(item_index(category_index("Window"), ":layout stack")));
}

#[test]
fn menu_stack_windows_selects_stacked_layout() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    leader(&mut wm);
    typ(&mut wm, 'm');
    // `w` is not an item mnemonic of System, so it opens the Window category; `s` then picks Stack Windows.
    typ(&mut wm, 'w');
    assert_eq!(wm.menubar_open_category(), Some(category_index("Window")));
    typ(&mut wm, 's');
    assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
    assert_eq!(wm.menubar_state(), None, "the menu closes after the pick");
}

#[test]
fn menu_window_mode_item_enters_the_same_mode() {
    let mut wm = wm3(linux_tui(), KeyProfileName::Linux);
    let cat = category_index("Window");
    wm.apply(Action::MenuOpen { category: Some(cat) }).unwrap();
    typ(&mut wm, 'm');
    assert_eq!(wm.key_mode(), KeyMode::Window(WinSub::Focus));
    // Enter on the highlighted item works too.
    let mut wm2 = wm3(linux_tui(), KeyProfileName::Linux);
    wm2.apply(Action::MenuOpen { category: Some(cat) }).unwrap();
    for _ in 0..item_index(cat, ":wmode") {
        press(&mut wm2, KeyCode::Down);
    }
    press(&mut wm2, KeyCode::Enter);
    assert_eq!(wm2.key_mode(), wm.key_mode());
}

#[test]
fn menu_hint_uses_platform_glyphs() {
    let window = category_index("Window");
    let toggle = &MENUBAR_CATEGORIES[window].items[item_index(window, ":layout cycle")];
    let stack = &MENUBAR_CATEGORIES[window].items[item_index(window, ":layout stack")];
    let hint = |os: HostOs, surface: Surface, item| {
        let wm = wm3(KeyEnv::new(os, surface), KeyProfileName::Auto);
        item_hint(&wm.keymap(), "Ctrl+X", item)
    };
    assert_eq!(hint(HostOs::MacOs, Surface::Gui, toggle).as_deref(), Some("⌘L"));
    assert_eq!(hint(HostOs::Windows, Surface::Gui, toggle).as_deref(), Some("Ctrl+Shift+L"));
    assert_eq!(hint(HostOs::Linux, Surface::Gui, toggle).as_deref(), Some("Ctrl+Shift+L"));
    assert_eq!(hint(HostOs::Linux, Surface::Tui, toggle).as_deref(), Some("Ctrl+X M"));
    assert_eq!(hint(HostOs::Linux, Surface::Tui, stack).as_deref(), Some("Ctrl+X w s"));
    assert_eq!(hint(HostOs::MacOs, Surface::Gui, stack).as_deref(), Some("Ctrl+X w s"));
    let wmode = &MENUBAR_CATEGORIES[window].items[item_index(window, ":wmode")];
    assert_eq!(hint(HostOs::MacOs, Surface::Gui, wmode).as_deref(), Some("⌘O"));
    assert_eq!(hint(HostOs::Linux, Surface::Tui, wmode).as_deref(), Some("Ctrl+X w"));
}

#[test]
fn menu_labels_carry_no_key_text_and_dropdown_renders_computed_hints() {
    for cat in MENUBAR_CATEGORIES {
        for item in cat.items {
            assert!(!item.label.contains('('), "{:?} embeds key text", item.label);
        }
    }
    let mut wm = wm3(KeyEnv::new(HostOs::MacOs, Surface::Gui), KeyProfileName::Auto);
    wm.config_mut().show_top_menubar = true;
    wm.apply(Action::MenuOpen { category: Some(category_index("Window")) }).unwrap();
    let screen = render(&mut wm);
    let line = |needle: &str| screen.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle} not drawn:\n{screen}")).to_string();
    assert!(line("Toggle Stack/Tile").contains("⌘L"));
    assert!(line("Stack Windows").contains("Ctrl+X w s"));
    assert!(line("Window Mode").contains("⌘O"));
    assert!(!screen.contains("Alt+"), "no Alt chord advertised on macOS");
}

#[test]
fn every_menu_command_parses() {
    for (c, cat) in MENUBAR_CATEGORIES.iter().enumerate() {
        for (i, item) in cat.items.iter().enumerate() {
            assert_eq!(menubar_item_command(c, i), Some(item.command));
            assert!(parse_colon_command(item.command).is_some_and(|r| r.is_ok()), "{}", item.command);
        }
    }
}

/// `true` when `text` names the bare `Alt+M` chord (not `Ctrl+Alt+M`, `Alt+Shift+M` or `Alt+Minus`).
fn contains_alt_m(text: &str) -> bool {
    text.match_indices("Alt+").any(|(i, _)| {
        let before_ok = !text[..i].ends_with('+');
        let mut after = text[i + 4..].chars();
        let is_m = matches!(after.next(), Some('M' | 'm'));
        before_ok && is_m && !after.next().is_some_and(char::is_alphanumeric)
    })
}

fn scan_docs(dir: &std::path::Path, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_docs(&path, hits);
        } else if path.extension().is_some_and(|e| e == "md") {
            scan_file(&path, hits);
        }
    }
}

fn scan_file(path: &std::path::Path, hits: &mut Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let mut legacy = false;
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') {
            legacy = line.contains("Legacy");
        }
        if !legacy && contains_alt_m(line) {
            hits.push(format!("{}:{}", path.display(), n + 1));
        }
    }
}

#[test]
fn no_alt_m_text_outside_legacy_section() {
    assert!(contains_alt_m("press Alt+M now") && contains_alt_m("Alt+m") && !contains_alt_m("Alt+Minus"));
    assert!(!contains_alt_m("Alt+Shift+M") && !contains_alt_m("Ctrl+Alt+M"));
    let mut hits: Vec<String> = Vec::new();
    for (cmd, keys, desc) in PALETTE_CATALOG {
        if contains_alt_m(cmd) || contains_alt_m(keys) || contains_alt_m(desc) {
            hits.push(format!("PALETTE_CATALOG {cmd}"));
        }
    }
    for (keys, desc) in HELP_LINES {
        if contains_alt_m(keys) || contains_alt_m(desc) {
            hits.push(format!("HELP_LINES {keys}"));
        }
    }
    for env in [KeyEnv::new(HostOs::MacOs, Surface::Gui), KeyEnv::new(HostOs::Windows, Surface::Gui), linux_tui()] {
        let wm = wm3(env, KeyProfileName::Auto);
        for cat in MENUBAR_CATEGORIES {
            for item in cat.items {
                let hint = item_hint(&wm.keymap(), "Ctrl+X", item).unwrap_or_default();
                if contains_alt_m(item.label) || contains_alt_m(&hint) {
                    hits.push(format!("menu {} {hint}", item.label));
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    scan_file(&root.join("README.md"), &mut hits);
    scan_docs(&root.join("docs"), &mut hits);
    assert!(hits.is_empty(), "Alt+M named outside a Legacy section: {hits:#?}");
}

fn menu_overlay(env: KeyEnv, category: &str) -> String {
    let mut wm = wm3(env, KeyProfileName::Auto);
    wm.apply(Action::MenuOpen { category: Some(category_index(category)) }).unwrap();
    render(&mut wm)
}

#[test]
fn snapshot_window_menu_linux_tui() {
    insta::assert_snapshot!("window_menu_linux_tui", menu_overlay(linux_tui(), "Window"));
}

#[test]
fn snapshot_window_menu_macos_gui() {
    insta::assert_snapshot!("window_menu_macos_gui", menu_overlay(KeyEnv::new(HostOs::MacOs, Surface::Gui), "Window"));
}

#[test]
fn snapshot_view_menu_windows_gui() {
    insta::assert_snapshot!("view_menu_windows_gui", menu_overlay(KeyEnv::new(HostOs::Windows, Surface::Gui), "View"));
}
