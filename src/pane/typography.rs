//! Per-pane typography options (`FontFamilyPreset`, `PaneFontOptions`) and high-DPI canvas resolution presets (`CanvasResolution`) (`FONT-02`, `CANV-03`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Cached terminal pixel size (`(wx << 32) | wy`, `u64::MAX` = unprobed, `0` = unavailable).
static CACHED_TERMINAL_PIXELS: AtomicU64 = AtomicU64::new(u64::MAX);

/// Process-wide registry of user-added `.ttf` / `.otf` / `.ttc` font files (`(family_name, path)`).
static CUSTOM_FONT_FILES: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

/// Target pixel resolution preset for high-DPI canvas rendering (`CANV-03`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasResolution {
    /// Query OS `TIOCGWINSZ` (`ws_xpixel × ws_ypixel`) or default to `2x` Retina HiDPI (`16×32` px/cell).
    #[default]
    Auto,
    /// Standard `1x` (`8×16` px/cell, e.g. `640×384` at `80×24`).
    Scale1x,
    /// Retina / HiDPI `2x` (`16×32` px/cell, e.g. `1280×768` at `80×24`).
    Scale2x,
    /// Ultra-HiDPI `4x` (`32×64` px/cell, e.g. `2560×1536` at `80×24`).
    Scale4x,
    /// HD 720p (`1280×720`).
    Hd720p,
    /// Full HD 1080p (`1920×1080`).
    FullHd1080p,
    /// 4K UHD (`3840×2160`).
    UltraHd4k,
    /// Custom explicit pixel dimensions (`width × height`).
    Custom { width: u32, height: u32 },
}

impl CanvasResolution {
    pub fn from_name(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_ascii_lowercase();
        match lower.as_str() {
            "auto" | "native" | "hidpi" => Some(Self::Auto),
            "1x" | "sd" | "standard" => Some(Self::Scale1x),
            "2x" | "retina" => Some(Self::Scale2x),
            "4x" | "ultra" => Some(Self::Scale4x),
            "hd" | "720p" => Some(Self::Hd720p),
            "fhd" | "1080p" | "fullhd" => Some(Self::FullHd1080p),
            "4k" | "2160p" | "uhd" => Some(Self::UltraHd4k),
            _ => {
                if let Some((w_s, h_s)) = lower.split_once('x')
                    && let (Ok(w), Ok(h)) = (w_s.parse::<u32>(), h_s.parse::<u32>())
                    && w >= 16
                    && h >= 16
                {
                    Some(Self::Custom {
                        width: w.min(7680),
                        height: h.min(4320),
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Resolves `(px_per_col, px_per_row)` for a given `(cols, rows)` terminal grid (`CANV-03`).
    pub fn resolve_px_per_cell(self, cols: u16, rows: u16) -> (u32, u32) {
        let c = u32::from(cols.max(1));
        let r = u32::from(rows.max(1));
        match self {
            Self::Auto => {
                if let Some((wx, wy)) = probe_terminal_pixel_size()
                    && wx >= c * 4
                    && wy >= r * 8
                {
                    return ((wx / c).clamp(8, 64), (wy / r).clamp(16, 128));
                }
                (16, 32)
            }
            Self::Scale1x => (8, 16),
            Self::Scale2x => (16, 32),
            Self::Scale4x => (32, 64),
            Self::Hd720p => ((1280 / c).max(4), (720 / r).max(8)),
            Self::FullHd1080p => ((1920 / c).max(4), (1080 / r).max(8)),
            Self::UltraHd4k => ((3840 / c).max(4), (2160 / r).max(8)),
            Self::Custom { width, height } => ((width / c).max(2), (height / r).max(4)),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Auto => "Auto (2x HiDPI / TIOCGWINSZ)".to_string(),
            Self::Scale1x => "1x (8x16 px/cell)".to_string(),
            Self::Scale2x => "2x Retina (16x32 px/cell)".to_string(),
            Self::Scale4x => "4x Ultra-HiDPI (32x64 px/cell)".to_string(),
            Self::Hd720p => "720p HD (1280x720)".to_string(),
            Self::FullHd1080p => "1080p Full HD (1920x1080)".to_string(),
            Self::UltraHd4k => "4K UHD (3840x2160)".to_string(),
            Self::Custom { width, height } => format!("Custom ({width}x{height})"),
        }
    }
}

/// Queries the host OS terminal's hardware pixel dimensions (`ws_xpixel, ws_ypixel`) via `TIOCGWINSZ`,
/// caching the result in an atomic so 60 FPS rendering never repeats the syscall (`CANV-03`).
pub fn probe_terminal_pixel_size() -> Option<(u32, u32)> {
    let cached = CACHED_TERMINAL_PIXELS.load(Ordering::Relaxed);
    if cached != u64::MAX {
        return if cached == 0 {
            None
        } else {
            Some(((cached >> 32) as u32, (cached & 0xffff_ffff) as u32))
        };
    }
    refresh_terminal_pixel_size()
}

/// Re-probes `ioctl(TIOCGWINSZ)` on terminal resize events and updates [`CACHED_TERMINAL_PIXELS`].
pub fn refresh_terminal_pixel_size() -> Option<(u32, u32)> {
    #[cfg(unix)]
    {
        let probed = unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
                && ws.ws_xpixel > 0
                && ws.ws_ypixel > 0
            {
                Some((u32::from(ws.ws_xpixel), u32::from(ws.ws_ypixel)))
            } else {
                None
            }
        };
        let encoded = probed.map_or(0, |(wx, wy)| (u64::from(wx) << 32) | u64::from(wy));
        CACHED_TERMINAL_PIXELS.store(encoded, Ordering::Relaxed);
        probed
    }
    #[cfg(not(unix))]
    {
        CACHED_TERMINAL_PIXELS.store(0, Ordering::Relaxed);
        None
    }
}

/// Selectable font family preset per pane or window (`FONT-02`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFamilyPreset {
    /// Classic IBM 3270 Mainframe phosphor terminal font (`"IBM 3270"`).
    #[default]
    Ibm3270,
    /// Classic DOS Borland Turbo Vision 8×16 bitmap font (`"PxPlus IBM VGA8"`).
    BorlandDosVga,
    /// Classic 1974 Tektronix 4014 storage-tube vector stroke font (`"Tektronix 4014"`).
    Tektronix4014,
    /// Modern developer ligature font (`"JetBrains Mono"`).
    JetBrainsMono,
    /// Modern functional programming font (`"Fira Code"`).
    FiraCode,
    /// High-density slender terminal font (`"Iosevka Term"`).
    IosevkaTerm,
    /// Microsoft terminal font (`"Cascadia Code"` / `"Cascadia Mono"`).
    CascadiaCode,
    /// Classic ClearType Windows programming font (`"Consolas"`).
    Consolas,
    /// IBM modern monospace family (`"IBM Plex Mono"`).
    IbmPlexMono,
    /// Clean open-source monospace font (`"Hack"`).
    Hack,
    /// Classic Windows console monospace font (`"Lucida Console"`).
    LucidaConsole,
    /// Classic slab-serif typewriter monospace font (`"Courier New"`).
    CourierNew,
    /// User-specified custom font family name.
    Custom(String),
}

impl FontFamilyPreset {
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ibm3270" | "3270" | "ibm" | "mainframe" => Some(Self::Ibm3270),
            "vga" | "vga8" | "dos" | "borland" | "cp437" | "turbo" => Some(Self::BorlandDosVga),
            "tek" | "tek4014" | "tektronix" | "vector" => Some(Self::Tektronix4014),
            "jetbrains" | "jetbrains-mono" | "jetbrains mono" | "jbmono" | "jb" => {
                Some(Self::JetBrainsMono)
            }
            "fira" | "firacode" | "fira-code" | "fira code" => Some(Self::FiraCode),
            "iosevka" | "iosevka-term" | "iosevka term" => Some(Self::IosevkaTerm),
            "cascadia" | "cascadia-code" | "cascadia code" | "cascadia-mono" | "cascadia mono" => {
                Some(Self::CascadiaCode)
            }
            "consolas" => Some(Self::Consolas),
            "plex" | "ibm-plex" | "ibm-plex-mono" | "ibm plex mono" | "ibmplex" => {
                Some(Self::IbmPlexMono)
            }
            "hack" => Some(Self::Hack),
            "lucida" | "lucida-console" | "lucida console" => Some(Self::LucidaConsole),
            "courier" | "courier-new" | "courier new" => Some(Self::CourierNew),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Ibm3270 => "IBM 3270",
            Self::BorlandDosVga => "PxPlus IBM VGA8",
            Self::Tektronix4014 => "Tektronix 4014",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::FiraCode => "Fira Code",
            Self::IosevkaTerm => "Iosevka Term",
            Self::CascadiaCode => "Cascadia Code",
            Self::Consolas => "Consolas",
            Self::IbmPlexMono => "IBM Plex Mono",
            Self::Hack => "Hack",
            Self::LucidaConsole => "Lucida Console",
            Self::CourierNew => "Courier New",
            Self::Custom(name) => name.as_str(),
        }
    }

    pub fn short_badge(&self) -> &str {
        match self {
            Self::Ibm3270 => "3270",
            Self::BorlandDosVga => "VGA8",
            Self::Tektronix4014 => "TEK",
            Self::JetBrainsMono => "JBMono",
            Self::FiraCode => "Fira",
            Self::IosevkaTerm => "Iosevka",
            Self::CascadiaCode => "Cascadia",
            Self::Consolas => "Consolas",
            Self::IbmPlexMono => "Plex",
            Self::Hack => "Hack",
            Self::LucidaConsole => "Lucida",
            Self::CourierNew => "Courier",
            Self::Custom(name) => name.as_str(),
        }
    }

    /// Ordered candidate OS TrueType font family names to probe when rendering in `--gui` mode.
    pub fn candidate_os_font_faces(&self) -> Vec<&str> {
        match self {
            Self::CascadiaCode => vec![
                "Cascadia Mono",
                "Cascadia Code",
                "Cascadia Mono NF",
                "CascadiaCode NF",
                "MesloLGS NF",
                "JetBrainsMono Nerd Font",
                "SF Mono",
                "Menlo",
                "Consolas",
                "Lucida Console",
                "Courier New",
            ],
            Self::Consolas => vec![
                "Consolas",
                "Cascadia Mono",
                "SF Mono",
                "Menlo",
                "Lucida Console",
                "Courier New",
            ],
            Self::JetBrainsMono => vec![
                "JetBrains Mono",
                "JetBrainsMono Nerd Font Mono",
                "JetBrainsMono Nerd Font",
                "JetBrainsMono NF",
                "Cascadia Mono",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::FiraCode => vec![
                "Fira Code",
                "FiraCode Nerd Font Mono",
                "FiraCode Nerd Font",
                "FiraCode NF",
                "Cascadia Code",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::IosevkaTerm => vec![
                "Iosevka Term",
                "Iosevka",
                "Iosevka Nerd Font Mono",
                "Iosevka NF",
                "Cascadia Mono",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::IbmPlexMono => vec![
                "IBM Plex Mono",
                "BlexMono Nerd Font Mono",
                "BlexMono NF",
                "Cascadia Mono",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::Hack => vec![
                "Hack",
                "Hack Nerd Font Mono",
                "Hack NF",
                "Cascadia Mono",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::Ibm3270 => vec![
                "3270 Nerd Font Mono",
                "3270 Nerd Font",
                "IBM 3270",
                "IBM Plex Mono",
                "Cascadia Mono",
                "MesloLGS NF",
                "SF Mono",
                "Menlo",
                "Consolas",
            ],
            Self::BorlandDosVga => vec![
                "PxPlus IBM VGA8",
                "Perfect DOS VGA 437",
                "More Perfect DOS VGA",
                "Menlo",
                "Consolas",
                "Lucida Console",
            ],
            Self::Tektronix4014 => vec!["Tektronix 4014", "Cascadia Mono", "SF Mono", "Menlo", "Consolas"],
            Self::LucidaConsole => vec!["Lucida Console", "Menlo", "Consolas", "Courier New"],
            Self::CourierNew => vec!["Courier New", "Consolas", "Menlo"],
            Self::Custom(name) => vec![
                name.as_str(),
                "Cascadia Mono",
                "Cascadia Code",
                "SF Mono",
                "Menlo",
                "Consolas",
                "Courier New",
            ],
        }
    }

    /// Cycles to the next built-in font family preset (`:font cycle`).
    pub fn next_preset(&self) -> Self {
        match self {
            Self::CascadiaCode => Self::Consolas,
            Self::Consolas => Self::JetBrainsMono,
            Self::JetBrainsMono => Self::FiraCode,
            Self::FiraCode => Self::IosevkaTerm,
            Self::IosevkaTerm => Self::IbmPlexMono,
            Self::IbmPlexMono => Self::Hack,
            Self::Hack => Self::Ibm3270,
            Self::Ibm3270 => Self::BorlandDosVga,
            Self::BorlandDosVga => Self::LucidaConsole,
            Self::LucidaConsole => Self::CourierNew,
            Self::CourierNew | Self::Tektronix4014 | Self::Custom(_) => Self::CascadiaCode,
        }
    }
}

/// Parses a TrueType (`.ttf`) or OpenType (`.otf`) binary `name` table to extract the internal
/// Font Family Name (`nameID == 1` or `nameID == 16`).
pub fn extract_ttf_family_name(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut name_offset = None;
    let mut name_len = 0usize;
    for i in 0..num_tables {
        let rec = 12 + i.checked_mul(16)?;
        if rec + 16 > bytes.len() {
            break;
        }
        if &bytes[rec..rec + 4] == b"name" {
            let off = u32::from_be_bytes([
                bytes[rec + 8],
                bytes[rec + 9],
                bytes[rec + 10],
                bytes[rec + 11],
            ]) as usize;
            let len = u32::from_be_bytes([
                bytes[rec + 12],
                bytes[rec + 13],
                bytes[rec + 14],
                bytes[rec + 15],
            ]) as usize;
            name_offset = Some(off);
            name_len = len;
            break;
        }
    }
    let base = name_offset?;
    if base + 6 > bytes.len() || base.saturating_add(name_len) > bytes.len() {
        return None;
    }
    let count = u16::from_be_bytes([bytes[base + 2], bytes[base + 3]]) as usize;
    let str_offset = u16::from_be_bytes([bytes[base + 4], bytes[base + 5]]) as usize;
    let storage_base = base.checked_add(str_offset)?;

    let mut fallback_ascii: Option<String> = None;
    for i in 0..count {
        let r = base + 6 + i.checked_mul(12)?;
        if r + 12 > bytes.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([bytes[r], bytes[r + 1]]);
        let name_id = u16::from_be_bytes([bytes[r + 6], bytes[r + 7]]);
        let length = u16::from_be_bytes([bytes[r + 8], bytes[r + 9]]) as usize;
        let offset = u16::from_be_bytes([bytes[r + 10], bytes[r + 11]]) as usize;
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let s_start = storage_base.checked_add(offset)?;
        let s_end = s_start.checked_add(length)?;
        if s_end > bytes.len() || length == 0 {
            continue;
        }
        let raw = &bytes[s_start..s_end];
        if (platform_id == 3 || platform_id == 0) && length.is_multiple_of(2) {
            let utf16: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_be_bytes(c))
                .collect();
            if let Ok(s) = String::from_utf16(&utf16) {
                let trimmed = s.trim().trim_matches('\0').to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        } else if platform_id == 1
            && let Ok(s) = std::str::from_utf8(raw)
        {
            let trimmed = s.trim().trim_matches('\0').to_string();
            if !trimmed.is_empty() && fallback_ascii.is_none() {
                fallback_ascii = Some(trimmed);
            }
        }
    }
    fallback_ascii
}

/// Returns the default user custom fonts directory (`~/.config/bilterm/fonts` or `%APPDATA%\bilterm\fonts`).
pub fn default_custom_fonts_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg).join("bilterm").join("fonts"));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(home).join(".config").join("bilterm").join("fonts"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(appdata).join("bilterm").join("fonts"));
    }
    if let Some(up) = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(up).join(".config").join("bilterm").join("fonts"));
    }
    None
}

/// Registers a `.ttf`, `.otf`, or `.ttc` font file in the runtime font registry and returns its resolved family name.
pub fn register_custom_font_file(path: &Path) -> std::result::Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Cannot read font file '{}': {e}", path.display()))?;
    let family = extract_ttf_family_name(&bytes).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom Font")
            .replace(['_', '-'], " ")
    });
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Ok(mut guard) = CUSTOM_FONT_FILES.lock()
        && !guard.iter().any(|(_, p)| p == &canonical)
    {
        guard.push((family.clone(), canonical));
    }
    Ok(family)
}

/// Returns all custom `(family_name, path)` font files registered during this session.
pub fn registered_custom_fonts() -> Vec<(String, PathBuf)> {
    CUSTOM_FONT_FILES
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// Scans `dir` for `.ttf`, `.otf`, and `.ttc` files and registers them in [`CUSTOM_FONT_FILES`].
pub fn scan_and_register_fonts_dir(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut added = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return added;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if let Some(ext) = p.extension().and_then(|e| e.to_str())
            && matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc")
            && let Ok(fam) = register_custom_font_file(&p)
        {
            added.push((fam, p));
        }
    }
    added
}

/// Per-pane / per-window selectable typography options (`FONT-02`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneFontOptions {
    pub family: FontFamilyPreset,
    pub size_pt: Option<u16>,
    pub scale_pct: Option<u16>,
    pub bold: bool,
    pub italic: bool,
    pub double_width: bool,
}

impl PaneFontOptions {
    pub fn effective_size_pt_with_defaults(&self, default_size_pt: u16, default_scale_pct: u16) -> u16 {
        let base = self.size_pt.unwrap_or(default_size_pt).clamp(6, 72);
        let pct = self.effective_scale_pct_with_default(default_scale_pct);
        ((u32::from(base) * u32::from(pct) + 50) / 100).clamp(6, 96) as u16
    }

    pub fn effective_scale_pct_with_default(&self, default_scale_pct: u16) -> u16 {
        self.scale_pct.unwrap_or(default_scale_pct).clamp(50, 400)
    }

    pub fn has_custom_overrides(&self) -> bool {
        self.family != FontFamilyPreset::Ibm3270
            || self.size_pt.is_some()
            || self.scale_pct.is_some()
            || self.bold
            || self.italic
            || self.double_width
    }

    pub fn summary_with_defaults(&self, default_size_pt: u16, default_scale_pct: u16) -> String {
        format!(
            "family='{}' size={}pt ({}%) bold={} italic={} double_width={}",
            self.family.display_name(),
            self.effective_size_pt_with_defaults(default_size_pt, default_scale_pct),
            self.effective_scale_pct_with_default(default_scale_pct),
            self.bold,
            self.italic,
            self.double_width
        )
    }

    pub fn execute_command_with_defaults(
        &mut self,
        default_size_pt: u16,
        default_scale_pct: u16,
        raw: &str,
    ) -> std::result::Result<String, String> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        match parts.first().copied().unwrap_or("status") {
            "status" => Ok(format!(
                "PANE FONT: {}",
                self.summary_with_defaults(default_size_pt, default_scale_pct)
            )),
            "list" | "presets" => {
                let custom = registered_custom_fonts();
                let custom_suffix = if custom.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = custom.into_iter().map(|(f, _)| f).collect();
                    format!(" | loaded: [{}]", names.join(", "))
                };
                Ok(format!(
                    "FONT PRESETS: cascadia | consolas | jetbrains | firacode | iosevka | plex | hack | ibm3270 | vga (Borland DOS) | tek4014 | lucida | courier | custom <name> | add <path.ttf>{custom_suffix}"
                ))
            }
            "cycle" | "next" => {
                self.family = self.family.next_preset();
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "add" | "file" | "load" => {
                let raw_path = parts
                    .get(1..)
                    .map(|s| s.join(" "))
                    .unwrap_or_default()
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if raw_path.is_empty() {
                    return Err("Usage: :font add <path/to/font.ttf>".to_string());
                }
                let fam = register_custom_font_file(Path::new(&raw_path))?;
                self.family = FontFamilyPreset::from_name(&fam)
                    .unwrap_or_else(|| FontFamilyPreset::Custom(fam.clone()));
                Ok(format!(
                    "LOADED FONT FILE '{}' -> {}",
                    raw_path,
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "reset" | "default" => {
                *self = Self::default();
                Ok(format!(
                    "PANE FONT RESET: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "+" | "in" | "up" | "bigger" => {
                let cur = self.effective_scale_pct_with_default(default_scale_pct);
                self.scale_pct = Some((cur + 10).clamp(50, 400));
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "-" | "out" | "down" | "smaller" => {
                let cur = self.effective_scale_pct_with_default(default_scale_pct);
                self.scale_pct = Some(cur.saturating_sub(10).clamp(50, 400));
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "bold" => {
                self.bold = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.bold,
                };
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "italic" => {
                self.italic = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.italic,
                };
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "double" | "2x" | "wide" => {
                self.double_width = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.double_width,
                };
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "size" => {
                let Some(v) = parts
                    .get(1)
                    .and_then(|s| s.trim_end_matches("pt").parse::<u16>().ok())
                else {
                    return Err("Usage: :font size <6..72>".to_string());
                };
                self.size_pt = Some(v.clamp(6, 72));
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "scale" => {
                let Some(v) = parts
                    .get(1)
                    .and_then(|s| s.trim_end_matches('%').parse::<u16>().ok())
                else {
                    return Err("Usage: :font scale <50..400>%".to_string());
                };
                self.scale_pct = Some(v.clamp(50, 400));
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            "custom" | "family" => {
                let name = parts
                    .get(1..)
                    .map(|s| s.join(" "))
                    .unwrap_or_default()
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                if name.is_empty() {
                    return Err("Usage: :font custom <Font Family Name>".to_string());
                }
                if let Some(preset) = FontFamilyPreset::from_name(&name) {
                    self.family = preset;
                } else {
                    self.family = FontFamilyPreset::Custom(name);
                }
                Ok(format!(
                    "PANE FONT: {}",
                    self.summary_with_defaults(default_size_pt, default_scale_pct)
                ))
            }
            other => {
                if let Some(preset) = FontFamilyPreset::from_name(other) {
                    self.family = preset;
                    if let Some(arg2) = parts.get(1).copied() {
                        if let Some(pt) = arg2.strip_suffix("pt").and_then(|s| s.parse::<u16>().ok()) {
                            self.size_pt = Some(pt.clamp(6, 72));
                        } else if let Some(pct) =
                            arg2.strip_suffix('%').and_then(|s| s.parse::<u16>().ok())
                        {
                            self.scale_pct = Some(pct.clamp(50, 400));
                        } else if let Ok(v) = arg2.parse::<u16>() {
                            if v < 50 {
                                self.size_pt = Some(v.clamp(6, 72));
                            } else {
                                self.scale_pct = Some(v.clamp(50, 400));
                            }
                        }
                    }
                    return Ok(format!(
                        "PANE FONT: {}",
                        self.summary_with_defaults(default_size_pt, default_scale_pct)
                    ));
                }
                if let Some(pt) = other.strip_suffix("pt").and_then(|s| s.parse::<u16>().ok()) {
                    self.size_pt = Some(pt.clamp(6, 72));
                    return Ok(format!(
                        "PANE FONT: {}",
                        self.summary_with_defaults(default_size_pt, default_scale_pct)
                    ));
                }
                if let Some(pct) = other.strip_suffix('%').and_then(|s| s.parse::<u16>().ok()) {
                    self.scale_pct = Some(pct.clamp(50, 400));
                    return Ok(format!(
                        "PANE FONT: {}",
                        self.summary_with_defaults(default_size_pt, default_scale_pct)
                    ));
                }
                if let Ok(v) = other.parse::<u16>() {
                    if v < 50 {
                        self.size_pt = Some(v.clamp(6, 72));
                    } else {
                        self.scale_pct = Some(v.clamp(50, 400));
                    }
                    return Ok(format!(
                        "PANE FONT: {}",
                        self.summary_with_defaults(default_size_pt, default_scale_pct)
                    ));
                }
                let joined = parts.join(" ");
                if let Some(preset) = FontFamilyPreset::from_name(&joined) {
                    self.family = preset;
                    return Ok(format!(
                        "PANE FONT: {}",
                        self.summary_with_defaults(default_size_pt, default_scale_pct)
                    ));
                }
                Err(format!(
                    "Unknown :font option '{other}'. Usage: :font <cascadia|consolas|jetbrains|firacode|iosevka|plex|hack|ibm3270|vga|tek4014|lucida|courier|custom <name>|add <path.ttf>|cycle|size <pt>|scale <pct>%|+|-|bold|italic|double|pane <id> ...|all ...|reset|list>"
                ))
            }
        }
    }
}
