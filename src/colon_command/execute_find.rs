//! Executor for `Action::Find` (Phase 50.1-01): modal state ops now, result-aware ops in 50.1-12.
//!
//! Also owns [`find_results_json`], the one JSON shape RPC, MCP and `:find` share (D-U3: every
//! row carries `source`). Secret text can never appear: rows only hold sanitised, already
//! classified commands and `masked` is a number.

use crate::api::JsonValue;
use crate::wm::WindowManager;
use crate::wm::find_cmd::{FindOp, cap_query};
use crate::wm::registry::{FIND_DEFAULT_LIMIT, FindResults, FindRow, Origin, ResultSource, RowData};

/// Rows currently listed in the Find window.
fn row_count(wm: &WindowManager) -> usize {
    wm.find_results().rows.len()
}

fn opt_str(v: &Option<String>) -> JsonValue {
    v.clone().map_or(JsonValue::Null, JsonValue::String)
}

fn num(n: u64) -> JsonValue {
    JsonValue::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

fn source_json(src: &ResultSource) -> JsonValue {
    JsonValue::obj([
        ("kind", JsonValue::String(src.origin.as_str().to_string())),
        ("name", JsonValue::String(src.name.clone())),
        ("path", opt_str(&src.path)),
    ])
}

fn row_json(row: &FindRow) -> JsonValue {
    let mut entries: Vec<(&str, JsonValue)> = vec![
        ("id", JsonValue::String(row.id.clone())),
        ("kind", JsonValue::String(row.kind.as_str().to_string())),
        ("row", JsonValue::String(row.data.row_name().to_string())),
        ("label", JsonValue::String(row.label.clone())),
        ("detail", JsonValue::String(row.detail.clone())),
        ("score", num(u64::from(row.score))),
        ("source", source_json(&row.source)),
        ("also_in", JsonValue::Array(row.also_in.iter().map(source_json).collect())),
        ("transport", JsonValue::String(row.transport.as_str().to_string())),
        ("host", opt_str(&row.host)),
        ("tmux_session", opt_str(&row.tmux_session)),
        ("tmux_window", opt_str(&row.tmux_window)),
        ("tmux_pane", row.tmux_pane.map_or(JsonValue::Null, |n| num(n as u64))),
        ("tags", JsonValue::Array(row.tags.iter().cloned().map(JsonValue::String).collect())),
        ("metadata_only", JsonValue::Bool(row.metadata_only)),
        ("pane", row.pane.map_or(JsonValue::Null, |n| num(n as u64))),
        ("session", JsonValue::String(row.session.clone())),
    ];
    match &row.data {
        RowData::Agent(a) => {
            entries.push(("state", JsonValue::String(a.state.as_str().to_string())));
            entries.push(("since_ms", num(a.since_ms)));
            entries.push(("profile", opt_str(&a.profile)));
        }
        RowData::Session(s) => {
            entries.push(("active", JsonValue::Bool(s.active)));
            entries.push(("pane_count", num(s.pane_count as u64)));
            entries.push(("parent", opt_str(&s.parent)));
        }
        RowData::Pane(p) => {
            entries.push(("title", JsonValue::String(p.title.clone())));
            entries.push(("cwd", opt_str(&p.cwd)));
            entries.push(("cwd_trusted", JsonValue::Bool(p.cwd_trusted)));
            entries.push(("running_command", opt_str(&p.running_command)));
        }
        RowData::History(h) => {
            entries.push(("seq", num(h.seq)));
            entries.push(("command", JsonValue::String(h.command.clone())));
            entries.push(("exit_code", h.exit_code.map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c)))));
            entries.push(("duration_ms", h.duration_ms.map_or(JsonValue::Null, num)));
        }
    }
    JsonValue::obj(entries)
}

/// The envelope returned by `find.*`, stashed by `:find ...` and wrapped by the MCP tool:
/// `{"kind","query","total","returned","truncated","masked","rows":[...]}`.
pub fn find_results_json(results: &FindResults) -> JsonValue {
    JsonValue::obj([
        ("kind", JsonValue::String(results.kind.as_str().to_string())),
        ("query", JsonValue::String(results.query.clone())),
        ("total", num(results.total as u64)),
        ("returned", num(results.rows.len() as u64)),
        ("truncated", JsonValue::Bool(results.truncated)),
        ("masked", num(results.masked as u64)),
        ("rows", JsonValue::Array(results.rows.iter().map(row_json).collect())),
    ])
}

/// `Find <kind> "<query>": <n> results[, <m> masked] (<n_live> live, <n_saved> saved, <n_log> log)`.
pub fn find_status_message(results: &FindResults) -> String {
    let count = |origin: Origin| results.rows.iter().filter(|r| r.source.origin == origin).count();
    let masked = if results.masked > 0 { format!(", {} masked", results.masked) } else { String::new() };
    format!(
        "Find {} \"{}\": {} results{masked} ({} live, {} saved, {} log)",
        results.kind.as_str(),
        results.query,
        results.total,
        count(Origin::Live),
        count(Origin::SavedSession),
        count(Origin::HistoryLog),
    )
}

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}

/// Executes one Find operation; always returns `false` (never quits).
pub fn execute_find_command(wm: &mut WindowManager, op: FindOp) -> bool {
    match op {
        FindOp::Open { kind, query } => {
            wm.open_find(kind, &query);
            let results = wm.run_find(kind, &cap_query(&query), FIND_DEFAULT_LIMIT);
            wm.set_command_data(find_results_json(&results).to_json_string());
            status(wm, find_status_message(&results));
            wm.set_find_results(results);
        }
        FindOp::Close => {
            wm.close_modal();
            wm.invalidate_find_cache();
        }
        FindOp::Activate | FindOp::Prefill => {
            if wm.is_find_visible() {
                status(wm, "Find: no result selected".to_string());
            }
        }
        op => edit_modal(wm, op),
    }
    false
}

/// Ops that only change `Modal::Find` fields; a no-op while the window is closed.
fn edit_modal(wm: &mut WindowManager, op: FindOp) {
    let rows = row_count(wm);
    let Some((kind, query, selected)) = wm.find_modal_mut_state() else {
        return;
    };
    match op {
        FindOp::SetKind(k) => {
            *kind = k;
            *selected = 0;
        }
        FindOp::CycleKind(forward) => {
            *kind = if forward { kind.next() } else { kind.prev() };
            *selected = 0;
        }
        FindOp::SetQuery(text) => {
            *query = cap_query(&text);
            *selected = 0;
        }
        FindOp::Push(c) => {
            if query.chars().count() < crate::wm::find_cmd::QUERY_CAP {
                query.push(c);
            }
            *selected = 0;
        }
        FindOp::Pop => {
            query.pop();
            *selected = 0;
        }
        FindOp::ClearQuery => {
            query.clear();
            *selected = 0;
        }
        FindOp::Move(n) => *selected = moved(*selected, n, rows),
        FindOp::Select(n) => *selected = n.min(rows.saturating_sub(1)),
        FindOp::Open { .. } | FindOp::Close | FindOp::Activate | FindOp::Prefill => {}
    }
}

/// New selection after `Move(n)`: `i16::MIN` top, `i16::MAX` bottom, otherwise relative, clamped.
fn moved(selected: usize, n: i16, rows: usize) -> usize {
    let last = rows.saturating_sub(1);
    match n {
        i16::MIN => 0,
        i16::MAX => last,
        n if n < 0 => selected.saturating_sub(usize::from(n.unsigned_abs())).min(last),
        n => selected.saturating_add(usize::from(n.unsigned_abs())).min(last),
    }
}
