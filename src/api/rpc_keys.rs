//! JSON-RPC handlers for the keyboard surface (`keys.*`, `menu.open`; `KEY-02`, `UNI-R01`, Phase 63-03).
//!
//! Every method lowers to the same [`Action`] the colon command, palette entry and menu item use
//! and runs through `WindowManager::apply`, so the JSON is whatever the colon path stored in
//! `command_data`. `keys.list|check|explain` and `keys.profile` without a name are read-only;
//! `keys.profile <name>`, `keys.window_mode` and `menu.open` mutate session state and are refused
//! for API callers unless `[api] allow_config_write = true` (`WindowManager::config_write_denied`).

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::action::{Action, KeysOp, WindowModeOp};
use crate::wm::KeyMode;

/// `keys.window_mode` operand -> operation, parsed by the `:wmode` grammar so both paths agree.
fn window_mode_op(op: &str) -> Result<WindowModeOp, String> {
    let op = op.trim().to_ascii_lowercase();
    if !matches!(op.as_str(), "on" | "off" | "toggle" | "focus" | "resize" | "move") {
        return Err(format!("invalid 'op' '{op}': use on, off, toggle, focus, resize or move"));
    }
    match crate::colon_command::parse_colon_command(&format!(":wmode {op}")) {
        Some(Ok(Action::WindowMode(w))) => Ok(w),
        _ => Err(format!("invalid 'op' '{op}'")),
    }
}

fn key_mode_name(m: KeyMode) -> String {
    match m {
        KeyMode::Normal => "normal".to_string(),
        KeyMode::LeaderPending => "leader".to_string(),
        KeyMode::Window(sub) => format!("window:{}", sub.name()),
    }
}

impl AutomationControlPlane {
    pub(super) fn dispatch_keys(&mut self, method: &str, params: &JsonValue) -> Result<JsonValue, String> {
        match method {
            "keys.list" => self.keys_query(Action::Keys(KeysOp::List)),
            "keys.check" => self.keys_query(Action::Keys(KeysOp::Check)),
            "keys.explain" => {
                let chord = params
                    .get("chord")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| "missing 'chord' parameter".to_string())?;
                self.keys_query(Action::Keys(KeysOp::Explain(chord.to_string())))
            }
            "keys.profile" => {
                let name = params.get("name").and_then(JsonValue::as_str).map(str::to_string);
                self.keys_query(Action::Keys(KeysOp::Profile(name)))
            }
            "keys.window_mode" => {
                let op = params.get("op").and_then(JsonValue::as_str).ok_or_else(|| "missing 'op' parameter".to_string())?;
                self.keys_query(Action::WindowMode(window_mode_op(op)?))
            }
            "menu.open" => {
                let category = match params.get("category") {
                    None | Some(JsonValue::Null) => None,
                    Some(v) => Some(v.as_usize().ok_or_else(|| "invalid 'category' parameter (menu index)".to_string())?),
                };
                self.keys_query(Action::MenuOpen { category })
            }
            other => Err(format!("Method not found: {other}")),
        }
    }

    /// Gate (mutations only), apply, and return the stored JSON or a state summary.
    fn keys_query(&mut self, action: Action) -> Result<JsonValue, String> {
        if let Some(err) = self.wm.config_write_denied(&action) {
            return Err(err);
        }
        self.apply_with_policy(action, SnapshotPolicy::Trusted)?;
        let data = self.wm.take_command_data().unwrap_or_default();
        Ok(JsonValue::parse(&data).unwrap_or_else(|| {
            JsonValue::obj([
                ("ok", JsonValue::Bool(true)),
                ("key_mode", JsonValue::String(key_mode_name(self.wm.key_mode()))),
                (
                    "menu_open",
                    self.wm.menubar_state().map_or(JsonValue::Null, |(c, i)| {
                        JsonValue::obj([("category", JsonValue::Number(c as i64)), ("item", JsonValue::Number(i as i64))])
                    }),
                ),
            ])
        }))
    }
}
