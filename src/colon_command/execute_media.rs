//! Execution for `:browser` (`BrowserOp`) and `:video` (`VideoOp`) actions (`WEB-01..02`, `VID-01..02`).

use crate::action::{BrowserOp, VideoOp};
use crate::pane::browser_canvas::{MediaMouseEvent, MediaMouseEventKind};
use crate::wm::WindowManager;
use crossterm::event::{KeyModifiers, MouseButton};

const TUI_SURFACE_HINT: &str = " [TUI half-block; run bilterm --gui for GPU canvas]";

fn surface_hint(wm: &WindowManager) -> &'static str {
    if wm.gui_surface { "" } else { TUI_SURFACE_HINT }
}

fn browser_backend_label(name: &str, url: &str) -> &'static str {
    if name == "cdp-chromium" {
        "CDP"
    } else if url.starts_with("demo://") || url.starts_with("about:") || url.starts_with("test://") {
        "synthetic preview"
    } else {
        "synthetic preview (Chrome not found)"
    }
}

fn video_backend_label(name: &str) -> &'static str {
    if name == "ffmpeg-pipe" { "ffmpeg" } else { "synthetic demo" }
}

pub fn execute_browser_command(wm: &mut WindowManager, op: BrowserOp) -> bool {
    let target_id = match &op {
        BrowserOp::Toggle => wm.active_pane_id(),
        BrowserOp::Open { pane_id, .. }
        | BrowserOp::Back { pane_id }
        | BrowserOp::Forward { pane_id }
        | BrowserOp::Reload { pane_id }
        | BrowserOp::Stop { pane_id }
        | BrowserOp::FocusViewport { pane_id, .. }
        | BrowserOp::Zoom { pane_id, .. }
        | BrowserOp::Click { pane_id, .. }
        | BrowserOp::Scroll { pane_id, .. }
        | BrowserOp::Close { pane_id } => pane_id.unwrap_or_else(|| wm.active_pane_id()),
    };

    let hint = surface_hint(wm);
    let Some(pane) = wm.pane_mut(target_id) else {
        return false;
    };

    match op {
        BrowserOp::Toggle => {
            if pane.browser_canvas().is_some() {
                pane.close_browser();
                pane.set_status_message(Some("Closed browser canvas".to_string()));
            } else {
                pane.open_browser("https://example.com");
                let label = pane
                    .browser_canvas()
                    .map(|b| browser_backend_label(b.backend_name(), "https://example.com"))
                    .unwrap_or("none");
                pane.set_status_message(Some(format!("Opened browser canvas: {label}{hint}")));
            }
        }
        BrowserOp::Open { url, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.navigate(&url);
            } else {
                pane.open_browser(&url);
            }
            let label = pane
                .browser_canvas()
                .map(|b| browser_backend_label(b.backend_name(), &url))
                .unwrap_or("none");
            pane.set_status_message(Some(format!("Browser opened: {url} ({label}){hint}")));
        }
        BrowserOp::Back { .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.go_back();
            }
        }
        BrowserOp::Forward { .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.go_forward();
            }
        }
        BrowserOp::Reload { .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.reload();
            }
        }
        BrowserOp::Stop { .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.stop();
            }
        }
        BrowserOp::FocusViewport { focused, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                match focused {
                    Some(f) => b.set_focus(f),
                    None => b.toggle_focus(),
                }
            }
        }
        BrowserOp::Zoom { delta_pct, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                b.zoom_delta(delta_pct);
            }
        }
        BrowserOp::Click { x_px, y_px, button, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let mb = match button {
                    1 => MouseButton::Right,
                    2 => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                let _ = b.dispatch_mouse(MediaMouseEvent {
                    kind: MediaMouseEventKind::Down(mb),
                    x_px,
                    y_px,
                    modifiers: KeyModifiers::NONE,
                });
            }
        }
        BrowserOp::Scroll { delta_x, delta_y, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.dispatch_mouse(MediaMouseEvent {
                    kind: MediaMouseEventKind::Wheel { delta_x, delta_y },
                    x_px: 0,
                    y_px: 0,
                    modifiers: KeyModifiers::NONE,
                });
            }
        }
        BrowserOp::Close { .. } => {
            pane.close_browser();
            pane.set_status_message(Some("Closed browser canvas".to_string()));
        }
    }
    false
}

pub fn execute_video_command(wm: &mut WindowManager, op: VideoOp) -> bool {
    let target_id = match &op {
        VideoOp::Toggle => wm.active_pane_id(),
        VideoOp::Open { pane_id, .. }
        | VideoOp::Play { pane_id }
        | VideoOp::Pause { pane_id }
        | VideoOp::TogglePlayPause { pane_id }
        | VideoOp::SeekRelative { pane_id, .. }
        | VideoOp::SeekPercent { pane_id, .. }
        | VideoOp::SeekAbsolute { pane_id, .. }
        | VideoOp::VolumeDelta { pane_id, .. }
        | VideoOp::ToggleMute { pane_id }
        | VideoOp::ToggleLoop { pane_id }
        | VideoOp::SetSpeed { pane_id, .. }
        | VideoOp::Close { pane_id } => pane_id.unwrap_or_else(|| wm.active_pane_id()),
    };

    let hint = surface_hint(wm);
    let Some(pane) = wm.pane_mut(target_id) else {
        return false;
    };

    match op {
        VideoOp::Toggle => {
            if pane.video_canvas().is_some() {
                pane.close_video();
                pane.set_status_message(Some("Closed video player canvas".to_string()));
            } else {
                pane.open_video("demo://smpte");
                let label = pane
                    .video_canvas()
                    .map(|v| video_backend_label(v.backend_name()))
                    .unwrap_or("none");
                pane.set_status_message(Some(format!(
                    "Opened video canvas (demo://smpte): {label}{hint}"
                )));
            }
        }
        VideoOp::Open { source, .. } => {
            pane.open_video(&source);
            let label = pane
                .video_canvas()
                .map(|v| video_backend_label(v.backend_name()))
                .unwrap_or("none");
            pane.set_status_message(Some(format!("Video player opened: {source} ({label}){hint}")));
        }
        VideoOp::Play { .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.play();
            }
        }
        VideoOp::Pause { .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.pause();
            }
        }
        VideoOp::TogglePlayPause { .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.toggle_play_pause();
            }
        }
        VideoOp::SeekRelative { delta_ms, .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.seek_relative(delta_ms);
            }
        }
        VideoOp::SeekPercent { pct, .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.seek_percent(pct);
            }
        }
        VideoOp::SeekAbsolute { position_ms, .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.seek_absolute(position_ms);
            }
        }
        VideoOp::VolumeDelta { delta_pct, .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.volume_delta(delta_pct);
            }
        }
        VideoOp::ToggleMute { .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.toggle_mute();
            }
        }
        VideoOp::ToggleLoop { .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.toggle_loop();
            }
        }
        VideoOp::SetSpeed { speed_x100, .. } => {
            if let Some(v) = pane.video_canvas_mut() {
                let _ = v.set_speed(speed_x100);
            }
        }
        VideoOp::Close { .. } => {
            pane.close_video();
            pane.set_status_message(Some("Closed video player canvas".to_string()));
        }
    }
    false
}
