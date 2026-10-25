//! Integration tests for Phase 27: High-Resolution Anti-Aliased Pixel/GPU Canvas &
//! Adaptive SSH/GUI Graphics Engine (`CANV-02`, `SSH-GFX-01`, `CLI-GFX-01`).

use std::process::Command;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::config::BiltermConfig;
use bilterm::pane::{
    CanvasRenderBackend, CanvasSubcellMode, CompositePane, DeclarativePlot, InteractiveImageView,
    PlotKind,
};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::cursor::SpotlightMode;
use bilterm::ui::pixel_canvas::{PixelCanvasBuffer, wrap_tmux_dcs_passthrough};
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::wm::WindowManager;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[test]
fn pixel_canvas_buffer_renders_aa_primitives_plots_images_focal_lens_and_exports_ppm_svg() {
    let mut buf = PixelCanvasBuffer::new(160, 96, [0, 0, 0, 0]);
    buf.draw_line_aa(4.0, 4.0, 150.0, 88.0, (0, 255, 200), 1.0);
    buf.draw_bezier_aa(
        (8.0, 80.0),
        (48.0, 8.0),
        (110.0, 88.0),
        (152.0, 12.0),
        (255, 180, 40),
    );
    buf.draw_circle_aa(80.0, 48.0, 18.0, true, (120, 255, 140));
    buf.draw_rect_aa(12.0, 12.0, 32.0, 20.0, false, (255, 100, 220), 0.9);

    let plot = DeclarativePlot {
        title: "Latency P99".to_string(),
        kind: PlotKind::Line(vec![10.0, 24.0, 18.0, 46.0, 39.0, 72.0, 64.0, 92.0]),
        rgb: (85, 255, 255),
    };
    buf.render_plot_hires(&plot, 2.0, 2.0);

    let img = InteractiveImageView::new_test_pattern(16, 12);
    buf.render_image_bilinear(&img, 2.0, 2.0);

    // Verify anti-aliased pixels are non-empty inside circle center and along line
    let center_px = buf.pixel_at(80, 48);
    assert!(center_px[3] > 200, "circle interior should be opaque");

    // Verify smooth pixel-space radial focal zoom lens
    let cfg = BiltermConfig {
        focal_zoom_mode: SpotlightMode::Circle,
        focal_zoom_pct: 200,
        ..Default::default()
    };
    let zoomed = buf.apply_focal_lens(&cfg, (12, 40), 80, 24);
    assert_eq!((zoomed.width, zoomed.height), (160, 96));

    // Export Netpbm P6 .ppm and standalone .svg
    let ppm = buf.export_ppm();
    assert!(ppm.starts_with(b"P6\n160 96\n255\n"));
    assert_eq!(ppm.len(), "P6\n160 96\n255\n".len() + 160 * 96 * 3);

    let svg = buf.export_svg("HighResTest");
    assert!(svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("<title>HighResTest</title>"));
    assert!(svg.contains("fill=\"rgb("));
}

#[test]
fn adaptive_ssh_graphics_backend_negotiation_and_tmux_passthrough_encoders_work() {
    // 1. Kitty / Ghostty / WezTerm -> Kitty APC G
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-kitty"), None, None, Some("1"), None, None, None),
        CanvasRenderBackend::Kitty
    );
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-256color"), Some("ghostty"), None, None, Some("/usr/share/ghostty"), None, None),
        CanvasRenderBackend::Kitty
    );
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-256color"), Some("WezTerm"), None, None, None, Some("/usr/bin/wezterm"), None),
        CanvasRenderBackend::Kitty
    );

    // 2. iTerm2 -> OSC 1337 inline images
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-256color"), Some("iTerm.app"), None, None, None, None, None),
        CanvasRenderBackend::ITerm2
    );

    // 3. foot / Windows Terminal / xterm-sixel -> DEC Sixel DCS q
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("foot-extra"), None, None, None, None, None, None),
        CanvasRenderBackend::Sixel
    );
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-256color"), None, None, None, None, None, Some("wt-guid")),
        CanvasRenderBackend::Sixel
    );

    // 4. Plain SSH session with standard TERM=xterm-256color -> 24-bit TrueColor HalfBlock (▀/▄); legacy vt100 -> Braille
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("xterm-256color"), None, None, None, None, None, None),
        CanvasRenderBackend::HalfBlock
    );
    assert_eq!(
        CanvasRenderBackend::detect_from_vars(Some("vt100"), None, None, None, None, None, None),
        CanvasRenderBackend::Braille
    );

    // 5. Verify encoders with and without tmux DCS passthrough wrapping
    let mut buf = PixelCanvasBuffer::new(16, 12, [0, 0, 0, 0]);
    buf.draw_circle_aa(8.0, 6.0, 4.0, true, (0, 255, 180));

    let kitty_raw = buf.encode_kitty_apc(20, 6, false);
    assert!(kitty_raw.starts_with("\x1b_Ga=T,f=32,s=16,v=12,c=20,r=6,q=2;"));
    assert!(kitty_raw.ends_with("\x1b\\"));

    let kitty_tmux = buf.encode_kitty_apc(20, 6, true);
    assert!(kitty_tmux.starts_with("\x1bPtmux;\x1b\x1b_G"));
    assert!(kitty_tmux.ends_with("\x1b\\"));

    let sixel_raw = buf.encode_sixel_dcs(false);
    assert!(sixel_raw.starts_with("\x1bP0;1;q\"1;1;16;12"));
    let sixel_tmux = buf.encode_sixel_dcs(true);
    assert!(sixel_tmux.starts_with("\x1bPtmux;\x1b\x1bP0;1;q"));

    let iterm_raw = buf.encode_iterm2_osc1337(20, 6, false);
    assert!(iterm_raw.starts_with("\x1b]1337;File=inline=1;"));
    let iterm_tmux = wrap_tmux_dcs_passthrough(&iterm_raw);
    assert!(iterm_tmux.starts_with("\x1bPtmux;\x1b\x1b]1337;File=inline=1;"));
}

#[test]
fn canvas_hires_halfblock_mode_renders_truecolor_cells_and_preserves_floating_prompt_and_gui_surface() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().process_pty_output(b"SSH_HOST_BANNER\r\n");

    let cmd = parse_colon_command(":canvas hires").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);

    assert!(wm.active_pane().canvas_scene().active);
    assert_eq!(
        wm.active_pane().canvas_scene().subcell_mode,
        CanvasSubcellMode::HalfBlock
    );

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);

    let mut halfblock_cells = 0usize;
    for y in 0..24 {
        for x in 0..80 {
            let sym = buf[(x, y)].symbol();
            if sym == "▀" || sym == "▄" {
                halfblock_cells += 1;
            }
        }
    }
    assert!(
        halfblock_cells > 20,
        "expected HalfBlock ▀/▄ TrueColor cells in TUI buffer, got {halfblock_cells}"
    );

    // Floating Command Prompt must still be rendered on top
    let outer = wm.compute_workspace_rects(area)[0].1;
    let prompt_rect = wm.active_pane().prompt_rect(outer);
    let mut prompt_border_found = false;
    for x in prompt_rect.x..prompt_rect.x + prompt_rect.width {
        let s = buf[(x, prompt_rect.y)].symbol();
        if s != "▀" && s != "▄" && s != " " {
            prompt_border_found = true;
            break;
        }
    }
    assert!(prompt_border_found, "floating prompt border should occlude HalfBlock canvas");

    // Native GUI renderer attaches high-resolution PixelCanvasBuffer texture and rasterizes cleanly
    let mut gui_renderer = GuiWorkspaceRenderer::default();
    let mut surface = GuiFrameSurface::new(8, 16);
    gui_renderer.render_workspace(&wm, area, &mut surface);
    assert!(
        surface.images.iter().any(|img| img.id >= 900_000),
        "GuiWorkspaceRenderer should attach high-res PixelCanvasBuffer texture"
    );
    let (w_px, h_px, rgba) = surface.rasterize_rgba();
    assert_eq!((w_px, h_px), (640, 384));
    assert_eq!(rgba.len(), (640 * 384 * 4) as usize);
}

#[test]
fn canvas_export_encode_and_cli_bilterm_plot_work_end_to_end() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.execute_canvas_command("hires").unwrap();

    let tmp_dir = tempfile::tempdir().unwrap();
    let ppm_path = tmp_dir.path().join("scene.ppm");
    let svg_path = tmp_dir.path().join("scene.svg");

    let msg_ppm = pane
        .execute_canvas_command(&format!("export {}", ppm_path.display()))
        .unwrap();
    assert!(msg_ppm.contains("CANVAS EXPORT: wrote 640x384"));
    assert!(ppm_path.exists());

    let msg_svg = pane
        .execute_canvas_command(&format!("export {}", svg_path.display()))
        .unwrap();
    assert!(msg_svg.contains("CANVAS EXPORT: wrote 640x384"));
    let svg_content = std::fs::read_to_string(&svg_path).unwrap();
    assert!(svg_content.contains("<svg"));

    let msg_enc = pane.execute_canvas_command("encode kitty").unwrap();
    assert!(msg_enc.contains("CANVAS ENCODE (Kitty"));

    // Verify `bilterm plot` CLI subcommand
    let cli_svg = tmp_dir.path().join("cli_plot.svg");
    let out = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "plot",
            "line",
            "10,30,20,50,40,80",
            "-o",
            cli_svg.to_str().unwrap(),
        ])
        .output()
        .expect("run bilterm plot");
    assert!(out.status.success());
    assert!(cli_svg.exists());
}
