//! AGENTS list floating window renderer (Phase 51-01 tracer): the frame plus one line per row.
//! Plan 51-06 owns the full chrome (header, badges, chips, selection, scroll).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::render_borland_frame_styled;
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::wm::WindowManager;

/// Window title.
pub const AGENTS_LIST_TITLE: &str = "AGENTS";

/// Renders the AGENTS list window when open (next to the AGENT TRAFFIC window, below modals).
pub fn render_agents_list(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(rect) = wm.agents_list_rect(workspace_area) else {
        return;
    };
    let rect = rect.intersection(buf.area);
    if rect.width < 8 || rect.height < 4 {
        return;
    }
    occlude_rect(buf, rect);
    let bg = Style::default()
        .fg(DesignTokens::from_palette(palette).text_primary)
        .bg(palette.window_frame_bg);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(bg);
            }
        }
    }
    render_borland_frame_styled(
        rect,
        AGENTS_LIST_TITLE,
        wm.agents_list().captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2);
    for (i, row) in wm.agents_list_rows().iter().take(usize::from(inner.height)).enumerate() {
        let line = format!("{:<8} {} #{}", row.state.as_str(), row.name, row.pane_id);
        put_line(buf, inner, i as u16, &line, bg);
    }
}

/// Writes `text` on line `dy` of `area`, clipped to its width by display width.
fn put_line(buf: &mut Buffer, area: Rect, dy: u16, text: &str, style: Style) {
    let mut x = area.x;
    let end = area.x + area.width;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0) as u16;
        if w == 0 || x + w > end {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, area.y + dy)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        x += w;
    }
}
