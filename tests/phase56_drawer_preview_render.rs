use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::action::{Action, DrawerPreviewOp};
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::{SplitDirection, WindowManager, WorkspaceLayoutMode};

fn make_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().history_session = format!("p56_render_{}", std::process::id());
    wm
}

#[test]
fn preview_renders_occluded_and_minimized_window_live_output_canvas() {
    let mut wm = make_wm();
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(1)
        .unwrap()
        .process_pty_output(b"TOP_FOREGROUND_WINDOW_ONE\r\n");
    wm.pane_mut(p2)
        .unwrap()
        .process_pty_output(b"SECRET_OCCLUDED_BACKEND_LOGS_42\r\n");

    wm.toggle_minimize_pane(p2);
    wm.focus_and_raise(1);
    assert!(wm.z_stack().is_minimized(p2));
    assert_eq!(wm.active_pane_id(), 1);

    let area = Rect::new(0, 0, 120, 36);
    wm.resize_all(area).unwrap();

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Pane(p2)))
        .unwrap();
    assert!(wm.drawer().preview.active);

    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);

    assert!(
        snap.contains("PEEK PREVIEW"),
        "expected PEEK PREVIEW frame header in snapshot:\n{snap}"
    );
    assert!(
        snap.contains("[MINIMIZED]"),
        "expected [MINIMIZED] state tag in preview banner:\n{snap}"
    );
    assert!(
        snap.contains("SECRET_OCCLUDED_BACKEND_LOGS_42"),
        "expected live VT output from occluded/minimized pane #2 in preview:\n{snap}"
    );

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Commit))
        .unwrap();
    assert!(!wm.z_stack().is_minimized(p2));
    assert_eq!(wm.active_pane_id(), p2);
}

#[test]
fn preview_renders_live_background_session_and_saved_session_snapshot_card() {
    let mut wm = make_wm();
    let first_session = wm.config().history_session.clone();
    let _ = wm.create_new_session(Some("p56_bg_alpha"), false).unwrap();
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2)
        .unwrap()
        .process_pty_output(b"BG_SESSION_ALPHA_LIVE_STREAM\r\n");

    let _ = wm
        .move_window_to_session(Some(p2), &first_session, false)
        .unwrap();
    let area = Rect::new(0, 0, 120, 36);
    wm.resize_all(area).unwrap();

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Session(
        first_session,
    )))
    .unwrap();
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(
        snap.contains("BG_SESSION_ALPHA_LIVE_STREAM"),
        "expected live background session PTY stream in preview:\n{snap}"
    );

    wm.apply(Action::DrawerPreview(DrawerPreviewOp::Session(
        "unloaded_saved_profile".to_string(),
    )))
    .unwrap();
    let mut buf2 = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf2);
    let snap2 = buffer_to_snapshot_string(&buf2);
    assert!(
        snap2.contains("SAVED SESSION SNAPSHOT"),
        "expected saved session snapshot card in preview:\n{snap2}"
    );
}
