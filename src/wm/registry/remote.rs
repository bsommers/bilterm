//! Transport facts about a pane: local, SSH, tmux, SSH+tmux (FND-R03, plan 50.1-06).
//!
//! Gaps encoded as data instead of guessed: `metadata_only` (an `:ssh-tmux` pane starts no tmux,
//! G2) and `cwd_trusted` (a remote pane's cwd is only known when the VT reported OSC 7, G8).

use super::types::{NAME_CAP, Transport, TransportInfo};
use crate::pane::PaneId;
use crate::remote::FleetNodeKind;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;

/// How `id` reaches its shell.
///
/// Order: a pane with a `remote_target` is `Ssh` (`user@host`) or `SshTmux` (`host/session`); a
/// pane mirrored from an attached `tmux -CC` bridge is `Tmux`; everything else is `Local`.
/// Never consults the local cwd fallback (wrong for remote panes, gap G8).
pub fn transport_of(wm: &WindowManager, id: PaneId) -> TransportInfo {
    let Some(pane) = wm.panes.get(&id) else {
        return TransportInfo::local();
    };
    let osc7 = pane.vt().reported_cwd().is_some();
    let mut info = TransportInfo::local();

    if let Some(target) = pane.remote_target() {
        match target.split_once('/') {
            Some((host, session)) => {
                info.transport = Transport::SshTmux;
                info.host = Some(clean(host));
                info.tmux_session = Some(clean(session));
                // `connect_ssh_tmux` starts no tmux (gap G2): the row is a label only.
                info.metadata_only = true;
            }
            None => {
                info.transport = Transport::Ssh;
                info.host = Some(clean(target));
            }
        }
        info.tags = fleet_tags(wm, id);
        info.cwd_trusted = osc7;
        return info;
    }

    // `tmux_bridge::new()` hard-maps `%0 -> pane 1`; only trust the map while a control stream
    // is attached, so a fresh bridge never mislabels a local pane.
    if wm.tmux_control.is_some()
        && let Some(&tmux_pane) = wm.tmux_bridge.bilterm_to_tmux.get(&id)
    {
        info.transport = Transport::Tmux;
        info.tmux_pane = Some(tmux_pane);
        info.tmux_session = wm.tmux_bridge.active_session_name.as_deref().map(clean);
        info.tmux_window = wm.tmux_bridge.window_of(tmux_pane).map(|w| {
            match wm.tmux_bridge.window_name(w).map(clean).filter(|n| !n.is_empty()) {
                Some(name) => format!("@{w}:{name}"),
                None => format!("@{w}"),
            }
        });
        info.cwd_trusted = osc7;
    }
    info
}

/// Display-safe, bounded copy of attacker-influenced text (T-50.1-12).
fn clean(raw: &str) -> String {
    sanitize_display(raw, NAME_CAP)
}

/// Fleet tags of the node(s) that own pane `id`, deduplicated, in node-name order.
fn fleet_tags(wm: &WindowManager, id: PaneId) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for node in wm.fleet().nodes.values() {
        let owns = match &node.kind {
            FleetNodeKind::SshPane { pane_id, .. } => *pane_id == id,
            FleetNodeKind::SshTmux { pane_ids, .. } => pane_ids.contains(&id),
            FleetNodeKind::HostProfile { .. } => false,
        };
        if owns {
            for t in &node.tags {
                let t = clean(t);
                if !t.is_empty() && !tags.contains(&t) {
                    tags.push(t);
                }
            }
        }
    }
    tags
}
