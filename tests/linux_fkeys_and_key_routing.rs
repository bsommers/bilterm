//! Regression and full-matrix tests for `<F7>`, `F1..F12` / special key routing, and
//! `ScriptStudio` window controls across normal mode, `ALT_SCREEN`, `RawPtyPassthrough`,
//! `Fixed`/`AutoHide` drawer modes, focused drawer (`Classic` and `Navigator`), utility windows,
//! Modern/Legacy bottom F-key bar clicks, top pill bar clicks, `KeyEventKind::Repeat`, and
//! Linux X11 `KeySym` translation.

use bilterm::action::Action;
use bilterm::app::App;
use bilterm::config::BiltermConfig;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pane::PaneScrollMode;
use bilterm::ui::RawFkeyMode;
use bilterm::ui::design::AppearanceProfile;
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::agents_list_cmd::AgentsListOp;
use bilterm::wm::command_window::CommandWindowOp;
use bilterm::wm::drawer::DrawerStyle;
use bilterm::wm::fonts_window::FontsWindowOp;
use bilterm::wm::script_studio::ScriptStudioOp;
use bilterm::wm::session_map::SessionMapOp;
use bilterm::wm::{DrawerVisibilityMode, Modal, WindowManager, WorkspaceLayoutMode};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

fn fkey(n: u8) -> KeyEvent {
    KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE)
}

fn shift_fkey(n: u8) -> KeyEvent {
    KeyEvent::new(KeyCode::F(n), KeyModifiers::SHIFT)
}

fn alt_fkey(n: u8) -> KeyEvent {
    KeyEvent::new(KeyCode::F(n), KeyModifiers::ALT)
}

#[test]
fn f7_opens_and_closes_drawer_repeatedly_in_alt_screen_auto_and_bilterm_modes() {
    let mut wm = WindowManager::new_headless(25, 100);
    // Enter DECSET 1049 alternate screen (like vim, htop, herdr, claude, codex)
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h");
    assert!(wm.active_pane().vt().is_alternate_screen());
    assert_eq!(wm.config().raw_fkeys, RawFkeyMode::Auto);
    assert!(!wm.drawer_is_open());

    for _ in 0..4 {
        handle_wm_key_event(&mut wm, fkey(7)).unwrap();
        assert!(wm.drawer_is_open(), "F7 must open drawer in ALT_SCREEN");
        assert!(wm.drawer().focused, "opening drawer focuses it");

        // Unfocus drawer while keeping it open (e.g., user clicked back into the ALT_SCREEN pane)
        wm.drawer_mut().focused = false;
        handle_wm_key_event(&mut wm, fkey(7)).unwrap();
        assert!(
            !wm.drawer_is_open(),
            "F7 must close drawer when unfocused in ALT_SCREEN"
        );
    }

    // Shift+F7 in ALT_SCREEN still forwards literal \x1b[18~ to the PTY without toggling the drawer
    handle_wm_key_event(&mut wm, shift_fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // Under RawFkeyMode::App, plain F7 is forwarded to the application unless the drawer is open
    wm.config_mut().raw_fkeys = RawFkeyMode::App;
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open(), "RawFkeyMode::App forwards F7 to PTY");
    // Alt+F7 still opens the drawer even in RawFkeyMode::App
    handle_wm_key_event(&mut wm, alt_fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    wm.drawer_mut().focused = false;
    // And once open, plain F7 still closes it
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());
}

#[test]
fn f7_closes_fixed_and_autohide_drawer_on_first_press() {
    // 1. Fixed mode from config startup (where drawer.open was previously false)
    let mut wm = WindowManager::new_headless(25, 100);
    let mut cfg = BiltermConfig::default();
    cfg.drawer.visibility = DrawerVisibilityMode::Fixed;
    wm.apply_config(cfg);
    assert!(wm.is_drawer_effectively_visible());

    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(
        !wm.is_drawer_effectively_visible(),
        "First F7 press must close Fixed drawer immediately"
    );

    // Also test if visibility was mutated to Fixed directly without setting drawer.open
    wm.config_mut().drawer.visibility = DrawerVisibilityMode::Fixed;
    wm.drawer_mut().open = false;
    wm.drawer_mut().focused = false;
    assert!(wm.is_drawer_effectively_visible());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(
        !wm.is_drawer_effectively_visible(),
        "First F7 press must close Fixed drawer even when drawer.open was false"
    );

    // 2. AutoHide mode when hover_active is true
    wm.config_mut().drawer.visibility = DrawerVisibilityMode::AutoHide;
    wm.drawer_mut().open = false;
    wm.drawer_mut().focused = false;
    wm.drawer_mut().hover_active = true;
    assert!(wm.is_drawer_effectively_visible());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(
        !wm.is_drawer_effectively_visible(),
        "First F7 press must close AutoHide hovered drawer"
    );
}

#[test]
fn f7_toggles_drawer_even_when_utility_windows_or_mcp_console_are_focused() {
    let mut wm = WindowManager::new_headless(30, 120);

    // 1. Fonts window focused
    wm.apply(Action::FontsWindow(FontsWindowOp::Open)).unwrap();
    assert!(wm.fonts_window().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(wm.drawer().focused);
    assert!(
        !wm.fonts_window().captures_input(),
        "focusing drawer blurs fonts_window"
    );
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 2. Command window focused
    wm.apply(Action::CommandWindow(CommandWindowOp::Open))
        .unwrap();
    assert!(wm.command_window().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(!wm.command_window().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 3. Script studio focused
    wm.apply(Action::ScriptStudio(ScriptStudioOp::Open)).unwrap();
    assert!(wm.script_studio().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(!wm.script_studio().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 4. Agents list focused
    wm.apply(Action::AgentsList(AgentsListOp::Open)).unwrap();
    assert!(wm.agents_list().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(!wm.agents_list().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 5. Session map focused
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();
    assert!(wm.session_map().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(!wm.session_map().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 6. Agents traffic window focused
    wm.apply(Action::Agents(AgentsOp::Zoom(None))).unwrap();
    assert!(wm.agents_window_captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    assert!(!wm.agents_window_captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());

    // 7. MCP console focused
    wm.apply(Action::McpCommand("open".to_string())).unwrap();
    assert!(wm.mcp_console().captures_input());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());
}

#[test]
fn focused_drawer_does_not_swallow_other_global_function_keys_or_shortcuts() {
    for style in [DrawerStyle::Classic, DrawerStyle::Navigator] {
        let mut wm = WindowManager::new_headless(30, 120);
        wm.config_mut().drawer.style = style;
        handle_wm_key_event(&mut wm, fkey(7)).unwrap();
        assert!(wm.drawer_is_open() && wm.drawer().focused);

        // F1 opens Help modal even while drawer is focused
        handle_wm_key_event(&mut wm, fkey(1)).unwrap();
        assert!(
            wm.is_help_visible(),
            "F1 must open Help while {style:?} drawer is focused"
        );
        handle_wm_key_event(&mut wm, fkey(1)).unwrap();
        assert!(!wm.is_help_visible());

        // F2 splits pane horizontally even while drawer is focused
        assert_eq!(wm.pane_count(), 1);
        handle_wm_key_event(&mut wm, fkey(2)).unwrap();
        assert_eq!(
            wm.pane_count(),
            2,
            "F2 must split pane while {style:?} drawer is focused"
        );

        // F4 cycles prompt mode (Floating -> Fixed -> Regular -> Floating), and Alt+M toggles Tiled/Stacked layout mode
        let mode_before = (
            wm.active_pane().prompt().anchor,
            wm.active_pane().prompt().manual_raw_override,
        );
        handle_wm_key_event(&mut wm, fkey(4)).unwrap();
        assert_ne!(
            (
                wm.active_pane().prompt().anchor,
                wm.active_pane().prompt().manual_raw_override,
            ),
            mode_before,
            "F4 must cycle prompt mode while {style:?} drawer is focused"
        );
        handle_wm_key_event(&mut wm, fkey(4)).unwrap();
        handle_wm_key_event(&mut wm, fkey(4)).unwrap();
        assert_eq!(
            (
                wm.active_pane().prompt().anchor,
                wm.active_pane().prompt().manual_raw_override,
            ),
            mode_before
        );

        assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT),
        )
        .unwrap();
        assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Stacked);
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT),
        )
        .unwrap();
        assert_eq!(wm.layout_mode(), WorkspaceLayoutMode::Tiled);

        // F6 shuffles deck
        let top_before = wm.active_pane_id();
        handle_wm_key_event(&mut wm, fkey(6)).unwrap();
        assert_ne!(
            wm.active_pane_id(),
            top_before,
            "F6 must cycle deck while drawer is focused"
        );

        // F8 toggles copy mode on active pane
        handle_wm_key_event(&mut wm, fkey(8)).unwrap();
        assert!(
            wm.active_pane().copy_mode().active,
            "F8 must toggle copy mode"
        );
        handle_wm_key_event(&mut wm, fkey(8)).unwrap();
        assert!(!wm.active_pane().copy_mode().active);

        // F9 toggles Inspector modal
        handle_wm_key_event(&mut wm, fkey(9)).unwrap();
        assert!(
            wm.is_inspector_visible(),
            "F9 must open Inspector while drawer is focused"
        );
        handle_wm_key_event(&mut wm, fkey(9)).unwrap();
        assert!(!wm.is_inspector_visible());

        // F10 toggles Quake drop-down terminal
        handle_wm_key_event(&mut wm, fkey(10)).unwrap();
        assert!(
            wm.is_quick_terminal_visible(),
            "F10 must toggle Quake terminal"
        );
        handle_wm_key_event(&mut wm, fkey(10)).unwrap();
        assert!(!wm.is_quick_terminal_visible());

        // F12 toggles manual raw override
        let raw_before = wm.active_pane().prompt().manual_raw_override;
        handle_wm_key_event(&mut wm, fkey(12)).unwrap();
        assert_ne!(wm.active_pane().prompt().manual_raw_override, raw_before);
        handle_wm_key_event(&mut wm, fkey(12)).unwrap();

        // Ctrl+P opens Command Palette while drawer is focused
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        )
        .unwrap();
        assert!(matches!(wm.modal(), Modal::Palette { .. }));
        wm.close_modal();
    }
}

#[test]
fn f7_closes_open_drawer_even_when_block_page_hold_is_active() {
    let mut wm = WindowManager::new_headless(20, 80);
    wm.active_pane_mut().set_scroll_mode(PaneScrollMode::BlockPage);
    for i in 0..40 {
        wm.active_pane_mut()
            .process_pty_output(format!("line {i}\r\n").as_bytes());
    }
    assert!(wm.active_pane().is_page_holding());

    // Open drawer with Alt+F7
    handle_wm_key_event(&mut wm, alt_fkey(7)).unwrap();
    assert!(wm.drawer_is_open());
    wm.drawer_mut().focused = false;

    // Plain F7 must close the open drawer first rather than being hijacked by ScrollPageBack
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());
}

#[test]
fn modern_and_legacy_bottom_fkey_bar_clicks_and_top_pill_bar_clicks_route_correctly() {
    let area = Rect::new(0, 0, 100, 25);

    // 1. Modern (MODERN-SLATE via Obsidian profile) bottom F-key bar click on "F7 Drawer" (cols 34..=46)
    let mut wm = WindowManager::new_headless(25, 100);
    let mut cfg = BiltermConfig::default();
    cfg.appearance.profile = Some(AppearanceProfile::Obsidian);
    cfg.show_dos_fkey_bar = true;
    wm.apply_config(cfg);
    wm.resize_all(area).unwrap();

    let click_f7 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 38,
        row: 24,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_f7, area).unwrap();
    assert!(
        wm.drawer_is_open(),
        "Clicking 'F7 Drawer' on Modern bottom bar must open drawer"
    );
    handle_wm_mouse_event(&mut wm, click_f7, area).unwrap();
    assert!(
        !wm.drawer_is_open(),
        "Clicking 'F7 Drawer' again must close drawer"
    );

    // Clicking "F12 Raw" (cols 60..=68) on Modern bottom bar toggles raw override
    let raw_before = wm.active_pane().prompt().manual_raw_override;
    let click_f12 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 63,
        row: 24,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_f12, area).unwrap();
    assert_ne!(wm.active_pane().prompt().manual_raw_override, raw_before);

    // 2. Top pill bar click blurs focused drawer so typing goes to the clicked pane
    wm.config_mut().show_top_menubar = false;
    wm.config_mut().appearance.profile = Some(AppearanceProfile::Obsidian);
    wm.split_focused(bilterm::wm::split_tree::SplitDirection::Horizontal)
        .unwrap();
    wm.resize_all(area).unwrap();
    wm.toggle_drawer();
    assert!(wm.drawer().focused);

    let click_pill = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 4,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_pill, area).unwrap();
    assert!(
        !wm.drawer().focused,
        "Clicking a pane pill in the top bar must blur drawer focus"
    );
}

#[test]
fn script_studio_window_controls_badges_toolbar_footer_and_drag_all_work() {
    let area = Rect::new(0, 0, 120, 40);
    let mut wm = WindowManager::new_headless(40, 120);
    wm.resize_all(area).unwrap();
    wm.apply(Action::ScriptStudio(ScriptStudioOp::Open)).unwrap();
    wm.apply(Action::ScriptStudio(ScriptStudioOp::ClearScript))
        .unwrap();
    wm.apply(Action::ScriptStudio(ScriptStudioOp::AppendLine(
        ":font vt323".to_string(),
    )))
    .unwrap();
    wm.apply(Action::ScriptStudio(ScriptStudioOp::AppendLine(
        ":font size 18".to_string(),
    )))
    .unwrap();

    let outer = wm.script_studio_rect(area).unwrap();
    let r_edge = outer.x + outer.width;

    let click = |wm: &mut WindowManager, col: u16, row: u16| {
        handle_wm_mouse_event(
            wm,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    };

    // 1. Click Pin badge [•] at r_edge - 10 (toggles pinned)
    assert!(wm.script_studio().pinned);
    click(&mut wm, r_edge - 10, outer.y);
    assert!(!wm.script_studio().pinned, "Pin badge click must unpin");
    click(&mut wm, r_edge - 10, outer.y);
    assert!(wm.script_studio().pinned, "Pin badge click must re-pin");

    // 2. Click Dock badge [□] at r_edge - 7 (toggles docked)
    assert!(!wm.script_studio().docked);
    click(&mut wm, r_edge - 7, outer.y);
    assert!(wm.script_studio().docked, "Dock badge click must dock");
    // Dragging title bar while docked undocks and moves the window
    let docked_outer = wm.script_studio_rect(area).unwrap();
    click(&mut wm, docked_outer.x + 4, docked_outer.y);
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 20,
            row: 6,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 20,
            row: 6,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(
        !wm.script_studio().docked,
        "Dragging docked Script Studio title bar must undock and move it"
    );

    // 3. Toolbar buttons on row = outer.y + 1
    let outer = wm.script_studio_rect(area).unwrap();
    let tb_y = outer.y + 1;
    // [▶ Run(F5)] at rel_x 1..=11
    click(&mut wm, outer.x + 1 + 4, tb_y);
    assert_eq!(wm.script_studio().run_log.len(), 2, "Run button must execute all lines");

    // [↺ Reset] at rel_x 27..=35
    click(&mut wm, outer.x + 1 + 30, tb_y);
    assert!(
        wm.script_studio().run_log.is_empty(),
        "Reset button must clear run log"
    );

    // [» Step(F10)] at rel_x 13..=25
    click(&mut wm, outer.x + 1 + 18, tb_y);
    assert_eq!(wm.script_studio().run_log.len(), 1, "Step button must execute 1 line");

    // [● Rec] at rel_x 37..=43
    assert!(!wm.script_studio().recording);
    click(&mut wm, outer.x + 1 + 40, tb_y);
    assert!(wm.script_studio().recording, "Rec button must toggle recording on");
    click(&mut wm, outer.x + 1 + 40, tb_y);
    assert!(!wm.script_studio().recording);

    // [✎ Edit] at rel_x 45..=52
    assert!(!wm.script_studio().edit_mode);
    click(&mut wm, outer.x + 1 + 48, tb_y);
    assert!(wm.script_studio().edit_mode, "Edit button must enter edit mode");
    click(&mut wm, outer.x + 1 + 48, tb_y);
    assert!(!wm.script_studio().edit_mode, "Edit button must exit edit mode");

    // 4. Editor line click selects line, sets cursor_col, and second click enters edit mode
    let editor_y = outer.y + 3;
    click(&mut wm, outer.x + 1 + 10, editor_y + 1);
    assert_eq!(wm.script_studio().cursor_line, 1);
    assert!(!wm.script_studio().edit_mode);
    click(&mut wm, outer.x + 1 + 10, editor_y + 1);
    assert!(
        wm.script_studio().edit_mode,
        "Clicking an already-selected line in Script Studio must enter edit mode"
    );
    // Esc exits edit mode without closing the window
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(!wm.script_studio().edit_mode);
    assert!(wm.script_studio().active);

    // 5. Keyboard '*' with KeyModifiers::SHIFT (as sent by X11/crossterm for Shift+8) toggles pin
    let pinned_before = wm.script_studio().pinned;
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('*'), KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_ne!(
        wm.script_studio().pinned,
        pinned_before,
        "'*' with SHIFT modifier must toggle pin"
    );

    // 6. Footer hotkey clicks on row = outer.y + outer.height - 2
    // FOOTER_TEXT = "F5:Run F10/n:Step r:Reset R:Rec i:Edit x:Del o:New s:Save *:Pin"
    let footer_y = outer.y + outer.height - 2;
    // Click "r:Reset" (rel_x 18..=24)
    click(&mut wm, outer.x + 1 + 20, footer_y);
    assert!(wm.script_studio().run_log.is_empty());
    // Click "o:New" (rel_x 45..=49)
    let lines_before = wm.script_studio().lines.len();
    click(&mut wm, outer.x + 1 + 46, footer_y);
    assert_eq!(wm.script_studio().lines.len(), lines_before + 1);
    assert!(wm.script_studio().edit_mode);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    // Click "x:Del" (rel_x 39..=43)
    click(&mut wm, outer.x + 1 + 40, footer_y);
    assert_eq!(wm.script_studio().lines.len(), lines_before);

    // 7. Click Close badge [X] at r_edge - 4 (the 'X' glyph itself!)
    let r_edge = outer.x + outer.width;
    click(&mut wm, r_edge - 4, outer.y);
    assert!(
        !wm.script_studio().active,
        "Clicking 'X' on Close badge must close Script Studio"
    );
}

#[test]
fn app_handles_both_key_press_and_key_repeat_events() {
    let wm = WindowManager::new_headless(25, 80);
    let mut app = App {
        wm,
        should_quit: false,
        needs_redraw: false,
        last_viewport: Rect::new(0, 0, 80, 25),
    };
    let mut repeat_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    repeat_key.kind = KeyEventKind::Repeat;
    app.handle_crossterm_event(Event::Key(repeat_key)).unwrap();
    assert_eq!(app.wm.active_pane().prompt().text(), "a");
}

fn repeat(mut key: KeyEvent) -> KeyEvent {
    key.kind = KeyEventKind::Repeat;
    key
}

fn headless_app(cols: u16, rows: u16) -> App {
    App {
        wm: WindowManager::new_headless(rows, cols),
        should_quit: false,
        needs_redraw: false,
        last_viewport: Rect::new(0, 0, cols, rows),
    }
}

#[test]
fn holding_f7_press_then_repeat_toggles_drawer_exactly_once() {
    let mut app = headless_app(80, 25);
    assert!(!app.wm.drawer_is_open());
    app.handle_crossterm_event(Event::Key(fkey(7))).unwrap();
    assert!(app.wm.drawer_is_open(), "F7 press opens the drawer");
    for _ in 0..5 {
        app.handle_crossterm_event(Event::Key(repeat(fkey(7)))).unwrap();
        assert!(app.wm.drawer_is_open(), "F7 auto-repeat must not flicker the drawer closed");
    }
    // Alt+F7 auto-repeat is ignored too; the next real press closes the drawer.
    app.handle_crossterm_event(Event::Key(repeat(alt_fkey(7)))).unwrap();
    assert!(app.wm.drawer_is_open());
    app.handle_crossterm_event(Event::Key(fkey(7))).unwrap();
    assert!(!app.wm.drawer_is_open(), "a fresh F7 press still closes the drawer");

    // Other WM toggles behave the same: F5 zoom and Ctrl+P palette fire once per press.
    handle_wm_key_event(&mut app.wm, KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)).unwrap();
    assert_eq!(app.wm.pane_count(), 2);
    app.handle_crossterm_event(Event::Key(repeat(fkey(2)))).unwrap();
    assert_eq!(app.wm.pane_count(), 2, "holding F2 must not keep splitting");
    app.handle_crossterm_event(Event::Key(fkey(5))).unwrap();
    let zoomed = app.wm.zoomed_pane();
    assert!(zoomed.is_some());
    app.handle_crossterm_event(Event::Key(repeat(fkey(5)))).unwrap();
    assert_eq!(app.wm.zoomed_pane(), zoomed, "holding F5 must not unzoom");
    let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.handle_crossterm_event(Event::Key(ctrl_p)).unwrap();
    assert!(matches!(app.wm.modal(), Modal::Palette { .. }));
    app.handle_crossterm_event(Event::Key(repeat(ctrl_p))).unwrap();
    assert!(matches!(app.wm.modal(), Modal::Palette { .. }), "holding Ctrl+P must not close the palette");
}

#[test]
fn pane_bound_keys_keep_auto_repeating() {
    let mut app = headless_app(80, 25);
    for c in "abcd".chars() {
        app.handle_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))
            .unwrap();
    }
    let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    app.handle_crossterm_event(Event::Key(backspace)).unwrap();
    app.handle_crossterm_event(Event::Key(repeat(backspace))).unwrap();
    assert_eq!(app.wm.active_pane().prompt().text(), "ab", "Backspace auto-repeat is forwarded");

    let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    app.handle_crossterm_event(Event::Key(left)).unwrap();
    app.handle_crossterm_event(Event::Key(repeat(left))).unwrap();
    app.handle_crossterm_event(Event::Key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE)))
        .unwrap();
    assert_eq!(app.wm.active_pane().prompt().text(), "Xab", "Left-arrow auto-repeat is forwarded");
}

#[test]
fn block_page_hold_f7_f8_repeat_keeps_paging_and_f7_still_closes_open_drawer() {
    let mut wm = WindowManager::new_headless(20, 80);
    wm.active_pane_mut().set_scroll_mode(PaneScrollMode::BlockPage);
    for i in 0..40 {
        wm.active_pane_mut()
            .process_pty_output(format!("line {i}\r\n").as_bytes());
    }
    assert!(wm.active_pane().is_page_holding());
    // F8 auto-repeat while holding is paging (not a toggle): it must not open Copy Mode.
    handle_wm_key_event(&mut wm, repeat(fkey(8))).unwrap();
    assert!(!wm.active_pane().copy_mode().active);
    // Alt+F7 opens the drawer; its auto-repeat is dropped; plain F7 closes it.
    handle_wm_key_event(&mut wm, alt_fkey(7)).unwrap();
    handle_wm_key_event(&mut wm, repeat(alt_fkey(7))).unwrap();
    assert!(wm.drawer_is_open());
    wm.drawer_mut().focused = false;
    handle_wm_key_event(&mut wm, fkey(7)).unwrap();
    assert!(!wm.drawer_is_open());
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn linux_x11_keysym_translator_handles_all_fkeys_locks_keypad_and_alt_shift() {
    use bilterm::ui::gui_font::linux_x11::translate_x11_keysym_and_bytes;

    // F1..F12 (0xFFBE..=0xFFC9) with plain, NumLock (Mod2Mask = 1<<4), CapsLock (LockMask = 1<<1), Shift, Alt, Ctrl
    for n in 1u8..=12 {
        let ks = 0xFFBE + u32::from(n - 1);
        assert_eq!(
            translate_x11_keysym_and_bytes(ks, 0, &[]),
            Some(KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE))
        );
        // NumLock + CapsLock active on Linux X11 must not pollute KeyModifiers on F1..F12
        assert_eq!(
            translate_x11_keysym_and_bytes(ks, (1 << 1) | (1 << 4), &[]),
            Some(KeyEvent::new(KeyCode::F(n), KeyModifiers::NONE))
        );
        assert_eq!(
            translate_x11_keysym_and_bytes(ks, 1 << 0, &[]),
            Some(KeyEvent::new(KeyCode::F(n), KeyModifiers::SHIFT))
        );
        assert_eq!(
            translate_x11_keysym_and_bytes(ks, 1 << 3, &[]),
            Some(KeyEvent::new(KeyCode::F(n), KeyModifiers::ALT))
        );
    }

    // Alt+Shift+H/J/K/L must preserve uppercase char so PaneResize bindings match instead of SplitHorizontal
    let shift_alt = (1 << 0) | (1 << 3);
    let ev = translate_x11_keysym_and_bytes(b'H' as u32, shift_alt, b"H").unwrap();
    assert_eq!(
        ev,
        KeyEvent::new(KeyCode::Char('H'), KeyModifiers::ALT | KeyModifiers::SHIFT)
    );
    let mut wm = WindowManager::new_headless(25, 80);
    handle_wm_key_event(&mut wm, ev).unwrap();
    assert_eq!(
        wm.pane_count(),
        1,
        "Alt+Shift+H in X11 must resize pane, not split horizontally"
    );

    // Keypad navigation keys (Home, Left, Up, Right, Down, PageUp, PageDown, End, Insert, Delete, Enter)
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF95, 0, &[]),
        Some(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF96, 0, &[]),
        Some(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF97, 0, &[]),
        Some(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF98, 0, &[]),
        Some(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF99, 0, &[]),
        Some(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF9A, 0, &[]),
        Some(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF9B, 0, &[]),
        Some(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF9C, 0, &[]),
        Some(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF9E, 0, &[]),
        Some(KeyEvent::new(KeyCode::Insert, KeyModifiers::NONE))
    );
    assert_eq!(
        translate_x11_keysym_and_bytes(0xFF9F, 0, &[]),
        Some(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE))
    );
}
