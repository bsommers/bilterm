//! TRON look: black canvas, neon-cyan light-cycle borders, orange "program" accent.
//!
//! TRON is an ordinary appearance profile (`AppearanceProfile::Tron`, palette `TRON`), so
//! `:appearance tron` and `[appearance].profile = "tron"` work through the existing machinery.
//! This module adds the style-flag surface on top (`--tron`, `:tron on|off|toggle`, `:style tron`,
//! the `F11` cycle): switching TRON on remembers the look that was active, switching it off restores it.

use ratatui::style::Color;

use super::AppearanceProfile;
use crate::config::BiltermConfig;
use crate::ui::theme::ThemePalette;

/// Status badge / `:style` label shown while TRON is active.
pub const TRON_STYLE_LABEL: &str = "◈ TRON (Neon Light-Cycle Grid)";

/// The look that was active when TRON was switched on (runtime only, never saved to disk).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TronRestore {
    pub profile: Option<AppearanceProfile>,
    pub active_custom: Option<String>,
    pub snazzy: bool,
    pub glass: bool,
    pub modern: bool,
    pub liquid: bool,
}

/// The `TRON` palette. Canvas `#000000`, surface `#0C141F`, grid/active border `#6FC3DF`,
/// hot cyan `#00F6FF`, orange accent `#FF9F1C` / `#DF740C`, text `#E6FFFF`, dim-teal idle border.
pub fn tron_palette() -> ThemePalette {
    let c = Color::Rgb;
    ThemePalette {
        name: "TRON".to_string(),
        prompt_surface_bg: c(12, 20, 31),
        window_frame_bg: c(0, 0, 0),
        active_border_fg: c(111, 195, 223),
        inactive_border_fg: c(28, 74, 88),
        title_fg: c(230, 255, 255),
        prompt_prefix_fg: c(255, 159, 28),
        oia_bg: c(6, 12, 20),
        oia_badge_bg: c(223, 116, 12),
        oia_badge_fg: c(0, 0, 0),
        crosshair_bg: c(10, 28, 38),
        fkey_bar_bg: c(6, 12, 20),
        fkey_bar_fg: c(170, 224, 238),
        fkey_num_fg: c(0, 246, 255),
        shadow_fg: c(12, 58, 72),
        shadow_bg: c(0, 0, 0),
        text_fg: Some(c(230, 255, 255)),
        controls_fg: Some(c(111, 195, 223)),
        cursor_fg: Some(c(0, 246, 255)),
    }
}

/// `true` while the TRON profile is the selected look.
pub fn tron_active(cfg: &BiltermConfig) -> bool {
    cfg.appearance.profile == Some(AppearanceProfile::Tron)
}

fn clear_style_flags(cfg: &mut BiltermConfig) {
    cfg.snazzy_mode = false;
    cfg.glass_mode = false;
    cfg.modern_mode = false;
    cfg.liquid_glass = false;
}

/// Switches TRON on, remembering the current look for [`tron_off`]. No-op when already on.
pub fn tron_on(cfg: &mut BiltermConfig) {
    if tron_active(cfg) {
        return;
    }
    cfg.appearance.tron_restore = Some(TronRestore {
        profile: cfg.appearance.profile,
        active_custom: cfg.appearance.active_custom.take(),
        snazzy: cfg.snazzy_mode,
        glass: cfg.glass_mode,
        modern: cfg.modern_mode,
        liquid: cfg.liquid_glass,
    });
    clear_style_flags(cfg);
    cfg.appearance.profile = Some(AppearanceProfile::Tron);
}

/// Switches TRON off and restores the look remembered by [`tron_on`]; Classic when there is none
/// (TRON came from `--tron`, the config file or `:appearance tron`). No-op when TRON is not on.
pub fn tron_off(cfg: &mut BiltermConfig) {
    if !tron_active(cfg) {
        return;
    }
    let saved = cfg.appearance.tron_restore.take().unwrap_or_default();
    cfg.appearance.profile = saved.profile.filter(|p| *p != AppearanceProfile::Tron);
    cfg.appearance.active_custom = saved.active_custom;
    cfg.snazzy_mode = saved.snazzy;
    cfg.glass_mode = saved.glass;
    cfg.modern_mode = saved.modern;
    cfg.liquid_glass = saved.liquid;
}

/// Leaves TRON for another style: the style flags are cleared (the caller sets the new ones) and
/// only an explicit pre-TRON `[appearance].profile` comes back. No-op when TRON is not on.
pub fn tron_leave(cfg: &mut BiltermConfig) {
    if !tron_active(cfg) {
        return;
    }
    tron_off(cfg);
    clear_style_flags(cfg);
}

/// Maps the argument of `:tron [on | off | toggle]` to its `:style` token (bare `:tron` toggles).
pub fn parse_tron_arg(arg: Option<&str>) -> Result<&'static str, String> {
    match arg {
        None | Some("toggle" | "cycle") => Ok("tron-toggle"),
        Some("on" | "true" | "1" | "enable") => Ok("tron"),
        Some("off" | "false" | "0" | "disable") => Ok("tron-off"),
        _ => Err("Usage: :tron [on | off | toggle]".to_string()),
    }
}

/// Applies the `--tron` command-line switch; returns `true` when it was present.
pub fn apply_cli_tron_switch(cfg: &mut BiltermConfig, args: &[String]) -> bool {
    let present = args.iter().any(|a| a == "--tron");
    if present {
        tron_on(cfg);
        // Nothing to go back to at startup: `:tron off` lands on Classic.
        cfg.appearance.tron_restore = None;
    }
    present
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_switch_turns_tron_on_and_off_lands_on_classic() {
        let mut cfg = BiltermConfig::default();
        assert!(!apply_cli_tron_switch(&mut cfg, &["--modern".to_string()]));
        assert!(!tron_active(&cfg));
        cfg.modern_mode = true;
        assert!(apply_cli_tron_switch(&mut cfg, &["--gui".to_string(), "--tron".to_string()]));
        assert!(tron_active(&cfg) && !cfg.modern_mode);
        assert_eq!(cfg.palette().name, "TRON");
        tron_off(&mut cfg);
        assert_eq!(AppearanceProfile::resolve(&cfg), AppearanceProfile::Legacy);
    }

    #[test]
    fn on_off_restores_previous_look_and_leave_keeps_only_the_profile() {
        let mut cfg = BiltermConfig { modern_mode: true, glass_mode: true, liquid_glass: true, ..Default::default() };
        tron_on(&mut cfg);
        tron_on(&mut cfg);
        assert!(tron_active(&cfg) && !cfg.liquid_glass && !cfg.is_glass());
        tron_off(&mut cfg);
        assert!(cfg.modern_mode && cfg.glass_mode && cfg.liquid_glass);
        assert_eq!(cfg.appearance.profile, None);
        tron_off(&mut cfg);
        assert!(cfg.liquid_glass);

        cfg.appearance.profile = Some(AppearanceProfile::Midnight);
        tron_on(&mut cfg);
        tron_leave(&mut cfg);
        assert_eq!(cfg.appearance.profile, Some(AppearanceProfile::Midnight));
        assert!(!cfg.liquid_glass && !cfg.modern_mode);
    }
}
