//! RPC handler for `session.map` (Phase 54, VIEW-02, VIEW-04).

use crate::action::Action;
use crate::api::json::JsonValue;
use crate::api::rpc::AutomationControlPlane;
use crate::wm::session_map::SessionMapOp;

impl AutomationControlPlane {
    pub(super) fn rpc_session_map(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let op_str = params.get("op").and_then(|v| v.as_str());
        if let Some(op) = op_str {
            let action_op = match op {
                "open" => SessionMapOp::Open,
                "close" => SessionMapOp::Close,
                "toggle" => SessionMapOp::Toggle,
                "dismiss" => SessionMapOp::Dismiss,
                "blur" => SessionMapOp::Blur,
                "pin" => {
                    let on = params.get("value").and_then(|v| v.as_bool());
                    SessionMapOp::Pin(on)
                }
                "dock" => {
                    let on = params.get("value").and_then(|v| v.as_bool());
                    SessionMapOp::Dock(on)
                }
                "pickup" => {
                    let pid = params.get("pane_id").and_then(|v| v.as_usize());
                    SessionMapOp::Pickup(pid)
                }
                "target" => {
                    let sess = params.get("session").and_then(|v| v.as_str()).unwrap_or("main");
                    SessionMapOp::TargetSession(sess.to_string())
                }
                "drop" => SessionMapOp::Drop,
                "cancel" => SessionMapOp::CancelDrag,
                "scale" => {
                    let delta = params.get("delta").and_then(|v| v.as_i64()).unwrap_or(10) as i16;
                    SessionMapOp::ScaleStep(delta)
                }
                _ => return Err(format!("unknown session.map op: {op}")),
            };
            let _ = self.wm.apply(Action::SessionMap(action_op));
        }

        let map = self.wm.session_map();
        let inputs = self.wm.build_session_pack_inputs();

        let mut out_sessions = Vec::new();
        for s in &inputs {
            let wins: Vec<JsonValue> = s
                .windows
                .iter()
                .map(|w| {
                    JsonValue::obj([
                        ("pane_id", JsonValue::Number(w.pane_id as i64)),
                        ("title", JsonValue::String(w.title.clone())),
                        ("focused", JsonValue::Bool(w.focused)),
                    ])
                })
                .collect();

            out_sessions.push(JsonValue::obj([
                ("name", JsonValue::String(s.session_name.clone())),
                (
                    "parent",
                    s.parent_session
                        .as_ref()
                        .map(|p| JsonValue::String(p.clone()))
                        .unwrap_or(JsonValue::Null),
                ),
                ("active", JsonValue::Bool(s.active)),
                ("windows", JsonValue::Array(wins)),
            ]));
        }

        let overrides: Vec<JsonValue> = map
            .window_session_overrides
            .iter()
            .map(|(&pid, sess)| {
                JsonValue::obj([
                    ("pane_id", JsonValue::Number(pid as i64)),
                    ("session", JsonValue::String(sess.clone())),
                ])
            })
            .collect();

        Ok(JsonValue::obj([
            ("active", JsonValue::Bool(map.active)),
            ("focused", JsonValue::Bool(map.focused)),
            ("pinned", JsonValue::Bool(map.pinned)),
            ("docked", JsonValue::Bool(map.docked)),
            (
                "target_session",
                map.target_session
                    .as_ref()
                    .map(|s| JsonValue::String(s.clone()))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "holding_pane_id",
                map.holding_pane_id
                    .map(|id| JsonValue::Number(id as i64))
                    .unwrap_or(JsonValue::Null),
            ),
            ("overrides", JsonValue::Array(overrides)),
            ("sessions", JsonValue::Array(out_sessions)),
        ]))
    }
}
