use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::pane::{CanvasSubcellMode, CompositePane};
use bilterm::ui::CompositePaneWidget;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::vt::{TekLineStyle, TekMode, encode_tek_10bit_point, encode_tek_12bit_point};
use bilterm::wm::WindowManager;

#[test]
fn tektronix_4010_4014_decodes_10bit_12bit_vectors_point_incremental_styles_and_phosphor_decay() {
    let mut pane = CompositePane::new_headless(1, 20, 60);
    assert!(!pane.vt().tektronix().active);

    // 1. Enter Tektronix 4010/4014 mode via DECSET ?38h, select Dotted line style (\x1ba),
    //    enter GS (0x1D) Graph mode, move to 10-bit (100, 100), and draw to 12-bit (3000, 2000)
    let mut stream = Vec::new();
    stream.extend_from_slice(b"\x1b[?38h\x1ba\x1d");
    stream.extend_from_slice(&encode_tek_10bit_point(100, 100));
    stream.extend_from_slice(&encode_tek_12bit_point(3000, 2000));
    // 2. Switch to FS (0x1C) Point Plot mode and plot a point at (512, 390)
    stream.push(0x1c);
    stream.extend_from_slice(&encode_tek_10bit_point(512, 390));
    // 3. Switch to RS (0x1E) Incremental Plot mode, pen down ('P'), step East ('A') and North ('D')
    stream.extend_from_slice(b"\x1ePAD");
    // 4. Return to Alpha mode (US 0x1F) and write a label
    stream.extend_from_slice(b"\x1fTEK_LABEL");

    pane.process_pty_output(&stream);

    let tek = pane.vt().tektronix();
    assert!(tek.active);
    assert_eq!(tek.mode, TekMode::Alpha);
    assert_eq!(tek.line_style, TekLineStyle::Dotted);
    // 1 bright vector + 1 point plot + 2 incremental steps = 4 segments
    assert_eq!(tek.segments.len(), 4);
    assert_eq!(tek.segments[0].x0, 400);
    assert_eq!(tek.segments[0].y0, 400);
    assert_eq!(tek.segments[0].x1, 3000);
    assert_eq!(tek.segments[0].y1, 2000);
    assert!(tek.segments[1].point_only);
    assert_eq!(tek.labels.len(), 1);
    assert_eq!(tek.labels[0].text, "TEK_LABEL");

    // 5. Screen erase (\x1b\x0c) moves active vectors into decaying_segments with phosphor_age = 1
    pane.process_pty_output(b"\x1b\x0c");
    assert_eq!(pane.vt().tektronix().segments.len(), 0);
    assert_eq!(pane.vt().tektronix().decaying_segments.len(), 4);
    assert_eq!(pane.vt().tektronix().decaying_segments[0].phosphor_age, 1);

    // 6. Return to VT100 mode via DECRST ?38l
    pane.process_pty_output(b"\x1b[?38l");
    assert!(!pane.vt().tektronix().active);
}

#[test]
fn modern_retained_canvas_scene_sprites_physics_aabb_collision_plots_and_image_inspector_work() {
    let mut pane = CompositePane::new_headless(1, 24, 80);

    // Spawn 2 sprites heading toward each other
    pane.execute_canvas_command("sprite 1 10 10 4 0 4 4")
        .expect("sprite 1");
    pane.execute_canvas_command("sprite 2 16 10 -4 0 4 4")
        .expect("sprite 2");
    assert_eq!(pane.canvas_scene().detect_sprite_collisions().len(), 0);

    // Step physics by dt=1.0 -> sprite 1 moves to x=14 (14..18), sprite 2 moves to x=12 (12..16) -> AABB collision!
    pane.execute_canvas_command("step 1.0").expect("step 1.0");
    assert_eq!(
        pane.canvas_scene().detect_sprite_collisions(),
        vec![(1, 2)]
    );

    // Add vector primitives, declarative plots, and interactive image
    pane.execute_canvas_command("line 0 0 40 20").unwrap();
    pane.execute_canvas_command("circle 20 15 6 fill").unwrap();
    pane.execute_canvas_command("bezier 0 0 10 20 30 0 40 20")
        .unwrap();
    pane.execute_canvas_command("plot candlestick 10,20,5,15 15,25,12,10")
        .unwrap();
    pane.execute_canvas_command("plot heatmap 1,2,3,4,5,6,7,8,9")
        .unwrap();
    pane.execute_canvas_command("image 16 12").unwrap();
    pane.execute_canvas_command("pan 2 1").unwrap();
    pane.execute_canvas_command("zoom 1.5").unwrap();

    let img = pane
        .canvas_scene()
        .image_view
        .as_ref()
        .expect("image_view loaded");
    assert_eq!((img.width, img.height), (16, 12));
    assert_eq!((img.pan_x, img.pan_y), (2, 1));
    assert!((img.zoom - 1.5).abs() < 1e-4);
    assert!(img.inspect_pixel(0, 0).is_some());
    assert!(img.inspect_pixel(99, 99).is_none());
}

#[test]
fn braille_sextant_quadrant_tui_rasterizer_and_gui_svg_ppm_preserve_floating_prompt_overlay() {
    let mut wm = WindowManager::new_headless(20, 60);
    wm.active_pane_mut().prompt_mut().set_text("echo OVER_TEK");
    wm.execute_canvas_action("tek demo");
    wm.execute_canvas_action("line 2 2 50 30");

    let area = Rect::new(0, 0, 60, 20);

    // 1. Braille mode (U+2800..U+28FF)
    wm.execute_canvas_action("mode braille");
    assert_eq!(
        wm.active_pane().canvas_scene().subcell_mode,
        CanvasSubcellMode::Braille
    );
    let mut buf_braille = Buffer::empty(area);
    CompositePaneWidget::new(wm.active_pane(), true).render(area, &mut buf_braille);
    let has_braille = (0..20).any(|y| {
        (0..60).any(|x| {
            buf_braille
                .cell((x, y))
                .and_then(|c| c.symbol().chars().next())
                .is_some_and(|ch| (0x2801..=0x28ff).contains(&(ch as u32)))
        })
    });
    assert!(has_braille, "expected Unicode 2x4 Braille glyphs on canvas");

    // Verify floating command prompt text ("echo OVER_TEK") is composited cleanly on top of the canvas
    let top_rows_text: String = (0..4)
        .map(|y| {
            (0..60)
                .map(|x| buf_braille.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        top_rows_text.contains("echo OVER_TEK"),
        "FloatingCommandPrompt must composite cleanly over graphical canvas"
    );

    // 2. Sextant mode (U+1FB00..U+1FB3B)
    wm.execute_canvas_action("mode sextant");
    let mut buf_sextant = Buffer::empty(area);
    CompositePaneWidget::new(wm.active_pane(), true).render(area, &mut buf_sextant);
    let has_sextant = (0..20).any(|y| {
        (0..60).any(|x| {
            buf_sextant
                .cell((x, y))
                .and_then(|c| c.symbol().chars().next())
                .is_some_and(|ch| {
                    (0x1fb00..=0x1fb3b).contains(&(ch as u32)) || matches!(ch, '▌' | '▐' | '█')
                })
        })
    });
    assert!(has_sextant, "expected Unicode 2x3 Sextant glyphs on canvas");

    // 3. Quadrant mode (2x2 block glyphs)
    wm.execute_canvas_action("mode quadrant");
    let mut buf_quad = Buffer::empty(area);
    CompositePaneWidget::new(wm.active_pane(), true).render(area, &mut buf_quad);
    let has_quad = (0..20).any(|y| {
        (0..60).any(|x| {
            buf_quad
                .cell((x, y))
                .and_then(|c| c.symbol().chars().next())
                .is_some_and(|ch| "▘▝▀▖▌▞▛▗▚▐▜▄▙▟█".contains(ch))
        })
    });
    assert!(has_quad, "expected 2x2 Quadrant glyphs on canvas");

    // 4. GuiWorkspaceRenderer SVG & PPM export
    let mut gui = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    gui.render_workspace(&wm, area, &mut surface);
    let svg = surface.export_svg("Tek 4014 + Canvas");
    assert!(svg.contains("<svg"));
    let ppm = surface.export_ppm();
    assert!(ppm.starts_with(b"P6\n480 320\n255\n"));
}

#[test]
fn colon_canvas_osc1337_and_rpc_mcp_canvas_endpoints_control_scene_end_to_end() {
    let mut cp = AutomationControlPlane::new_headless(24, 80);

    // 1. In-band OSC 1337;Canvas=... sequence fed via PTY output
    cp.wm
        .active_pane_mut()
        .process_pty_output(b"\x1b]1337;Canvas=line 1 1 30 15\x07");
    assert_eq!(cp.wm.active_pane().canvas_scene().primitives.len(), 1);

    // 2. :canvas colon command
    let cmd = parse_colon_command(":canvas tek demo").unwrap().unwrap();
    execute_wm_colon_command(&mut cp.wm, cmd);
    assert!(cp.wm.active_pane().vt().tektronix().active);

    // 3. JSON-RPC canvas.command
    let res = cp
        .dispatch_method(
            "canvas.command",
            &JsonValue::obj([("command", JsonValue::String("plot bar 5,15,25".to_string()))]),
        )
        .expect("canvas.command plot bar");
    assert_eq!(res.get("plots").and_then(JsonValue::as_usize), Some(1));
    assert_eq!(res.get("primitives").and_then(JsonValue::as_usize), Some(1));
    assert_eq!(
        res.get("tektronix_active").and_then(JsonValue::as_bool),
        Some(true)
    );
}
