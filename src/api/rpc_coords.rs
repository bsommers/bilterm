//! JSON-RPC 2.0 handlers for `coords.resolve` / `coords.format` (`COORD-04`, Phase 71).

use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::pane::PaneId;
use crate::wm::coordinates::{CoordDisplayStyle, TargetCoord};

impl AutomationControlPlane {
    /// `coords.resolve` `{coord}` -> `{panes:[id], coords:["d0:w1:main:y2"]}`.
    pub(super) fn rpc_coords_resolve(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let text = params
            .get("coord")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| "missing 'coord' parameter".to_string())?;
        let target = TargetCoord::parse(text).map_err(|e| e.to_string())?;
        let res = self.wm.resolve_coord(&target);
        Ok(JsonValue::obj([
            ("panes", JsonValue::Array(res.panes.iter().map(|p| JsonValue::Number(*p as i64)).collect())),
            ("coords", JsonValue::Array(res.coords.iter().map(|c| JsonValue::String(c.to_path())).collect())),
        ]))
    }

    /// `coords.format` `{pane_id?, style?}` -> `{text, path}` (style defaults to compact).
    pub(super) fn rpc_coords_format(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let id: PaneId = match params.get("pane_id") {
            Some(v) => v.as_usize().ok_or_else(|| "invalid 'pane_id' parameter".to_string())? as PaneId,
            None => self.wm.active_pane_id(),
        };
        let style = match params.get("style").and_then(JsonValue::as_str) {
            Some(s) => CoordDisplayStyle::from_name(s)
                .ok_or_else(|| format!("unknown style '{s}' (accepted: verbose, compact, daemon, off)"))?,
            None => CoordDisplayStyle::Compact,
        };
        let c = self.wm.coord_of(id).ok_or_else(|| format!("no such pane {id}"))?;
        let text = c.format(style).map(JsonValue::String).unwrap_or(JsonValue::Null);
        Ok(JsonValue::obj([("text", text), ("path", JsonValue::String(c.to_path()))]))
    }
}
