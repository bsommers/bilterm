//! 1-row Browser Navigation Bar renderer and hit-testing (`WEB-01`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::BrowserCanvasState;

/// Hit-test target on the 1-row Browser Navigation Bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserNavHit {
    Back,
    Forward,
    Reload,
    UrlBar,
    ToggleFocus,
    Close,
}

/// Renders the 1-row Browser Navigation Bar into `buf` across `nav_rect`.
pub fn render_browser_nav_bar(
    buf: &mut Buffer,
    nav_rect: Rect,
    state: &BrowserCanvasState,
    active: bool,
) {
    if nav_rect.height == 0 || nav_rect.width == 0 {
        return;
    }

    let y = nav_rect.y;
    let bg = if active {
        Color::Rgb(24, 38, 64)
    } else {
        Color::Rgb(18, 24, 36)
    };
    let btn_style = Style::default().fg(Color::Cyan).bg(bg).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White).bg(bg);
    let alert_style = Style::default().fg(Color::LightRed).bg(bg).add_modifier(Modifier::BOLD);

    // Fill background
    for col in nav_rect.x..nav_rect.right() {
        buf[(col, y)].set_char(' ').set_style(Style::default().bg(bg));
    }

    let mut cur_x = nav_rect.x;

    // [◄] Back
    let back_str = "[◄]";
    buf.set_string(cur_x, y, back_str, btn_style);
    cur_x += back_str.len() as u16 + 1;

    // [►] Forward
    let fwd_str = "[►]";
    buf.set_string(cur_x, y, fwd_str, btn_style);
    cur_x += fwd_str.len() as u16 + 1;

    // [↻] Reload
    let reload_str = "[↻]";
    buf.set_string(cur_x, y, reload_str, btn_style);
    cur_x += reload_str.len() as u16 + 1;

    // Right-aligned buttons: [✕] Close, [FOCUS/PROMPT]
    let close_str = "[✕]";
    let focus_str = if state.viewport_focused {
        "[FOCUS:WEB]"
    } else {
        "[FOCUS:PROMPT]"
    };
    let right_reserve = close_str.len() as u16 + 1 + focus_str.len() as u16 + 1;

    // Center/URL Bar
    let max_url_w = nav_rect.right().saturating_sub(cur_x).saturating_sub(right_reserve) as usize;
    if max_url_w > 6 {
        if let Some(edit) = state.url_edit() {
            let edit_bg = Color::Rgb(250, 250, 250);
            let edit_style = Style::default().fg(Color::Black).bg(edit_bg);
            for col in cur_x..cur_x + max_url_w as u16 {
                buf[(col, y)].set_char(' ').set_style(edit_style);
            }
            // Scroll so the cursor stays visible.
            let start = (edit.cursor + 1).saturating_sub(max_url_w);
            let visible: String = edit.text.chars().skip(start).take(max_url_w).collect();
            buf.set_string(cur_x, y, &visible, edit_style);
            let cx = cur_x + (edit.cursor - start) as u16;
            if cx < cur_x + max_url_w as u16 {
                buf[(cx, y)].set_style(
                    Style::default().fg(Color::White).bg(Color::Black).add_modifier(Modifier::REVERSED),
                );
            }
        } else {
            let display_url = format!("🔒 {}", state.current_url());
            let truncated: String = display_url.chars().take(max_url_w).collect();
            buf.set_string(cur_x, y, &truncated, text_style);
        }
    }

    // Right buttons
    if nav_rect.right() > right_reserve {
        let fx = nav_rect.right().saturating_sub(right_reserve);
        let focus_style = if state.viewport_focused {
            Style::default().fg(Color::Green).bg(bg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow).bg(bg)
        };
        buf.set_string(fx, y, focus_str, focus_style);

        let cx = nav_rect.right().saturating_sub(close_str.len() as u16);
        buf.set_string(cx, y, close_str, alert_style);
    }
}

/// Hit-tests a mouse coordinate `(col, row)` against the 1-row Browser Navigation Bar.
pub fn hit_test_browser_nav_bar(nav_rect: Rect, col: u16, row: u16) -> Option<BrowserNavHit> {
    if row != nav_rect.y || col < nav_rect.x || col >= nav_rect.right() {
        return None;
    }

    let rel_x = col.saturating_sub(nav_rect.x);

    // [◄] width 3: cols 0..3
    if rel_x < 3 {
        return Some(BrowserNavHit::Back);
    }
    // space at 3, [►] width 3: cols 4..7
    if (4..7).contains(&rel_x) {
        return Some(BrowserNavHit::Forward);
    }
    // space at 7, [↻] width 3: cols 8..11 (or 3 UTF-8 / char width)
    if (8..12).contains(&rel_x) {
        return Some(BrowserNavHit::Reload);
    }

    let close_w = 3u16;
    let focus_w = 14u16;
    let total_right = close_w + 1 + focus_w;

    if col >= nav_rect.right().saturating_sub(close_w) {
        return Some(BrowserNavHit::Close);
    }
    if col >= nav_rect.right().saturating_sub(total_right)
        && col < nav_rect.right().saturating_sub(close_w + 1)
    {
        return Some(BrowserNavHit::ToggleFocus);
    }

    Some(BrowserNavHit::UrlBar)
}
