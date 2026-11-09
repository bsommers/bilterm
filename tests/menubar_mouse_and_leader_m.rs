use bilterm::action::Action;
use bilterm::config::keys::KeyProfileName;
use bilterm::input::keymap::default_leader_action;
use bilterm::input::{handle_wm_mouse_event, AppAction};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[test]
fn leader_m_opens_menu_across_all_profiles() {
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE);
    assert_eq!(
        default_leader_action(key),
        Some(Action::MenuOpen { category: None }),
        "default_leader_action should map 'm' to Action::MenuOpen"
    );

    for profile in [
        KeyProfileName::Legacy,
        KeyProfileName::Linux,
        KeyProfileName::Mac,
        KeyProfileName::Windows,
        KeyProfileName::Portable,
    ] {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.set_key_profile(profile);
        assert_eq!(
            wm.keymap().leader_action(key),
            Some(Action::MenuOpen { category: None }),
            "profile {:?} should map leader m to Action::MenuOpen",
            profile
        );
    }
}

#[test]
fn menubar_dropdown_click_over_drawer_executes_menu_action_not_drawer() {
    let mut wm = WindowManager::new_headless(24, 80);
    let screen = Rect::new(0, 0, 80, 24);
    let ws_area = Rect::new(0, 1, 80, 22);

    // Open drawer on left side
    wm.toggle_drawer();
    assert!(wm.is_drawer_effectively_visible());
    let dr = wm.drawer_rect(ws_area).expect("drawer rect must exist");

    // Open menubar category 0 ("≡ System")
    wm.apply(Action::MenuOpen { category: Some(0) }).unwrap();
    assert_eq!(wm.menubar_state(), Some((0, 0)));

    let dropdown = bilterm::ui::modals::menubar_dropdown_rect(0, ws_area);
    let inner = bilterm::wm::inset_window_frame(dropdown);

    // Click item 0 (":inspector") inside the dropdown
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inner.x + 2,
        row: inner.y,
        modifiers: KeyModifiers::NONE,
    };

    // Ensure click is inside dropdown
    assert!(
        click.column >= dropdown.x
            && click.column < dropdown.x + dropdown.width
            && click.row >= dropdown.y
            && click.row < dropdown.y + dropdown.height,
        "click must be within dropdown rect"
    );

    // Ensure click overlaps the drawer rect
    assert!(
        click.column >= dr.x
            && click.column < dr.x + dr.width
            && click.row >= dr.y
            && click.row < dr.y + dr.height,
        "click must overlap drawer rect"
    );

    let initial_visibility = wm.drawer_config().visibility;
    let initial_drawer_index = wm.drawer().selected_index;
    assert!(!wm.is_inspector_visible());

    let res = handle_wm_mouse_event(&mut wm, click, screen).unwrap();
    assert_eq!(res, AppAction::Handled);

    // Menubar dropdown must have executed its action and closed
    assert_eq!(wm.menubar_state(), None, "menubar dropdown should be closed");
    assert!(wm.is_inspector_visible(), "menu action :inspector must have executed");

    // Drawer must NOT have been triggered or mutated
    assert_eq!(
        wm.drawer_config().visibility,
        initial_visibility,
        "drawer visibility must not change from menu click"
    );
    assert_eq!(
        wm.drawer().selected_index,
        initial_drawer_index,
        "drawer selected row must not change from menu click"
    );
    assert!(
        !wm.drawer().resizing_drag,
        "drawer resizing drag must not be triggered"
    );
}
