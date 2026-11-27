//! Window and session management JSON-RPC handlers (`SES-05`, `UNI-R01`).

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::action::Action;
use crate::wm::window_manage::{
    BulkWindowOp, SessionManageOp, WindowManageOp, WindowSelectTarget,
};

impl AutomationControlPlane {
    /// Handles `window.move` JSON-RPC requests (`SES-01`, `SES-05`).
    pub fn handle_window_move(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let target_session = params
            .get("target_session")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'target_session' parameter".to_string())?
            .to_string();
        let window_id = params.get("window_id").and_then(JsonValue::as_usize);
        let switch = params
            .get("switch")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);

        let wid = window_id.unwrap_or_else(|| self.wm.active_pane_id());
        let src_session = self.wm.config().history_session.clone();

        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Move {
                window_id,
                target_session: target_session.clone(),
                switch,
            }),
            SnapshotPolicy::Trusted,
        )?;

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("window_id", JsonValue::Number(wid as i64)),
            ("source_session", JsonValue::String(src_session)),
            ("target_session", JsonValue::String(target_session)),
        ]))
    }

    /// Handles `window.reorder` JSON-RPC requests: `{window_id?, to}` with `to` = up|down|top|bottom|1..N.
    pub fn handle_window_reorder(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let to = Self::move_spec(params)?;
        let window_id = params.get("window_id").and_then(JsonValue::as_usize);
        let wid = window_id.unwrap_or_else(|| self.wm.active_pane_id());
        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Reorder { window_id, to }),
            SnapshotPolicy::Trusted,
        )?;
        let order = self.wm.ordered_pane_ids();
        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("window_id", JsonValue::Number(wid as i64)),
            ("order", JsonValue::Array(order.into_iter().map(|p| JsonValue::Number(p as i64)).collect())),
        ]))
    }

    /// Handles `window.place` JSON-RPC requests: `{window_id?, target?, how}` with `how` = swap|left|right|up|down|float.
    pub fn handle_window_place(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let how = params
            .get("how")
            .and_then(JsonValue::as_str)
            .and_then(crate::wm::window_drag_place::PlaceHow::from_name)
            .ok_or("how must be one of swap|left|right|up|down|float")?;
        let window_id = params.get("window_id").and_then(JsonValue::as_usize);
        let target = params.get("target").and_then(JsonValue::as_usize);
        let wid = window_id.unwrap_or_else(|| self.wm.active_pane_id());
        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Place { window_id, target, how }),
            SnapshotPolicy::Trusted,
        )?;
        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("window_id", JsonValue::Number(wid as i64)),
            ("how", JsonValue::String(how.name().to_string())),
        ]))
    }

    /// Handles `session.move` JSON-RPC requests: `{name?, to}`; `session.reorder`: `{names: [..]}`.
    pub fn handle_session_move(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op = if let Some(arr) = params.get("names").and_then(JsonValue::as_array) {
            SessionManageOp::Reorder {
                names: arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            }
        } else {
            SessionManageOp::MoveOrder {
                name: params.get("name").and_then(JsonValue::as_str).map(String::from),
                target_pos: Self::move_spec(params)?,
            }
        };
        self.apply_with_policy(Action::SessionManage(op), SnapshotPolicy::Trusted)?;
        let order = self.wm.ordered_sessions_with_active();
        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("order", JsonValue::Array(order.into_iter().map(JsonValue::String).collect())),
        ]))
    }

    /// `to` as a string (`up|down|top|bottom|N`) or a 1-based integer.
    fn move_spec(params: &JsonValue) -> Result<String, String> {
        match params.get("to") {
            Some(v) if v.as_str().is_some() => Ok(v.as_str().unwrap_or_default().to_string()),
            Some(v) if v.as_usize().is_some() => Ok(v.as_usize().unwrap_or(1).to_string()),
            _ => Err("missing 'to' parameter (up|down|top|bottom|1..N)".to_string()),
        }
    }

    /// Handles `window.rename` JSON-RPC requests (`SES-02`, `SES-05`).
    pub fn handle_window_rename(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let new_name = params
            .get("new_name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'new_name' parameter".to_string())?
            .to_string();
        let window_id = params.get("window_id").and_then(JsonValue::as_usize);
        let wid = window_id.unwrap_or_else(|| self.wm.active_pane_id());

        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Rename {
                window_id,
                new_name: new_name.clone(),
            }),
            SnapshotPolicy::Trusted,
        )?;

        let actual_name = self
            .wm
            .pane(wid)
            .map(|p| p.title().to_string())
            .unwrap_or(new_name);

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("window_id", JsonValue::Number(wid as i64)),
            ("name", JsonValue::String(actual_name)),
        ]))
    }

    /// Handles `session.rename` JSON-RPC requests (`SES-02`, `SES-05`).
    pub fn handle_session_rename(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let new_name = params
            .get("new_name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'new_name' parameter".to_string())?
            .to_string();
        let old_name = params
            .get("old_name")
            .and_then(JsonValue::as_str)
            .map(String::from);

        let old_name_used = old_name
            .clone()
            .unwrap_or_else(|| self.wm.config().history_session.clone());

        self.apply_with_policy(
            Action::SessionManage(SessionManageOp::Rename {
                old_name,
                new_name: new_name.clone(),
            }),
            SnapshotPolicy::Trusted,
        )?;

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("old_name", JsonValue::String(old_name_used)),
            ("new_name", JsonValue::String(new_name)),
        ]))
    }

    /// Handles `window.select` JSON-RPC requests (`SES-03`, `SES-05`).
    pub fn handle_window_select(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action = params.get("action").and_then(JsonValue::as_str).ok_or_else(|| {
            "missing 'action' parameter (toggle, range, all, none, invert, explicit)".to_string()
        })?;

        let target = match action {
            "toggle" => {
                let wid = params
                    .get("window_id")
                    .and_then(JsonValue::as_usize)
                    .unwrap_or_else(|| self.wm.active_pane_id());
                WindowSelectTarget::Toggle(wid)
            }
            "range" => {
                let from = params
                    .get("from")
                    .and_then(JsonValue::as_usize)
                    .ok_or_else(|| "missing 'from' parameter for range select".to_string())?;
                let to = params
                    .get("to")
                    .and_then(JsonValue::as_usize)
                    .ok_or_else(|| "missing 'to' parameter for range select".to_string())?;
                WindowSelectTarget::Range { from, to }
            }
            "all" => WindowSelectTarget::All,
            "none" => WindowSelectTarget::None,
            "invert" => WindowSelectTarget::Invert,
            "explicit" => {
                let raw_list = params
                    .get("windows")
                    .and_then(JsonValue::as_array)
                    .ok_or_else(|| {
                        "missing 'windows' array parameter for explicit select".to_string()
                    })?;
                let mut windows = Vec::new();
                for item in raw_list {
                    if let Some(id) = item.as_usize() {
                        windows.push(id);
                    }
                }
                WindowSelectTarget::Explicit(windows)
            }
            other => return Err(format!("unknown window.select action '{other}'")),
        };

        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Select(target)),
            SnapshotPolicy::Trusted,
        )?;

        let selected = self
            .wm
            .selected_windows()
            .iter()
            .map(|&id| JsonValue::Number(id as i64))
            .collect::<Vec<_>>();
        let count = selected.len();

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("selected", JsonValue::Array(selected)),
            ("count", JsonValue::Number(count as i64)),
        ]))
    }

    /// Handles `window.bulk` JSON-RPC requests (`SES-03`, `SES-05`).
    pub fn handle_window_bulk(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action = params.get("action").and_then(JsonValue::as_str).ok_or_else(|| {
            "missing 'action' parameter (move, close, rename_prefix, tile, pin)".to_string()
        })?;

        let affected_before = self
            .wm
            .selected_windows()
            .iter()
            .map(|&id| JsonValue::Number(id as i64))
            .collect::<Vec<_>>();
        let count = affected_before.len();

        let bulk_op = match action {
            "move" => {
                let target_session = params
                    .get("target_session")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        "missing 'target_session' parameter for bulk move".to_string()
                    })?
                    .to_string();
                BulkWindowOp::Move { target_session }
            }
            "close" => {
                let force = params
                    .get("force")
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(false);
                if force {
                    BulkWindowOp::ForceClose
                } else {
                    BulkWindowOp::Close
                }
            }
            "rename_prefix" => {
                let prefix = params
                    .get("prefix")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        "missing 'prefix' parameter for bulk rename_prefix".to_string()
                    })?
                    .to_string();
                BulkWindowOp::RenamePrefix { prefix }
            }
            "tile" => BulkWindowOp::Tile,
            "pin" => {
                let pinned = params.get("pinned").and_then(JsonValue::as_bool);
                BulkWindowOp::Pin(pinned)
            }
            other => return Err(format!("unknown window.bulk action '{other}'")),
        };

        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Bulk(bulk_op)),
            SnapshotPolicy::Trusted,
        )?;

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("action", JsonValue::String(action.to_string())),
            ("affected_windows", JsonValue::Array(affected_before)),
            ("count", JsonValue::Number(count as i64)),
        ]))
    }

    /// Handles `window.list` JSON-RPC requests (`SES-01..05`).
    pub fn handle_window_list(&mut self) -> Result<JsonValue, String> {
        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::List),
            SnapshotPolicy::Trusted,
        )?;
        let data = self
            .wm
            .take_command_data()
            .and_then(|d| JsonValue::parse(&d))
            .unwrap_or_else(|| self.wm.list_windows_json());
        Ok(data)
    }

    /// Handles `window.undo` JSON-RPC requests (`SES-04`, `SES-05`).
    pub fn handle_window_undo(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let desc = self
            .wm
            .window_manage()
            .undo_stack
            .last()
            .map(|u| u.description.clone())
            .unwrap_or_else(|| "operation".to_string());

        self.apply_with_policy(
            Action::WindowManage(WindowManageOp::Undo),
            SnapshotPolicy::Trusted,
        )?;

        Ok(JsonValue::obj([
            ("status", JsonValue::String("ok".to_string())),
            ("reverted", JsonValue::String(desc)),
        ]))
    }
}
