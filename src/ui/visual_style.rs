//! Visual Style Modes (`Snazzy` Navigator-Rich Glyphs & `--gui` `Glass` Frosted Translucent Style)
//! and `RawFkeyMode` Function-Key Routing (`F1..F12` in `RawPtyPassthrough`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::BiltermConfig;
use crate::pane::{CompositePane, PaneId};
use crate::ui::design::tron;
use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;
use crate::wm::drawer::BRAILLE_SPINNER_FRAMES;
use crate::wm::WindowManager;

/// Controls how Function Keys (`F1..F12`) behave when a pane is in `RawPtyPassthrough` (`RAW`) mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawFkeyMode {
    /// `F1..F12` trigger Bilterm actions (`F1` Help, `F2` Split, `F5` Zoom, `F7` Drawer, `F12` Raw Toggle, etc.);
    /// `Shift+F1..F12` sends the raw `F1..F12` VT escape sequence through to the child application in the PTY.
    Bilterm,
    /// `F1..F11` are forwarded directly as raw VT escape sequences to the application running in the PTY
    /// (`F12` or `Ctrl+G` exits RAW mode, and `Shift+F12` sends raw `F12` to the PTY);
    /// `Alt+F1..F12` or `Ctrl+X F1..F12` triggers Bilterm actions.
    App,
    /// Uses `Bilterm` F-keys when at the normal shell screen (`!ALT_SCREEN`) and automatically forwards
    /// plain `F1..F11` to the application when `DECSET ?1049` (`ALT_SCREEN`, e.g. `mc`, `htop`, `vim`) is active.
    #[default]
    Auto,
}

impl RawFkeyMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "bilterm" | "wm" | "host" | "mux" | "commands" | "cmd" => Some(Self::Bilterm),
            "app" | "raw" | "pty" | "passthrough" | "pass" | "term" | "terminal" => {
                Some(Self::App)
            }
            "auto" | "smart" | "alt" | "alt-screen" => Some(Self::Auto),
            _ => None,
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::Bilterm => Self::App,
            Self::App => Self::Auto,
            Self::Auto => Self::Bilterm,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Bilterm => "BILTERM",
            Self::App => "RAW-APP",
            Self::Auto => "AUTO",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Bilterm => {
                "F1..F12 run Bilterm actions in RAW mode; Shift+F1..F12 sends raw F-key to terminal app"
            }
            Self::App => {
                "F1..F11 send raw VT keys to terminal app (F12 exits RAW, Shift+F12 sends F12); Alt+F1..F12 or Ctrl+X F1..F12 run Bilterm actions"
            }
            Self::Auto => {
                "F1..F12 run Bilterm actions in shell RAW mode, and auto-forward to ALT_SCREEN apps (mc/htop/vim)"
            }
        }
    }

    /// Returns `true` if plain `F1..F11` in `RawPtyPassthrough` should be intercepted by Bilterm
    /// rather than sent as raw VT escape sequences to the PTY.
    pub fn intercept_plain_fkeys_in_raw(self, is_alternate_screen: bool) -> bool {
        match self {
            Self::Bilterm => true,
            Self::App => false,
            Self::Auto => !is_alternate_screen,
        }
    }
}

/// Parses `"F1"`..`"F12"` or `"1"`..`"12"` into `1..=12`.
pub fn parse_fkey_number(token: &str) -> Option<u8> {
    let t = token.trim().to_ascii_uppercase();
    let num_str = t.strip_prefix('F').unwrap_or(&t);
    let n = num_str.parse::<u8>().ok()?;
    (1..=12).contains(&n).then_some(n)
}

/// Returns a rich Navigator-inspired glyph icon for a shell or foreground command name.
pub fn navigator_process_icon(cmd_or_title: &str) -> &'static str {
    let lower = cmd_or_title.trim().to_ascii_lowercase();
    let head = lower
        .split_whitespace()
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    match head {
        "bash" | "sh" | "dash" | "ksh" | "csh" | "tcsh" => "🐚",
        "zsh" | "fish" | "nu" | "nushell" | "elvish" => "⚡",
        "pwsh" | "powershell" | "cmd" | "cmd.exe" => "⊞",
        "cargo" | "rustc" | "rustup" | "rust-analyzer" => "🦀",
        "python" | "python3" | "ipython" | "pytest" | "uv" | "pip" => "🐍",
        "node" | "npm" | "pnpm" | "yarn" | "bun" | "deno" => "⬢",
        "docker" | "podman" | "containerd" => "🐳",
        "kubectl" | "k9s" | "helm" => "☸",
        "ssh" | "mosh" | "scp" | "sftp" => "🌐",
        "tmux" | "screen" | "zellij" | "bilterm" => "❐",
        "vim" | "nvim" | "vi" | "nano" | "emacs" | "hx" | "helix" => "📝",
        "htop" | "btop" | "top" | "atop" | "glances" | "vmstat" => "🎛",
        "git" | "gh" | "lazygit" | "tig" => "🌿",
        "claude" | "gemini" | "codex" | "aider" | "ollama" => "🤖",
        "make" | "cmake" | "ninja" | "gcc" | "clang" => "⚙",
        _ if lower.contains("ssh") => "🌐",
        _ if lower.contains("cargo") || lower.contains("rust") => "🦀",
        _ if lower.contains("python") => "🐍",
        _ if lower.contains("docker") => "🐳",
        _ => "❖",
    }
}

/// Modern Obsidian / Liquid Glass Slate palette used when `--modern` or `--liquid-glass` is active
/// and the user has not selected a custom theme.
/// Palette for an explicitly selected non-Legacy look, or `None` for Legacy / Obsidian / Liquid
/// (those reuse `modern_obsidian_palette`). Midnight and Arctic are additive new looks.
pub fn profile_palette(profile: crate::ui::design::AppearanceProfile) -> Option<ThemePalette> {
    use crate::ui::design::AppearanceProfile as P;
    let c = |r, g, b| Color::Rgb(r, g, b);
    let (name, p, text, controls) = match profile {
        P::Midnight => (
            "MIDNIGHT",
            [c(16, 18, 34), c(8, 10, 20), c(167, 139, 250), c(167, 139, 250), c(232, 232, 255), c(55, 60, 90),
             c(12, 14, 28), c(167, 139, 250), c(8, 10, 20), c(12, 14, 28), c(196, 198, 230), c(167, 139, 250),
             c(20, 22, 44), c(40, 36, 80), c(5, 6, 14)],
            c(226, 228, 250), c(140, 144, 190),
        ),
        P::Arctic => (
            "ARCTIC",
            [c(255, 255, 255), c(246, 248, 251), c(37, 99, 235), c(37, 99, 235), c(15, 23, 42), c(203, 213, 225),
             c(234, 239, 246), c(37, 99, 235), c(255, 255, 255), c(234, 239, 246), c(51, 65, 85), c(37, 99, 235),
             c(226, 232, 240), c(148, 163, 184), c(226, 232, 240)],
            c(15, 23, 42), c(71, 85, 105),
        ),
        P::Amber => (
            "AMBER",
            [c(28, 20, 10), c(18, 12, 6), c(255, 176, 0), c(92, 64, 24), c(255, 232, 184), c(255, 176, 0),
             c(22, 15, 8), c(255, 176, 0), c(18, 12, 6), c(22, 15, 8), c(230, 196, 140), c(255, 176, 0),
             c(36, 26, 12), c(120, 80, 16), c(10, 7, 3)],
            c(255, 232, 184), c(196, 156, 92),
        ),
        P::Synthwave => (
            "SYNTHWAVE",
            [c(32, 18, 56), c(20, 11, 36), c(5, 217, 232), c(90, 52, 136), c(245, 230, 255), c(255, 42, 133),
             c(26, 14, 46), c(255, 42, 133), c(20, 11, 36), c(26, 14, 46), c(224, 204, 248), c(5, 217, 232),
             c(42, 22, 72), c(140, 28, 110), c(12, 6, 22)],
            c(245, 230, 255), c(178, 148, 216),
        ),
        P::Solarized => (
            "SOLARIZED",
            [c(7, 54, 66), c(0, 43, 54), c(181, 137, 0), c(88, 110, 117), c(238, 232, 213), c(42, 161, 152),
             c(5, 48, 60), c(42, 161, 152), c(0, 43, 54), c(5, 48, 60), c(147, 161, 161), c(181, 137, 0),
             c(12, 62, 76), c(42, 108, 116), c(0, 30, 38)],
            c(238, 232, 213), c(147, 161, 161),
        ),
        P::Tron => return Some(crate::ui::design::tron::tron_palette()),
        _ => return None,
    };
    let [a, b, c1, d, e, f, g, h, i, j, k, l, m, n, o] = p;
    let mut pal = ThemePalette::new(name.to_string(), a, b, c1, d, e, f, g, h, i, j, k, l, m, n, o);
    pal.text_fg = Some(text);
    pal.controls_fg = Some(controls);
    pal.cursor_fg = Some(c1);
    Some(pal)
}

pub fn modern_obsidian_palette(liquid_glass: bool) -> ThemePalette {
    let mut pal = if liquid_glass {
        ThemePalette::new(
            "LIQUID-GLASS".to_string(),
            Color::Rgb(20, 30, 48),     // prompt_surface_bg: translucent deep midnight glass
            Color::Rgb(11, 15, 25),     // window_frame_bg: deep obsidian canvas
            Color::Rgb(110, 231, 255),  // prompt_prefix_fg: luminous ice cyan
            Color::Rgb(125, 211, 252),  // active_border_fg: specular sky refraction
            Color::Rgb(238, 246, 255),  // title_fg: crisp ice white
            Color::Rgb(71, 85, 105),    // inactive_border_fg: muted slate hairline
            Color::Rgb(15, 23, 38),     // oia_bg: minimal dark glass footer
            Color::Rgb(56, 189, 248),   // oia_badge_bg: electric sky badge
            Color::Rgb(11, 15, 25),     // oia_badge_fg
            Color::Rgb(15, 23, 38),     // fkey_bar_bg
            Color::Rgb(203, 213, 225),  // fkey_bar_fg
            Color::Rgb(125, 211, 252),  // fkey_num_fg
            Color::Rgb(22, 36, 58),     // crosshair_bg
            Color::Rgb(56, 189, 248),   // shadow_fg: soft ambient cyan glow
            Color::Rgb(9, 13, 22),      // shadow_bg
        )
    } else {
        ThemePalette::new(
            "MODERN-SLATE".to_string(),
            Color::Rgb(22, 27, 34),     // prompt_surface_bg: clean GitHub/Warp dark card
            Color::Rgb(13, 17, 23),     // window_frame_bg: deep neutral obsidian
            Color::Rgb(88, 166, 255),   // prompt_prefix_fg: clean modern blue
            Color::Rgb(88, 166, 255),   // active_border_fg: crisp hairline accent
            Color::Rgb(230, 237, 243),  // title_fg: crisp neutral white
            Color::Rgb(68, 76, 86),     // inactive_border_fg: subtle slate border
            Color::Rgb(17, 22, 29),     // oia_bg: minimal status strip
            Color::Rgb(88, 166, 255),   // oia_badge_bg
            Color::Rgb(13, 17, 23),     // oia_badge_fg
            Color::Rgb(17, 22, 29),     // fkey_bar_bg
            Color::Rgb(201, 209, 217),  // fkey_bar_fg
            Color::Rgb(88, 166, 255),   // fkey_num_fg
            Color::Rgb(24, 32, 44),     // crosshair_bg
            Color::Rgb(48, 54, 61),     // shadow_fg
            Color::Rgb(9, 12, 16),      // shadow_bg
        )
    };
    if liquid_glass {
        pal.text_fg = Some(Color::Rgb(230, 238, 248));
        pal.controls_fg = Some(Color::Rgb(148, 163, 184));
        pal.cursor_fg = Some(Color::Rgb(110, 231, 255));
    } else {
        pal.text_fg = Some(Color::Rgb(230, 237, 243));
        pal.controls_fg = Some(Color::Rgb(139, 148, 158));
        pal.cursor_fg = Some(Color::Rgb(88, 166, 255));
    }
    pal
}

/// Clean, non-retro minimal glyph set preserving exact cell widths (`6`, `3`, `3`, `3`, `4`)
/// so all mouse hit-boxes and editor cursor columns remain 100% invariant.
pub const fn modern_glyph_set() -> GlyphSet {
    GlyphSet {
        pin_top_bottom: " ↑  ↓ ",
        minimize_badge: " ─ ",
        close_badge: " × ",
        zoom_badge: " □ ",
        resize_corner: "⌟",
        oia_separator: "·",
        prompt_prefix: "  ❯ ",
        prompt_continuation: "  · ",
        status_ready_icon: "●",
        status_run_icon: "◌",
        status_lock_icon: "◈",
        status_alt_icon: "◇",
        status_scroll_icon: "↕",
        output_len_icon: "≡",
        cursor_pos_icon: "⌖",
    }
}

/// Formats the visual style status badge (`CLASSIC`, `✨ SNAZZY`, `💎 GLASS`, `◢ MODERN`, `💧 LIQUID GLASS`, or `◈ TRON`).
pub fn visual_style_label(cfg: &BiltermConfig) -> &'static str {
    if tron::tron_active(cfg) {
        tron::TRON_STYLE_LABEL
    } else if cfg.liquid_glass {
        if cfg.snazzy_mode {
            "💧✨ LIQUID GLASS + SNAZZY (Modern Specular Refraction + Rich Glyphs)"
        } else {
            "💧 LIQUID GLASS (Modern Non-Retro + Specular Refraction)"
        }
    } else if cfg.modern_mode {
        if cfg.glass_mode {
            "◢💎 MODERN + GLASS (Clean Minimal Non-Retro + Frosted Glass)"
        } else if cfg.snazzy_mode {
            "◢✨ MODERN + SNAZZY (Clean Minimal Non-Retro + Rich Glyphs)"
        } else {
            "◢ MODERN (Clean Minimal Non-Retro)"
        }
    } else {
        match (cfg.snazzy_mode, cfg.glass_mode) {
            (false, false) => "CLASSIC (3270/Borland)",
            (true, false) => "✨ SNAZZY (Rich Glyphs)",
            (false, true) => "💎 GLASS (Frosted Translucent)",
            (true, true) => "✨💎 SNAZZY + GLASS (Rich Glyphs + Frosted Glass)",
        }
    }
}

/// Cycles `BiltermConfig` through
/// `Classic -> Snazzy -> Glass -> Snazzy+Glass -> Modern -> Liquid Glass -> TRON -> Classic`
/// or applies an explicit style token (`classic`, `snazzy`, `glass`, `both`, `modern`, `liquid`, `tron`, `toggle`).
/// Picking Classic, Modern or Liquid (or cycling) while TRON is on leaves TRON first.
pub fn apply_visual_style_mode(cfg: &mut BiltermConfig, mode_opt: Option<&str>) -> &'static str {
    let token = mode_opt.map(|s| s.trim().to_ascii_lowercase());
    match token.as_deref() {
        Some("tron" | "tron-on" | "grid" | "light-cycle" | "lightcycle") => {
            tron::tron_on(cfg);
            return visual_style_label(cfg);
        }
        Some("tron-off") => {
            tron::tron_off(cfg);
            return visual_style_label(cfg);
        }
        Some("tron-toggle") => {
            if tron::tron_active(cfg) {
                tron::tron_off(cfg);
            } else {
                tron::tron_on(cfg);
            }
            return visual_style_label(cfg);
        }
        None | Some("" | "cycle" | "next" | "toggle") if tron::tron_active(cfg) => {
            tron::tron_leave(cfg);
            return visual_style_label(cfg);
        }
        Some(
            "classic" | "off" | "reset" | "default" | "retro" | "borland" | "dos" | "modern" | "minimal"
            | "non-retro" | "nonretro" | "clean" | "modern-only" | "modern-toggle" | "liquid"
            | "liquid-glass" | "liquidglass" | "aqua" | "liquid-toggle" | "liquid-glass-toggle"
            | "snazzy-only" | "glass-only",
        ) => tron::tron_leave(cfg),
        _ => {}
    }
    match token.as_deref() {
        None | Some("" | "cycle" | "next" | "toggle") => {
            if cfg.liquid_glass {
                tron::tron_on(cfg);
            } else if cfg.modern_mode {
                cfg.modern_mode = true;
                cfg.liquid_glass = true;
                cfg.glass_mode = true;
            } else {
                match (cfg.snazzy_mode, cfg.glass_mode) {
                    (false, false) => {
                        cfg.snazzy_mode = true;
                        cfg.glass_mode = false;
                    }
                    (true, false) => {
                        cfg.snazzy_mode = false;
                        cfg.glass_mode = true;
                    }
                    (false, true) => {
                        cfg.snazzy_mode = true;
                        cfg.glass_mode = true;
                    }
                    (true, true) => {
                        cfg.snazzy_mode = false;
                        cfg.glass_mode = false;
                        cfg.modern_mode = true;
                        cfg.liquid_glass = false;
                    }
                }
            }
        }
        Some("classic" | "off" | "reset" | "default" | "retro" | "borland" | "dos") => {
            cfg.snazzy_mode = false;
            cfg.glass_mode = false;
            cfg.modern_mode = false;
            cfg.liquid_glass = false;
        }
        Some("modern" | "minimal" | "non-retro" | "nonretro" | "clean") => {
            cfg.modern_mode = true;
            cfg.liquid_glass = false;
        }
        Some("modern-only") => {
            cfg.modern_mode = true;
            cfg.liquid_glass = false;
            cfg.snazzy_mode = false;
            cfg.glass_mode = false;
        }
        Some("modern-off") => {
            cfg.modern_mode = false;
            cfg.liquid_glass = false;
        }
        Some("modern-toggle") => {
            if cfg.modern_mode || cfg.liquid_glass {
                cfg.modern_mode = false;
                cfg.liquid_glass = false;
            } else {
                cfg.modern_mode = true;
            }
        }
        Some("liquid" | "liquid-glass" | "liquidglass" | "aqua") => {
            cfg.modern_mode = true;
            cfg.glass_mode = true;
            cfg.liquid_glass = true;
        }
        Some("liquid-off" | "liquid-glass-off") => {
            cfg.liquid_glass = false;
        }
        Some("liquid-toggle" | "liquid-glass-toggle") => {
            if cfg.liquid_glass {
                cfg.liquid_glass = false;
                cfg.glass_mode = false;
            } else {
                cfg.modern_mode = true;
                cfg.glass_mode = true;
                cfg.liquid_glass = true;
            }
        }
        Some("snazzy" | "herdr" | "rich") => {
            cfg.snazzy_mode = true;
        }
        Some("snazzy-only") => {
            cfg.snazzy_mode = true;
            cfg.glass_mode = false;
            cfg.modern_mode = false;
            cfg.liquid_glass = false;
        }
        Some("snazzy-off") => {
            cfg.snazzy_mode = false;
        }
        Some("snazzy-toggle") => {
            cfg.snazzy_mode = !cfg.snazzy_mode;
        }
        Some("glass" | "frosted" | "aero" | "translucent") => {
            cfg.glass_mode = true;
        }
        Some("glass-only") => {
            cfg.snazzy_mode = false;
            cfg.glass_mode = true;
            cfg.modern_mode = false;
            cfg.liquid_glass = false;
        }
        Some("glass-off") => {
            cfg.glass_mode = false;
            cfg.liquid_glass = false;
        }
        Some("glass-toggle") => {
            cfg.glass_mode = !cfg.glass_mode;
            if !cfg.glass_mode {
                cfg.liquid_glass = false;
            }
        }
        Some("both" | "all" | "snazzy-glass" | "snazzy+glass" | "max" | "ultra") => {
            cfg.snazzy_mode = true;
            cfg.glass_mode = true;
        }
        _ => {}
    }
    visual_style_label(cfg)
}

/// Formats a clean, non-retro window header pill when `cfg.is_modern()` is active.
pub fn format_modern_window_title(
    wm: &WindowManager,
    pane: &CompositePane,
    id: PaneId,
    _z_badge: &str,
    lock_tag: &str,
) -> String {
    let state_glyph = if pane.has_foreground_subcommand() {
        " ◌"
    } else if pane.vt().is_alternate_screen() {
        " ◇"
    } else {
        ""
    };
    let liq = if wm.config().liquid_glass {
        " · 💧"
    } else {
        ""
    };
    if wm.config().snazzy_mode {
        let active_cmd = pane
            .prompt()
            .running_command
            .as_deref()
            .or_else(|| pane.vt().command_blocks().last().map(|b| b.command.as_str()))
            .unwrap_or_else(|| pane.title());
        let icon = navigator_process_icon(active_cmd);
        let spark = wm
            .drawer()
            .sparkline_for(id, pane.vt().total_bytes_processed());
        format!("● ● ●  {id} · {icon} {} {spark}{state_glyph}{lock_tag}{liq}", pane.title())
    } else {
        format!("● ● ●  {id} · {}{state_glyph}{lock_tag}{liq}", pane.title())
    }
}

/// Formats a rich Navigator-inspired window title bar string when `cfg.snazzy_mode` is active.
pub fn format_snazzy_window_title(
    wm: &WindowManager,
    pane: &CompositePane,
    id: PaneId,
    z_badge: &str,
    lock_tag: &str,
) -> String {
    let active_cmd = pane
        .prompt()
        .running_command
        .as_deref()
        .or_else(|| pane.vt().command_blocks().last().map(|b| b.command.as_str()))
        .unwrap_or_else(|| pane.title());
    let icon = navigator_process_icon(active_cmd);
    let spark = wm
        .drawer()
        .sparkline_for(id, pane.vt().total_bytes_processed());
    let spinner = if pane.has_foreground_subcommand() {
        let idx = (pane.vt().total_bytes_processed() / 64) % BRAILLE_SPINNER_FRAMES.len();
        BRAILLE_SPINNER_FRAMES[idx]
    } else if pane.vt().is_alternate_screen() {
        '❖'
    } else {
        '●'
    };
    let glass_tag = if wm.config().glass_mode { " 💎" } else { "" };
    format!(
        "▐ #{id} {icon} {} {spinner} {spark}{lock_tag}{glass_tag} ({z_badge}) ▌",
        pane.title()
    )
}

fn color_to_rgb(c: Color, fallback: (u8, u8, u8)) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (180, 40, 40),
        Color::Green => (40, 190, 80),
        Color::Yellow => (220, 190, 50),
        Color::Blue => (30, 80, 190),
        Color::Magenta => (180, 50, 180),
        Color::Cyan => (50, 200, 210),
        Color::Gray => (160, 160, 160),
        Color::DarkGray => (85, 85, 85),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (85, 255, 120),
        Color::LightYellow => (255, 255, 85),
        Color::LightBlue => (85, 140, 255),
        Color::LightMagenta => (255, 85, 255),
        Color::LightCyan => (85, 255, 255),
        Color::White => (240, 244, 255),
        _ => fallback,
    }
}

/// Renders a translucent frosted glass background across `rect` in `buf`:
/// blends the underlying terminal cell colors with `base_bg` (`~70%` glass tint + `30%` underlying
/// canvas bleed-through) and applies a subtle vertical/diagonal specular glass sheen.
pub fn apply_frosted_glass_surface(
    buf: &mut Buffer,
    rect: Rect,
    base_bg: Color,
    palette: &ThemePalette,
) {
    let bounds = rect.intersection(*buf.area());
    if bounds.width == 0 || bounds.height == 0 {
        return;
    }
    let glass_rgb = color_to_rgb(base_bg, (16, 28, 56));
    let tint_rgb = color_to_rgb(palette.active_border_fg, (100, 210, 255));
    let h_max = bounds.height.saturating_sub(1).max(1) as i32;
    let w_max = bounds.width.saturating_sub(1).max(1) as i32;

    for y in bounds.y..bounds.y + bounds.height {
        let rel_y = (y - bounds.y) as i32;
        // Top-to-bottom specular sheen (+16 at top edge, tapering to -8 at bottom)
        let v_sheen = 16 - (rel_y * 24) / h_max;
        for x in bounds.x..bounds.x + bounds.width {
            let rel_x = (x - bounds.x) as i32;
            let h_sheen = 6 - (rel_x * 12) / w_max;
            let sheen = v_sheen + h_sheen;
            if let Some(cell) = buf.cell_mut((x, y)) {
                let under_bg = color_to_rgb(cell.bg, glass_rgb);
                let under_fg = color_to_rgb(cell.fg, under_bg);
                let has_glyph = !cell.symbol().trim().is_empty();
                let sample = if has_glyph {
                    (
                        ((u16::from(under_bg.0) * 2 + u16::from(under_fg.0)) / 3) as u8,
                        ((u16::from(under_bg.1) * 2 + u16::from(under_fg.1)) / 3) as u8,
                        ((u16::from(under_bg.2) * 2 + u16::from(under_fg.2)) / 3) as u8,
                    )
                } else {
                    under_bg
                };

                // Blend: 66% glass surface + 24% underlying canvas + 10% ice specular tint + sheen
                let r = ((i32::from(glass_rgb.0) * 66
                    + i32::from(sample.0) * 24
                    + i32::from(tint_rgb.0) * 10)
                    / 100
                    + sheen)
                    .clamp(0, 255) as u8;
                let g = ((i32::from(glass_rgb.1) * 66
                    + i32::from(sample.1) * 24
                    + i32::from(tint_rgb.1) * 10)
                    / 100
                    + sheen)
                    .clamp(0, 255) as u8;
                let b = ((i32::from(glass_rgb.2) * 66
                    + i32::from(sample.2) * 24
                    + i32::from(tint_rgb.2) * 10)
                    / 100
                    + sheen)
                    .clamp(0, 255) as u8;

                cell.reset();
                cell.set_symbol(" ");
                cell.set_bg(Color::Rgb(r, g, b));
            }
        }
    }
}

/// Applies a frosted-glass post-pass to an already-rendered panel (`rect`):
/// adds a subtle specular reflection along the top/left border and a translucent glass gradient
/// to interior background cells matching `flat_bg`.
pub fn polish_glass_panel_rect(
    buf: &mut Buffer,
    rect: Rect,
    flat_bg: Color,
    palette: &ThemePalette,
    active: bool,
) {
    let bounds = rect.intersection(*buf.area());
    if bounds.width < 2 || bounds.height < 2 {
        return;
    }
    let base = color_to_rgb(flat_bg, (16, 28, 56));
    let border_rgb = color_to_rgb(
        if active {
            palette.active_border_fg
        } else {
            palette.inactive_border_fg
        },
        (120, 220, 255),
    );
    let h_max = bounds.height.saturating_sub(1).max(1) as i32;
    let w_max = bounds.width.saturating_sub(1).max(1) as i32;

    for y in bounds.y..bounds.y + bounds.height {
        let rel_y = (y - bounds.y) as i32;
        let v_sheen = 18 - (rel_y * 26) / h_max;
        for x in bounds.x..bounds.x + bounds.width {
            let rel_x = (x - bounds.x) as i32;
            let h_sheen = 7 - (rel_x * 14) / w_max;
            let is_top_or_left = y == bounds.y || x == bounds.x;
            if let Some(cell) = buf.cell_mut((x, y)) {
                let cur_bg = color_to_rgb(cell.bg, base);
                let dist = (i32::from(cur_bg.0) - i32::from(base.0)).abs()
                    + (i32::from(cur_bg.1) - i32::from(base.1)).abs()
                    + (i32::from(cur_bg.2) - i32::from(base.2)).abs();
                if dist <= 48 {
                    let sheen = v_sheen + h_sheen;
                    let r = ((i32::from(cur_bg.0) * 88 + i32::from(border_rgb.0) * 12) / 100
                        + sheen)
                        .clamp(0, 255) as u8;
                    let g = ((i32::from(cur_bg.1) * 88 + i32::from(border_rgb.1) * 12) / 100
                        + sheen)
                        .clamp(0, 255) as u8;
                    let b = ((i32::from(cur_bg.2) * 88 + i32::from(border_rgb.2) * 12) / 100
                        + sheen)
                        .clamp(0, 255) as u8;
                    cell.set_bg(Color::Rgb(r, g, b));
                }
                if is_top_or_left && active {
                    let cur_fg = color_to_rgb(cell.fg, border_rgb);
                    let hr = (i32::from(cur_fg.0) + 36).clamp(0, 255) as u8;
                    let hg = (i32::from(cur_fg.1) + 36).clamp(0, 255) as u8;
                    let hb = (i32::from(cur_fg.2) + 42).clamp(0, 255) as u8;
                    cell.set_fg(Color::Rgb(hr, hg, hb));
                } else if (y == bounds.y + bounds.height - 1 || x == bounds.x + bounds.width - 1)
                    && active
                    && palette.is_liquid_glass()
                {
                    // Bottom/right chromatic refraction edge (cool indigo/cyan dispersion)
                    cell.set_fg(Color::Rgb(96, 165, 250));
                }
            }
        }
    }
}

/// Renders a soft 1-cell ambient outer glow halo around `rect` inside `bounds` when `liquid_glass`
/// is active (replaces the heavy DOS `░`/`█` checkerboard drop-shadow with a subtle translucent glow).
pub fn render_liquid_ambient_halo(
    buf: &mut Buffer,
    rect: Rect,
    bounds: Rect,
    palette: &ThemePalette,
) {
    let clamped = bounds.intersection(*buf.area());
    if rect.width == 0 || rect.height == 0 || clamped.width == 0 || clamped.height == 0 {
        return;
    }
    let glow_rgb = color_to_rgb(palette.active_border_fg, (88, 196, 255));
    let x0 = rect.x.saturating_sub(1).max(clamped.x);
    let y0 = rect.y.saturating_sub(1).max(clamped.y);
    let x1 = (rect.x + rect.width + 1).min(clamped.x + clamped.width);
    let y1 = (rect.y + rect.height + 1).min(clamped.y + clamped.height);

    for y in y0..y1 {
        for x in x0..x1 {
            let inside =
                x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height;
            if inside {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                let bg = color_to_rgb(cell.bg, (11, 15, 25));
                let r = ((u16::from(bg.0) * 82 + u16::from(glow_rgb.0) * 18) / 100) as u8;
                let g = ((u16::from(bg.1) * 82 + u16::from(glow_rgb.1) * 18) / 100) as u8;
                let b = ((u16::from(bg.2) * 82 + u16::from(glow_rgb.2) * 18) / 100) as u8;
                cell.set_bg(Color::Rgb(r, g, b));
            }
        }
    }
}
