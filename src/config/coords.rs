//! `[coords]` config section (Phase 71): default window-title coordinate badge style.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::wm::coordinates::CoordDisplayStyle;

/// Hierarchical coordinate badge settings. `badge = "off|compact|verbose|daemon"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoordsConfig {
    #[serde(serialize_with = "ser_style", deserialize_with = "de_style")]
    pub badge: CoordDisplayStyle,
}

fn ser_style<S: Serializer>(s: &CoordDisplayStyle, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(s.name())
}

fn de_style<'de, D: Deserializer<'de>>(de: D) -> Result<CoordDisplayStyle, D::Error> {
    let raw = String::deserialize(de)?;
    CoordDisplayStyle::from_name(&raw).ok_or_else(|| {
        serde::de::Error::custom(format!(
            "invalid [coords] badge '{raw}' (expected off|compact|verbose|daemon)"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BiltermConfig;

    #[test]
    fn default_round_trip_and_invalid() {
        assert_eq!(BiltermConfig::default().coords.badge, CoordDisplayStyle::Off);
        let cfg: BiltermConfig = toml::from_str("[coords]\nbadge = \"compact\"\n").unwrap();
        assert_eq!(cfg.coords.badge, CoordDisplayStyle::Compact);
        let back: BiltermConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back.coords, cfg.coords);
        let err = toml::from_str::<BiltermConfig>("[coords]\nbadge = \"bogus\"\n").unwrap_err();
        assert!(err.to_string().contains("invalid [coords] badge"));
    }
}
