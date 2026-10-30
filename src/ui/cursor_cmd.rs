//! Cursor shape, custom character, blink mode, and color customization (`UI-05`).
//!
//! Supports `:cursor` commands:
//! - Shapes: `block`, `hollow` (`hollow-block`), `underline` (`underscore`), `bar` (`beam`), `custom <char>` (`char <glyph>`)
//! - Blink modes: `blink` (`slow`), `solid` (`steady` / `noblink`), `rapid` (`fast`), `auto`
//! - Colors: `color <hex|name|theme>`, `fg <hex|name>`, or direct `<#RRGGBB|amber|green|cyan|yellow|crimson|magenta|purple|orange|white>`
//! - Compound: e.g. `:cursor hollow blink amber`, `:cursor custom ◆ solid #00ff66`, `:cursor block cyan`

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use crate::config::BiltermConfig;
use crate::ui::cursor::{CaretStyle, CursorBlinkMode, render_caret_full};
use crate::ui::theme::{ThemePalette, parse_hex_color};

/// Parses a hex color (`#RRGGBB`, `RRGGBB`, `#RGB`) or named retro/phosphor cursor color.
pub fn parse_cursor_color(spec: &str) -> Option<Color> {
    let s = spec.trim().trim_matches('"').trim_matches('\'').trim();
    if s.is_empty() {
        return None;
    }
    if let Some(c) = parse_hex_color(s) {
        return Some(c);
    }
    // 3-digit hex shorthand `#RGB`
    let hex3 = s.strip_prefix('#').unwrap_or(s);
    if hex3.len() == 3 && hex3.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut chars = hex3.chars();
        let r = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&chars.next()?.to_string().repeat(2), 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }

    match s.to_ascii_lowercase().as_str() {
        "green" | "phosphor" | "phosphor-green" | "3278" => Some(Color::Rgb(0, 255, 102)),
        "amber" | "phosphor-amber" | "3279" => Some(Color::Rgb(255, 176, 0)),
        "cyan" | "turbo-cyan" | "aqua" => Some(Color::Rgb(0, 240, 255)),
        "yellow" | "turbo-yellow" | "gold" => Some(Color::Rgb(255, 230, 0)),
        "red" | "crimson" | "tron-red" => Some(Color::Rgb(255, 42, 42)),
        "magenta" | "pink" | "hot-pink" | "neon" => Some(Color::Rgb(255, 64, 192)),
        "purple" | "violet" => Some(Color::Rgb(189, 147, 249)),
        "blue" | "electric-blue" => Some(Color::Rgb(85, 170, 255)),
        "orange" => Some(Color::Rgb(255, 140, 0)),
        "lime" => Some(Color::Rgb(166, 227, 161)),
        "teal" => Some(Color::Rgb(20, 200, 180)),
        "white" | "mono" => Some(Color::Rgb(255, 255, 255)),
        "black" => Some(Color::Rgb(0, 0, 0)),
        "gray" | "grey" | "silver" => Some(Color::Rgb(170, 178, 190)),
        _ => None,
    }
}

/// Renders the software caret at `(cursor_x, cursor_y)` using all `BiltermConfig` cursor settings.
pub fn render_caret_configured(
    buf: &mut Buffer,
    cursor_x: u16,
    cursor_y: u16,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
) {
    if cfg.cursor_blink_hidden && cfg.cursor_blink.blink_interval(cfg.cursor_style).is_some() {
        return;
    }
    let custom_color = cfg.cursor_color.as_deref().and_then(parse_cursor_color);
    let custom_text = cfg
        .cursor_text_color
        .as_deref()
        .and_then(parse_cursor_color);
    render_caret_full(
        buf,
        cursor_x,
        cursor_y,
        cfg.cursor_style,
        cfg.cursor_blink,
        &cfg.cursor_custom_char,
        palette.active_border_fg,
        custom_color,
        custom_text,
    );
}

impl crate::wm::WindowManager {
    /// Advances the software cursor blink phase (`Blink` = 500ms, `RapidBlink` = 240ms) (`UI-05`).
    /// Returns `true` if the visible/hidden phase toggled and the screen should be redrawn.
    pub fn tick_cursor_blink(&mut self, now: std::time::Instant) -> bool {
        let Some(interval) = self
            .config()
            .cursor_blink
            .blink_interval(self.config().cursor_style)
        else {
            if self.config().cursor_blink_hidden {
                self.config_mut().cursor_blink_hidden = false;
                return true;
            }
            return false;
        };
        if now.duration_since(self.last_blink_toggle) >= interval {
            self.last_blink_toggle = now;
            let next = !self.config().cursor_blink_hidden;
            self.config_mut().cursor_blink_hidden = next;
            true
        } else {
            false
        }
    }

    /// Resets the cursor blink phase to visible (`ON`) when the user interacts or updates `:cursor`.
    pub fn reset_cursor_blink(&mut self) {
        self.last_blink_toggle = std::time::Instant::now();
        self.config_mut().cursor_blink_hidden = false;
    }
}

/// Builds interactive console lines for `:cursor` / `:cursor list`.
pub fn build_cursor_catalog_lines(cfg: &BiltermConfig) -> Vec<String> {
    let color_lbl = cfg.cursor_color.as_deref().unwrap_or("theme-default");
    let text_lbl = cfg.cursor_text_color.as_deref().unwrap_or("auto-contrast");
    let mut lines = vec![
        format!(
            "◈ ACTIVE CURSOR: STYLE={} | BLINK={} | COLOR={} | TEXT={} | CHAR='{}'",
            cfg.cursor_style.label(),
            cfg.cursor_blink.label(),
            color_lbl,
            text_lbl,
            cfg.cursor_custom_char
        ),
        "─".repeat(72),
        "CURSOR SHAPES (Enter/Click to apply):".to_string(),
        "  • cursor block — 1. Solid Block Cursor (█)".to_string(),
        "  • cursor hollow — 2. Hollow Block Cursor (▯)".to_string(),
        "  • cursor underline — 3. Underline / Underscore Cursor (_)".to_string(),
        "  • cursor bar — 4. Vertical Bar / Beam Cursor (▏)".to_string(),
        "CUSTOM CHARACTER CURSORS (or :cursor custom <char>):".to_string(),
        "  • cursor custom ◆ — Diamond Cursor (◆)".to_string(),
        "  • cursor custom ● — Circle / Dot Cursor (●)".to_string(),
        "  • cursor custom ▸ — Triangle Pointer Cursor (▸)".to_string(),
        "  • cursor custom ▓ — Dark Shade Block Cursor (▓)".to_string(),
        "  • cursor custom ▒ — Medium Shade Block Cursor (▒)".to_string(),
        "  • cursor custom ╬ — Crosshair Box Cursor (╬)".to_string(),
        "  • cursor custom ❯ — Chevron Prompt Cursor (❯)".to_string(),
        "BLINK vs SOLID MODES:".to_string(),
        "  • cursor solid — Steady / Non-Blinking Cursor".to_string(),
        "  • cursor blink — Standard Blinking Cursor (SLOW_BLINK)".to_string(),
        "  • cursor rapid — Fast Blinking Cursor (RAPID_BLINK)".to_string(),
        "CURSOR COLORS (or :cursor color #RRGGBB):".to_string(),
        "  • cursor color green — IBM 3278 Phosphor Green (#00FF66)".to_string(),
        "  • cursor color amber — IBM 3279 Phosphor Amber (#FFB000)".to_string(),
        "  • cursor color cyan — Borland Turbo Cyan (#00F0FF)".to_string(),
        "  • cursor color yellow — Borland Turbo Yellow (#FFE600)".to_string(),
        "  • cursor color crimson — TRON Crimson Red (#FF2A2A)".to_string(),
        "  • cursor color magenta — Neon Magenta (#FF40C0)".to_string(),
        "  • cursor color purple — Dracula Purple (#BD93F9)".to_string(),
        "  • cursor color orange — Solar Orange (#FF8C00)".to_string(),
        "  • cursor color white — Pure White (#FFFFFF)".to_string(),
        "  • cursor color theme — Use Active Theme Default Color".to_string(),
        "  • cursor reset — Reset All Cursor Settings to Defaults".to_string(),
    ];
    lines.push(
        "Combine options freely: e.g. :cursor hollow blink amber | :cursor custom ◆ solid #00ff66"
            .to_string(),
    );
    lines
}

/// Validates `:cursor [args...]` tokens at parse time (`UI-05`).
pub fn validate_cursor_command_args(tokens: &[&str]) -> Result<(), String> {
    if tokens.is_empty()
        || (tokens.len() == 1 && matches!(tokens[0].to_ascii_lowercase().as_str(), "list" | "ls"))
    {
        return Ok(());
    }
    let mut idx = 0usize;
    while idx < tokens.len() {
        let tok = tokens[idx];
        let lower = tok.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "reset" | "default" | "defaults" | "theme" | "theme-color"
        ) {
            idx += 1;
            continue;
        }
        if matches!(lower.as_str(), "custom" | "char" | "glyph" | "custom-char") {
            if let Some(&next_tok) = tokens.get(idx + 1)
                && CaretStyle::from_name(next_tok).is_none()
                && CursorBlinkMode::from_name(next_tok).is_none()
                && parse_cursor_color(next_tok).is_none()
                && !matches!(
                    next_tok.to_ascii_lowercase().as_str(),
                    "color" | "fg" | "bg" | "text" | "blink" | "solid"
                )
            {
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }
        if matches!(lower.as_str(), "blink" | "blinking") {
            if let Some(&next_tok) = tokens.get(idx + 1)
                && CursorBlinkMode::from_name(next_tok).is_some()
            {
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }
        if CursorBlinkMode::from_name(&lower).is_some()
            || CaretStyle::from_name(&lower).is_some()
            || parse_cursor_color(tok).is_some()
        {
            idx += 1;
            continue;
        }
        if matches!(
            lower.as_str(),
            "color" | "colour" | "bg" | "fg" | "text" | "text-color"
        ) {
            let Some(&next_tok) = tokens.get(idx + 1) else {
                return Err("Usage: :cursor color <#RRGGBB|green|amber|cyan|yellow|crimson|magenta|purple|orange|white|theme>".to_string());
            };
            if matches!(
                next_tok.to_ascii_lowercase().as_str(),
                "theme" | "default" | "reset" | "none" | "auto"
            ) || parse_cursor_color(next_tok).is_some()
            {
                idx += 2;
                continue;
            }
            return Err(format!("Unknown cursor color '{next_tok}'"));
        }
        let clean_tok = tok.trim_matches('"').trim_matches('\'');
        if clean_tok.chars().count() == 1 {
            idx += 1;
            continue;
        }
        return Err(
            "Usage: :cursor [block | hollow | underline | bar | custom <char>] [blink | solid | rapid] [color <#hex|name|theme> | reset]"
                .to_string(),
        );
    }
    Ok(())
}

/// Executes `:cursor [args...]` commands (`UI-05`).
pub fn execute_cursor_command(wm: &mut crate::wm::WindowManager, raw_args: &str) {
    let trimmed = raw_args.trim();
    let body = trimmed
        .strip_prefix("cursor")
        .map(str::trim)
        .unwrap_or(trimmed);

    if body.is_empty() || body.eq_ignore_ascii_case("list") || body.eq_ignore_ascii_case("ls") {
        let lines = build_cursor_catalog_lines(wm.config());
        wm.open_mcp_popup_window("CURSOR CUSTOMIZATION (SHAPES, BLINK, COLORS, GLYPHS)", lines);
        let msg = format!(
            "Cursor: {} ({}, color={}) — Enter/Click row to apply",
            wm.config().cursor_style.label(),
            wm.config().cursor_blink.label(),
            wm.config()
                .cursor_color
                .as_deref()
                .unwrap_or("theme-default")
        );
        wm.active_pane_mut().set_status_message(Some(msg));
        return;
    }

    let tokens: Vec<&str> = body.split_whitespace().collect();
    let mut idx = 0usize;
    let mut changed = false;

    while idx < tokens.len() {
        let tok = tokens[idx];
        let lower = tok.to_ascii_lowercase();

        if matches!(lower.as_str(), "reset" | "default" | "defaults") {
            let cfg = wm.config_mut();
            cfg.cursor_style = CaretStyle::SolidBlock;
            cfg.cursor_blink = CursorBlinkMode::Auto;
            cfg.cursor_color = None;
            cfg.cursor_text_color = None;
            cfg.cursor_custom_char = "◆".to_string();
            changed = true;
            idx += 1;
            continue;
        }

        if matches!(lower.as_str(), "custom" | "char" | "glyph" | "custom-char") {
            wm.config_mut().cursor_style = CaretStyle::CustomChar;
            if let Some(&next_tok) = tokens.get(idx + 1)
                && CaretStyle::from_name(next_tok).is_none()
                && CursorBlinkMode::from_name(next_tok).is_none()
                && parse_cursor_color(next_tok).is_none()
                && !matches!(
                    next_tok.to_ascii_lowercase().as_str(),
                    "color" | "fg" | "bg" | "text" | "blink" | "solid"
                )
            {
                let ch = next_tok
                    .trim_matches('"')
                    .trim_matches('\'')
                    .chars()
                    .next()
                    .unwrap_or('◆');
                wm.config_mut().cursor_custom_char = ch.to_string();
                idx += 2;
            } else {
                idx += 1;
            }
            changed = true;
            continue;
        }

        if matches!(lower.as_str(), "blink" | "blinking") {
            if let Some(&next_tok) = tokens.get(idx + 1)
                && let Some(bm) = CursorBlinkMode::from_name(next_tok)
            {
                wm.config_mut().cursor_blink = bm;
                idx += 2;
            } else {
                wm.config_mut().cursor_blink = CursorBlinkMode::Blink;
                idx += 1;
            }
            changed = true;
            continue;
        }

        if matches!(
            lower.as_str(),
            "solid" | "steady" | "noblink" | "no-blink" | "rapid" | "fast" | "rapid-blink" | "slow" | "auto"
        ) && let Some(bm) = CursorBlinkMode::from_name(&lower)
        {
            wm.config_mut().cursor_blink = bm;
            changed = true;
            idx += 1;
            continue;
        }

        if let Some(style) = CaretStyle::from_name(&lower) {
            wm.config_mut().cursor_style = style;
            changed = true;
            idx += 1;
            continue;
        }

        if matches!(lower.as_str(), "color" | "colour" | "bg") {
            if let Some(&next_tok) = tokens.get(idx + 1) {
                if matches!(
                    next_tok.to_ascii_lowercase().as_str(),
                    "theme" | "default" | "reset" | "none" | "auto"
                ) {
                    wm.config_mut().cursor_color = None;
                    changed = true;
                    idx += 2;
                    continue;
                }
                if parse_cursor_color(next_tok).is_some() {
                    wm.config_mut().cursor_color = Some(next_tok.to_string());
                    changed = true;
                    idx += 2;
                    continue;
                }
            }
            wm.active_pane_mut().set_status_message(Some(
                "Usage: :cursor color <#RRGGBB | green | amber | cyan | yellow | crimson | magenta | purple | orange | white | theme>"
                    .to_string(),
            ));
            return;
        }

        if matches!(lower.as_str(), "fg" | "text" | "text-color") {
            if let Some(&next_tok) = tokens.get(idx + 1) {
                if matches!(
                    next_tok.to_ascii_lowercase().as_str(),
                    "auto" | "default" | "reset" | "none"
                ) {
                    wm.config_mut().cursor_text_color = None;
                    changed = true;
                    idx += 2;
                    continue;
                }
                if parse_cursor_color(next_tok).is_some() {
                    wm.config_mut().cursor_text_color = Some(next_tok.to_string());
                    changed = true;
                    idx += 2;
                    continue;
                }
            }
            wm.active_pane_mut().set_status_message(Some(
                "Usage: :cursor fg <#RRGGBB | black | white | auto>".to_string(),
            ));
            return;
        }

        if matches!(lower.as_str(), "theme" | "theme-color") {
            wm.config_mut().cursor_color = None;
            changed = true;
            idx += 1;
            continue;
        }

        if parse_cursor_color(tok).is_some() {
            wm.config_mut().cursor_color = Some(tok.to_string());
            changed = true;
            idx += 1;
            continue;
        }

        // Single character token (e.g. `:cursor ◆` or `:cursor ●` or `:cursor ▓`)
        let clean_tok = tok.trim_matches('"').trim_matches('\'');
        if clean_tok.chars().count() == 1 {
            wm.config_mut().cursor_style = CaretStyle::CustomChar;
            wm.config_mut().cursor_custom_char = clean_tok.to_string();
            changed = true;
            idx += 1;
            continue;
        }

        wm.active_pane_mut().set_status_message(Some(format!(
            "Unknown cursor option '{tok}'. Usage: :cursor [block|hollow|underline|bar|custom <char>] [blink|solid|rapid] [color <#hex|name|theme>|reset]"
        )));
        return;
    }

    if changed {
        wm.reset_cursor_blink();
        if wm.mcp_console().active && wm.mcp_console().title.contains("CURSOR") {
            let lines = build_cursor_catalog_lines(wm.config());
            wm.open_mcp_popup_window(
                "CURSOR CUSTOMIZATION (SHAPES, BLINK, COLORS, GLYPHS)",
                lines,
            );
        }
        let msg = {
            let cfg = wm.config();
            let style_lbl = if cfg.cursor_style == CaretStyle::CustomChar {
                format!("CUSTOM('{}')", cfg.cursor_custom_char)
            } else {
                cfg.cursor_style.label().to_string()
            };
            let blink_lbl = cfg.cursor_blink.label();
            let color_lbl = cfg.cursor_color.as_deref().unwrap_or("theme");
            format!("Cursor updated: style={style_lbl}, blink={blink_lbl}, color={color_lbl}")
        };
        wm.active_pane_mut().set_status_message(Some(msg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::cursor::{
        SpotlightMode, render_crosshair_cursor, render_focal_zoom_cursor, render_spotlight_cursor,
    };
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;

    #[test]
    fn crosshair_cursor_tints_full_row_and_column_non_destructively() {
        let area = Rect::new(0, 0, 10, 6);
        let mut buf = Buffer::empty(area);
        buf.cell_mut((2, 3)).unwrap().set_symbol("A");

        let tint = Color::Rgb(0, 51, 0);
        render_crosshair_cursor(&mut buf, area, 5, 3, tint);
        assert_eq!(buf.cell((2, 3)).unwrap().symbol(), "A");
        assert_eq!(buf.cell((2, 3)).unwrap().style().bg, Some(tint));
        assert_eq!(buf.cell((5, 1)).unwrap().style().bg, Some(tint));
    }

    #[test]
    fn spotlight_and_focal_zoom_modulate_cells() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        for y in 0..20 {
            for x in 0..40 {
                let c = buf.cell_mut((x, y)).unwrap();
                c.set_style(c.style().fg(Color::Rgb(0, 240, 100)).bg(Color::Rgb(0, 20, 40)));
            }
        }
        buf.cell_mut((20, 10)).unwrap().set_symbol("X");
        render_spotlight_cursor(
            &mut buf,
            area,
            20,
            10,
            SpotlightMode::Ellipse,
            12,
            5,
            45,
            false,
            Color::Rgb(0, 80, 40),
        );
        assert_eq!(
            buf.cell((20, 10)).unwrap().style().fg.unwrap(),
            Color::Rgb(0, 240, 100)
        );

        let mut buf_px = Buffer::empty(area);
        buf_px.cell_mut((20, 10)).unwrap().set_symbol("#");
        render_focal_zoom_cursor(
            &mut buf_px,
            area,
            20,
            10,
            SpotlightMode::Circle,
            200,
            10,
            5,
            50,
            true,
            Color::Rgb(0, 60, 80),
        );
        assert_eq!(buf_px.cell((20, 10)).unwrap().symbol(), "#");
        assert_eq!(buf_px.cell((21, 10)).unwrap().symbol(), "#");
    }

    #[test]
    fn cursor_styles_custom_char_blink_and_colors_render_and_execute() {
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        let mut cfg = BiltermConfig::default();
        let pal = cfg.palette();

        // 1. Custom character + rapid blink + amber color
        cfg.cursor_style = CaretStyle::CustomChar;
        cfg.cursor_custom_char = "◆".to_string();
        cfg.cursor_blink = CursorBlinkMode::RapidBlink;
        cfg.cursor_color = Some("amber".to_string());
        render_caret_configured(&mut buf, 3, 1, &cfg, &pal);
        let cell = buf.cell((3, 1)).unwrap();
        assert_eq!(cell.symbol(), "◆");
        assert_eq!(cell.style().fg, Some(Color::Rgb(255, 176, 0)));
        assert!(cell.style().add_modifier.contains(Modifier::RAPID_BLINK));

        // 2. Hollow block forced solid + custom hex color
        cfg.cursor_style = CaretStyle::BlinkingHollowBlock;
        cfg.cursor_blink = CursorBlinkMode::Solid;
        cfg.cursor_color = Some("#00f0ff".to_string());
        render_caret_configured(&mut buf, 4, 1, &cfg, &pal);
        let cell_h = buf.cell((4, 1)).unwrap();
        assert_eq!(cell_h.symbol(), "▯");
        assert_eq!(cell_h.style().fg, Some(Color::Rgb(0, 240, 255)));
        assert!(!cell_h.style().add_modifier.contains(Modifier::SLOW_BLINK));

        // 3. Solid block with custom color and blink
        cfg.cursor_style = CaretStyle::SolidBlock;
        cfg.cursor_blink = CursorBlinkMode::Blink;
        cfg.cursor_color = Some("green".to_string());
        render_caret_configured(&mut buf, 5, 1, &cfg, &pal);
        let cell_b = buf.cell((5, 1)).unwrap();
        assert_eq!(cell_b.style().bg, Some(Color::Rgb(0, 255, 102)));
        assert_eq!(cell_b.style().fg, Some(Color::Rgb(0, 0, 0)));
        assert!(cell_b.style().add_modifier.contains(Modifier::SLOW_BLINK));

        // 4. Software blink hidden phase leaves underlying cell untouched
        cfg.cursor_blink_hidden = true;
        buf.cell_mut((6, 1)).unwrap().set_symbol("A");
        render_caret_configured(&mut buf, 6, 1, &cfg, &pal);
        assert_eq!(buf.cell((6, 1)).unwrap().symbol(), "A");
        assert_ne!(buf.cell((6, 1)).unwrap().style().bg, Some(Color::Rgb(0, 255, 102)));
    }

    #[test]
    fn software_cursor_blink_timer_toggles_and_menubar_dropdown_mouse_works() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use std::time::{Duration, Instant};

        let mut wm = crate::wm::WindowManager::new_headless(24, 80);
        wm.config_mut().cursor_blink = CursorBlinkMode::Blink;
        let t0 = Instant::now();
        wm.last_blink_toggle = t0;
        assert!(!wm.config().cursor_blink_hidden);
        assert!(!wm.tick_cursor_blink(t0 + Duration::from_millis(200)));
        assert!(wm.tick_cursor_blink(t0 + Duration::from_millis(550)));
        assert!(wm.config().cursor_blink_hidden);
        assert!(wm.tick_cursor_blink(t0 + Duration::from_millis(1100)));
        assert!(!wm.config().cursor_blink_hidden);

        // Enable top Borland menu bar and click "Window" category (cols 10..18)
        wm.config_mut().show_top_menubar = true;
        let vp = Rect::new(0, 0, 80, 24);
        wm.resize_all(vp).unwrap();
        assert_eq!(crate::ui::modals::menubar_category_at_col(5), Some(0)); // ≡ System
        assert_eq!(crate::ui::modals::menubar_category_at_col(12), Some(1)); // Window
        assert_eq!(crate::ui::modals::menubar_category_at_col(20), Some(2)); // Prompt
        assert_eq!(crate::ui::modals::menubar_category_at_col(28), Some(3)); // View
        assert_eq!(crate::ui::modals::menubar_category_at_col(35), Some(4)); // Theme
        assert_eq!(crate::ui::modals::menubar_category_at_col(41), Some(5)); // Tmux
        assert_eq!(crate::ui::modals::menubar_category_at_col(47), Some(6)); // Help

        crate::input::handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 12,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            vp,
        )
        .unwrap();
        assert_eq!(wm.menubar_state(), Some((1, 0)));

        // Hover over item 1 ("Split Vertical (Alt+V)") inside the Window dropdown
        let ws_area = Rect::new(0, 1, 80, 23);
        let dd = crate::ui::modals::menubar_dropdown_rect(1, ws_area);
        crate::input::handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Moved,
                column: dd.x + 3,
                row: dd.y + 2,
                modifiers: KeyModifiers::NONE,
            },
            vp,
        )
        .unwrap();
        assert_eq!(wm.menubar_state(), Some((1, 1)));

        // Click item 1 ("Split Vertical") inside the Window dropdown -> splits pane and closes dropdown!
        assert_eq!(wm.pane_count(), 1);
        crate::input::handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: dd.x + 3,
                row: dd.y + 2,
                modifiers: KeyModifiers::NONE,
            },
            vp,
        )
        .unwrap();
        assert_eq!(wm.menubar_state(), None);
        assert_eq!(wm.pane_count(), 2);
    }
}
