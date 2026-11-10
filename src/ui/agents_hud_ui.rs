//! Renderer of the Mission Control HUD and the turn replay view (Phase 70, HUD-01/02).
//!
//! Pure buffer writes from `&WindowManager`: no pane or PTY is touched, so drawing never resizes
//! or signals a child. The same cells serve the TUI and the GUI surface.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::WindowManager;
use crate::wm::agents_hud::hud_inner;
use crate::wm::turn_replay::TurnReplayLog;

fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, max: usize, style: Style) {
    if max > 0 && buf.area.contains((x, y).into()) {
        let room = usize::from(buf.area.x + buf.area.width - x);
        buf.set_stringn(x, y, text, max.min(room), style);
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

/// Renders the HUD (or, when open, the replay view) over the workspace.
pub fn render_agents_hud(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(outer) = wm.agents_hud_rect(workspace_area) else {
        return;
    };
    if outer.width < 12 || outer.height < 6 || outer.intersection(buf.area) != outer {
        return;
    }
    let st = wm.agents_hud();
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, outer, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, outer, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, outer);
    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default()
        .fg(tokens.text_primary)
        .bg(palette.window_frame_bg);
    fill(buf, outer, bg);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, outer, palette.window_frame_bg, palette);
    }
    let title = match st.replay_pane() {
        Some(p) => format!(" AGENT REPLAY #{p} (read-only) "),
        None => " AGENT MISSION CONTROL ".to_string(),
    };
    render_borland_frame_styled(
        outer,
        &title,
        st.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = hud_inner(outer);
    let muted = bg.fg(tokens.text_muted);
    let footer_y = outer.y + outer.height - 2;
    let fw = usize::from(outer.width.saturating_sub(2));
    match st.replay_pane() {
        Some(pane) => {
            render_replay(st.log(pane), inner, st.viewport(), bg, muted, buf);
            put(
                buf,
                outer.x + 1,
                footer_y,
                "Left/Right turn  Up/Down scroll  Home/End first/last  Esc exit",
                fw,
                muted,
            );
        }
        None => {
            st.hud().render(inner, buf);
            let size = st.page_size().max(1);
            let hud = st.hud();
            if hud.is_empty() {
                put(
                    buf,
                    inner.x + 1,
                    inner.y,
                    "No agent panes",
                    usize::from(inner.width),
                    muted,
                );
            }
            let hint = format!(
                "arrows move  a Approve  p Prompt  d Diff  z Zoom  r Replay  Esc close   page {}/{}  {} pane(s)",
                hud.page() + 1,
                hud.page_count(size),
                hud.len()
            );
            put(buf, outer.x + 1, footer_y, &hint, fw, muted);
        }
    }
}

fn render_replay(
    log: Option<&TurnReplayLog>,
    inner: Rect,
    viewport: usize,
    bg: Style,
    muted: Style,
    buf: &mut Buffer,
) {
    let w = usize::from(inner.width);
    let Some((log, turn)) = log.and_then(|l| l.current().map(|t| (l, t))) else {
        put(
            buf,
            inner.x + 1,
            inner.y,
            "No recorded turns for this pane yet",
            w,
            muted,
        );
        return;
    };
    let idx = log
        .turns()
        .position(|t| t.turn_id == turn.turn_id)
        .map_or(0, |i| i + 1);
    let state = if turn.end_ts.is_some() {
        "done"
    } else {
        "running"
    };
    let head = format!(
        "Turn {idx}/{} (#{}, {state})  {} line(s)  scroll {}",
        log.len(),
        turn.turn_id,
        turn.snapshot.len(),
        log.scroll()
    );
    put(
        buf,
        inner.x,
        inner.y,
        &head,
        w,
        bg.add_modifier(Modifier::BOLD),
    );
    let files = if turn.files.is_empty() {
        String::new()
    } else {
        format!("  files: {}", turn.files.join(", "))
    };
    let prompt = format!("> {}{files}", turn.prompt.lines().next().unwrap_or(""));
    put(buf, inner.x, inner.y + 1, &prompt, w, muted);
    for (i, line) in log.visible_lines(viewport).iter().enumerate() {
        let y = inner.y + 2 + u16::try_from(i).unwrap_or(u16::MAX);
        if y >= inner.y + inner.height {
            break;
        }
        put(buf, inner.x, y, line, w, bg);
    }
}
