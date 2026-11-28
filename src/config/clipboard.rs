//! `[clipboard.images]` config section: image paste delivery and resource budgets.

use serde::{Deserialize, Serialize};

use crate::clipboard_image::{ImageDelivery, ImageLimits, ImagePasteSettings};

const MIB: u64 = 1024 * 1024;

/// `[clipboard]` table (only `images` today).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipboardConfig {
    pub images: ClipboardImagesConfig,
}

/// Image paste settings. Budgets are clamped to hard ceilings in [`Self::settings`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipboardImagesConfig {
    /// Master switch; `false` makes `Ctrl+V` behave as before and `:paste image` refuse.
    pub enabled: bool,
    /// `path` (default: save a private file and paste its path) or `passthrough` (raw `Ctrl+V`).
    pub delivery: ImageDelivery,
    /// Largest single image read from the clipboard, in MiB (default 10, max 64).
    pub max_image_mib: u64,
    /// Read-buffer budget across concurrent pastes, in MiB (default 32, max 256).
    pub max_memory_mib: u64,
    /// Stored-image budget on disk, in MiB (default 256, max 4096).
    pub max_disk_mib: u64,
    /// Clipboard tool timeout in milliseconds (default 3000, 200..=30000).
    pub read_timeout_ms: u64,
}

impl Default for ClipboardImagesConfig {
    fn default() -> Self {
        Self { enabled: true, delivery: ImageDelivery::Path, max_image_mib: 10, max_memory_mib: 32, max_disk_mib: 256, read_timeout_ms: 3000 }
    }
}

impl ClipboardImagesConfig {
    /// The runtime policy handed to panes, with every budget clamped to a sane range.
    pub fn settings(&self) -> ImagePasteSettings {
        let d = ImageLimits::default();
        let image = self.max_image_mib.clamp(1, 64) * MIB;
        let memory = (self.max_memory_mib.clamp(1, 256) * MIB).max(image);
        let disk = (self.max_disk_mib.clamp(1, 4096) * MIB).max(image);
        ImagePasteSettings {
            enabled: self.enabled,
            delivery: self.delivery,
            limits: ImageLimits {
                max_image_bytes: image as usize,
                max_memory_bytes: memory as usize,
                max_disk_bytes: disk,
                read_timeout: std::time::Duration::from_millis(self.read_timeout_ms.clamp(200, 30_000)),
                ..d
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BiltermConfig;

    #[test]
    fn defaults_match_owner_budgets() {
        let s = BiltermConfig::default().clipboard.images.settings();
        assert!(s.enabled);
        assert_eq!(s.delivery, ImageDelivery::Path);
        assert_eq!(s.limits.max_image_bytes, 10 * 1024 * 1024);
        assert_eq!(s.limits.max_memory_bytes, 32 * 1024 * 1024);
        assert_eq!(s.limits.max_disk_bytes, 256 * 1024 * 1024);
    }

    #[test]
    fn toml_round_trip_and_clamping() {
        let cfg: BiltermConfig = toml::from_str(
            "[clipboard.images]\ndelivery = \"passthrough\"\nmax_image_mib = 9999\nmax_memory_mib = 0\nread_timeout_ms = 1\n",
        )
        .unwrap();
        assert_eq!(cfg.clipboard.images.delivery, ImageDelivery::Passthrough);
        let s = cfg.clipboard.images.settings();
        assert_eq!(s.limits.max_image_bytes, 64 * 1024 * 1024, "image ceiling");
        assert!(s.limits.max_memory_bytes >= s.limits.max_image_bytes, "memory never below one image");
        assert_eq!(s.limits.read_timeout.as_millis(), 200);
        let back: BiltermConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back.clipboard, cfg.clipboard);
        assert_eq!(toml::from_str::<BiltermConfig>("").unwrap().clipboard, ClipboardConfig::default());
    }
}
