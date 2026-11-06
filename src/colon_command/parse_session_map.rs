//! Parser for `:sessions map [verb]` and `:packing [verb]` (Phase 54, VIEW-02).

use crate::action::Action;
use crate::wm::session_map::SessionMapOp;

pub fn parse_session_map(parts: &[&str]) -> Result<Action, String> {
    if parts.is_empty() {
        return Ok(Action::SessionMap(SessionMapOp::Toggle));
    }
    match parts[0] {
        "open" => Ok(Action::SessionMap(SessionMapOp::Open)),
        "close" => Ok(Action::SessionMap(SessionMapOp::Close)),
        "toggle" => Ok(Action::SessionMap(SessionMapOp::Toggle)),
        "dismiss" => Ok(Action::SessionMap(SessionMapOp::Dismiss)),
        "blur" => Ok(Action::SessionMap(SessionMapOp::Blur)),
        "pin" => {
            let opt = match parts.get(1).copied() {
                Some("on" | "true" | "1") => Some(true),
                Some("off" | "false" | "0") => Some(false),
                _ => None,
            };
            Ok(Action::SessionMap(SessionMapOp::Pin(opt)))
        }
        "dock" => {
            let opt = match parts.get(1).copied() {
                Some("on" | "true" | "1") => Some(true),
                Some("off" | "false" | "0") => Some(false),
                _ => None,
            };
            Ok(Action::SessionMap(SessionMapOp::Dock(opt)))
        }
        "step-window" => {
            let n: i32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            Ok(Action::SessionMap(SessionMapOp::StepWindow(n)))
        }
        "next-window" => Ok(Action::SessionMap(SessionMapOp::StepWindow(1))),
        "prev-window" => Ok(Action::SessionMap(SessionMapOp::StepWindow(-1))),
        "step-session" => {
            let n: i32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            Ok(Action::SessionMap(SessionMapOp::StepSession(n)))
        }
        "next-session" => Ok(Action::SessionMap(SessionMapOp::StepSession(1))),
        "prev-session" => Ok(Action::SessionMap(SessionMapOp::StepSession(-1))),
        "select" | "select-window" => {
            let id: usize = parts
                .get(1)
                .and_then(|s| s.trim_start_matches('#').parse().ok())
                .ok_or_else(|| "Usage: :sessions map select <pane_id>".to_string())?;
            Ok(Action::SessionMap(SessionMapOp::SelectWindow(id)))
        }
        "select-session" | "target" => {
            let name = parts
                .get(1)
                .ok_or_else(|| "Usage: :sessions map target <session_name>".to_string())?;
            Ok(Action::SessionMap(SessionMapOp::TargetSession(name.to_string())))
        }
        "pickup" => {
            let id: Option<usize> = parts
                .get(1)
                .and_then(|s| s.trim_start_matches('#').parse().ok());
            Ok(Action::SessionMap(SessionMapOp::Pickup(id)))
        }
        "drop" => Ok(Action::SessionMap(SessionMapOp::Drop)),
        "cancel" => Ok(Action::SessionMap(SessionMapOp::CancelDrag)),
        "scale" => {
            let arg = parts
                .get(1)
                .ok_or_else(|| "Usage: :sessions map scale <+10|-10|pct>".to_string())?;
            if let Some(delta) = arg.strip_prefix('+').and_then(|s| s.parse::<i16>().ok()) {
                Ok(Action::SessionMap(SessionMapOp::ScaleStep(delta)))
            } else if let Some(delta) = arg.strip_prefix('-').and_then(|s| s.parse::<i16>().ok()) {
                Ok(Action::SessionMap(SessionMapOp::ScaleStep(-delta)))
            } else if let Ok(n) = arg.trim_end_matches('%').parse::<u16>() {
                Ok(Action::PanelScaleOf(
                    crate::ui::typography::PanelTarget::SessionMap,
                    n,
                ))
            } else {
                Err("Usage: :sessions map scale <+10|-10|pct>".to_string())
            }
        }
        "move" => {
            let dx: i16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let dy: i16 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            Ok(Action::SessionMap(SessionMapOp::Move { dx, dy }))
        }
        "resize" => {
            let dw: i16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let dh: i16 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            Ok(Action::SessionMap(SessionMapOp::Resize { dw, dh }))
        }
        "rect" => {
            let x: u16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let y: u16 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let w: u16 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(74);
            let h: u16 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(22);
            Ok(Action::SessionMap(SessionMapOp::SetRect { x, y, w, h }))
        }
        _ => Err(format!("Unknown session map command: {}", parts[0])),
    }
}
