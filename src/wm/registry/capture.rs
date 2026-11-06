//! Bounded command ring, tick drain and exit-code join (FND-R01, FND-R03, FND-R04, plan 50.1-10).
//!
//! Panes push already-filtered commands into a private outbox (`CompositePane::submit_command`:
//! password input and secret-looking commands never get there). Each `WindowManager::tick`
//! drains the outboxes into a ring bounded by `CMD_RING_CAP`, tags the session and a monotonic
//! `seq`, then joins exit codes from the pane's OSC 133 command blocks.

use std::collections::HashMap;
use std::time::Instant;

use super::RegistryState;
use super::remote::transport_of;
use super::types::{
    CMD_CAP, CMD_RING_CAP, CapturedEntry, FindKind, FindRow, HistoryRow, NAME_CAP, Recency,
    ResultSource, RowData, TransportInfo,
};
use crate::pane::PaneId;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;

/// Newest ring entries examined for an exit code per tick (keeps the tick cost constant).
const EXIT_JOIN_WINDOW: usize = 256;

impl RegistryState {
    /// Appends a drained command, evicting the oldest beyond `CMD_RING_CAP`.
    fn push_entry(&mut self, mut entry: CapturedEntry) {
        entry.seq = self.next_seq;
        self.next_seq += 1;
        if self.cmds.len() >= CMD_RING_CAP {
            self.cmds.pop_front();
        }
        self.cmds.push_back(entry);
    }
}

/// History rows for the live ring (newest `seq` sorts first through `order`).
pub fn candidates(wm: &WindowManager) -> Vec<FindRow> {
    let mut infos: HashMap<PaneId, TransportInfo> = HashMap::new();
    let mut rows = Vec::with_capacity(wm.registry.cmds.len());
    for entry in &wm.registry.cmds {
        let info = infos
            .entry(entry.pane)
            .or_insert_with(|| transport_of(wm, entry.pane));
        let mut row = FindRow::empty(FindKind::History);
        row.id = format!("history:{}/{}", entry.session, entry.seq);
        row.label = entry.command.clone();
        let exit = entry.exit_code.map(|c| format!(" exit {c}")).unwrap_or_default();
        row.detail = format!(
            "#{} {}{exit}",
            entry.pane,
            entry.host.as_deref().unwrap_or("local")
        );
        row.haystack = match &entry.host {
            Some(host) => format!("{} {host}", entry.command),
            None => entry.command.clone(),
        };
        row.source = ResultSource::live(&entry.session);
        row.session = entry.session.clone();
        row.pane = Some(entry.pane);
        row.transport = info.transport;
        row.host = entry.host.clone();
        row.tmux_session = info.tmux_session.as_deref().map(|s| sanitize_display(s, NAME_CAP));
        row.tmux_window = info.tmux_window.as_deref().map(|s| sanitize_display(s, NAME_CAP));
        row.tmux_pane = info.tmux_pane;
        row.tags = info.tags.iter().map(|t| sanitize_display(t, NAME_CAP)).collect();
        row.order = entry.seq;
        row.data = RowData::History(HistoryRow {
            seq: entry.seq,
            pane: Some(entry.pane),
            cwd: entry.cwd.clone(),
            command: entry.command.clone(),
            exit_code: entry.exit_code,
            duration_ms: entry.duration_ms,
            at: Recency::Live(entry.at),
        });
        rows.push(row);
    }
    rows
}

/// Drains pane outboxes into the ring and joins exit codes (called from `WindowManager::tick`).
pub fn tick(wm: &mut WindowManager, _now: Instant) {
    let session = sanitize_display(&wm.config.history_session, NAME_CAP);
    for (&pane_id, pane) in wm.panes.iter_mut() {
        let (cmds, masked) = pane.drain_captured();
        wm.registry.masked_live += u64::from(masked);
        for c in cmds {
            wm.registry.push_entry(CapturedEntry {
                seq: 0,
                pane: pane_id,
                session: session.clone(),
                command: sanitize_display(&c.command, CMD_CAP),
                cwd: c.cwd,
                host: c.host,
                at: c.at,
                block_row: c.block_row,
                exit_code: None,
                duration_ms: None,
                exit_logged: false,
            });
        }
    }
    join_exit_codes(wm);
}

/// Copies exit code and duration from the pane's command block into recent ring entries and
/// writes one exit annotation per entry. A block evicted with scrollback leaves the exit unknown.
fn join_exit_codes(wm: &mut WindowManager) {
    let panes = &mut wm.panes;
    for entry in wm.registry.cmds.iter_mut().rev().take(EXIT_JOIN_WINDOW) {
        let Some(row) = entry.block_row.filter(|_| !entry.exit_logged) else {
            continue;
        };
        let Some(pane) = panes.get_mut(&entry.pane) else {
            continue;
        };
        let found = pane
            .vt()
            .command_blocks()
            .iter()
            .rev()
            .find(|b| {
                b.absolute_row == row && sanitize_display(&b.command, CMD_CAP) == entry.command
            })
            .and_then(|b| b.exit_code.map(|code| (code, b.duration_ms, b.command.clone())));
        let Some((code, duration_ms, original)) = found else {
            continue;
        };
        entry.exit_code = Some(code);
        entry.duration_ms = duration_ms;
        entry.exit_logged = true;
        pane.record_history_exit(&original, code);
    }
}
