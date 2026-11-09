//! `[traffic]` config section (Phase 64). Converts into the leaf layer's plain view, so
//! `crate::traffic` never imports `config`.

use serde::{Deserialize, Serialize};

use crate::traffic::TrafficConfigView;

/// Terminal traffic monitor settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrafficConfig {
    /// Count bytes at every I/O boundary (cheap; a disabled cell costs one relaxed load).
    pub enabled: bool,
    /// Classify escape vs control vs printable bytes even when the monitor window is closed.
    pub classify: bool,
    /// Show the `rx/tx` rate indicator in the OIA status bar.
    pub oia_indicator: bool,
    /// Show real ssh/tmux host names; `false` maps them to `host-<n>`.
    pub label_hosts: bool,
}

impl Default for TrafficConfig {
    fn default() -> Self {
        Self { enabled: true, classify: false, oia_indicator: false, label_hosts: true }
    }
}

impl TrafficConfig {
    pub fn to_view(&self) -> TrafficConfigView {
        TrafficConfigView {
            enabled: self.enabled,
            classify: self.classify,
            oia_indicator: self.oia_indicator,
            label_hosts: self.label_hosts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BiltermConfig;

    #[test]
    fn defaults_and_toml_round_trip() {
        let d = TrafficConfig::default();
        assert!(d.enabled && !d.classify && !d.oia_indicator && d.label_hosts);
        assert_eq!(BiltermConfig::default().traffic, d);
        let cfg: BiltermConfig = toml::from_str("[traffic]\nclassify = true\nlabel_hosts = false\n").unwrap();
        assert!(cfg.traffic.classify && !cfg.traffic.label_hosts && cfg.traffic.enabled);
        let s = toml::to_string(&cfg).unwrap();
        let back: BiltermConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.traffic, cfg.traffic);
        let none: BiltermConfig = toml::from_str("").unwrap();
        assert_eq!(none.traffic, d);
    }
}
