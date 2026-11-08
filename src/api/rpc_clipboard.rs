//! JSON-RPC 2.0 handlers for `clipboard.*` (`CLIP-04`, Phase 57).

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::wm::clipboard_registry::{ClipboardEntry, ClipboardFilterScope, ClipboardMime};
use crate::wm::clipboard_window::ClipboardWindowOp;

fn entry_to_json(entry: &ClipboardEntry) -> JsonValue {
    let (mime_kind, mime_type) = match &entry.mime {
        ClipboardMime::TextPlain => ("text", "text/plain"),
        ClipboardMime::MediaRef { mime, .. } => ("media", mime.as_str()),
    };
    JsonValue::obj([
        ("id", JsonValue::Number(entry.id as i64)),
        ("session", JsonValue::String(entry.session_name.clone())),
        (
            "pane_id",
            entry
                .pane_id
                .map_or(JsonValue::Null, |id| JsonValue::Number(id as i64)),
        ),
        ("pane_title", JsonValue::String(entry.window_title.clone())),
        ("source", JsonValue::String(entry.source.badge().to_string())),
        ("mime_kind", JsonValue::String(mime_kind.to_string())),
        ("mime", JsonValue::String(mime_type.to_string())),
        ("snippet", JsonValue::String(entry.masked_snippet(120))),
        (
            "preview_lines",
            JsonValue::Array(
                entry
                    .masked_preview_lines(16, 120)
                    .into_iter()
                    .map(JsonValue::String)
                    .collect(),
            ),
        ),
        ("char_len", JsonValue::Number(entry.char_count as i64)),
        ("line_count", JsonValue::Number(entry.line_count as i64)),
        ("pinned", JsonValue::Bool(entry.pinned)),
        ("redacted", JsonValue::Bool(entry.redacted)),
    ])
}

impl AutomationControlPlane {
    pub(super) fn rpc_clipboard_list(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.wm.ensure_clipboard_registry_seeded();
        let scope = match params.get("scope").and_then(JsonValue::as_str) {
            None | Some("all") => ClipboardFilterScope::All,
            Some("session" | "active_session") => ClipboardFilterScope::Session,
            Some("window" | "pane" | "active_window") => ClipboardFilterScope::Window,
            Some("pinned" | "pins") => ClipboardFilterScope::Pinned,
            Some("media" | "images" | "video") => ClipboardFilterScope::Media,
            Some(other) => {
                return Err(format!(
                    "invalid 'scope' parameter '{other}': expected all, session, window, pinned, or media"
                ));
            }
        };
        let session_filter = params
            .get("session")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);
        let pane_filter = params
            .get("window")
            .or_else(|| params.get("pane_id"))
            .and_then(JsonValue::as_usize);
        let active_session = self.wm.config().history_session.clone();
        let active_pane = self.wm.active_pane_id();
        let query = params.get("query").and_then(JsonValue::as_str).unwrap_or("");
        let limit = params
            .get("limit")
            .and_then(JsonValue::as_usize)
            .unwrap_or(100)
            .clamp(1, 200);

        let entries: Vec<JsonValue> = self
            .wm
            .clipboard_registry()
            .filtered_entries(
                scope,
                session_filter.as_deref(),
                pane_filter,
                &active_session,
                active_pane,
                query,
            )
            .into_iter()
            .take(limit)
            .map(entry_to_json)
            .collect();

        let win = self.wm.clipboard_window();
        Ok(JsonValue::obj([
            ("count", JsonValue::Number(entries.len() as i64)),
            (
                "total",
                JsonValue::Number(self.wm.clipboard_registry().len() as i64),
            ),
            ("scope", JsonValue::String(scope.label().to_ascii_lowercase())),
            ("active_session", JsonValue::String(active_session)),
            ("active_pane_id", JsonValue::Number(active_pane as i64)),
            ("window_open", JsonValue::Bool(win.active)),
            ("window_pinned", JsonValue::Bool(win.pinned)),
            ("window_docked", JsonValue::Bool(win.docked)),
            ("entries", JsonValue::Array(entries)),
        ]))
    }

    pub(super) fn rpc_clipboard_get(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.wm.ensure_clipboard_registry_seeded();
        let id = params
            .get("id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| "missing 'id' parameter".to_string())?;
        let entry = self
            .wm
            .clipboard_registry()
            .get_by_id(id)
            .ok_or_else(|| format!("clipboard entry #{id} not found"))?;
        Ok(entry_to_json(entry))
    }

    pub(super) fn rpc_clipboard_copy(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let text = params
            .get("text")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'text' parameter".to_string())?;
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(ClipboardWindowOp::CopyText(
                text.to_string(),
            )))
            .map_err(|e| e.to_string())?;
        let head = self
            .wm
            .clipboard_registry()
            .entries()
            .first()
            .map(entry_to_json)
            .unwrap_or(JsonValue::Null);
        Ok(JsonValue::obj([
            ("status", JsonValue::String("copied".to_string())),
            ("entry", head),
        ]))
    }

    pub(super) fn rpc_clipboard_paste(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.wm.ensure_clipboard_registry_seeded();
        let op = if let Some(id) = params
            .get("id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
        {
            let pos = self
                .wm
                .clipboard_registry()
                .entries()
                .iter()
                .position(|e| e.id == id)
                .ok_or_else(|| format!("clipboard entry #{id} not found"))?;
            ClipboardWindowOp::PasteIndex(pos + 1)
        } else if let Some(idx) = params.get("index").and_then(JsonValue::as_usize) {
            ClipboardWindowOp::PasteIndex(idx.max(1))
        } else {
            ClipboardWindowOp::PasteSelected
        };
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(op))
            .map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([
            ("status", JsonValue::String("pasted".to_string())),
            (
                "active_pane_id",
                JsonValue::Number(self.wm.active_pane_id() as i64),
            ),
        ]))
    }

    pub(super) fn rpc_clipboard_pin(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.wm.ensure_clipboard_registry_seeded();
        let pinned = params.get("pinned").and_then(JsonValue::as_bool);
        let op = if let Some(id) = params
            .get("id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
        {
            ClipboardWindowOp::PinEntryId(id, pinned)
        } else {
            ClipboardWindowOp::TogglePinSelected
        };
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(op))
            .map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([(
            "status",
            JsonValue::String("updated".to_string()),
        )]))
    }

    pub(super) fn rpc_clipboard_delete(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        self.wm.ensure_clipboard_registry_seeded();
        let op = if let Some(id) = params
            .get("id")
            .and_then(JsonValue::as_i64)
            .and_then(|n| u64::try_from(n).ok())
        {
            ClipboardWindowOp::DeleteEntryId(id)
        } else {
            ClipboardWindowOp::DeleteSelected
        };
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(op))
            .map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([
            ("status", JsonValue::String("deleted".to_string())),
            (
                "total",
                JsonValue::Number(self.wm.clipboard_registry().len() as i64),
            ),
        ]))
    }

    pub(super) fn rpc_clipboard_clear(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(ClipboardWindowOp::ClearUnpinned))
            .map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([
            ("status", JsonValue::String("cleared_unpinned".to_string())),
            (
                "remaining",
                JsonValue::Number(self.wm.clipboard_registry().len() as i64),
            ),
        ]))
    }

    pub(super) fn rpc_clipboard_window(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op_str = params.get("op").and_then(JsonValue::as_str).unwrap_or("toggle");
        let op = match op_str {
            "open" => ClipboardWindowOp::Open { scope: None },
            "close" => ClipboardWindowOp::Close,
            "toggle" => ClipboardWindowOp::Toggle,
            "dismiss" => ClipboardWindowOp::Dismiss,
            "blur" => ClipboardWindowOp::Blur,
            "pin" => ClipboardWindowOp::Pin(params.get("value").and_then(JsonValue::as_bool)),
            "unpin" => ClipboardWindowOp::Pin(Some(false)),
            "dock" => ClipboardWindowOp::Dock(params.get("value").and_then(JsonValue::as_bool)),
            "float" => ClipboardWindowOp::Dock(Some(false)),
            "all" => ClipboardWindowOp::SetScope(ClipboardFilterScope::All),
            "session" => ClipboardWindowOp::SetScope(ClipboardFilterScope::Session),
            "window" => ClipboardWindowOp::SetScope(ClipboardFilterScope::Window),
            "pinned" => ClipboardWindowOp::SetScope(ClipboardFilterScope::Pinned),
            "media" => ClipboardWindowOp::SetScope(ClipboardFilterScope::Media),
            "filter" | "search" => {
                let q = params.get("query").and_then(JsonValue::as_str).unwrap_or("");
                if q.is_empty() {
                    ClipboardWindowOp::SearchClear
                } else {
                    ClipboardWindowOp::SearchSet(q.to_string())
                }
            }
            "paste" => ClipboardWindowOp::PasteSelected,
            "yank" => ClipboardWindowOp::YankSelected,
            "clear" => ClipboardWindowOp::ClearUnpinned,
            other => return Err(format!("unknown clipboard.window op: {other}")),
        };
        let _ = self
            .wm
            .apply(Action::ClipboardWindow(op))
            .map_err(|e| e.to_string())?;
        let win = self.wm.clipboard_window();
        Ok(JsonValue::obj([
            ("open", JsonValue::Bool(win.active)),
            ("focused", JsonValue::Bool(win.focused)),
            ("pinned", JsonValue::Bool(win.pinned)),
            ("docked", JsonValue::Bool(win.docked)),
            (
                "scope",
                JsonValue::String(win.scope.label().to_ascii_lowercase()),
            ),
            ("search_query", JsonValue::String(win.search_query.clone())),
            ("selected", JsonValue::Number(win.selected as i64)),
        ]))
    }
}
