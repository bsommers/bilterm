//! Phase 70 RPC: `agent.hud`, `agent.replay`, `agent.watchdog` (HUD-01..03).
//!
//! Reads come from the same state the HUD / replay view / watchdog render. The optional `op`
//! parameter of `agent.hud` and `agent.replay` lowers to the same [`Action`] as the colon form and
//! runs through `WindowManager::apply` (`UNI-R01`).

use super::agents_hud_json::{hud_snapshot, replay_list, replay_turn, watchdog_status};
use super::json::JsonValue;
use super::playbook::AutomationEvent;
use super::rpc::{AutomationControlPlane, SnapshotPolicy};
use crate::action::Action;
use crate::pane::PaneId;
use crate::wm::agents_hud_cmd::{HudOp, ReplayOp};
use crate::wm::mission_control_hud::HudButtonKind;

impl AutomationControlPlane {
    /// The pane named by `selector` / `pane_id`, if either is present.
    fn optional_target(&mut self, params: &JsonValue) -> Result<Option<PaneId>, String> {
        if params.get("selector").is_some() || params.get("pane_id").is_some() {
            return self.target_pane(params).map(Some);
        }
        Ok(None)
    }

    /// Moves watchdog alerts queued by the window manager into the RPC event queue.
    pub(super) fn drain_watchdog_events(&mut self) {
        for a in self.wm.take_watchdog_events() {
            self.push_event(AutomationEvent::WatchdogAlert {
                pane_id: a.pane,
                kind: a.kind.to_string(),
                detail: a.detail,
                count: a.count,
            });
        }
    }

    /// `agent.hud`: snapshot of every HUD tile; optional `op` drives the HUD like its colon form
    /// (`open|close|toggle|next|prev|page_next|page_prev|select|approve|prompt|diff|zoom`).
    pub(super) fn rpc_agent_hud(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let mut status = JsonValue::Null;
        if let Some(op) = params.get("op").and_then(JsonValue::as_str) {
            let pane = self.optional_target(params)?;
            let generation = params
                .get("generation")
                .and_then(JsonValue::as_i64)
                .and_then(|g| u64::try_from(g).ok());
            let press = |kind| HudOp::Press {
                pane,
                kind,
                generation,
            };
            let op = match op {
                "open" => HudOp::Open,
                "close" => HudOp::Close,
                "toggle" => HudOp::Toggle,
                "next" => HudOp::Step(1),
                "prev" => HudOp::Step(-1),
                "page_next" => HudOp::Page(1),
                "page_prev" => HudOp::Page(-1),
                "select" => HudOp::SelectPane(pane.ok_or("select needs 'pane_id' or 'selector'")?),
                "approve" => press(HudButtonKind::Approve),
                "prompt" => press(HudButtonKind::Prompt),
                "diff" => press(HudButtonKind::Diff),
                "zoom" => press(HudButtonKind::Zoom),
                other => return Err(format!("unknown hud op '{other}'")),
            };
            self.apply_with_policy(Action::AgentsHud(op), SnapshotPolicy::Trusted)?;
            status = JsonValue::String("ok".to_string());
        }
        self.wm.agents_hud_refresh(false);
        let JsonValue::Object(mut out) = hud_snapshot(&self.wm) else {
            return Err("internal".to_string());
        };
        out.insert("status".to_string(), status);
        Ok(JsonValue::Object(out))
    }

    /// `agent.replay`: turns recorded for a pane (`turn_id` returns one turn with its output).
    /// Optional `op` (`open|close|prev|next|first|last|turn|scroll`) drives the read-only view.
    pub(super) fn rpc_agent_replay(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let pane = self.target_pane(params)?;
        if let Some(op) = params.get("op").and_then(JsonValue::as_str) {
            let turn = params
                .get("turn_id")
                .and_then(JsonValue::as_i64)
                .and_then(|t| u64::try_from(t).ok());
            let lines = params
                .get("lines")
                .and_then(JsonValue::as_i64)
                .map_or(1, |l| l.clamp(-1000, 1000) as i16);
            let op = match op {
                "show" => ReplayOp::Show,
                "open" => ReplayOp::Open(pane),
                "close" => ReplayOp::Close,
                "prev" => ReplayOp::Prev,
                "next" => ReplayOp::Next,
                "first" => ReplayOp::First,
                "last" => ReplayOp::Last,
                "turn" => ReplayOp::Goto(turn.ok_or("turn needs 'turn_id'")?),
                "scroll" => ReplayOp::Scroll(lines),
                other => return Err(format!("unknown replay op '{other}'")),
            };
            self.apply_with_policy(Action::AgentsReplay(op), SnapshotPolicy::Trusted)?;
        }
        if let Some(t) = params
            .get("turn_id")
            .and_then(JsonValue::as_i64)
            .and_then(|t| u64::try_from(t).ok())
        {
            return replay_turn(&self.wm, pane, t)
                .ok_or_else(|| format!("pane #{pane} has no turn {t}"));
        }
        Ok(replay_list(&self.wm, pane))
    }

    /// `agent.watchdog`: configuration, watched pane count and recent alerts.
    pub(super) fn rpc_agent_watchdog(&mut self, _params: &JsonValue) -> Result<JsonValue, String> {
        self.drain_watchdog_events();
        Ok(watchdog_status(&self.wm))
    }
}
