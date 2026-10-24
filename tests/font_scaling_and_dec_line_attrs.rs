use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::handle_wm_key_event;
use bilterm::pane::CompositePane;
use bilterm::ui::CompositePaneWidget;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::ui::typography::LineSizeAttr;
use bilterm::wm::WindowManager;

#[test]
fn ctrl_plus_minus_zero_colon_fontsize_and_rpc_scale_font_and_emit_osc50_passthrough() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.config().font_size_pt, 14);
    assert_eq!(wm.config().font_scale_pct, 100);
    assert_eq!(wm.config().scaled_cell_dimensions_px(), (8, 16));

    // 1. Ctrl++ zooms in (+10%) and queues OSC 50 passthrough
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('+'), KeyModifiers::CONTROL),
    )
    .expect("Ctrl++");
    assert_eq!(wm.config().font_scale_pct, 110);
    assert_eq!(
        wm.active_pane().last_osc52_sequence(),
        Some("\x1b]50;fontsize=15;scale=110%\x07")
    );

    // 2. Ctrl+= zooms in another +10% (to 120%)
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('='), KeyModifiers::CONTROL),
    )
    .expect("Ctrl+=");
    assert_eq!(wm.config().font_scale_pct, 120);

    // 3. Ctrl+- zooms out (-10% -> 110%)
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL),
    )
    .expect("Ctrl+-");
    assert_eq!(wm.config().font_scale_pct, 110);

    // 4. Ctrl+0 resets font scale to 100%
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::CONTROL),
    )
    .expect("Ctrl+0");
    assert_eq!(wm.config().font_scale_pct, 100);

    // 5. :fontsize 200% and :fontsize 21pt via colon command parser
    let cmd_200 = parse_colon_command(":fontsize 200%").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd_200);
    assert_eq!(wm.config().font_scale_pct, 200);
    assert_eq!(wm.config().scaled_font_size_pt(), 28);
    assert_eq!(wm.config().scaled_cell_dimensions_px(), (16, 32));

    let cmd_reset = parse_colon_command(":fontsize reset").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd_reset);
    assert_eq!(wm.config().font_scale_pct, 100);

    // 6. JSON-RPC workspace.fontsize queries and updates font scale
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    let res = cp
        .dispatch_method(
            "workspace.fontsize",
            &JsonValue::obj([("action", JsonValue::String("150%".to_string()))]),
        )
        .expect("workspace.fontsize 150%");
    assert_eq!(
        res.get("font_scale_pct").and_then(JsonValue::as_usize),
        Some(150)
    );
    assert_eq!(
        res.get("font_size_pt").and_then(JsonValue::as_usize),
        Some(21)
    );
    assert_eq!(
        res.get("cell_width_px").and_then(JsonValue::as_usize),
        Some(12)
    );
    assert_eq!(
        res.get("cell_height_px").and_then(JsonValue::as_usize),
        Some(24)
    );
}

#[test]
fn gui_workspace_renderer_scales_cells_svg_ppm_and_recalculates_pixel_window_grid() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().process_pty_output(b"SCALED_TEXT\r\n");

    let mut gui = GuiWorkspaceRenderer::new(8, 16);
    let mut surface_100 = GuiFrameSurface::new(8, 16);
    let vp_100 = gui
        .resize_workspace_for_pixel_window(&mut wm, 640, 384)
        .expect("resize at 100%");
    assert_eq!(vp_100, Rect::new(0, 0, 80, 24));
    gui.render_workspace(&wm, vp_100, &mut surface_100);
    assert_eq!(surface_100.cell_width_px, 8);
    assert_eq!(surface_100.cell_height_px, 16);
    assert_eq!(surface_100.pixel_dimensions(), (640, 384));
    let svg_100 = surface_100.export_svg("Scale 100%");
    assert!(svg_100.contains(r#"font-size="14""#));

    // Scale to 200%: cell dimensions double to 16x32px, so a 640x384px window fits 40x12 cells
    wm.font_set_size(200);
    let vp_200 = gui
        .resize_workspace_for_pixel_window(&mut wm, 640, 384)
        .expect("resize at 200%");
    assert_eq!(vp_200, Rect::new(0, 0, 40, 12));
    assert_eq!(wm.active_pane().vt().size(), (11, 40));

    let mut surface_200 = GuiFrameSurface::new(8, 16);
    gui.render_workspace(&wm, vp_100, &mut surface_200);
    assert_eq!(surface_200.cell_width_px, 16);
    assert_eq!(surface_200.cell_height_px, 32);
    assert_eq!(surface_200.font_size_pt, 28);
    assert_eq!(surface_200.pixel_dimensions(), (1280, 768));

    let svg_200 = surface_200.export_svg("Scale 200%");
    assert!(svg_200.contains(r#"font-size="28""#));

    let ppm_200 = surface_200.export_ppm();
    assert!(ppm_200.starts_with(b"P6\n1280 768\n255\n"));
}

#[test]
fn dec_decdwl_and_decdhl_line_size_attributes_render_double_width_cells_in_tui_mode() {
    let mut pane = CompositePane::new_headless(1, 12, 40);
    // Hide floating prompt so row 0..3 of the canvas are directly visible without occlusion
    pane.toggle_manual_raw_override();

    // Row 0: DECDWL (\x1b#6) double-width line
    // Row 1: DECDHL top (\x1b#3) double-height top half
    // Row 2: DECDHL bottom (\x1b#4) double-height bottom half
    // Row 3: DECSWL (\x1b#5) normal single-width line
    pane.process_pty_output(
        b"\x1b[H\x1b#6WIDE\r\n\x1b#3TOP\r\n\x1b#4BOT\r\n\x1b#5NORM",
    );

    assert_eq!(pane.vt().line_size_attr(0), LineSizeAttr::DoubleWidth);
    assert_eq!(pane.vt().line_size_attr(1), LineSizeAttr::DoubleHeightTop);
    assert_eq!(
        pane.vt().line_size_attr(2),
        LineSizeAttr::DoubleHeightBottom
    );
    assert_eq!(pane.vt().line_size_attr(3), LineSizeAttr::Single);

    let snap = pane.vt().snapshot();
    assert_eq!(snap.grid_rows[0].line_size, LineSizeAttr::DoubleWidth);
    assert_eq!(snap.grid_rows[1].line_size, LineSizeAttr::DoubleHeightTop);
    assert_eq!(
        snap.grid_rows[2].line_size,
        LineSizeAttr::DoubleHeightBottom
    );
    assert_eq!(snap.grid_rows[3].line_size, LineSizeAttr::Single);

    let area = Rect::new(0, 0, 40, 12);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);

    // Row 0 ("WIDE" in DECDWL): each character occupies 2 horizontal cells (cols 0, 2, 4, 6)
    assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "W");
    assert_eq!(buf.cell((1, 0)).unwrap().symbol(), " ");
    assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "I");
    assert_eq!(buf.cell((4, 0)).unwrap().symbol(), "D");
    assert_eq!(buf.cell((6, 0)).unwrap().symbol(), "E");
    assert!(buf.cell((0, 0)).unwrap().modifier.contains(Modifier::BOLD));

    // Row 2 ("BOT" in DECDHL bottom half): double-width + bold + underlined
    assert_eq!(buf.cell((0, 2)).unwrap().symbol(), "B");
    assert_eq!(buf.cell((2, 2)).unwrap().symbol(), "O");
    assert_eq!(buf.cell((4, 2)).unwrap().symbol(), "T");
    assert!(
        buf.cell((0, 2))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );

    // Row 3 ("NORM" in DECSWL): standard 1x1 spacing (cols 0, 1, 2, 3)
    assert_eq!(buf.cell((0, 3)).unwrap().symbol(), "N");
    assert_eq!(buf.cell((1, 3)).unwrap().symbol(), "O");
    assert_eq!(buf.cell((2, 3)).unwrap().symbol(), "R");
    assert_eq!(buf.cell((3, 3)).unwrap().symbol(), "M");
}
