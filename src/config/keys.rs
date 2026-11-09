//! `[keys]` configuration table (`KEY-02`, Phase 63).
//!
//! ```toml
//! [keys]
//! profile = "auto"          # auto|mac|windows|linux|portable|legacy
//! kitty_protocol = "auto"   # auto|off
//! legacy_alt = "auto"       # auto|on|off
//! which_key = true
//! which_key_delay_ms = 400
//! unbind = ["Alt+V"]
//! [keys.bind]
//! "Primary+L" = ":layout cycle"
//! [keys.window_mode]
//! "g" = ":agents list toggle"
//! ```
//!
//! The legacy `leader_key`, `[keybindings]` and `macos_option_as_alt` fields stay valid; `keys.bind`
//! wins over `[keybindings]` when both bind a chord.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::BiltermError;
use crate::input::chord::MAX_CHORD_LEN;

/// Maximum entries in `[keys.bind]` / `[keys.window_mode]` (config input cap).
pub const MAX_BIND_ENTRIES: usize = 512;

/// Which shipped key profile is active.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyProfileName {
    /// Pick by host OS (`mac`, `windows` or `linux`).
    #[default]
    Auto,
    Mac,
    Windows,
    Linux,
    /// Leader + Window mode + F-keys only; no Alt or Primary chords.
    Portable,
    /// The pre-Phase-63 tables bit-for-bit (migration safety, headless default).
    Legacy,
}

impl KeyProfileName {
    /// Every selectable name in display order.
    pub const ALL: [Self; 6] =
        [Self::Auto, Self::Mac, Self::Windows, Self::Linux, Self::Portable, Self::Legacy];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mac => "mac",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Portable => "portable",
            Self::Legacy => "legacy",
        }
    }

    /// Parses a profile name (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.as_str().eq_ignore_ascii_case(s.trim()))
    }
}

/// Whether to push the kitty keyboard protocol on capable terminals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KittyProtocol {
    #[default]
    Auto,
    Off,
}

/// Whether the legacy `Alt+<printable>` layer stays active on Windows/Linux profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyAlt {
    /// `on` for one release, then `off`.
    #[default]
    Auto,
    On,
    Off,
}

/// The `[keys]` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeysConfig {
    pub profile: KeyProfileName,
    pub kitty_protocol: KittyProtocol,
    pub legacy_alt: LegacyAlt,
    pub which_key: bool,
    pub which_key_delay_ms: u32,
    /// `chord = ":command"` (same semantics as `[keybindings]`, but accepts `Primary`/`Cmd` tokens).
    #[serde(deserialize_with = "de_capped_map")]
    pub bind: BTreeMap<String, String>,
    /// `key = ":command"` inside Window mode.
    #[serde(deserialize_with = "de_capped_map")]
    pub window_mode: BTreeMap<String, String>,
    /// Shipped chords to remove from the active profile.
    pub unbind: Vec<String>,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            profile: KeyProfileName::Auto,
            kitty_protocol: KittyProtocol::Auto,
            legacy_alt: LegacyAlt::Auto,
            which_key: true,
            which_key_delay_ms: 400,
            bind: BTreeMap::new(),
            window_mode: BTreeMap::new(),
            unbind: Vec::new(),
        }
    }
}

fn check_map(name: &str, map: &BTreeMap<String, String>) -> Result<(), String> {
    if map.len() > MAX_BIND_ENTRIES {
        return Err(format!("[keys.{name}] has {} entries (maximum {MAX_BIND_ENTRIES})", map.len()));
    }
    if let Some(k) = map.keys().find(|k| k.chars().count() > MAX_CHORD_LEN) {
        return Err(format!("[keys.{name}] chord longer than {MAX_CHORD_LEN} characters: '{}'", k.chars().take(16).collect::<String>()));
    }
    Ok(())
}

fn de_capped_map<'de, D>(d: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let map = BTreeMap::<String, String>::deserialize(d)?;
    check_map("bind/window_mode", &map).map_err(serde::de::Error::custom)?;
    Ok(map)
}

impl KeysConfig {
    /// Validates entry-count and chord-length caps and that every chord parses.
    pub fn validate(&self) -> Result<(), BiltermError> {
        check_map("bind", &self.bind).map_err(BiltermError::Config)?;
        check_map("window_mode", &self.window_mode).map_err(BiltermError::Config)?;
        if self.unbind.len() > MAX_BIND_ENTRIES {
            return Err(BiltermError::Config(format!("[keys] unbind has {} entries (maximum {MAX_BIND_ENTRIES})", self.unbind.len())));
        }
        for chord in self.bind.keys().chain(self.unbind.iter()) {
            crate::input::chord::KeyChord::parse(chord).map_err(BiltermError::Config)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_toml_round_trip() {
        let d = KeysConfig::default();
        assert_eq!(d.profile, KeyProfileName::Auto);
        assert!(d.which_key);
        assert_eq!(d.which_key_delay_ms, 400);
        let cfg: KeysConfig = toml::from_str(
            "profile = \"mac\"\nlegacy_alt = \"off\"\nunbind = [\"Alt+V\"]\n[bind]\n\"Primary+L\" = \":layout cycle\"\n",
        )
        .unwrap();
        assert_eq!(cfg.profile, KeyProfileName::Mac);
        assert_eq!(cfg.legacy_alt, LegacyAlt::Off);
        assert_eq!(cfg.bind["Primary+L"], ":layout cycle");
        cfg.validate().unwrap();
        assert_eq!(KeyProfileName::parse("PORTABLE"), Some(KeyProfileName::Portable));
        assert_eq!(KeyProfileName::parse("nope"), None);
    }

    #[test]
    fn caps_reject_oversized_tables_and_chords() {
        let mut big = String::from("[bind]\n");
        for i in 0..=MAX_BIND_ENTRIES {
            big.push_str(&format!("\"Ctrl+Alt+F{}x{i}\" = \":help\"\n", i % 20));
        }
        assert!(toml::from_str::<KeysConfig>(&big).is_err(), "513 entries rejected at load");
        let long = format!("[bind]\n\"{}\" = \":help\"\n", "A".repeat(65));
        assert!(toml::from_str::<KeysConfig>(&long).is_err());
        let mut cfg = KeysConfig::default();
        cfg.bind.insert("Hyper+X".into(), ":help".into());
        assert!(cfg.validate().is_err());
    }
}
