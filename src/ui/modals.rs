//! Modal overlays and Borland Turbo Vision top menubar renderers.

#[path = "help_catalog.rs"]
pub mod help_catalog;
pub use help_catalog::{HELP_LINES, PALETTE_CATALOG};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens};
use crate::ui::glyphs::GlyphSet;
use crate::ui::menu_model::MENUBAR_CATEGORIES;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{WindowManager, inset_window_frame};

fn role(cfg: &BiltermConfig, palette: &ThemePalette, role: ColorRole, legacy: Color) -> Color {
    if AppearanceProfile::resolve(cfg) == AppearanceProfile::Legacy {
        legacy
    } else {
        DesignTokens::from_palette(palette).get(role)
    }
}

/// Returns the number of header rows inside `render_palette_modal` (`2` in modern mode with `inner.height >= 6`, else `1`).
pub fn palette_header_rows(cfg: &BiltermConfig, inner_height: u16) -> u16 {
    if cfg.is_modern() && inner_height >= 6 { 2 } else { 1 }
}

fn render_modal_surface_and_frame(
    rect: Rect,
    bounds: Rect,
    title: &str,
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
    render_borland_frame_styled(rect, title, true, cfg.window_outline, palette, glyphs, buf);
}

pub(super) fn render_palette_modal(
    wm: &WindowManager,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let pop_rect = wm.active_modal_rect(canvas).unwrap_or_else(|| {
        let pop_w = canvas.width.saturating_sub(4).clamp(24, 78).min(canvas.width);
        let pop_h = canvas.height.saturating_sub(2).clamp(6, 16).min(canvas.height);
        let pop_x = canvas.x + (canvas.width.saturating_sub(pop_w)) / 2;
        let pop_y = canvas.y + (canvas.height.saturating_sub(pop_h)) / 2;
        Rect::new(pop_x, pop_y, pop_w, pop_h)
    });

    let title = if cfg.liquid_glass {
        "● ● ●  💧 Command Palette · Enter Run · ↑↓ Select · Esc Close"
    } else if cfg.is_modern() {
        "● ● ●  ◢ Command Palette · Enter Run · ↑↓ Select · Esc Close"
    } else {
        "[P] COMMAND PALETTE [Enter=Run  Up/Down=Select  Esc=Close]"
    };
    render_modal_surface_and_frame(pop_rect, canvas, title, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(pop_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let (palette_query, palette_selected) = wm.palette_state().unwrap_or(("", 0));
    let items = wm.filtered_palette_items();
    let hdr_rows = palette_header_rows(cfg, inner.height);
    let mut lines = Vec::new();
    if cfg.is_modern() {
        lines.push(Line::from(Span::styled(
            format!("  ❯  {palette_query}▍   ({} commands)", items.len()),
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(palette.oia_bg)
                .add_modifier(Modifier::BOLD),
        )));
        if hdr_rows >= 2 {
            lines.push(Line::from(Span::styled(
                "─".repeat(usize::from(inner.width)),
                Style::default()
                    .fg(palette.inactive_border_fg)
                    .bg(palette.window_frame_bg),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            format!("Search > {palette_query}_"),
            Style::default()
                .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 255, 85)))
                .bg(palette.window_frame_bg)
                .add_modifier(Modifier::BOLD),
        )));
    }

    let list_rows = usize::from(inner.height.saturating_sub(hdr_rows)).max(1);
    let sel = palette_selected.min(items.len().saturating_sub(1));
    let start = sel.saturating_sub(list_rows.saturating_sub(1));
    for (idx, (cmd, key, desc)) in items.iter().enumerate().skip(start).take(list_rows) {
        let is_sel = idx == sel;
        if cfg.is_modern() {
            let bg = if is_sel {
                if cfg.liquid_glass {
                    role(cfg, palette, ColorRole::Overlay, Color::Rgb(24, 46, 78))
                } else {
                    role(cfg, palette, ColorRole::Overlay, Color::Rgb(30, 41, 59))
                }
            } else {
                palette.window_frame_bg
            };
            let fg = if is_sel { palette.title_fg } else { palette.text_fg.unwrap_or(Color::White) };
            let bar = if is_sel { "▎" } else { " " };
            lines.push(Line::from(Span::styled(
                format!("{bar} {:<20} [{:<10}] {}", cmd, key, desc),
                Style::default().fg(fg).bg(bg).add_modifier(if is_sel {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            )));
        } else {
            let bg = if is_sel { palette.active_border_fg } else { palette.window_frame_bg };
            let fg = if is_sel { Color::Black } else { Color::White };
            lines.push(Line::from(Span::styled(
                format!(" {:<20} [{:<10}] {}", cmd, key, desc),
                Style::default().fg(fg).bg(bg),
            )));
        }
    }
    Paragraph::new(lines).render(inner, buf);
    if cfg.is_glass() {
        polish_glass_panel_rect(buf, pop_rect, palette.window_frame_bg, palette, true);
    }
}

pub(super) fn render_history_popup(
    pane: &CompositePane,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let pop_w = canvas.width.saturating_sub(6).clamp(24, 72).min(canvas.width);
    let pop_h = canvas.height.saturating_sub(2).clamp(5, 14).min(canvas.height);
    let pop_x = canvas.x + (canvas.width.saturating_sub(pop_w)) / 2;
    let pop_y = canvas.y + (canvas.height.saturating_sub(pop_h)) / 2;
    let pop_rect = Rect::new(pop_x, pop_y, pop_w, pop_h);

    let persist_tag = if pane.history_store().enabled { "Persist:On" } else { "Persist:Off" };
    let hist_title = if cfg.is_modern() {
        format!("● ● ●  Command History · {persist_tag} · Enter Select · s Save · Esc Close")
    } else {
        format!("[H] COMMAND HISTORY ({persist_tag}) [Enter=Select  s=Save  Esc=Close]")
    };
    render_modal_surface_and_frame(pop_rect, canvas, &hist_title, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(pop_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    let header = format!(
        "Session: {} | Pane #{} | Entries: {}",
        pane.history_store().session_name,
        pane.id(),
        pane.prompt().history.len()
    );
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(85, 255, 255)))
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    )));

    let list_rows = usize::from(inner.height.saturating_sub(1)).max(1);
    if pane.prompt().history.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No commands recorded in this pane yet)",
            Style::default()
                .fg(Color::Yellow)
                .bg(palette.window_frame_bg),
        )));
    } else {
        let sel = pane
            .prompt()
            .history_popup_selected
            .min(pane.prompt().history.len() - 1);
        let start = sel.saturating_sub(list_rows.saturating_sub(1));
        for (idx, cmd) in pane
            .prompt()
            .history
            .iter()
            .enumerate()
            .skip(start)
            .take(list_rows)
        {
            let is_sel = idx == sel;
            let row_bg = if is_sel {
                palette.active_border_fg
            } else {
                palette.window_frame_bg
            };
            let row_fg = if is_sel { Color::Black } else { Color::White };
            let prefix = if is_sel { "►" } else { " " };
            lines.push(Line::from(Span::styled(
                format!("{prefix} [{:03}] {}", idx + 1, cmd.replace('\n', " ; ")),
                Style::default()
                    .fg(row_fg)
                    .bg(row_bg)
                    .add_modifier(if is_sel {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )));
        }
    }

    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

pub(super) fn render_unsafe_paste_modal(
    pending_paste: &str,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    let glyphs = cfg.glyph_set();
    let w = canvas.width.saturating_sub(6).clamp(24, 68).min(canvas.width);
    let h = canvas.height.saturating_sub(2).clamp(5, 9).min(canvas.height);
    let x = canvas.x + (canvas.width.saturating_sub(w)) / 2;
    let y = canvas.y + (canvas.height.saturating_sub(h)) / 2;
    let modal = Rect::new(x, y, w, h);

    render_modal_surface_and_frame(
        modal,
        canvas,
        "[!] UNSAFE PASTE GUARD [y/Enter=Confirm  n/Esc=Cancel]",
        cfg,
        palette,
        &glyphs,
        buf,
    );
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let line_count = pending_paste.lines().count().max(1);
    let preview: String = pending_paste
        .replace(['\r', '\n'], " ↵ ")
        .chars()
        .take(usize::from(inner.width.saturating_sub(2)))
        .collect();
    let lines = vec![
        Line::from(Span::styled(
            format!(" Multi-line/raw paste ({line_count} lines) into active PTY:"),
            Style::default()
                .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 255, 85)))
                .bg(palette.window_frame_bg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {preview}"),
            Style::default()
                .fg(role(cfg, palette, ColorRole::TextPrimary, Color::Rgb(85, 255, 255)))
                .bg(palette.window_frame_bg),
        )),
        Line::from(Span::styled(
            " Press [y/Enter] to paste or [n/Esc] to abort.",
            Style::default()
                .fg(Color::White)
                .bg(palette.window_frame_bg),
        )),
    ];
    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

pub(super) fn render_help_modal(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let scroll = wm.help_scroll();
    let modal = wm.active_modal_rect(workspace_area).unwrap_or_else(|| {
        let w = workspace_area
            .width
            .saturating_sub(4)
            .clamp(20, 78)
            .min(workspace_area.width);
        let h = workspace_area
            .height
            .saturating_sub(2)
            .clamp(6, 22)
            .min(workspace_area.height);
        let x = workspace_area.x + (workspace_area.width.saturating_sub(w)) / 2;
        let y = workspace_area.y + (workspace_area.height.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    });

    let help_title = if cfg.is_modern() {
        concat!("● ● ●  Bilterm v", env!("CARGO_PKG_VERSION"), " · Help · ↑↓ Scroll · Esc Close")
    } else {
        concat!("[F1] BILTERM v", env!("CARGO_PKG_VERSION"), " — 3270 // TURBO VISION HELP (Up/Down/PgUp/PgDn, Esc/F1=Close)")
    };
    render_modal_surface_and_frame(modal, workspace_area, help_title, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let max_scroll = HELP_LINES.len().saturating_sub(usize::from(inner.height));
    let start = scroll.min(max_scroll);
    let mut rendered = Vec::with_capacity(usize::from(inner.height));
    for &(lhs, rhs) in HELP_LINES.iter().skip(start).take(usize::from(inner.height)) {
        if rhs.is_empty() {
            rendered.push(Line::from(Span::styled(
                format!(" {lhs}"),
                Style::default()
                    .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 255, 85)))
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            rendered.push(Line::from(vec![
                Span::styled(
                    format!(" {:<32}", lhs),
                    Style::default()
                        .fg(role(cfg, palette, ColorRole::TextPrimary, Color::Rgb(85, 255, 255)))
                        .bg(palette.window_frame_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    rhs,
                    Style::default()
                        .fg(Color::White)
                        .bg(palette.window_frame_bg),
                ),
            ]));
        }
    }

    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(rendered).render(inner, buf);
}

/// Outer width of a pull-down: room for the longest label plus a right-aligned accelerator.
pub const MENUBAR_DROPDOWN_WIDTH: u16 = 40;

/// Returns the menu bar category index (`0..MENUBAR_CATEGORIES.len()`) at `rel_col`
/// relative to the left edge of the top menu bar (`ECO-03`).
pub fn menubar_category_at_col(rel_col: u16) -> Option<usize> {
    let mut x = 0u16;
    for (idx, cat) in MENUBAR_CATEGORIES.iter().enumerate() {
        let w = cat.title.chars().count() as u16 + 2;
        if rel_col >= x && rel_col < x + w {
            return Some(idx);
        }
        x = x.saturating_add(w);
    }
    None
}

/// Returns the starting column offset of `category` along the top menu bar (`ECO-03`).
pub fn menubar_category_x_offset(category: usize) -> u16 {
    MENUBAR_CATEGORIES
        .iter()
        .take(category.min(MENUBAR_CATEGORIES.len()))
        .map(|cat| cat.title.chars().count() as u16 + 2)
        .sum()
}

/// Returns the number of items in `category` of the top menu bar (`ECO-03`).
/// Number of top-level menubar categories (used for keyboard wrap-around).
pub fn menubar_category_count() -> usize {
    MENUBAR_CATEGORIES.len()
}

pub fn menubar_category_item_count(category: usize) -> usize {
    let cat_idx = category.min(MENUBAR_CATEGORIES.len().saturating_sub(1));
    MENUBAR_CATEGORIES[cat_idx].items.len()
}

/// Returns the `:colon` command string for `(category, item)` in the top menu bar (`ECO-03`).
pub fn menubar_item_command(category: usize, item: usize) -> Option<&'static str> {
    MENUBAR_CATEGORIES.get(category)?.items.get(item).map(|entry| entry.command)
}

/// Computes the outer `Rect` of the open pull-down menu for `category` inside `workspace_area` (`ECO-03`).
pub fn menubar_dropdown_rect(category: usize, workspace_area: Rect) -> Rect {
    let cat_idx = category.min(MENUBAR_CATEGORIES.len().saturating_sub(1));
    let items = MENUBAR_CATEGORIES[cat_idx].items;
    let x_off = menubar_category_x_offset(cat_idx);
    let w = MENUBAR_DROPDOWN_WIDTH.min(workspace_area.width);
    let h = (items.len() as u16 + 2).min(workspace_area.height);
    let x = (workspace_area.x + x_off)
        .min(workspace_area.x + workspace_area.width.saturating_sub(w));
    let y = workspace_area.y;
    Rect::new(x, y, w, h)
}

/// Splits `text` into spans with the mnemonic letter underlined (when the text contains it).
fn mnemonic_spans(text: &str, mnemonic: char, style: Style, lead: &str, trail: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(4);
    match crate::ui::menu_model::mnemonic_index(text, mnemonic) {
        Some(i) => {
            let c_len = text[i..].chars().next().map_or(1, char::len_utf8);
            spans.push(Span::styled(format!("{lead}{}", &text[..i]), style));
            spans.push(Span::styled(text[i..i + c_len].to_string(), style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled(format!("{}{trail}", &text[i + c_len..]), style));
        }
        None => spans.push(Span::styled(format!("{lead}{text}{trail}"), style)),
    }
    spans
}

pub(super) fn render_top_menubar(
    rect: Rect,
    wm: &WindowManager,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let cfg = wm.config();
    let is_modern = cfg.is_modern();
    let bar_bg = if is_modern {
        palette.oia_bg
    } else {
        role(cfg, palette, ColorRole::Overlay, Color::Rgb(170, 170, 170))
    };
    let bar_fg = if is_modern {
        palette.title_fg
    } else {
        Color::Black
    };
    let mut spans = Vec::new();
    for (idx, cat) in MENUBAR_CATEGORIES.iter().enumerate() {
        let is_open = wm.menubar_open_category() == Some(idx);
        let (fg, bg) = if is_open {
            if is_modern {
                (
                    role(cfg, palette, ColorRole::Scrim, Color::Rgb(11, 15, 25)),
                    palette.active_border_fg,
                )
            } else {
                (
                    Color::White,
                    role(cfg, palette, ColorRole::BorderFocus, Color::Rgb(0, 128, 0)),
                )
            }
        } else {
            (bar_fg, bar_bg)
        };
        let style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);
        spans.extend(mnemonic_spans(cat.title, cat.mnemonic, style, " ", " "));
    }
    if wm.is_broadcast_input() {
        spans.push(Span::styled(
            " 📡 BCAST ",
            Style::default()
                .fg(Color::Black)
                .bg(role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 176, 0)))
                .add_modifier(Modifier::BOLD),
        ));
    } else if palette.is_tron() {
        spans.push(Span::styled(
            "  ◈ TRON ",
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
    } else if wm.config().liquid_glass {
        spans.push(Span::styled(
            "  💧 LIQUID GLASS ",
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
    } else if is_modern {
        spans.push(Span::styled(
            "  ◢ MODERN ",
            Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Block::default()
        .style(Style::default().fg(bar_fg).bg(bar_bg))
        .render(rect, buf);
    Paragraph::new(Line::from(spans)).render(rect, buf);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_menubar_dropdown(
    category: usize,
    selected_item: usize,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    hints: &[Option<String>],
    buf: &mut Buffer,
) {
    let cat_idx = category.min(MENUBAR_CATEGORIES.len() - 1);
    let (cat_title, items) = (MENUBAR_CATEGORIES[cat_idx].title, MENUBAR_CATEGORIES[cat_idx].items);
    let dropdown = menubar_dropdown_rect(cat_idx, workspace_area);

    render_modal_surface_and_frame(dropdown, workspace_area, cat_title, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(dropdown);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::with_capacity(items.len());
    for (idx, entry) in items.iter().enumerate() {
        let is_sel = idx == selected_item.min(items.len().saturating_sub(1));
        let (fg, bg) = if cfg.is_modern() {
            if is_sel {
                (
                    palette.title_fg,
                    if cfg.liquid_glass {
                        role(cfg, palette, ColorRole::Overlay, Color::Rgb(24, 46, 78))
                    } else {
                        role(cfg, palette, ColorRole::Overlay, Color::Rgb(30, 41, 59))
                    },
                )
            } else {
                (palette.text_fg.unwrap_or(Color::White), palette.window_frame_bg)
            }
        } else if is_sel {
            (
                Color::Black,
                role(cfg, palette, ColorRole::BorderFocus, Color::Rgb(0, 255, 102)),
            )
        } else {
            (Color::White, palette.window_frame_bg)
        };
        let prefix = if is_sel {
            if cfg.is_modern() { "▎ " } else { "► " }
        } else {
            "  "
        };
        let style = Style::default().fg(fg).bg(bg).add_modifier(if is_sel {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
        let hint = hints.get(idx).and_then(|h| h.as_deref()).unwrap_or("");
        let text_w = usize::from(inner.width.saturating_sub(2));
        let hint_w = hint.chars().count();
        let label_w = entry.label.chars().count();
        // Right-align the accelerator; a hint that does not fit next to the label is dropped.
        let (hint, pad) = if label_w + hint_w < text_w {
            (hint, text_w - label_w - hint_w)
        } else {
            ("", text_w.saturating_sub(label_w))
        };
        let mut spans = vec![Span::styled(prefix.to_string(), style)];
        spans.extend(mnemonic_spans(entry.label, entry.mnemonic, style, "", ""));
        spans.push(Span::styled(" ".repeat(pad), style));
        spans.push(Span::styled(hint.to_string(), style.add_modifier(Modifier::DIM)));
        lines.push(Line::from(spans));
    }
    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
    if cfg.is_glass() {
        polish_glass_panel_rect(buf, dropdown, palette.window_frame_bg, palette, true);
    }
}

pub(super) fn render_inspector_modal(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let modal = wm.active_modal_rect(workspace_area).unwrap_or_else(|| {
        let w = workspace_area
            .width
            .saturating_sub(4)
            .clamp(24, 76)
            .min(workspace_area.width);
        let h = workspace_area
            .height
            .saturating_sub(2)
            .clamp(6, 20)
            .min(workspace_area.height);
        let x = workspace_area.x + (workspace_area.width.saturating_sub(w)) / 2;
        let y = workspace_area.y + (workspace_area.height.saturating_sub(h)) / 2;
        Rect::new(x, y, w, h)
    });

    let insp_title = if cfg.is_modern() {
        "● ● ●  Terminal Inspector · ↑↓ Scroll · F9/Esc Close"
    } else {
        "[F9] 3270 TERMINAL INSPECTOR (Up/Down Scroll, F9/Esc=Close)"
    };
    render_modal_surface_and_frame(modal, workspace_area, insp_title, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let pane = wm.active_pane();
    let vt = pane.vt();
    let (rows, cols) = vt.size();
    let (cur_r, cur_c) = vt.cursor_position();
    let cell_opt = vt.cell_snapshot(cur_r, cur_c);
    let cell_grapheme = cell_opt
        .as_ref()
        .map(|c| {
            let s = c.grapheme.as_str();
            if s.is_empty() { " " } else { s }
        })
        .unwrap_or(" ");
    let codepoints: String = cell_grapheme
        .chars()
        .map(|ch| format!("U+{:04X}", ch as u32))
        .collect::<Vec<_>>()
        .join(" ");

    let mut info_rows: Vec<(String, String)> = vec![
        (
            "PANE & GRID".to_string(),
            format!(
                "Pane #{} ({}) | Grid: {}x{} | Gen: #{} | Out: {}B",
                pane.id(),
                pane.title(),
                cols,
                rows,
                vt.snapshot().generation,
                vt.total_bytes_processed()
            ),
        ),
        (
            "DECSET MODES (1)".to_string(),
            format!(
                "?1049(Alt):{} ?2004(Paste):{} ?1006(Mouse):{} ?1007(AltScrl):{}",
                on_off(vt.is_alternate_screen()),
                on_off(vt.is_bracketed_paste()),
                on_off(vt.is_mouse_sgr_or_tracking()),
                on_off(vt.is_mouse_alternate_scroll()),
            ),
        ),
        (
            "DECSET MODES (2)".to_string(),
            format!(
                "?2026(Sync):{} ?2027(Graph):{} ?2031(Theme):{} ?2048(Size):{}",
                on_off(vt.is_synchronized_output()),
                on_off(vt.is_grapheme_cluster_2027()),
                on_off(vt.is_report_color_scheme_2031()),
                on_off(vt.is_in_band_size_2048()),
            ),
        ),
        (
            "KEYBOARD & LOCK".to_string(),
            format!(
                "DECCKM(?1):{} DECBKM(?67):{} KittyFlags:0x{:02X} ReadOnly:{} Bcast:{}",
                on_off(vt.is_application_cursor()),
                on_off(vt.is_backarrow_key_67()),
                vt.kitty_keyboard_flags(),
                on_off(pane.is_read_only()),
                on_off(wm.is_broadcast_input()),
            ),
        ),
        (
            "CURSOR & CELL".to_string(),
            format!(
                "R:{} C:{} | '{}' ({}) | UL:{:?} Overline:{}",
                cur_r + 1,
                cur_c + 1,
                cell_grapheme,
                codepoints,
                vt.active_underline_kind(),
                on_off(vt.is_overline()),
            ),
        ),
        (
            "LINE DISCIPLINE".to_string(),
            format!(
                "Mode:{:?} | ECHO:{} | Subcmd:{} | OSC133:{:?}",
                pane.effective_routing_mode(),
                on_off(pane.line_discipline().echo_enabled),
                on_off(pane.has_foreground_subcommand()),
                vt.osc133_phase(),
            ),
        ),
        (
            "SCROLL & PAGE".to_string(),
            format!(
                "Mode:{} | Hold:{} | Inspected:{} | Pending:+{}L",
                pane.scroll_mode().label(),
                on_off(pane.is_page_holding()),
                on_off(pane.is_holding_inspected()),
                pane.pending_page_lines(),
            ),
        ),
        (
            "OSC 9;4 & LINKS".to_string(),
            format!(
                "Progress:{:?} | OSC8 Links:{} | Blocks:{}",
                vt.osc9_progress(),
                vt.osc8_links().len(),
                vt.command_blocks().len(),
            ),
        ),
    ];

    if vt.recent_escape_sequences().is_empty() {
        info_rows.push((
            "ESCAPE RING".to_string(),
            "(No CSI/OSC control sequences recorded yet)".to_string(),
        ));
    } else {
        for (i, seq) in vt.recent_escape_sequences().iter().rev().enumerate() {
            info_rows.push((format!("ESC[-{i:02}]"), seq.clone()));
        }
    }

    let max_scroll = info_rows.len().saturating_sub(usize::from(inner.height));
    let start = wm.inspector_scroll().min(max_scroll);
    let mut lines = Vec::with_capacity(usize::from(inner.height));
    let cfg = wm.config();
    for (k, v) in info_rows
        .into_iter()
        .skip(start)
        .take(usize::from(inner.height))
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<16} ", k),
                Style::default()
                    .fg(role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 255, 85)))
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                v,
                Style::default()
                    .fg(role(cfg, palette, ColorRole::TextPrimary, Color::Rgb(85, 255, 255)))
                    .bg(palette.window_frame_bg),
            ),
        ]));
    }

    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

fn on_off(b: bool) -> &'static str {
    if b { "ON" } else { "OFF" }
}
