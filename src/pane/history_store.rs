use std::collections::HashMap;
use std::fs;
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

/// Optional per-command context written as extra header fields (`host=`, `transport=`, `cwd=`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryMeta {
    pub host: Option<String>,
    pub transport: Option<String>,
    pub cwd: Option<String>,
}

/// A history entry plus its optional meta fields and joined exit code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRecord {
    pub entry: HistoryEntry,
    pub host: Option<String>,
    pub transport: Option<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
}

/// Prefix of an exit annotation line (never starts with `[`, so legacy readers skip it).
const EXIT_ANNOTATION_PREFIX: &str = "~exit";

fn is_safe_field_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'/' | b'@' | b':' | b'+' | b'-')
}

/// Percent-encodes every byte outside `[A-Za-z0-9._~/@:+-]` so a value can never contain
/// a space, `|`, `]`, `%` or a newline.
fn percent_encode_field(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &b in value.as_bytes() {
        if is_safe_field_byte(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Reverses `percent_encode_field`; malformed escapes are kept verbatim.
fn percent_decode_field(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
            && let Some(v) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// FNV-1a 32-bit hash of `text`'s bytes as 8 lowercase hex digits.
fn fnv1a_32_hex(text: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for &b in text.as_bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// Parses a command line including the optional meta header fields; `None` for non-command lines.
pub fn parse_history_record(line: &str) -> Option<HistoryRecord> {
    let entry = parse_history_line(line)?;
    let (header, _) = line.strip_prefix('[')?.split_once("] ")?;
    let mut rec = HistoryRecord {
        entry,
        host: None,
        transport: None,
        cwd: None,
        exit_code: None,
    };
    for field in header.split(" | ").skip(1) {
        if let Some(v) = field.strip_prefix("host=") {
            rec.host = Some(percent_decode_field(v));
        } else if let Some(v) = field.strip_prefix("transport=") {
            rec.transport = Some(percent_decode_field(v));
        } else if let Some(v) = field.strip_prefix("cwd=") {
            rec.cwd = Some(percent_decode_field(v));
        }
    }
    Some(rec)
}

/// One parsed `~exit | session=S | run=R | pane=N | h=<hex> | exit=<n>` annotation.
struct ExitAnnotation {
    run_id: String,
    pane_id: Option<usize>,
    hash: String,
    exit_code: i32,
}

fn parse_annotation(line: &str) -> Option<ExitAnnotation> {
    let rest = line.strip_prefix(EXIT_ANNOTATION_PREFIX)?;
    let mut ann = ExitAnnotation {
        run_id: String::new(),
        pane_id: None,
        hash: String::new(),
        exit_code: 0,
    };
    let mut have_exit = false;
    for field in rest.split(" | ") {
        if let Some(v) = field.strip_prefix("run=") {
            ann.run_id = v.to_string();
        } else if let Some(v) = field.strip_prefix("pane=") {
            ann.pane_id = v.parse().ok();
        } else if let Some(v) = field.strip_prefix("h=") {
            ann.hash = v.to_string();
        } else if let Some(v) = field.strip_prefix("exit=") {
            ann.exit_code = v.trim().parse().ok()?;
            have_exit = true;
        }
    }
    (have_exit && !ann.hash.is_empty()).then_some(ann)
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
    pub clipboard_persist: bool,
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

/// Returns `<base_dir>/sessions/<name>/history.log`.
pub fn session_history_path(base_dir: &Path, session_name: &str) -> PathBuf {
    base_dir
        .join("sessions")
        .join(session_name)
        .join("history.log")
}

/// Returns `<base_dir>/sessions/<name>/clipboard.log`.
pub fn session_clipboard_path(base_dir: &Path, session_name: &str) -> PathBuf {
    base_dir
        .join("sessions")
        .join(session_name)
        .join("clipboard.log")
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
    if let Some(dir) = custom_dir
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
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
    match crate::secure_fs::private_runtime_dir()
        .and_then(|d| crate::secure_fs::private_dir_in(&d, "history"))
    {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!(error = %e, "private history dir unavailable; history persistence disabled");
            PathBuf::new()
        }
    }
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
    pub clipboard_persist: bool,
    pub session_name: String,
    pub run_id: String,
    pub pane_id: usize,
    pub base_dir: PathBuf,
    pub session_file_path: PathBuf,
    pub clipboard_file_path: PathBuf,
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

fn write_export_exclusive(path: &Path, content: &str) -> std::io::Result<()> {
    let mut f = crate::secure_fs::create_private_file(path)?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    Ok(())
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
        let (session_file_path, clipboard_file_path) = if base_dir.as_os_str().is_empty() {
            (PathBuf::new(), PathBuf::new())
        } else {
            (
                session_history_path(&base_dir, &session_name),
                session_clipboard_path(&base_dir, &session_name),
            )
        };
        Self {
            enabled,
            clipboard_persist: enabled,
            session_name,
            run_id: run_id.into(),
            pane_id,
            base_dir,
            session_file_path,
            clipboard_file_path,
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
        self.max_files = settings.max_files.max(1);
        self.max_entries = settings.max_entries.max(1);
        self.load_limit = settings.load_limit;
        let custom_dir = settings.dir.as_deref().filter(|d| !d.trim().is_empty());
        if let Some(dir) = custom_dir {
            self.base_dir = PathBuf::from(dir);
            self.enabled = settings.enabled;
            self.clipboard_persist = settings.clipboard_persist;
        } else if !self.headless {
            self.base_dir = default_history_dir(None);
            self.enabled = settings.enabled;
            self.clipboard_persist = settings.clipboard_persist;
        } else {
            self.enabled = false;
            self.clipboard_persist = settings.clipboard_persist && settings.enabled;
        }
        self.session_name = sanitize_session_name(&settings.session);
        let (new_path, new_clip_path) = if self.base_dir.as_os_str().is_empty() {
            (PathBuf::new(), PathBuf::new())
        } else {
            (
                session_history_path(&self.base_dir, &self.session_name),
                session_clipboard_path(&self.base_dir, &self.session_name),
            )
        };
        let changed = new_path != self.session_file_path || new_clip_path != self.clipboard_file_path;
        if changed {
            self.session_file_path = new_path;
            self.clipboard_file_path = new_clip_path;
            self.cached_entry_count = None;
        }
        changed
    }

    /// Appends a command entry to the session file, compacting it past `max_entries` and
    /// pruning least-recently-modified session directories beyond `max_files`.
    pub fn record_command(&mut self, command: &str) -> std::io::Result<Option<PathBuf>> {
        self.record_command_meta(command, &HistoryMeta::default())
    }

    /// Like `record_command`, appending `host=`/`transport=`/`cwd=` header fields (percent-encoded)
    /// for each `Some` non-empty field of `meta`. With a default meta the line is byte-identical
    /// to `record_command`'s historical output.
    pub fn record_command_meta(
        &mut self,
        command: &str,
        meta: &HistoryMeta,
    ) -> std::io::Result<Option<PathBuf>> {
        if !self.enabled || self.base_dir.as_os_str().is_empty() || command.trim().is_empty() {
            return Ok(None);
        }
        let mut extra = String::new();
        for (key, value) in [
            ("host", &meta.host),
            ("transport", &meta.transport),
            ("cwd", &meta.cwd),
        ] {
            if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
                extra.push_str(&format!(" | {key}={}", percent_encode_field(v)));
            }
        }
        let record_line = format!(
            "[{} | session={} | run={} | pane={}{}] {}\n",
            format_iso_utc_now(),
            self.session_name,
            self.run_id,
            self.pane_id,
            extra,
            escape_history_command(command)
        );
        self.append_line(&record_line)?;
        let _ = self.prune_excess_files();
        Ok(Some(self.session_file_path.clone()))
    }

    /// Appends an exit annotation line for `command` (only its FNV-1a hash is stored, never the
    /// text). Readers join it to the most recent matching entry of the same run and pane.
    /// Disabled or path-less stores return `Ok(())`.
    pub fn record_exit(&mut self, command: &str, exit_code: i32) -> std::io::Result<()> {
        if !self.enabled || self.base_dir.as_os_str().is_empty() || command.trim().is_empty() {
            return Ok(());
        }
        let line = format!(
            "{EXIT_ANNOTATION_PREFIX} | session={} | run={} | pane={} | h={} | exit={}\n",
            self.session_name,
            self.run_id,
            self.pane_id,
            fnv1a_32_hex(command),
            exit_code
        );
        self.append_line(&line)
    }

    /// Appends one already-formatted line (0600 file, 0700 dir), tracking the line count and
    /// compacting past the 5% slack above `max_entries`.
    fn append_line(&mut self, line: &str) -> std::io::Result<()> {
        if let Some(parent) = self.session_file_path.parent() {
            create_private_dir_all(parent)?;
        }
        let mut file = crate::secure_fs::private_open_options()
            .create(true)
            .append(true)
            .open(&self.session_file_path)?;
        file.write_all(line.as_bytes())?;
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
        Ok(())
    }

    /// Rewrites the session file keeping only the newest `max_entries` lines (temp file + atomic rename).
    pub fn compact(&mut self) -> std::io::Result<usize> {
        if !self.enabled || self.base_dir.as_os_str().is_empty() {
            return Ok(0);
        }
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
            let mut tmp = crate::secure_fs::private_open_options()
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
        if self.base_dir.as_os_str().is_empty() {
            return Ok(Vec::new());
        }
        match fs::read_to_string(&self.session_file_path) {
            Ok(raw) => Ok(raw.lines().filter_map(parse_history_line).collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Reads every command entry (oldest first) with its meta fields and exit code joined from
    /// annotation lines in one forward pass. An annotation attaches to the most recent preceding
    /// entry with the same `(run, pane)` and equal command hash that has no exit yet.
    pub fn load_entries_annotated(&self) -> std::io::Result<Vec<HistoryRecord>> {
        if self.base_dir.as_os_str().is_empty() {
            return Ok(Vec::new());
        }
        let raw = match fs::read_to_string(&self.session_file_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut records: Vec<HistoryRecord> = Vec::new();
        let mut hashes: Vec<String> = Vec::new();
        // Per (run, pane): indices of entries that still have no exit, oldest first.
        let mut pending: HashMap<(String, Option<usize>), Vec<usize>> = HashMap::new();
        for line in raw.lines() {
            if let Some(rec) = parse_history_record(line) {
                let key = (rec.entry.run_id.clone(), rec.entry.pane_id);
                pending.entry(key).or_default().push(records.len());
                hashes.push(fnv1a_32_hex(&rec.entry.command));
                records.push(rec);
            } else if let Some(ann) = parse_annotation(line)
                && let Some(stack) = pending.get_mut(&(ann.run_id, ann.pane_id))
                && let Some(pos) = stack.iter().rposition(|&i| hashes[i] == ann.hash)
            {
                let idx = stack.remove(pos);
                records[idx].exit_code = Some(ann.exit_code);
            }
        }
        Ok(records)
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

    /// Appends a cut/copy/yank entry to the session's `clipboard.log` (keeping up to 50 recent entries).
    pub fn record_clipboard_entry(&mut self, text: &str) -> std::io::Result<Option<PathBuf>> {
        if !self.clipboard_persist
            || self.base_dir.as_os_str().is_empty()
            || self.clipboard_file_path.as_os_str().is_empty()
            || text.is_empty()
        {
            return Ok(None);
        }
        if let Some(parent) = self.clipboard_file_path.parent() {
            create_private_dir_all(parent)?;
        }
        let record_line = format!(
            "[{} | session={} | run={} | pane={}] {}\n",
            format_iso_utc_now(),
            self.session_name,
            self.run_id,
            self.pane_id,
            escape_history_command(text)
        );
        let mut file = crate::secure_fs::private_open_options()
            .create(true)
            .append(true)
            .open(&self.clipboard_file_path)?;
        file.write_all(record_line.as_bytes())?;
        file.flush()?;
        Ok(Some(self.clipboard_file_path.clone()))
    }

    /// Loads the newest `max_items` unique clipboard/yank entries for this session (newest first).
    pub fn load_recent_clipboard(&self, max_items: usize) -> Vec<String> {
        if !self.clipboard_persist
            || self.base_dir.as_os_str().is_empty()
            || self.clipboard_file_path.as_os_str().is_empty()
        {
            return Vec::new();
        }
        let Ok(raw) = fs::read_to_string(&self.clipboard_file_path) else {
            return Vec::new();
        };
        let mut ring: Vec<String> = Vec::new();
        for entry in raw.lines().filter_map(parse_history_line) {
            if entry.command.is_empty() {
                continue;
            }
            ring.retain(|existing| existing != &entry.command);
            ring.insert(0, entry.command);
            if ring.len() > max_items.max(1) {
                ring.truncate(max_items.max(1));
            }
        }
        ring
    }

    /// Removes the persisted `clipboard.log` for this session if present.
    pub fn clear_clipboard_history(&self) -> std::io::Result<()> {
        if !self.clipboard_file_path.as_os_str().is_empty()
            && self.clipboard_file_path.exists()
        {
            fs::remove_file(&self.clipboard_file_path)?;
        }
        Ok(())
    }

    /// Exports the given command history slice to a temporary file (or `custom_path`), returning the written `PathBuf`.
    pub fn export_to_temp_file(
        &mut self,
        commands: &[String],
        custom_path: Option<&Path>,
    ) -> std::io::Result<PathBuf> {
        let (target_path, is_default) = match custom_path {
            Some(p) => (p.to_path_buf(), false),
            None => {
                let runtime = crate::secure_fs::private_runtime_dir()?;
                let export_dir = crate::secure_fs::private_dir_in(&runtime, "exports")?;
                let filename = format!(
                    "bilterm-commands-pane{}-{}-{}-{}.sh",
                    self.pane_id,
                    self.session_name,
                    self.run_id,
                    crate::secure_fs::unique_suffix()
                );
                (export_dir.join(filename), true)
            }
        };

        if !is_default
            && let Some(parent) = target_path.parent()
        {
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

        if is_default {
            write_export_exclusive(&target_path, &out)?;
        } else {
            let mut file = crate::secure_fs::private_open_options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&target_path)?;
            file.write_all(out.as_bytes())?;
            file.sync_all()?;
        }
        self.last_exported_temp_path = Some(target_path.clone());
        Ok(target_path)
    }

    /// Removes the least-recently-modified session directories under `<base_dir>/sessions`
    /// while more than `max_files` exist. The active session is never removed.
    pub fn prune_excess_files(&self) -> std::io::Result<usize> {
        if self.base_dir.as_os_str().is_empty() {
            return Ok(0);
        }
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
#[path = "history_store_tests.rs"]
mod tests;
