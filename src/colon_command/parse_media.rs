//! Colon command parsing for `:browser` (`:bcanvas`) and `:video` (`:vcanvas`) (`WEB-01..02`, `VID-01..02`).

use crate::action::{Action, BrowserOp, VideoOp};
use crate::colon_command::ColonCommand;

pub fn parse_media_colon_command(
    head: &str,
    parts: &[&str],
    _rest: &str,
) -> Option<Result<ColonCommand, String>> {
    match head {
        "browser" | "bcanvas" => Some(parse_browser_command(&parts[1..])),
        "video" | "vcanvas" => Some(parse_video_command(&parts[1..])),
        _ => None,
    }
}

fn parse_browser_command(args: &[&str]) -> Result<ColonCommand, String> {
    if args.is_empty() {
        return Ok(Action::Browser(BrowserOp::Toggle));
    }

    match args[0] {
        "toggle" => Ok(Action::Browser(BrowserOp::Toggle)),
        "open" => {
            let url = args.get(1).copied().unwrap_or("https://example.com");
            Ok(Action::Browser(BrowserOp::Open {
                url: url.to_string(),
                pane_id: None,
            }))
        }
        "back" => Ok(Action::Browser(BrowserOp::Back { pane_id: None })),
        "forward" | "fwd" => Ok(Action::Browser(BrowserOp::Forward { pane_id: None })),
        "reload" | "refresh" => Ok(Action::Browser(BrowserOp::Reload { pane_id: None })),
        "stop" => Ok(Action::Browser(BrowserOp::Stop { pane_id: None })),
        "focus" => {
            let focused = match args.get(1).copied() {
                Some("on") | Some("true") | Some("1") => Some(true),
                Some("off") | Some("false") | Some("0") => Some(false),
                _ => None,
            };
            Ok(Action::Browser(BrowserOp::FocusViewport {
                pane_id: None,
                focused,
            }))
        }
        "zoom" => {
            let delta = if let Some(arg) = args.get(1) {
                let s = arg.trim_end_matches('%');
                s.parse::<i16>().unwrap_or(10)
            } else {
                10
            };
            Ok(Action::Browser(BrowserOp::Zoom {
                delta_pct: delta,
                pane_id: None,
            }))
        }
        "close" => Ok(Action::Browser(BrowserOp::Close { pane_id: None })),
        other => {
            // If user typed `:browser https://...`, treat as `:browser open <url>`
            if other.contains("://") || other.contains('.') {
                Ok(Action::Browser(BrowserOp::Open {
                    url: other.to_string(),
                    pane_id: None,
                }))
            } else {
                Err(format!("Unknown browser subcommand: {other}"))
            }
        }
    }
}

fn parse_video_command(args: &[&str]) -> Result<ColonCommand, String> {
    if args.is_empty() {
        return Ok(Action::Video(VideoOp::Toggle));
    }

    match args[0] {
        "toggle" => Ok(Action::Video(VideoOp::TogglePlayPause { pane_id: None })),
        "open" => {
            let source = args.get(1).copied().unwrap_or("demo://smpte");
            Ok(Action::Video(VideoOp::Open {
                source: source.to_string(),
                pane_id: None,
            }))
        }
        "play" => Ok(Action::Video(VideoOp::Play { pane_id: None })),
        "pause" => Ok(Action::Video(VideoOp::Pause { pane_id: None })),
        "seek" => {
            if let Some(arg) = args.get(1) {
                if let Some(pct_str) = arg.strip_suffix('%') {
                    let pct = pct_str.parse::<u8>().unwrap_or(0);
                    return Ok(Action::Video(VideoOp::SeekPercent {
                        pct,
                        pane_id: None,
                    }));
                }
                if arg.starts_with('+') || arg.starts_with('-') {
                    let secs = arg.parse::<f64>().unwrap_or(5.0);
                    let delta_ms = (secs * 1000.0) as i64;
                    return Ok(Action::Video(VideoOp::SeekRelative {
                        delta_ms,
                        pane_id: None,
                    }));
                }
                let secs = arg.parse::<f64>().unwrap_or(0.0);
                let position_ms = (secs.max(0.0) * 1000.0) as u64;
                Ok(Action::Video(VideoOp::SeekAbsolute {
                    position_ms,
                    pane_id: None,
                }))
            } else {
                Ok(Action::Video(VideoOp::SeekRelative {
                    delta_ms: 5000,
                    pane_id: None,
                }))
            }
        }
        "volume" | "vol" => {
            let delta = if let Some(arg) = args.get(1) {
                arg.trim_end_matches('%').parse::<i8>().unwrap_or(5)
            } else {
                5
            };
            Ok(Action::Video(VideoOp::VolumeDelta {
                delta_pct: delta,
                pane_id: None,
            }))
        }
        "mute" => Ok(Action::Video(VideoOp::ToggleMute { pane_id: None })),
        "loop" => Ok(Action::Video(VideoOp::ToggleLoop { pane_id: None })),
        "speed" => {
            let speed_x100 = if let Some(arg) = args.get(1) {
                let raw = arg.trim_end_matches('x');
                if let Ok(f) = raw.parse::<f32>() {
                    (f * 100.0).clamp(25.0, 400.0) as u16
                } else {
                    100
                }
            } else {
                100
            };
            Ok(Action::Video(VideoOp::SetSpeed {
                speed_x100,
                pane_id: None,
            }))
        }
        "close" => Ok(Action::Video(VideoOp::Close { pane_id: None })),
        other => {
            // If user typed `:video demo://...` or path, treat as `:video open <source>`
            if other.contains("://") || other.contains('.') {
                Ok(Action::Video(VideoOp::Open {
                    source: other.to_string(),
                    pane_id: None,
                }))
            } else {
                Err(format!("Unknown video subcommand: {other}"))
            }
        }
    }
}
