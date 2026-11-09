//! TRAFFIC MONITOR window TUI renderer (`TMON-03`, Phase 64-03). Draws only from
//! `traffic_monitor_layout`; never computes its own rect.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Sparkline, Widget};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::traffic::{GroupRow, Locality, Rates};
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::traffic_monitor_geometry::{Col, FOOTER_CHIPS, LOC_CHIPS, TABS, TrafficLayout, traffic_monitor_layout};

/// Compact byte count: `512B`, `1.5K`, `3.2M`, `1.1G`.
pub fn fmt_bytes(n: u64) -> String {
    const U: [&str; 4] = ["K", "M", "G", "T"];
    if n < 1024 {
        return format!("{n}B");
    }
    let mut v = n as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v >= 100.0 { format!("{v:.0}{}", U[i]) } else { format!("{v:.1}{}", U[i]) }
}

/// Short locality tag used in the LOC column.
pub fn loc_tag(l: Option<Locality>) -> &'static str {
    match l {
        Some(Locality::Local) => "L",
        Some(Locality::Loopback) => "LB",
        Some(Locality::Network) => "NET",
        None => "-",
    }
}

fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut cx = x;
    let end = clip.x.saturating_add(clip.width);
    if y < clip.y || y >= clip.y.saturating_add(clip.height) {
        return;
    }
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 {
            continue;
        }
        if cx < clip.x || cx.saturating_add(w) > end {
            if cx >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((cx, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        cx += w;
    }
}

fn fill(buf: &mut Buffer, clip: Rect, r: Rect, style: Style) {
    let r = r.intersection(clip);
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

fn right(s: &str, w: u16) -> String {
    let w = w as usize;
    let s: String = s.chars().take(w).collect();
    format!("{s:>w$}")
}

fn series_sum(r: &GroupRow, n: usize) -> Vec<u64> {
    let len = r.series_rx.len().min(r.series_tx.len());
    let from = len.saturating_sub(n);
    (from..len).map(|i| r.series_rx[i] + r.series_tx[i]).collect()
}

fn spark(buf: &mut Buffer, area: Rect, data: &[u64], style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let max = data.iter().copied().max().unwrap_or(0).max(1);
    Sparkline::default().data(data).max(max).style(style).render(area, buf);
}

fn cell_text(c: Col, r: &GroupRow, w: u16) -> String {
    let rate = |a: u64| right(&fmt_bytes(a), w - 1);
    match c {
        Col::Name => sanitize_display(&r.label, usize::from(w.saturating_sub(1)).max(1)),
        Col::Loc => format!(" {}", loc_tag(r.locality)),
        Col::Rx1 => format!(" {}", rate(r.rates.rx_1)),
        Col::Tx1 => format!(" {}", rate(r.rates.tx_1)),
        Col::R10 => format!(" {}", rate(r.rates.rx_10 + r.rates.tx_10)),
        Col::R60 => format!(" {}", rate(r.rates.rx_60 + r.rates.tx_60)),
        Col::TotRx => format!(" {}", rate(r.rx_total)),
        Col::TotTx => format!(" {}", rate(r.tx_total)),
        Col::Peak => format!(" {}", rate(r.peak_rx + r.peak_tx)),
        Col::Esc => format!(" {}", right(&format!("{}%", r.esc_pct), w - 1)),
        Col::Ev => format!(" {}", right(&r.events_per_s.to_string(), w - 1)),
        Col::Spark => String::new(),
    }
}

fn header_text(rows: &[GroupRow], totals: &crate::traffic::Totals) -> String {
    let mut x = Rates::default();
    for r in rows {
        x.add(&r.rates);
    }
    format!(
        "1s RX{} TX{}|10s RX{} TX{}|1m RX{} TX{}|TOT RX{} TX{}",
        fmt_bytes(x.rx_1),
        fmt_bytes(x.tx_1),
        fmt_bytes(x.rx_10),
        fmt_bytes(x.tx_10),
        fmt_bytes(x.rx_60),
        fmt_bytes(x.tx_60),
        fmt_bytes(totals.rx),
        fmt_bytes(totals.tx)
    )
}

/// Renders the TRAFFIC MONITOR window when open.
pub fn render_traffic_monitor(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(lay) = traffic_monitor_layout(wm, workspace_area) else {
        return;
    };
    let rect = lay.outer.intersection(buf.area);
    if rect != lay.outer || rect.width < 20 || rect.height < 8 {
        return;
    }
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    fill(buf, rect, rect, bg);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    let st = wm.traffic_monitor();
    let res = wm.traffic_rows();
    let title = format!(
        " TRAFFIC MONITOR{}{} ",
        if st.docked { " [docked]" } else { "" },
        if st.paused.is_some() { " PAUSED" } else { "" }
    );
    render_borland_frame_styled(rect, &title, st.captures_input(), cfg.window_outline, palette, glyphs, buf);
    draw_chrome(&lay, wm, &tokens, palette, buf);
    draw_body(&lay, wm, &res, &tokens, bg, buf);
}

fn draw_chrome(lay: &TrafficLayout, wm: &WindowManager, tokens: &DesignTokens, palette: &ThemePalette, buf: &mut Buffer) {
    let st = wm.traffic_monitor();
    let badge = Style::default()
        .fg(palette.controls_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    if lay.outer.width >= 20 {
        for (b, x) in &lay.badges {
            use crate::wm::traffic_monitor_geometry::TrafficBadge as B;
            let g = match b {
                B::Pin if st.pinned => "[•]",
                B::Pin => "[o]",
                B::Dock => "[□]",
                B::Close => "[×]",
            };
            put(buf, lay.outer, *x, lay.outer.y, g, badge);
        }
    }
    let _ = tokens;
}

fn draw_body(lay: &TrafficLayout, wm: &WindowManager, res: &crate::traffic::GroupResult, tokens: &DesignTokens, bg: Style, buf: &mut Buffer) {
    let st = wm.traffic_monitor();
    let inner = lay.inner;
    let muted = bg.fg(tokens.text_muted);
    let accent = bg.fg(tokens.accent).add_modifier(Modifier::BOLD);
    let active = Style::default().fg(tokens.accent_on).bg(tokens.accent).add_modifier(Modifier::BOLD);

    put(buf, inner, inner.x, lay.header_y, &header_text(&res.rows, &res.totals), bg);
    for (g, x, w) in &lay.tabs {
        let label = TABS.iter().find(|(k, _)| k == g).map_or("", |(_, l)| *l);
        let t = format!("[{label}]");
        debug_assert_eq!(t.len() as u16, *w);
        put(buf, inner, *x, lay.tabs_y, &t, if *g == st.group { active } else { muted });
    }
    let cursor = if st.filter_focused { "█" } else { "" };
    let ftxt = format!("/ filter: {}{cursor}", sanitize_display(&st.filter.text, crate::traffic::FILTER_CAP));
    let fstyle = if st.filter_focused { accent.bg(tokens.surface_raised) } else { bg };
    let fr = Rect::new(lay.filter.0, lay.filter_y, lay.filter.1, 1);
    fill(buf, inner, fr, fstyle);
    put(buf, fr, fr.x, fr.y, &ftxt, fstyle);
    for (l, x, _) in &lay.chips {
        let t = LOC_CHIPS.iter().find(|(k, _)| k == l).map_or("", |(_, t)| *t);
        put(buf, inner, *x, lay.filter_y, t, if *l == st.filter.locality { active } else { muted });
    }

    let mark = if st.sort.descending { "v" } else { "^" };
    let mut marked = false;
    for (c, x, w) in &lay.columns {
        let is_sort = c.sort_key() == Some(st.sort.key) && !marked;
        marked |= is_sort;
        let t = if *c == Col::Name { c.title().to_string() } else { format!(" {}", right(c.title(), w - 1)) };
        let t = if is_sort { format!("{}{mark}", t.trim_end()) } else { t };
        let t: String = if is_sort && *c != Col::Name { right(&t, *w) } else { t };
        put(buf, Rect::new(*x, lay.colhead_y, *w, 1), *x, lay.colhead_y, &t, if is_sort { accent } else { muted });
    }

    if res.rows.is_empty() {
        put(buf, inner, inner.x, lay.rows_y, " (no traffic yet - metering idle or filtered out)", muted);
    }
    for i in 0..usize::from(lay.rows_h) {
        let idx = st.scroll + i;
        let Some(r) = res.rows.get(idx) else { break };
        let y = lay.rows_y + i as u16;
        let sel = idx == st.selected;
        let rs = if sel { bg.bg(tokens.surface_raised).add_modifier(Modifier::REVERSED) } else { bg };
        fill(buf, inner, Rect::new(inner.x, y, inner.width, 1), rs);
        for (c, x, w) in &lay.columns {
            let cell = Rect::new(*x, y, *w, 1);
            if *c == Col::Spark {
                spark(buf, Rect::new(x + 1, y, w - 1, 1), &series_sum(r, usize::from(w - 1)), rs.fg(tokens.accent));
                continue;
            }
            let style = match (c, r.locality) {
                (Col::Loc, Some(Locality::Local)) => rs.fg(tokens.text_muted),
                (Col::Loc, Some(Locality::Loopback)) => rs.fg(tokens.success),
                (Col::Loc, Some(Locality::Network)) => rs.fg(tokens.warning),
                _ => rs,
            };
            put(buf, cell, *x, y, &cell_text(*c, r, *w), style);
        }
    }

    if let Some(r) = res.rows.get(st.selected) {
        let name = sanitize_display(&r.label, 24);
        let label = format!("{name} RX ");
        put(buf, inner, inner.x, lay.detail_y, &label, muted);
        let sx = inner.x + label.chars().count() as u16;
        let half = (inner.width.saturating_sub(label.chars().count() as u16 + 5)) / 2;
        let rx_area = Rect::new(sx, lay.detail_y, half, 1);
        let max = r.series_rx.iter().chain(r.series_tx.iter()).copied().max().unwrap_or(0).max(1);
        let tail = |v: &[u64], n: u16| v[v.len().saturating_sub(usize::from(n))..].to_vec();
        Sparkline::default().data(tail(&r.series_rx, half)).max(max).style(bg.fg(tokens.success)).render(rx_area, buf);
        put(buf, inner, sx + half, lay.detail_y, " TX ", muted);
        let tx_area = Rect::new(sx + half + 4, lay.detail_y, half, 1);
        Sparkline::default().data(tail(&r.series_tx, half)).max(max).style(bg.fg(tokens.warning)).render(tx_area, buf);
    }

    for (chip, x, _) in &lay.footer {
        let t = FOOTER_CHIPS.iter().find(|(k, _)| k == chip).map_or("", |(_, t)| *t);
        put(buf, inner, *x, lay.footer_y, t, accent);
    }
    let fx = lay.footer.last().map_or(inner.x, |(_, x, w)| x + w + 2);
    let hint = st.status.as_deref().unwrap_or("Tab:group s:sort /:filter l:locality P/D:pin/dock Esc:close");
    put(buf, inner, fx, lay.footer_y, &sanitize_display(hint, 80), muted);
}
