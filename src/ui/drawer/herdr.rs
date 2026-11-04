//! `herdr`-style drawer renderer: one buffer line per workspace > session > pane row (`FLT-01`).
//!
//! Called from [`super::render_workspace_drawer`] via a single early-return branch, so the classic
//! renderer is never touched when `drawer.style = classic`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::{fill_rect, write_clipped_line};
use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{DrawerLayoutMode, DrawerRow, DrawerRowKind, WindowManager, inset_window_frame};

/// Computes `(header_rows, footer_rows, rows_per_item)` for the herdr layout: no header, an optional
/// one-line footer and exactly one line per item.
pub fn herdr_layout_metrics(inner_height: u16) -> (u16, u16, usize) {
    (0, u16::from(inner_height >= 4), 1)
}

/// Text of one herdr row after the current-pane marker column.
fn row_label(row: &DrawerRow) -> String {
    let depth = row.herdr.as_ref().map_or(0, |m| usize::from(m.depth));
    let label = match &row.kind {
        DrawerRowKind::Pane(id) => format!("#{id} {}", row.title),
        _ => row.title.clone(),
    };
    format!("{}{label}", "  ".repeat(depth))
}

/// Renders the herdr-style drawer into `drawer_rect` (`FLT-01`).
pub fn render_herdr_drawer(
    wm: &WindowManager,
    workspace_area: Rect,
    drawer_rect: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let dcfg = wm.drawer_config();
    let drawer = wm.drawer();
    let is_modern = cfg.is_modern();

    if dcfg.layout == DrawerLayoutMode::Overlay {
        if cfg.liquid_glass {
            render_liquid_ambient_halo(buf, drawer_rect, workspace_area, palette);
        } else if cfg.drop_shadows {
            render_drop_shadow_styled(buf, drawer_rect, workspace_area, cfg.shadow_style, palette);
        }
    }
    occlude_rect(buf, drawer_rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, drawer_rect, palette.window_frame_bg, palette);
    }

    let title = if is_modern { "◢ WORKSPACES" } else { "WORKSPACES" };
    render_borland_frame_styled(
        drawer_rect,
        title,
        drawer.focused,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    let inner = inset_window_frame(drawer_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let base_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.inactive_border_fg))
        .bg(palette.window_frame_bg);
    if !cfg.is_glass() {
        fill_rect(buf, inner, base_style);
    }

    let rows = wm.drawer_rows();
    let selected = drawer.selected_index.min(rows.len().saturating_sub(1));
    let (header_h, footer_h, row_step) = herdr_layout_metrics(inner.height);
    let list_y = inner.y + header_h;
    let list_h = usize::from(inner.height.saturating_sub(header_h + footer_h)).max(1);
    let cap = (list_h / row_step).max(1);
    let scroll = drawer.effective_scroll(selected, cap, rows.len());
    let list_bottom = inner.y + inner.height.saturating_sub(footer_h);

    for (vis_idx, (row_idx, row)) in rows.iter().enumerate().skip(scroll).take(cap).enumerate() {
        let y = list_y + (vis_idx * row_step) as u16;
        if y >= list_bottom {
            break;
        }
        let mut style = if row.is_focused_pane {
            base_style.fg(palette.title_fg).add_modifier(Modifier::BOLD)
        } else {
            base_style
        };
        if row_idx == selected {
            style = if is_modern {
                Style::default()
                    .fg(palette.title_fg)
                    .bg(Color::Rgb(30, 41, 59))
                    .add_modifier(Modifier::BOLD)
            } else {
                style.add_modifier(Modifier::REVERSED)
            };
            fill_rect(buf, Rect::new(inner.x, y, inner.width, 1), style);
        }
        let marker = if row.is_focused_pane { "›" } else { " " };
        let line = format!("{marker}{}", row_label(row));
        write_clipped_line(buf, inner.x, y, inner.width, &line, style);
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(buf, drawer_rect, palette.window_frame_bg, palette, drawer.focused);
    }
}
