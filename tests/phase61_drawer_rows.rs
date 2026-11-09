use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::drawer::{
    ClassicHeaderChipKind, classic_header_chips, drawer_vertical_layout_metrics,
};
use bilterm::ui::modals::{menubar_category_item_count, menubar_item_command};
use bilterm::wm::drawer::{DrawerConfig, DrawerEdge, DrawerRowKind, DrawerStyle};
use bilterm::wm::{Modal, SplitDirection, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

fn isolate_xdg() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-p61-rows-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn classic_wm_with_panes(n: usize, cols: u16, rows: u16) -> (WindowManager, Rect) {
    isolate_xdg();
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    while wm.pane_count() < n {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.pane_mut(1)
        .unwrap()
        .process_pty_output(b"\x1b]2;Alpha\x07");
    for id in 2..=n {
        wm.pane_mut(id)
            .unwrap()
            .process_pty_output(format!("\x1b]2;Pane-{id}\x07").as_bytes());
    }
    wm.set_drawer_style(Some(DrawerStyle::Classic));
    wm.set_drawer_edge(Some(DrawerEdge::Left));
    wm.resize_all(area).unwrap();
    wm.focus_drawer();
    (wm, area)
}

fn render_buffer(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn row_string(buf: &Buffer, y: u16, x_end: u16) -> String {
    (0..x_end.min(buf.area.width))
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect::<String>()
}

#[test]
fn config_default_is_auto_and_toml_overrides() {
    let dcfg = DrawerConfig::default();
    assert_eq!(dcfg.classic_row_lines, 0);
    assert_eq!(dcfg.classic_row_lines_mode(), 0);
    assert_eq!(dcfg.row_lines, 2);

    let parsed = BiltermConfig::from_toml_str("[drawer]\nclassic_row_lines = 2\n").unwrap();
    assert_eq!(parsed.drawer.classic_row_lines, 2);
    assert_eq!(parsed.drawer.classic_row_lines_mode(), 2);

    let parsed_bad = BiltermConfig::from_toml_str("[drawer]\nclassic_row_lines = 7\n").unwrap();
    assert_eq!(parsed_bad.drawer.classic_row_lines, 7);
    assert_eq!(parsed_bad.drawer.classic_row_lines_mode(), 0);
}

#[test]
fn setter_writes_style_owned_field() {
    let (mut wm, _) = classic_wm_with_panes(2, 100, 24);
    assert_eq!(wm.config().drawer.style, DrawerStyle::Classic);
    assert_eq!(wm.config().drawer.classic_row_lines, 0);
    assert_eq!(wm.config().drawer.row_lines, 2);

    let _ = wm.apply(Action::DrawerRows(2));
    assert_eq!(wm.config().drawer.classic_row_lines, 2);
    assert_eq!(wm.config().drawer.row_lines, 2);
    let msg = wm.active_pane().status_message().unwrap_or("");
    assert!(
        msg.contains("classic style"),
        "expected classic style status message, got: {msg}"
    );

    wm.set_drawer_style(Some(DrawerStyle::Navigator));
    let _ = wm.apply(Action::DrawerRows(1));
    assert_eq!(wm.config().drawer.row_lines, 1);
    assert_eq!(
        wm.config().drawer.classic_row_lines,
        2,
        "classic_row_lines must remain untouched when setting rows in Navigator style"
    );
    let nav_msg = wm.active_pane().status_message().unwrap_or("");
    assert!(
        nav_msg.contains("navigator style"),
        "expected navigator style status message, got: {nav_msg}"
    );

    // Horizontal Classic drawer notes one-line rendering.
    wm.set_drawer_style(Some(DrawerStyle::Classic));
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    let _ = wm.apply(Action::DrawerRows(2));
    let horiz_msg = wm.active_pane().status_message().unwrap_or("");
    assert!(
        horiz_msg.contains("horizontal drawers show one line"),
        "expected horizontal note in status message, got: {horiz_msg}"
    );
}

#[test]
fn layout_step_two_in_legacy_look() {
    let mut cfg = BiltermConfig::default();
    assert!(!cfg.is_modern());

    // Default 0 (auto) in Legacy look -> step 1.
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 5).2, 1);

    // Explicit 2 in Legacy look -> step 2 when avail >= 2.
    cfg.drawer.classic_row_lines = 2;
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 5).2, 2);
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 20).2, 2);

    // Explicit 1 in Legacy look -> step 1.
    cfg.drawer.classic_row_lines = 1;
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 5).2, 1);

    // LiquidGlass look: auto (0) is 2 when item_count * 2 <= avail, 1 when crowded,
    // and forced to 2 when classic_row_lines = 2.
    cfg.liquid_glass = true;
    cfg.drawer.classic_row_lines = 0;
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 3).2, 2);
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 15).2, 1);
    cfg.drawer.classic_row_lines = 2;
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 15).2, 2);
    cfg.drawer.classic_row_lines = 1;
    assert_eq!(drawer_vertical_layout_metrics(&cfg, 20, 3).2, 1);
}

#[test]
fn render_two_line_rows_legacy() {
    let (mut wm, area) = classic_wm_with_panes(2, 120, 24);
    wm.resize_drawer(60);

    // 2-line rows in Legacy look: title on line L, telemetry ("Ready") on line L + 1.
    let _ = wm.apply(Action::DrawerRows(2));
    let buf2 = render_buffer(&wm, area);
    let lines2: Vec<String> = (0..24).map(|y| row_string(&buf2, y, 60)).collect();
    let alpha_line_2 = lines2
        .iter()
        .position(|l| l.contains("Alpha"))
        .expect("Alpha row must render in 2-line mode");
    assert!(
        !lines2[alpha_line_2].contains("Ready"),
        "title line in 2-line mode should not inline telemetry: {:?}",
        lines2[alpha_line_2]
    );
    assert!(
        lines2[alpha_line_2 + 1].contains("Ready"),
        "line L+1 in 2-line mode must contain telemetry 'Ready': {:?}",
        lines2[alpha_line_2 + 1]
    );

    // 1-line rows in Legacy look: title and telemetry share the same line.
    let _ = wm.apply(Action::DrawerRows(1));
    let buf1 = render_buffer(&wm, area);
    let lines1: Vec<String> = (0..24).map(|y| row_string(&buf1, y, 60)).collect();
    let alpha_line_1 = lines1
        .iter()
        .position(|l| l.contains("Alpha"))
        .expect("Alpha row must render in 1-line mode");
    assert!(
        lines1[alpha_line_1].contains("✓ 0") && lines1[alpha_line_1].contains("▁▁▁"),
        "1-line mode should keep title and telemetry on one row: {:?}",
        lines1[alpha_line_1]
    );
}

#[test]
fn mouse_second_line_selects_row() {
    let (mut wm, area) = classic_wm_with_panes(3, 100, 24);
    let _ = wm.apply(Action::DrawerRows(2));

    let items = wm.drawer_rows();
    let pane_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, it)| {
            if matches!(it.kind, DrawerRowKind::Pane(_)) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    assert!(pane_indices.len() >= 2);
    let target_idx = pane_indices[1];

    let inner_height = area.height.saturating_sub(2);
    let (header_rows, _footer_rows, step) =
        drawer_vertical_layout_metrics(wm.config(), inner_height, items.len());
    assert_eq!(step, 2);
    let list_start_y = 1 + header_rows;
    let second_line_y = list_start_y + (target_idx as u16) * (step as u16) + 1;

    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 8,
        row: second_line_y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click, area).unwrap();
    assert_eq!(wm.drawer().selected_index, target_idx);
}

#[test]
fn mouse_wheel_uses_step() {
    let (mut wm, area) = classic_wm_with_panes(10, 100, 16);
    let _ = wm.apply(Action::DrawerRows(2));
    wm.drawer_mut().preview.auto_preview_on_scroll = false;
    wm.drawer_mut().preview.active = false;
    wm.drawer_mut().selected_index = 0;
    wm.drawer_mut().scroll_offset = 0;

    let items = wm.drawer_rows();
    let inner_height = area.height.saturating_sub(2);
    let (header_rows, footer_rows, step) =
        drawer_vertical_layout_metrics(wm.config(), inner_height, items.len());
    assert_eq!(step, 2);
    let list_rows = inner_height.saturating_sub(header_rows + footer_rows) as usize;
    let cap_2line = (list_rows / step).max(1);
    assert!(items.len() > cap_2line);

    for _ in 0..(items.len() + 4) {
        let ev = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 5,
            row: 6,
            modifiers: KeyModifiers::NONE,
        };
        handle_wm_mouse_event(&mut wm, ev, area).unwrap();
    }
    assert_eq!(
        wm.drawer().effective_scroll(0, cap_2line, items.len()),
        items.len().saturating_sub(cap_2line),
        "wheel scroll offset must use 2-line row capacity"
    );
}

#[test]
fn discover_chip_key_menu_reach_same_state() {
    let (mut wm, area) = classic_wm_with_panes(2, 120, 24);
    wm.resize_drawer(52);

    // 1. Header chip click toggles classic_row_lines to 2, then 1.
    let inner_h = area.height.saturating_sub(2);
    let chips = classic_header_chips(&wm, 52, inner_h, false);
    let (_, range, _) = chips
        .iter()
        .find(|(_, _, k)| matches!(k, ClassicHeaderChipKind::RowLines(_)))
        .expect("[R:n] chip should be present on widened Classic header");
    let chip_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: range.start + 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, chip_click, area).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 2);

    handle_wm_mouse_event(&mut wm, chip_click, area).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 1);

    // 2. Focused drawer keys '2' and '1' reach the same state.
    wm.focus_drawer();
    let key2 = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key2).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 2);

    // Pressing '2' again toggles back to 1.
    handle_wm_key_event(&mut wm, key2).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 1);

    // Pressing '2' sets 2, then '1' explicitly sets 1.
    handle_wm_key_event(&mut wm, key2).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 2);
    let key1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key1).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 1);
    wm.drawer_mut().focused = false;

    // 3. View menu items (category 3) dispatch `:drawer rows 1` and `:drawer rows 2`.
    let view_cat = 3usize;
    let count = menubar_category_item_count(view_cat);
    let rows2_idx = (0..count)
        .find(|&i| menubar_item_command(view_cat, i) == Some(":drawer rows 2"))
        .expect("View menu contains :drawer rows 2");
    let rows1_idx = (0..count)
        .find(|&i| menubar_item_command(view_cat, i) == Some(":drawer rows 1"))
        .expect("View menu contains :drawer rows 1");

    *wm.modal_mut() = Modal::Menubar {
        category: view_cat,
        item: rows2_idx,
    };
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, enter).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 2);

    *wm.modal_mut() = Modal::Menubar {
        category: view_cat,
        item: rows1_idx,
    };
    handle_wm_key_event(&mut wm, enter).unwrap();
    assert_eq!(wm.config().drawer.classic_row_lines, 1);

    // 4. Colon command parses to Action::DrawerRows(2).
    assert_eq!(
        parse_colon_command(":drawer rows 2"),
        Some(Ok(Action::DrawerRows(2)))
    );
}

#[test]
fn parity_usage_lists_rows() {
    let err = parse_colon_command(":drawer bogus")
        .expect("drawer command recognized")
        .expect_err("bogus subcommand must fail");
    assert!(
        err.contains("rows"),
        "expected :drawer usage error to mention rows, got: {err}"
    );

    // Setting rows = 2 on a top-edge drawer and rendering does not panic.
    let (mut wm, area) = classic_wm_with_panes(2, 100, 24);
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    let _ = wm.apply(Action::DrawerRows(2));
    let _buf = render_buffer(&wm, area);
}
