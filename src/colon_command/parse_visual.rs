//! Visual, shader, typography, drawer, and canvas `:colon` command parsing (`ARCH-R04`).

use crate::colon_command::ColonCommand;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode};

const WINDOW_STYLE_USAGE: &str = "Usage: :window [menu | show | reset | theme [name|list|browse|import|next|prev|reset] | outline <style|next|reset> | bold [on|off|reset] | <font-color|bg|prompt-bg|prompt-color|cursor-color|frame-color|inactive-border|title-color|controls-color|status-color|badge-color> <color|reset>]";

/// Validates `:window ...` per-window chrome arguments and returns the normalized subcommand.
pub(super) fn validate_window_style_args(args: &[&str]) -> Result<String, String> {
    use crate::ui::cursor_cmd::parse_cursor_color;
    use crate::wm::chrome_style::canonical_window_color_element;

    let is_reset_kw = |s: &str| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "reset" | "default" | "global" | "theme" | "clear" | "none" | "auto"
        )
    };

    match args.first().copied() {
        None => return Ok("menu".to_string()),
        Some("menu" | "list" | "ls" | "customize") if args.len() == 1 => {
            return Ok("menu".to_string());
        }
        Some("show" | "status" | "info") if args.len() == 1 => return Ok("show".to_string()),
        Some("reset" | "clear") if args.len() == 1 => return Ok("reset".to_string()),
        Some("browse" | "files" | "explorer") => {
            let rest = args[1..].join(" ");
            return Ok(if rest.is_empty() {
                "theme browse".to_string()
            } else {
                format!("theme browse {rest}")
            });
        }
        Some("theme" | "themes" | "color" | "colors") => {
            let rest = args[1..].join(" ");
            return Ok(if rest.trim().is_empty() {
                "theme list".to_string()
            } else {
                format!("theme {}", rest.trim())
            });
        }
        Some("outline" | "border" | "frame") if args.len() == 2 => {
            let v = args[1];
            if matches!(v, "next" | "cycle" | "reset" | "default" | "global")
                || OutlineStyle::from_name(v).is_some()
            {
                return Ok(format!("outline {v}"));
            }
            if is_reset_kw(v) || parse_cursor_color(v).is_some() {
                return Ok(format!("color frame-color {v}"));
            }
        }
        Some("bold")
            if args.len() == 1
                || (args.len() == 2
                    && matches!(
                        args[1],
                        "on" | "off" | "toggle" | "true" | "false" | "reset"
                    )) =>
        {
            return Ok(args.join(" "));
        }
        Some(first) => {
            // Support both `:window font-color amber` and two-word `:window font color amber` / `:window frame color cyan`
            let (elem_candidate, val_slice) = if args.get(1).is_some_and(|s| {
                matches!(
                    s.to_ascii_lowercase().as_str(),
                    "color" | "fg" | "bg" | "border" | "frame" | "badge"
                )
            }) && canonical_window_color_element(&format!(
                "{first}-{}",
                args[1].to_ascii_lowercase()
            ))
            .is_some()
            {
                (
                    format!("{first}-{}", args[1].to_ascii_lowercase()),
                    &args[2..],
                )
            } else {
                (first.to_string(), &args[1..])
            };

            if let Some(canon) = canonical_window_color_element(&elem_candidate) {
                match val_slice.first().copied() {
                    None | Some("list" | "ls" | "menu") if val_slice.len() <= 1 => {
                        return Ok(format!("menu {canon}"));
                    }
                    Some(v) if val_slice.len() == 1 && (is_reset_kw(v) || parse_cursor_color(v).is_some()) => {
                        return Ok(format!("color {canon} {v}"));
                    }
                    _ => return Err(WINDOW_STYLE_USAGE.to_string()),
                }
            }
        }
    }
    Err(WINDOW_STYLE_USAGE.to_string())
}

/// Parses visual, shader, typography, drawer, and canvas `:colon` commands.
pub(super) fn parse_visual_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    match head {
        "browse" | "files" | "filebrowser" | "explorer" => {
            let sub = if parts.len() > 1 {
                format!("browse {}", parts[1..].join(" "))
            } else {
                "browse".to_string()
            };
            Some(Ok(ColonCommand::ThemeCommand(sub)))
        }
        "itermcolors" | "iterm2colors" => match parts.get(1).copied() {
            None | Some("list" | "ls") => {
                Some(Ok(ColonCommand::ThemeCommand("itermcolors".to_string())))
            }
            Some("import" | "load" | "browse" | "files") => {
                Some(Ok(ColonCommand::ThemeCommand(parts[1..].join(" "))))
            }
            Some(_) => Some(Ok(ColonCommand::ThemeCommand(format!(
                "import {}",
                parts[1..].join(" ")
            )))),
        },
        "theme" => match parts.get(1).copied() {
            None | Some("list" | "ls" | "itermcolors" | "iterm2colors") => {
                let sub = if parts.len() > 1 {
                    parts[1..].join(" ")
                } else {
                    "list".to_string()
                };
                Some(Ok(ColonCommand::ThemeCommand(sub)))
            }
            Some("window" | "win" | "pane" | "this") => {
                let mut args = vec!["theme"];
                args.extend_from_slice(&parts[2..]);
                Some(validate_window_style_args(&args).map(ColonCommand::WindowStyle))
            }
            Some("global" | "all") => {
                let rest = parts[2..].join(" ");
                if rest.trim().is_empty() {
                    Some(Err("Usage: :theme global <preset|name>".to_string()))
                } else {
                    Some(Ok(ColonCommand::ThemeCommand(format!("global {}", rest.trim()))))
                }
            }
            Some("import" | "load" | "export" | "save" | "browse" | "files" | "open" | "config" | "where") => {
                let sub = parts[1..].join(" ");
                Some(Ok(ColonCommand::ThemeCommand(sub)))
            }
            Some(tok) => match ThemePreset::from_name(tok) {
                Some(preset) if parts.len() == 2 => Some(Ok(ColonCommand::Theme(preset))),
                _ => {
                    let arg = parts[1..].join(" ");
                    let lower = arg.to_ascii_lowercase();
                    if lower.ends_with(".itermcolors")
                        || lower.ends_with(".toml")
                        || lower.ends_with(".yaml")
                        || lower.ends_with(".yml")
                        || arg.contains('/')
                        || arg.contains('*')
                        || arg.contains('~')
                        || crate::ui::file_browser::expand_user_path(&arg).exists()
                    {
                        Some(Ok(ColonCommand::ThemeCommand(format!("import {arg}"))))
                    } else {
                        Some(Err(
                            "Usage: :theme <preset> | list | browse [dir] | import [path|dir|glob] | export [file.toml]"
                                .to_string(),
                        ))
                    }
                }
            },
        },
        "appearance" => match parts
            .get(1)
            .and_then(|s| crate::ui::design::AppearanceProfile::from_name(s))
        {
            Some(p) => Some(Ok(ColonCommand::Appearance(p))),
            None => Some(Err(
                "Usage: :appearance legacy | obsidian | midnight | arctic | liquid".to_string(),
            )),
        },
        "glyphs" => match parts.get(1).and_then(|s| GlyphPack::from_name(s)) {
            Some(pack) => Some(Ok(ColonCommand::Glyphs(pack))),
            None => Some(Err(
                "Usage: :glyphs unicode | cp437 | nerd-font | ascii".to_string(),
            )),
        },
        "cursor" => {
            if parts.len() == 2
                && !matches!(
                    parts[1].to_ascii_lowercase().as_str(),
                    "solid" | "custom" | "char" | "glyph" | "custom-char"
                )
                && let Some(style) = CaretStyle::from_name(parts[1])
            {
                return Some(Ok(ColonCommand::Cursor(style)));
            }
            match crate::ui::cursor_cmd::validate_cursor_command_args(&parts[1..]) {
                Ok(()) => {
                    let sub = if parts.len() > 1 {
                        format!("cursor {}", parts[1..].join(" "))
                    } else {
                        "cursor".to_string()
                    };
                    Some(Ok(ColonCommand::ThemeCommand(sub)))
                }
                Err(e) => Some(Err(e)),
            }
        }
        "window" | "win" | "pane-style" => match parts.get(1).copied() {
            Some("left" | "h" | "a" | "west") if parts.len() == 2 => {
                Some(Ok(ColonCommand::PaneFocusDirection { dx: -1, dy: 0 }))
            }
            Some("right" | "l" | "d" | "east") if parts.len() == 2 => {
                Some(Ok(ColonCommand::PaneFocusDirection { dx: 1, dy: 0 }))
            }
            Some("up" | "k" | "w" | "north") if parts.len() == 2 => {
                Some(Ok(ColonCommand::PaneFocusDirection { dx: 0, dy: -1 }))
            }
            Some("down" | "j" | "s" | "south") if parts.len() == 2 => {
                Some(Ok(ColonCommand::PaneFocusDirection { dx: 0, dy: 1 }))
            }
            Some("next" | "n") if parts.len() == 2 => Some(Ok(ColonCommand::DeckNext)),
            Some("prev" | "p") if parts.len() == 2 => Some(Ok(ColonCommand::DeckPrev)),
            Some(s) if parts.len() == 2 && s.parse::<usize>().is_ok() => {
                Some(Ok(ColonCommand::FocusPane(s.parse::<usize>().unwrap_or(1))))
            }
            _ => Some(validate_window_style_args(&parts[1..]).map(ColonCommand::WindowStyle)),
        },
        "outline" | "border" => match (parts.get(1).copied(), parts.get(2).copied()) {
            (Some("bold" | "heavy-text"), arg) => Some(Ok(ColonCommand::WindowStyle(format!(
                "global-bold {}",
                arg.unwrap_or("toggle")
            )))),
            (Some("pane" | "this" | "focused"), Some(style_str)) => Some(
                validate_window_style_args(&["outline", style_str]).map(ColonCommand::WindowStyle),
            ),
            (Some("window" | "win"), Some(style_str)) => OutlineStyle::from_name(style_str)
                .map(|s| Ok(ColonCommand::Outline { window: Some(s), prompt: None }))
                .or_else(|| Some(Err("Unknown outline style".to_string()))),
            (Some("prompt" | "cmd"), Some(style_str)) => OutlineStyle::from_name(style_str)
                .map(|s| Ok(ColonCommand::Outline { window: None, prompt: Some(s) }))
                .or_else(|| Some(Err("Unknown outline style".to_string()))),
            (Some(style_str), None) => OutlineStyle::from_name(style_str)
                .map(|s| Ok(ColonCommand::Outline { window: Some(s), prompt: Some(s) }))
                .or_else(|| {
                    Some(Err(
                        "Usage: :outline [window|prompt] <double|single|thin|rounded|thick|bold|dotted|dashed|quadruple|ascii>"
                            .to_string(),
                    ))
                }),
            _ => Some(Err(
                "Usage: :outline [window|prompt] <double|single|thin|rounded|thick|bold|dotted|dashed|quadruple|ascii>"
                    .to_string(),
            )),
        },
        "crosshair" if parts.get(1) == Some(&"cycle") => Some(Ok(ColonCommand::CrosshairCycle)),
        "focal" => match (parts.get(1), parts.get(2).and_then(|s| s.parse::<i16>().ok())) {
            (Some(&"radius"), Some(dx)) => {
                let dy = parts.get(3).and_then(|s| s.parse::<i16>().ok()).unwrap_or(0);
                Some(Ok(ColonCommand::FocalRadiusResize(dx, dy)))
            }
            _ => Some(Err("Usage: :focal radius <+N|-N> [+N|-N]".to_string())),
        },
        "crosshair" => {
            let (enabled, fullscreen) = match parts.get(1).copied() {
                Some("on" | "true") => (Some(true), None),
                Some("off" | "false") => (Some(false), None),
                Some("full" | "fullscreen") => (Some(true), Some(true)),
                Some("pane" | "local") => (Some(true), Some(false)),
                _ => (None, None),
            };
            Some(Ok(ColonCommand::CrosshairToggle {
                enabled,
                fullscreen,
            }))
        }
        "spotlight" | "spot" | "focus-light" => parse_spotlight_command(parts),
        "focalzoom" | "fzoom" | "lens" | "magnify" => parse_focalzoom_command(parts),
        "shadows" | "shadow" => {
            let arg = parts.get(1).and_then(|s| ShadowStyle::from_name(s));
            Some(Ok(ColonCommand::Shadows(arg)))
        }
        "fkeybar" => {
            let arg = match parts.get(1).copied() {
                Some("on") | Some("true") => Some(true),
                Some("off") | Some("false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::FKeyBarToggle(arg)))
        }
        "menubar" | "menu" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::MenuBarToggle(arg)))
        }
        "version" | "ver" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true" | "show") => Some(true),
                Some("off" | "false" | "hide") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::VersionToggle(arg)))
        }
        "crt" | "scanlines" => {
            let arg = match parts.get(1).copied() {
                Some("on" | "true") => Some(true),
                Some("off" | "false") => Some(false),
                _ => None,
            };
            Some(Ok(ColonCommand::CrtToggle(arg)))
        }
        "drawer" | "sidebar" | "panels" | "navigator" => parse_drawer_command(parts),
        "fkeys" | "fkey" | "function-keys" => parse_fkeys_command(parts),
        "snazzy" | "herdr-style" => match parts.get(1).copied() {
            None | Some("toggle" | "cycle") => Some(Ok(ColonCommand::VisualStyleCommand(Some(
                "snazzy-toggle".to_string(),
            )))),
            Some("on" | "true" | "1" | "enable") => Some(Ok(ColonCommand::VisualStyleCommand(
                Some("snazzy".to_string()),
            ))),
            Some("off" | "false" | "0" | "disable") => Some(Ok(
                ColonCommand::VisualStyleCommand(Some("snazzy-off".to_string())),
            )),
            _ => Some(Err("Usage: :snazzy [on | off | toggle]".to_string())),
        },
        "glass" | "frosted" | "aero" => match parts.get(1).copied() {
            None | Some("toggle" | "cycle") => Some(Ok(ColonCommand::VisualStyleCommand(Some(
                "glass-toggle".to_string(),
            )))),
            Some("on" | "true" | "1" | "enable") => Some(Ok(ColonCommand::VisualStyleCommand(
                Some("glass".to_string()),
            ))),
            Some("off" | "false" | "0" | "disable") => Some(Ok(
                ColonCommand::VisualStyleCommand(Some("glass-off".to_string())),
            )),
            _ => Some(Err("Usage: :glass [on | off | toggle]".to_string())),
        },
        "modern" | "minimal" | "non-retro" | "nonretro" => match parts.get(1).copied() {
            None | Some("toggle" | "cycle") => Some(Ok(ColonCommand::VisualStyleCommand(Some(
                "modern-toggle".to_string(),
            )))),
            Some("on" | "true" | "1" | "enable") => Some(Ok(ColonCommand::VisualStyleCommand(
                Some("modern".to_string()),
            ))),
            Some("off" | "false" | "0" | "disable") => Some(Ok(
                ColonCommand::VisualStyleCommand(Some("modern-off".to_string())),
            )),
            _ => Some(Err("Usage: :modern [on | off | toggle]".to_string())),
        },
        "liquid" | "liquid-glass" | "liquidglass" => match parts.get(1).copied() {
            None | Some("toggle" | "cycle") => Some(Ok(ColonCommand::VisualStyleCommand(Some(
                "liquid-toggle".to_string(),
            )))),
            Some("on" | "true" | "1" | "enable") => Some(Ok(ColonCommand::VisualStyleCommand(
                Some("liquid".to_string()),
            ))),
            Some("off" | "false" | "0" | "disable") => Some(Ok(
                ColonCommand::VisualStyleCommand(Some("liquid-off".to_string())),
            )),
            _ => Some(Err("Usage: :liquid [on | off | toggle]".to_string())),
        },
        "style" | "visual" | "visual-style" => match parts.get(1).copied() {
            None | Some("cycle" | "next" | "toggle") => {
                Some(Ok(ColonCommand::VisualStyleCommand(None)))
            }
            Some(
                tok @ ("classic"
                | "off"
                | "reset"
                | "default"
                | "retro"
                | "snazzy"
                | "snazzy-only"
                | "snazzy-off"
                | "herdr"
                | "rich"
                | "glass"
                | "glass-only"
                | "glass-off"
                | "frosted"
                | "aero"
                | "translucent"
                | "both"
                | "all"
                | "snazzy-glass"
                | "snazzy+glass"
                | "max"
                | "ultra"
                | "modern"
                | "minimal"
                | "non-retro"
                | "nonretro"
                | "clean"
                | "modern-off"
                | "liquid"
                | "liquid-glass"
                | "liquidglass"
                | "liquid-off"),
            ) => Some(Ok(ColonCommand::VisualStyleCommand(Some(tok.to_string())))),
            _ => Some(Err(
                "Usage: :style [classic | snazzy | glass | snazzy-glass | modern | liquid | cycle]".to_string(),
            )),
        },
        "fontsize" | "textsize" => match parts.get(1).copied() {
            None | Some("reset" | "0" | "default") => Some(Ok(ColonCommand::FontZoomReset)),
            Some("+" | "in" | "up" | "grow" | "larger") => Some(Ok(ColonCommand::FontZoomIn)),
            Some("-" | "out" | "down" | "shrink" | "smaller") => Some(Ok(ColonCommand::FontZoomOut)),
            Some(raw) => {
                let cleaned = raw.trim_end_matches("pt").trim_end_matches('%');
                if let Ok(val) = cleaned.parse::<u16>() {
                    Some(Ok(ColonCommand::FontSetSize(val)))
                } else {
                    Some(Err("Usage: :fontsize [+|-|reset|<pt>|<pct>%]".to_string()))
                }
            }
        },
        "font" | "panefont" | "winfont" => {
            let subcmd = if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                "status".to_string()
            };
            Some(Ok(ColonCommand::PaneFontCommand(subcmd)))
        }
        "canvas" | "tek" => {
            let subcmd = if parts[0].eq_ignore_ascii_case("tek") {
                if parts.len() > 1 {
                    format!("tek {}", parts[1..].join(" "))
                } else {
                    "tek".to_string()
                }
            } else if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                "demo".to_string()
            };
            Some(Ok(ColonCommand::CanvasCommand(subcmd)))
        }
        _ => None,
    }
}

fn parse_spotlight_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    let mut mode = None;
    let mut pixelated = None;
    let mut fullscreen = None;
    let mut rx = None;
    let mut ry = None;
    let mut delta = None;
    let mut feather_pct = None;

    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "off" | "none" | "false" => mode = Some(SpotlightMode::Off),
            "circle" | "circular" | "round" | "on" | "true" => {
                mode = Some(SpotlightMode::Circle);
            }
            "ellipse" | "elliptical" | "oval" => mode = Some(SpotlightMode::Ellipse),
            "pixel" | "pixelated" | "block" | "blocks" | "cell" => pixelated = Some(true),
            "fuzzy" | "smooth" | "soft" => pixelated = Some(false),
            "full" | "fullscreen" => fullscreen = Some(true),
            "pane" | "local" => fullscreen = Some(false),
            "+" | "grow" | "larger" | "bigger" => delta = Some((4, 1)),
            "-" | "shrink" | "smaller" => delta = Some((-4, -1)),
            "size" | "radius" => {
                let Some(rx_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) else {
                    return Some(Err("Usage: :spotlight size <rx_cols> [ry_rows]".to_string()));
                };
                rx = Some(rx_val.clamp(4, 120));
                i += 1;
                if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                    ry = Some(ry_val.clamp(2, 60));
                    i += 1;
                }
            }
            "feather" | "edge" => {
                let Some(raw) = parts.get(i + 1) else {
                    return Some(Err(
                        "Usage: :spotlight feather <5..100 | 0.05..1.0>".to_string(),
                    ));
                };
                let pct = if let Ok(f) = raw.parse::<f32>() {
                    if f <= 1.0 {
                        (f * 100.0).round() as u8
                    } else {
                        f.round() as u8
                    }
                } else {
                    return Some(Err(
                        "Usage: :spotlight feather <5..100 | 0.05..1.0>".to_string(),
                    ));
                };
                feather_pct = Some(pct.clamp(5, 100));
                i += 1;
            }
            other => {
                if let Ok(rx_val) = other.parse::<u16>() {
                    rx = Some(rx_val.clamp(4, 120));
                    if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                        ry = Some(ry_val.clamp(2, 60));
                        i += 1;
                    }
                } else {
                    return Some(Err(
                        "Usage: :spotlight [circle|ellipse|off] [fuzzy|pixel] [full|pane] [+|-] [size <rx> [ry]] [feather <pct>]"
                            .to_string(),
                    ));
                }
            }
        }
        i += 1;
    }

    Some(Ok(ColonCommand::Spotlight {
        mode,
        pixelated,
        fullscreen,
        rx,
        ry,
        delta,
        feather_pct,
    }))
}

fn parse_focalzoom_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    let mut mode = None;
    let mut zoom_pct = None;
    let mut step_pct = None;
    let mut pixelated = None;
    let mut fullscreen = None;
    let mut rx = None;
    let mut ry = None;
    let mut delta_size = None;
    let mut feather_pct = None;

    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "off" | "none" | "false" | "1x" | "1.0x" | "100%" => {
                mode = Some(SpotlightMode::Off);
            }
            "circle" | "circular" | "round" => mode = Some(SpotlightMode::Circle),
            "ellipse" | "elliptical" | "oval" | "on" | "true" => {
                mode = Some(SpotlightMode::Ellipse);
            }
            "subtle" | "low" => zoom_pct = Some(125),
            "medium" | "med" => zoom_pct = Some(150),
            "high" => zoom_pct = Some(200),
            "ultra" | "max" => zoom_pct = Some(300),
            "pixel" | "pixelated" | "block" | "blocks" | "cell" => pixelated = Some(true),
            "fuzzy" | "smooth" | "soft" => pixelated = Some(false),
            "full" | "fullscreen" => fullscreen = Some(true),
            "pane" | "local" => fullscreen = Some(false),
            "+" | "in" | "up" => step_pct = Some(25),
            "-" | "out" | "down" => step_pct = Some(-25),
            "grow" | "larger" | "bigger" => delta_size = Some((4, 1)),
            "shrink" | "smaller" => delta_size = Some((-4, -1)),
            "size" | "radius" => {
                let Some(rx_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) else {
                    return Some(Err("Usage: :focalzoom size <rx_cols> [ry_rows]".to_string()));
                };
                rx = Some(rx_val.clamp(4, 120));
                i += 1;
                if let Some(ry_val) = parts.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                    ry = Some(ry_val.clamp(2, 60));
                    i += 1;
                }
            }
            "feather" | "edge" => {
                let Some(raw) = parts.get(i + 1) else {
                    return Some(Err(
                        "Usage: :focalzoom feather <5..100 | 0.05..1.0>".to_string(),
                    ));
                };
                let pct = if let Ok(f) = raw.parse::<f32>() {
                    if f <= 1.0 {
                        (f * 100.0).round() as u8
                    } else {
                        f.round() as u8
                    }
                } else {
                    return Some(Err(
                        "Usage: :focalzoom feather <5..100 | 0.05..1.0>".to_string(),
                    ));
                };
                feather_pct = Some(pct.clamp(5, 100));
                i += 1;
            }
            other => {
                let cleaned = other.trim_end_matches('x').trim_end_matches('%');
                if let Ok(val) = cleaned.parse::<f32>() {
                    let pct = if val <= 6.0 {
                        (val * 100.0).round() as u16
                    } else {
                        val.round() as u16
                    };
                    if pct <= 100 {
                        mode = Some(SpotlightMode::Off);
                    } else {
                        zoom_pct = Some(pct.clamp(105, 400));
                    }
                } else {
                    return Some(Err(
                        "Usage: :focalzoom [1.25x|1.5x|2x|3x|off] [circle|ellipse] [fuzzy|pixel] [+|-] [grow|shrink] [size <rx> [ry]] [feather <pct>]"
                            .to_string(),
                    ));
                }
            }
        }
        i += 1;
    }

    Some(Ok(ColonCommand::FocalZoom {
        mode,
        zoom_pct,
        step_pct,
        pixelated,
        fullscreen,
        rx,
        ry,
        delta_size,
        feather_pct,
    }))
}

/// Parses the navigator operations of `:drawer` (`next|prev|enter|row|fold|scroll|widen|new-session|
/// new-pane|rename|close-pane`); `None` for every other verb. `:drawer close` stays the classic minimize.
fn parse_drawer_nav(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    use crate::wm::drawer::DrawerNavOp as Op;
    let num = |usage: &str| -> Result<i16, String> {
        parts.get(2).and_then(|v| v.parse::<i16>().ok()).ok_or_else(|| usage.to_string())
    };
    let op = match parts.get(1).copied()? {
        "fold" => Op::Fold,
        "enter" | "activate" => Op::Activate,
        "next" | "down" => Op::Next,
        "prev" | "up" => Op::Prev,
        "row" => match parts.get(2).and_then(|v| v.parse::<usize>().ok()).filter(|&n| n >= 1) {
            Some(n) => Op::Row(n - 1),
            None => return Some(Err("Usage: :drawer row <n>".to_string())),
        },
        "scroll" => match num("Usage: :drawer scroll <+-n>") {
            Ok(n) => Op::Scroll(n),
            Err(e) => return Some(Err(e)),
        },
        "widen" => match num("Usage: :drawer widen <+-n>") {
            Ok(n) => Op::Widen(n),
            Err(e) => return Some(Err(e)),
        },
        // `:drawer expand` without `all` keeps its older meaning (maximize) in the caller.
        "expand" if parts.get(2) == Some(&"all") => Op::ExpandAll,
        "collapse" => match parts.get(2) {
            Some(&"all") => Op::CollapseAll,
            _ => return Some(Err("Usage: :drawer expand all | collapse all".to_string())),
        },
        "new-session" => Op::NewSession,
        "new-pane" | "new-window" => Op::NewPane,
        "rename" => match parts.len() {
            2 => Op::RenamePrompt,
            3 => Op::Rename(parts[2].to_string()),
            _ => return Some(Err("Usage: :drawer rename [name]".to_string())),
        },
        "close-pane" => Op::Close,
        _ => return None,
    };
    Some(Ok(ColonCommand::DrawerNav(op)))
}

fn parse_drawer_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    if let Some(nav) = parse_drawer_nav(parts) {
        return Some(nav);
    }
    match parts.get(1).copied() {
        None | Some("toggle") => Some(Ok(ColonCommand::DrawerToggle)),
        Some("focus" | "open" | "show") => Some(Ok(ColonCommand::DrawerFocus)),
        Some("left") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Left)))),
        Some("right") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Right)))),
        Some("top") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Top)))),
        Some("bottom" | "bot") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Bottom)))),
        Some("edge" | "dock") => match parts.get(2).copied() {
            Some("left") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Left)))),
            Some("right") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Right)))),
            Some("top") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Top)))),
            Some("bottom" | "bot") => Some(Ok(ColonCommand::DrawerDock(Some(DrawerEdge::Bottom)))),
            None => Some(Ok(ColonCommand::DrawerDock(None))),
            _ => Some(Err(
                "Usage: :drawer edge [left|right|top|bottom]".to_string(),
            )),
        },
        Some("fixed" | "pin") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::Fixed,
        )))),
        Some("unpin") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::OnDemand,
        )))),
        Some("scale") => match parts.get(2).and_then(|v| v.trim_end_matches('%').parse::<u16>().ok()) {
            Some(n) if (75..=200).contains(&n) => Some(Ok(ColonCommand::PanelScaleOf(
                crate::ui::typography::PanelTarget::Drawer,
                n,
            ))),
            _ => Some(Err("Usage: :drawer scale <75..200> (GUI only)".to_string())),
        },
        Some("autohide" | "auto" | "auto-hide") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::AutoHide,
        )))),
        Some("ondemand" | "on-demand") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::OnDemand,
        )))),
        Some("float" | "overlay" | "fly" | "fly-out" | "flyout") => Some(Ok(
            ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Overlay)),
        )),
        Some("push" | "bump" | "push-out" | "pushout" | "dock-push") => {
            Some(Ok(ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Push))))
        }
        Some("mode" | "layout") => match parts.get(2).copied() {
            Some("float" | "overlay" | "fly" | "fly-out" | "flyout") => Some(Ok(
                ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Overlay)),
            )),
            Some("push" | "bump" | "push-out" | "pushout") => {
                Some(Ok(ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Push))))
            }
            None => Some(Ok(ColonCommand::DrawerLayout(None))),
            _ => Some(Err("Usage: :drawer mode [float|push]".to_string())),
        },
        Some("style") => {
            use crate::wm::drawer::DrawerStyle;
            match parts.get(2).copied() {
                None | Some("toggle") => Some(Ok(ColonCommand::DrawerStyle(None))),
                Some("classic") => Some(Ok(ColonCommand::DrawerStyle(Some(DrawerStyle::Classic)))),
                Some("navigator" | "herdr") => {
                    // `herdr` is the deprecated hidden alias of `navigator`.
                    Some(Ok(ColonCommand::DrawerStyle(Some(DrawerStyle::Navigator))))
                }
                _ => Some(Err("Usage: :drawer style [classic|navigator|toggle]".to_string())),
            }
        }
        Some("attention" | "next-blocked" | "next-attention") => Some(Ok(ColonCommand::DrawerNav(
            crate::wm::drawer::DrawerNavOp::NextAttention,
        ))),
        Some("minimize" | "min" | "hide" | "close") => Some(Ok(ColonCommand::DrawerSize(0))),
        Some("maximize" | "max" | "expand") => Some(Ok(ColonCommand::DrawerSize(u16::MAX))),
        Some("size" | "width" | "height") => {
            if let Some(n) = parts.get(2).and_then(|s| s.parse::<u16>().ok()) {
                Some(Ok(ColonCommand::DrawerSize(n)))
            } else {
                Some(Err("Usage: :drawer size <cols_or_rows>".to_string()))
            }
        }
        _ => Some(Err(
            "Usage: :drawer [toggle|focus|left|right|top|bottom|fixed|autohide|ondemand|float|push|attention|fold|enter|next|prev|row|scroll|widen|new-session|new-pane|rename|close-pane|expand all|collapse all|style|minimize|maximize|size <n>]"
                .to_string(),
        )),
    }
}

fn parse_fkeys_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    use crate::ui::visual_style::{RawFkeyMode, parse_fkey_number};
    match parts.get(1).copied() {
        None | Some("toggle" | "cycle" | "next") => Some(Ok(ColonCommand::FkeyModeCommand(None))),
        Some("status" | "show" | "info") => {
            Some(Ok(ColonCommand::FkeyModeCommand(Some("status".to_string()))))
        }
        Some("send" | "pass" | "pty") => {
            if let Some(tok) = parts.get(2).copied()
                && let Some(n) = parse_fkey_number(tok)
            {
                Some(Ok(ColonCommand::FkeyModeCommand(Some(format!("send F{n}")))))
            } else {
                Some(Err("Usage: :fkeys send <F1..F12>".to_string()))
            }
        }
        Some(tok) => {
            if let Some(mode) = RawFkeyMode::from_name(tok) {
                let canon = match mode {
                    RawFkeyMode::Bilterm => "bilterm",
                    RawFkeyMode::App => "app",
                    RawFkeyMode::Auto => "auto",
                };
                Some(Ok(ColonCommand::FkeyModeCommand(Some(canon.to_string()))))
            } else if let Some(n) = parse_fkey_number(tok) {
                Some(Ok(ColonCommand::FkeyModeCommand(Some(format!("send F{n}")))))
            } else {
                Some(Err(
                    "Usage: :fkeys [bilterm | app | auto | toggle | status | send <F1..F12>]"
                        .to_string(),
                ))
            }
        }
    }
}
