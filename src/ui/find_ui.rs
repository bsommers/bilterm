//! Find window renderer and geometry (Phase 50.1): frame, tabs, query line, result rows, footer.
//!
//! 50.1-01 owns the frame and the geometry (`find_layout`); 50.1-07 draws rows, tabs and chips into
//! the rects `find_layout` returns, 50.1-08 hit-tests the same rects.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::{WindowManager, inset_window_frame};

/// Window title (the frame draws it between the borders).
pub const FIND_TITLE: &str = " FIND ";

/// Minimum workspace area the modal needs to be drawn at all.
pub const FIND_MIN_AREA: (u16, u16) = (40, 8);

fn render_frame(
    rect: Rect,
    bounds: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, bounds, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, bounds, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    render_borland_frame_styled(rect, FIND_TITLE, true, cfg.window_outline, palette, glyphs, buf);
}

/// Draws the Find window when `Modal::Find` is open.
pub(super) fn render_find_modal(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some((kind, query, _selected)) = wm.find_modal_state() else {
        return;
    };
    let Some(outer) = wm.active_modal_rect(workspace_area) else {
        return;
    };
    render_frame(outer, workspace_area, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(outer);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let header = Line::from(Span::styled(
        format!("Find {} > {query}_", kind.as_str()),
        Style::default()
            .fg(palette.prompt_prefix_fg)
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    ));
    Paragraph::new(vec![header]).render(inner, buf);
}
