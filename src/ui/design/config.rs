//! `[appearance]` configuration section (all fields optional; defaults preserve legacy behaviour).

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use super::AppearanceProfile;

/// Row/column breathing room used by shell and panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    #[default]
    #[serde(alias = "standard")]
    Comfortable,
    Spacious,
}

impl Density {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "compact" => Some(Self::Compact),
            "comfortable" | "standard" => Some(Self::Comfortable),
            "spacious" => Some(Self::Spacious),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Spacious => "spacious",
        }
    }

    /// Cells of inner padding for panels/drawers.
    pub const fn padding(self) -> u16 {
        match self {
            Self::Compact => 0,
            Self::Comfortable => 1,
            Self::Spacious => 2,
        }
    }
    /// Cells between adjacent panes.
    pub const fn gap(self) -> u16 {
        match self {
            Self::Compact | Self::Comfortable => 0,
            Self::Spacious => 1,
        }
    }
}

/// Visual weight of pane borders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BorderWeight {
    None,
    #[default]
    Hairline,
    Regular,
    Bold,
    Double,
}

impl BorderWeight {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "hairline" | "thin" => Some(Self::Hairline),
            "regular" | "normal" | "solid" => Some(Self::Regular),
            "bold" | "thick" => Some(Self::Bold),
            "double" => Some(Self::Double),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Hairline => "hairline",
            Self::Regular => "regular",
            Self::Bold => "bold",
            Self::Double => "double",
        }
    }
}

/// How much application chrome is shown (consumed by Phase 46 shell).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    /// Top bar, pane titles, status bar.
    #[default]
    Standard,
    /// Top bar only on focus; minimal pane chrome. Also spelled `minimal`.
    #[serde(alias = "minimal")]
    Focus,
    /// All chrome hidden except a one-line input row.
    Zen,
}

impl DisplayMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "standard" | "std" => Some(Self::Standard),
            "minimal" | "focus" => Some(Self::Focus),
            "zen" => Some(Self::Zen),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Focus => "minimal",
            Self::Zen => "zen",
        }
    }
}

/// Which pane frames show a title (non-Legacy only; `None` in config keeps the display-mode default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitlePolicy {
    #[default]
    All,
    Focused,
    None,
}

impl TitlePolicy {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "focused" | "focus" => Some(Self::Focused),
            "none" | "off" => Some(Self::None),
            _ => Option::None,
        }
    }

    /// Whether a pane with focus state `active` shows its title.
    pub const fn shows(self, active: bool) -> bool {
        match self {
            Self::All => true,
            Self::Focused => active,
            Self::None => false,
        }
    }
}

/// User-defined custom appearance profile built on top of one of the 8 built-in profiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomAppearanceProfile {
    pub base: AppearanceProfile,
    pub surface: Option<String>,
    pub density: Option<Density>,
    pub radius: Option<String>,
    pub border: Option<BorderWeight>,
    pub shadow: Option<String>,
    pub motion: Option<String>,
    pub opacity_pct: Option<u8>,
    pub blur_strength: Option<u8>,
    pub accent_override: Option<String>,
}

/// `[appearance]` section. Percent fields are `0..=100`; `None` means "use the profile default".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Explicit profile. `None` derives it from the legacy style flags (see `AppearanceProfile::resolve`).
    pub profile: Option<AppearanceProfile>,
    /// Surface opacity override (`0` fully clear .. `100` opaque).
    pub opacity_pct: Option<u8>,
    /// Halo/glow strength override (`0..=100`).
    pub glow_pct: Option<u8>,
    /// Corner radius override in pixels for the GUI renderer.
    pub corner_radius_px: Option<u8>,
    pub border: BorderWeight,
    pub density: Density,
    pub display_mode: DisplayMode,
    /// Pane title policy override. `None` keeps the display-mode default (Standard: all, Minimal: none).
    pub title: Option<TitlePolicy>,
    /// Accent-colored border on the focused pane (non-Legacy only).
    pub focus_ring: bool,
    /// Optional bundled/UI font family for chrome text (GUI only; `None` keeps `font_family`).
    pub ui_font: Option<String>,
    /// Named user-created custom profiles (`:appearance duplicate`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_profiles: BTreeMap<String, CustomAppearanceProfile>,
    /// Active custom profile name if a custom profile is currently selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_custom: Option<String>,
}

fn radius_px_from_token(token: &str) -> Option<(u8, String)> {
    let lower = token.trim().to_ascii_lowercase();
    match lower.as_str() {
        "sharp" | "none" => Some((0, "sharp".to_string())),
        "subtle" | "small" => Some((4, "subtle".to_string())),
        "rounded" | "medium" => Some((8, "rounded".to_string())),
        "pill" | "large" => Some((16, "pill".to_string())),
        _ => {
            let n = lower.strip_suffix("px").unwrap_or(&lower).parse::<u8>().ok()?;
            (n <= 24).then(|| (n, format!("{n}px")))
        }
    }
}

fn surface_presets(token: &str) -> Option<(&'static str, u8, u8)> {
    match token.trim().to_ascii_lowercase().as_str() {
        "opaque" | "solid" => Some(("solid", 100, 0)),
        "matte" => Some(("matte", 98, 0)),
        "translucent" => Some(("translucent", 88, 15)),
        "acrylic" => Some(("acrylic", 80, 35)),
        "glass" => Some(("glass", 72, 55)),
        _ => None,
    }
}

fn validate_file_path(path: &Path) -> Result<(), String> {
    let raw = path.as_os_str().to_string_lossy();
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err("invalid empty or NUL path".to_string());
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("path traversal ('..') is not allowed".to_string());
    }
    Ok(())
}

impl AppearanceConfig {
    /// Clamps user-supplied percentages into range.
    pub fn sanitized(mut self) -> Self {
        self.opacity_pct = self.opacity_pct.map(|v| v.min(100));
        self.glow_pct = self.glow_pct.map(|v| v.min(100));
        self.corner_radius_px = self.corner_radius_px.map(|v| v.min(24));
        for custom in self.custom_profiles.values_mut() {
            custom.opacity_pct = custom.opacity_pct.map(|v| v.min(100));
            custom.blur_strength = custom.blur_strength.map(|v| v.min(100));
        }
        self
    }

    fn apply_custom(&mut self, custom: &CustomAppearanceProfile) {
        self.profile = Some(custom.base);
        if let Some(ref s) = custom.surface
            && let Some((_, op, glow)) = surface_presets(s)
        {
            self.opacity_pct = Some(op);
            self.glow_pct = Some(glow);
        }
        if let Some(d) = custom.density {
            self.density = d;
        }
        if let Some(ref r) = custom.radius
            && let Some((px, _)) = radius_px_from_token(r)
        {
            self.corner_radius_px = Some(px);
        }
        if let Some(b) = custom.border {
            self.border = b;
        }
        if let Some(op) = custom.opacity_pct {
            self.opacity_pct = Some(op.min(100));
        }
        if let Some(blur) = custom.blur_strength {
            self.glow_pct = Some(blur.min(100));
        }
    }

    /// Selects one of the 8 built-in profiles (clearing `active_custom`) or an existing custom profile.
    pub fn select_by_name(&mut self, name: &str) -> Result<String, String> {
        let trimmed = name.trim();
        if let Some(p) = AppearanceProfile::from_name(trimmed) {
            self.profile = Some(p);
            self.active_custom = None;
            return Ok(format!("Appearance profile: {}", p.label()));
        }
        if let Some(custom) = self.custom_profiles.get(trimmed).cloned() {
            self.apply_custom(&custom);
            self.active_custom = Some(trimmed.to_string());
            return Ok(format!(
                "Appearance custom profile: {} (base: {})",
                trimmed,
                custom.base.label()
            ));
        }
        Err(format!("unknown appearance profile '{trimmed}'"))
    }

    /// Duplicates a built-in or custom profile into `custom_profiles[new_name]` and activates it.
    pub fn duplicate_profile(&mut self, src: &str, new_name: &str) -> Result<String, String> {
        let target = new_name.trim();
        crate::secure_fs::validate_safe_name(target)?;
        if AppearanceProfile::from_name(target).is_some() {
            return Err(format!("cannot overwrite built-in profile '{target}'"));
        }
        let src_trimmed = src.trim();
        let entry = if src_trimmed.is_empty()
            || src_trimmed.eq_ignore_ascii_case("current")
            || src_trimmed.eq_ignore_ascii_case("active")
        {
            if let Some(ref active) = self.active_custom
                && let Some(existing) = self.custom_profiles.get(active).cloned()
            {
                existing
            } else {
                CustomAppearanceProfile {
                    base: self.profile.unwrap_or(AppearanceProfile::Obsidian),
                    density: Some(self.density),
                    border: Some(self.border),
                    opacity_pct: self.opacity_pct,
                    blur_strength: self.glow_pct,
                    radius: self.corner_radius_px.map(|px| format!("{px}px")),
                    ..Default::default()
                }
            }
        } else if let Some(existing) = self.custom_profiles.get(src_trimmed).cloned() {
            existing
        } else if let Some(builtin) = AppearanceProfile::from_name(src_trimmed) {
            CustomAppearanceProfile {
                base: builtin,
                ..Default::default()
            }
        } else {
            return Err(format!("unknown source profile '{src_trimmed}'"));
        };

        self.apply_custom(&entry);
        self.custom_profiles.insert(target.to_string(), entry);
        self.active_custom = Some(target.to_string());
        Ok(format!("Duplicated '{src_trimmed}' as '{target}'"))
    }

    /// Modifies a design-system setting on `self` (and on the active custom profile, if any).
    pub fn modify_setting(&mut self, key: &str, value: &str) -> Result<String, String> {
        let k = key.trim().to_ascii_lowercase();
        let v = value.trim();
        match k.as_str() {
            "profile" | "base" => {
                let p = AppearanceProfile::from_name(v)
                    .ok_or_else(|| format!("unknown appearance profile '{v}'"))?;
                self.profile = Some(p);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.base = p;
                }
                Ok(format!("Set {k} = {}", p.label()))
            }
            "surface" | "material" => {
                let (canonical, op, glow) = surface_presets(v).ok_or_else(|| {
                    format!(
                        "invalid surface '{v}'; expected opaque|solid|matte|translucent|acrylic|glass"
                    )
                })?;
                self.opacity_pct = Some(op);
                self.glow_pct = Some(glow);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.surface = Some(canonical.to_string());
                    custom.opacity_pct = Some(op);
                    custom.blur_strength = Some(glow);
                }
                Ok(format!("Set surface = {canonical}"))
            }
            "density" => {
                let d = Density::from_name(v).ok_or_else(|| {
                    format!("invalid density '{v}'; expected compact|standard|spacious")
                })?;
                self.density = d;
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.density = Some(d);
                }
                Ok(format!("Set density = {}", d.label()))
            }
            "radius" | "corner-radius" | "corner_radius" => {
                let (px, label) = radius_px_from_token(v).ok_or_else(|| {
                    format!("invalid radius '{v}'; expected sharp|subtle|rounded|pill or 0..=24")
                })?;
                self.corner_radius_px = Some(px);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.radius = Some(label.clone());
                }
                Ok(format!("Set radius = {label}"))
            }
            "border" => {
                let b = BorderWeight::from_name(v).ok_or_else(|| {
                    format!("invalid border '{v}'; expected none|hairline|regular|bold|double")
                })?;
                self.border = b;
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.border = Some(b);
                }
                Ok(format!("Set border = {}", b.label()))
            }
            "shadow" => {
                let lower = v.to_ascii_lowercase();
                let glow = match lower.as_str() {
                    "none" => 0,
                    "soft" => 15,
                    "deep" => 35,
                    "classic" => 10,
                    _ => {
                        return Err(format!(
                            "invalid shadow '{v}'; expected none|soft|deep|classic"
                        ));
                    }
                };
                self.glow_pct = Some(glow);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.shadow = Some(lower.clone());
                }
                Ok(format!("Set shadow = {lower}"))
            }
            "motion" => {
                let lower = v.to_ascii_lowercase();
                if !matches!(lower.as_str(), "none" | "reduced" | "standard" | "fluid") {
                    return Err(format!(
                        "invalid motion '{v}'; expected none|reduced|standard|fluid"
                    ));
                }
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.motion = Some(lower.clone());
                }
                Ok(format!("Set motion = {lower}"))
            }
            "opacity" | "opacity_pct" | "opacity-pct" => {
                let pct = v
                    .strip_suffix('%')
                    .unwrap_or(v)
                    .parse::<u8>()
                    .ok()
                    .filter(|&n| n <= 100)
                    .ok_or_else(|| format!("invalid opacity '{v}'; expected 0..=100"))?;
                self.opacity_pct = Some(pct);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.opacity_pct = Some(pct);
                }
                Ok(format!("Set opacity = {pct}%"))
            }
            "blur" | "blur_strength" | "blur-strength" | "glow" | "glow_pct" => {
                let pct = v
                    .strip_suffix('%')
                    .unwrap_or(v)
                    .parse::<u8>()
                    .ok()
                    .filter(|&n| n <= 100)
                    .ok_or_else(|| format!("invalid blur '{v}'; expected 0..=100"))?;
                self.glow_pct = Some(pct);
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.blur_strength = Some(pct);
                }
                Ok(format!("Set blur = {pct}%"))
            }
            "accent" | "accent_override" => {
                if v.is_empty() {
                    return Err("accent override cannot be empty".to_string());
                }
                if let Some(ref active) = self.active_custom
                    && let Some(custom) = self.custom_profiles.get_mut(active)
                {
                    custom.accent_override = Some(v.to_string());
                }
                Ok(format!("Set accent = {v}"))
            }
            _ => Err(format!("unknown appearance setting '{key}'")),
        }
    }

    /// Resets overrides and `active_custom` back to defaults (`Obsidian`).
    pub fn reset(&mut self) -> String {
        let kept_custom = std::mem::take(&mut self.custom_profiles);
        *self = Self {
            profile: Some(AppearanceProfile::Obsidian),
            custom_profiles: kept_custom,
            active_custom: None,
            ..Self::default()
        };
        "Appearance reset to obsidian defaults".to_string()
    }

    /// Serializes `self` as TOML and writes atomically via `crate::secure_fs::write_private_file`.
    pub fn export_file(&self, path: &Path) -> Result<String, String> {
        validate_file_path(path)?;
        let toml_text = toml::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize appearance config: {e}"))?;
        crate::secure_fs::write_private_file(path, toml_text.as_bytes())
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        Ok(format!("Exported appearance config to {}", path.display()))
    }

    /// Reads TOML from `path`, deserializes `AppearanceConfig`, and replaces `*self`.
    pub fn import_file(&mut self, path: &Path) -> Result<String, String> {
        validate_file_path(path)?;
        let md = std::fs::symlink_metadata(path)
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        if md.file_type().is_symlink() {
            return Err(format!("refusing to import symlink {}", path.display()));
        }
        if !md.is_file() {
            return Err(format!("{} is not a regular file", path.display()));
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let parsed: AppearanceConfig = toml::from_str(&raw)
            .map_err(|e| format!("invalid appearance TOML in {}: {e}", path.display()))?;
        *self = parsed.sanitized();
        if let Some(ref active) = self.active_custom.clone()
            && let Some(custom) = self.custom_profiles.get(active).cloned()
        {
            self.apply_custom(&custom);
        }
        Ok(format!("Imported appearance config from {}", path.display()))
    }

    /// Human-readable summary of all 8 built-in profiles, any `custom_profiles`, and the active selection.
    pub fn list_profiles_text(&self) -> String {
        let active_builtin = self.profile.unwrap_or(AppearanceProfile::Legacy);
        let active_summary = match &self.active_custom {
            Some(c) => format!("{c} (custom)"),
            None => active_builtin.label().to_string(),
        };
        let builtins: Vec<String> = AppearanceProfile::ALL
            .iter()
            .map(|&p| {
                if self.active_custom.is_none() && p == active_builtin {
                    format!("*{}", p.label())
                } else {
                    p.label().to_string()
                }
            })
            .collect();
        let mut out = format!(
            "Active: {active_summary} | Built-in ({}): {}",
            AppearanceProfile::ALL.len(),
            builtins.join(", ")
        );
        if !self.custom_profiles.is_empty() {
            let customs: Vec<String> = self
                .custom_profiles
                .iter()
                .map(|(name, c)| {
                    let mark = if self.active_custom.as_deref() == Some(name.as_str()) {
                        "*"
                    } else {
                        ""
                    };
                    format!("{mark}{name} (base: {})", c.base.label())
                })
                .collect();
            out.push_str(&format!(" | Custom ({}): {}", customs.len(), customs.join(", ")));
        }
        out
    }
}
