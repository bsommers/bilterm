//! Executor for `Action::Find` (Phase 50.1-01): modal state ops now, result-aware ops in 50.1-12.

use crate::wm::WindowManager;
use crate::wm::find_cmd::{FindOp, cap_query};

/// Rows currently listed in the Find window.
fn row_count(_wm: &WindowManager) -> usize {
    0
}

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}

/// Executes one Find operation; always returns `false` (never quits).
pub fn execute_find_command(wm: &mut WindowManager, op: FindOp) -> bool {
    match op {
        FindOp::Open { kind, query } => {
            wm.open_find(kind, &query);
            status(wm, format!("Find {} \"{}\"", kind.as_str(), cap_query(&query)));
        }
        FindOp::Close => wm.close_modal(),
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
