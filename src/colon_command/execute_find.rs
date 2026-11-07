//! Executor for `Action::Find` (Phase 50.1-01, result-aware ops in 50.1-12): open, re-rank,
//! kind switching and the jump/prefill matrix. Enter never runs a command (T-50.1-27).
//!
//! Also owns [`find_results_json`], the one JSON shape RPC, MCP and `:find` share (D-U3: every
//! row carries `source`). Secret text can never appear: rows only hold sanitised, already
//! classified commands and `masked` is a number.

use crate::api::JsonValue;
use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::find_cmd::{FindOp, cap_query};
use crate::wm::registry::{
    CMD_CAP, FIND_DEFAULT_LIMIT, FIND_QUERY_CAP, FindResults, FindRow, NAME_CAP, Origin, ResultSource, RowData,
};

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
        FindOp::Close => close(wm),
        FindOp::Activate => activate(wm),
        FindOp::Prefill => prefill(wm),
        op => edit_modal(wm, op),
    }
    false
}

/// Closes the window and drops the per-open candidate and disk caches.
fn close(wm: &mut WindowManager) {
    wm.close_modal();
    wm.invalidate_find_cache();
}

/// Ops that change `Modal::Find` fields. Query and kind edits re-rank the cached candidates
/// (the disk is read at most once per kind per open); a no-op while the window is closed.
fn edit_modal(wm: &mut WindowManager, op: FindOp) {
    let rows = row_count(wm);
    let Some((kind, query, selected)) = wm.find_modal_mut_state() else {
        return;
    };
    let rerank = match op {
        FindOp::SetKind(k) => {
            *kind = k;
            true
        }
        FindOp::CycleKind(forward) => {
            *kind = if forward { kind.next() } else { kind.prev() };
            true
        }
        FindOp::SetQuery(text) => {
            *query = cap_query(&text);
            true
        }
        FindOp::Push(c) => {
            if query.chars().count() < FIND_QUERY_CAP {
                query.push(c);
            }
            true
        }
        FindOp::Pop => {
            query.pop();
            true
        }
        FindOp::ClearQuery => {
            query.clear();
            true
        }
        FindOp::Move(n) => {
            *selected = moved(*selected, n, rows);
            false
        }
        FindOp::Select(n) => {
            *selected = n.min(rows.saturating_sub(1));
            false
        }
        FindOp::Open { .. } | FindOp::Close | FindOp::Activate | FindOp::Prefill => false,
    };
    if !rerank {
        return;
    }
    *selected = 0;
    let (kind, query) = (*kind, query.clone());
    let results = wm.rerank_find(kind, &query, FIND_DEFAULT_LIMIT);
    status(wm, find_status_message(&results));
    wm.set_find_results(results);
}

/// The currently selected row (cloned), if the window is open and has results.
fn selected_row(wm: &WindowManager) -> Option<FindRow> {
    let (_, _, selected) = wm.find_modal_state()?;
    wm.find_results().rows.get(selected).cloned()
}

/// Display-safe text for status messages (rows are already sanitised; names are re-capped).
fn safe(text: &str) -> String {
    sanitize_display(text, NAME_CAP)
}

/// Focuses a live pane and closes the window; `false` (window stays open) when it vanished.
fn jump_to_pane(wm: &mut WindowManager, id: PaneId, tag: &str) -> bool {
    if wm.pane(id).is_none() {
        status(wm, format!("pane #{id} is gone"));
        return false;
    }
    wm.focus_and_raise(id);
    close(wm);
    status(wm, format!("Jumped to #{id} ({tag})"));
    true
}

/// Switches to the saved session `name` (autosaving the outgoing one); `false` on failure.
fn switch_session(wm: &mut WindowManager, name: &str, tag: &str) -> bool {
    let autosave = wm.config().layout_autosave;
    match wm.switch_or_create_session(name) {
        Ok((target, _, created)) => {
            close(wm);
            let note = match (created, autosave) {
                (true, _) => "no saved layout was found, new session created",
                (false, true) => "outgoing session autosaved",
                (false, false) => "layout autosave is off, outgoing session not saved",
            };
            status(wm, format!("Switched to session {} ({tag}); {note}", safe(&target)));
            true
        }
        Err(e) => {
            status(wm, format!("Cannot switch to session {} ({tag}): {}", safe(name), safe(&e.to_string())));
            false
        }
    }
}

/// Scrolls `id` to the newest command block whose command equals `command`.
fn scroll_to_command(wm: &mut WindowManager, id: PaneId, command: &str) -> bool {
    let Some(pane) = wm.pane_mut(id) else { return false };
    let want = command.trim();
    let row = pane
        .vt()
        .command_blocks()
        .iter()
        .rev()
        .find(|b| b.command.trim() == want || sanitize_display(&b.command, CMD_CAP).trim() == want)
        .map(|b| b.absolute_row);
    row.is_some_and(|r| pane.vt_mut().scroll_to_absolute_row(r))
}

/// Puts `command` in the active pane's prompt without running it (T-50.1-27): never touches the
/// PTY, never submits, `prompt.history` unchanged.
fn place_in_prompt(wm: &mut WindowManager, command: &str, tag: &str) {
    if !wm.active_pane().is_prompt_visible() {
        status(wm, "Find: the prompt is hidden (raw mode), nothing was placed".to_string());
        return;
    }
    wm.active_pane_mut().prompt_mut().set_text(command);
    close(wm);
    status(wm, format!("Placed command from {tag} in the prompt (not run)"));
}

fn no_selection(wm: &mut WindowManager) {
    status(wm, "Find: no result selected".to_string());
}

/// Enter: jump to the selected agent, pane, session or command (FND-R02).
fn activate(wm: &mut WindowManager) {
    if !wm.is_find_visible() {
        return;
    }
    let Some(row) = selected_row(wm) else {
        return no_selection(wm);
    };
    let tag = safe(&row.source.label());
    let live = row.source.origin == Origin::Live;
    match &row.data {
        RowData::Agent(_) | RowData::Pane(_) if live => {
            let Some(id) = row.pane else { return };
            if jump_to_pane(wm, id, &tag) && row.metadata_only {
                status(wm, format!("#{id} is a label-only remote row: no remote tmux is attached ({tag})"));
            }
        }
        RowData::Agent(_) | RowData::Pane(_) => {
            if switch_session(wm, &row.session, &tag)
                && let Some(id) = row.pane
                && wm.pane(id).is_some()
            {
                wm.focus_and_raise(id);
            }
        }
        RowData::Session(s) => match row.source.origin {
            Origin::Live => {
                close(wm);
                status(wm, format!("Already in session {} ({tag})", safe(&s.name)));
            }
            Origin::SavedSession => {
                switch_session(wm, &s.name, &tag);
            }
            Origin::HistoryLog => {
                status(wm, format!("session {} has history only (no saved layout)", safe(&s.name)));
            }
            Origin::Daemon => {
                close(wm);
                status(wm, format!("Daemon session {} ({tag})", safe(&s.name)));
            }
        },
        RowData::History(h) => {
            let live_pane = h.pane.filter(|id| live && wm.pane(*id).is_some());
            if let Some(id) = live_pane {
                let scrolled = scroll_to_command(wm, id, &h.command);
                if jump_to_pane(wm, id, &tag) {
                    let how = if scrolled { "scrolled to the command" } else { "command no longer in scrollback" };
                    status(wm, format!("Jumped to #{id} ({tag}); {how}"));
                }
            } else {
                place_in_prompt(wm, &h.command, &tag);
            }
        }
    }
}

/// Ctrl+Y: place the selected history command in the prompt, never run it (T-50.1-27).
fn prefill(wm: &mut WindowManager) {
    if !wm.is_find_visible() {
        return;
    }
    let Some(row) = selected_row(wm) else {
        return no_selection(wm);
    };
    match &row.data {
        RowData::History(h) => place_in_prompt(wm, &h.command, &safe(&row.source.label())),
        _ => status(wm, "Only history rows can be placed in the prompt".to_string()),
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
