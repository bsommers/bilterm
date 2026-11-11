//! Google Fonts and font archive (.zip / URL) fetching and installation for Bilterm (`FONT-02`).
//!
//! Provides network-isolated fast path for bundled / registered presets, Google Fonts specimen
//! URL parsing, CSS `@font-face` TrueType URL extraction, and archive extraction into the private
//! custom fonts directory.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pane::typography::{
    default_custom_fonts_dir, register_custom_font_file, registered_custom_fonts,
    FontFamilyPreset,
};

/// Validates whether the first 4 bytes of a buffer match known TrueType / OpenType font signatures:
/// `0x00010000`, `OTTO`, `ttcf`, or `true`.
pub fn is_font_magic(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    matches!(&bytes[..4], [0, 1, 0, 0] | b"OTTO" | b"ttcf" | b"true")
}

/// Expands a leading `~/` to `$HOME`.
pub fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(p)
}

/// Parses a Google Fonts specimen URL or raw family name into a canonical font family name.
///
/// Handles URLs such as `https://fonts.google.com/specimen/VT323?preview.script=Latn`,
/// `https://fonts.google.com/specimen/Share+Tech+Mono`, or raw names such as `"Share Tech Mono"`.
pub fn parse_google_font_family(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        return Err("Empty Google Font family name or URL".to_string());
    }
    let mut part = trimmed;
    if let Some(idx) = part.find("/specimen/") {
        part = &part[idx + "/specimen/".len()..];
    }
    if let Some(idx) = part.find('?') {
        part = &part[..idx];
    }
    if let Some(idx) = part.find('#') {
        part = &part[..idx];
    }
    let decoded = part.replace("%20", " ").replace('+', " ");
    let family = decoded.trim().to_string();
    if family.is_empty() {
        return Err("Could not extract Google Font family name from URL".to_string());
    }
    if !family.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_')) {
        return Err(format!("Invalid characters in font family name: '{family}'"));
    }
    Ok(family)
}

/// Extracts the first TrueType / OpenType URL from a Google Fonts CSS `@font-face` response.
pub fn extract_ttf_url_from_google_css(css: &str) -> Option<String> {
    let mut search = css;
    while let Some(start_idx) = search.find("url(") {
        let after = &search[start_idx + 4..];
        if let Some(end_idx) = after.find(')') {
            let candidate = after[..end_idx].trim().trim_matches('"').trim_matches('\'').trim();
            if (candidate.starts_with("https://fonts.gstatic.com/")
                || candidate.starts_with("http://fonts.gstatic.com/"))
                && (candidate.ends_with(".ttf") || candidate.ends_with(".otf"))
            {
                return Some(candidate.to_string());
            }
            search = &after[end_idx + 1..];
        } else {
            break;
        }
    }
    None
}

/// Downloads and installs a Google Font by family name or specimen URL into the private fonts directory.
///
/// Fast path: if the font family matches a bundled preset (e.g. `VT323`, `Lazenby Computer`,
/// `JetBrains Mono`) or is already registered in this session, network I/O is skipped completely.
pub fn install_google_font(raw: &str) -> Result<String, String> {
    let family = parse_google_font_family(raw)?;
    if let Some(preset) = FontFamilyPreset::from_name(&family)
        && matches!(
            preset,
            FontFamilyPreset::Vt323
                | FontFamilyPreset::LazenbyComputer
                | FontFamilyPreset::LazenbyLiquid
                | FontFamilyPreset::JetBrainsMono
        )
    {
        return Ok(preset.display_name().to_string());
    }
    if registered_custom_fonts()
        .iter()
        .any(|(f, _)| f.eq_ignore_ascii_case(&family))
    {
        return Ok(family);
    }

    let encoded_family = family.replace(' ', "+");
    let css_url = format!("https://fonts.googleapis.com/css2?family={encoded_family}");

    let mut ttf_url_opt = None;
    if let Ok(output) = Command::new("curl")
        .args(["-fsSL", "-m", "15", "-A", "Mozilla/5.0", &css_url])
        .output()
        && output.status.success()
        && let Ok(css) = String::from_utf8(output.stdout)
    {
        ttf_url_opt = extract_ttf_url_from_google_css(&css);
    }

    let ttf_url = ttf_url_opt.unwrap_or_else(|| {
        let slug_lower = family.to_ascii_lowercase().replace([' ', '-', '_'], "");
        let slug_no_spaces = family.replace([' ', '-', '_'], "");
        format!("https://raw.githubusercontent.com/google/fonts/main/ofl/{slug_lower}/{slug_no_spaces}-Regular.ttf")
    });

    let dest_dir = default_custom_fonts_dir().ok_or_else(|| "Cannot determine fonts directory".to_string())?;
    ensure_fonts_dir(&dest_dir).map_err(|e| format!("Cannot create fonts directory: {e}"))?;

    let file_stem = family.replace(' ', "_");
    let dest_path = dest_dir.join(format!("{file_stem}.ttf"));

    let dl_output = Command::new("curl")
        .args(["-fsSL", "-m", "20", &ttf_url])
        .output()
        .map_err(|e| format!("Failed to download font via curl: {e}"))?;

    if !dl_output.status.success() || dl_output.stdout.len() < 1000 || !is_font_magic(&dl_output.stdout) {
        return Err(format!("Failed to download valid font data for '{family}' from {ttf_url}"));
    }

    crate::secure_fs::write_private_file(&dest_path, &dl_output.stdout)
        .map_err(|e| format!("Failed to write font file: {e}"))?;

    register_custom_font_file(&dest_path)
}

/// Installs a font from a local file path (`.ttf`, `.otf`, `.zip`), a URL, or a Google Fonts link.
pub fn install_font_source(raw: &str) -> Result<String, String> {
    let raw = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if raw.is_empty() {
        return Err("Usage: :font add <path.ttf|path.zip|url>".to_string());
    }
    if raw.contains("fonts.google.com/specimen/") {
        return install_google_font(raw);
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        let dest_dir = default_custom_fonts_dir().ok_or_else(|| "Cannot determine fonts directory".to_string())?;
        ensure_fonts_dir(&dest_dir).map_err(|e| format!("Cannot create fonts directory: {e}"))?;

        if raw.split('?').next().unwrap_or(raw).ends_with(".zip") {
            let temp_zip = dest_dir.join(format!(".download-{}.zip", std::process::id()));
            let dl = Command::new("curl")
                .args(["-fsSL", "-m", "30", raw])
                .output()
                .map_err(|e| format!("Failed to download font archive: {e}"))?;
            if !dl.status.success() || dl.stdout.is_empty() {
                return Err(format!("Download failed for '{raw}'"));
            }
            crate::secure_fs::write_private_file(&temp_zip, &dl.stdout)
                .map_err(|e| format!("Failed to write downloaded archive: {e}"))?;
            let res = install_zip_archive(&temp_zip, &dest_dir);
            let _ = std::fs::remove_file(&temp_zip);
            return res;
        }

        let url_path = raw.split('?').next().unwrap_or(raw);
        let file_name = Path::new(url_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("custom_font.ttf");
        let dest_path = dest_dir.join(file_name);

        let dl = Command::new("curl")
            .args(["-fsSL", "-m", "20", raw])
            .output()
            .map_err(|e| format!("Failed to download font file: {e}"))?;
        if !dl.status.success() || dl.stdout.len() < 1000 || !is_font_magic(&dl.stdout) {
            return Err(format!("Downloaded data from '{raw}' is not a valid font"));
        }
        crate::secure_fs::write_private_file(&dest_path, &dl.stdout)
            .map_err(|e| format!("Failed to write font file: {e}"))?;
        return register_custom_font_file(&dest_path);
    }

    let path = expand_tilde(raw);
    if path.extension().and_then(|e| e.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
        let dest_dir = default_custom_fonts_dir().ok_or_else(|| "Cannot determine fonts directory".to_string())?;
        ensure_fonts_dir(&dest_dir).map_err(|e| format!("Cannot create fonts directory: {e}"))?;
        return install_zip_archive(&path, &dest_dir);
    }

    register_custom_font_file(&path)
}

/// Creates the fonts directory on a fresh profile (its `bilterm/` parent may not exist yet), then
/// applies the usual private-directory checks (owner, 0700, not a symlink) to the directory itself.
fn ensure_fonts_dir(dir: &Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent() {
        crate::secure_fs::create_private_dir_all(parent)?;
    }
    crate::secure_fs::ensure_private_dir(dir)
}

/// Unzips a font archive into a private staging directory, moves valid font files to `dest_dir`,
/// registers them, and returns the primary font family name.
fn install_zip_archive(zip_path: &Path, dest_dir: &Path) -> Result<String, String> {
    if !zip_path.exists() {
        return Err(format!("Zip archive '{}' does not exist", zip_path.display()));
    }
    let staging_name = format!(".staging-zip-{}", std::process::id());
    let staging_dir = dest_dir.join(staging_name);
    let _ = std::fs::remove_dir_all(&staging_dir);
    crate::secure_fs::ensure_private_dir(&staging_dir)
        .map_err(|e| format!("Cannot create zip staging directory: {e}"))?;

    let unzip_res = Command::new("unzip")
        .args(["-oq", "-j"])
        .arg(zip_path)
        .arg("-d")
        .arg(&staging_dir)
        .status();

    let Ok(status) = unzip_res else {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err("Failed to execute unzip tool".to_string());
    };
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(format!("unzip failed on '{}'", zip_path.display()));
    }

    let mut primary_family = None;
    if let Ok(entries) = std::fs::read_dir(&staging_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file()
                && let Ok(bytes) = std::fs::read(&p)
                && is_font_magic(&bytes)
                && let Some(fname) = p.file_name()
            {
                let target = dest_dir.join(fname);
                if let Ok(()) = crate::secure_fs::write_private_file(&target, &bytes)
                    && let Ok(fam) = register_custom_font_file(&target)
                    && primary_family.is_none()
                {
                    primary_family = Some(fam);
                }
            }
        }
    }

    let _ = std::fs::remove_dir_all(&staging_dir);
    primary_family.ok_or_else(|| {
        format!(
            "No valid TrueType or OpenType font files found in '{}'",
            zip_path.display()
        )
    })
}
