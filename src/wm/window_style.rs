//! `:window` per-window theme browser, individual element color overrides, outline & bold border commands.

use crate::ui::chrome::OutlineStyle;
use crate::ui::cursor_cmd::parse_cursor_color;
use crate::ui::file_browser::{
    ThemeImportTarget, default_browser_start_dir, expand_user_path, is_theme_file_path,
    persist_imported_theme_file, register_theme_source_dir, resolve_theme_import_target,
};
use crate::ui::theme::ThemePreset;
use crate::ui::theme_import::{
    discover_candidate_theme_files, load_open_theme_file, resolve_theme_file_or_name,
};
use crate::wm::WindowManager;
use crate::wm::chrome_style::{
    WINDOW_COLOR_ELEMENTS, canonical_window_color_element, resolve_named_palette,
};

const QUICK_COLOR_SWATCHES: &[(&str, &str)] = &[
    ("green", "#00FF66 (3278 Phosphor Green)"),
    ("amber", "#FFB000 (3279 Gas-Plasma Amber)"),
    ("cyan", "#00F0FF (Borland Turbo Cyan)"),
    ("yellow", "#FFE600 (Borland Turbo Yellow)"),
    ("crimson", "#FF2A2A (TRON Crimson Red)"),
    ("magenta", "#FF40C0 (Cyberpunk Neon Magenta)"),
    ("purple", "#BD93F9 (Dracula Purple)"),
    ("blue", "#55AAFF (Electric Sky Blue)"),
    ("navy", "#001C58 (Borland Royal Navy)"),
    ("orange", "#FF8C00 (Solar Orange)"),
    ("lime", "#A6E3A1 (Catppuccin Mint/Lime)"),
    ("teal", "#14C8B4 (Solarized Teal)"),
    ("white", "#FFFFFF (High-Contrast White)"),
    ("gray", "#AAB2BE (Slate Silver Gray)"),
    ("black", "#000000 (Deep CRT Black)"),
];

fn parse_on_off(arg: Option<&str>, current: bool) -> bool {
    match arg {
        Some("on" | "true") => true,
        Some("off" | "false") => false,
        _ => !current,
    }
}

impl WindowManager {
    /// Executes a normalized `:window ...` / `:outline bold ...` subcommand on the focused window.
    pub fn execute_window_style_command(&mut self, subcmd: &str) {
        let parts: Vec<&str> = subcmd.split_whitespace().collect();
        let arg = parts.get(1).copied();
        let msg = match parts.first().copied().unwrap_or("menu") {
            "global-bold" => {
                let next = parse_on_off(arg, self.config.window_outline_bold);
                self.config.window_outline_bold = next;
                format!("Window outlines bold: {}", if next { "ON" } else { "OFF" })
            }
            "menu" | "list" | "ls" | "customize" => {
                self.open_window_style_customizer(arg);
                let id = self.active_pane_id();
                let desc = self.pane_chrome(id).describe(&self.config);
                format!(
                    "Window #{id} Customizer ({desc}) — ↑/↓/Enter or Click to apply per-window themes & element colors"
                )
            }
            "theme" | "themes" => self.set_window_theme(&parts[1..].join(" ")),
            "color" => {
                let elem = parts.get(1).copied().unwrap_or("font-color");
                let val = parts.get(2).copied().unwrap_or("reset");
                self.set_window_element_color(elem, val)
            }
            "outline" | "border" | "frame" => {
                let cfg_outline = self.config.window_outline;
                let chrome = self.active_pane_chrome_mut();
                chrome.outline = match arg.unwrap_or("next") {
                    "reset" | "default" | "global" => None,
                    "next" | "cycle" => Some(chrome.outline.unwrap_or(cfg_outline).next()),
                    name => OutlineStyle::from_name(name).or(chrome.outline),
                };
                let resolved = chrome.outline.unwrap_or(cfg_outline);
                self.refresh_window_customizer_if_open();
                format!("Window outline: {resolved:?}")
            }
            "bold" => {
                let global = self.config.window_outline_bold;
                let chrome = self.active_pane_chrome_mut();
                chrome.bold = if arg == Some("reset") {
                    None
                } else {
                    Some(parse_on_off(arg, chrome.bold.unwrap_or(global)))
                };
                let on = chrome.bold.unwrap_or(global);
                self.refresh_window_customizer_if_open();
                format!("Window border bold: {}", if on { "ON" } else { "OFF" })
            }
            "reset" | "clear" => {
                let id = self.active_pane_id();
                self.pane_chrome.remove(&id);
                self.refresh_window_customizer_if_open();
                "Window style reset to global theme, colors & outline".to_string()
            }
            sub if canonical_window_color_element(sub).is_some() => {
                if let Some(val) = arg {
                    self.set_window_element_color(sub, val)
                } else {
                    self.open_window_style_customizer(Some(sub));
                    format!("Window #{} {sub} color picker opened", self.active_pane_id())
                }
            }
            _ => {
                let desc = self
                    .pane_chrome(self.active_pane_id())
                    .describe(&self.config);
                format!("Window #{} style: {desc}", self.active_pane().id())
            }
        };
        self.active_pane_mut().set_status_message(Some(msg));
    }

    fn set_window_element_color(&mut self, raw_elem: &str, raw_val: &str) -> String {
        let Some(canon) = canonical_window_color_element(raw_elem) else {
            let msg = format!("Unknown window element '{raw_elem}'");
            self.record_automation_error_msg(msg.clone());
            return msg;
        };
        let val = raw_val.trim();
        let is_reset = matches!(
            val.to_ascii_lowercase().as_str(),
            "reset" | "default" | "global" | "theme" | "clear" | "none" | "auto"
        );
        let id = self.active_pane_id();
        if is_reset {
            let chrome = self.active_pane_chrome_mut();
            chrome.colors.remove(canon);
            if chrome.is_default() {
                self.pane_chrome.remove(&id);
            }
            self.refresh_window_customizer_if_open();
            return format!("Window #{id} {canon}: reset to theme default");
        }
        if parse_cursor_color(val).is_none() {
            let msg = format!("Unknown color '{val}' for :window {canon}");
            self.record_automation_error_msg(msg.clone());
            return msg;
        }
        self.active_pane_chrome_mut()
            .colors
            .insert(canon.to_string(), val.to_string());
        self.refresh_window_customizer_if_open();
        format!("Window #{id} {canon}: {val}")
    }

    fn set_window_theme(&mut self, raw_arg: &str) -> String {
        let name = raw_arg.trim();
        let parts: Vec<&str> = name.split_whitespace().collect();
        let first = parts.first().copied().unwrap_or("list");

        if name.is_empty()
            || matches!(
                first.to_ascii_lowercase().as_str(),
                "list" | "ls" | "menu" | "itermcolors" | "iterm2colors"
            )
        {
            self.open_window_theme_browser();
            let id = self.active_pane_id();
            return format!(
                "Window #{id} Theme Browser ({} presets + open schemes) — Enter/Click to apply to Window #{id}",
                ThemePreset::all().len()
            );
        }

        if matches!(
            first.to_ascii_lowercase().as_str(),
            "browse" | "files" | "dir" | "open" | "explorer"
        ) {
            return self.open_window_file_theme_browser(&parts[1..].join(" "));
        }

        if matches!(first.to_ascii_lowercase().as_str(), "import" | "load") {
            let sub_path = parts[1..].join(" ");
            let clean = sub_path.trim().trim_matches('"').trim_matches('\'');
            if clean.is_empty() {
                return self.open_window_file_theme_browser("");
            }
            return self.import_window_theme_target(clean);
        }

        match name {
            "reset" | "default" | "global" => {
                let id = self.active_pane_id();
                let chrome = self.active_pane_chrome_mut();
                chrome.theme_name = None;
                chrome.palette = None;
                if chrome.is_default() {
                    self.pane_chrome.remove(&id);
                }
                self.refresh_window_customizer_if_open();
                "Window theme reset to global theme".to_string()
            }
            "next" | "cycle" | "prev" => {
                let all = ThemePreset::all();
                let cur = self
                    .pane_chrome(self.active_pane_id())
                    .theme_name
                    .as_deref()
                    .and_then(|n| all.iter().position(|p| p.label() == n))
                    .or_else(|| all.iter().position(|&p| p == self.config.theme))
                    .unwrap_or(0);
                let idx = if name == "prev" {
                    (cur + all.len() - 1) % all.len()
                } else {
                    (cur + 1) % all.len()
                };
                let preset = all[idx];
                let chrome = self.active_pane_chrome_mut();
                chrome.theme_name = Some(preset.label().to_string());
                chrome.palette = Some(preset.palette());
                self.refresh_window_customizer_if_open();
                format!("Window theme: {}", preset.label())
            }
            _ => match resolve_named_palette(name) {
                Some((label, palette)) => {
                    let chrome = self.active_pane_chrome_mut();
                    chrome.theme_name = Some(label.clone());
                    chrome.palette = Some(palette);
                    self.refresh_window_customizer_if_open();
                    format!("Window theme: {label}")
                }
                None => {
                    let expanded = expand_user_path(name);
                    if expanded.is_dir() || name.contains('*') || name.contains('/') {
                        return self.import_window_theme_target(name);
                    }
                    let msg = format!("Unknown theme '{name}' (try :window theme list)");
                    self.record_automation_error_msg(msg.clone());
                    msg
                }
            },
        }
    }

    fn open_window_file_theme_browser(&mut self, raw_arg: &str) -> String {
        let id = self.active_pane_id();
        self.mcp_console.window_theme_target = Some(id);
        let expanded = expand_user_path(raw_arg);
        if !raw_arg.trim().is_empty() && expanded.is_dir() {
            self.mcp_console.open_file_browser(expanded.clone(), None);
            return format!(
                "Window #{id} Theme File Browser: {} (Enter/Click applies theme to Window #{id})",
                expanded.display()
            );
        }
        match resolve_theme_import_target(raw_arg) {
            ThemeImportTarget::OpenBrowser { dir, filter }
            | ThemeImportTarget::Directory {
                effective_dir: dir,
                filter,
                ..
            } => {
                self.mcp_console.open_file_browser(dir.clone(), filter);
                format!(
                    "Window #{id} Theme File Browser: {} (Enter/Click applies theme to Window #{id})",
                    dir.display()
                )
            }
            ThemeImportTarget::SingleFile(path) => {
                if path.is_file() && is_theme_file_path(&path) {
                    self.import_window_theme_target(&path.display().to_string())
                } else {
                    let dir = default_browser_start_dir();
                    self.mcp_console
                        .open_file_browser(dir.clone(), Some(raw_arg.to_string()));
                    format!("Window #{id} Theme File Browser: {}", dir.display())
                }
            }
        }
    }

    fn import_window_theme_target(&mut self, path_str: &str) -> String {
        let id = self.active_pane_id();
        self.mcp_console.window_theme_target = Some(id);
        match resolve_theme_import_target(path_str) {
            ThemeImportTarget::OpenBrowser { dir, filter } => {
                self.mcp_console.open_file_browser(dir.clone(), filter);
                format!("Window #{id} Theme File Browser: {}", dir.display())
            }
            ThemeImportTarget::Directory {
                effective_dir,
                files,
                filter,
            } => {
                register_theme_source_dir(&effective_dir);
                self.mcp_console
                    .open_file_browser(effective_dir.clone(), filter);
                if let Some((first_path, first_scheme)) = files
                    .iter()
                    .find_map(|p| load_open_theme_file(p).ok().map(|s| (p, s)))
                {
                    persist_imported_theme_file(first_path);
                    let label = first_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&first_scheme.name)
                        .to_string();
                    let chrome = self.active_pane_chrome_mut();
                    chrome.theme_name = Some(label.clone());
                    chrome.palette = Some(first_scheme.to_theme_palette());
                    format!("Window theme: {label} (imported from {})", effective_dir.display())
                } else {
                    format!("No themes found in {} (opened Window Theme Browser)", effective_dir.display())
                }
            }
            ThemeImportTarget::SingleFile(path) => {
                if let Ok(scheme) = load_open_theme_file(&path) {
                    persist_imported_theme_file(&path);
                    if let Some(parent) = path.parent() {
                        register_theme_source_dir(parent);
                    }
                    let label = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&scheme.name)
                        .to_string();
                    let chrome = self.active_pane_chrome_mut();
                    chrome.theme_name = Some(label.clone());
                    chrome.palette = Some(scheme.to_theme_palette());
                    self.refresh_window_customizer_if_open();
                    format!("Window theme: {label}")
                } else if let Some((resolved_path, scheme)) = resolve_theme_file_or_name(path_str) {
                    persist_imported_theme_file(&resolved_path);
                    let label = resolved_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&scheme.name)
                        .to_string();
                    let chrome = self.active_pane_chrome_mut();
                    chrome.theme_name = Some(label.clone());
                    chrome.palette = Some(scheme.to_theme_palette());
                    self.refresh_window_customizer_if_open();
                    format!("Window theme: {label}")
                } else {
                    let msg = format!("Unknown theme '{path_str}' (try :window theme list)");
                    self.record_automation_error_msg(msg.clone());
                    msg
                }
            }
        }
    }

    /// Opens the interactive Per-Window Theme Browser popup (`:window theme` / `:window theme list`).
    pub fn open_window_theme_browser(&mut self) {
        let id = self.active_pane_id();
        self.mcp_console.window_theme_target = Some(id);
        let chrome = self.pane_chrome(id);
        let active_win_theme = chrome.theme_name.as_deref();
        let mut lines = vec![
            format!(
                "◈ WINDOW #{id} THEME BROWSER | ACTIVE: {} (Global: {})",
                active_win_theme.unwrap_or("(following global)"),
                self.config.theme.label()
            ),
            "─".repeat(72),
            "PER-WINDOW THEME PRESETS (Enter/Click to apply ONLY to Window #".to_string()
                + &format!("{id}):"),
        ];
        for (idx, preset) in ThemePreset::all().iter().enumerate() {
            let badge = if active_win_theme.is_some_and(|n| n.eq_ignore_ascii_case(preset.label()))
            {
                " [ACTIVE ON WINDOW]"
            } else if active_win_theme.is_none() && *preset == self.config.theme {
                " [GLOBAL DEFAULT]"
            } else {
                ""
            };
            lines.push(format!(
                "  • window theme {} — {:2}. {}{badge}",
                preset.label().to_ascii_lowercase(),
                idx + 1,
                preset.label()
            ));
        }
        lines.push(
            "  • window theme browse — Open Interactive File & .itermcolors Browser for this Window"
                .to_string(),
        );
        lines.push("  • window theme next — Cycle to Next Per-Window Theme Preset".to_string());
        lines.push(
            "  • window theme reset — Reset Window Theme to Follow Global Theme".to_string(),
        );
        lines.push(
            "  • window menu — Open Individual Window Element Color Customizer (Font/Cursor/Frame/Controls/Status)"
                .to_string(),
        );

        let discovered = discover_candidate_theme_files(None);
        if !discovered.is_empty() {
            lines.push("─".repeat(72));
            lines.push(format!(
                "DISCOVERED .ITERMCOLORS & OPEN SCHEMES FOR WINDOW #{id} ({} found):",
                discovered.len()
            ));
            for (path, scheme) in discovered {
                lines.push(format!(
                    "  • window theme import {} — {}",
                    path.display(),
                    scheme.name
                ));
            }
        }
        self.open_mcp_popup_window(
            &format!("WINDOW #{id} THEME BROWSER (PER-WINDOW)"),
            lines,
        );
    }

    /// Opens the interactive Per-Window Style & Individual Element Color Customizer (`:window` / `:window menu [element]`).
    pub fn open_window_style_customizer(&mut self, focus_element: Option<&str>) {
        let id = self.active_pane_id();
        self.mcp_console.window_theme_target = Some(id);
        let chrome = self.pane_chrome(id);
        let focus_canon = focus_element.and_then(canonical_window_color_element);

        let mut lines = vec![
            format!(
                "◈ WINDOW #{id} STYLE & ELEMENT CUSTOMIZER | {}",
                chrome.describe(&self.config)
            ),
            "─".repeat(72),
        ];

        if let Some(elem) = focus_canon {
            let current = chrome.colors.get(elem).map(String::as_str).unwrap_or("theme-default");
            lines.push(format!(
                "🎨 PICK COLOR FOR WINDOW #{id} '{elem}' (Current: {current}) — Enter/Click to apply:"
            ));
            for &(swatch, desc) in QUICK_COLOR_SWATCHES {
                lines.push(format!("  • window {elem} {swatch} — {desc}"));
            }
            lines.push(format!(
                "  • window {elem} reset — Reset '{elem}' to Theme Default"
            ));
            lines.push("  • window menu — ⬅ Back to All Window Elements & Theme Options".to_string());
            self.open_mcp_popup_window(
                &format!("WINDOW #{id} COLOR PICKER // {elem}"),
                lines,
            );
            return;
        }

        lines.push("1. PER-WINDOW THEME & OUTLINE:".to_string());
        lines.push(
            "  • window theme list — Browse All 14 Built-in Themes & .itermcolors for This Window"
                .to_string(),
        );
        lines.push(
            "  • window theme browse — Open File & Directory Theme Browser for This Window"
                .to_string(),
        );
        lines.push("  • window theme next — Cycle Per-Window Theme Preset".to_string());
        lines.push(
            "  • window outline next — Cycle Per-Window Frame Outline (Double/Solid/Rounded/Thick/Dotted/Dashed/Ascii)"
                .to_string(),
        );
        lines.push("  • window bold toggle — Toggle Per-Window Bold Border Frame".to_string());
        lines.push("─".repeat(72));
        lines.push(
            "2. INDIVIDUAL THEME ELEMENT COLORS (Enter/Click element to pick color, or :window <elem> <#RRGGBB|name>):"
                .to_string(),
        );
        for &(elem, desc) in WINDOW_COLOR_ELEMENTS {
            let cur = chrome
                .colors
                .get(elem)
                .map(|v| format!(" [ACTIVE: {v}]"))
                .unwrap_or_default();
            lines.push(format!("  • window {elem} — {desc}{cur}"));
        }
        lines.push("─".repeat(72));
        lines.push("3. QUICK PER-WINDOW ELEMENT PRESETS:".to_string());
        lines.push("  • window font-color green — Phosphor Green Terminal & Prompt Text (#00FF66)".to_string());
        lines.push("  • window font-color amber — Gas-Plasma Amber Terminal & Prompt Text (#FFB000)".to_string());
        lines.push("  • window cursor-color crimson — TRON Crimson Cursor in This Window (#FF2A2A)".to_string());
        lines.push("  • window frame-color cyan — Turbo Cyan Active Window Frame (#00F0FF)".to_string());
        lines.push("  • window controls-color yellow — Turbo Yellow [_][□][X] & ◢ Controls (#FFE600)".to_string());
        lines.push("  • window status-color navy — Royal Navy 3270-OIA Status Bar (#001C58)".to_string());
        lines.push("  • window reset — Reset All Per-Window Theme, Outline & Element Overrides".to_string());

        self.open_mcp_popup_window(
            &format!("WINDOW #{id} STYLE & ELEMENT CUSTOMIZER"),
            lines,
        );
    }

    fn refresh_window_customizer_if_open(&mut self) {
        if !self.mcp_console.active {
            return;
        }
        let prev_sel = self.mcp_console.selected_row;
        let prev_scrl = self.mcp_console.scroll_offset;
        let title = self.mcp_console.title.clone();
        if title.contains("THEME BROWSER (PER-WINDOW)") {
            self.open_window_theme_browser();
            self.mcp_console.selected_row =
                prev_sel.min(self.mcp_console.view_lines.len().saturating_sub(1));
            self.mcp_console.scroll_offset = prev_scrl;
        } else if let Some((_, elem)) = title.split_once("COLOR PICKER // ") {
            let elem_owned = elem.trim().to_string();
            self.open_window_style_customizer(Some(&elem_owned));
            self.mcp_console.selected_row =
                prev_sel.min(self.mcp_console.view_lines.len().saturating_sub(1));
            self.mcp_console.scroll_offset = prev_scrl;
        } else if title.contains("STYLE & ELEMENT CUSTOMIZER") {
            self.open_window_style_customizer(None);
            self.mcp_console.selected_row =
                prev_sel.min(self.mcp_console.view_lines.len().saturating_sub(1));
            self.mcp_console.scroll_offset = prev_scrl;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
    use crate::ui::chrome::OutlineStyle;
    use crate::wm::WindowManager;

    fn run(wm: &mut WindowManager, cmd: &str) {
        execute_wm_colon_command(wm, parse_colon_command(cmd).unwrap().unwrap());
    }

    #[test]
    fn per_window_theme_outline_and_bold_are_independent() {
        let mut wm = WindowManager::new_headless(24, 80);
        let first = wm.active_pane_id();
        run(&mut wm, ":window theme dracula");
        run(&mut wm, ":window outline dashed");
        run(&mut wm, ":window bold on");
        assert_eq!(
            wm.pane_chrome(wm.active_pane_id()).outline,
            Some(OutlineStyle::Dashed)
        );
        assert_eq!(wm.pane_chrome(wm.active_pane_id()).bold, Some(true));
        assert!(wm.pane_chrome(wm.active_pane_id()).palette.is_some());

        run(&mut wm, ":split v");
        assert_ne!(wm.active_pane_id(), first);
        assert!(wm.pane_chrome(wm.active_pane_id()).is_default());

        run(&mut wm, ":outline pane thin");
        assert_eq!(
            wm.pane_chrome(wm.active_pane_id()).outline,
            Some(OutlineStyle::Solid)
        );
        run(&mut wm, ":outline bold on");
        assert!(wm.config().window_outline_bold);
        run(&mut wm, ":window reset");
        assert!(wm.pane_chrome(wm.active_pane_id()).is_default());
        assert!(
            parse_colon_command(":window outline bogus")
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn per_window_theme_browser_and_element_color_overrides_work_independently() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::Color;
        use ratatui::widgets::Widget;

        let mut wm = WindowManager::new_headless(24, 80);
        let first = wm.active_pane_id();

        // 1. `:window theme` / `:window theme list` opens the Per-Window Theme Browser
        run(&mut wm, ":window theme list");
        assert!(wm.mcp_console().active);
        assert!(wm.mcp_console().title.contains("THEME BROWSER (PER-WINDOW)"));
        assert_eq!(wm.mcp_console().window_theme_target, Some(first));

        // Selecting a row in the Per-Window Theme Browser applies that theme ONLY to `first`
        let dracula_row = wm
            .mcp_console()
            .view_lines
            .iter()
            .position(|l| l.contains("window theme dracula"))
            .expect("dracula row in window theme browser");
        wm.mcp_console_mut().select_row_at(dracula_row);
        assert!(wm.activate_selected_mcp_row());
        assert_eq!(
            wm.pane_chrome(first).theme_name.as_deref(),
            Some("DRACULA")
        );

        // 2. `:window` / `:window menu` opens the Per-Window Style & Element Customizer
        run(&mut wm, ":window");
        assert!(wm.mcp_console().title.contains("STYLE & ELEMENT CUSTOMIZER"));

        // 3. Individual element color overrides (hyphenated & two-word forms)
        run(&mut wm, ":window font-color #00ff66");
        run(&mut wm, ":window cursor color crimson");
        run(&mut wm, ":window frame-color cyan");
        run(&mut wm, ":window controls color yellow");
        run(&mut wm, ":window status-color navy");
        run(&mut wm, ":window title-color gold");
        run(&mut wm, ":window prompt-color amber");
        run(&mut wm, ":window bg #101420");
        run(&mut wm, ":window badge-color purple");

        let pal1 = wm.pane_chrome(first).resolve_palette(wm.config());
        assert_eq!(pal1.text_fg, Some(Color::Rgb(0, 255, 102)));
        assert_eq!(pal1.cursor_fg, Some(Color::Rgb(255, 42, 42)));
        assert_eq!(pal1.active_border_fg, Color::Rgb(0, 240, 255));
        assert_eq!(pal1.controls_fg, Some(Color::Rgb(255, 230, 0)));
        assert_eq!(pal1.oia_bg, Color::Rgb(0, 28, 88));
        assert_eq!(pal1.title_fg, Color::Rgb(255, 230, 0));
        assert_eq!(pal1.prompt_prefix_fg, Color::Rgb(255, 176, 0));
        assert_eq!(pal1.window_frame_bg, Color::Rgb(16, 20, 32));
        assert_eq!(pal1.oia_badge_bg, Color::Rgb(189, 147, 249));

        // 4. Split a second window and verify it remains on the global theme & colors
        run(&mut wm, ":split v");
        let second = wm.active_pane_id();
        assert_ne!(second, first);
        assert!(wm.pane_chrome(second).is_default());
        let pal2 = wm.pane_chrome(second).resolve_palette(wm.config());
        assert_eq!(pal2.text_fg, None);
        assert_eq!(pal2.controls_fg, None);
        assert_eq!(pal2.cursor_fg, None);

        // 5. Render workspace and verify no panic + per-element reset works
        wm.focus_and_raise(first);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        crate::ui::WorkspaceWidget::new(&wm).render(buf.area, &mut buf);

        run(&mut wm, ":window font-color reset");
        assert_eq!(wm.pane_chrome(first).resolve_palette(wm.config()).text_fg, None);
        assert!(wm.pane_chrome(first).resolve_palette(wm.config()).controls_fg.is_some());

        run(&mut wm, ":window reset");
        assert!(wm.pane_chrome(first).is_default());
    }
}
