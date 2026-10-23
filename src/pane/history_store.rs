use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
static CURRENT_RUN_ID: OnceLock<String> = OnceLock::new();

/// Default named history session (`history_session = "default"`).
pub const DEFAULT_HISTORY_SESSION: &str = "default";
/// Default maximum entries retained in a session history file before compaction.
pub const DEFAULT_HISTORY_MAX_ENTRIES: usize = 100_000;
/// Default number of most recent commands restored into a new pane's prompt history.
pub const DEFAULT_HISTORY_LOAD_LIMIT: usize = 10_000;

/// Generates a unique run identifier combining unix epoch seconds, process ID, and an atomic counter.
pub fn generate_session_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pid = std::process::id();
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{secs}-{pid}-{seq}")
}

/// Returns the process-wide run identifier shared by every pane of this bilterm run (one history segment).
pub fn current_run_id() -> String {
    CURRENT_RUN_ID.get_or_init(generate_session_id).clone()
}

/// Sanitizes a history session name for filesystem use (`[A-Za-z0-9._-]`, max 64 chars, no leading dots).
pub fn sanitize_session_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let cleaned = cleaned.trim_start_matches('.');
    if cleaned.is_empty() {
        DEFAULT_HISTORY_SESSION.to_string()
    } else {
        cleaned.to_string()
    }
}

/// Escapes a command for single-line storage (backslash, newline and CR become `\\`, `\n`, `\r`).
pub fn escape_history_command(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    for c in command.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Reverses `escape_history_command`; unknown escapes are kept verbatim.
pub fn unescape_history_command(stored: &str) -> String {
    let mut out = String::with_capacity(stored.len());
    let mut chars = stored.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// One persisted command record (`[ts | session=<name> | run=<runid> | pane=<id>] <cmd>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub session: String,
    pub run_id: String,
    pub pane_id: Option<usize>,
    pub command: String,
}

/// Parses one history log line; returns `None` for malformed lines.
pub fn parse_history_line(line: &str) -> Option<HistoryEntry> {
    let rest = line.strip_prefix('[')?;
    let (header, cmd) = rest.split_once("] ")?;
    let mut fields = header.split(" | ");
    let timestamp = fields.next()?.to_string();
    let mut entry = HistoryEntry {
        timestamp,
        session: String::new(),
        run_id: String::new(),
        pane_id: None,
        command: unescape_history_command(cmd),
    };
    for field in fields {
        if let Some(v) = field.strip_prefix("session=") {
            entry.session = v.to_string();
        } else if let Some(v) = field.strip_prefix("run=") {
            entry.run_id = v.to_string();
        } else if let Some(v) = field.strip_prefix("pane=") {
            entry.pane_id = v.parse().ok();
        }
    }
    Some(entry)
}

/// A group of commands recorded by one bilterm run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySegment {
    pub run_id: String,
    pub started_at: String,
    pub count: usize,
}

/// History persistence settings pushed from `BiltermConfig` to every pane's `HistoryStore`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySettings {
    pub enabled: bool,
    pub max_files: usize,
    pub max_entries: usize,
    pub load_limit: usize,
    pub dir: Option<String>,
    pub session: String,
}

fn create_private_dir_all(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

fn private_open_options() -> OpenOptions {
    #[allow(unused_mut)]
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts
}

/// Returns `<base_dir>/sessions/<name>/history.log`.
pub fn session_history_path(base_dir: &Path, session_name: &str) -> PathBuf {
    base_dir
        .join("sessions")
        .join(session_name)
        .join("history.log")
}

/// Returns the current UTC/epoch timestamp formatted as `YYYY-MM-DDTHH:MM:SSZ` (computed deterministically without extra crate deps).
pub fn format_iso_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_timestamp_iso(secs)
}

/// Converts Unix epoch seconds into an ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn format_unix_timestamp_iso(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let rem = epoch_secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    // Howard Hinnant civil-from-days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Resolves the default directory for persisted Bilterm session history files.
pub fn default_history_dir(custom_dir: Option<&str>) -> PathBuf {
    if let Some(dir) = custom_dir {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("bilterm").join("history");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("bilterm")
            .join("history");
    }
    std::env::temp_dir().join("bilterm-history")
}

/// Persistent, session-segmented command history manager for a `CompositePane`.
///
/// All panes of all runs sharing a named session append to one file,
/// `<base_dir>/sessions/<name>/history.log`. Each record carries:
/// - ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`)
/// - Session name (`session=<name>`) and run id (`run=<epoch>-<pid>-<seq>`, one segment per run)
/// - Terminal/Pane ID (`pane=<id>`)
///
/// The file is compacted to the newest `max_entries` records, and at most `max_files`
/// session directories are retained.
#[derive(Clone, Debug)]
pub struct HistoryStore {
    pub enabled: bool,
    pub session_name: String,
    pub run_id: String,
    pub pane_id: usize,
    pub base_dir: PathBuf,
    pub session_file_path: PathBuf,
    /// Maximum number of retained session directories.
    pub max_files: usize,
    /// Maximum entries kept in the session file (compacted once 5% past this).
    pub max_entries: usize,
    /// Number of most recent commands restored by `load_recent_commands`.
    pub load_limit: usize,
    /// Headless/test stores never resolve the real user history directory.
    pub headless: bool,
    pub last_exported_temp_path: Option<PathBuf>,
    cached_entry_count: Option<usize>,
}

impl HistoryStore {
    /// Creates a `HistoryStore` for `pane_id` recording into session `session_name` under `base_dir`.
    pub fn new(
        pane_id: usize,
        session_name: &str,
        run_id: impl Into<String>,
        base_dir: PathBuf,
        enabled: bool,
        max_files: usize,
    ) -> Self {
        let session_name = sanitize_session_name(session_name);
        let session_file_path = session_history_path(&base_dir, &session_name);
        Self {
            enabled,
            session_name,
            run_id: run_id.into(),
            pane_id,
            base_dir,
            session_file_path,
            max_files: max_files.max(1),
            max_entries: DEFAULT_HISTORY_MAX_ENTRIES,
            load_limit: DEFAULT_HISTORY_LOAD_LIMIT,
            headless: false,
            last_exported_temp_path: None,
            cached_entry_count: None,
        }
    }

    /// Creates a headless in-temp `HistoryStore` (disabled by default in unit tests unless enabled).
    pub fn new_headless(pane_id: usize) -> Self {
        let base_dir = std::env::temp_dir().join("bilterm-test-history");
        let mut store = Self::new(
            pane_id,
            DEFAULT_HISTORY_SESSION,
            generate_session_id(),
            base_dir,
            false,
            25,
        );
        store.headless = true;
        store
    }

    /// Creates a live `HistoryStore` enabled by default and backed by `default_history_dir`.
    pub fn new_live(pane_id: usize) -> Self {
        Self::new(
            pane_id,
            DEFAULT_HISTORY_SESSION,
            current_run_id(),
            default_history_dir(None),
            true,
            25,
        )
    }

    /// Applies config-driven settings. Returns `true` when the session file location changed
    /// (callers should then reload prompt history).
    pub fn apply_settings(&mut self, settings: &HistorySettings) -> bool {
        self.enabled = settings.enabled;
        self.max_files = settings.max_files.max(1);
        self.max_entries = settings.max_entries.max(1);
        self.load_limit = settings.load_limit;
        let custom_dir = settings.dir.as_deref().filter(|d| !d.trim().is_empty());
        if let Some(dir) = custom_dir {
            self.base_dir = PathBuf::from(dir);
        } else if !self.headless {
            self.base_dir = default_history_dir(None);
        }
        self.session_name = sanitize_session_name(&settings.session);
        let new_path = session_history_path(&self.base_dir, &self.session_name);
        let changed = new_path != self.session_file_path;
        if changed {
            self.session_file_path = new_path;
            self.cached_entry_count = None;
        }
        changed
    }

    /// Appends a command entry to the session file, compacting it past `max_entries` and
    /// pruning least-recently-modified session directories beyond `max_files`.
    pub fn record_command(&mut self, command: &str) -> std::io::Result<Option<PathBuf>> {
        if !self.enabled || command.trim().is_empty() {
            return Ok(None);
        }
        if let Some(parent) = self.session_file_path.parent() {
            create_private_dir_all(parent)?;
        }
        let record_line = format!(
            "[{} | session={} | run={} | pane={}] {}\n",
            format_iso_utc_now(),
            self.session_name,
            self.run_id,
            self.pane_id,
            escape_history_command(command)
        );

        let mut file = private_open_options()
            .create(true)
            .append(true)
            .open(&self.session_file_path)?;
        file.write_all(record_line.as_bytes())?;
        file.flush()?;
        drop(file);

        let count = match self.cached_entry_count {
            Some(n) => n + 1,
            None => fs::read_to_string(&self.session_file_path)?.lines().count(),
        };
        self.cached_entry_count = Some(count);
        // 5% slack avoids rewriting the whole file on every command once at the cap.
        if count > self.max_entries + self.max_entries / 20 {
            self.compact()?;
        }

        let _ = self.prune_excess_files();
        Ok(Some(self.session_file_path.clone()))
    }

    /// Rewrites the session file keeping only the newest `max_entries` lines (temp file + atomic rename).
    pub fn compact(&mut self) -> std::io::Result<usize> {
        let raw = match fs::read_to_string(&self.session_file_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let lines: Vec<&str> = raw.lines().collect();
        if lines.len() <= self.max_entries {
            self.cached_entry_count = Some(lines.len());
            return Ok(0);
        }
        let dropped = lines.len() - self.max_entries;
        let mut out = lines[dropped..].join("\n");
        out.push('\n');

        let tmp_path = self.session_file_path.with_file_name(format!(
            "history.log.tmp-{}-{}",
            std::process::id(),
            SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write_res = (|| {
            let mut tmp = private_open_options()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            tmp.write_all(out.as_bytes())?;
            tmp.sync_all()?;
            fs::rename(&tmp_path, &self.session_file_path)
        })();
        if write_res.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        write_res?;
        self.cached_entry_count = Some(self.max_entries);
        Ok(dropped)
    }

    /// Reads every parseable entry of the session file (oldest first); a missing file yields no entries.
    pub fn load_entries(&self) -> std::io::Result<Vec<HistoryEntry>> {
        match fs::read_to_string(&self.session_file_path) {
            Ok(raw) => Ok(raw.lines().filter_map(parse_history_line).collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Groups the session's entries by run id (one segment per bilterm run), ordered by first appearance.
    pub fn segments(&self) -> Vec<HistorySegment> {
        let mut segs: Vec<HistorySegment> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for entry in self.load_entries().unwrap_or_default() {
            if let Some(&i) = index.get(&entry.run_id) {
                segs[i].count += 1;
            } else {
                index.insert(entry.run_id.clone(), segs.len());
                segs.push(HistorySegment {
                    run_id: entry.run_id,
                    started_at: entry.timestamp,
                    count: 1,
                });
            }
        }
        segs
    }

    /// Returns the newest `load_limit` commands (oldest -> newest) with consecutive duplicates collapsed.
    /// Returns nothing while persistence is disabled.
    pub fn load_recent_commands(&self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        let mut cmds: Vec<String> = Vec::new();
        for entry in self.load_entries().unwrap_or_default() {
            if cmds.last() != Some(&entry.command) {
                cmds.push(entry.command);
            }
        }
        let skip = cmds.len().saturating_sub(self.load_limit);
        cmds.split_off(skip)
    }

    /// Exports the given command history slice to a temporary file (or `custom_path`), returning the written `PathBuf`.
    pub fn export_to_temp_file(
        &mut self,
        commands: &[String],
        custom_path: Option<&Path>,
    ) -> std::io::Result<PathBuf> {
        let target_path = match custom_path {
            Some(p) => p.to_path_buf(),
            None => {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                std::env::temp_dir().join(format!(
                    "bilterm-commands-pane{}-{}-{}-{ts}.sh",
                    self.pane_id, self.session_name, self.run_id
                ))
            }
        };

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out = String::new();
        out.push_str(&format!(
            "# Bilterm Exported Command History\n# Session: {}\n# Run: {}\n# Terminal Pane: #{}\n# Exported At: {}\n\n",
            self.session_name,
            self.run_id,
            self.pane_id,
            format_iso_utc_now()
        ));
        for (idx, cmd) in commands.iter().enumerate() {
            out.push_str(&format!("# [{:03}]\n{}\n", idx + 1, cmd));
        }

        fs::write(&target_path, out)?;
        self.last_exported_temp_path = Some(target_path.clone());
        Ok(target_path)
    }

    /// Removes the least-recently-modified session directories under `<base_dir>/sessions`
    /// while more than `max_files` exist. The active session is never removed.
    pub fn prune_excess_files(&self) -> std::io::Result<usize> {
        let Ok(entries) = fs::read_dir(self.base_dir.join("sessions")) else {
            return Ok(0);
        };
        let active_dir = self.session_file_path.parent().map(Path::to_path_buf);
        let mut dirs: Vec<(PathBuf, SystemTime)> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| {
                let path = e.path();
                let mtime = fs::metadata(path.join("history.log"))
                    .or_else(|_| e.metadata())
                    .and_then(|m| m.modified())
                    .unwrap_or(UNIX_EPOCH);
                (path, mtime)
            })
            .collect();

        if dirs.len() <= self.max_files {
            return Ok(0);
        }

        // Oldest first by modification time, then name
        dirs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let mut excess = dirs.len() - self.max_files;
        let mut removed = 0;
        for (path, _) in dirs {
            if excess == 0 {
                break;
            }
            if Some(&path) == active_dir.as_ref() {
                continue;
            }
            if fs::remove_dir_all(&path).is_ok() {
                removed += 1;
            }
            excess -= 1;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(dir: &Path, session: &str) -> HistorySettings {
        HistorySettings {
            enabled: true,
            max_files: 25,
            max_entries: 100,
            load_limit: 1000,
            dir: Some(dir.to_string_lossy().into_owned()),
            session: session.to_string(),
        }
    }

    fn store(dir: &Path, session: &str, run: &str, pane: usize) -> HistoryStore {
        let mut s = HistoryStore::new_headless(pane);
        s.run_id = run.to_string();
        s.apply_settings(&settings(dir, session));
        s
    }

    #[test]
    fn entries_survive_a_new_store_instance_for_the_same_session() {
        let tmp = tempfile::tempdir().unwrap();
        let mut first = store(tmp.path(), "work", "run-a", 1);
        let written = first.record_command("tail -f /dev/null").unwrap().unwrap();
        assert_eq!(written, tmp.path().join("sessions/work/history.log"));
        let text = fs::read_to_string(&written).unwrap();
        assert!(text.contains("session=work | run=run-a | pane=1] tail -f /dev/null"));

        let second = store(tmp.path(), "work", "run-b", 1);
        let entries = second.load_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "tail -f /dev/null");
        assert_eq!(entries[0].run_id, "run-a");
        assert_eq!(entries[0].session, "work");
        assert_eq!(
            second.load_recent_commands(),
            vec!["tail -f /dev/null".to_string()]
        );
    }

    #[cfg(unix)]
    #[test]
    fn session_dir_and_file_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let mut s = store(tmp.path(), "default", "r", 1);
        let path = s.record_command("ls").unwrap().unwrap();
        let fmode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let dmode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(fmode, 0o600);
        assert_eq!(dmode, 0o700);
    }

    #[test]
    fn different_session_names_are_isolated_and_sanitized() {
        let tmp = tempfile::tempdir().unwrap();
        let mut a = store(tmp.path(), "alpha", "r1", 1);
        let mut b = store(tmp.path(), "../be ta/", "r1", 2);
        a.record_command("echo alpha").unwrap();
        b.record_command("echo beta").unwrap();
        assert_eq!(b.session_name, "_be_ta_");
        assert!(b.session_file_path.starts_with(tmp.path().join("sessions")));
        assert_eq!(a.load_recent_commands(), vec!["echo alpha".to_string()]);
        assert_eq!(b.load_recent_commands(), vec!["echo beta".to_string()]);
        assert_eq!(sanitize_session_name("   "), "default");
        assert_eq!(sanitize_session_name(".."), "default");
    }

    #[test]
    fn segments_group_entries_by_run() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r1 = store(tmp.path(), "s", "run-1", 1);
        r1.record_command("one").unwrap();
        r1.record_command("two").unwrap();
        let mut r2 = store(tmp.path(), "s", "run-2", 3);
        r2.record_command("three").unwrap();
        let segs = r2.segments();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].run_id, "run-1");
        assert_eq!(segs[0].count, 2);
        assert!(segs[0].started_at.ends_with('Z'));
        assert_eq!(segs[1].run_id, "run-2");
        assert_eq!(segs[1].count, 1);
    }

    #[test]
    fn compaction_keeps_newest_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = store(tmp.path(), "s", "r", 1);
        s.max_entries = 3;
        for i in 0..7 {
            s.record_command(&format!("cmd {i}")).unwrap();
        }
        let cmds: Vec<String> = s
            .load_entries()
            .unwrap()
            .into_iter()
            .map(|e| e.command)
            .collect();
        assert_eq!(cmds, vec!["cmd 4", "cmd 5", "cmd 6"]);
        let leftovers: Vec<_> = fs::read_dir(s.session_file_path.parent().unwrap())
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(leftovers.len(), 1, "temp file must be renamed away");
    }

    #[test]
    fn recent_commands_unescape_newlines_collapse_duplicates_and_respect_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = store(tmp.path(), "s", "r", 1);
        for c in [
            "ls",
            "ls",
            "for i in 1 2\ndo echo $i\ndone",
            "printf 'a\\nb'",
            "ls",
            "ls",
        ] {
            s.record_command(c).unwrap();
        }
        let raw = fs::read_to_string(&s.session_file_path).unwrap();
        assert_eq!(raw.lines().count(), 6);
        assert_eq!(
            s.load_recent_commands(),
            vec![
                "ls",
                "for i in 1 2\ndo echo $i\ndone",
                "printf 'a\\nb'",
                "ls"
            ]
        );
        s.load_limit = 2;
        assert_eq!(s.load_recent_commands(), vec!["printf 'a\\nb'", "ls"]);
    }

    #[test]
    fn prunes_least_recently_modified_session_dirs_but_never_active() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["old1", "old2"] {
            let mut s = store(tmp.path(), name, "r", 1);
            s.record_command("x").unwrap();
        }
        let mut active = store(tmp.path(), "active", "r", 1);
        active.max_files = 2;
        active.record_command("y").unwrap();
        let mut names: Vec<String> = fs::read_dir(tmp.path().join("sessions"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"active".to_string()));
        let legacy: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("bilterm-history-")
            })
            .collect();
        assert!(legacy.is_empty());
    }

    #[test]
    fn headless_store_never_resolves_user_dir_and_exports_temp_file() {
        let mut s = HistoryStore::new_headless(4);
        assert!(s.base_dir.starts_with(std::env::temp_dir()));
        let changed = s.apply_settings(&HistorySettings {
            dir: None,
            ..settings(Path::new("/unused"), "other")
        });
        assert!(changed);
        assert!(s.base_dir.starts_with(std::env::temp_dir()));

        let exported = s
            .export_to_temp_file(&["ls -la".to_string(), "cargo test".to_string()], None)
            .unwrap();
        let exp_text = fs::read_to_string(&exported).unwrap();
        assert!(exp_text.contains("# Session: other"));
        assert!(exp_text.contains("cargo test"));
        let _ = fs::remove_file(&exported);
    }
}
