use ratatui::layout::Rect;

use bilterm::action::{Action, DrawerPreviewOp};
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::{DrawerEdge, DrawerPreviewTarget, SplitDirection, WindowManager};

fn make_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().history_session = format!("p56_state_{}", std::process::id());
    wm
}

#[test]
fn colon_parse_drawer_preview_and_peek_aliases() {
    assert_eq!(
        parse_colon_command(":drawer preview"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Toggle)))
    );
    assert_eq!(
        parse_colon_command(":drawer preview on"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::On)))
    );
    assert_eq!(
        parse_colon_command(":drawer preview off"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Off)))
    );
    assert_eq!(
        parse_colon_command(":drawer preview commit"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Commit)))
    );
    assert_eq!(
        parse_colon_command(":drawer preview auto"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::AutoToggle)))
    );
    assert_eq!(
        parse_colon_command(":drawer preview row 2"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Row(2))))
    );
    assert_eq!(
        parse_colon_command(":drawer preview pane 3"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Pane(3))))
    );
    assert_eq!(
        parse_colon_command(":drawer preview session alpha"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Session(
            "alpha".to_string()
        ))))
    );
    assert_eq!(
        parse_colon_command(":peek"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Toggle)))
    );
    assert_eq!(
        parse_colon_command(":peek on"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::On)))
    );
    assert_eq!(
        parse_colon_command(":peek off"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Off)))
    );
    assert_eq!(
        parse_colon_command(":peek commit"),
        Some(Ok(Action::DrawerPreview(DrawerPreviewOp::Commit)))
    );
}

#[test]
fn drawer_preview_geometry_respects_all_dock_edges_without_mutating_split_tree() {
    let mut wm = make_wm();
    let _p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let area = Rect::new(0, 0, 120, 40);
    wm.resize_all(area).unwrap();

    let rects_before = wm.compute_workspace_rects(area);
    wm.apply(Action::DrawerPreview(DrawerPreviewOp::On))
        .unwrap();
    assert!(wm.drawer().preview.active);
    assert!(wm.is_drawer_effectively_visible());

    for edge in [
        DrawerEdge::Left,
        DrawerEdge::Right,
        DrawerEdge::Top,
        DrawerEdge::Bottom,
    ] {
        wm.set_drawer_edge(Some(edge));
        let dr = wm.drawer_rect(area).expect("drawer rect");
        let pr = wm.drawer_preview_rect(area).expect("preview rect");
        assert!(pr.width >= 20 && pr.height >= 6);
        assert!(pr.x + pr.width <= area.width);
        assert!(pr.y + pr.height <= area.height);
        match edge {
            DrawerEdge::Left => assert_eq!(pr.x, dr.x + dr.width),
            DrawerEdge::Right => assert_eq!(pr.x + pr.width, dr.x),
            DrawerEdge::Top => assert_eq!(pr.y, dr.y + dr.height),
            DrawerEdge::Bottom => assert_eq!(pr.y + pr.height, dr.y),
        }
    }

    wm.set_drawer_edge(Some(DrawerEdge::Left));
    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Off))
        .unwrap();
    assert!(!wm.drawer().preview.active);
    assert!(wm.drawer_preview_rect(area).is_none());

    wm.close_drawer();
    let rects_after = wm.compute_workspace_rects(area);
    assert_eq!(rects_before, rects_after);
}

#[test]
fn drawer_preview_resolves_active_pane_background_pane_and_saved_session() {
    let mut wm = make_wm();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(1);

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Pane(p2)))
        .unwrap();
    assert!(wm.drawer().preview.active);
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(
        wm.resolve_drawer_preview_target(),
        Some(DrawerPreviewTarget::ActivePane(p2))
    );

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Commit))
        .unwrap();
    assert!(!wm.drawer().preview.active);
    assert_eq!(wm.active_pane_id(), p2);

    let first_session = wm.config().history_session.clone();
    let _ = wm.create_new_session(Some("bg_test_sess"), false).unwrap();
    let p_bg = wm.split_focused(SplitDirection::Vertical).unwrap();
    let _ = wm
        .move_window_to_session(Some(p_bg), &first_session, false)
        .unwrap();
    assert!(
        wm.window_manage()
            .background_sessions
            .contains_key(&first_session)
    );

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Session(
        first_session.clone(),
    )))
    .unwrap();
    assert!(wm.drawer().preview.active);
    match wm.resolve_drawer_preview_target() {
        Some(DrawerPreviewTarget::BackgroundPane { session, pane_id }) => {
            assert_eq!(session, first_session);
            assert_eq!(pane_id, p_bg);
        }
        other => panic!("expected BackgroundPane target, got {other:?}"),
    }

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Commit))
        .unwrap();
    assert_eq!(wm.config().history_session, first_session);
    assert_eq!(wm.active_pane_id(), p_bg);
}
