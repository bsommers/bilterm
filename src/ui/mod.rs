pub mod bitmap_font;
pub mod canvas_raster;
pub mod exit_guard_ui;
pub mod gui_font;
#[cfg(target_os = "macos")]
pub mod gui_macos;
pub mod gui_window;
pub mod chrome;
pub mod cursor;
pub mod drawer;
pub mod glyphs;
pub mod hires_subcell;
pub mod mcp_ui;
pub mod modals;
pub mod occlusion;
pub mod oia;
pub mod pixel_canvas;
pub mod renderer;
pub mod status_bar;
pub mod theme;
pub mod theme_import;
pub mod typography;

pub use canvas_raster::{braille_char_from_mask, render_graphical_canvas, sextant_char_from_mask};
pub use drawer::render_workspace_drawer;
pub use exit_guard_ui::{render_exit_guard_modal, render_held_shell_exited_banner};
pub use hires_subcell::{CanvasResolution, probe_terminal_pixel_size, subcell_mask_to_octant};
pub use pixel_canvas::{PixelCanvasBuffer, wrap_tmux_dcs_passthrough};
pub use status_bar::{render_oia_status_bar, render_oia_status_bar_themed};
pub use theme_import::{OpenColorScheme, ThemeTypographyProfile};
pub use typography::{
    FontFamilyPreset, LineSizeAttr, PaneFontOptions, encode_host_font_passthrough,
    scan_dec_line_size_escapes,
};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::cursor::{
    SpotlightMode, render_caret, render_crosshair_cursor, render_focal_zoom_cursor,
    render_spotlight_cursor,
};
use crate::ui::modals::{
    render_help_modal, render_history_popup, render_inspector_modal, render_menubar_dropdown,
    render_palette_modal, render_top_menubar, render_unsafe_paste_modal,
};
use crate::ui::occlusion::occlude_rect;
use crate::ui::status_bar::{render_dos_fkey_bar, render_floating_prompt};
use crate::ui::theme::ThemePalette;
use crate::vt::snapshot::CellSnapshot;
use crate::wm::{WindowManager, WorkspaceLayoutMode, inset_window_frame};

/// Ratatui widget rendering a `CompositePane` in 3 immediate-mode passes:
/// 1. **Pass 1 (`OutputCanvas`)**: Renders the full-height VT100/ANSI/TrueColor screen (`area.height - 1`)
/// 2. **Pass 2 (`FloatingCommandPrompt`)**: If visible, clears `prompt_rect` (`occlude_rect`), draws optional Borland drop shadow, and renders the floating command prompt + optional IBM 3270 cross-hair cursor on top
/// 3. **Pass 3 (`OiaStatusBar`)**: Renders the bottom 1-row Operator Information Area (OIA) status bar
pub struct CompositePaneWidget<'a> {
    pub pane: &'a CompositePane,
    pub active: bool,
    pub config: Option<&'a BiltermConfig>,
}

impl<'a> CompositePaneWidget<'a> {
    pub fn new(pane: &'a CompositePane, active: bool) -> Self {
        Self {
            pane,
            active,
            config: None,
        }
    }

    pub fn with_config(pane: &'a CompositePane, active: bool, config: &'a BiltermConfig) -> Self {
        Self {
            pane,
            active,
            config: Some(config),
        }
    }
}

impl Widget for CompositePaneWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let default_cfg = BiltermConfig::default();
        let cfg = self.config.unwrap_or(&default_cfg);
        let palette = cfg.palette();
        let glyphs = cfg.glyph_set();

        let canvas = self.pane.canvas_rect(area);
        let status_rect = self.pane.status_bar_rect(area);

        // Pass 1: Render full-height OutputCanvas from VtParser screen
        render_output_canvas(self.pane, canvas, cfg, &palette, self.active, buf);

        // Pass 2: Render overlaid or split FloatingCommandPrompt if visible
        if self.pane.is_prompt_visible() && canvas.height >= 2 {
            let mut prompt_rect = self.pane.prompt_rect(area);
            if self.pane.copy_mode().active && !self.pane.prompt().anchor.is_split() {
                let cr = canvas.y + self.pane.copy_mode().cursor_row;
                if cr >= prompt_rect.y && cr < prompt_rect.y + prompt_rect.height {
                    if prompt_rect.y <= canvas.y + canvas.height / 2 {
                        prompt_rect.y =
                            canvas.y + canvas.height.saturating_sub(prompt_rect.height);
                    } else {
                        prompt_rect.y = canvas.y;
                    }
                }
            }
            if cfg.drop_shadows && !self.pane.prompt().anchor.is_split() {
                render_drop_shadow_styled(buf, prompt_rect, canvas, cfg.shadow_style, &palette);
            }
            occlude_rect(buf, prompt_rect);
            render_floating_prompt(
                self.pane,
                self.active && !self.pane.copy_mode().active,
                prompt_rect,
                canvas,
                cfg,
                &palette,
                &glyphs,
                buf,
            );
        }

        // Pass 3: Render bottom 1-row Operator Information Area (OIA) status bar
        render_oia_status_bar_themed(self.pane, status_rect, &palette, &glyphs, buf);

        // Pass 4: Render optional Borland-style Command History Pop-up Window if active on this pane
        if self.pane.is_prompt_visible()
            && self.pane.prompt().history_popup_open
            && canvas.height >= 5
        {
            render_history_popup(self.pane, canvas, cfg, &palette, &glyphs, buf);
        }

        // Pass 5: Render unsafe multi-line paste confirmation modal if pending (FND-01)
        if let Some(pending_paste) = self.pane.pending_unsafe_paste()
            && canvas.height >= 5
            && canvas.width >= 24
        {
            render_unsafe_paste_modal(pending_paste, canvas, cfg, &palette, buf);
        }
    }
}

fn is_safe_terminal_cluster(s: &str, expected_width: usize, grapheme_mode_2027: bool) -> bool {
    if s.is_empty() {
        return false;
    }
    let w = unicode_width::UnicodeWidthStr::width(s);
    if w != expected_width {
        let is_single_pua = expected_width == 1
            && s.chars().count() == 1
            && s.chars().next().is_some_and(gui_font::geometric::is_private_use_area);
        if !is_single_pua {
            return false;
        }
    }
    for ch in s.chars() {
        let code = ch as u32;
        if grapheme_mode_2027 && matches!(code, 0x200D | 0xFE0F) {
            continue;
        }
        if ch.is_control()
            || matches!(
                code,
                0x200B..=0x200F
                    | 0x2028..=0x202F
                    | 0x2060..=0x206F
                    | 0xFE00..=0xFE0F
                    | 0xFEFF
                    | 0xFFF9..=0xFFFB
            )
        {
            return false;
        }
    }
    true
}

fn render_output_canvas(
    pane: &CompositePane,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    active: bool,
    buf: &mut Buffer,
) {
    let vt = pane.vt();
    let (vt_rows, vt_cols) = vt.size();
    let render_rows = canvas.height.min(vt_rows);
    let render_cols = canvas.width.min(vt_cols);
    let grapheme_2027 = vt.is_grapheme_cluster_2027();

    let font_opts = pane.font_options();
    for r in 0..render_rows {
        let mut line_attr = vt.line_size_attr(r);
        if !line_attr.is_double_width() && font_opts.double_width {
            line_attr = LineSizeAttr::DoubleWidth;
        }
        if line_attr.is_double_width() {
            typography::render_double_width_row(vt, r, canvas, render_cols, line_attr, buf);
            continue;
        }
        let mut skip_next_wide_continuation = false;
        for c in 0..render_cols {
            let Some(vt_cell) = vt.cell_view(r, c) else {
                skip_next_wide_continuation = false;
                continue;
            };
            let dst_x = canvas.x + c;
            let dst_y = canvas.y + r;
            let mut style = vt_cell.to_ratatui_style();
            if font_opts.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if font_opts.italic {
                style = style.add_modifier(Modifier::ITALIC);
            }

            if skip_next_wide_continuation {
                skip_next_wide_continuation = false;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_style(style);
                }
                continue;
            }

            if vt_cell.is_wide_continuation() {
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
                continue;
            }

            let contents = vt_cell.grapheme;
            if vt_cell.is_wide() {
                // Prevent a 2-column wide glyph at the rightmost column from overflowing/wrapping the host terminal
                if c + 1 < render_cols && is_safe_terminal_cluster(contents, 2, grapheme_2027) {
                    if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                        dst_cell.set_symbol(contents);
                        dst_cell.set_style(style);
                    }
                    skip_next_wide_continuation = true;
                } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
            } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                if contents.is_empty() {
                    dst_cell.set_symbol(" ");
                } else if is_safe_terminal_cluster(contents, 1, grapheme_2027) {
                    dst_cell.set_symbol(contents);
                } else {
                    dst_cell.set_symbol("·");
                }
                dst_cell.set_style(style);
            }
        }
    }

    // Highlight detected URLs with underline + bright cyan tint (VAL-03)
    for url in pane.vt().detect_visible_urls() {
        if url.row < render_rows {
            for c in url.start_col..=url.end_col.min(render_cols.saturating_sub(1)) {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + url.row)) {
                    cell.set_style(
                        cell.style()
                            .fg(Color::Rgb(85, 255, 255))
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
            }
        }
    }

    // Highlight custom output triggers (v0.7.0)
    for trigger in &cfg.triggers {
        for m in pane.vt().search_canvas(trigger) {
            if m.row < render_rows {
                for c in m.start_col..=m.end_col.min(render_cols.saturating_sub(1)) {
                    if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + m.row)) {
                        cell.set_style(
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Rgb(200, 0, 80))
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }

    // Apply CRT phosphor scanline effect on odd rows when enabled (v0.6.0)
    if cfg.crt_scanlines {
        for r in (1..render_rows).step_by(2) {
            for c in 0..render_cols {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r)) {
                    cell.set_style(cell.style().add_modifier(Modifier::DIM));
                }
            }
        }
    }

    // Render Kitty Graphics (APC G) and DEC Sixel (DCS q) placements using TrueColor half-blocks (GFX-01)
    for gfx in pane.vt().graphic_placements() {
        let max_r = gfx.rows.max(1);
        let max_c = gfx.cols.max(1);
        for dr in 0..max_r {
            let dst_r = gfx.row.saturating_add(dr);
            if dst_r >= render_rows {
                break;
            }
            for dc in 0..max_c {
                let dst_c = gfx.col.saturating_add(dc);
                if dst_c >= render_cols {
                    break;
                }
                let px_w = usize::from(gfx.pixel_width.max(1));
                let px_h = usize::from(gfx.pixel_height.max(1));
                let sample_x = ((usize::from(dc) * px_w) / usize::from(max_c)).min(px_w - 1);
                let top_y =
                    ((usize::from(dr) * 2 * px_h) / (usize::from(max_r) * 2)).min(px_h - 1);
                let bot_y =
                    (((usize::from(dr) * 2 + 1) * px_h) / (usize::from(max_r) * 2)).min(px_h - 1);
                let top_off = (top_y * px_w + sample_x) * 4;
                let bot_off = (bot_y * px_w + sample_x) * 4;
                if top_off + 3 < gfx.rgba_bytes.len() && bot_off + 3 < gfx.rgba_bytes.len() {
                    let tr = gfx.rgba_bytes[top_off];
                    let tg = gfx.rgba_bytes[top_off + 1];
                    let tb = gfx.rgba_bytes[top_off + 2];
                    let br = gfx.rgba_bytes[bot_off];
                    let bg_g = gfx.rgba_bytes[bot_off + 1];
                    let bb = gfx.rgba_bytes[bot_off + 2];
                    if let Some(cell) = buf.cell_mut((canvas.x + dst_c, canvas.y + dst_r)) {
                        cell.set_symbol("▀");
                        cell.set_style(
                            Style::default()
                                .fg(Color::Rgb(tr, tg, tb))
                                .bg(Color::Rgb(br, bg_g, bb)),
                        );
                    }
                }
            }
        }
    }

    // Render Tektronix 4010/4014 storage-tube vectors and modern 2026/2027 graphical canvas (TEK-01, CANV-01)
    canvas_raster::render_graphical_canvas(pane, canvas, cfg, buf);

    // Highlight active OutputCanvas search matches (VAL-02)
    if pane.canvas_search().active && !pane.canvas_search().matches.is_empty() {
        for r in 0..render_rows {
            for c in 0..render_cols {
                let (is_match, is_sel) = pane.canvas_search().cell_match_status(r, c);
                if is_match
                    && let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r))
                {
                    let bg = if is_sel {
                        Color::Rgb(255, 176, 0)
                    } else {
                        Color::Rgb(255, 255, 85)
                    };
                    cell.set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }

    // Highlight Vi Copy Mode or mouse drag selection and cursor (VAL-04)
    if pane.copy_mode().active || pane.copy_mode().has_selection() {
        for r in 0..render_rows {
            for c in 0..render_cols {
                if pane.copy_mode().is_cell_selected(r, c)
                    && let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r))
                {
                    cell.set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(85, 255, 255))
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
        if pane.copy_mode().active {
            let cr = pane.copy_mode().cursor_row.min(render_rows.saturating_sub(1));
            let cc = pane.copy_mode().cursor_col.min(render_cols.saturating_sub(1));
            let cx = canvas.x + cc;
            let cy = canvas.y + cr;
            if active && cfg.crosshair_cursor {
                let target_rect = if cfg.crosshair_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_crosshair_cursor(buf, target_rect, cx, cy, palette.crosshair_bg);
            }
            render_caret(buf, cx, cy, cfg.cursor_style, Color::Rgb(255, 255, 85));
        }
    }

    // Render QuickSelect 3270 Field Hints badges ([a], [b], ...) when active (ECO-01)
    if pane.hints_mode().active && !pane.hints_mode().hints.is_empty() {
        for (idx, hint) in pane.hints_mode().hints.iter().enumerate() {
            if hint.row >= render_rows {
                continue;
            }
            let is_sel = idx == pane.hints_mode().selected;
            let span_fg = if is_sel {
                Color::Rgb(255, 255, 85)
            } else {
                Color::Rgb(85, 255, 255)
            };
            for c in hint.start_col..=hint.end_col.min(render_cols.saturating_sub(1)) {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + hint.row)) {
                    cell.set_style(
                        cell.style()
                            .fg(span_fg)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    );
                }
            }
            let badge = format!("[{}]", hint.label);
            let badge_bg = if is_sel {
                Color::Rgb(255, 176, 0)
            } else {
                Color::Rgb(255, 255, 85)
            };
            for (ch_idx, ch) in badge.chars().enumerate() {
                let col = hint.start_col.saturating_add(ch_idx as u16);
                if col < render_cols
                    && let Some(cell) = buf.cell_mut((canvas.x + col, canvas.y + hint.row))
                {
                    let mut sym = [0u8; 4];
                    cell.set_symbol(ch.encode_utf8(&mut sym));
                    cell.set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(badge_bg)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }

    // When in RawPtyPassthrough and not scrolled back, highlight the live PTY cursor cell
    if pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
        && vt.scrollback_offset() == 0
        && vt.is_cursor_visible()
    {
        let (cur_r, cur_c) = vt.cursor_position();
        if cur_r < canvas.height && cur_c < canvas.width {
            let cx = canvas.x + cur_c;
            let cy = canvas.y + cur_r;
            if active
                && cfg.focal_zoom_mode != SpotlightMode::Off
                && cfg.focal_zoom_pct > 100
            {
                let target_rect = if cfg.focal_zoom_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_focal_zoom_cursor(
                    buf,
                    target_rect,
                    cx,
                    cy,
                    cfg.focal_zoom_mode,
                    cfg.focal_zoom_pct,
                    cfg.focal_zoom_rx,
                    cfg.focal_zoom_ry,
                    cfg.focal_zoom_feather_pct,
                    cfg.focal_zoom_pixelated,
                    palette.crosshair_bg,
                );
            }
            if active && cfg.spotlight_mode != SpotlightMode::Off {
                let target_rect = if cfg.spotlight_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_spotlight_cursor(
                    buf,
                    target_rect,
                    cx,
                    cy,
                    cfg.spotlight_mode,
                    cfg.spotlight_rx,
                    cfg.spotlight_ry,
                    cfg.spotlight_feather_pct,
                    cfg.spotlight_pixelated,
                    palette.crosshair_bg,
                );
            }
            if active && cfg.crosshair_cursor {
                let target_rect = if cfg.crosshair_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_crosshair_cursor(buf, target_rect, cx, cy, palette.crosshair_bg);
            }
            render_caret(buf, cx, cy, cfg.cursor_style, palette.active_border_fg);
        }
        if let Some(held) = pane.held_exited_shell() {
            render_held_shell_exited_banner(buf, canvas, palette, held);
        }
    }
}

/// Multi-pane workspace compositor (`WM-01..05`, `UI-01..05`) rendering:
/// 1. Tiled H/V splits (`SplitTree`) or overlapping z-order stacked windows (`ZStack`) in ascending z-order
/// 2. Borland Turbo Vision window frames (`╔═[1] PANE #1 ═╗` active / `┌─[2] PANE #2 ─┐` inactive) and translucent drop shadows
/// 3. Top-anchored persistent Guake/Quake-style drop-down `QuickTerminal` overlay (`[Q] QUAKE QUICK TERM [~]`)
/// 4. Optional bottom 1-row Borland DOS Function-Key bar (`F1 Help  F2 SplitH ...`)
pub struct WorkspaceWidget<'a> {
    pub wm: &'a WindowManager,
}

impl<'a> WorkspaceWidget<'a> {
    pub fn new(wm: &'a WindowManager) -> Self {
        Self { wm }
    }
}

impl Widget for WorkspaceWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let cfg = self.wm.config();
        let palette = cfg.palette();
        let glyphs = cfg.glyph_set();

        let top_offset = u16::from(cfg.show_top_menubar && area.height > 3);
        let bottom_offset = u16::from(cfg.show_dos_fkey_bar && area.height > 2 + top_offset);
        let workspace_area = Rect {
            x: area.x,
            y: area.y + top_offset,
            width: area.width,
            height: area
                .height
                .saturating_sub(top_offset + bottom_offset)
                .max(1),
        };

        let mut rects = self.wm.compute_workspace_rects(workspace_area);
        let framed = self.wm.is_framed();
        let top_workspace_id = self.wm.z_stack().top();
        let qt_visible = self.wm.is_quick_terminal_visible();
        if self.wm.layout_mode() == WorkspaceLayoutMode::Tiled {
            rects.sort_by_key(|(id, _)| !qt_visible && top_workspace_id == Some(*id));
        }

        for (id, outer) in rects {
            let Some(pane) = self.wm.pane(id) else {
                continue;
            };
            let is_active = !qt_visible && top_workspace_id == Some(id);
            if framed {
                if cfg.drop_shadows && self.wm.layout_mode() == WorkspaceLayoutMode::Stacked {
                    render_drop_shadow_styled(
                        buf,
                        outer,
                        workspace_area,
                        cfg.shadow_style,
                        &palette,
                    );
                }
                occlude_rect(buf, outer);
                let z_badge = self
                    .wm
                    .z_stack()
                    .rank_of(id)
                    .map(|(r, total)| format!("Z:{r}/{total}"))
                    .unwrap_or_default();
                let lock_tag = if pane.is_read_only() { " 🔒" } else { "" };
                render_borland_frame_styled(
                    outer,
                    &format!("[{id}] {}{lock_tag} ({z_badge})", pane.title()),
                    is_active,
                    cfg.window_outline,
                    &palette,
                    &glyphs,
                    buf,
                );
                let inner = inset_window_frame(outer);
                CompositePaneWidget::with_config(pane, is_active, cfg).render(inner, buf);
            } else {
                CompositePaneWidget::with_config(pane, is_active, cfg).render(outer, buf);
            }
        }

        // Render `herdr`-style slide-out Sessions / Windows / Panes & Fleet drawer (DRW-01, DRW-02)
        render_workspace_drawer(self.wm, workspace_area, cfg, &palette, &glyphs, buf);

        // Render persistent Guake/Quake-style drop-down quick terminal overlay on top (WM-05)
        if qt_visible {
            let qt_outer = self.wm.quick_terminal().overlay_rect(workspace_area);
            if cfg.drop_shadows {
                render_drop_shadow_styled(
                    buf,
                    qt_outer,
                    workspace_area,
                    cfg.shadow_style,
                    &palette,
                );
            }
            occlude_rect(buf, qt_outer);
            render_borland_frame_styled(
                qt_outer,
                "[Q] QUAKE QUICK TERM [~] (Alt+` to hide)",
                true,
                cfg.window_outline,
                &palette,
                &glyphs,
                buf,
            );
            let qt_inner = inset_window_frame(qt_outer);
            CompositePaneWidget::with_config(&self.wm.quick_terminal().pane, true, cfg)
                .render(qt_inner, buf);
        }

        // Render optional top 1-row Borland Turbo Vision Menu Bar & active pull-down menu (ECO-03)
        if top_offset > 0 {
            let menubar_rect = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            render_top_menubar(menubar_rect, self.wm, &palette, buf);
            if let Some((cat, sel_item)) = self.wm.menubar_state()
                && workspace_area.height >= 6
                && workspace_area.width >= 24
            {
                render_menubar_dropdown(
                    cat,
                    sel_item,
                    workspace_area,
                    cfg,
                    &palette,
                    &glyphs,
                    buf,
                );
            }
        }

        if bottom_offset > 0 {
            let fkey_rect = Rect {
                x: area.x,
                y: area.y + area.height - 1,
                width: area.width,
                height: 1,
            };
            render_dos_fkey_bar(fkey_rect, &palette, buf);
        }

        // Render interactive <F9> Live 3270 Terminal Inspector modal on top of workspace (BEL-01)
        if self.wm.is_inspector_visible() && workspace_area.width >= 24 && workspace_area.height >= 6 {
            render_inspector_modal(
                self.wm,
                workspace_area,
                cfg,
                &palette,
                &glyphs,
                buf,
            );
        }

        // Render interactive <F1> Borland Turbo Vision Help Modal on top of the workspace
        if self.wm.is_help_visible() && workspace_area.width >= 20 && workspace_area.height >= 6 {
            render_help_modal(
                self.wm,
                workspace_area,
                cfg,
                &palette,
                &glyphs,
                buf,
            );
        }

        // Render interactive Fuzzy Command Palette modal on top of the workspace (v0.6.0)
        if self.wm.is_palette_visible() && workspace_area.width >= 24 && workspace_area.height >= 6 {
            render_palette_modal(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }

        if let Some(exit_state) = self.wm.exit_guard_state() {
            render_exit_guard_modal(buf, workspace_area, &palette, exit_state);
        }

        mcp_ui::render_mcp_console_overlay(self.wm, workspace_area, cfg, &palette, buf);

        // Capture full composited workspace frame if Whole-Workspace Asciicast v2 recording is active (REC-02)
        self.wm.record_workspace_frame(buf);
    }
}

/// Converts a `ratatui::buffer::Buffer` into a deterministic newline-delimited string for snapshot assertions.
pub fn buffer_to_snapshot_string(buf: &Buffer) -> String {
    let area = buf.area;
    let mut lines = Vec::with_capacity(usize::from(area.height));
    for y in area.y..area.y + area.height {
        let mut row = String::with_capacity(usize::from(area.width));
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell((x, y)) {
                row.push_str(cell.symbol());
            }
        }
        lines.push(row.trim_end().to_string());
    }
    lines.join("\n")
}

/// Converts a [`CellSnapshot`] into a `ratatui::style::Style` preserving 16-color, 256-color,
/// and 24-bit TrueColor attributes plus bold, italic, underline, and inverse modifiers (`ARCH-R03`).
pub fn cell_to_ratatui_style(cell: &CellSnapshot) -> Style {
    let mut style = Style::default()
        .fg(cell.fg.to_ratatui_color())
        .bg(cell.bg.to_ratatui_color());

    let mut modifiers = Modifier::empty();
    if cell.bold {
        modifiers |= Modifier::BOLD;
    }
    if cell.italic {
        modifiers |= Modifier::ITALIC;
    }
    if cell.underline.is_underlined() {
        modifiers |= Modifier::UNDERLINED;
    }
    if cell.inverse {
        modifiers |= Modifier::REVERSED;
    }
    style = style.add_modifier(modifiers);
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_composite_pane_with_floating_prompt_and_oia_bar() {
        let mut pane = CompositePane::new_headless(1, 10, 60);
        pane.process_pty_output(b"LINE_0_UNDER_PROMPT\r\nLINE_1_UNDER_PROMPT\r\nLINE_2_UNDER_PROMPT\r\nLINE_3_VISIBLE_BELOW\r\n");
        pane.prompt_mut().set_text("ls -la");

        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        CompositePaneWidget::new(&pane, true).render(area, &mut buf);

        let rendered = buffer_to_snapshot_string(&buf);
        assert!(rendered.contains("BILTERM 3270 // CMD [TOP]"));
        assert!(rendered.contains("==> ls -la"));
        assert!(rendered.contains("[▲][▼]"));
        assert!(rendered.contains("LINE_3_VISIBLE_BELOW"));
        assert!(rendered.contains("INPUT: CMD BOX"));
    }
}
