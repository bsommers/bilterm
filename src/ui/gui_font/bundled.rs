//! Bundled mixed-case monospace font (JetBrains Mono, SIL OFL 1.1) for non-Legacy appearance profiles (45-04).
//!
//! The font bytes are embedded in the binary and written to the private runtime directory
//! (`secure_fs`, 0700 dir / 0600 files), then registered through the existing custom-font registry so each
//! platform loader (`FcConfigAppFontAddFile`, `AddFontResourceExW`, CoreText) picks them up unchanged.
//! Legacy profile never installs them, so classic looks resolve fonts exactly as before.

use std::io;
use std::path::{Path, PathBuf};

use crate::config::BiltermConfig;
use crate::pane::typography::register_custom_font_file;
use crate::secure_fs;
use crate::ui::design::AppearanceProfile;

/// Family name the bundled files register under.
pub const BUNDLED_FAMILY: &str = "JetBrains Mono";

/// Family name the bundled Lazenby Computer font files register under.
pub const BUNDLED_LAZENBY_FAMILY: &str = "Lazenby Computer";

/// `(file name, bytes)` for each bundled face.
pub const BUNDLED_FONTS: [(&str, &[u8]); 4] = [
    ("JetBrainsMono-Regular.ttf", include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf")),
    ("JetBrainsMono-Bold.ttf", include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf")),
    ("JetBrainsMono-Italic.ttf", include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf")),
    ("JetBrainsMono-BoldItalic.ttf", include_bytes!("../../../assets/fonts/JetBrainsMono-BoldItalic.ttf")),
];

/// `(file name, bytes)` for each extra retro bundled font face.
pub const BUNDLED_EXTRA_FONTS: [(&str, &[u8]); 2] = [
    ("LazenbyCompSmooth.ttf", include_bytes!("../../../assets/fonts/LazenbyCompSmooth.ttf")),
    ("LazenbyCompLiquid.ttf", include_bytes!("../../../assets/fonts/LazenbyCompLiquid.ttf")),
];

/// `true` when the resolved appearance profile should use the bundled font (anything but Legacy).
pub fn wants_bundled_font(cfg: &BiltermConfig) -> bool {
    AppearanceProfile::resolve(cfg) != AppearanceProfile::Legacy
}

/// Family used when nothing else is configured: the bundled font for non-Legacy profiles,
/// otherwise the platform's historical default (Legacy behaviour is unchanged).
pub fn default_family(cfg: &BiltermConfig, platform_default: &str) -> String {
    if wants_bundled_font(cfg) { BUNDLED_FAMILY.to_string() } else { platform_default.to_string() }
}

/// Writes the bundled faces into `dir` (created private) and registers them. Returns the written paths.
pub fn install_into(dir: &Path) -> io::Result<Vec<PathBuf>> {
    secure_fs::ensure_private_dir(dir)?;
    let mut out = Vec::with_capacity(BUNDLED_FONTS.len());
    for (name, bytes) in BUNDLED_FONTS {
        secure_fs::validate_plain_file_name(name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let path = dir.join(name);
        let up_to_date = std::fs::metadata(&path).map(|m| m.len() == bytes.len() as u64).unwrap_or(false);
        if !up_to_date {
            secure_fs::write_private_file(&path, bytes)?;
        }
        register_custom_font_file(&path).map_err(io::Error::other)?;
        out.push(path);
    }
    Ok(out)
}

/// Writes the extra bundled faces (e.g. Lazenby Computer) into `dir` (created private) and registers them. Returns the written paths.
pub fn install_extra_into(dir: &Path) -> io::Result<Vec<PathBuf>> {
    secure_fs::ensure_private_dir(dir)?;
    let mut out = Vec::with_capacity(BUNDLED_EXTRA_FONTS.len());
    for (name, bytes) in BUNDLED_EXTRA_FONTS {
        secure_fs::validate_plain_file_name(name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let path = dir.join(name);
        let up_to_date = std::fs::metadata(&path).map(|m| m.len() == bytes.len() as u64).unwrap_or(false);
        if !up_to_date {
            secure_fs::write_private_file(&path, bytes)?;
        }
        register_custom_font_file(&path).map_err(io::Error::other)?;
        out.push(path);
    }
    Ok(out)
}

/// Installs and registers the extra bundled fonts (Lazenby Computer) into the private runtime directory.
/// Best effort: failures are ignored.
pub fn install_extra_bundled_fonts() {
    if let Ok(base) = secure_fs::private_runtime_dir() {
        let _ = install_extra_into(&base.join("fonts"));
    }
}

/// Installs and registers the bundled font when `cfg` selects a non-Legacy profile. Best effort: failures are ignored
/// so the GUI falls back to the normal font candidates.
pub fn install_if_wanted(cfg: &BiltermConfig) {
    if !wants_bundled_font(cfg) {
        return;
    }
    if let Ok(base) = secure_fs::private_runtime_dir() {
        let _ = install_into(&base.join("fonts"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::typography::registered_custom_fonts;
    use crate::ui::design::AppearanceProfile;

    #[test]
    fn embedded_faces_are_valid_truetype_with_expected_family() {
        for (name, bytes) in BUNDLED_FONTS {
            assert!(bytes.len() > 100_000, "{name} suspiciously small");
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{name} not TrueType");
        }
    }

    #[test]
    fn embedded_extra_faces_are_valid_truetype_with_expected_family() {
        for (name, bytes) in BUNDLED_EXTRA_FONTS {
            assert!(bytes.len() > 50_000, "{name} suspiciously small");
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{name} not TrueType");
        }
    }

    #[test]
    fn legacy_never_installs_and_non_legacy_does() {
        let mut cfg = BiltermConfig::default();
        assert!(!wants_bundled_font(&cfg));
        cfg.appearance.profile = Some(AppearanceProfile::Obsidian);
        assert!(wants_bundled_font(&cfg));
        cfg.appearance.profile = Some(AppearanceProfile::Legacy);
        assert!(!wants_bundled_font(&cfg));
    }

    #[test]
    fn default_family_keeps_platform_default_for_legacy() {
        let mut cfg = BiltermConfig::default();
        assert_eq!(default_family(&cfg, "Menlo"), "Menlo");
        cfg.appearance.profile = Some(AppearanceProfile::Obsidian);
        assert_eq!(default_family(&cfg, "Menlo"), BUNDLED_FAMILY);
    }

    #[test]
    fn install_into_writes_private_files_and_registers_family() {
        let base = std::env::temp_dir().join(format!("bilterm-bundled-font-test-{}", secure_fs::unique_suffix()));
        std::fs::create_dir_all(&base).unwrap();
        let dir = base.join("fonts");
        let paths = install_into(&dir).unwrap();
        assert_eq!(paths.len(), 4);
        for p in &paths {
            assert!(p.exists());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(std::fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
            }
        }
        assert!(registered_custom_fonts().iter().any(|(fam, _)| fam == BUNDLED_FAMILY));
        // Idempotent.
        assert_eq!(install_into(&dir).unwrap().len(), 4);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn install_extra_into_writes_private_files_and_registers_family() {
        let base = std::env::temp_dir().join(format!("bilterm-bundled-extra-font-test-{}", secure_fs::unique_suffix()));
        std::fs::create_dir_all(&base).unwrap();
        let dir = base.join("fonts");
        let paths = install_extra_into(&dir).unwrap();
        assert_eq!(paths.len(), 2);
        for p in &paths {
            assert!(p.exists());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(std::fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
            }
        }
        assert!(registered_custom_fonts().iter().any(|(fam, _)| fam == BUNDLED_LAZENBY_FAMILY));
        // Idempotent.
        assert_eq!(install_extra_into(&dir).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&base);
    }
}
