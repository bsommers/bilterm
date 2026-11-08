//! Floating Drawer Window Peek & Live Preview Pane renderer (`DRW-PEEK-01..03`, Phase 56).
//!
//! Renders a non-destructive live preview overlay adjacent to the workspace drawer so users can
//! inspect occluded stacked windows, minimized windows, live background-session windows, or saved
//! session snapshots before committing focus.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{
    DrawerPreviewTarget, SavedWorkspaceSession, WindowManager, WorkspaceLayoutMode,
    inset_window_frame,
};

/// Renders the floating Drawer Peek Preview Pane into `buf` when `wm.drawer().preview.active` is true.
pub fn render_drawer_preview_overlay(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(pr) = wm.drawer_preview_rect(workspace_area) else {
        return;
    };
    let Some(target) = wm.resolve_drawer_preview_target() else {
        return;
    };

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, pr, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, pr, workspace_area, cfg.shadow_style, palette);
    }

    occlude_rect(buf, pr);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, pr, palette.window_frame_bg, palette);
    }

    let frame_title = match &target {
        DrawerPreviewTarget::ActivePane(id) => {
            let title = wm
                .panes()
                .get(id)
                .map(|p| p.title())
                .unwrap_or("WINDOW");
            format!("PEEK PREVIEW // #{id} {title}")
        }
        DrawerPreviewTarget::QuickTerminal => "PEEK PREVIEW // QUICK TERMINAL".to_string(),
        DrawerPreviewTarget::BackgroundPane { session, pane_id } => {
            format!("PEEK PREVIEW // [{session}] #{pane_id}")
        }
        DrawerPreviewTarget::SavedSession { name } => {
            format!("PEEK PREVIEW // SESSION [{name}]")
        }
    };

    render_borland_frame_styled(
        pr,
        &frame_title,
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    if cfg.liquid_glass {
        polish_glass_panel_rect(buf, pr, palette.window_frame_bg, palette, true);
    }

    // Stamp the interactive header controls (`[⏎/Click: Focus] [×]`) on the top border row
    let badge = "[⏎/Click: Focus] [×]";
    let badge_w = badge.chars().count() as u16;
    if pr.width > badge_w + 4 {
        let bx = pr.x + pr.width.saturating_sub(badge_w + 2);
        let badge_style = Style::default()
            .fg(palette.controls_fg.unwrap_or(palette.title_fg))
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD);
        buf.set_stringn(bx, pr.y, badge, usize::from(badge_w), badge_style);
    }

    let inner = inset_window_frame(pr);
    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let header_style = Style::default()
        .fg(palette.title_fg)
        .bg(palette.oia_bg)
        .add_modifier(Modifier::BOLD);
    for x in inner.x..inner.x + inner.width {
        if let Some(cell) = buf.cell_mut((x, inner.y)) {
            cell.set_symbol(" ");
            cell.set_style(header_style);
        }
    }

    let content_rect = Rect::new(
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        inner.height.saturating_sub(1),
    );

    match target {
        DrawerPreviewTarget::ActivePane(id) => {
            if let Some(pane) = wm.panes().get(&id) {
                let state_tag = if wm.z_stack().is_minimized(id) {
                    "[MINIMIZED]"
                } else if wm.active_pane_id() == id {
                    "[ACTIVE]"
                } else if wm.layout_mode() == WorkspaceLayoutMode::Stacked {
                    "[OCCLUDED]"
                } else {
                    "[TILED]"
                };
                let banner = format!(
                    " PEEK #{id} {} {state_tag} — Enter/Click to Focus  [×]",
                    pane.title()
                );
                buf.set_stringn(
                    inner.x,
                    inner.y,
                    &banner,
                    usize::from(inner.width),
                    header_style,
                );
                render_live_pane_preview(wm, id, pane, content_rect, cfg, palette, buf);
            }
        }
        DrawerPreviewTarget::QuickTerminal => {
            let banner = " PEEK [~] QUAKE QUICK TERM — Enter/Click to Open  [×]";
            buf.set_stringn(
                inner.x,
                inner.y,
                banner,
                usize::from(inner.width),
                header_style,
            );
            render_pane_canvas_and_oia(&wm.quick_terminal().pane, content_rect, cfg, palette, buf);
        }
        DrawerPreviewTarget::BackgroundPane { session, pane_id } => {
            if let Some(pane) = wm
                .window_manage()
                .background_sessions
                .get(&session)
                .and_then(|bg| bg.panes.get(&pane_id))
            {
                let banner = format!(
                    " PEEK [{session}] #{pane_id} {} [LIVE BG] — Enter/Click to Switch  [×]",
                    pane.title()
                );
                buf.set_stringn(
                    inner.x,
                    inner.y,
                    &banner,
                    usize::from(inner.width),
                    header_style,
                );
                render_pane_canvas_and_oia(pane, content_rect, cfg, palette, buf);
            }
        }
        DrawerPreviewTarget::SavedSession { name } => {
            let banner = format!(
                " PEEK SESSION [{name}] [SAVED SNAPSHOT] — Enter/Click to Restore  [×]"
            );
            buf.set_stringn(
                inner.x,
                inner.y,
                &banner,
                usize::from(inner.width),
                header_style,
            );
            let cached = wm
                .drawer()
                .preview
                .cached_saved_session
                .as_ref()
                .and_then(|(n, snap)| (n == &name).then_some(snap.as_ref()))
                .flatten();
            render_saved_session_preview(wm, &name, cached, content_rect, palette, buf);
        }
    }
}

fn render_live_pane_preview(
    wm: &WindowManager,
    id: usize,
    pane: &CompositePane,
    area: Rect,
    cfg: &BiltermConfig,
    _fallback_palette: &ThemePalette,
    buf: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let chrome_style = wm.pane_chrome(id);
    let (mut pane_cfg, pane_palette) = chrome_style.resolve_config_and_palette(cfg);
    pane_cfg.crosshair_fullscreen = false;
    pane_cfg.spotlight_fullscreen = false;
    pane_cfg.focal_zoom_fullscreen = false;
    render_pane_canvas_and_oia(pane, area, &pane_cfg, &pane_palette, buf);
}

fn render_pane_canvas_and_oia(
    pane: &CompositePane,
    area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let canvas_h = area.height.saturating_sub(1).max(1);
    let canvas = Rect::new(area.x, area.y, area.width, canvas_h);
    super::render_output_canvas(pane, canvas, cfg, palette, false, buf);
    if area.height >= 2 {
        let status_rect = Rect::new(area.x, area.y + canvas_h, area.width, 1);
        super::render_oia_status_bar_themed(pane, status_rect, palette, &cfg.glyph_set(), buf);
    }
}

fn render_saved_session_preview(
    wm: &WindowManager,
    name: &str,
    snap: Option<&SavedWorkspaceSession>,
    area: Rect,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    let bg_style = Style::default()
        .fg(palette.text_fg.unwrap_or(Color::White))
        .bg(palette.window_frame_bg);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(bg_style);
            }
        }
    }

    let mut lines: Vec<(String, Style)> = Vec::new();
    let title_style = Style::default()
        .fg(palette.title_fg)
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default()
        .fg(palette.inactive_border_fg)
        .bg(palette.window_frame_bg);

    lines.push((" SAVED SESSION SNAPSHOT".to_string(), title_style));
    if let Some(s) = snap {
        let mode_lbl = match s.layout_mode {
            WorkspaceLayoutMode::Tiled => "Tiled Splits",
            WorkspaceLayoutMode::Stacked => "Z-Order Stacked Deck",
        };
        lines.push((
            format!(
                " Session '{}' — {} ({} window(s))",
                s.name,
                mode_lbl,
                s.panes.len()
            ),
            title_style,
        ));
        lines.push((" ─".repeat(usize::from(area.width / 2)), dim_style));
        for p in &s.panes {
            let cmd = p
                .running_command
                .as_deref()
                .unwrap_or("(idle shell)");
            lines.push((
                format!(" • Window #{} [{}] — cmd: {cmd}", p.id, p.title),
                bg_style.add_modifier(Modifier::BOLD),
            ));
            if !p.cwd.is_empty() {
                lines.push((format!("   cwd: {}", p.cwd), dim_style));
            }
            for sb in p.scrollback_lines.iter().rev().take(2).rev() {
                if !sb.trim().is_empty() {
                    lines.push((format!("   │ {sb}"), bg_style));
                }
            }
        }
    } else {
        let tree_node = wm.daemon_state().session_tree.node(name);
        let pane_count = tree_node.map_or(0, |n| n.pane_count);
        lines.push((
            format!(" Session '{name}' ({pane_count} window(s))"),
            title_style,
        ));
        lines.push((
            " Press Enter or Click inside this preview to switch/restore session.".to_string(),
            bg_style,
        ));
    }

    for (idx, (text, style)) in lines.iter().enumerate() {
        let y = area.y.saturating_add(idx as u16);
        if y >= area.y + area.height {
            break;
        }
        buf.set_stringn(area.x, y, text, usize::from(area.width), *style);
    }
}
