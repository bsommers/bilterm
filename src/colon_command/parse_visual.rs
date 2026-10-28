//! Visual, shader, typography, drawer, and canvas `:colon` command parsing (`ARCH-R04`).

use crate::colon_command::ColonCommand;
use crate::ui::chrome::{OutlineStyle, ShadowStyle};
use crate::ui::cursor::{CaretStyle, SpotlightMode};
use crate::ui::glyphs::GlyphPack;
use crate::ui::theme::ThemePreset;
use crate::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode};

/// Parses visual, shader, typography, drawer, and canvas `:colon` commands.
pub(super) fn parse_visual_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    match head {
        "theme" => match parts.get(1).copied() {
            None | Some("list" | "ls") => Some(Ok(ColonCommand::ThemeCommand("list".to_string()))),
            Some("import" | "load" | "export" | "save") => {
                let sub = parts[1..].join(" ");
                Some(Ok(ColonCommand::ThemeCommand(sub)))
            }
            Some(tok) => match ThemePreset::from_name(tok) {
                Some(preset) => Some(Ok(ColonCommand::Theme(preset))),
                None => Some(Err(
                    "Usage: :theme <preset> | list | import <path> | export [file.toml]"
                        .to_string(),
                )),
            },
        },
        "glyphs" => match parts.get(1).and_then(|s| GlyphPack::from_name(s)) {
            Some(pack) => Some(Ok(ColonCommand::Glyphs(pack))),
            None => Some(Err(
                "Usage: :glyphs unicode | cp437 | nerd-font | ascii".to_string(),
            )),
        },
        "cursor" => match parts.get(1).and_then(|s| CaretStyle::from_name(s)) {
            Some(style) => Some(Ok(ColonCommand::Cursor(style))),
            None => Some(Err(
                "Usage: :cursor block | hollow | underscore | bar".to_string(),
            )),
        },
        "outline" | "border" => match (parts.get(1).copied(), parts.get(2).copied()) {
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
                        "Usage: :outline [window|prompt] <solid|double|rounded|thick|dotted|dashed|quadruple|ascii>"
                            .to_string(),
                    ))
                }),
            _ => Some(Err(
                "Usage: :outline [window|prompt] <solid|double|rounded|thick|dotted|dashed|quadruple|ascii>"
                    .to_string(),
            )),
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
        "drawer" | "sidebar" | "panels" => parse_drawer_command(parts),
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

fn parse_drawer_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
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
        Some("autohide" | "auto" | "auto-hide") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::AutoHide,
        )))),
        Some("ondemand" | "on-demand") => Some(Ok(ColonCommand::DrawerVisibility(Some(
            DrawerVisibilityMode::OnDemand,
        )))),
        Some("float" | "overlay") => Some(Ok(ColonCommand::DrawerLayout(Some(
            DrawerLayoutMode::Overlay,
        )))),
        Some("push" | "bump") => Some(Ok(ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Push)))),
        Some("mode" | "layout") => match parts.get(2).copied() {
            Some("float" | "overlay") => Some(Ok(ColonCommand::DrawerLayout(Some(
                DrawerLayoutMode::Overlay,
            )))),
            Some("push" | "bump") => {
                Some(Ok(ColonCommand::DrawerLayout(Some(DrawerLayoutMode::Push))))
            }
            None => Some(Ok(ColonCommand::DrawerLayout(None))),
            _ => Some(Err("Usage: :drawer mode [float|push]".to_string())),
        },
        Some("size" | "width" | "height") => {
            if let Some(n) = parts.get(2).and_then(|s| s.parse::<u16>().ok()) {
                Some(Ok(ColonCommand::DrawerSize(n)))
            } else {
                Some(Err("Usage: :drawer size <cols_or_rows>".to_string()))
            }
        }
        _ => Some(Err(
            "Usage: :drawer [toggle|focus|left|right|top|bottom|fixed|autohide|ondemand|float|push|size <n>]"
                .to_string(),
        )),
    }
}
