//! Per-pane typography options (`FontFamilyPreset`, `PaneFontOptions`) and high-DPI canvas resolution presets (`CanvasResolution`) (`FONT-02`, `CANV-03`).

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Cached terminal pixel size (`(wx << 32) | wy`, `u64::MAX` = unprobed, `0` = unavailable).
static CACHED_TERMINAL_PIXELS: AtomicU64 = AtomicU64::new(u64::MAX);

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
    /// Microsoft terminal font (`"Cascadia Code"`).
    CascadiaCode,
    /// Clean open-source monospace font (`"Hack"`).
    Hack,
    /// User-specified custom font family name.
    Custom(String),
}

impl FontFamilyPreset {
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ibm3270" | "3270" | "ibm" | "mainframe" => Some(Self::Ibm3270),
            "vga" | "vga8" | "dos" | "borland" | "cp437" | "turbo" => Some(Self::BorlandDosVga),
            "tek" | "tek4014" | "tektronix" | "vector" => Some(Self::Tektronix4014),
            "jetbrains" | "jetbrains-mono" | "jbmono" | "jb" => Some(Self::JetBrainsMono),
            "fira" | "firacode" | "fira-code" => Some(Self::FiraCode),
            "iosevka" | "iosevka-term" => Some(Self::IosevkaTerm),
            "cascadia" | "cascadia-code" => Some(Self::CascadiaCode),
            "hack" => Some(Self::Hack),
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
            Self::Hack => "Hack",
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
            Self::Hack => "Hack",
            Self::Custom(name) => name.as_str(),
        }
    }
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
            "list" | "presets" => Ok(
                "FONT PRESETS: ibm3270 | vga (Borland DOS) | tek4014 | jetbrains | firacode | iosevka | cascadia | hack | custom <name>"
                    .to_string(),
            ),
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
                let name = parts.get(1..).map(|s| s.join(" ")).unwrap_or_default();
                if name.trim().is_empty() {
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
                Err(format!(
                    "Unknown :font option '{other}'. Usage: :font <ibm3270|vga|tek4014|jetbrains|firacode|iosevka|cascadia|hack|custom <name>|size <pt>|scale <pct>%|+|-|bold|italic|double|pane <id> ...|all ...|reset|list>"
                ))
            }
        }
    }
}
