//! 1-row OSD Transport Bar renderer and mouse hit-testing (`VID-01..02`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::VideoCanvasState;
use crate::pane::browser_canvas::MediaRgbaFrame;

/// Hit-test target on the 1-row Video OSD Transport Bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoOsdHit {
    TogglePlayPause,
    SeekBack,
    SeekForward,
    ScrubBar { pct: u8 },
    ToggleMute,
    ToggleLoop,
    Speed,
    Close,
}

/// Formats milliseconds as `MM:SS`.
fn format_time(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

/// Terminal cell width of `s` (byte length over-counts multibyte glyphs).
fn cell_w(s: &str) -> u16 {
    unicode_width::UnicodeWidthStr::width(s) as u16
}

/// Renders the 1-row bottom Video OSD Transport Bar into `buf` across `osd_rect`.
pub fn render_video_osd_bar(
    buf: &mut Buffer,
    osd_rect: Rect,
    state: &VideoCanvasState,
    active: bool,
) {
    if osd_rect.height == 0 || osd_rect.width == 0 {
        return;
    }

    let y = osd_rect.y;
    let bg = if active {
        Color::Rgb(28, 20, 36)
    } else {
        Color::Rgb(18, 14, 24)
    };
    let btn_style = Style::default().fg(Color::Yellow).bg(bg).add_modifier(Modifier::BOLD);
    let time_style = Style::default().fg(Color::White).bg(bg);
    let alert_style = Style::default().fg(Color::LightRed).bg(bg).add_modifier(Modifier::BOLD);

    // Fill background
    for col in osd_rect.x..osd_rect.right() {
        buf[(col, y)].set_char(' ').set_style(Style::default().bg(bg));
    }

    let snap = state.snapshot();
    let mut cur_x = osd_rect.x;

    // [▶] or [⏸] Play/Pause
    let play_str = if snap.playing { "[⏸]" } else { "[▶]" };
    buf.set_string(cur_x, y, play_str, btn_style);
    cur_x += cell_w(play_str) + 1;

    // [⏮] -5s
    let back_str = "[⏮]";
    buf.set_string(cur_x, y, back_str, btn_style);
    cur_x += cell_w(back_str) + 1;

    // [⏭] +5s
    let fwd_str = "[⏭]";
    buf.set_string(cur_x, y, fwd_str, btn_style);
    cur_x += cell_w(fwd_str) + 1;

    // Right-aligned controls: [✕] Close, [🔁] Loop, [1.0x] Speed, [🔊 100%] Volume
    let close_str = "[✕]";
    let loop_str = if snap.looping { "[🔁]" } else { "[➔]" };
    let speed_str = format!("[{:.1}x]", snap.speed_x100 as f32 / 100.0);
    let vol_str = if snap.muted {
        "[🔇 MUTE]".to_string()
    } else {
        format!("[🔊 {}%]", snap.volume_pct)
    };

    let right_total = cell_w(close_str) + 1 + cell_w(loop_str) + 1 + cell_w(&speed_str) + 1 + cell_w(&vol_str) + 1;

    // Time text
    let time_str = format!("{} / {}", format_time(snap.position_ms), format_time(snap.duration_ms));

    // Middle area for scrub bar
    let available_middle = osd_rect.right().saturating_sub(cur_x).saturating_sub(right_total);
    if available_middle > 16 {
        let scrub_w = available_middle.saturating_sub(cell_w(&time_str) + 3) as usize;
        if scrub_w > 4 {
            let pct = if snap.duration_ms == 0 {
                0.0
            } else {
                (snap.position_ms as f64) / (snap.duration_ms as f64)
            };
            let head_idx = ((pct * (scrub_w as f64)) as usize).min(scrub_w.saturating_sub(1));
            let mut bar_chars = Vec::with_capacity(scrub_w);
            for i in 0..scrub_w {
                if i == head_idx {
                    bar_chars.push('●');
                } else if i < head_idx {
                    bar_chars.push('━');
                } else {
                    bar_chars.push('─');
                }
            }
            let bar_str: String = bar_chars.into_iter().collect();
            buf.set_string(cur_x, y, format!("[{bar_str}]"), Style::default().fg(Color::Cyan).bg(bg));
            cur_x += scrub_w as u16 + 2 + 1;
        }

        buf.set_string(cur_x, y, &time_str, time_style);
    }

    // Right-side controls
    if osd_rect.right() > right_total {
        let mut rx = osd_rect.right().saturating_sub(right_total);
        buf.set_string(rx, y, &vol_str, btn_style);
        rx += cell_w(&vol_str) + 1;

        buf.set_string(rx, y, &speed_str, btn_style);
        rx += cell_w(&speed_str) + 1;

        buf.set_string(rx, y, loop_str, btn_style);

        let cx = osd_rect.right().saturating_sub(cell_w(close_str));
        buf.set_string(cx, y, close_str, alert_style);
    }
}

/// Hit-tests a mouse coordinate `(col, row)` against the 1-row Video OSD Transport Bar.
pub fn hit_test_video_osd_bar(osd_rect: Rect, col: u16, row: u16) -> Option<VideoOsdHit> {
    if row != osd_rect.y || col < osd_rect.x || col >= osd_rect.right() {
        return None;
    }

    let rel_x = col.saturating_sub(osd_rect.x);

    // [▶] or [⏸] cols 0..3
    if rel_x < 3 {
        return Some(VideoOsdHit::TogglePlayPause);
    }
    // [⏮] cols 4..7
    if (4..7).contains(&rel_x) {
        return Some(VideoOsdHit::SeekBack);
    }
    // [⏭] cols 8..11
    if (8..11).contains(&rel_x) {
        return Some(VideoOsdHit::SeekForward);
    }

    // Right buttons
    let close_w = 3u16;
    if col >= osd_rect.right().saturating_sub(close_w) {
        return Some(VideoOsdHit::Close);
    }
    let loop_w = 3u16;
    if col >= osd_rect.right().saturating_sub(close_w + 1 + loop_w)
        && col < osd_rect.right().saturating_sub(close_w + 1)
    {
        return Some(VideoOsdHit::ToggleLoop);
    }
    let speed_w = 6u16;
    if col >= osd_rect.right().saturating_sub(close_w + 1 + loop_w + 1 + speed_w)
        && col < osd_rect.right().saturating_sub(close_w + 1 + loop_w + 1)
    {
        return Some(VideoOsdHit::Speed);
    }
    let vol_w = 10u16;
    if col >= osd_rect.right().saturating_sub(close_w + 1 + loop_w + 1 + speed_w + 1 + vol_w)
        && col < osd_rect.right().saturating_sub(close_w + 1 + loop_w + 1 + speed_w + 1)
    {
        return Some(VideoOsdHit::ToggleMute);
    }

    // Scrub bar click detection (middle zone)
    let scrub_start = osd_rect.x + 12;
    let scrub_end = osd_rect.right().saturating_sub(close_w + 1 + loop_w + 1 + speed_w + 1 + vol_w + 16);
    if col >= scrub_start && col <= scrub_end && scrub_end > scrub_start {
        let pct = ((col - scrub_start) as u32 * 100 / (scrub_end - scrub_start) as u32).min(100) as u8;
        return Some(VideoOsdHit::ScrubBar { pct });
    }

    None
}

/// Renders an RGBA frame into `area` using Unicode upper-half blocks (`▀`): the top pixel of each
/// cell becomes the foreground and the bottom pixel the background (nearest-neighbour sampling).
pub fn render_half_block_frame(buf: &mut Buffer, area: Rect, frame: &MediaRgbaFrame) {
    let (fw, fh) = (frame.width_px as usize, frame.height_px as usize);
    if area.width == 0 || area.height == 0 || fw == 0 || fh == 0 || frame.rgba.len() < fw * fh * 4 {
        return;
    }
    let area = area.intersection(buf.area);
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (cols, rows) = (area.width as usize, area.height as usize * 2);
    let px = |cx: usize, cy: usize| -> Color {
        let x = (cx * fw / cols).min(fw - 1);
        let y = (cy * fh / rows).min(fh - 1);
        let i = (y * fw + x) * 4;
        Color::Rgb(frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2])
    };
    for row in 0..area.height as usize {
        for col in 0..area.width as usize {
            buf[(area.x + col as u16, area.y + row as u16)]
                .set_char('▀')
                .set_fg(px(col, row * 2))
                .set_bg(px(col, row * 2 + 1));
        }
    }
}
