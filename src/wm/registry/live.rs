//! Live agents, panes and the active workspace session as registry rows (FND-R01, D-U3).
//!
//! Read-only `&self` views over in-memory state: no disk I/O, no local-cwd resolver (it would
//! show a local path for an SSH pane, research B pitfall 1). Every free-text field goes through
//! `sanitize_display` so remote-controlled titles never reach the UI or JSON (T-50.1-06).

use super::remote::transport_of;
use super::types::{
    AgentRow, FindKind, FindRow, NAME_CAP, PANES_PER_SESSION_CAP, PaneRow, Registry, ResultSource, RowData,
    SessionRow, TransportInfo, Transport, CMD_CAP,
};
use crate::pane::CompositePane;
use crate::pane::PaneId;
use crate::pane::redact::classify;
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::agent_state::AgentStatus;

/// Characters of a cwd shown in a row.
const PATH_CAP: usize = 256;

/// Everything the live workspace knows, no disk I/O.
pub fn snapshot(wm: &WindowManager) -> Registry {
    Registry {
        agents: agent_rows(wm),
        panes: pane_rows(wm),
        sessions: vec![session_row(wm)],
        history: Vec::new(),
        masked: 0,
    }
}

/// Live candidates for `kind` (Agent: agents; Session: session and pane rows; History: none).
pub fn candidates(wm: &WindowManager, kind: FindKind) -> Vec<FindRow> {
    match kind {
        FindKind::Agent => agent_rows(wm),
        FindKind::Session => {
            let mut rows = vec![session_row(wm)];
            rows.extend(pane_rows(wm));
            rows
        }
        FindKind::History => Vec::new(),
    }
}

/// Name of the active workspace session (`config.history_session`, sanitised).
fn session_name(wm: &WindowManager) -> String {
    sanitize_display(&wm.config.history_session, NAME_CAP)
}

fn clean(raw: &str) -> String {
    sanitize_display(raw, NAME_CAP)
}

/// `12s` / `3m` / `1h`.
fn age(since_ms: u64) -> String {
    let secs = since_ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// A live row with the transport facts of `pane` copied in.
fn base_row(kind: FindKind, session: &str, pane: PaneId, info: &TransportInfo) -> FindRow {
    let mut row = FindRow::empty(kind);
    row.source = ResultSource::live(session);
    row.session = session.to_string();
    row.pane = Some(pane);
    row.transport = info.transport;
    row.host = info.host.as_deref().map(clean);
    row.tmux_session = info.tmux_session.as_deref().map(clean);
    row.tmux_window = info.tmux_window.as_deref().map(clean);
    row.tmux_pane = info.tmux_pane;
    row.tags = info.tags.iter().map(|t| clean(t)).collect();
    row.metadata_only = info.metadata_only;
    row.order = u64::MAX - pane as u64;
    row
}

fn pane_title(pane: &CompositePane) -> String {
    clean(pane.title())
}

fn agent_rows(wm: &WindowManager) -> Vec<FindRow> {
    let session = session_name(wm);
    wm.agent_statuses().into_iter().filter_map(|st| agent_row(wm, &session, st)).collect()
}

fn agent_row(wm: &WindowManager, session: &str, st: AgentStatus) -> Option<FindRow> {
    let pane = wm.panes.get(&st.pane_id)?;
    let info = transport_of(wm, st.pane_id);
    let mut row = base_row(FindKind::Agent, session, st.pane_id, &info);
    let name = clean(&st.name);
    let title = pane_title(pane);
    let profile = st.profile.as_deref().map(clean);
    let host_word = row.host.clone().unwrap_or_else(|| "local".to_string());
    row.id = format!("agent:{session}/{}", st.pane_id);
    row.label = clean(&format!("{name} #{}", st.pane_id));
    row.detail = format!("{} {} {}", st.state.as_str(), host_word, age(st.since_ms));
    row.haystack = [
        row.label.as_str(),
        title.as_str(),
        row.host.as_deref().unwrap_or(""),
        st.state.as_str(),
        profile.as_deref().unwrap_or(""),
    ]
    .join(" ");
    row.data = RowData::Agent(AgentRow {
        pane: st.pane_id,
        name,
        state: st.state,
        since_ms: st.since_ms,
        profile,
        title,
    });
    Some(row)
}

/// The cwd to show for `pane`, only when it says something true about the pane's shell: an OSC 7
/// report always does; the locally tracked cwd only for a local pane.
fn trusted_cwd(pane: &CompositePane, info: &TransportInfo) -> Option<String> {
    let remote = pane.remote_target().is_some() || !info.cwd_trusted;
    let path = pane.vt().reported_cwd().or_else(|| if remote { None } else { pane.tracked_cwd() })?;
    let shown = sanitize_display(&path.to_string_lossy(), PATH_CAP);
    (!shown.is_empty()).then_some(shown)
}

/// `false` for a remote pane that has not reported its own cwd (OSC 7).
fn cwd_trusted(pane: &CompositePane, info: &TransportInfo) -> bool {
    pane.vt().reported_cwd().is_some() || (info.cwd_trusted && pane.remote_target().is_none())
}

/// Running command text; a secret-looking command is never shown (D-U2).
fn running_command(pane: &CompositePane) -> Option<String> {
    let cmd = pane.prompt().running_command.as_deref()?.trim();
    if classify(cmd).is_some() {
        return None;
    }
    let shown = sanitize_display(cmd, CMD_CAP);
    (!shown.is_empty()).then_some(shown)
}

fn pane_rows(wm: &WindowManager) -> Vec<FindRow> {
    let session = session_name(wm);
    wm.panes
        .iter()
        .take(PANES_PER_SESSION_CAP)
        .map(|(&id, pane)| pane_row(wm, &session, id, pane))
        .collect()
}

fn pane_row(wm: &WindowManager, session: &str, id: PaneId, pane: &CompositePane) -> FindRow {
    let info = transport_of(wm, id);
    let mut row = base_row(FindKind::Session, session, id, &info);
    let title = pane_title(pane);
    let cwd = trusted_cwd(pane, &info);
    let running = running_command(pane);
    let mut parts = vec![format!("#{id}")];
    if info.metadata_only {
        parts.push("label-only".to_string());
    } else if info.transport != Transport::Local {
        parts.push(info.transport.as_str().to_string());
    }
    parts.extend(row.host.clone());
    parts.extend(cwd.clone());
    parts.extend(running.clone());
    row.id = format!("pane:{session}/{id}");
    row.label = if title.is_empty() { format!("pane #{id}") } else { title.clone() };
    row.detail = parts.join(" ");
    row.haystack = [
        row.label.as_str(),
        row.host.as_deref().unwrap_or(""),
        &row.tags.join(" "),
        row.tmux_session.as_deref().unwrap_or(""),
        row.tmux_window.as_deref().unwrap_or(""),
        cwd.as_deref().unwrap_or(""),
        running.as_deref().unwrap_or(""),
    ]
    .join(" ");
    row.data = RowData::Pane(PaneRow {
        pane: id,
        title,
        cwd_trusted: cwd_trusted(pane, &info),
        cwd,
        running_command: running,
    });
    row
}

/// The active workspace session. Its haystack also holds every pane title, host, tag and tmux
/// name so `:find session box` finds it through an SSH pane host. Only the active session is a
/// live row, so its position in the ordered list is always first (`order = u64::MAX`); that
/// avoids `ordered_sessions_with_active`, which lists the session directory (disk I/O).
fn session_row(wm: &WindowManager) -> FindRow {
    let name = session_name(wm);
    let mut row = FindRow::empty(FindKind::Session);
    row.source = ResultSource::live(&name);
    row.session = name.clone();
    row.order = u64::MAX;
    row.id = format!("session:{name}");
    row.label = name.clone();
    let pane_count = wm.panes.len();
    row.detail = format!("{pane_count} panes, active");
    let mut hay = vec![name.clone()];
    for (&id, pane) in wm.panes.iter().take(PANES_PER_SESSION_CAP) {
        let info = transport_of(wm, id);
        hay.push(pane_title(pane));
        hay.extend(info.host.as_deref().map(clean));
        hay.extend(info.tags.iter().map(|t| clean(t)));
        hay.extend(info.tmux_session.as_deref().map(clean));
        hay.extend(info.tmux_window.as_deref().map(clean));
    }
    row.haystack = hay.join(" ");
    let parent = wm.session_tree().node(&name).and_then(|n| n.parent.as_deref().map(clean));
    row.data = RowData::Session(SessionRow { name, active: true, parent, pane_count });
    row
}
