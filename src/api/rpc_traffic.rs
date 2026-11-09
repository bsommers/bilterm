//! JSON-RPC 2.0 handlers for `traffic.*` (`TMON-04`, Phase 64-03). The data comes from
//! `WindowManager::traffic_group_json` (the same text `:traffic show` stores), so colon, direct
//! RPC, TCP, HTTP and MCP return byte-identical snapshots.

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::traffic::{GroupKey, SortKey, SortSpec};
use crate::wm::traffic_monitor::TrafficMonitorOp;
use crate::wm::traffic_monitor::data::{
    GROUP_NAMES, TrafficQuery, clean_filter, history_json, names_of, parse_group, parse_locality, parse_sort,
};

fn parsed(text: String) -> Result<JsonValue, String> {
    JsonValue::parse(&text).ok_or_else(|| "internal error: traffic JSON did not parse".to_string())
}

fn str_param<'a>(p: &'a JsonValue, k: &str) -> Option<&'a str> {
    p.get(k).and_then(JsonValue::as_str)
}

/// Validates the shared `group_by/sort/descending/locality/filter/window/limit` parameters.
fn query_from(p: &JsonValue) -> Result<TrafficQuery, String> {
    let mut q = TrafficQuery::default();
    if let Some(g) = str_param(p, "group_by") {
        q.group = parse_group(g)?;
    }
    let key = match str_param(p, "sort") {
        Some(s) => parse_sort(s)?,
        None => SortKey::Rate1s,
    };
    let descending = p.get("descending").and_then(JsonValue::as_bool).unwrap_or(key != SortKey::Name);
    q.sort = SortSpec { key, descending };
    if let Some(l) = str_param(p, "locality") {
        q.filter.locality = parse_locality(l)?;
    }
    if let Some(f) = str_param(p, "filter") {
        q.filter.text = clean_filter(f);
    }
    if let Some(w) = p.get("window").and_then(JsonValue::as_usize) {
        q = q.with_window(w);
    }
    if let Some(n) = p.get("limit").and_then(JsonValue::as_usize) {
        q = q.with_limit(n);
    }
    Ok(q)
}

impl AutomationControlPlane {
    /// Applies a traffic action as an API-origin caller (write-gated verbs are refused unless
    /// `[api] allow_config_write = true`).
    fn traffic_apply_api(&mut self, op: TrafficMonitorOp) -> Result<(), String> {
        self.wm.set_api_origin(true);
        let r = self.wm.apply_automation(Action::TrafficMonitor(op), None);
        self.wm.set_api_origin(false);
        r.map(|_| ())
    }

    fn traffic_window_json(&self) -> JsonValue {
        use crate::wm::traffic_monitor::data::{group_name, locality_name, sort_name};
        let w = self.wm.traffic_monitor();
        JsonValue::obj([
            ("open", JsonValue::Bool(w.active())),
            ("focused", JsonValue::Bool(w.focused)),
            ("pinned", JsonValue::Bool(w.pinned)),
            ("docked", JsonValue::Bool(w.docked)),
            ("paused", JsonValue::Bool(w.paused.is_some())),
            ("group_by", JsonValue::String(group_name(w.group).to_string())),
            ("sort", JsonValue::String(sort_name(w.sort.key).to_string())),
            ("descending", JsonValue::Bool(w.sort.descending)),
            ("locality", JsonValue::String(locality_name(w.filter.locality).to_string())),
            ("filter", JsonValue::String(w.filter.text.clone())),
            ("selected", JsonValue::Number(w.selected as i64)),
        ])
    }

    pub(super) fn rpc_traffic_snapshot(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let q = query_from(params)?;
        parsed(self.wm.traffic_group_json(&q))
    }

    pub(super) fn rpc_traffic_history(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let group: GroupKey = match str_param(params, "group_by") {
            Some(g) => parse_group(g)?,
            None => GroupKey::Connection,
        };
        let key = str_param(params, "key").ok_or_else(|| {
            format!("missing 'key' parameter (a row key of group_by={}; groups: {})", crate::wm::traffic_monitor::data::group_name(group), names_of(GROUP_NAMES))
        })?;
        let n = params.get("n").and_then(JsonValue::as_usize).unwrap_or(60).clamp(1, 60);
        let q = TrafficQuery { group, ..TrafficQuery::default() }.with_limit(500);
        let res = self.wm.traffic_group_result(&q);
        let row = res.rows.iter().find(|r| r.key == key);
        parsed(history_json(row, key, group, n))
    }

    pub(super) fn rpc_traffic_window(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action = str_param(params, "action").or_else(|| str_param(params, "op")).unwrap_or("toggle");
        let action = action.trim().to_ascii_lowercase().replace('_', "-");
        let value = params.get("value").map(|v| match v {
            JsonValue::String(s) => s.clone(),
            JsonValue::Bool(b) => (if *b { "on" } else { "off" }).to_string(),
            JsonValue::Number(n) => n.to_string(),
            _ => String::new(),
        });
        let mut line = format!(":traffic {action}");
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            line.push(' ');
            line.push_str(&v);
        }
        let parsed = crate::colon_command::parse_colon_command(&line)
            .ok_or_else(|| "internal error: not a colon command".to_string())??;
        let Action::TrafficMonitor(op) = parsed else {
            return Err("not a traffic action".to_string());
        };
        self.traffic_apply_api(op)?;
        Ok(self.traffic_window_json())
    }

    pub(super) fn rpc_traffic_reset(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        self.traffic_apply_api(TrafficMonitorOp::Reset)?;
        Ok(JsonValue::obj([("reset", JsonValue::Bool(true))]))
    }

    pub(super) fn rpc_traffic_config(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        if let Some(set) = params.get("set") {
            let JsonValue::Object(entries) = set else {
                return Err("'set' must be an object with enabled, classify and/or oia_indicator".to_string());
            };
            for (k, v) in entries.iter() {
                let b = v.as_bool().ok_or_else(|| format!("'{k}' must be a boolean"))?;
                let op = match k.as_str() {
                    "enabled" => TrafficMonitorOp::Enable(Some(b)),
                    "classify" => TrafficMonitorOp::Classify(Some(b)),
                    "oia_indicator" => TrafficMonitorOp::Oia(Some(b)),
                    other => {
                        return Err(format!("unknown traffic setting '{other}' (accepted: enabled, classify, oia_indicator)"));
                    }
                };
                self.traffic_apply_api(op)?;
            }
        }
        let c = &self.wm.config().traffic;
        let enabled = self.wm.traffic_snapshot(1).enabled;
        Ok(JsonValue::obj([
            ("enabled", JsonValue::Bool(enabled)),
            ("classify", JsonValue::Bool(self.wm.traffic_monitor().classify_override.unwrap_or(c.classify))),
            ("oia_indicator", JsonValue::Bool(c.oia_indicator)),
            ("label_hosts", JsonValue::Bool(c.label_hosts)),
        ]))
    }
}
