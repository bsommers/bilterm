//! `:window theme|outline|bold|reset|show` per-window chrome overrides (`WindowManager` side).

use crate::ui::chrome::OutlineStyle;
use crate::ui::theme::ThemePreset;
use crate::wm::WindowManager;
use crate::wm::chrome_style::resolve_named_palette;

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
        let msg = match parts.first().copied().unwrap_or("show") {
            "global-bold" => {
                let next = parse_on_off(arg, self.config.window_outline_bold);
                self.config.window_outline_bold = next;
                format!("Window outlines bold: {}", if next { "ON" } else { "OFF" })
            }
            "theme" | "color" | "colors" => self.set_window_theme(&parts[1..].join(" ")),
            "outline" | "border" | "frame" => {
                let cfg_outline = self.config.window_outline;
                let chrome = self.active_pane_chrome_mut();
                chrome.outline = match arg.unwrap_or("next") {
                    "reset" | "default" | "global" => None,
                    "next" | "cycle" => Some(chrome.outline.unwrap_or(cfg_outline).next()),
                    name => OutlineStyle::from_name(name).or(chrome.outline),
                };
                format!(
                    "Window outline: {:?}",
                    chrome.outline.unwrap_or(cfg_outline)
                )
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
                format!("Window border bold: {}", if on { "ON" } else { "OFF" })
            }
            "reset" | "clear" => {
                let id = self.active_pane_id();
                self.pane_chrome.remove(&id);
                "Window style reset to global theme & outline".to_string()
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

    fn set_window_theme(&mut self, name: &str) -> String {
        let name = name.trim();
        match name {
            "reset" | "default" | "global" => {
                let chrome = self.active_pane_chrome_mut();
                chrome.theme_name = None;
                chrome.palette = None;
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
                format!("Window theme: {}", preset.label())
            }
            _ => match resolve_named_palette(name) {
                Some((label, palette)) => {
                    let chrome = self.active_pane_chrome_mut();
                    chrome.theme_name = Some(label.clone());
                    chrome.palette = Some(palette);
                    format!("Window theme: {label}")
                }
                None => {
                    let msg = format!("Unknown theme '{name}' (try :theme list)");
                    self.record_automation_error_msg(msg.clone());
                    msg
                }
            },
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
}
