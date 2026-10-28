use std::path::Path;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::ui::theme::ThemePalette;

/// Optional typography preferences bundled inside an open theme file (`THM-01`, `THM-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ThemeTypographyProfile {
    pub font_family: Option<String>,
    pub font_size_pt: Option<u16>,
    pub font_scale_pct: Option<u16>,
}

/// Universal 16-color + semantic open terminal color scheme (`THM-01`).
///
/// Compatible with `mbadolato/iterm2-color-schemes` (`.itermcolors` XML plists),
/// Alacritty TOML, WezTerm/Ghostty/Bilterm Open TOML, and Base16 YAML/TOML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenColorScheme {
    pub name: String,
    pub foreground: Color,
    pub background: Color,
    pub bold: Option<Color>,
    pub cursor: Color,
    pub cursor_text: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub badge: Option<Color>,
    pub link: Option<Color>,
    pub ansi: [Color; 16],
    pub typography: ThemeTypographyProfile,
}

impl Default for OpenColorScheme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            foreground: Color::Rgb(220, 224, 232),
            background: Color::Rgb(24, 26, 36),
            bold: None,
            cursor: Color::Rgb(0, 255, 102),
            cursor_text: Color::Rgb(0, 0, 0),
            selection_bg: Color::Rgb(60, 72, 104),
            selection_fg: Color::Rgb(255, 255, 255),
            badge: None,
            link: None,
            ansi: default_ansi_palette(),
            typography: ThemeTypographyProfile::default(),
        }
    }
}

const fn default_ansi_palette() -> [Color; 16] {
    [
        Color::Rgb(0, 0, 0),
        Color::Rgb(205, 49, 49),
        Color::Rgb(13, 188, 121),
        Color::Rgb(229, 229, 16),
        Color::Rgb(36, 114, 200),
        Color::Rgb(188, 63, 188),
        Color::Rgb(17, 168, 205),
        Color::Rgb(229, 229, 229),
        Color::Rgb(102, 102, 102),
        Color::Rgb(241, 76, 76),
        Color::Rgb(35, 209, 139),
        Color::Rgb(245, 245, 67),
        Color::Rgb(59, 142, 234),
        Color::Rgb(214, 112, 214),
        Color::Rgb(41, 184, 219),
        Color::Rgb(255, 255, 255),
    ]
}

/// Converts any `ratatui::style::Color` to `(r, g, b)` bytes.
pub const fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::Gray => (204, 204, 204),
        Color::DarkGray => (102, 102, 102),
        Color::LightRed => (241, 76, 76),
        Color::LightGreen => (35, 209, 139),
        Color::LightYellow => (245, 245, 67),
        Color::LightBlue => (59, 142, 234),
        Color::LightMagenta => (214, 112, 214),
        Color::LightCyan => (41, 184, 219),
        Color::White | Color::Reset => (255, 255, 255),
        Color::Indexed(idx) => (idx, idx, idx),
    }
}

pub fn format_hex_color(c: Color) -> String {
    let (r, g, b) = color_to_rgb(c);
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn parse_flexible_hex(s: &str) -> Option<Color> {
    let trimmed = s
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .strip_prefix('#')
        .or_else(|| s.trim().strip_prefix("0x"))
        .or_else(|| s.trim().strip_prefix("0X"))
        .unwrap_or_else(|| s.trim().trim_matches('"').trim_matches('\''));
    if trimmed.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).ok()?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).ok()?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Computes WCAG 2.1 relative luminance (`0.0..=1.0`) for a `Color`.
pub fn luminance(c: Color) -> f32 {
    fn channel_lum(v: u8) -> f32 {
        let s = f32::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    let (r, g, b) = color_to_rgb(c);
    0.2126 * channel_lum(r) + 0.7152 * channel_lum(g) + 0.0722 * channel_lum(b)
}

/// Computes WCAG 2.1 contrast ratio (`1.0..=21.0`) between two colors.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let la = luminance(a);
    let lb = luminance(b);
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

fn blend_rgb(a: Color, b: Color, weight_b: f32) -> Color {
    let (ar, ag, ab) = color_to_rgb(a);
    let (br, bg, bb) = color_to_rgb(b);
    let w = weight_b.clamp(0.0, 1.0);
    let inv = 1.0 - w;
    Color::Rgb(
        (f32::from(ar) * inv + f32::from(br) * w).round() as u8,
        (f32::from(ag) * inv + f32::from(bg) * w).round() as u8,
        (f32::from(ab) * inv + f32::from(bb) * w).round() as u8,
    )
}

fn ensure_min_contrast(fg: Color, bg: Color, fallback: Color, min_ratio: f32) -> Color {
    if contrast_ratio(fg, bg) >= min_ratio {
        return fg;
    }
    if contrast_ratio(fallback, bg) >= min_ratio {
        return fallback;
    }
    if contrast_ratio(Color::White, bg) >= contrast_ratio(Color::Black, bg) {
        Color::White
    } else {
        Color::Black
    }
}

impl OpenColorScheme {
    /// Creates an [`OpenColorScheme`] from a built-in [`crate::ui::theme::ThemePreset`].
    pub fn from_preset(preset: crate::ui::theme::ThemePreset) -> Self {
        let pal = preset.palette();
        let mut ansi = default_ansi_palette();
        ansi[0] = pal.window_frame_bg;
        ansi[4] = pal.oia_bg;
        ansi[8] = pal.inactive_border_fg;
        ansi[9] = pal.fkey_num_fg;
        ansi[10] = pal.prompt_prefix_fg;
        ansi[11] = pal.title_fg;
        ansi[14] = pal.active_border_fg;
        ansi[15] = pal.fkey_bar_fg;
        Self {
            name: pal.name,
            foreground: pal.fkey_bar_fg,
            background: pal.window_frame_bg,
            bold: Some(pal.title_fg),
            cursor: pal.active_border_fg,
            cursor_text: pal.oia_badge_fg,
            selection_bg: pal.crosshair_bg,
            selection_fg: pal.title_fg,
            badge: Some(pal.oia_badge_bg),
            link: Some(pal.active_border_fg),
            ansi,
            typography: ThemeTypographyProfile::default(),
        }
    }

    /// Synthesizes a contrast-verified Borland Turbo Vision / IBM 3270 [`ThemePalette`]
    /// from this 16-color open terminal scheme (`THM-01`).
    pub fn to_theme_palette(&self) -> ThemePalette {
        let bg_lum = luminance(self.background);
        let is_dark = bg_lum < 0.35;
        let tint_target = if is_dark {
            Color::Rgb(255, 255, 255)
        } else {
            Color::Rgb(0, 0, 0)
        };

        let window_frame_bg = self.background;
        let prompt_surface_bg = blend_rgb(self.background, tint_target, 0.06);
        let oia_bg = blend_rgb(self.background, self.selection_bg, 0.35);
        let crosshair_bg = blend_rgb(self.background, self.selection_bg, 0.25);
        let fkey_bar_bg = blend_rgb(self.background, tint_target, 0.08);

        let raw_active = if contrast_ratio(self.ansi[14], window_frame_bg) >= 3.0 {
            self.ansi[14]
        } else if contrast_ratio(self.ansi[10], window_frame_bg) >= 3.0 {
            self.ansi[10]
        } else {
            self.cursor
        };
        let active_border_fg =
            ensure_min_contrast(raw_active, window_frame_bg, self.foreground, 3.0);

        let raw_inactive = if contrast_ratio(self.ansi[8], window_frame_bg) >= 2.0 {
            self.ansi[8]
        } else {
            self.ansi[4]
        };
        let inactive_border_fg = blend_rgb(raw_inactive, self.foreground, 0.25);

        let raw_title = self.bold.unwrap_or(self.ansi[11]);
        let title_fg = ensure_min_contrast(raw_title, window_frame_bg, self.foreground, 3.0);
        let prompt_prefix_fg =
            ensure_min_contrast(self.ansi[10], prompt_surface_bg, active_border_fg, 3.0);

        let oia_badge_bg = self.badge.unwrap_or(self.ansi[14]);
        let contrast_black = contrast_ratio(Color::Black, oia_badge_bg);
        let contrast_white = contrast_ratio(Color::White, oia_badge_bg);
        let oia_badge_fg = if contrast_black >= contrast_white {
            Color::Black
        } else {
            Color::White
        };

        let fkey_bar_fg = ensure_min_contrast(self.foreground, fkey_bar_bg, Color::White, 3.0);
        let fkey_num_fg = ensure_min_contrast(self.ansi[9], fkey_bar_bg, active_border_fg, 3.0);
        let shadow_fg = self.ansi[8];
        let shadow_bg = blend_rgb(self.background, Color::Black, 0.55);

        ThemePalette {
            name: self.name.clone(),
            prompt_surface_bg,
            window_frame_bg,
            active_border_fg,
            inactive_border_fg,
            title_fg,
            prompt_prefix_fg,
            oia_bg,
            oia_badge_bg,
            oia_badge_fg,
            crosshair_bg,
            fkey_bar_bg,
            fkey_bar_fg,
            fkey_num_fg,
            shadow_fg,
            shadow_bg,
        }
    }
}

/// Parses an Apple / iTerm2 `.itermcolors` XML Plist string (`mbadolato/iterm2-color-schemes`)
/// without external XML crates (`THM-01`).
pub fn parse_itermcolors_xml(name: &str, xml: &str) -> Result<OpenColorScheme, String> {
    if !xml.contains("<plist") && !xml.contains("<dict>") {
        return Err("Not a valid .itermcolors XML plist".to_string());
    }
    let mut scheme = OpenColorScheme {
        name: name.to_string(),
        ..OpenColorScheme::default()
    };
    let mut found_any = false;

    for idx in 0..16 {
        let key = format!("Ansi {idx} Color");
        if let Some(c) = extract_plist_color(xml, &key) {
            scheme.ansi[idx] = c;
            found_any = true;
        }
    }
    if let Some(c) = extract_plist_color(xml, "Foreground Color") {
        scheme.foreground = c;
        found_any = true;
    }
    if let Some(c) = extract_plist_color(xml, "Background Color") {
        scheme.background = c;
        found_any = true;
    }
    scheme.bold = extract_plist_color(xml, "Bold Color");
    if let Some(c) = extract_plist_color(xml, "Cursor Color") {
        scheme.cursor = c;
        found_any = true;
    }
    if let Some(c) = extract_plist_color(xml, "Cursor Text Color") {
        scheme.cursor_text = c;
    }
    if let Some(c) = extract_plist_color(xml, "Selection Color") {
        scheme.selection_bg = c;
    }
    if let Some(c) = extract_plist_color(xml, "Selected Text Color") {
        scheme.selection_fg = c;
    }
    scheme.badge = extract_plist_color(xml, "Badge Color");
    scheme.link = extract_plist_color(xml, "Link Color");

    if !found_any {
        return Err("No iTerm2 color entries found in XML plist".to_string());
    }
    Ok(scheme)
}

fn extract_plist_color(xml: &str, key_name: &str) -> Option<Color> {
    let needle = format!("<key>{key_name}</key>");
    let start = xml.find(&needle)? + needle.len();
    let rest = &xml[start..];
    let dict_start = rest.find("<dict>")? + "<dict>".len();
    let dict_rest = &rest[dict_start..];
    let dict_end = dict_rest.find("</dict>")?;
    let dict_body = &dict_rest[..dict_end];

    let r = extract_plist_component(dict_body, "Red Component")?;
    let g = extract_plist_component(dict_body, "Green Component")?;
    let b = extract_plist_component(dict_body, "Blue Component")?;
    Some(Color::Rgb(r, g, b))
}

fn extract_plist_component(dict_body: &str, comp_key: &str) -> Option<u8> {
    let needle = format!("<key>{comp_key}</key>");
    let pos = dict_body.find(&needle)? + needle.len();
    let after = dict_body[pos..].trim_start();
    let raw_val = if let Some(r) = after.strip_prefix("<real>") {
        let end = r.find("</real>")?;
        &r[..end]
    } else {
        let i = after.strip_prefix("<integer>")?;
        let end = i.find("</integer>")?;
        &i[..end]
    };
    let v: f64 = raw_val.trim().parse().ok()?;
    let scaled = if v > 1.0 {
        v.clamp(0.0, 255.0)
    } else {
        (v.clamp(0.0, 1.0) * 255.0).round()
    };
    Some(scaled as u8)
}

/// Auto-detects and parses `.itermcolors` XML, Alacritty TOML, WezTerm/Ghostty/Bilterm Open TOML,
/// or Base16 YAML/TOML into an [`OpenColorScheme`] (`THM-01`).
pub fn parse_open_theme_str(name: &str, content: &str) -> Result<OpenColorScheme, String> {
    let trimmed = content.trim();
    if trimmed.contains("<plist") || trimmed.contains("Red Component") {
        return parse_itermcolors_xml(name, trimmed);
    }
    if let Ok(val) = toml::from_str::<toml::Value>(trimmed)
        && let Some(tbl) = val.as_table()
        && !tbl.is_empty()
    {
        return parse_toml_theme(name, tbl);
    }
    if trimmed.contains("base00") || trimmed.contains("scheme:") || trimmed.contains("palette:") {
        return parse_yaml_or_keyval_theme(name, trimmed);
    }
    Err("Unrecognized theme format (expected .itermcolors XML, Alacritty/WezTerm/Open TOML, or Base16 YAML)".to_string())
}

fn parse_toml_theme(
    fallback_name: &str,
    tbl: &toml::map::Map<String, toml::Value>,
) -> Result<OpenColorScheme, String> {
    let mut scheme = OpenColorScheme {
        name: tbl
            .get("name")
            .or_else(|| tbl.get("scheme"))
            .and_then(|v| v.as_str())
            .unwrap_or(fallback_name)
            .to_string(),
        ..OpenColorScheme::default()
    };

    if let Some(typo) = tbl.get("typography").and_then(|v| v.as_table()) {
        scheme.typography.font_family = typo
            .get("font_family")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        scheme.typography.font_size_pt = typo
            .get("font_size_pt")
            .and_then(|v| v.as_integer())
            .and_then(|v| u16::try_from(v).ok());
        scheme.typography.font_scale_pct = typo
            .get("font_scale_pct")
            .and_then(|v| v.as_integer())
            .and_then(|v| u16::try_from(v).ok());
    }

    // 1. Alacritty `[colors.primary]`, `[colors.normal]`, `[colors.bright]`
    if let Some(colors) = tbl.get("colors").and_then(|v| v.as_table()) {
        if let Some(prim) = colors.get("primary").and_then(|v| v.as_table()) {
            if let Some(bg) = get_toml_color(prim, "background") {
                scheme.background = bg;
            }
            if let Some(fg) = get_toml_color(prim, "foreground") {
                scheme.foreground = fg;
            }
        }
        if let Some(cur) = colors.get("cursor").and_then(|v| v.as_table()) {
            if let Some(c) = get_toml_color(cur, "cursor") {
                scheme.cursor = c;
            }
            if let Some(t) = get_toml_color(cur, "text") {
                scheme.cursor_text = t;
            }
        }
        if let Some(sel) = colors.get("selection").and_then(|v| v.as_table()) {
            if let Some(bg) = get_toml_color(sel, "background") {
                scheme.selection_bg = bg;
            }
            if let Some(fg) = get_toml_color(sel, "text").or_else(|| get_toml_color(sel, "foreground"))
            {
                scheme.selection_fg = fg;
            }
        }
        if let Some(norm) = colors.get("normal").and_then(|v| v.as_table()) {
            apply_named_ansi_block(norm, &mut scheme.ansi, 0);
        }
        if let Some(bright) = colors.get("bright").and_then(|v| v.as_table()) {
            apply_named_ansi_block(bright, &mut scheme.ansi, 8);
        }
        return Ok(scheme);
    }

    // 2. Base16 TOML (`base00`..`base0F`)
    if tbl.contains_key("base00") && tbl.contains_key("base05") {
        let mut kv = std::collections::BTreeMap::new();
        for (k, v) in tbl {
            if let Some(s) = v.as_str() {
                kv.insert(k.clone(), s.to_string());
            }
        }
        apply_base16_map(&kv, &mut scheme)?;
        return Ok(scheme);
    }

    // 3. WezTerm / Ghostty / Bilterm Open TOML (`foreground`, `background`, `ansi = [...]`, `brights = [...]`)
    let root = tbl
        .get("colors")
        .and_then(|v| v.as_table())
        .unwrap_or(tbl);
    if let Some(fg) = get_toml_color(root, "foreground") {
        scheme.foreground = fg;
    }
    if let Some(bg) = get_toml_color(root, "background") {
        scheme.background = bg;
    }
    scheme.bold = get_toml_color(root, "bold");
    if let Some(c) = get_toml_color(root, "cursor_bg").or_else(|| get_toml_color(root, "cursor")) {
        scheme.cursor = c;
    }
    if let Some(ct) =
        get_toml_color(root, "cursor_fg").or_else(|| get_toml_color(root, "cursor_text"))
    {
        scheme.cursor_text = ct;
    }
    if let Some(sbg) = get_toml_color(root, "selection_bg") {
        scheme.selection_bg = sbg;
    }
    if let Some(sfg) = get_toml_color(root, "selection_fg") {
        scheme.selection_fg = sfg;
    }
    scheme.badge = get_toml_color(root, "badge");
    scheme.link = get_toml_color(root, "link");

    if let Some(arr) = root.get("ansi").and_then(|v| v.as_array()) {
        for (i, item) in arr.iter().take(16).enumerate() {
            if let Some(c) = item.as_str().and_then(parse_flexible_hex) {
                scheme.ansi[i] = c;
            }
        }
    }
    if let Some(arr) = root.get("brights").and_then(|v| v.as_array()) {
        for (i, item) in arr.iter().take(8).enumerate() {
            if let Some(c) = item.as_str().and_then(parse_flexible_hex) {
                scheme.ansi[8 + i] = c;
            }
        }
    }
    Ok(scheme)
}

fn get_toml_color(tbl: &toml::map::Map<String, toml::Value>, key: &str) -> Option<Color> {
    tbl.get(key)?.as_str().and_then(parse_flexible_hex)
}

fn apply_named_ansi_block(
    tbl: &toml::map::Map<String, toml::Value>,
    ansi: &mut [Color; 16],
    offset: usize,
) {
    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    for (idx, key) in NAMES.iter().enumerate() {
        if let Some(c) = get_toml_color(tbl, key) {
            ansi[offset + idx] = c;
        }
    }
}

fn parse_yaml_or_keyval_theme(name: &str, text: &str) -> Result<OpenColorScheme, String> {
    let mut kv = std::collections::BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or(raw_line).trim();
        let line = if let Some(comment_idx) = raw_line.find(':') {
            let key = raw_line[..comment_idx].trim().trim_matches('"').trim_matches('\'');
            let val_part = raw_line[comment_idx + 1..].trim();
            let val = val_part
                .split(" #")
                .next()
                .unwrap_or(val_part)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            kv.insert(key.to_ascii_lowercase(), val.to_string());
            continue;
        } else {
            line
        };
        let _ = line;
    }
    let mut scheme = OpenColorScheme {
        name: kv
            .get("scheme")
            .or_else(|| kv.get("name"))
            .cloned()
            .unwrap_or_else(|| name.to_string()),
        ..OpenColorScheme::default()
    };
    apply_base16_map(&kv, &mut scheme)?;
    Ok(scheme)
}

fn apply_base16_map(
    kv: &std::collections::BTreeMap<String, String>,
    scheme: &mut OpenColorScheme,
) -> Result<(), String> {
    let get = |k: &str| kv.get(k).and_then(|s| parse_flexible_hex(s));
    let bg = get("base00").ok_or_else(|| "Base16 scheme missing base00".to_string())?;
    let fg = get("base05").ok_or_else(|| "Base16 scheme missing base05".to_string())?;
    scheme.background = bg;
    scheme.foreground = fg;
    scheme.selection_bg = get("base02").unwrap_or(scheme.selection_bg);
    scheme.selection_fg = get("base06").unwrap_or(fg);
    scheme.cursor = get("base06").unwrap_or(fg);
    scheme.cursor_text = bg;

    let slots = [
        ("base00", 0),
        ("base08", 1),
        ("base0b", 2),
        ("base0a", 3),
        ("base0d", 4),
        ("base0e", 5),
        ("base0c", 6),
        ("base05", 7),
        ("base03", 8),
        ("base09", 9),
        ("base01", 10),
        ("base02", 11),
        ("base04", 12),
        ("base06", 13),
        ("base0f", 14),
        ("base07", 15),
    ];
    for (key, idx) in slots {
        if let Some(c) = get(key).or_else(|| get(&key.to_ascii_uppercase())) {
            scheme.ansi[idx] = c;
        }
    }
    Ok(())
}

/// Exports any [`OpenColorScheme`] to open Bilterm/WezTerm-compatible TOML (`THM-01`).
pub fn export_open_theme_toml(scheme: &OpenColorScheme) -> String {
    let mut out = String::new();
    out.push_str(&format!("name = {:?}\n", scheme.name));
    out.push_str(&format!(
        "foreground = {:?}\n",
        format_hex_color(scheme.foreground)
    ));
    out.push_str(&format!(
        "background = {:?}\n",
        format_hex_color(scheme.background)
    ));
    if let Some(bold) = scheme.bold {
        out.push_str(&format!("bold = {:?}\n", format_hex_color(bold)));
    }
    out.push_str(&format!(
        "cursor_bg = {:?}\n",
        format_hex_color(scheme.cursor)
    ));
    out.push_str(&format!(
        "cursor_fg = {:?}\n",
        format_hex_color(scheme.cursor_text)
    ));
    out.push_str(&format!(
        "selection_bg = {:?}\n",
        format_hex_color(scheme.selection_bg)
    ));
    out.push_str(&format!(
        "selection_fg = {:?}\n",
        format_hex_color(scheme.selection_fg)
    ));
    if let Some(badge) = scheme.badge {
        out.push_str(&format!("badge = {:?}\n", format_hex_color(badge)));
    }
    if let Some(link) = scheme.link {
        out.push_str(&format!("link = {:?}\n", format_hex_color(link)));
    }
    let ansi_strs: Vec<String> = scheme.ansi[0..8]
        .iter()
        .map(|c| format!("{:?}", format_hex_color(*c)))
        .collect();
    let bright_strs: Vec<String> = scheme.ansi[8..16]
        .iter()
        .map(|c| format!("{:?}", format_hex_color(*c)))
        .collect();
    out.push_str(&format!("ansi = [{}]\n", ansi_strs.join(", ")));
    out.push_str(&format!("brights = [{}]\n", bright_strs.join(", ")));

    if scheme.typography.font_family.is_some()
        || scheme.typography.font_size_pt.is_some()
        || scheme.typography.font_scale_pct.is_some()
    {
        out.push_str("\n[typography]\n");
        if let Some(ref fam) = scheme.typography.font_family {
            out.push_str(&format!("font_family = {:?}\n", fam));
        }
        if let Some(sz) = scheme.typography.font_size_pt {
            out.push_str(&format!("font_size_pt = {sz}\n"));
        }
        if let Some(sc) = scheme.typography.font_scale_pct {
            out.push_str(&format!("font_scale_pct = {sc}\n"));
        }
    }
    out
}

/// Loads an [`OpenColorScheme`] from a file on disk (`.itermcolors`, `.toml`, `.yaml`, `.yml`).
pub fn load_open_theme_file(path: &Path) -> Result<OpenColorScheme, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read theme file {}: {e}", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom-theme");
    parse_open_theme_str(stem, &content)
}

/// Scans `dir` for `.itermcolors`, `.toml`, `.yaml`, and `.yml` theme files sorted by name (`THM-01`).
pub fn discover_themes_in_dir(dir: &Path) -> Vec<OpenColorScheme> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "itermcolors" | "toml" | "yaml" | "yml"
                    )
                })
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| load_open_theme_file(&p).ok())
        .collect()
}
