//! Floating `DAEMON SUPERVISOR` manager window renderer (TUI; the GUI reuses the same cells).
//!
//! Draws exactly at the positions of `DaemonWindowLayout` (shared with hit-testing).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::WindowManager;
use crate::wm::daemon_window::DaemonWindowLayout;

/// Pads or truncates `s` to exact column width `w` (ellipsis when truncated).
fn fit_pad(s: &str, w: usize) -> String {
    let total: usize = s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum();
    let mut out = String::new();
    let mut used = 0usize;
    let limit = if total > w { w.saturating_sub(1) } else { w };
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cw == 0 || used + cw > limit {
            break;
        }
        used += cw;
        out.push(ch);
    }
    if total > w && w > 0 {
        out.push('…');
        used += 1;
    }
    out.extend(std::iter::repeat_n(' ', w.saturating_sub(used)));
    out
}

/// Writes `text` at `(x, y)` clipped to `clip`.
fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 || x < clip.x || x.saturating_add(w) > end || y < clip.y || y >= clip.y.saturating_add(clip.height) {
            if x >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        x += w;
    }
}

fn fill(buf: &mut Buffer, target: Rect, style: Style) {
    let r = buf.area.intersection(target);
    for y in r.y..r.y.saturating_add(r.height) {
        for x in r.x..r.x.saturating_add(r.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the `DAEMON SUPERVISOR` floating window when active.
pub fn render_daemon_window(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(outer) = wm.daemon_window_rect(workspace_area) else {
        return;
    };
    if outer.width < 12 || outer.height < 6 || outer.intersection(buf.area) != outer {
        return;
    }
    let state = wm.daemon_window();
    let lay = DaemonWindowLayout::new(outer);

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, outer, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, outer, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, outer);

    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    fill(buf, outer, bg);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, outer, palette.window_frame_bg, palette);
    }

    render_borland_frame_styled(
        outer,
        " DAEMON SUPERVISOR ",
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    let badge = Style::default()
        .fg(palette.controls_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    put(buf, outer, lay.close_x, outer.y, "[X]", badge);

    let inner_w = outer.width.saturating_sub(2) as usize;
    let ix = outer.x + 1;
    let name_w = 14usize;
    let line = |name: &str, status: &str, pid: &str, sp: &str, sock: &str| -> String {
        let head = format!("{} {} {} {} ", fit_pad(name, name_w), fit_pad(status, 10), fit_pad(pid, 7), fit_pad(sp, 9));
        let used = head.chars().count();
        format!("{head}{}", fit_pad(sock, inner_w.saturating_sub(used)))
    };

    put(
        buf,
        outer,
        ix,
        lay.header_y,
        &fit_pad(&format!(" {}", line("NAME", "STATUS", "PID", "SESS/PANES", "SOCKET")), inner_w),
        bg.fg(tokens.text_muted).add_modifier(Modifier::BOLD),
    );

    if state.entries.is_empty() {
        put(buf, outer, ix + 1, lay.rows.y, "No daemons found", bg.fg(tokens.text_muted));
    }
    let visible = lay.rows.height as usize;
    for (i, e) in state.entries.iter().enumerate().skip(state.scroll).take(visible) {
        let y = lay.rows.y + (i - state.scroll) as u16;
        let selected = i == state.selected;
        let name = if e.current { format!("*{}", e.name) } else { format!(" {}", e.name) };
        let (badge_txt, badge_fg) = if e.running {
            ("● RUNNING", tokens.success)
        } else if e.is_crashed() {
            ("✖ CRASHED", tokens.danger)
        } else {
            ("○ STOPPED", tokens.text_muted)
        };
        let pid = e.pid.map_or_else(|| "-".to_string(), |p| p.to_string());
        let sp = format!("{}/{}", e.sessions, e.panes);
        let text = format!(" {}", line(&name, badge_txt, &pid, &sp, &e.socket_path));
        let row_style = if selected {
            Style::default().fg(palette.window_frame_bg).bg(tokens.accent).add_modifier(Modifier::BOLD)
        } else {
            bg
        };
        put(buf, outer, ix, y, &fit_pad(&text, inner_w), row_style);
        if !selected {
            // Status badge sits after " " + name column + " ".
            let bx = ix + 1 + name_w as u16 + 1;
            put(buf, outer, bx, y, &fit_pad(badge_txt, 10), bg.fg(badge_fg).add_modifier(Modifier::BOLD));
        }
    }

    put(
        buf,
        outer,
        ix,
        lay.status_y,
        &fit_pad(&format!(" {}", state.status_line), inner_w),
        bg.fg(tokens.warning),
    );

    for (b, x, _w) in lay.button_spans() {
        put(buf, outer, x, lay.footer_y, b.label(), bg.fg(tokens.accent).add_modifier(Modifier::BOLD));
    }

    if cfg.liquid_glass {
        polish_glass_panel_rect(buf, outer, palette.window_frame_bg, palette, state.captures_input());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wm::daemon_window::DaemonMenuEntry;

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
    }

    fn render(cols: u16, rows: u16, entries: Vec<DaemonMenuEntry>) -> (Buffer, Rect) {
        let mut wm = WindowManager::new_headless(cols, rows);
        {
            let d = wm.daemon_window_mut();
            d.active = true;
            d.focused = true;
            d.entries = entries;
            d.status_line = "ready".into();
        }
        let ws = Rect::new(0, 0, cols, rows);
        let mut buf = Buffer::empty(ws);
        render_daemon_window(&wm, ws, &BiltermConfig::default(), &ThemePalette::default(), &GlyphSet::default(), &mut buf);
        let r = wm.daemon_window_rect(ws).unwrap();
        (buf, r)
    }

    fn two() -> Vec<DaemonMenuEntry> {
        vec![
            DaemonMenuEntry { name: "main".into(), running: true, pid: Some(42), sessions: 2, panes: 5, socket_path: "/run/x/main.sock".into(), current: true },
            DaemonMenuEntry { name: "dev".into(), running: false, socket_path: "/ü".repeat(80), ..Default::default() },
        ]
    }

    #[test]
    fn renders_title_rows_buttons_close() {
        let (buf, r) = render(100, 30, two());
        let lay = DaemonWindowLayout::new(r);
        assert!(row_text(&buf, r.y).contains("DAEMON SUPERVISOR"));
        let title = row_text(&buf, r.y);
        let cx = lay.close_x as usize;
        assert_eq!(&title.chars().skip(cx).take(3).collect::<String>(), "[X]");
        let rows: String = (lay.rows.y..lay.rows.y + 2).map(|y| row_text(&buf, y)).collect();
        assert!(rows.contains("● RUNNING"));
        assert!(rows.contains("○ STOPPED"));
        assert!(rows.contains("*main"));
        let foot = row_text(&buf, lay.footer_y);
        for (b, x, _) in lay.button_spans() {
            let got: String = foot.chars().skip(x as usize).take(b.label().chars().count()).collect();
            assert_eq!(got, b.label());
        }
        assert!(row_text(&buf, lay.status_y).contains("ready"));
    }

    #[test]
    fn empty_list_and_tiny_ws_do_not_panic() {
        let (buf, r) = render(100, 30, vec![]);
        let lay = DaemonWindowLayout::new(r);
        assert!(row_text(&buf, lay.rows.y).contains("No daemons found"));
        let _ = render(40, 10, two());
    }
}
