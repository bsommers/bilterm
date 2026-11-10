//! Executor for `:get`, `:set`, `:config list|save`, `:appearance save` and `:state` (Phase 49-04).
//!
//! Each command sets a status message and stores structured JSON via `WindowManager::set_command_data`
//! so RPC/MCP callers receive a `data` field. API-originated writes are gated (`[api] allow_config_write`).

use crate::api::JsonValue;
use crate::colon_command::ColonCommand;
use crate::config::{dotted, BiltermConfig};
use crate::wm::WindowManager;

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}

fn fail(wm: &mut WindowManager, msg: String) {
    wm.record_automation_error(msg.clone());
    status(wm, msg);
}

fn save(wm: &mut WindowManager, prefix: Option<&str>) {
    let Some(path) = BiltermConfig::default_path() else {
        return fail(wm, "Config save failed: cannot resolve config path".to_string());
    };
    match dotted::save_changes(wm.config(), &path, prefix) {
        Ok(keys) => {
            let data = JsonValue::obj([
                ("path", JsonValue::String(path.display().to_string())),
                ("written", JsonValue::Array(keys.iter().cloned().map(JsonValue::String).collect())),
            ]);
            wm.set_command_data(data.to_json_string());
            let msg = if keys.is_empty() {
                format!("Config: nothing to save ({})", path.display())
            } else {
                format!("Config saved to {}: {}", path.display(), keys.join(", "))
            };
            status(wm, msg);
        }
        Err(e) => fail(wm, format!("Config save failed: {e}")),
    }
}

fn state_json(wm: &WindowManager) -> JsonValue {
    let cfg = wm.config();
    let layout = match wm.layout_mode() {
        crate::wm::WorkspaceLayoutMode::Tiled => "tiled",
        crate::wm::WorkspaceLayoutMode::Stacked => "stacked",
    };
    JsonValue::obj([
        ("appearance", JsonValue::String(crate::ui::design::AppearanceProfile::resolve(cfg).label().to_string())),
        ("display_mode", JsonValue::String(crate::ui::shell::effective_display_mode(cfg).label().to_string())),
        ("layout_mode", JsonValue::String(layout.to_string())),
        ("active_pane_id", JsonValue::Number(wm.active_pane_id() as i64)),
        ("pane_count", JsonValue::Number(wm.pane_count() as i64)),
        ("drawer_visible", JsonValue::Bool(wm.is_drawer_effectively_visible())),
    ])
}

/// Executes one of the config/state commands; always returns `false` (never quits).
pub(super) fn execute_config_command(wm: &mut WindowManager, cmd: ColonCommand) -> bool {
    if wm.is_api_origin()
        && let Some(err) = wm.config_write_denied(&cmd)
    {
        fail(wm, err);
        return false;
    }
    match cmd {
        ColonCommand::ConfigGet(key) => match dotted::get_key(wm.config(), &key) {
            Ok(text) => {
                let data = JsonValue::obj([("key", JsonValue::String(key.clone())), ("value", JsonValue::String(text.clone()))]);
                wm.set_command_data(data.to_json_string());
                status(wm, format!("{key} = {text}"));
            }
            Err(e) => fail(wm, e),
        },
        ColonCommand::ConfigSet(key, value) => {
            let mut next = wm.config().clone();
            match dotted::set_key(&mut next, &key, &value) {
                Ok(()) => {
                    *wm.config_mut() = next;
                    if key.starts_with("coords.") {
                        let style = wm.config().coords.badge;
                        wm.set_coord_style(style);
                    }
                    let shown = dotted::get_key(wm.config(), &key).unwrap_or(value);
                    let data = JsonValue::obj([("key", JsonValue::String(key.clone())), ("value", JsonValue::String(shown.clone()))]);
                    wm.set_command_data(data.to_json_string());
                    status(wm, format!("{key} = {shown} (session only; :config save to persist)"));
                }
                Err(e) => fail(wm, format!("Cannot set {key}: {e}")),
            }
        }
        ColonCommand::ConfigList => {
            let items = dotted::list_keys(wm.config());
            let n = items.len();
            let data = JsonValue::obj(items.into_iter().map(|(k, v)| (k, JsonValue::String(v))));
            wm.set_command_data(data.to_json_string());
            status(wm, format!("Config: {n} keys (see command data / :get <key>)"));
        }
        ColonCommand::ConfigSave => save(wm, None),
        ColonCommand::AppearanceSave => save(wm, Some("appearance.")),
        ColonCommand::StateQuery => {
            let state = state_json(wm);
            let line = state.to_json_string();
            wm.set_command_data(line.clone());
            status(wm, format!("State: {line}"));
        }
        _ => {}
    }
    false
}
