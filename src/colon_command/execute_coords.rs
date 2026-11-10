//! Executor for `Action::Coords` (`COORD-04`, Phase 71): show / style / locate / copy.

use crate::api::json::JsonValue;
use crate::wm::WindowManager;
use crate::wm::coordinates::{CoordDisplayStyle, CoordsOp, TargetCoord};

fn fail(wm: &mut WindowManager, msg: String) {
    wm.record_automation_error(msg.clone());
    wm.active_pane_mut().set_status_message(Some(msg));
}

/// Runs one coords op; never quits.
pub fn execute_coords(wm: &mut WindowManager, op: CoordsOp) -> bool {
    match op {
        CoordsOp::Show => {
            let id = wm.active_pane_id();
            let Some(c) = wm.coord_of(id) else {
                fail(wm, "coords: no focused window".to_string());
                return false;
            };
            let path = c.to_path();
            let compact = c.format(CoordDisplayStyle::Compact).unwrap_or_default();
            wm.set_command_data(
                JsonValue::obj([("path", JsonValue::String(path.clone())), ("compact", JsonValue::String(compact.clone()))])
                    .to_json_string(),
            );
            wm.active_pane_mut().set_status_message(Some(format!("Coordinate: {compact}  {path}")));
        }
        CoordsOp::Cycle => {
            let style = wm.coord_style().next();
            wm.set_coord_style(style);
            wm.active_pane_mut().set_status_message(Some(format!("Coordinate badge: {}", style.name())));
        }
        CoordsOp::Style(style) => {
            wm.set_coord_style(style);
            wm.active_pane_mut().set_status_message(Some(format!("Coordinate badge: {}", style.name())));
        }
        CoordsOp::Locate(text) => match TargetCoord::parse(&text) {
            Err(e) => fail(wm, format!("coords locate: {e}")),
            Ok(target) => {
                let res = wm.resolve_coord(&target);
                match res.panes.first().copied() {
                    None => fail(wm, format!("coords locate: no window matches '{}'", text.trim())),
                    Some(id) => {
                        wm.focus_and_raise(id);
                        let n = res.panes.len();
                        let path = res.coords.first().map(TargetCoord::to_path).unwrap_or_default();
                        let extra = if n > 1 { format!(" (+{} more)", n - 1) } else { String::new() };
                        wm.active_pane_mut().set_status_message(Some(format!("Focused {path}{extra}")));
                    }
                }
            }
        },
        CoordsOp::Copy => {
            let id = wm.active_pane_id();
            let Some(c) = wm.coord_of(id) else {
                fail(wm, "coords: no focused window".to_string());
                return false;
            };
            let path = c.to_path();
            crate::clipboard::set_system_clipboard(&path);
            wm.clipboard_window_apply(crate::wm::ClipboardWindowOp::CopyText(path.clone()));
            wm.active_pane_mut().set_status_message(Some(format!("Copied coordinate {path}")));
        }
    }
    false
}
