//! Disk half of the registry: saved workspace sessions and `history.log` (FND-R01/R02, plan 50.1-11).
//!
//! D-U3: every row carries a [`ResultSource`] (origin, session name, path). D-U2: every command
//! read from `history.log` or a saved pane history goes through [`classify`]; secret-looking lines
//! are dropped and only counted into the [`DiskCache`] masked counter.
//!
//! I/O is bounded and hostile-file safe (T-50.1-24/25): at most `SAVED_SESSIONS_CAP` entries, only
//! regular files (`symlink_metadata`, no symlink following), files above `SAVED_FILE_BYTES_CAP`
//! skipped, names must be plain `[A-Za-z0-9._-]` (no leading dot), malformed TOML skipped. The
//! result of each kind is cached in the [`DiskCache`] so a second call does no I/O.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::types::{
    CMD_CAP, DiskCache, FIND_HISTORY_READ_CAP, FindKind, FindRow, HISTORY_LOG_BYTES_CAP, HistoryRow, NAME_CAP,
    Origin, PANES_PER_SESSION_CAP, PaneRow, Recency, ResultSource, RowData, SAVED_FILE_BYTES_CAP,
    SAVED_SESSIONS_CAP, SessionRow, Transport,
};
use crate::pane::history_store::{
    HistoryRecord, HistoryStore, current_run_id, default_history_dir, parse_history_record,
    sanitize_session_name, session_history_path,
};
use crate::pane::redact::{Exclusion, classify};
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::layout_profile::sanitize_profile_name;
use crate::wm::session::{SavedPaneState, SavedWorkspaceSession};

/// Characters of a path shown in a row.
const PATH_CAP: usize = 256;

/// Disk candidates for Session or History; adds secret-looking lines to the cache's masked count.
pub fn candidates(wm: &WindowManager, kind: FindKind, cache: &mut DiskCache) -> Vec<FindRow> {
    if kind == FindKind::Agent {
        return Vec::new();
    }
    if cache.is_loaded(kind) {
        return cache.cached(kind).to_vec();
    }
    let mut masked = 0usize;
    let rows = match kind {
        FindKind::Session => session_rows(wm),
        FindKind::History => history_rows(wm, &mut masked),
        FindKind::Agent => Vec::new(),
    };
    cache.add_masked(masked);
    cache.store(kind, rows.clone());
    rows
}

// ---------------------------------------------------------------------------------------------
// Safe file access
// ---------------------------------------------------------------------------------------------

/// A plain, bounded session / directory name (no separators, NUL, leading dot or odd bytes).
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= NAME_CAP
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && crate::secure_fs::validate_plain_file_name(name).is_ok()
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

fn is_plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

fn shown_path(path: &Path) -> String {
    sanitize_display(&path.to_string_lossy(), PATH_CAP)
}

/// Reads a regular file of at most `SAVED_FILE_BYTES_CAP` bytes as text.
fn read_small_file(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > SAVED_FILE_BYTES_CAP {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let mut raw = String::new();
    file.take(SAVED_FILE_BYTES_CAP).read_to_string(&mut raw).ok()?;
    Some(raw)
}

fn load_saved_file(path: &Path) -> Option<SavedWorkspaceSession> {
    let raw = read_small_file(path)?;
    match toml::from_str::<SavedWorkspaceSession>(&raw) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::debug!(error = %e, "skipping malformed saved session");
            None
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Saved sessions
// ---------------------------------------------------------------------------------------------

struct Saved {
    name: String,
    /// `Some` only when `<session_dir>/<name>.toml` is a regular file.
    path: Option<PathBuf>,
    snap: SavedWorkspaceSession,
}

impl Saved {
    fn source(&self) -> ResultSource {
        ResultSource {
            origin: Origin::SavedSession,
            name: self.name.clone(),
            path: self.path.as_deref().map(shown_path),
        }
    }
}

fn is_meta_name(name: &str) -> bool {
    matches!(name, "layout_profile_meta" | "session_order")
}

/// Non-active saved sessions: in-memory snapshots first, then `<session_dir>/<name>.toml`.
fn saved_sessions(wm: &WindowManager) -> Vec<Saved> {
    let active = sanitize_profile_name(&wm.config.history_session);
    let dir = wm.resolve_session_dir().ok();
    let file_of = |name: &str| -> Option<PathBuf> {
        let p = dir.as_ref()?.join(format!("{name}.toml"));
        is_regular_file(&p).then_some(p)
    };
    let mut out: Vec<Saved> = Vec::new();
    for (name, snap) in &wm.layout_profiles_mem {
        if out.len() >= SAVED_SESSIONS_CAP {
            break;
        }
        if is_meta_name(name) || *name == active || !valid_name(name) {
            continue;
        }
        out.push(Saved { name: name.clone(), path: file_of(name), snap: snap.clone() });
    }
    let Some(dir) = dir.as_deref() else { return out };
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        // Bounded scan: a directory of thousands of files is never walked in full.
        for entry in entries.flatten().take(SAVED_SESSIONS_CAP * 4) {
            let path = entry.path();
            let (Some(stem), Some("toml")) =
                (path.file_stem().and_then(|s| s.to_str()), path.extension().and_then(|s| s.to_str()))
            else {
                continue;
            };
            if is_meta_name(stem) || stem == active || !valid_name(stem) || !is_regular_file(&path) {
                continue;
            }
            if !names.iter().any(|n| n == stem) && !out.iter().any(|s| s.name == stem) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    for name in names {
        if out.len() >= SAVED_SESSIONS_CAP {
            break;
        }
        let path = dir.join(format!("{name}.toml"));
        if let Some(snap) = load_saved_file(&path) {
            out.push(Saved { name, path: Some(path), snap });
        }
    }
    out
}

fn clean(raw: &str) -> String {
    sanitize_display(raw, NAME_CAP)
}

/// Splits a saved `remote_target` the way `remote::transport_of` does: `host/session` is
/// ssh+tmux (label only), anything else is ssh.
fn remote_parts(target: &str) -> (Transport, String, Option<String>) {
    match target.split_once('/') {
        Some((host, session)) => (Transport::SshTmux, clean(host), Some(clean(session))),
        None => (Transport::Ssh, clean(target), None),
    }
}

/// Running command text for a saved pane; a secret-looking command is never shown (D-U2).
fn saved_running_command(pane: &SavedPaneState) -> Option<String> {
    let cmd = pane.running_command.as_deref()?.trim();
    if classify(cmd).is_some() {
        return None;
    }
    let shown = sanitize_display(cmd, CMD_CAP);
    (!shown.is_empty()).then_some(shown)
}

fn saved_pane_row(saved: &Saved, pane: &SavedPaneState) -> FindRow {
    let name = clean(&saved.name);
    let mut row = FindRow::empty(FindKind::Session);
    row.source = saved.source();
    row.session = name.clone();
    // The pane id belongs to the saved snapshot, not to a live pane: never a jump target.
    row.pane = None;
    row.order = u64::MAX - pane.id as u64;
    let title = clean(&pane.title);
    let remote = pane.remote_target.as_deref().map(remote_parts);
    let running = saved_running_command(pane);
    let cwd = if remote.is_none() {
        Some(sanitize_display(&pane.cwd, PATH_CAP)).filter(|c| !c.is_empty())
    } else {
        None
    };
    if let Some((transport, host, tmux)) = &remote {
        row.transport = *transport;
        row.host = Some(host.clone());
        row.tmux_session = tmux.clone();
        row.metadata_only = *transport == Transport::SshTmux;
    }
    let mut parts = vec![format!("#{}", pane.id)];
    if row.metadata_only {
        parts.push("label-only".to_string());
    } else if row.transport != Transport::Local {
        parts.push(row.transport.as_str().to_string());
    }
    parts.extend(row.host.clone());
    parts.extend(cwd.clone());
    parts.extend(running.clone());
    row.id = format!("pane:{name}/{}", pane.id);
    row.label = if title.is_empty() { format!("pane #{}", pane.id) } else { title.clone() };
    row.detail = parts.join(" ");
    row.haystack = [
        row.label.as_str(),
        row.host.as_deref().unwrap_or(""),
        row.tmux_session.as_deref().unwrap_or(""),
        cwd.as_deref().unwrap_or(""),
        running.as_deref().unwrap_or(""),
    ]
    .join(" ");
    row.data = RowData::Pane(PaneRow {
        pane: pane.id,
        title,
        cwd_trusted: remote.is_none(),
        cwd,
        running_command: running,
    });
    row
}

fn saved_session_row(saved: &Saved, index: usize, pane_rows: &[FindRow]) -> FindRow {
    let name = clean(&saved.name);
    let mut row = FindRow::empty(FindKind::Session);
    row.source = saved.source();
    row.session = name.clone();
    row.order = u64::MAX - index as u64;
    row.id = format!("session:{name}");
    row.label = name.clone();
    let pane_count = saved.snap.panes.len();
    row.detail = format!("{pane_count} panes, saved");
    let mut hay = vec![name.clone()];
    for p in pane_rows {
        hay.push(p.label.clone());
        hay.extend(p.host.clone());
        hay.extend(p.tmux_session.clone());
    }
    row.haystack = hay.join(" ");
    row.data = RowData::Session(SessionRow { name, active: false, parent: None, pane_count });
    row
}

/// History-only sessions: `<history_dir>/sessions/<name>/history.log` regular files.
fn history_only_sessions(wm: &WindowManager) -> Vec<(String, PathBuf)> {
    let base = default_history_dir(wm.config.history_dir.as_deref());
    if base.as_os_str().is_empty() {
        return Vec::new();
    }
    let active = sanitize_session_name(&wm.config.history_session);
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(entries) = fs::read_dir(base.join("sessions")) {
        for entry in entries.flatten().take(SAVED_SESSIONS_CAP * 4) {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
            if name == active || !valid_name(&name) || !is_plain_dir(&entry.path()) {
                continue;
            }
            let log = session_history_path(&base, &name);
            if is_regular_file(&log) {
                found.push((name, log));
            }
        }
    }
    found.sort();
    found.truncate(SAVED_SESSIONS_CAP);
    found
}

fn session_rows(wm: &WindowManager) -> Vec<FindRow> {
    let mut rows: Vec<FindRow> = Vec::new();
    for (i, saved) in saved_sessions(wm).iter().enumerate() {
        let panes: Vec<FindRow> =
            saved.snap.panes.iter().take(PANES_PER_SESSION_CAP).map(|p| saved_pane_row(saved, p)).collect();
        rows.push(saved_session_row(saved, i, &panes));
        rows.extend(panes);
    }
    for (i, (name, log)) in history_only_sessions(wm).into_iter().enumerate() {
        let shown = clean(&name);
        let mut row = FindRow::empty(FindKind::Session);
        row.source = ResultSource { origin: Origin::HistoryLog, name: shown.clone(), path: Some(shown_path(&log)) };
        row.session = shown.clone();
        row.order = u64::MAX - i as u64;
        row.id = format!("session:{shown}");
        row.label = shown.clone();
        row.detail = "history log only".to_string();
        row.haystack = shown.clone();
        row.data = RowData::Session(SessionRow { name: shown, active: false, parent: None, pane_count: 0 });
        rows.push(row);
    }
    if let Ok(runtime_dir) = crate::secure_fs::private_runtime_dir() {
        for (i, d) in crate::daemon::registry::DaemonRegistry::list_active(&runtime_dir).into_iter().enumerate() {
            let shown = clean(&d.name);
            let mut row = FindRow::empty(FindKind::Session);
            row.source = ResultSource::daemon(shown.clone());
            row.session = shown.clone();
            row.order = u64::MAX - i as u64;
            row.id = format!("daemon:{shown}");
            row.label = format!("daemon:{shown}");
            row.detail = format!("daemon (pid {}, {} sess, {} panes)", d.pid, d.sessions_count, d.panes_count);
            row.haystack = format!("daemon:{shown} {} {}", d.pid, d.version);
            row.data = RowData::Session(SessionRow {
                name: shown,
                active: true,
                parent: None,
                pane_count: d.panes_count,
            });
            rows.push(row);
        }
    }
    rows
}

// ---------------------------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------------------------

/// Multiset of `(pane, command)` held by the live ring, used to skip log lines of this run that
/// the ring already shows.
struct LiveSet(HashMap<(usize, String), usize>);

impl LiveSet {
    fn from_rows(rows: &[FindRow]) -> Self {
        let mut map: HashMap<(usize, String), usize> = HashMap::new();
        for r in rows {
            if let (Some(p), RowData::History(h)) = (r.pane, &r.data) {
                *map.entry((p, h.command.clone())).or_default() += 1;
            }
        }
        Self(map)
    }

    /// Consumes one matching live entry; `true` when the log line duplicates it.
    fn take(&mut self, pane: usize, command: &str) -> bool {
        match self.0.get_mut(&(pane, command.to_string())) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }
}

fn transport_from_log(raw: Option<&str>) -> Transport {
    match raw {
        Some("ssh") => Transport::Ssh,
        Some("tmux") => Transport::Tmux,
        Some("ssh-tmux") => Transport::SshTmux,
        _ => Transport::Local,
    }
}

/// Newest records of the active session's log: the annotated loader (exit codes) when the file
/// fits the tail window, else the newest `HISTORY_LOG_BYTES_CAP` bytes without exit codes.
fn read_log_records(log: &Path, base: &Path, session: &str) -> Vec<HistoryRecord> {
    let Ok(meta) = fs::symlink_metadata(log) else { return Vec::new() };
    if !meta.file_type().is_file() {
        return Vec::new();
    }
    if meta.len() <= HISTORY_LOG_BYTES_CAP {
        let store = HistoryStore::new(0, session, "find", base.to_path_buf(), false, 1);
        return store.load_entries_annotated().unwrap_or_default();
    }
    use std::io::{Seek, SeekFrom};
    let Ok(mut file) = fs::File::open(log) else { return Vec::new() };
    if file.seek(SeekFrom::Start(meta.len() - HISTORY_LOG_BYTES_CAP)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file.take(HISTORY_LOG_BYTES_CAP).read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    // The window starts mid-line: drop the first partial line.
    let body = text.split_once('\n').map_or("", |(_, rest)| rest);
    body.lines().filter_map(parse_history_record).collect()
}

fn log_row(rec: &HistoryRecord, order: usize, name: &str, path: &str, current: bool) -> FindRow {
    let cmd = &rec.entry.command;
    let label = sanitize_display(cmd, CMD_CAP);
    let host = rec.host.as_deref().map(clean).filter(|h| !h.is_empty());
    let cwd = rec.cwd.as_deref().map(|c| sanitize_display(c, PATH_CAP)).filter(|c| !c.is_empty());
    let mut row = FindRow::empty(FindKind::History);
    row.source = ResultSource { origin: Origin::HistoryLog, name: name.to_string(), path: Some(path.to_string()) };
    row.session = name.to_string();
    row.transport = transport_from_log(rec.transport.as_deref());
    row.host = host.clone();
    // A pane id only means something for lines of this run.
    row.pane = rec.entry.pane_id.filter(|_| current);
    row.order = order as u64;
    row.id = format!("history:{name}/{order}");
    let mut parts = vec![host.clone().unwrap_or_else(|| "local".to_string())];
    parts.extend(cwd.clone());
    if let Some(code) = rec.exit_code {
        parts.push(format!("exit {code}"));
    }
    row.detail = parts.join(" ");
    row.haystack = [label.as_str(), host.as_deref().unwrap_or(""), cwd.as_deref().unwrap_or("")].join(" ");
    row.label = label.clone();
    row.data = RowData::History(HistoryRow {
        seq: order as u64,
        pane: row.pane,
        cwd,
        command: label,
        exit_code: rec.exit_code,
        duration_ms: None,
        at: Recency::Logged(sanitize_display(&rec.entry.timestamp, NAME_CAP)),
    });
    row
}

/// History rows from the active session's `history.log` plus saved pane histories of the
/// non-active saved sessions. Secret lines are only counted into `masked`.
fn history_rows(wm: &WindowManager, masked: &mut usize) -> Vec<FindRow> {
    let mut rows: Vec<FindRow> = Vec::new();
    let base = default_history_dir(wm.config.history_dir.as_deref());
    if !base.as_os_str().is_empty() {
        let session_dir_name = sanitize_session_name(&wm.config.history_session);
        let log = session_history_path(&base, &session_dir_name);
        let shown = shown_path(&log);
        let name = clean(&wm.config.history_session);
        let records = read_log_records(&log, &base, &session_dir_name);
        let start = records.len().saturating_sub(FIND_HISTORY_READ_CAP);
        let run = current_run_id();
        let mut live = LiveSet::from_rows(&super::capture::candidates(wm));
        for (i, rec) in records.iter().enumerate().skip(start) {
            match classify(&rec.entry.command) {
                Some(Exclusion::Secret) => *masked += 1,
                Some(_) => {}
                None => {
                    let current = rec.entry.run_id == run;
                    let dup = current
                        && rec.entry.pane_id.is_some_and(|p| {
                            live.take(p, &sanitize_display(&rec.entry.command, CMD_CAP))
                        });
                    if !dup {
                        rows.push(log_row(rec, i, &name, &shown, current));
                    }
                }
            }
        }
    }
    for saved in saved_sessions(wm) {
        for pane in saved.snap.panes.iter().take(PANES_PER_SESSION_CAP) {
            if rows.len() >= FIND_HISTORY_READ_CAP.saturating_mul(2) {
                return rows;
            }
            let start = pane.history.len().saturating_sub(FIND_HISTORY_READ_CAP);
            for (i, line) in pane.history.iter().enumerate().skip(start) {
                match classify(line) {
                    Some(Exclusion::Secret) => *masked += 1,
                    Some(_) => {}
                    None => rows.push(saved_history_row(&saved, pane.id, i, line)),
                }
            }
        }
    }
    rows
}

fn saved_history_row(saved: &Saved, pane: usize, index: usize, command: &str) -> FindRow {
    let name = clean(&saved.name);
    let label = sanitize_display(command, CMD_CAP);
    let mut row = FindRow::empty(FindKind::History);
    row.source = saved.source();
    row.session = name.clone();
    row.order = index as u64;
    row.id = format!("history:{name}/{pane}.{index}");
    row.detail = format!("saved pane #{pane}");
    row.haystack = label.clone();
    row.label = label.clone();
    row.data = RowData::History(HistoryRow {
        seq: index as u64,
        pane: None,
        cwd: None,
        command: label,
        exit_code: None,
        duration_ms: None,
        at: Recency::Saved,
    });
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_name_rejects_unsafe_names() {
        assert!(valid_name("work"));
        assert!(valid_name("my-work_2.v1"));
        for bad in ["", ".hidden", "..", "a/b", "a\0b", "..%2fx", "a b", "é"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(!valid_name(&"x".repeat(NAME_CAP + 1)));
    }

    #[test]
    fn live_set_matches_as_a_multiset() {
        let mut row = FindRow::empty(FindKind::History);
        row.pane = Some(2);
        if let RowData::History(h) = &mut row.data {
            h.command = "ls".to_string();
        }
        let mut live = LiveSet::from_rows(&[row.clone(), row]);
        assert!(live.take(2, "ls"));
        assert!(live.take(2, "ls"));
        assert!(!live.take(2, "ls"), "only two live copies exist");
        assert!(!live.take(3, "ls"));
        assert!(!live.take(2, "pwd"));
    }
}
