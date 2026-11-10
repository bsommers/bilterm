pub mod agents_gui;
pub mod agents_hud_ui;
pub mod agents_list_ui;
pub mod agents_ui;
pub mod clipboard_window_gui;
pub mod coord_badge;
pub mod clipboard_window_ui;
pub mod traffic_monitor_gui;
pub mod traffic_monitor_ui;
pub mod command_window_ui;
pub mod bitmap_font;
pub mod canvas_cache;
pub mod canvas_layer;
pub mod canvas_raster;
pub mod contrast;
pub mod copy_hints;
pub mod daemon_window_ui;
pub mod exit_guard_ui;
pub mod file_browser;
pub mod find_ui;
pub mod fonts_window_ui;
pub mod gui_font;
#[cfg(target_os = "macos")]
pub mod gui_macos;
pub mod gui_window;
pub mod chrome;
pub mod cursor;
pub mod cursor_cmd;
pub mod design;
pub mod drawer;
pub mod drawer_preview;
pub mod glyphs;
pub mod graph_raster;
pub mod hires_subcell;
pub mod mcp_ui;
pub mod media_canvas_gui;
pub mod menu_model;
pub mod modals;
pub mod occlusion;
pub mod pane_actions;
pub mod oia;
pub mod pixel_canvas;
pub mod navigator;
pub mod rename_dialog_ui;
pub mod renderer;
pub mod script_studio_ui;
pub mod session_map_gui;
pub mod session_map_ui;
pub mod shell;
pub mod status_bar;
pub mod theme;
pub mod theme_import;
pub mod typography;
pub mod visual_style;
pub mod which_key_ui;

pub use canvas_raster::{braille_char_from_mask, render_graphical_canvas, sextant_char_from_mask};
pub use drawer::render_workspace_drawer;
pub use exit_guard_ui::{render_exit_guard_modal, render_held_shell_exited_banner, render_link_banner};
pub use hires_subcell::{CanvasResolution, probe_terminal_pixel_size, subcell_mask_to_octant};
pub use pixel_canvas::{PixelCanvasBuffer, wrap_tmux_dcs_passthrough};
pub use status_bar::{render_oia_status_bar, render_oia_status_bar_themed};
pub use theme_import::{OpenColorScheme, ThemeTypographyProfile};
pub use typography::{
    FontFamilyPreset, LineSizeAttr, PaneFontOptions, encode_host_font_passthrough,
    scan_dec_line_size_escapes,
};
pub use visual_style::RawFkeyMode;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::cursor::{
    SpotlightMode, render_crosshair_cursor, render_focal_zoom_cursor,
    render_spotlight_cursor,
};
use crate::ui::modals::{
    render_help_modal, render_history_popup, render_inspector_modal, render_menubar_dropdown,
    render_palette_modal, render_top_menubar, render_unsafe_paste_modal,
};
use crate::ui::occlusion::occlude_rect;
use crate::ui::design::DisplayMode;
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
    /// Optional per-window palette override (`:window theme <name>`).
    pub palette: Option<ThemePalette>,
    /// `Some(is_gui)` draws the OIA `[TUI]`/`[GUI]` badge on non-Legacy profiles.
    pub surface_badge: Option<bool>,
}

impl<'a> CompositePaneWidget<'a> {
    pub fn new(pane: &'a CompositePane, active: bool) -> Self {
        Self {
            pane,
            active,
            config: None,
            palette: None,
            surface_badge: None,
        }
    }

    /// Applies a per-window palette override (`:window theme <name>`).
    pub fn with_palette(mut self, palette: Option<ThemePalette>) -> Self {
        self.palette = palette;
        self
    }

    /// Opts into the OIA surface badge; Legacy (Classic) profiles still render without it.
    pub fn with_surface_badge(mut self, gui_surface: bool) -> Self {
        self.surface_badge = Some(gui_surface);
        self
    }

    pub fn with_config(pane: &'a CompositePane, active: bool, config: &'a BiltermConfig) -> Self {
        Self {
            pane,
            active,
            config: Some(config),
            palette: None,
            surface_badge: None,
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
        let palette = self.palette.clone().unwrap_or_else(|| cfg.palette());
        let glyphs = cfg.glyph_set();

        let canvas = self.pane.canvas_rect(area);
        let status_rect = self.pane.status_bar_rect(area);

        // Pass 1: Render full-height OutputCanvas from VtParser screen
        render_output_canvas(self.pane, canvas, cfg, &palette, self.active, buf);

        if let Some(browser) = self.pane.browser_canvas()
            && canvas.height > 1
        {
            let body = Rect::new(canvas.x, canvas.y + 1, canvas.width, canvas.height - 1);
            rasterize_frame_halfblocks(buf, body, browser.cached_frame());
        }
        if let Some(video) = self.pane.video_canvas()
            && canvas.height > 1
        {
            let body = Rect::new(canvas.x, canvas.y, canvas.width, canvas.height - 1);
            rasterize_frame_halfblocks(buf, body, video.cached_frame());
        }
        if let Some(browser) = self.pane.browser_canvas() {
            crate::pane::browser_canvas::render_browser_nav_bar(
                buf,
                Rect::new(canvas.x, canvas.y, canvas.width, 1),
                browser,
                self.active,
            );
        }
        if let Some(video) = self.pane.video_canvas() {
            crate::pane::video_canvas::render_video_osd_bar(
                buf,
                Rect::new(canvas.x, canvas.bottom().saturating_sub(1), canvas.width, 1),
                video,
                self.active,
            );
        }

        // Pass 2: Render overlaid or split FloatingCommandPrompt if visible
        if self.pane.is_prompt_visible() && canvas.height >= 2 {
            let prompt_rect = self.pane.prompt_rect(area);
            if cfg.liquid_glass && !self.pane.prompt().anchor.is_split() {
                visual_style::render_liquid_ambient_halo(buf, prompt_rect, canvas, &palette);
            } else if cfg.drop_shadows && !cfg.is_modern() && !self.pane.prompt().anchor.is_split() {
                render_drop_shadow_styled(buf, prompt_rect, canvas, cfg.shadow_style, &palette);
            }
            if cfg.is_glass() {
                visual_style::apply_frosted_glass_surface(buf, prompt_rect, palette.prompt_surface_bg, &palette);
            } else {
                occlude_rect(buf, prompt_rect);
            }
            render_floating_prompt(
                self.pane, self.active && !self.pane.copy_mode().active, prompt_rect, canvas,
                cfg, &palette, &glyphs, buf,
            );
        }

        // Pass 3: Render bottom 1-row Operator Information Area (OIA) status bar
        if shell::effective_display_mode(cfg) == DisplayMode::Zen {
            shell::render_zen_row(self.pane, status_rect, &palette, buf);
        } else if let Some(gui) = self.surface_badge
            && design::AppearanceProfile::resolve(cfg) != design::AppearanceProfile::Legacy
        {
            oia::render_oia_status_bar_with_surface(self.pane, status_rect, &palette, &glyphs, gui, buf);
        } else {
            render_oia_status_bar_themed(self.pane, status_rect, &palette, &glyphs, buf);
        }

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

/// Rasterizes an RGBA frame into `area` using TrueColor `▀` half-blocks (2 pixel rows per cell).
fn rasterize_frame_halfblocks(
    buf: &mut Buffer,
    area: Rect,
    frame: Option<&crate::pane::browser_canvas::MediaRgbaFrame>,
) {
    let Some(f) = frame else { return };
    let (fw, fh) = (f.width_px as usize, f.height_px as usize);
    if fw == 0 || fh == 0 || area.width == 0 || area.height == 0 || f.rgba.len() < fw * fh * 4 {
        return;
    }
    let (cols, prows) = (area.width as usize, area.height as usize * 2);
    let px = |cx: usize, py: usize| {
        let x = (cx * fw / cols).min(fw - 1);
        let y = (py * fh / prows).min(fh - 1);
        let i = (y * fw + x) * 4;
        Color::Rgb(f.rgba[i], f.rgba[i + 1], f.rgba[i + 2])
    };
    for r in 0..area.height as usize {
        for c in 0..cols {
            let cell = &mut buf[(area.x + c as u16, area.y + r as u16)];
            cell.set_char('▀').set_fg(px(c, r * 2)).set_bg(px(c, r * 2 + 1));
        }
    }
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

    // Layer 1 (cached per pane): background fill, VT cells with contrast, URL and trigger
    // highlights, CRT scanlines. Everything below is cheap or state-dependent and redrawn each frame.
    let layer_inputs = canvas_layer::CanvasLayerInputs::gather(pane, canvas, cfg, palette);
    canvas_cache::paint_canvas_layer_cached(vt, &layer_inputs, buf);

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

    // Highlight Vi Copy Mode or mouse drag selection and cursor (VAL-04, COPY-02)
    if pane.copy_mode().active
        || pane.copy_mode().has_selection()
        || pane.copy_mode().flash_region.is_some()
    {
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
        if active && pane.copy_mode().active {
            let cr = pane.copy_mode().cursor_row.min(render_rows.saturating_sub(1));
            let cc = pane.copy_mode().cursor_col.min(render_cols.saturating_sub(1));
            let cx = canvas.x + cc;
            let cy = canvas.y + cr;
            if cfg.crosshair_cursor {
                let target_rect = if cfg.crosshair_fullscreen {
                    buf.area
                } else {
                    canvas
                };
                render_crosshair_cursor(buf, target_rect, cx, cy, palette.crosshair_bg);
            }
            cursor_cmd::render_caret_configured(buf, cx, cy, cfg, palette);
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

    // When in RawPtyPassthrough and not scrolled back (nor in active Copy Mode), highlight the live PTY cursor cell
    if active
        && pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
        && !pane.copy_mode().active
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
            cursor_cmd::render_caret_configured(buf, cx, cy, cfg, palette);
        }
        if let Some(held) = pane.held_exited_shell() {
            render_held_shell_exited_banner(buf, canvas, palette, held);
        }
        if let Some(banner) = pane.link_banner() {
            render_link_banner(buf, canvas, palette, banner);
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

        let shell = shell::ShellLayout::compute(area, cfg, self.wm.pane_count());
        let workspace_area = shell.workspace;

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
            // Per-window theme / outline / bold / cursor overrides (`:window`, `:theme`, `:cursor`)
            let chrome_style = self.wm.pane_chrome(id);
            let (pane_cfg, pane_palette) = chrome_style.resolve_config_and_palette(cfg);
            if framed {
                if cfg.liquid_glass && self.wm.layout_mode() == WorkspaceLayoutMode::Stacked {
                    visual_style::render_liquid_ambient_halo(buf, outer, workspace_area, &pane_palette);
                } else if cfg.drop_shadows && !cfg.is_modern() && self.wm.layout_mode() == WorkspaceLayoutMode::Stacked {
                    render_drop_shadow_styled(buf, outer, workspace_area, cfg.shadow_style, &pane_palette);
                }
                if cfg.is_glass() && self.wm.layout_mode() == WorkspaceLayoutMode::Stacked {
                    visual_style::apply_frosted_glass_surface(buf, outer, pane_palette.window_frame_bg, &pane_palette);
                } else {
                    occlude_rect(buf, outer);
                }
                let z_badge = self.wm.z_stack().rank_of(id).map(|(r, total)| format!("Z:{r}/{total}")).unwrap_or_default();
                let lock_tag = if pane.is_read_only() {
                    if cfg.is_modern() { " · ◈" } else { " 🔒" }
                } else {
                    ""
                };
                let frame_title = if cfg.is_modern() {
                    visual_style::format_modern_window_title(self.wm, pane, id, &z_badge, lock_tag)
                } else if cfg.snazzy_mode {
                    visual_style::format_snazzy_window_title(self.wm, pane, id, &z_badge, lock_tag)
                } else {
                    format!("[{id}] {}{lock_tag} ({z_badge})", pane.title())
                };
                let non_legacy = design::AppearanceProfile::resolve(cfg) != design::AppearanceProfile::Legacy;
                let show_title = match cfg.appearance.title {
                    Some(policy) if non_legacy => policy.shows(is_active),
                    _ => shell::effective_display_mode(cfg) != DisplayMode::Focus,
                };
                let frame_title = if show_title { frame_title } else { String::new() };
                let mut frame_palette = pane_palette.clone();
                if non_legacy && cfg.appearance.focus_ring {
                    frame_palette.active_border_fg = design::DesignTokens::from_palette(&pane_palette).accent;
                }
                chrome::render_borland_frame_ext(
                    outer, &frame_title, is_active, chrome_style.resolve_outline(cfg),
                    chrome_style.resolve_bold(cfg), &frame_palette, &glyphs, buf,
                );
                if self.wm.coord_style() != crate::wm::coordinates::CoordDisplayStyle::Off
                    && let Some(badge) = self.wm.coord_of(id).and_then(|c| c.format(self.wm.coord_style()))
                {
                    coord_badge::render_coord_badge(outer, &frame_title, &badge, &frame_palette, &glyphs, buf);
                }
                if is_active {
                    pane_actions::render(cfg, outer, &pane_palette, buf);
                }
                if cfg.liquid_glass {
                    visual_style::polish_glass_panel_rect(buf, outer, pane_palette.window_frame_bg, &pane_palette, is_active);
                }
                let inner = inset_window_frame(outer);
                CompositePaneWidget::with_config(pane, is_active, &pane_cfg)
                    .with_palette(Some(pane_palette))
                    .with_surface_badge(self.wm.is_gui_surface())
                    .render(inner, buf);
            } else {
                CompositePaneWidget::with_config(pane, is_active, &pane_cfg)
                    .with_palette(Some(pane_palette))
                    .with_surface_badge(self.wm.is_gui_surface())
                    .render(outer, buf);
            }
        }

        // Render Navigator-style slide-out Sessions / Windows / Panes & Fleet drawer (DRW-01, DRW-02)
        // Scaled panels (GUI only) are drawn by the GUI renderer in their own grid.
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::Drawer) == 100 {
            render_workspace_drawer(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        drawer_preview::render_drawer_preview_overlay(
            self.wm,
            workspace_area,
            cfg,
            &palette,
            &glyphs,
            buf,
        );

        if qt_visible {
            let qt_outer = self.wm.quick_terminal().overlay_rect(workspace_area);
            if cfg.liquid_glass {
                visual_style::render_liquid_ambient_halo(buf, qt_outer, workspace_area, &palette);
            } else if cfg.drop_shadows && !cfg.is_modern() {
                render_drop_shadow_styled(buf, qt_outer, workspace_area, cfg.shadow_style, &palette);
            }
            occlude_rect(buf, qt_outer);
            if cfg.is_glass() {
                visual_style::apply_frosted_glass_surface(buf, qt_outer, palette.window_frame_bg, &palette);
            }
            let qt_title = if cfg.is_modern() {
                "● ● ●  Quick Terminal · ~ · Alt+`"
            } else {
                "[Q] QUAKE QUICK TERM [~] (Alt+` to hide)"
            };
            render_borland_frame_styled(
                qt_outer, qt_title, true,
                cfg.window_outline, &palette, &glyphs, buf,
            );
            if cfg.liquid_glass {
                visual_style::polish_glass_panel_rect(buf, qt_outer, palette.window_frame_bg, &palette, true);
            }
            let qt_inner = inset_window_frame(qt_outer);
            CompositePaneWidget::with_config(&self.wm.quick_terminal().pane, true, cfg)
                .with_surface_badge(self.wm.is_gui_surface())
                .render(qt_inner, buf);
        }

        if shell.top_bar_kind == Some(shell::TopBarKind::Pills)
            && let Some(bar) = shell.top_bar
        {
            navigator::render_navigator(self.wm, bar, &palette, buf);
        } else if let Some(menubar_rect) = shell.top_bar {
            render_top_menubar(menubar_rect, self.wm, &palette, buf);
        }
        if let Some((cat, sel_item)) = self.wm.menubar_state()
            && workspace_area.height >= 6
            && workspace_area.width >= 24
        {
            let hints = menu_model::dropdown_hints(self.wm, cat);
            render_menubar_dropdown(cat, sel_item, workspace_area, cfg, &palette, &glyphs, &hints, buf);
        }

        if let Some(fkey_rect) = shell.bottom_bar {
            render_dos_fkey_bar(fkey_rect, &palette, buf);
        }

        // Drawn below every workspace modal so a modal raised while the window is open (for example
        // the Exit Guard) is never hidden behind it (WR-07).
        // At a non-100 panel scale (GUI only) the GUI renderer draws the window itself in its own
        // grid; an unscaled copy here would be a second, offset window.
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::Agents) == 100 {
            agents_ui::render_agents_window(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::AgentsList) == 100 {
            agents_list_ui::render_agents_list(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::SessionMap) == 100 {
            session_map_ui::render_session_map(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        fonts_window_ui::render_fonts_window(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        daemon_window_ui::render_daemon_window(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        command_window_ui::render_command_window(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::Clipboard) == 100 {
            clipboard_window_ui::render_clipboard_window(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        if self.wm.scale_pct(crate::ui::typography::PanelTarget::Traffic) == 100 {
            traffic_monitor_ui::render_traffic_monitor(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }
        script_studio_ui::render_script_studio(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        agents_hud_ui::render_agents_hud(self.wm, workspace_area, cfg, &palette, &glyphs, buf);

        if self.wm.is_inspector_visible() && workspace_area.width >= 24 && workspace_area.height >= 6 {
            render_inspector_modal(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }

        if self.wm.is_help_visible() && workspace_area.width >= 20 && workspace_area.height >= 6 {
            render_help_modal(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }

        // Render interactive Fuzzy Command Palette modal on top of the workspace (v0.6.0)
        if self.wm.is_palette_visible() && workspace_area.width >= 24 && workspace_area.height >= 6 {
            render_palette_modal(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }

        if self.wm.is_find_visible()
            && workspace_area.width >= find_ui::FIND_MIN_AREA.0
            && workspace_area.height >= find_ui::FIND_MIN_AREA.1
        {
            find_ui::render_find_modal(self.wm, workspace_area, cfg, &palette, &glyphs, buf);
        }

        which_key_ui::render_which_key(self.wm, workspace_area, cfg, &palette, &glyphs, buf);

        if let Some(exit_state) = self.wm.exit_guard_state() {
            render_exit_guard_modal(buf, workspace_area, &palette, exit_state);
        }

        if self.wm.scale_pct(crate::ui::typography::PanelTarget::Mcp) == 100 {
            mcp_ui::render_mcp_console_overlay(self.wm, workspace_area, cfg, &palette, buf);
        }

        if self.wm.drawer().navigator_menu_open() || self.wm.context_menu_anchor().is_some() {
            crate::ui::drawer::navigator::render_standalone_context_menu(self.wm, area, buf);
        }

        if self.wm.rename_dialog_open() {
            rename_dialog_ui::render_rename_dialog(self.wm, area, buf);
        }

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
