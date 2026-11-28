//! JSON-RPC handlers for `editor.*` (`EDIT-01`, `UNI-R01`): every method builds an
//! `Action::Editor` and runs it through `WindowManager::apply`, the same path as `:edit`,
//! `:scratch` and `:editor ...`. File verbs (open with a path, save, save_as, revert) need
//! `[api] allow_config_write` and are confined to the editor's working directory; `artifact` and
//! `broadcast` need `[api] allow_artifact_write` (or `allow_config_write`); overwriting needs an
//! explicit `force: true`.

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::action::Action;
use crate::editor::EditorOp;

pub const EDITOR_METHODS: &[&str] = &[
    "editor.open", "editor.scratch", "editor.artifact", "editor.board", "editor.save", "editor.save_as", "editor.wrap",
    "editor.render", "editor.check", "editor.broadcast", "editor.preview", "editor.find", "editor.undo",
    "editor.redo", "editor.goto", "editor.revert", "editor.insert", "editor.text", "editor.status", "editor.close", "editor.menu",
];

fn opt_str(p: &JsonValue, k: &str) -> Result<Option<String>, String> {
    match p.get(k) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("'{k}' must be a string")),
    }
}

fn req_str(p: &JsonValue, k: &str) -> Result<String, String> {
    opt_str(p, k)?.ok_or_else(|| format!("missing '{k}'"))
}

fn flag(p: &JsonValue, k: &str) -> bool {
    p.get(k).and_then(JsonValue::as_bool).unwrap_or(false)
}

/// Builds the op for `editor.<verb>`; `None` for an unknown verb.
pub(super) fn editor_op(verb: &str, p: &JsonValue) -> Result<Option<EditorOp>, String> {
    Ok(Some(match verb {
        "open" => EditorOp::Open { path: opt_str(p, "path")? },
        "scratch" => EditorOp::Scratch { name: opt_str(p, "name")? },
        "artifact" => EditorOp::Artifact { name: opt_str(p, "name")? },
        "board" => EditorOp::Board { name: opt_str(p, "name")? },
        "save" => EditorOp::Save { force: flag(p, "force") },
        "save_as" => EditorOp::SaveAs { path: req_str(p, "path")?, force: flag(p, "force") },
        "wrap" => EditorOp::Wrap(p.get("enabled").and_then(JsonValue::as_bool)),
        "render" => EditorOp::Render(p.get("enabled").and_then(JsonValue::as_bool)),
        "check" => EditorOp::Check,
        "broadcast" => EditorOp::Broadcast { note: opt_str(p, "note")? },
        "preview" => EditorOp::Preview,
        "find" => EditorOp::Find { query: req_str(p, "query")?, backward: flag(p, "backward") },
        "undo" => EditorOp::Undo,
        "redo" => EditorOp::Redo,
        "goto" => EditorOp::Goto(p.get("line").and_then(JsonValue::as_usize).ok_or("missing 'line'")?),
        "revert" => EditorOp::Revert,
        "insert" => EditorOp::Insert(req_str(p, "text")?),
        "text" => EditorOp::Text,
        "status" => EditorOp::Status,
        "close" => EditorOp::Close { force: flag(p, "force") },
        "menu" => {
            let item = opt_str(p, "item")?;
            let menu = opt_str(p, "menu")?;
            let op = opt_str(p, "op")?.unwrap_or_else(|| if item.is_some() { "do" } else { "open" }.into());
            let line = match op.as_str() {
                "open" => format!("menu open {}", menu.unwrap_or_default()),
                "close" => "menu close".to_string(),
                "do" => format!("menu do {}", item.ok_or("missing 'item'")?),
                o => return Err(format!("menu op must be open|close|do, got '{o}'")),
            };
            let parts: Vec<&str> = std::iter::once("editor").chain(line.split_whitespace()).collect();
            EditorOp::parse("editor", &parts).ok_or("menu parse")??
        }
        _ => return Ok(None),
    }))
}

impl AutomationControlPlane {
    pub(super) fn rpc_editor(&mut self, method: &str, params: &JsonValue) -> Result<JsonValue, String> {
        let verb = method.strip_prefix("editor.").unwrap_or(method);
        let op = editor_op(verb, params)?.ok_or_else(|| format!("unknown method {method}"))?;
        let action = Action::Editor(op);
        if let Some(err) = self.wm.config_write_denied(&action) {
            return Err(err);
        }
        self.wm.take_command_data();
        // API origin: apply_editor_op re-checks the gates and confines host paths to the workspace.
        let prev = self.wm.is_api_origin();
        self.wm.set_api_origin(true);
        let applied = self.apply_with_policy(action, SnapshotPolicy::Trusted);
        self.wm.set_api_origin(prev);
        applied?;
        let data = self.wm.take_command_data().ok_or_else(|| format!("no {method} data"))?;
        let v = JsonValue::parse(&data).ok_or_else(|| format!("invalid {method} data"))?;
        if v.get("status").and_then(JsonValue::as_str) == Some("error") {
            return Err(format!("{method}: {}", v.get("message").and_then(JsonValue::as_str).unwrap_or("failed")));
        }
        Ok(v)
    }
}
