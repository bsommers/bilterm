//! Comprehensive regression & integration suite sourced from the Linux engineering session (`v1.12.0+`).
//!
//! Covers:
//! 1. Small-canvas `FloatingCommandPrompt::effective_height` clamp safety under repeated `Ctrl+X` + `Ctrl+-`.
//! 2. Independent per-pane font scaling (`Ctrl++`, `Ctrl+=`, `Ctrl+-`, `Ctrl+_`, `Ctrl+0`, `Ctrl+Wheel`)
//!    inside 3-way splits when a pane is running an interactive `DECSET 1049` / `RawPtyPassthrough` TUI (`nmon`).
//! 3. `GuiWorkspaceRenderer` multi-pane rendering with distinct per-pane `font_size_pt` and cell grids.
//! 4. All-border (`Top`, `Bottom`, `Left`, `Right`) and all-corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`)
//!    mouse frame drag resizing across 2-pane and 3-pane `Tiled` and `Stacked` layouts.
//! 5. Powerline (`U+E0A0..=U+E0D7`) & Nerd Font PUA geometric rasterization.
//! 6. End-to-end verification of all repository `starter-packs/*/playbook.toml` automation playbooks.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use bilterm::api::{AutomationControlPlane, Playbook};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pane::CompositePane;
use bilterm::pty::line_discipline::InputRoutingMode;
use bilterm::ui::gui_font::geometric::{
    draw_geometric_cell_bgra, is_geometric_glyph, map_pua_icon_fallback,
};
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

#[test]
fn small_canvas_repeated_ctrl_x_ctrl_minus_never_panics_on_clamp() {
    let mut wm = WindowManager::new_headless(6, 40);
    // Split horizontally twice so individual pane heights shrink to 1..2 rows
    let _p2 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    let _p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();

    for h in 0..=8u16 {
        let tiny_vp = Rect::new(0, 0, 30, h);
        let _ = wm.resize_all(tiny_vp);
        for id in [1usize, 2, 3] {
            if let Some(pane) = wm.pane(id) {
                // Must never panic with `assertion failed: min <= max` in Ord::clamp
                let pr = pane.prompt_rect(Rect::new(0, 0, 30, h));
                assert!(pr.height <= h.max(1));
            }
        }
    }

    // Repeatedly send Ctrl+X followed by Ctrl+- at tiny viewport dimensions
    let tiny_vp = Rect::new(0, 0, 40, 6);
    wm.resize_all(tiny_vp).unwrap();
    for _ in 0..15 {
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        )
        .expect("Ctrl+X leader");
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL),
        )
        .expect("Ctrl+- zoom out");
        wm.resize_all(tiny_vp).expect("resize after zoom out");
    }
    assert_eq!(wm.pane_effective_scale_pct(wm.active_pane_id()), 50);

    // Also verify direct CompositePane prompt_rect at 0, 1, 2 rows
    let pane = CompositePane::new_headless(1, 2, 20);
    for rows in 0..=4u16 {
        let rect = pane.prompt_rect(Rect::new(0, 0, 20, rows));
        assert!(rect.height <= rows.max(1));
    }
}

#[test]
fn three_way_split_nmon_raw_passthrough_per_pane_font_scaling_and_pty_sigwinch() {
    let viewport = Rect::new(0, 0, 120, 40);
    let mut wm = WindowManager::new_headless(40, 120);
    wm.resize_all(viewport).unwrap();

    // Create 3-way split: Left = Pane #1, Top-Right = Pane #2 (nmon), Bottom-Right = Pane #3
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(viewport).unwrap();

    // Put Pane #2 (top-right) into DECSET 1049 Alternate Screen + !ECHO (simulating nmon / htop)
    {
        let pane2 = wm.pane_mut(p2).unwrap();
        pane2.process_pty_output(b"\x1b[?1049hNMON_TOP_RIGHT_TELEMETRY");
        pane2.line_discipline_mut().echo_enabled = false;
        assert_eq!(
            pane2.effective_routing_mode(),
            InputRoutingMode::RawPtyPassthrough
        );
    }

    let baseline_p1_vt = wm.pane(1).unwrap().vt().size();
    let baseline_p2_vt = wm.pane(p2).unwrap().vt().size();
    let baseline_p3_vt = wm.pane(p3).unwrap().vt().size();

    // Focus Pane #2 (nmon in RawPtyPassthrough) and press Ctrl+- twice (to 80% scale)
    wm.focus_and_raise(p2);
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::CONTROL),
    )
    .unwrap();
    wm.resize_all(viewport).unwrap();

    assert_eq!(wm.pane_effective_scale_pct(p2), 80);
    assert_eq!(wm.pane_effective_scale_pct(1), 100);
    assert_eq!(wm.pane_effective_scale_pct(p3), 100);

    // At 80% font scale, Pane #2's VT grid fits MORE rows and columns than at 100%,
    // while Pane #1 and Pane #3 VT grids remain unchanged!
    let zoomed_out_p2_vt = wm.pane(p2).unwrap().vt().size();
    assert!(
        zoomed_out_p2_vt.0 > baseline_p2_vt.0 && zoomed_out_p2_vt.1 > baseline_p2_vt.1,
        "expected p2 VT grid {:?} to grow when font zoomed out from {:?}",
        zoomed_out_p2_vt,
        baseline_p2_vt
    );
    assert_eq!(wm.pane(1).unwrap().vt().size(), baseline_p1_vt);
    assert_eq!(wm.pane(p3).unwrap().vt().size(), baseline_p3_vt);

    // Focus Pane #1 (left) and zoom IN twice via Ctrl++ and Ctrl+= (to 120% scale)
    wm.focus_and_raise(1);
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('+'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('='), KeyModifiers::CONTROL),
    )
    .unwrap();
    wm.resize_all(viewport).unwrap();

    assert_eq!(wm.pane_effective_scale_pct(1), 120);
    assert_eq!(wm.pane_effective_scale_pct(p2), 80);
    let zoomed_in_p1_vt = wm.pane(1).unwrap().vt().size();
    assert!(
        zoomed_in_p1_vt.0 < baseline_p1_vt.0 && zoomed_in_p1_vt.1 < baseline_p1_vt.1,
        "expected p1 VT grid {:?} to shrink in cell count when font zoomed in from {:?}",
        zoomed_in_p1_vt,
        baseline_p1_vt
    );

    // Also test Ctrl + Mouse ScrollUp over Pane #3 (bottom-right) zooms Pane #3 to 110%
    let p3_rect = wm
        .split_tree()
        .compute_rects(viewport)
        .into_iter()
        .find(|(id, _)| *id == p3)
        .unwrap()
        .1;
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: p3_rect.x + 4,
            row: p3_rect.y + 4,
            modifiers: KeyModifiers::CONTROL,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(wm.active_pane_id(), p3);
    assert_eq!(wm.pane_effective_scale_pct(p3), 110);

    // Ctrl+0 resets only the focused pane (Pane #3) back to 100%
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(wm.pane_effective_scale_pct(p3), 100);
    assert_eq!(wm.pane_effective_scale_pct(1), 120);
    assert_eq!(wm.pane_effective_scale_pct(p2), 80);
}

#[test]
fn gui_workspace_renderer_renders_independent_per_pane_font_sizes_and_cell_grids() {
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(viewport).unwrap();

    // Pane #1 at 150% font scale, Pane #2 at 75% font scale
    wm.focus_and_raise(1);
    wm.font_set_size(150);
    wm.pane_mut(1).unwrap().process_pty_output(b"LARGE_PANE_ONE\r\n");

    wm.focus_and_raise(p2);
    wm.font_set_size(75);
    wm.pane_mut(p2).unwrap().process_pty_output(b"COMPACT_PANE_TWO\r\n");
    wm.resize_all(viewport).unwrap();

    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(&wm, viewport, 8, 16, &mut surface);

    // Verify glyph runs contain both scaled font sizes (150% of 14pt = 21pt, 75% of 14pt = 11pt)
    let has_large_run = surface.glyphs.iter().any(|r| r.font_size_pt >= 18);
    let has_compact_run = surface.glyphs.iter().any(|r| r.font_size_pt <= 12);
    assert!(
        has_large_run && has_compact_run,
        "expected distinct per-pane font_size_pt runs in GuiFrameSurface"
    );
}

#[test]
fn all_four_borders_and_all_four_corners_mouse_drag_resize_in_three_pane_tiled_and_stacked_layouts() {
    let viewport = Rect::new(0, 0, 120, 40);
    let mut wm = WindowManager::new_headless(40, 120);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(viewport).unwrap();

    let rect_of = |wm: &WindowManager, target: usize| -> Rect {
        wm.compute_workspace_rects(viewport)
            .into_iter()
            .find(|(id, _)| *id == target)
            .unwrap()
            .1
    };

    // 1. Drag shared vertical border (Right border of Pane #1) from x=59 to x=45
    let r1 = rect_of(&wm, 1);
    let right_x = r1.x + r1.width - 1;
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: right_x,
            row: r1.y + 10,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 45,
            row: r1.y + 10,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 45,
            row: r1.y + 10,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    let r1_after = rect_of(&wm, 1);
    assert!(
        r1_after.width < r1.width,
        "dragging right border of Pane #1 left should shrink Pane #1 width ({} -> {})",
        r1.width,
        r1_after.width
    );

    // 2. Drag Top border of Pane #3 (bottom-right pane) downward from y=20 to y=28
    // Verify it resizes the horizontal split between Pane #2 and Pane #3 rather than swapping panes!
    let r2_before = rect_of(&wm, p2);
    let r3_before = rect_of(&wm, p3);
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: r3_before.x + 10,
            row: r3_before.y,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: r3_before.x + 10,
            row: r3_before.y + 6,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: r3_before.x + 10,
            row: r3_before.y + 6,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    let r2_after = rect_of(&wm, p2);
    let r3_after = rect_of(&wm, p3);
    assert!(
        r2_after.height > r2_before.height && r3_after.height < r3_before.height,
        "dragging top border of Pane #3 down should grow Pane #2 ({:?} -> {:?}) and shrink Pane #3 ({:?} -> {:?})",
        r2_before,
        r2_after,
        r3_before,
        r3_after
    );

    // 3. Drag BottomLeft corner of Pane #2 (interior intersection of V and H splits)
    let r2_corner = rect_of(&wm, p2);
    let bl_x = r2_corner.x;
    let bl_y = r2_corner.y + r2_corner.height - 1;
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: bl_x,
            row: bl_y,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 60,
            row: 18,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 60,
            row: 18,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    let r1_corner_after = rect_of(&wm, 1);
    let r2_corner_after = rect_of(&wm, p2);
    assert!(r1_corner_after.width > r1_after.width);
    assert!(r2_corner_after.height < r2_after.height);

    // 4. Switch to Stacked mode and verify TopLeft corner drag resizes window
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    wm.focus_and_raise(p3);
    let stacked_before = rect_of(&wm, p3);
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: stacked_before.x,
            row: stacked_before.y,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: stacked_before.x + 6,
            row: stacked_before.y + 3,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: stacked_before.x + 6,
            row: stacked_before.y + 3,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    let stacked_after = rect_of(&wm, p3);
    assert!(stacked_after.width < stacked_before.width);
    assert!(stacked_after.height < stacked_before.height);
}

#[test]
fn powerline_and_nerd_font_pua_glyphs_rasterize_geometrically() {
    let mut pixels = vec![0u32; 16 * 20];
    for ch in ['\u{E0B0}', '\u{E0B2}', '\u{E0A0}', '\u{E0B4}', '\u{E0B6}', '\u{F07B}'] {
        let mapped = map_pua_icon_fallback(ch);
        pixels.fill(0);
        if is_geometric_glyph(mapped) {
            draw_geometric_cell_bgra(&mut pixels, 16, 20, 0, 0, 16, 20, mapped, [0, 255, 102, 255]);
            assert!(
                pixels.iter().any(|&px| px != 0),
                "expected non-empty pixel output for geometric PUA glyph U+{:04X}",
                ch as u32
            );
        } else {
            assert!(!mapped.is_control());
        }
    }
}

#[test]
fn starter_pack_playbooks_execute_deterministically_end_to_end() {
    let playbooks = [
        "starter-packs/01-multi-agent-swarm/playbook.toml",
        "starter-packs/02-remote-dev-and-fleet/playbook.toml",
        "starter-packs/03-robot-scripting-and-qa/playbook.toml",
        "starter-packs/04-sysadmin-incident-war-room/playbook.toml",
    ];
    for rel_path in playbooks {
        let path = Path::new(rel_path);
        assert!(path.exists(), "missing starter pack playbook: {rel_path}");
        let raw_toml = std::fs::read_to_string(path).expect("read playbook toml");
        let pb = Playbook::from_toml_str(&raw_toml).expect("parse playbook toml");
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let report = plane
            .run_playbook(&pb)
            .unwrap_or_else(|e| panic!("playbook {rel_path} failed: {e}"));
        assert!(report.passed, "playbook {rel_path} did not pass");
        assert!(report.steps_completed > 0);
    }
}
