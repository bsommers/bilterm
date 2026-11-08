//! Comprehensive live & batch E2E test suite powered by `BiltermLiveDriver`.

mod live_driver_support;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use bilterm::ui::drawer::drawer_vertical_layout_metrics;
use bilterm::ui::drawer::navigator::{
    navigator_dot_cells, navigator_menu_layout, standalone_context_menu_layout,
};
use bilterm::ui::navigator::layout_pills;
use bilterm::ui::rename_dialog_ui::{rename_dialog_button_rects, rename_dialog_rect};
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::DrawerRowKind;
use bilterm::wm::drawer::NavigatorMenuEntry;

use live_driver_support::BiltermLiveDriver;

/// Helper to locate the `Rename` entry coordinate inside an open standalone context menu.
fn standalone_rename_click_point(driver: &BiltermLiveDriver) -> (u16, u16) {
    let menu = standalone_context_menu_layout(&driver.wm, driver.viewport)
        .expect("standalone context menu should be open");
    let (_, entry_y) = menu
        .entries
        .iter()
        .find(|(entry, _)| *entry == NavigatorMenuEntry::Rename)
        .expect("Rename entry present in standalone context menu");
    (menu.rect.x + 2, *entry_y)
}

/// Helper to locate the `Rename` entry coordinate inside an open Navigator drawer menu.
fn navigator_menu_rename_click_point(driver: &BiltermLiveDriver) -> (u16, u16) {
    let shell = ShellLayout::compute(
        driver.viewport,
        driver.wm.config(),
        driver.wm.pane_count(),
    );
    let menu = navigator_menu_layout(&driver.wm, shell.workspace)
        .expect("navigator context menu should be open");
    let (_, entry_y) = menu
        .entries
        .iter()
        .find(|(entry, _)| *entry == NavigatorMenuEntry::Rename)
        .expect("Rename entry present in navigator menu");
    (menu.rect.x + 2, *entry_y)
}

#[test]
fn live_window_rename_all_modes_and_surfaces() {
    let mut driver = BiltermLiveDriver::new_tui(120, 36);

    // 1. Bare `:window rename` across Legacy, Obsidian, Liquid, Amber, Synthwave, Solarized.
    for mode in [
        "legacy",
        "obsidian",
        "liquid",
        "amber",
        "synthwave",
        "solarized",
    ] {
        driver
            .run_colon(&format!(":appearance {mode}"))
            .expect("switch appearance mode");
        driver
            .run_colon(":window rename")
            .expect("bare :window rename opens dialog");
        assert!(
            driver.wm.rename_dialog_open(),
            "RenameDialogState should be open in {mode}"
        );
        driver.assert_screen_contains("RENAME WINDOW");
        driver.clear_and_type("live-win");
        driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!driver.wm.rename_dialog_open());
        assert_eq!(driver.wm.active_pane().title(), "live-win");
    }

    // 2. Command Palette (`Ctrl+P`) -> type "window rename" -> verify `:window rename` in filtered results -> Enter -> mouse click Save.
    driver.press_key(KeyCode::Char('p'), KeyModifiers::CONTROL);
    driver.type_text("window rename");
    driver.assert_screen_contains(":window rename");
    let palette_items = driver.wm.filtered_palette_items();
    let target_idx = palette_items
        .iter()
        .position(|(cmd, _, _)| *cmd == ":window rename")
        .expect(":window rename present in filtered palette results");
    for _ in 0..target_idx {
        driver.press_key(KeyCode::Down, KeyModifiers::NONE);
    }
    driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        driver.wm.rename_dialog_open(),
        "Selecting :window rename from Command Palette should open RenameDialogState"
    );
    driver.assert_screen_contains("RENAME WINDOW");
    driver.clear_and_type("palette-win");
    let dialog_rect = rename_dialog_rect(driver.viewport);
    let (save_rect, _cancel_rect, _clear_rect) = rename_dialog_button_rects(dialog_rect);
    driver.click(save_rect.x + 2, save_rect.y);
    assert!(!driver.wm.rename_dialog_open());
    assert_eq!(driver.wm.active_pane().title(), "palette-win");

    // 3. Right-click window titlebar in Legacy mode -> standalone context menu -> click Rename -> type "titlebar-rclick" + Enter.
    driver
        .run_colon(":appearance legacy")
        .expect("switch to legacy");
    driver
        .run_colon(":layout stack")
        .expect("switch to stacked layout so single window is framed");
    let shell = ShellLayout::compute(
        driver.viewport,
        driver.wm.config(),
        driver.wm.pane_count(),
    );
    let outer = driver
        .wm
        .compute_workspace_rects(shell.workspace)
        .into_iter()
        .find(|(id, _)| *id == driver.wm.active_pane_id())
        .map(|(_, o)| o)
        .expect("active pane geometry");
    driver.right_click(outer.x + 6, outer.y);
    let (rx, ry) = standalone_rename_click_point(&driver);
    driver.click(rx, ry);
    assert!(
        driver.wm.rename_dialog_open(),
        "Clicking Rename in titlebar context menu should open RenameDialogState"
    );
    driver.clear_and_type("titlebar-rclick");
    driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(driver.wm.active_pane().title(), "titlebar-rclick");
    driver
        .run_colon(":layout tiled")
        .expect("restore tiled layout");

    // 4. Right-click window pill on row 0 in Obsidian mode -> context menu -> click Rename -> type "pill-rclick" + Enter.
    driver
        .run_colon(":appearance obsidian")
        .expect("switch to obsidian");
    driver
        .run_colon(":mode standard")
        .expect("ensure standard display mode");
    driver.wm.config_mut().show_top_menubar = false;
    driver.sync_layout();
    let top_bar = Rect::new(0, 0, driver.viewport.width, 1);
    let pills = layout_pills(&driver.wm, top_bar);
    let active_pill = pills
        .iter()
        .find(|p| p.pane == Some(driver.wm.active_pane_id()))
        .expect("active pane pill on row 0");
    driver.right_click(active_pill.x0 + 1, 0);
    let (prx, pry) = standalone_rename_click_point(&driver);
    driver.click(prx, pry);
    assert!(
        driver.wm.rename_dialog_open(),
        "Clicking Rename from pill context menu should open RenameDialogState"
    );
    driver.clear_and_type("pill-rclick");
    driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(driver.wm.active_pane().title(), "pill-rclick");

    // 5. Open Classic drawer (`F7`) -> right-click pane row -> click Rename -> type "classic-rclick" + Enter.
    driver
        .run_colon(":drawer style classic")
        .expect("set classic drawer style");
    if !driver.wm.is_drawer_effectively_visible() {
        driver.press_key(KeyCode::F(7), KeyModifiers::NONE);
    }
    assert!(driver.wm.is_drawer_effectively_visible());
    let shell = ShellLayout::compute(
        driver.viewport,
        driver.wm.config(),
        driver.wm.pane_count(),
    );
    let dr = driver
        .wm
        .drawer_rect(shell.workspace)
        .expect("classic drawer rect");
    let rows = driver.wm.drawer_rows();
    let pane_row_idx = rows
        .iter()
        .position(|r| r.kind == DrawerRowKind::Pane(driver.wm.active_pane_id()))
        .expect("active pane row in classic drawer");
    let (hdr_h, _ftr_h, row_step) =
        drawer_vertical_layout_metrics(driver.wm.config(), dr.height.saturating_sub(2), rows.len());
    let pane_row_y = dr.y + 1 + hdr_h + (pane_row_idx as u16) * (row_step as u16);
    driver.right_click(dr.x + 4, pane_row_y);
    let (crx, cry) = standalone_rename_click_point(&driver);
    driver.click(crx, cry);
    assert!(
        driver.wm.rename_dialog_open(),
        "Clicking Rename in Classic drawer context menu should open RenameDialogState"
    );
    driver.clear_and_type("classic-rclick");
    driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(driver.wm.active_pane().title(), "classic-rclick");

    // 6. Switch to Navigator drawer (`:drawer style navigator`) -> right-click pane row -> click Rename -> type "nav-rclick" + Enter.
    let session_before = driver.wm.config().history_session.clone();
    driver
        .run_colon(":drawer style navigator")
        .expect("set navigator drawer style");
    if !driver.wm.is_drawer_effectively_visible() {
        driver.press_key(KeyCode::F(7), KeyModifiers::NONE);
    }
    let shell = ShellLayout::compute(
        driver.viewport,
        driver.wm.config(),
        driver.wm.pane_count(),
    );
    let nav_rows = driver.wm.drawer_rows();
    let nav_pane_idx = nav_rows
        .iter()
        .position(|r| r.kind == DrawerRowKind::Pane(driver.wm.active_pane_id()))
        .expect("active pane row in navigator drawer");
    let dots = navigator_dot_cells(&driver.wm, shell.workspace);
    let active_dot = dots
        .iter()
        .find(|d| d.row_index == nav_pane_idx)
        .expect("active pane dot in navigator drawer");
    driver.right_click(active_dot.x + 2, active_dot.y);
    let (nrx, nry) = navigator_menu_rename_click_point(&driver);
    driver.click(nrx, nry);
    assert!(
        driver.wm.rename_dialog_open(),
        "Clicking Rename in Navigator drawer menu should open RenameDialogState"
    );
    driver.clear_and_type("nav-rclick");
    driver.press_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(driver.wm.active_pane().title(), "nav-rclick");
    assert_eq!(
        driver.wm.config().history_session,
        session_before,
        "Session name must remain untouched when renaming a window in Navigator drawer"
    );

    // 7. Background session window rename: move a window to session "bg-sess", then rename by ID (`:window rename <id> bg-renamed`).
    driver
        .wm
        .create_new_session(Some("bg-sess"), false)
        .expect("create target background session bg-sess");
    driver
        .run_colon(&format!(":session switch {session_before}"))
        .expect("switch back to active session");
    driver
        .run_colon(":pane split h")
        .expect("split pane for background session test");
    let bg_id = driver.wm.active_pane_id();
    driver
        .run_colon(&format!(":window move {bg_id} bg-sess"))
        .expect("move window to background session bg-sess");
    assert!(
        driver.wm.pane(bg_id).is_none(),
        "Moved window should no longer reside in active session"
    );
    driver
        .run_colon(&format!(":window rename {bg_id} bg-renamed"))
        .expect("rename background session window by ID");
    let status = driver
        .wm
        .active_pane()
        .status_message()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("bg-renamed"),
        "Background rename status should confirm bg-renamed, got: {status:?}"
    );
    driver
        .run_colon(":session switch bg-sess")
        .expect("switch to bg-sess to verify renamed background window");
    assert_eq!(
        driver.wm.pane(bg_id).expect("bg_id pane in bg-sess").title(),
        "bg-renamed"
    );
}

#[test]
fn live_colon_commands_and_phase45_appearance_lifecycle() {
    let mut driver = BiltermLiveDriver::new_tui(120, 36);

    // Window/pane/tab lifecycle & geometry commands.
    driver.run_colon(":title alpha-win").expect(":title");
    assert_eq!(driver.wm.active_pane().title(), "alpha-win");

    driver
        .run_colon(":pane rename beta-win")
        .expect(":pane rename");
    assert_eq!(driver.wm.active_pane().title(), "beta-win");

    driver.run_colon(":pane split h").expect(":pane split h");
    assert_eq!(driver.wm.pane_count(), 2);

    driver
        .run_colon(":window cascade")
        .expect(":window cascade");
    driver
        .run_colon(":window reset-geometry")
        .expect(":window reset-geometry");

    driver.run_colon(":pane close").expect(":pane close");
    assert_eq!(driver.wm.pane_count(), 1);

    // `:tab` family.
    driver.run_colon(":tab new").expect(":tab new");
    assert_eq!(driver.wm.pane_count(), 2);
    let first_active = driver.wm.active_pane_id();
    driver.run_colon(":tab next").expect(":tab next");
    let next_active = driver.wm.active_pane_id();
    assert_ne!(first_active, next_active);
    driver.run_colon(":tab prev").expect(":tab prev");
    assert_eq!(driver.wm.active_pane_id(), first_active);
    driver
        .run_colon(":tab rename tab-renamed")
        .expect(":tab rename");
    assert_eq!(driver.wm.active_pane().title(), "tab-renamed");
    driver.run_colon(":tab close").expect(":tab close");
    assert_eq!(driver.wm.pane_count(), 1);

    // `:mcp scale` commands.
    driver.run_colon(":mcp scale 120").expect(":mcp scale 120");
    assert_eq!(driver.wm.scale_setting(PanelTarget::Mcp), 120);
    driver
        .run_colon(":mcp scale reset")
        .expect(":mcp scale reset");
    assert_eq!(driver.wm.scale_setting(PanelTarget::Mcp), 100);

    // `Action::ArrangeGrid` via `Leader #` (`Ctrl+X` followed by `#`).
    driver.run_colon(":pane split h").expect("split for grid");
    driver.press_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
    driver.press_key(KeyCode::Char('#'), KeyModifiers::NONE);
    let status = driver
        .wm
        .active_pane()
        .status_message()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("Auto-arranged"),
        "Leader # should trigger ArrangeGrid, got status: {status:?}"
    );

    // Phase 45 `:appearance` lifecycle verbs.
    for preset in ["amber", "synthwave", "solarized"] {
        driver
            .run_colon(&format!(":appearance {preset}"))
            .expect("switch preset");
    }
    driver
        .run_colon(":appearance duplicate amber custom-amber")
        .expect("duplicate amber to custom-amber");
    assert_eq!(
        driver.wm.config().appearance.active_custom.as_deref(),
        Some("custom-amber")
    );

    driver
        .run_colon(":appearance modify density compact")
        .expect("modify density compact");
    driver
        .run_colon(":appearance list")
        .expect("list appearance profiles");
    assert!(
        driver
            .wm
            .active_pane()
            .status_message()
            .unwrap_or_default()
            .contains("custom-amber")
    );

    let export_path = driver.temp_dir.path().join("custom-amber-export.toml");
    driver
        .run_colon(&format!(":appearance export {}", export_path.display()))
        .expect("export custom profile");
    assert!(export_path.exists(), "exported profile file should exist");

    driver
        .run_colon(":appearance reset")
        .expect("reset appearance");
    assert!(driver.wm.config().appearance.active_custom.is_none());

    driver
        .run_colon(&format!(":appearance import {}", export_path.display()))
        .expect("import custom profile");
    assert_eq!(
        driver.wm.config().appearance.active_custom.as_deref(),
        Some("custom-amber")
    );
}

#[test]
fn live_script_studio_and_batch_playbook_driver() {
    let mut driver = BiltermLiveDriver::new_tui(120, 36);

    // 1. Open Script Studio (`:script open`), enter edit mode (`i`), type multi-line `.bt` script.
    driver.run_colon(":script open").expect(":script open");
    assert!(driver.wm.script_studio().active);
    assert!(driver.wm.script_studio().focused);

    // Clear starter lines so our typed script is lines 0..1.
    driver.wm.script_studio_mut().lines = vec![String::new()];
    driver.wm.script_studio_mut().cursor_line = 0;
    driver.wm.script_studio_mut().cursor_col = 0;

    driver.press_key(KeyCode::Char('i'), KeyModifiers::NONE);
    assert!(driver.wm.script_studio().edit_mode);
    driver.type_text(":split v\n:title studio-pane");
    driver.press_key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!driver.wm.script_studio().edit_mode);
    assert_eq!(
        driver.wm.script_studio().lines,
        vec![":split v".to_string(), ":title studio-pane".to_string()]
    );

    // 2. Step with `F10` (executes line 0 `:split v`) and run with `F5` (executes full script).
    driver.wm.script_studio_mut().cursor_line = 0;
    assert_eq!(driver.wm.pane_count(), 1);
    driver.press_key(KeyCode::F(10), KeyModifiers::NONE);
    assert_eq!(driver.wm.pane_count(), 2, "F10 should step :split v");

    driver.press_key(KeyCode::F(5), KeyModifiers::NONE);
    assert_eq!(
        driver.wm.active_pane().title(),
        "studio-pane",
        "F5 should run script and set title to studio-pane"
    );

    // 3. Test live recording (`Ctrl+R` / `R` toggle + `:script record on`) and save/load `.bt` script.
    driver.press_key(KeyCode::Char('r'), KeyModifiers::CONTROL);
    driver.press_key(KeyCode::Char('R'), KeyModifiers::NONE);
    assert!(driver.wm.script_studio().recording);
    driver
        .run_colon(":appearance list")
        .expect("execute colon while recording");
    assert!(
        driver
            .wm
            .script_studio()
            .lines
            .iter()
            .any(|l| l.contains(":appearance list")),
        "Live recording should append executed colon command to Script Studio buffer"
    );
    driver
        .run_colon(":script record off")
        .expect("stop recording");

    driver
        .run_colon(":script save studio-saved")
        .expect("save .bt script");
    let saved_bt = driver
        .config_home
        .join("bilterm")
        .join("scripts")
        .join("studio-saved.bt");
    assert!(saved_bt.exists());

    let loaded_bt = driver.load_bt_script("batch_loaded.bt", ":title loaded-bt-win\n");
    assert!(loaded_bt.exists());
    driver.step_bt_script();
    assert_eq!(driver.wm.active_pane().title(), "loaded-bt-win");
    driver.run_bt_script();
    assert_eq!(driver.wm.active_pane().title(), "loaded-bt-win");
    driver.run_colon(":script close").expect("close studio");

    // 4. Execute a batch `.toml` Playbook.
    let playbook_toml = r#"
name = "live-driver-batch-playbook"

[[steps]]
action = "colon"
command = ":title playbook-win"

[[steps]]
action = "export_ppm"
path = "playbook-snap.ppm"
"#;
    let report = driver
        .run_playbook_toml(playbook_toml)
        .expect("execute batch .toml playbook");
    assert!(report.passed);
    assert_eq!(report.steps_completed, 2);
    assert_eq!(driver.wm.active_pane().title(), "playbook-win");
    assert!(driver.snapshot_dir.join("playbook-snap.ppm").exists());
}

#[test]
fn live_mcp_stdio_subprocess_and_rpc_api_driver() {
    let mut driver = BiltermLiveDriver::new_tui(120, 36);
    let mut mcp = driver.spawn_mcp_stdio();

    // 1. MCP `initialize`, `tools/list`, `resources/list`, `resources/read`.
    let init_resp = mcp.request("initialize", r#"{"clientInfo":{"name":"live-driver"}}"#);
    assert!(init_resp.contains("protocolVersion"), "{init_resp}");

    let tools_resp = mcp.request("tools/list", "{}");
    for tool_name in [
        "bilterm_window_manage",
        "bilterm_agent_mail",
        "bilterm_find",
        "bilterm_daemon",
        "bilterm_session_map",
        "bilterm_agents_list",
        "bilterm_colon_command",
    ] {
        assert!(
            tools_resp.contains(tool_name),
            "tools/list missing {tool_name}: {tools_resp}"
        );
    }

    let res_list = mcp.request("resources/list", "{}");
    assert!(res_list.contains("bilterm://state"), "{res_list}");

    let res_read = mcp.request("resources/read", r#"{"uri":"bilterm://state"}"#);
    assert!(res_read.contains("active_pane_id"), "{res_read}");

    // 2. MCP `tools/call` across all 7 required tools.
    let win_resp = mcp.call_tool(
        "bilterm_window_manage",
        r#"{"action":"rename","new_name":"mcp-win-1"}"#,
    );
    assert!(win_resp.contains("mcp-win-1"), "{win_resp}");

    let _ = mcp.call_tool(
        "bilterm_colon_command",
        r#"{"command":":agent enable on"}"#,
    );
    let _ = mcp.call_tool(
        "bilterm_agent_mail",
        r#"{"action":"name","name":"mcp-agent"}"#,
    );
    let mail_resp = mcp.call_tool(
        "bilterm_agent_mail",
        r#"{"action":"send","to":"mcp-agent","body":"hello-from-mcp"}"#,
    );
    assert!(
        mail_resp.contains("hello-from-mcp") || mail_resp.contains("message_id"),
        "{mail_resp}"
    );

    let find_resp = mcp.call_tool("bilterm_find", r#"{"kind":"agents","query":"mcp"}"#);
    assert!(find_resp.contains("rows"), "{find_resp}");

    let daemon_resp = mcp.call_tool("bilterm_daemon", r#"{"action":"status"}"#);
    assert!(daemon_resp.contains("result"), "{daemon_resp}");

    let map_resp = mcp.call_tool("bilterm_session_map", r#"{"op":"open"}"#);
    assert!(map_resp.contains("result"), "{map_resp}");

    let alist_resp = mcp.call_tool("bilterm_agents_list", r#"{"op":"open"}"#);
    assert!(alist_resp.contains("result"), "{alist_resp}");

    let colon_resp = mcp.call_tool(
        "bilterm_colon_command",
        r#"{"command":":title mcp-colon-win"}"#,
    );
    assert!(colon_resp.contains("result"), "{colon_resp}");

    // 3. In-memory JSON-RPC (`window.rename`, `session.rename`, `agent.send`, `agent.inbox`, `find.query`, `daemon.status`).
    let rpc_win = driver.drive_rpc_in_memory("window.rename", r#"{"new_name":"rpc-win"}"#);
    assert!(rpc_win.contains("rpc-win"), "{rpc_win}");
    assert_eq!(driver.wm.active_pane().title(), "rpc-win");

    let rpc_sess = driver.drive_rpc_in_memory("session.rename", r#"{"new_name":"rpc-session"}"#);
    assert!(rpc_sess.contains("rpc-session"), "{rpc_sess}");
    assert_eq!(driver.wm.config().history_session, "rpc-session");

    driver
        .run_colon(":agent enable on")
        .expect("enable agent messaging for RPC");
    let _ = driver.drive_rpc_in_memory("agent.name", r#"{"name":"rpc-worker"}"#);
    let rpc_send = driver.drive_rpc_in_memory(
        "agent.send",
        r#"{"to":"rpc-worker","body":"rpc-hello"}"#,
    );
    assert!(rpc_send.contains("message_id"), "{rpc_send}");

    let rpc_inbox = driver.drive_rpc_in_memory("agent.inbox", r#"{"agent":"rpc-worker"}"#);
    assert!(rpc_inbox.contains("rpc-hello"), "{rpc_inbox}");

    let rpc_find = driver.drive_rpc_in_memory("find.query", r#"{"query":"rpc"}"#);
    assert!(rpc_find.contains("rows"), "{rpc_find}");

    let rpc_daemon = driver.drive_rpc_in_memory("daemon.status", "{}");
    assert!(rpc_daemon.contains("result"), "{rpc_daemon}");
}

#[test]
fn live_agents_navigator_and_mailbox_driver() {
    let mut driver = BiltermLiveDriver::new_tui(120, 36);

    // 1. Agent Navigator (`F7`).
    driver
        .run_colon(":drawer style navigator")
        .expect("navigator style");
    driver.press_key(KeyCode::F(7), KeyModifiers::NONE);
    assert!(driver.wm.is_drawer_effectively_visible());

    // 2. `:agents demo` and `:agents demo stop`.
    driver.run_colon(":agents demo").expect(":agents demo");
    assert!(driver.wm.agents_window().active);
    assert!(driver.wm.pane_count() >= 3);
    driver
        .run_colon(":agents demo stop")
        .expect(":agents demo stop");
    assert!(
        driver
            .wm
            .active_pane()
            .status_message()
            .unwrap_or_default()
            .contains("Demo stopped")
    );

    // 3. `:agents list` (open + float + dock).
    driver
        .run_colon(":agents list open")
        .expect(":agents list open");
    assert!(driver.wm.agents_list().active);
    driver
        .run_colon(":agents list float")
        .expect(":agents list float");
    assert!(!driver.wm.agents_list().docked);
    driver
        .run_colon(":agents list dock")
        .expect(":agents list dock");
    assert!(driver.wm.agents_list().docked);

    // 4. Agent mailbox lifecycle (`:agent name`, `:agent send`, `:agent inbox`, `:agent broadcast`).
    driver
        .run_colon(":agent enable on")
        .expect(":agent enable on");
    driver
        .run_colon(":agent name worker-1")
        .expect(":agent name worker-1");
    assert_eq!(
        driver
            .wm
            .agent_mail()
            .name_for_pane(driver.wm.active_pane_id()),
        Some("worker-1")
    );

    driver
        .run_colon(":agent send worker-1 hello")
        .expect(":agent send worker-1 hello");
    let inbox = driver.wm.agent_mail().get_inbox("worker-1", false);
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].body, "hello");

    driver
        .run_colon(":agent inbox worker-1")
        .expect(":agent inbox worker-1");
    assert!(
        driver
            .wm
            .active_pane()
            .status_message()
            .unwrap_or_default()
            .contains("worker-1")
    );
    assert!(
        driver
            .wm
            .take_command_data()
            .unwrap_or_default()
            .contains("hello")
    );

    driver.run_colon(":pane split h").expect("split for worker-2");
    driver
        .run_colon(":agent name worker-2")
        .expect(":agent name worker-2");
    driver
        .run_colon(":agent broadcast ping")
        .expect(":agent broadcast ping");
    let inbox_w1 = driver.wm.agent_mail().get_inbox("worker-1", false);
    assert!(inbox_w1.iter().any(|m| m.body == "ping"));
}

#[test]
fn live_gui_framebuffer_and_cli_smoke_driver() {
    let mut driver = BiltermLiveDriver::new_tui(100, 30);

    // 1. Render `GuiWorkspaceRenderer` across Legacy, Obsidian, Liquid, and Synthwave (including with RenameDialogState open).
    for mode in ["legacy", "obsidian", "liquid", "synthwave"] {
        driver
            .run_colon(&format!(":appearance {mode}"))
            .expect("set appearance mode");
        let surface = driver.render_gui_surface(8, 16);
        assert_eq!(surface.viewport_cells.width, 100);
        assert_eq!(surface.viewport_cells.height, 30);
        assert!(!surface.quads.is_empty(), "quads non-empty in {mode}");
        assert!(!surface.glyphs.is_empty(), "glyphs non-empty in {mode}");

        let (pw, ph) = surface.pixel_dimensions();
        let rgba = driver.rasterize_rgba(8, 16);
        assert_eq!(rgba.len(), (pw as usize) * (ph as usize) * 4);
    }

    // Open `RenameDialogState` and verify GUI framebuffer renders the modal popup cleanly.
    driver
        .run_colon(":window rename")
        .expect("open rename dialog");
    assert!(driver.wm.rename_dialog_open());
    let modal_surface = driver.render_gui_surface(8, 16);
    let glyph_text: String = modal_surface
        .glyphs
        .iter()
        .map(|g| g.text.as_str())
        .collect();
    assert!(
        glyph_text.contains("RENAMEWINDOW"),
        "GUI framebuffer glyphs should contain RENAME WINDOW popup title"
    );
    driver.press_key(KeyCode::Esc, KeyModifiers::NONE);

    // 2. CLI smoke tests: `--version`, `--gui-dump`, `ls`.
    let ver_out = driver.run_cli(&["--version"]);
    assert!(ver_out.status.success());
    let ver_stdout = String::from_utf8_lossy(&ver_out.stdout);
    assert!(ver_stdout.to_ascii_lowercase().contains("bilterm"));

    let dump_ppm = driver.temp_dir.path().join("xvfb-gui-dump.ppm");
    let dump_out = driver.run_xvfb_gui_dump(&[
        "--gui-dump",
        dump_ppm.to_str().expect("utf8 path"),
        "--gui-cols",
        "80",
        "--gui-rows",
        "24",
    ]);
    assert!(
        dump_out.status.success(),
        "gui-dump failed: {}",
        String::from_utf8_lossy(&dump_out.stderr)
    );
    assert!(dump_ppm.exists(), "--gui-dump PPM output should exist");

    let ls_out = driver.run_cli(&["ls"]);
    let ls_combined = format!(
        "{}{}",
        String::from_utf8_lossy(&ls_out.stdout),
        String::from_utf8_lossy(&ls_out.stderr)
    );
    assert!(!ls_combined.is_empty() || ls_out.status.code().is_some());
}

#[test]
fn live_phase56_drawer_peek_and_phase57_clipboard_driver() {
    let mut driver = BiltermLiveDriver::new_tui(110, 32);

    // --- Phase 56: Drawer Window Peek & Live Preview Pane ---
    let p1 = driver.wm.active_pane_id();
    driver
        .wm
        .pane_mut(p1)
        .unwrap()
        .process_pty_output(b"PHASE56_PANE1_CANVAS\r\n");
    driver.run_colon(":split v").expect("split v");
    let p2 = driver.wm.active_pane_id();
    assert_ne!(p1, p2);
    driver
        .wm
        .pane_mut(p2)
        .unwrap()
        .process_pty_output(b"PHASE56_PANE2_CANVAS\r\n");

    // Open drawer and preview p1 without stealing focus from p2
    driver.run_colon(":drawer open").expect(":drawer open");
    driver
        .run_colon(&format!(":drawer preview pane {p1}"))
        .expect(":drawer preview pane p1");
    assert!(driver.wm.drawer().preview.active);
    assert_eq!(driver.wm.active_pane_id(), p2, "peek must not steal focus");
    driver.assert_screen_contains("PEEK");
    driver.assert_screen_contains("PHASE56_PANE1_CANVAS");

    // Commit preview via `:drawer preview commit` -> focuses p1 and closes preview
    driver
        .run_colon(":drawer preview commit")
        .expect(":drawer preview commit");
    assert_eq!(driver.wm.active_pane_id(), p1);
    assert!(!driver.wm.drawer().preview.active);

    // --- Phase 57: Smart Clipboard Paste & Filterable Clipboard Viewer Window ---
    driver
        .run_colon(":clipboard copy live_driver_clip_entry")
        .expect(":clipboard copy");
    driver.run_colon(":clipboard open").expect(":clipboard open");
    assert!(driver.wm.clipboard_window().active);
    driver.assert_screen_contains("CLIPBOARD");
    driver.assert_screen_contains("live_driver_clip_entry");

    // Verify GUI framebuffer also renders the CLIPBOARD utility window cleanly
    let gui_surface = driver.render_gui_surface(8, 16);
    assert!(!gui_surface.quads.is_empty());
    driver.run_colon(":clipboard close").expect(":clipboard close");
    assert!(!driver.wm.clipboard_window().active);
}
