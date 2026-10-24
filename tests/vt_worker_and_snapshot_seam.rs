use std::time::{Duration, Instant};

use bilterm::pty::PtyConfig;
use bilterm::ui::renderer::{
    RatatuiWorkspaceRenderer, WorkspaceFrameRenderer, WorkspaceFrameSnapshot,
};
use bilterm::vt::VtParser;
use bilterm::vt::snapshot::{RgbTriplet, UnderlineKind};
use bilterm::vt::worker::{VtWorkerCommand, VtWorkerEvent, VtWorkerHandle};
use bilterm::wm::WindowManager;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn grid_snapshot_captures_extended_sgr_underlines_overline_and_underline_rgb() {
    let mut vt = VtParser::new(10, 40, 200);
    // SGR 4:3 (curly underline) + SGR 58;2;255;85;85 (red underline color) + SGR 53 (overline) + Bold
    vt.process(b"\x1b[1;4:3;53;58;2;255;85;85mERR\x1b[0m");

    let snap = vt.snapshot();
    assert_eq!(snap.rows, 10);
    assert_eq!(snap.cols, 40);

    let c0 = snap.cell(0, 0).expect("cell (0,0)");
    assert_eq!(c0.grapheme, "E");
    assert!(c0.bold);
    assert_eq!(c0.underline, UnderlineKind::Curly);
    assert_eq!(c0.underline_color, Some(RgbTriplet::new(255, 85, 85)));

    // Verify SGR 0 reset cleared active extended SGR state
    assert_eq!(vt.active_underline_kind(), UnderlineKind::None);
    assert_eq!(vt.active_underline_color(), None);
    assert!(!vt.is_overline());
}

#[test]
fn horizontal_resize_reflows_soft_wrapped_lines_and_emits_in_band_2048_report() {
    let mut vt = VtParser::new(6, 20, 200);
    // Enable DECSET ?2048 in-band resize notifications
    vt.process(b"\x1b[?2048h");
    // Write a 35-character line without newline -> soft-wraps across row 0 (20 chars) and row 1 (15 chars)
    let line = "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456789";
    vt.process(line.as_bytes());

    let snap_narrow = vt.snapshot();
    assert_eq!(snap_narrow.row_text(0), "ABCDEFGHIJKLMNOPQRST");
    assert_eq!(snap_narrow.row_text(1), "UVWXYZ123456789");

    // Now widen the pane to 50 columns -> soft-wrapped line reflows onto row 0!
    vt.set_size(6, 50);
    let snap_wide = vt.snapshot();
    assert_eq!(snap_wide.row_text(0), line);
    assert_eq!(snap_wide.row_text(1), "");

    // Verify DECSET ?2048 emitted CSI 48 ; 6 ; 50 ; 0 ; 0 t
    let replies = vt.take_query_responses();
    assert_eq!(replies, vec![b"\x1b[48;6;50;0;0t".to_vec()]);
}

#[test]
fn dedicated_vt_worker_thread_processes_stream_and_publishes_snapshots() {
    let worker = VtWorkerHandle::spawn_headless(12, 60, 500).expect("spawn headless worker");
    worker.feed_output(
        b"\x1b]2;BILTERM-WORKER-TITLE\x07\x1b]7;file://localhost/tmp\x07\x1b]52;c;aGVsbG8=\x07Hello Worker Grid!\r\n",
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snap = worker.snapshot();
        if snap.row_text(0).contains("Hello Worker Grid!") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for VtWorkerHandle snapshot"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let events = worker.drain_events();
    assert!(
        events.contains(&VtWorkerEvent::TitleChanged(
            "BILTERM-WORKER-TITLE".to_string()
        ))
    );
    assert!(
        events.contains(&VtWorkerEvent::CwdChanged(std::path::PathBuf::from("/tmp")))
    );
    assert!(
        events.contains(&VtWorkerEvent::Osc52Clipboard(
            "\x1b]52;c;aGVsbG8=\x07".to_string()
        ))
    );

    // Resize worker and verify updated snapshot dimensions
    worker.resize(20, 80);
    let resize_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if worker.shared_snapshot().size() == (20, 80) {
            break;
        }
        assert!(
            Instant::now() < resize_deadline,
            "Timed out waiting for worker resize"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    worker.send(VtWorkerCommand::Shutdown);
}

#[test]
fn dedicated_vt_worker_thread_pipes_live_portable_pty_into_grid_snapshot() {
    let cfg = PtyConfig {
        rows: 10,
        cols: 60,
        program: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            "printf 'LIVE_PTY_SNAPSHOT_OK\\r\\n'".to_string(),
        ],
        ..Default::default()
    };

    let worker = VtWorkerHandle::spawn_pty(cfg, 500, || {}).expect("spawn live pty worker");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let snap = worker.snapshot();
        if snap.row_text(0).contains("LIVE_PTY_SNAPSHOT_OK")
            || snap.row_text(1).contains("LIVE_PTY_SNAPSHOT_OK")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for live portable-pty -> VtWorkerHandle snapshot"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn workspace_frame_snapshot_and_renderer_decouple_ui_from_terminal_core() {
    let viewport = Rect::new(0, 0, 100, 30);
    let mut wm = WindowManager::new_headless(30, 100);
    wm.pane_mut(1)
        .unwrap()
        .process_pty_output(b"Pane 1 Snapshot Text\r\n");
    let p2 = wm
        .split_focused(bilterm::wm::split_tree::SplitDirection::Vertical)
        .expect("split vertical");
    wm.pane_mut(p2)
        .unwrap()
        .process_pty_output(b"Pane 2 Snapshot Text\r\n");

    let frame = WorkspaceFrameSnapshot::capture(&wm, viewport);
    assert_eq!(frame.panes.len(), 2);
    assert!(frame.panes[0].grid.row_text(0).contains("Pane 1 Snapshot Text"));
    assert!(frame.panes[1].grid.row_text(0).contains("Pane 2 Snapshot Text"));

    let mut renderer = RatatuiWorkspaceRenderer;
    let mut buf = Buffer::empty(viewport);
    renderer.render_workspace(&wm, viewport, &mut buf);
    assert_eq!(buf.area, viewport);
}

#[test]
fn no_vt100_ctt_types_appear_outside_src_vt() {
    use std::path::Path;

    fn visit_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_rs_files(&path, out);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    out.push(path);
                }
            }
        }
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let vt_dir = src_dir.join("vt");
    let mut rs_files = Vec::new();
    visit_rs_files(&src_dir, &mut rs_files);
    assert!(!rs_files.is_empty());

    for file in rs_files {
        let content = std::fs::read_to_string(&file).expect("read source file");
        let lines = content.lines().count();
        assert!(
            lines <= 800,
            "{} exceeds 800-line ceiling ({} lines)",
            file.display(),
            lines
        );
        if !file.starts_with(&vt_dir) {
            assert!(
                !content.contains("vt100_ctt"),
                "{} leaked vt100_ctt outside src/vt/",
                file.display()
            );
        }
    }
}

#[test]
fn tmux_control_subprocess_polls_into_window_manager() {
    use bilterm::tmux::TmuxControlSubprocess;
    use bilterm::vt::{MockTerminalBackend, assert_terminal_backend_conformance};

    assert_terminal_backend_conformance(VtParser::new);
    assert_terminal_backend_conformance(MockTerminalBackend::new);

    let mut wm = WindowManager::new_headless(20, 80);
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let subproc = TmuxControlSubprocess::from_channels("bilterm-live", cmd_tx, line_rx);

    wm.attach_tmux_control(subproc);
    assert!(wm.is_tmux_control_attached());

    line_tx
        .send("%output %0 \\033[32mTMUX_SUBPROC_PANE_1\\033[0m\\015\\012".to_string())
        .unwrap();
    line_tx
        .send("%output %1 TMUX_SUBPROC_PANE_2\\015\\012".to_string())
        .unwrap();

    wm.poll_all_ptys();
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.pane(1).unwrap().vt().contents().contains("TMUX_SUBPROC_PANE_1"));
    assert!(wm.pane(2).unwrap().vt().contents().contains("TMUX_SUBPROC_PANE_2"));

    line_tx.send("%exit client detached".to_string()).unwrap();
    wm.poll_all_ptys();
    assert!(!wm.is_tmux_control_attached());
}
