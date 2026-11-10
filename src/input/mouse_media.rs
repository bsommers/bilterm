//! Mouse event routing for Browser Navigation Bar and Video OSD controls (`WEB-01`, `VID-01`).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::input::AppAction;
use crate::pane::browser_canvas::{
    BrowserNavHit, MediaMouseEvent, MediaMouseEventKind, hit_test_browser_nav_bar,
};
use crate::pane::video_canvas::{VideoOsdHit, hit_test_video_osd_bar};
use crate::pane::CompositePane;

/// Routes mouse clicks, drags, and wheel events on active media canvases (nav bar, OSD, or viewport).
pub fn handle_media_canvas_mouse(
    pane: &mut CompositePane,
    mouse: MouseEvent,
    canvas_rect: Rect,
    in_prompt: bool,
) -> Option<AppAction> {
    if canvas_rect.width == 0 || canvas_rect.height <= 1 {
        return None;
    }

    let col = mouse.column;
    let row = mouse.row;

    // 1. Browser Navigation Bar (top row)
    if pane.browser_canvas().is_some() && row == canvas_rect.y {
        let nav_rect = Rect::new(canvas_rect.x, canvas_rect.y, canvas_rect.width, 1);
        if let Some(hit) = hit_test_browser_nav_bar(nav_rect, col, row)
            && let MouseEventKind::Down(MouseButton::Left) = mouse.kind
        {
            match hit {
                BrowserNavHit::Back => {
                    let _ = pane.browser_canvas_mut().map(|b| b.go_back());
                }
                BrowserNavHit::Forward => {
                    let _ = pane.browser_canvas_mut().map(|b| b.go_forward());
                }
                BrowserNavHit::Reload => {
                    let _ = pane.browser_canvas_mut().map(|b| b.reload());
                }
                BrowserNavHit::ToggleFocus => {
                    if let Some(b) = pane.browser_canvas_mut() {
                        b.toggle_focus();
                    }
                }
                BrowserNavHit::Close => {
                    pane.close_browser();
                }
                BrowserNavHit::UrlBar => {
                    if let Some(b) = pane.browser_canvas_mut() {
                        b.begin_url_edit();
                    }
                }
            }
            return Some(AppAction::Handled);
        }
    }

    // 2. Video OSD Transport Bar (bottom row)
    let osd_y = canvas_rect.bottom().saturating_sub(1);
    if pane.video_canvas().is_some() && row == osd_y {
        let osd_rect = Rect::new(canvas_rect.x, osd_y, canvas_rect.width, 1);
        if let Some(hit) = hit_test_video_osd_bar(osd_rect, col, row)
            && let MouseEventKind::Down(MouseButton::Left) = mouse.kind
        {
            match hit {
                    VideoOsdHit::TogglePlayPause => {
                        let _ = pane.video_canvas_mut().map(|v| v.toggle_play_pause());
                    }
                    VideoOsdHit::SeekBack => {
                        let _ = pane.video_canvas_mut().map(|v| v.seek_relative(-5000));
                    }
                    VideoOsdHit::SeekForward => {
                        let _ = pane.video_canvas_mut().map(|v| v.seek_relative(5000));
                    }
                    VideoOsdHit::ScrubBar { pct } => {
                        let _ = pane.video_canvas_mut().map(|v| v.seek_percent(pct));
                    }
                    VideoOsdHit::ToggleMute => {
                        let _ = pane.video_canvas_mut().map(|v| v.toggle_mute());
                    }
                    VideoOsdHit::ToggleLoop => {
                        let _ = pane.video_canvas_mut().map(|v| v.toggle_loop());
                    }
                    VideoOsdHit::Speed => {
                        if let Some(v) = pane.video_canvas_mut() {
                            let next_speed = match v.snapshot().speed_x100 {
                                100 => 150,
                                150 => 200,
                                200 => 50,
                                _ => 100,
                            };
                            let _ = v.set_speed(next_speed);
                        }
                    }
                    VideoOsdHit::Close => {
                        pane.close_video();
                    }
                }
                return Some(AppAction::Handled);
            }
        }

    // 3. Viewport forwarding
    if pane.browser_canvas().is_some() && row > canvas_rect.y && row < canvas_rect.bottom() {
        if in_prompt {
            return None;
        }
        let rel_x = col.saturating_sub(canvas_rect.x);
        let rel_y = row.saturating_sub(canvas_rect.y + 1);
        let x_px = rel_x.saturating_mul(10);
        let y_px = rel_y.saturating_mul(18);

        let kind = match mouse.kind {
            MouseEventKind::Down(mb) => Some(MediaMouseEventKind::Down(mb)),
            MouseEventKind::Up(mb) => Some(MediaMouseEventKind::Up(mb)),
            MouseEventKind::Drag(mb) => Some(MediaMouseEventKind::Drag(mb)),
            MouseEventKind::Moved => Some(MediaMouseEventKind::Move),
            MouseEventKind::ScrollDown => Some(MediaMouseEventKind::Wheel { delta_x: 0, delta_y: 53 }),
            MouseEventKind::ScrollUp => Some(MediaMouseEventKind::Wheel { delta_x: 0, delta_y: -53 }),
            _ => None,
        };

        if let Some(k) = kind {
            let ev = MediaMouseEvent {
                kind: k,
                x_px,
                y_px,
                modifiers: mouse.modifiers,
            };
            let _ = pane.browser_canvas_mut().map(|b| b.dispatch_mouse(ev));
            return Some(AppAction::Handled);
        }
    }

    if pane.video_canvas().is_some() && row >= canvas_rect.y && row < osd_y {
        if in_prompt {
            return None;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                let _ = pane.video_canvas_mut().map(|v| v.volume_delta(5));
                return Some(AppAction::Handled);
            }
            MouseEventKind::ScrollDown => {
                let _ = pane.video_canvas_mut().map(|v| v.volume_delta(-5));
                return Some(AppAction::Handled);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let _ = pane.video_canvas_mut().map(|v| v.toggle_play_pause());
                return Some(AppAction::Handled);
            }
            _ => {}
        }
    }

    None
}
