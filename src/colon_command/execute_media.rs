//! Execution for `:browser` (`BrowserOp`) and `:video` (`VideoOp`) actions (`WEB-01..02`, `VID-01..02`).

use crate::action::{BrowserOp, VideoOp};
use crate::pane::browser_canvas::{MediaMouseEvent, MediaMouseEventKind};
use crate::wm::WindowManager;
use crossterm::event::{KeyModifiers, MouseButton};

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
                pane.set_status_message(Some("Opened browser canvas".to_string()));
            }
        }
        BrowserOp::Open { url, .. } => {
            if let Some(b) = pane.browser_canvas_mut() {
                let _ = b.navigate(&url);
            } else {
                pane.open_browser(&url);
            }
            pane.set_status_message(Some(format!("Browser opened: {url}")));
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
                pane.set_status_message(Some("Opened video canvas (demo://smpte)".to_string()));
            }
        }
        VideoOp::Open { source, .. } => {
            pane.open_video(&source);
            pane.set_status_message(Some(format!("Video player opened: {source}")));
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
