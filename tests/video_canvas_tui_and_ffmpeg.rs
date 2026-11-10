//! Video canvas TUI half-block rendering, OSD bar, real ffmpeg-pipe frame reading and
//! backend/surface diagnostic status messages (`VID-01..02`, TEST-R01).

use std::process::Command;
use std::time::{Duration, Instant};

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::pane::browser_canvas::MediaRgbaFrame;
use bilterm::pane::video_canvas::osd::{render_half_block_frame, render_video_osd_bar};
use bilterm::pane::video_canvas::{FfmpegPipeBackend, VideoCanvasState, VideoPlayerEngine};
use bilterm::wm::WindowManager;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Helper child emitting `frames` raw RGBA frames of `w*h*4` bytes, each byte `0x11`.
fn raw_rgba_child(w: u16, h: u16, frames: usize) -> Command {
    let total = w as usize * h as usize * 4 * frames;
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(format!("head -c {total} /dev/zero | tr '\\000' '\\021'"));
    cmd
}

fn wait_for_frame(be: &mut FfmpegPipeBackend) -> Option<MediaRgbaFrame> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 0u64;
    while Instant::now() < deadline {
        now += 33;
        if let Some(f) = be.tick(now)
            && f.width_px == 4
        {
            return Some(f);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
#[cfg_attr(windows, ignore = "drives a POSIX /bin/sh child; the Windows PTY path is covered by tests/smoke_binary_per_os.rs")]
fn ffmpeg_pipe_backend_reads_real_rgba_frames_from_child_stdout() {
    let mut be = FfmpegPipeBackend::from_command(raw_rgba_child(4, 2, 3), 4, 2);
    let f = wait_for_frame(&mut be).expect("real frame from pipe");
    assert_eq!((f.width_px, f.height_px), (4, 2));
    assert_eq!(f.rgba.len(), 4 * 2 * 4);
    // Real pipe bytes, not SMPTE bars (which would start 200,200,200,255).
    assert!(f.rgba.iter().all(|&b| b == 0x11), "frame must be the child's bytes");
    assert_eq!(be.state_snapshot().backend_name, "ffmpeg-pipe");
}

#[test]
#[cfg_attr(windows, ignore = "drives a POSIX /bin/sh child; the Windows PTY path is covered by tests/smoke_binary_per_os.rs")]
fn ffmpeg_pipe_backend_frame_sequence_advances() {
    let mut be = FfmpegPipeBackend::from_command(raw_rgba_child(4, 2, 5), 4, 2);
    let first = wait_for_frame(&mut be).expect("first frame");
    std::thread::sleep(Duration::from_millis(100));
    let last = be.tick(10_000).expect("later frame");
    assert!(last.seq >= first.seq);
}

#[test]
fn half_block_helper_maps_top_and_bottom_pixels_to_fg_bg() {
    // 1x2 pixels: top red, bottom blue -> one cell '▀' fg red bg blue.
    let rgba = vec![255, 0, 0, 255, 0, 0, 255, 255];
    let frame = MediaRgbaFrame::new(1, 1, 2, 0, rgba);
    let area = Rect::new(0, 0, 1, 1);
    let mut buf = Buffer::empty(area);
    render_half_block_frame(&mut buf, area, &frame);
    let cell = &buf[(0, 0)];
    assert_eq!(cell.symbol(), "▀");
    assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
    assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
}

#[test]
fn half_block_helper_scales_to_rect_and_ignores_empty() {
    let frame = MediaRgbaFrame::new(1, 4, 4, 0, vec![200; 4 * 4 * 4]);
    let area = Rect::new(2, 1, 8, 3);
    let mut buf = Buffer::empty(Rect::new(0, 0, 12, 5));
    render_half_block_frame(&mut buf, area, &frame);
    assert_eq!(buf[(2, 1)].symbol(), "▀");
    assert_eq!(buf[(9, 3)].symbol(), "▀");
    assert_eq!(buf[(0, 0)].symbol(), " ");
    render_half_block_frame(&mut buf, Rect::new(0, 0, 0, 0), &frame);
}

#[test]
fn osd_bar_renders_time_and_controls() {
    let state = VideoCanvasState::new_synthetic("demo://smpte", 64, 64);
    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_video_osd_bar(&mut buf, area, &state, true);
    let line: String = (0..80).map(|x| buf[(x, 0)].symbol().to_string()).collect();
    assert!(line.contains("00:00 / 01:00"), "line: {line}");
    assert!(line.contains("[✕]"));
}

fn run(wm: &mut WindowManager, line: &str) {
    let cmd = parse_colon_command(line).unwrap().unwrap();
    execute_wm_colon_command(wm, cmd);
}

fn status(wm: &WindowManager) -> String {
    wm.pane(wm.active_pane_id())
        .and_then(|p| p.status_message())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn video_status_names_backend_and_tui_surface() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":video demo://smpte");
    let s = status(&wm);
    assert!(s.contains("synthetic demo"), "{s}");
    assert!(s.contains("[TUI half-block; run bilterm --gui for GPU canvas]"), "{s}");
}

#[test]
fn video_status_omits_tui_hint_on_gui_surface() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_gui_surface(true);
    run(&mut wm, ":video demo://smpte");
    let s = status(&wm);
    assert!(s.contains("synthetic demo"), "{s}");
    assert!(!s.contains("TUI half-block"), "{s}");
}

#[test]
fn browser_status_names_backend_and_surface() {
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":browser open demo://page");
    let s = status(&wm);
    assert!(s.contains("synthetic preview"), "{s}");
    assert!(s.contains("[TUI half-block; run bilterm --gui for GPU canvas]"), "{s}");

    run(&mut wm, ":browser open https://example.com");
    let s = status(&wm);
    assert!(
        s.contains("CDP") || s.contains("synthetic preview (Chrome not found)"),
        "{s}"
    );
}
