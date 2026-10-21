use bilterm::tmux::{TmuxControlBridge, TmuxControlEvent, parse_tmux_layout_panes};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::vt::GraphicProtocol;
use bilterm::vt::ghostty_backend::GhosttyCAbiTerminal;
use bilterm::wm::WindowManager;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[test]
fn ghostty_c_abi_vtable_terminal_processes_bytes_resizes_and_replies() {
    let mut term = GhosttyCAbiTerminal::new(24, 80, 1000);
    term.vt_write(b"\x1b[38;2;0;255;102mGHOSTTY_ABI_OK\x1b[0m\x1b[5n");

    let snap = term.snapshot();
    assert_eq!(snap.rows, 24);
    assert_eq!(snap.cols, 80);
    assert!(snap.row_text(0).starts_with("GHOSTTY_ABI_OK"));

    let replies = term.take_query_responses();
    assert_eq!(replies, vec![b"\x1b[0n".to_vec()]);

    term.resize(12, 40);
    let resized_snap = term.snapshot();
    assert_eq!(resized_snap.rows, 12);
    assert_eq!(resized_snap.cols, 40);
}

#[test]
fn kitty_graphics_and_sixel_decode_and_render_on_tui_and_gpu_surfaces() {
    let mut wm = WindowManager::new_headless(20, 80);

    // 1. Kitty Graphics query (a=q,i=42) -> replies with \x1b_Gi=42;OK\x1b\\
    wm.active_pane_mut().process_pty_output(b"\x1b_Ga=q,i=42;\x1b\\");
    let replies = wm.active_pane_mut().vt.take_query_responses();
    assert_eq!(replies, vec![b"\x1b_Gi=42;OK\x1b\\".to_vec()]);

    // 2. Kitty Graphics transmit & display (a=T,i=7,f=24,s=2,v=2,c=4,r=2) with 4 RGB pixels (/wAA = 255,0,0)
    // 4 RGB pixels = 12 bytes: [255,0,0, 0,255,0, 0,0,255, 255,255,0] -> Base64 "/wAAAP8AAAD///8A"
    wm.active_pane_mut()
        .process_pty_output(b"\x1b_Ga=T,i=7,f=24,s=2,v=2,c=4,r=2;/wAAAP8AAAD///8A\x1b\\");
    assert_eq!(wm.active_pane().vt.graphic_placements().len(), 1);
    assert_eq!(
        wm.active_pane().vt.graphic_placements()[0].protocol,
        GraphicProtocol::Kitty
    );
    assert_eq!(wm.active_pane().vt.graphic_placements()[0].id, 7);

    // 3. DEC Sixel placement (\x1bPq"1;1;8;6#1;2;100;0;0!8~\x1b\\)
    wm.active_pane_mut()
        .process_pty_output(b"\r\n\r\n\r\n\x1bPq\"1;1;8;6#1;2;100;0;0!8~\x1b\\");
    assert_eq!(wm.active_pane().vt.graphic_placements().len(), 2);
    assert_eq!(
        wm.active_pane().vt.graphic_placements()[1].protocol,
        GraphicProtocol::Sixel
    );

    // 4. Verify TUI OutputCanvas renders half-block '▀' glyphs for graphics
    let viewport = Rect::new(0, 0, 80, 20);
    let mut tui_buf = Buffer::empty(viewport);
    WorkspaceWidget::new(&wm).render(viewport, &mut tui_buf);
    let has_half_block = (0..20).any(|y| {
        (0..80).any(|x| {
            tui_buf
                .cell((x, y))
                .is_some_and(|c| c.symbol() == "▀")
        })
    });
    assert!(has_half_block, "TUI canvas should render half-block graphics cells");

    // 5. Verify GPU GuiWorkspaceRenderer produces quads, glyphs, images, and PPM framebuffer
    let mut gpu_renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut gpu_surface = GuiFrameSurface::new(8, 16);
    gpu_renderer.render_workspace(&wm, viewport, &mut gpu_surface);
    assert_eq!(gpu_surface.pixel_dimensions(), (640, 320));
    assert!(!gpu_surface.quads.is_empty());
    assert!(!gpu_surface.glyphs.is_empty());
    assert_eq!(gpu_surface.images.len(), 2);

    let ppm = gpu_surface.export_ppm();
    assert!(ppm.starts_with(b"P6\n640 320\n255\n"));

    // 6. Kitty Graphics delete (a=d,i=7) removes placement #7
    wm.active_pane_mut()
        .process_pty_output(b"\x1b_Ga=d,i=7;\x1b\\");
    assert_eq!(wm.active_pane().vt.graphic_placements().len(), 1);
}

#[test]
fn tmux_control_bridge_hydrates_layout_panes_and_routes_bidirectional_commands() {
    let mut wm = WindowManager::new_headless(24, 80);
    let mut bridge = TmuxControlBridge::new();

    // Parse layout descriptor with 3 panes (%0, %1, %2)
    let layout = "b25d,160x48,0,0{80x48,0,0,0,79x48,81,0[79x24,81,0,1,79x23,81,25,2]}";
    let parsed_panes = parse_tmux_layout_panes(layout);
    assert_eq!(parsed_panes, vec![0, 1, 2]);

    bridge.handle_control_line(&mut wm, &format!("%layout-change @1 {layout}"));
    assert_eq!(wm.panes.len(), 3);

    // Focus change from tmux (%window-pane-changed @1 %2) raises corresponding Bilterm pane
    let bilterm_pane_2 = *bridge.tmux_to_bilterm.get(&2).unwrap();
    bridge.handle_control_line(&mut wm, "%window-pane-changed @1 %2");
    assert_eq!(wm.active_pane_id(), bilterm_pane_2);

    // Submitting a prompt command in Bilterm pane routes hex send-keys back to tmux %2
    let cmd = bridge
        .route_prompt_submission(bilterm_pane_2, "echo hi")
        .expect("mapped tmux pane");
    assert_eq!(cmd, "send-keys -t %2 -H 65 63 68 6f 20 68 69 0a\n");

    let split_cmd = bridge.queue_split(bilterm_pane_2, true).unwrap();
    assert_eq!(split_cmd, "split-window -v -t %2\n");

    let resize_cmd = bridge.queue_resize_client(120, 40);
    assert_eq!(resize_cmd, "refresh-client -C 120x40\n");
    assert_eq!(bridge.take_outbound_commands().len(), 3);

    let ev = bridge.handle_control_line(&mut wm, "%exit detached");
    assert_eq!(
        ev,
        TmuxControlEvent::Exit {
            reason: Some("detached".to_string())
        }
    );
    assert!(bridge.exited);
}
