//! Modal overlays and Borland Turbo Vision top menubar renderers.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::wm::{WindowManager, inset_window_frame};

pub(super) fn render_palette_modal(
    wm: &WindowManager,
    canvas: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let pop_w = canvas.width.saturating_sub(4).clamp(24, 76).min(canvas.width);
    let pop_h = canvas.height.saturating_sub(2).clamp(6, 14).min(canvas.height);
    let pop_x = canvas.x + (canvas.width.saturating_sub(pop_w)) / 2;
    let pop_y = canvas.y + (canvas.height.saturating_sub(pop_h)) / 2;
    let pop_rect = Rect::new(pop_x, pop_y, pop_w, pop_h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, pop_rect, canvas, cfg.shadow_style, palette);
    }
    occlude_rect(buf, pop_rect);
    render_borland_frame_styled(
        pop_rect,
        "[P] COMMAND PALETTE [Enter=Run  Up/Down=Select  Esc=Close]",
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(pop_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let items = wm.filtered_palette_items();
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("Search > {}_", wm.palette_query),
        Style::default()
            .fg(Color::Rgb(255, 255, 85))
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    )));

    let list_rows = usize::from(inner.height.saturating_sub(1)).max(1);
    let sel = wm.palette_selected.min(items.len().saturating_sub(1));
    let start = sel.saturating_sub(list_rows.saturating_sub(1));
    for (idx, (cmd, key, desc)) in items.iter().enumerate().skip(start).take(list_rows) {
        let is_sel = idx == sel;
        let bg = if is_sel {
            palette.active_border_fg
        } else {
            palette.window_frame_bg
        };
        let fg = if is_sel { Color::Black } else { Color::White };
        lines.push(Line::from(Span::styled(
            format!(" {:<20} [{:<10}] {}", cmd, key, desc),
            Style::default().fg(fg).bg(bg),
        )));
    }
    Paragraph::new(lines).render(inner, buf);
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

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, pop_rect, canvas, cfg.shadow_style, palette);
    }
    occlude_rect(buf, pop_rect);
    let persist_tag = if pane.history_store.enabled {
        "PERSIST:ON"
    } else {
        "PERSIST:OFF"
    };
    render_borland_frame_styled(
        pop_rect,
        &format!(
            "[H] COMMAND HISTORY ({}) [Enter=Select  s=Save  Esc=Close]",
            persist_tag
        ),
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(pop_rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    let header = format!(
        "Session: {} | Pane #{} | Entries: {}",
        pane.history_store.session_name,
        pane.id,
        pane.prompt.history.len()
    );
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(Color::Rgb(85, 255, 255))
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    )));

    let list_rows = usize::from(inner.height.saturating_sub(1)).max(1);
    if pane.prompt.history.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No commands recorded in this pane yet)",
            Style::default()
                .fg(Color::Yellow)
                .bg(palette.window_frame_bg),
        )));
    } else {
        let sel = pane
            .prompt
            .history_popup_selected
            .min(pane.prompt.history.len() - 1);
        let start = sel.saturating_sub(list_rows.saturating_sub(1));
        for (idx, cmd) in pane
            .prompt
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

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, modal, canvas, cfg.shadow_style, palette);
    }
    occlude_rect(buf, modal);
    render_borland_frame_styled(
        modal,
        "[!] UNSAFE PASTE GUARD [y/Enter=Confirm  n/Esc=Cancel]",
        true,
        cfg.window_outline,
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
                .fg(Color::Rgb(255, 255, 85))
                .bg(palette.window_frame_bg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {preview}"),
            Style::default()
                .fg(Color::Rgb(85, 255, 255))
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
    scroll: usize,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
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
    let modal = Rect::new(x, y, w, h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, modal, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, modal);
    render_borland_frame_styled(
        modal,
        "[F1] BILTERM 3270 // TURBO VISION HELP (Up/Down/PgUp/PgDn, Esc/F1=Close)",
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    const HELP_LINES: &[(&str, &str)] = &[
        ("═══ LEADER PREFIX (Ctrl+X) & GLOBAL HOTKEYS ═══", ""),
        ("Ctrl+X <key>", "Universal Leader Prefix (works in Command Box & Raw PTY)"),
        ("Ctrl+X ? / F1 / :help", "Toggle this interactive Help reference window"),
        ("Ctrl+X - / F2 / Alt+H", "Split focused composite pane horizontally"),
        ("Ctrl+X | / Alt+V", "Split focused composite pane vertically"),
        ("Ctrl+X z / F5 / Alt+Z", "Toggle maximize/zoom on focused pane"),
        ("Ctrl+X o / F6 / Alt+Tab", "Shuffle pane deck top-to-bottom (cycle focus)"),
        ("Ctrl+X 1..9 / Alt+1..9", "Selectively raise & focus pane #1..#9"),
        ("Ctrl+X M / Alt+M", "Toggle Tiled Splits vs Z-Order Stacked Windows"),
        ("Ctrl+X q / Ctrl+\\ / F10", "Toggle persistent Guake Quick Terminal (macOS-safe)"),
        ("Ctrl+X e / F4 / Alt+E", "Cycle Floating Prompt -> Fixed Prompt -> Regular Terminal"),
        ("Ctrl+X g / F12 / Ctrl+G", "Toggle Command Box vs Regular Terminal (Raw PTY)"),
        ("Ctrl+X x / Alt+X", "Cycle IBM 3270 Cross-Hair (Pane -> Full -> Off)"),
        ("Alt+O / :spotlight", "Cycle Focal Spotlight (Circle/Ellipse × Fuzzy/Pixelated)"),
        ("Alt+Shift+O / :focalzoom", "Cycle Focal Zoom Lens (1.25x -> 1.50x -> 2.00x -> 3.00x -> Off)"),
        ("Alt+Shift+] / Alt+Shift+[", "Grow / shrink Focal Spotlight or Focal Zoom radius"),
        ("Ctrl+Q / :quit", "Cleanly exit Bilterm"),
        ("═══ COMMAND PROMPT, READLINE & HISTORY ═══", ""),
        ("Alt+Left/Right / Alt+B/F", "Move cursor word left / right (macOS Option+Arrows)"),
        ("Alt+Bksp / Alt+D / Ctrl+W", "Delete previous word / next word in command prompt"),
        ("Ctrl+X t/b / Alt+T/Shift+B", "Pin floating prompt to Top / Bottom of canvas"),
        ("Ctrl+X s / Alt+S", "Toggle Fixed (Non-Overlapping) vs Floating prompt"),
        ("Alt+Up / Alt+Down", "Float prompt vertically row-by-row"),
        ("Alt+Plus / Alt+Minus", "Increase / decrease command prompt height"),
        ("Alt+] / Alt+[", "Increase / decrease command prompt width"),
        ("Shift+PgUp / Shift+PgDn", "Scroll multi-line command prompt content"),
        ("Tab / Shift+Tab", "Tab-complete paths, commands, $ENV, and :commands"),
        ("Ctrl+R", "Fuzzy search command history (nucleo-matcher)"),
        ("Alt+P / :history", "Open Command History Pop-up Window (s = Save tmp)"),
        (":history save [path]", "Export session commands to a temporary script"),
        (":history on | off", "Enable or disable disk history persistence"),
        ("═══ SEARCH, COPY MODE & LINKS (v0.2.0) ═══", ""),
        ("Ctrl+X / / Alt+/", "Search OutputCanvas & highlight matches"),
        ("Alt+n / Alt+N", "Jump to next / previous canvas search match"),
        ("Ctrl+X [ / Alt+C / :copy", "Toggle Vi Copy Mode (hjkl, v char, V line, b block, y yank)"),
        ("Alt+LeftDrag", "Mouse rectangular block selection on OutputCanvas"),
        ("Ctrl+Click / :links", "Yank URL under mouse or last OSC 8/visible URL via OSC 52"),
        ("═══ 3270 INSPECTOR, FIELD HINTS & SESSIONS (v0.3.0) ═══", ""),
        ("Ctrl+X i / F9 / :inspector", "Toggle Live 3270 Terminal Inspector modal"),
        ("Ctrl+X f / F3 / Alt+Shift+F", "QuickSelect 3270 Field Hints (URLs, paths, SHAs, IPs)"),
        ("Ctrl+X l / Alt+L / :lock", "Toggle Read-Only Protected 3270 Field Lock (🔒 PROT)"),
        (":broadcast [on|off]", "Synchronized Multi-Pane Input Broadcast (📡 BCAST)"),
        (":session save|restore", "Save or restore named workspace layout & sessions"),
        (":menubar [on|off]", "Toggle interactive top Borland Turbo Vision Menu Bar"),
        ("═══ THEMES, BORDERS & SHADOWS ═══", ""),
        (":theme <name>", "turbo-blue | turbo-cyan | 3278 | 3279 | mono"),
        (":outline <style>", "double | solid | rounded | thick | dotted | dashed | quadruple | ascii"),
        (":shadows <style>", "on | off | translucent | tinted | solid"),
        (":glyphs <pack>", "unicode | cp437 | nerd-font | ascii"),
        (":cursor <style>", "block | hollow | underscore | bar"),
        (":crosshair full|pane|off", "Configure full-screen or pane-local cross-hairs"),
        (":spotlight <mode>", "circle | ellipse | pixel | fuzzy | size <rx> [ry] | + | - | off"),
    ];

    let max_scroll = HELP_LINES.len().saturating_sub(usize::from(inner.height));
    let start = scroll.min(max_scroll);
    let mut rendered = Vec::with_capacity(usize::from(inner.height));
    for &(lhs, rhs) in HELP_LINES.iter().skip(start).take(usize::from(inner.height)) {
        if rhs.is_empty() {
            rendered.push(Line::from(Span::styled(
                format!(" {lhs}"),
                Style::default()
                    .fg(Color::Rgb(255, 255, 85))
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            rendered.push(Line::from(vec![
                Span::styled(
                    format!(" {:<24}", lhs),
                    Style::default()
                        .fg(Color::Rgb(85, 255, 255))
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

const MENUBAR_CATEGORIES: &[(&str, &[(&str, &str)])] = &[
    (
        "≡ System",
        &[
            ("3270 Inspector (F9)", ":inspector"),
            ("Save Session", ":session save default"),
            ("Restore Session", ":session restore default"),
            ("Quit (Ctrl+Q)", ":quit"),
        ],
    ),
    (
        "Window",
        &[
            ("Split Horizontal (F2)", ":split h"),
            ("Split Vertical (Alt+V)", ":split v"),
            ("Zoom Toggle (F5)", ":zoom"),
            ("Stack Windows (Alt+M)", ":layout stack"),
        ],
    ),
    (
        "Prompt",
        &[
            ("Pin Top (Alt+T)", ":prompt top"),
            ("Pin Bottom (Alt+B)", ":prompt bottom"),
            ("Fixed / Float (Alt+S)", ":prompt split"),
            ("Cycle Float/Fix/Term (F4)", ":mode cycle"),
        ],
    ),
    (
        "View",
        &[
            ("3270 Field Hints (Alt+F)", ":hints"),
            ("Vi Copy Mode (Alt+C)", ":copy"),
            ("Cross-Hair Ruler (Alt+X)", ":crosshair"),
            ("DOS F-Key Bar", ":fkeybar"),
        ],
    ),
    (
        "Theme",
        &[
            ("Borland Turbo Blue", ":theme turbo-blue"),
            ("Borland Turbo Cyan", ":theme turbo-cyan"),
            ("IBM 3278 Phosphor Green", ":theme 3278"),
            ("IBM 3279 Amber CRT", ":theme 3279"),
        ],
    ),
    (
        "Tmux",
        &[
            ("Broadcast Input", ":broadcast"),
            ("Read-Only Lock (Alt+L)", ":lock"),
            ("Quake Terminal (F10)", ":quake"),
            ("Raw Passthrough (F12)", ":raw"),
        ],
    ),
    (
        "Help",
        &[
            ("Help Reference (F1)", ":help"),
            ("3270 Inspector (F9)", ":inspector"),
            ("List Sessions", ":session list"),
            ("Close Help", ":help"),
        ],
    ),
];

pub(super) fn render_top_menubar(
    rect: Rect,
    wm: &WindowManager,
    palette: &ThemePalette,
    buf: &mut Buffer,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let bar_bg = Color::Rgb(170, 170, 170);
    let mut spans = Vec::new();
    for (idx, &(title, _)) in MENUBAR_CATEGORIES.iter().enumerate() {
        let is_open = wm.menubar_open_category == Some(idx);
        let (fg, bg) = if is_open {
            (Color::White, Color::Rgb(0, 128, 0))
        } else {
            (Color::Black, bar_bg)
        };
        spans.push(Span::styled(
            format!(" {title} "),
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        ));
    }
    if wm.broadcast_input {
        spans.push(Span::styled(
            " 📡 BCAST ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 176, 0))
                .add_modifier(Modifier::BOLD),
        ));
    }
    Block::default()
        .style(Style::default().fg(Color::Black).bg(bar_bg))
        .render(rect, buf);
    let _ = palette;
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
    buf: &mut Buffer,
) {
    let cat_idx = category.min(MENUBAR_CATEGORIES.len() - 1);
    let (cat_title, items) = MENUBAR_CATEGORIES[cat_idx];
    let x_off = (cat_idx as u16) * 8;
    let w = 30u16.min(workspace_area.width);
    let h = (items.len() as u16 + 2).min(workspace_area.height);
    let x = (workspace_area.x + x_off)
        .min(workspace_area.x + workspace_area.width.saturating_sub(w));
    let y = workspace_area.y;
    let dropdown = Rect::new(x, y, w, h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, dropdown, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, dropdown);
    render_borland_frame_styled(
        dropdown,
        cat_title,
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(dropdown);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::with_capacity(items.len());
    for (idx, &(label, _)) in items.iter().enumerate() {
        let is_sel = idx == selected_item.min(items.len().saturating_sub(1));
        let (fg, bg) = if is_sel {
            (Color::Black, Color::Rgb(0, 255, 102))
        } else {
            (Color::White, palette.window_frame_bg)
        };
        let prefix = if is_sel { "► " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!(
                "{prefix}{label:<width$}",
                width = usize::from(inner.width.saturating_sub(2))
            ),
            Style::default().fg(fg).bg(bg).add_modifier(if is_sel {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        )));
    }
    Block::default()
        .style(Style::default().fg(Color::White).bg(palette.window_frame_bg))
        .render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

pub(super) fn render_inspector_modal(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
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
    let modal = Rect::new(x, y, w, h);

    if cfg.drop_shadows {
        render_drop_shadow_styled(buf, modal, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, modal);
    render_borland_frame_styled(
        modal,
        "[F9] 3270 TERMINAL INSPECTOR (Up/Down Scroll, F9/Esc=Close)",
        true,
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    let inner = inset_window_frame(modal);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let pane = wm.active_pane();
    let vt = &pane.vt;
    let (rows, cols) = vt.size();
    let (cur_r, cur_c) = vt.screen().cursor_position();
    let cell_opt = vt.screen().cell(cur_r, cur_c);
    let cell_grapheme = cell_opt
        .map(|c| {
            let s = c.contents();
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
                pane.id,
                pane.title,
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
                on_off(pane.read_only),
                on_off(wm.broadcast_input),
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
                on_off(pane.line_discipline.echo_enabled),
                on_off(pane.line_discipline.has_foreground_subcommand()),
                vt.osc133_phase(),
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
    let start = wm.inspector_scroll.min(max_scroll);
    let mut lines = Vec::with_capacity(usize::from(inner.height));
    for (k, v) in info_rows
        .into_iter()
        .skip(start)
        .take(usize::from(inner.height))
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<16} ", k),
                Style::default()
                    .fg(Color::Rgb(255, 255, 85))
                    .bg(palette.window_frame_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                v,
                Style::default()
                    .fg(Color::Rgb(85, 255, 255))
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
