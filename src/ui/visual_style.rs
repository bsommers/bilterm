//! Visual Style Modes (`Snazzy` `herdr`-Rich Glyphs & `--gui` `Glass` Frosted Translucent Style)
//! and `RawFkeyMode` Function-Key Routing (`F1..F12` in `RawPtyPassthrough`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::BiltermConfig;
use crate::pane::{CompositePane, PaneId};
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

/// Returns a rich `herdr`-inspired glyph icon for a shell or foreground command name.
pub fn herdr_process_icon(cmd_or_title: &str) -> &'static str {
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

/// Formats the visual style status badge (`✨ SNAZZY`, `💎 GLASS`, `✨💎 SNAZZY+GLASS`, or `CLASSIC`).
pub fn visual_style_label(cfg: &BiltermConfig) -> &'static str {
    match (cfg.snazzy_mode, cfg.glass_mode) {
        (false, false) => "CLASSIC (3270/Borland)",
        (true, false) => "✨ SNAZZY (herdr Rich Glyphs)",
        (false, true) => "💎 GLASS (Frosted Translucent)",
        (true, true) => "✨💎 SNAZZY + GLASS (herdr Glyphs + Frosted Glass)",
    }
}

/// Cycles `BiltermConfig` through `Classic -> Snazzy -> Glass -> Snazzy+Glass -> Classic`
/// or applies an explicit style token (`classic`, `snazzy`, `glass`, `both`, `toggle`).
pub fn apply_visual_style_mode(cfg: &mut BiltermConfig, mode_opt: Option<&str>) -> &'static str {
    match mode_opt.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("" | "cycle" | "next" | "toggle") => {
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
                }
            }
        }
        Some("classic" | "off" | "reset" | "default" | "retro") => {
            cfg.snazzy_mode = false;
            cfg.glass_mode = false;
        }
        Some("snazzy" | "herdr" | "rich") => {
            cfg.snazzy_mode = true;
        }
        Some("snazzy-only") => {
            cfg.snazzy_mode = true;
            cfg.glass_mode = false;
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
        }
        Some("glass-off") => {
            cfg.glass_mode = false;
        }
        Some("glass-toggle") => {
            cfg.glass_mode = !cfg.glass_mode;
        }
        Some("both" | "all" | "snazzy-glass" | "snazzy+glass" | "max" | "ultra") => {
            cfg.snazzy_mode = true;
            cfg.glass_mode = true;
        }
        _ => {}
    }
    visual_style_label(cfg)
}

/// Formats a rich `herdr`-inspired window title bar string when `cfg.snazzy_mode` is active.
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
    let icon = herdr_process_icon(active_cmd);
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
                }
            }
        }
    }
}
