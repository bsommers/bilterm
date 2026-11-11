//! 1-row Browser Navigation Bar renderer and hit-testing (`WEB-01`).
//!
//! Rendering and hit-testing share one layout ([`nav_layout`]) measured in display columns, so a
//! click always lands on the button that is drawn there.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::{BrowserCanvasState, EngineHealth};

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

pub const BACK_LABEL: &str = "[◄]";
pub const FORWARD_LABEL: &str = "[►]";
pub const RELOAD_LABEL: &str = "[↻]";
pub const STOP_LABEL: &str = "[■]";
pub const CLOSE_LABEL: &str = "[✕]";
pub const FOCUS_WEB_LABEL: &str = "[FOCUS:WEB]";
pub const FOCUS_PROMPT_LABEL: &str = "[FOCUS:PROMPT]";

/// A horizontal zone on the nav bar: start column and width, in display columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zone {
    pub x: u16,
    pub width: u16,
}

impl Zone {
    fn contains(&self, col: u16) -> bool {
        col >= self.x && col < self.x.saturating_add(self.width)
    }
}

/// Column layout of the nav bar. The focus zone has one fixed width for both labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavLayout {
    pub back: Zone,
    pub forward: Zone,
    pub reload: Zone,
    pub url: Zone,
    pub focus: Option<Zone>,
    pub close: Option<Zone>,
}

fn cols(s: &str) -> u16 {
    UnicodeWidthStr::width(s) as u16
}

/// Computes the nav bar zones for `nav_rect` from the display width of each label.
pub fn nav_layout(nav_rect: Rect) -> NavLayout {
    let mut x = nav_rect.x;
    let mut button = |label: &str| {
        let zone = Zone { x, width: cols(label) };
        x = x.saturating_add(zone.width + 1);
        zone
    };
    let back = button(BACK_LABEL);
    let forward = button(FORWARD_LABEL);
    let reload = button(RELOAD_LABEL);
    let url_start = x;

    let close_w = cols(CLOSE_LABEL);
    let focus_w = cols(FOCUS_WEB_LABEL).max(cols(FOCUS_PROMPT_LABEL));
    let right_reserve = close_w + 1 + focus_w + 1;
    let right = nav_rect.right();
    let has_right = right.saturating_sub(url_start) >= right_reserve;
    let (focus, close, url_end) = if has_right {
        let close = Zone { x: right - close_w, width: close_w };
        let focus = Zone { x: close.x - 1 - focus_w, width: focus_w };
        (Some(focus), Some(close), focus.x.saturating_sub(1))
    } else {
        (None, None, right)
    };
    let url = Zone { x: url_start, width: url_end.saturating_sub(url_start) };
    NavLayout { back, forward, reload, url, focus, close }
}

/// Truncates `text` to at most `max_cols` display columns.
fn fit_cols(text: &str, max_cols: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > max_cols {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

/// Text shown in the URL zone: status, lock (https only), URL and page title.
pub fn url_zone_text(state: &BrowserCanvasState) -> String {
    let url = state.current_url();
    let mut text = String::new();
    match state.health() {
        EngineHealth::Starting => text.push_str("◌ starting… "),
        EngineHealth::Crashed(_) => text.push_str("✖ crashed "),
        _ if state.is_loading() => text.push_str("◌ "),
        _ => {}
    }
    if url.starts_with("https://") {
        text.push_str("🔒 ");
    }
    text.push_str(url);
    if let Some(notice) = state.notice() {
        text.push_str("  ⚠ ");
        text.push_str(notice);
    } else if !state.page_title().is_empty() {
        text.push_str("  — ");
        text.push_str(state.page_title());
    }
    text
}

/// Renders the 1-row Browser Navigation Bar into `buf` across `nav_rect`.
pub fn render_browser_nav_bar(buf: &mut Buffer, nav_rect: Rect, state: &BrowserCanvasState, active: bool) {
    if nav_rect.height == 0 || nav_rect.width == 0 {
        return;
    }
    let y = nav_rect.y;
    let bg = if active { Color::Rgb(24, 38, 64) } else { Color::Rgb(18, 24, 36) };
    let btn_style = Style::default().fg(Color::Cyan).bg(bg).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray).bg(bg);
    let text_style = Style::default().fg(Color::White).bg(bg);
    let alert_style = Style::default().fg(Color::LightRed).bg(bg).add_modifier(Modifier::BOLD);

    for col in nav_rect.x..nav_rect.right() {
        buf[(col, y)].set_char(' ').set_style(Style::default().bg(bg));
    }
    let layout = nav_layout(nav_rect);
    let right = nav_rect.right();
    let mut put = |zone: Zone, label: &str, style: Style| {
        if zone.x < right {
            buf.set_stringn(zone.x, y, label, usize::from(right - zone.x), style);
        }
    };
    put(layout.back, BACK_LABEL, if state.can_go_back() { btn_style } else { dim_style });
    put(layout.forward, FORWARD_LABEL, if state.can_go_forward() { btn_style } else { dim_style });
    put(layout.reload, if state.is_loading() { STOP_LABEL } else { RELOAD_LABEL }, btn_style);
    if let Some(focus) = layout.focus {
        let (label, style) = if state.viewport_focused {
            (FOCUS_WEB_LABEL, Style::default().fg(Color::Green).bg(bg).add_modifier(Modifier::BOLD))
        } else {
            (FOCUS_PROMPT_LABEL, Style::default().fg(Color::Yellow).bg(bg))
        };
        put(focus, label, style);
    }
    if let Some(close) = layout.close {
        put(close, CLOSE_LABEL, alert_style);
    }

    let url = layout.url;
    let max_url_w = usize::from(url.width);
    if max_url_w <= 6 {
        return;
    }
    if let Some(edit) = state.url_edit() {
        let edit_style = Style::default().fg(Color::Black).bg(Color::Rgb(250, 250, 250));
        for col in url.x..url.x + url.width {
            buf[(col, y)].set_char(' ').set_style(edit_style);
        }
        // Scroll so the cursor stays visible.
        let start = (edit.cursor + 1).saturating_sub(max_url_w);
        let visible: String = edit.text.chars().skip(start).collect();
        let visible = fit_cols(&visible, max_url_w);
        buf.set_string(url.x, y, &visible, edit_style);
        let before: String = edit.text.chars().skip(start).take(edit.cursor - start).collect();
        let cx = url.x + cols(&before);
        if cx < url.x + url.width {
            buf[(cx, y)].set_style(Style::default().fg(Color::White).bg(Color::Black).add_modifier(Modifier::REVERSED));
        }
    } else {
        let style = if state.notice().is_some() || state.is_crashed() { alert_style } else { text_style };
        buf.set_string(url.x, y, fit_cols(&url_zone_text(state), max_url_w), style);
    }
}

/// Draws the "starting" / "crashed" message over the viewport when there is no frame to show.
pub fn render_browser_status_overlay(buf: &mut Buffer, body: Rect, state: &BrowserCanvasState) {
    let Some(message) = state.overlay_message() else { return };
    if body.width == 0 || body.height == 0 {
        return;
    }
    let fill = Style::default().fg(Color::Gray).bg(Color::Rgb(18, 24, 36));
    for row in body.y..body.bottom() {
        for col in body.x..body.right() {
            buf[(col, row)].set_char(' ').set_style(fill);
        }
    }
    let text = fit_cols(&message, usize::from(body.width));
    let x = body.x + (body.width - cols(&text)) / 2;
    let style = if state.is_crashed() { fill.fg(Color::LightRed).add_modifier(Modifier::BOLD) } else { fill };
    buf.set_string(x, body.y + body.height / 2, text, style);
}

/// Hit-tests a mouse coordinate `(col, row)` against the 1-row Browser Navigation Bar.
pub fn hit_test_browser_nav_bar(nav_rect: Rect, col: u16, row: u16) -> Option<BrowserNavHit> {
    if row != nav_rect.y || col < nav_rect.x || col >= nav_rect.right() {
        return None;
    }
    let layout = nav_layout(nav_rect);
    if layout.close.is_some_and(|z| z.contains(col)) {
        return Some(BrowserNavHit::Close);
    }
    if layout.focus.is_some_and(|z| z.contains(col)) {
        return Some(BrowserNavHit::ToggleFocus);
    }
    if layout.back.contains(col) {
        return Some(BrowserNavHit::Back);
    }
    if layout.forward.contains(col) {
        return Some(BrowserNavHit::Forward);
    }
    if layout.reload.contains(col) {
        return Some(BrowserNavHit::Reload);
    }
    Some(BrowserNavHit::UrlBar)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(buf: &Buffer, rect: Rect) -> Vec<String> {
        (rect.x..rect.right()).map(|c| buf[(c, rect.y)].symbol().to_string()).collect()
    }

    /// Column where `label` is drawn, found by matching its first cell.
    fn drawn_at(cells: &[String], label: &str) -> usize {
        let chars: Vec<String> = label.chars().map(|c| c.to_string()).collect();
        (0..cells.len())
            .find(|&i| chars.iter().enumerate().all(|(k, ch)| cells.get(i + k) == Some(ch)))
            .unwrap_or_else(|| panic!("{label} not drawn in {cells:?}"))
    }

    #[test]
    fn every_drawn_button_is_hit_where_it_is_drawn() {
        for focused in [false, true] {
            let mut state = BrowserCanvasState::new_synthetic("test://page", 320, 200);
            state.set_focus(focused);
            let rect = Rect::new(3, 2, 70, 1);
            let mut buf = Buffer::empty(Rect::new(0, 0, 80, 4));
            render_browser_nav_bar(&mut buf, rect, &state, true);
            let cells = row(&buf, rect);
            let focus_label = if focused { FOCUS_WEB_LABEL } else { FOCUS_PROMPT_LABEL };
            for (label, hit) in [
                (BACK_LABEL, BrowserNavHit::Back),
                (FORWARD_LABEL, BrowserNavHit::Forward),
                (RELOAD_LABEL, BrowserNavHit::Reload),
                (focus_label, BrowserNavHit::ToggleFocus),
                (CLOSE_LABEL, BrowserNavHit::Close),
            ] {
                let start = drawn_at(&cells, label);
                for k in 0..label.chars().count() {
                    let col = rect.x + (start + k) as u16;
                    assert_eq!(hit_test_browser_nav_bar(rect, col, rect.y), Some(hit), "{label} col {k} focused={focused}");
                }
            }
        }
    }

    #[test]
    fn gaps_and_url_zone_hit_the_url_bar_and_zones_use_display_columns() {
        let rect = Rect::new(0, 0, 60, 1);
        let l = nav_layout(rect);
        // "[◄]" is 3 columns wide although it is 5 bytes long.
        assert_eq!(l.back, Zone { x: 0, width: 3 });
        assert_eq!(l.forward, Zone { x: 4, width: 3 });
        assert_eq!(l.reload, Zone { x: 8, width: 3 });
        assert_eq!(l.url.x, 12);
        assert_eq!(l.close, Some(Zone { x: 57, width: 3 }));
        assert_eq!(l.focus, Some(Zone { x: 42, width: 14 }));
        assert_eq!(hit_test_browser_nav_bar(rect, 11, 0), Some(BrowserNavHit::UrlBar), "column after reload is not reload");
        assert_eq!(hit_test_browser_nav_bar(rect, 20, 0), Some(BrowserNavHit::UrlBar));
        assert_eq!(hit_test_browser_nav_bar(rect, 56, 0), Some(BrowserNavHit::UrlBar), "gap before close");
        assert_eq!(hit_test_browser_nav_bar(rect, 20, 1), None);
        assert_eq!(hit_test_browser_nav_bar(rect, 60, 0), None);
    }

    #[test]
    fn narrow_bars_drop_the_right_buttons_instead_of_overlapping() {
        let rect = Rect::new(0, 0, 20, 1);
        let l = nav_layout(rect);
        assert_eq!((l.focus, l.close), (None, None));
        assert_eq!(hit_test_browser_nav_bar(rect, 19, 0), Some(BrowserNavHit::UrlBar));
        let state = BrowserCanvasState::new_synthetic("test://page", 320, 200);
        let mut buf = Buffer::empty(rect);
        render_browser_nav_bar(&mut buf, rect, &state, false);
        assert!(!row(&buf, rect).concat().contains('✕'));
    }

    #[test]
    fn lock_icon_only_for_https_and_title_is_shown() {
        let https = BrowserCanvasState::new_synthetic("test://x", 320, 200);
        assert!(!url_zone_text(&https).contains('🔒'), "non-https has no lock");
        let mut s = BrowserCanvasState::new_synthetic("test://x", 320, 200);
        s.navigate("https://example.com").unwrap();
        let text = url_zone_text(&s);
        assert!(text.starts_with("🔒 https://example.com"), "{text}");
        assert!(text.contains("— Synthetic - https://example.com"), "title in the nav bar: {text}");
        s.navigate("http://example.com").unwrap();
        assert!(!url_zone_text(&s).contains('🔒'));
        assert!(s.navigate("javascript:alert(1)").is_err());
        assert!(url_zone_text(&s).contains("⚠ blocked URL"), "{}", url_zone_text(&s));
    }

    #[test]
    fn fit_cols_never_splits_a_wide_glyph() {
        assert_eq!(fit_cols("🔒 ab", 2), "🔒");
        assert_eq!(fit_cols("🔒 ab", 1), "");
        assert_eq!(fit_cols("abc", 10), "abc");
    }
}
