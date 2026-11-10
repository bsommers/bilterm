//! Executors for `Action::Context`, `Action::AgentQueue`, `Action::AgentGate` (UNI-R01, Phase 68).

use crate::api::json::JsonValue;
use crate::wm::WindowManager;
use crate::wm::context_bus::ContextEntry;
use crate::wm::context_queue_ops::{AgentGateOp, AgentQueueOp, ContextOp};
use crate::wm::prompt_queue::{TAIL_LINES, approve_payload, classify_blocked, deny_payload};

fn fail(wm: &mut WindowManager, msg: String) {
    wm.record_automation_error(msg.clone());
    wm.active_pane_mut().set_status_message(Some(msg));
}

fn entry_json(e: &ContextEntry) -> JsonValue {
    JsonValue::obj([
        ("key", JsonValue::String(e.key.clone())),
        ("value", JsonValue::String(e.value.clone())),
        ("sender", e.sender.map_or(JsonValue::Null, |s| JsonValue::Number(s as i64))),
        ("revision", JsonValue::Number(e.revision as i64)),
    ])
}

fn emit(wm: &mut WindowManager, status: String, data: JsonValue) {
    wm.set_command_data(data.to_json_string());
    wm.active_pane_mut().set_status_message(Some(status));
}

/// Runs one `:context` op; never quits.
pub fn execute_context(wm: &mut WindowManager, op: ContextOp) -> bool {
    match &op {
        ContextOp::Set { key, value } if key.is_empty() || value.is_empty() => {
            fail(wm, "Usage: :context set <key> <value>".to_string());
            return false;
        }
        ContextOp::Get { key } | ContextOp::Clear { key } if key.is_empty() => {
            fail(wm, "Usage: :context get|clear <key>".to_string());
            return false;
        }
        _ => {}
    }
    match op {
        ContextOp::Set { key, value } => {
            let sender = Some(wm.active_pane_id());
            match wm.context_bus().set(&key, &value, sender) {
                Ok(rev) => emit(
                    wm,
                    format!("context: set {key} (rev {rev})"),
                    JsonValue::obj([("key", JsonValue::String(key)), ("revision", JsonValue::Number(rev as i64))]),
                ),
                Err(e) => fail(wm, format!("context set: {e}")),
            }
        }
        ContextOp::Get { key } => match wm.context_bus().get(&key) {
            Some(e) => emit(wm, format!("context: {key} = {}", e.value), entry_json(&e)),
            None => fail(wm, format!("context get: no such key '{key}'")),
        },
        ContextOp::List { prefix } => {
            let entries = wm.context_bus().list(prefix.as_deref());
            let status = format!("context: {} entr{}", entries.len(), if entries.len() == 1 { "y" } else { "ies" });
            let items = JsonValue::Array(entries.iter().map(entry_json).collect());
            emit(wm, status, JsonValue::obj([("entries", items), ("revision", JsonValue::Number(wm.context_bus().revision() as i64))]));
        }
        ContextOp::Clear { key } => {
            if wm.context_bus().delete(&key) {
                emit(wm, format!("context: cleared {key}"), JsonValue::obj([("key", JsonValue::String(key))]));
            } else {
                fail(wm, format!("context clear: no such key '{key}'"));
            }
        }
    }
    false
}

/// Runs one `:agent queue` op against the active pane's queue; never quits.
pub fn execute_agent_queue(wm: &mut WindowManager, op: AgentQueueOp) -> bool {
    let id = wm.active_pane_id();
    match op {
        AgentQueueOp::Add(text) if text.trim().is_empty() => fail(wm, "Usage: :agent queue add <text>".to_string()),
        AgentQueueOp::Drop(0) => fail(wm, "Usage: :agent queue drop <n>  (n >= 1)".to_string()),
        AgentQueueOp::Add(text) => match wm.prompt_queue_mut().enqueue(id, text) {
            Ok(n) => emit(wm, format!("agent queue: queued ({n} pending)"), JsonValue::obj([("len", JsonValue::Number(n as i64))])),
            Err(e) => fail(wm, format!("agent queue add: {e:?}")),
        },
        AgentQueueOp::List => {
            let items: Vec<JsonValue> =
                wm.prompt_queue().items(id).iter().map(|s| JsonValue::String((*s).to_string())).collect();
            let n = items.len();
            let paused = wm.prompt_queue().is_paused(id);
            emit(
                wm,
                format!("agent queue: {n} pending"),
                JsonValue::obj([
                    ("pane_id", JsonValue::Number(id as i64)),
                    ("paused", JsonValue::Bool(paused)),
                    ("items", JsonValue::Array(items)),
                ]),
            );
        }
        AgentQueueOp::Clear => {
            let mut n = 0;
            while wm.prompt_queue_mut().pop_front(id).is_some() {
                n += 1;
            }
            emit(wm, format!("agent queue: cleared {n}"), JsonValue::obj([("cleared", JsonValue::Number(n))]));
        }
        AgentQueueOp::Drop(n) => match wm.prompt_queue_mut().remove(id, n - 1) {
            Some(text) => emit(wm, format!("agent queue: dropped #{n}"), JsonValue::obj([("dropped", JsonValue::String(text))])),
            None => {
                let len = wm.prompt_queue().len(id);
                fail(wm, format!("agent queue drop: #{n} out of range ({len} pending)"));
            }
        },
    }
    false
}

/// `:agent approve|deny`: classifies the active pane's tail and sends the matching keystrokes.
pub fn execute_agent_gate(wm: &mut WindowManager, op: AgentGateOp) -> bool {
    let verb = if op == AgentGateOp::Approve { "approve" } else { "deny" };
    let tail: Vec<String> = {
        let vt = wm.active_pane_mut().vt();
        let (rows, _) = vt.size();
        let non_empty: Vec<String> = (0..rows).map(|r| vt.row_text(r)).filter(|l| !l.trim().is_empty()).collect();
        non_empty[non_empty.len().saturating_sub(TAIL_LINES)..].to_vec()
    };
    let refs: Vec<&str> = tail.iter().map(String::as_str).collect();
    let Some(reason) = classify_blocked(&refs) else {
        fail(wm, format!("agent {verb}: active pane is not blocked on a prompt"));
        return false;
    };
    let payload = if op == AgentGateOp::Approve { approve_payload(&reason) } else { deny_payload(&reason) };
    let Some(bytes) = payload else {
        fail(wm, format!("agent {verb}: refusing to answer {reason:?} automatically"));
        return false;
    };
    match wm.active_pane_mut().write_input(&bytes) {
        Ok(()) => emit(
            wm,
            format!("agent {verb}: sent for {reason:?}"),
            JsonValue::obj([("reason", JsonValue::String(format!("{reason:?}"))), ("sent", JsonValue::Bool(true))]),
        ),
        Err(e) => fail(wm, format!("agent {verb}: write failed: {e}")),
    }
    false
}
