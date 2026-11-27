//! `[ui]` section: pane-mail envelope and left-side attention indicator / TRON chime toggles.

use serde::{Deserialize, Serialize};

/// `[ui]` settings.
/// - `pane_mail_indicator` shows the envelope on a pane's title when another session, a tool
///   (RPC/MCP/CLI) or agent mail messages it; `pane_mail_flash` blinks it while unread.
/// - `attention_indicator` shows a gentle pulsing indicator on the left of any session or window
///   row in the drawer (and a compact left-edge beacon on the workspace when the drawer is closed)
///   when a built-in session or window needs attention.
/// - `attention_flash` pulses the left-side attention indicator at a calm ~0.55 Hz cadence
///   (`900 ms` lit / `900 ms` dim); when `false`, the indicator is rendered steady.
/// - `attention_sound` plays a 1970s TRON electronic synthesizer arpeggio chime when a session or
///   window newly requires attention (or on `:find-beep`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub pane_mail_indicator: bool,
    pub pane_mail_flash: bool,
    pub attention_indicator: bool,
    pub attention_flash: bool,
    pub attention_sound: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            pane_mail_indicator: true,
            pane_mail_flash: true,
            attention_indicator: true,
            attention_flash: true,
            attention_sound: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_on_and_parses_toml() {
        let d = UiConfig::default();
        assert!(d.pane_mail_indicator && d.pane_mail_flash);
        assert!(d.attention_indicator && d.attention_flash && d.attention_sound);
        let c: crate::config::BiltermConfig =
            toml::from_str("[ui]\npane_mail_flash = false\nattention_sound = false\n").unwrap();
        assert!(c.ui.pane_mail_indicator && !c.ui.pane_mail_flash);
        assert!(c.ui.attention_indicator && c.ui.attention_flash && !c.ui.attention_sound);
        let c: crate::config::BiltermConfig = toml::from_str("").unwrap();
        assert!(c.ui.pane_mail_indicator && c.ui.pane_mail_flash && c.ui.attention_indicator);
    }
}
