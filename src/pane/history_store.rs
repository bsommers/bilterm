use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generates a unique session identifier combining unix epoch seconds, process ID, and an atomic counter.
pub fn generate_session_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pid = std::process::id();
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{secs}-{pid}-{seq}")
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

/// Persistent session command history manager for a `CompositePane`.
///
/// Records every submitted command with:
/// - ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`)
/// - Unique session ID (`<epoch>-<pid>-<seq>`)
/// - Terminal/Pane ID (`pane=<id>`)
///
/// Enforces automatic file rotation (`max_files`).
#[derive(Clone, Debug)]
pub struct HistoryStore {
    pub enabled: bool,
    pub session_id: String,
    pub pane_id: usize,
    pub base_dir: PathBuf,
    pub session_file_path: PathBuf,
    pub max_files: usize,
    pub last_exported_temp_path: Option<PathBuf>,
}

impl HistoryStore {
    /// Creates a `HistoryStore` for `pane_id` inside `base_dir`.
    pub fn new(pane_id: usize, session_id: impl Into<String>, base_dir: PathBuf, enabled: bool, max_files: usize) -> Self {
        let sid = session_id.into();
        let file_name = format!("bilterm-history-{sid}-pane-{pane_id}.log");
        let session_file_path = base_dir.join(file_name);
        Self {
            enabled,
            session_id: sid,
            pane_id,
            base_dir,
            session_file_path,
            max_files: max_files.max(1),
            last_exported_temp_path: None,
        }
    }

    /// Creates a headless in-temp `HistoryStore` (disabled by default in unit tests unless enabled).
    pub fn new_headless(pane_id: usize) -> Self {
        let sid = generate_session_id();
        let base_dir = std::env::temp_dir().join("bilterm-test-history");
        Self::new(pane_id, sid, base_dir, false, 25)
    }

    /// Creates a live `HistoryStore` enabled by default and backed by `default_history_dir`.
    pub fn new_live(pane_id: usize) -> Self {
        let sid = generate_session_id();
        let base_dir = default_history_dir(None);
        Self::new(pane_id, sid, base_dir, true, 25)
    }

    /// Reconfigures the history directory, enabled flag, and max retained session files.
    pub fn configure(&mut self, enabled: bool, max_files: usize, custom_dir: Option<&str>) {
        self.enabled = enabled;
        self.max_files = max_files.max(1);
        if let Some(dir) = custom_dir {
            self.base_dir = default_history_dir(Some(dir));
            let file_name = format!("bilterm-history-{}-pane-{}.log", self.session_id, self.pane_id);
            self.session_file_path = self.base_dir.join(file_name);
        }
    }

    /// Appends a command entry with timestamp, session ID, and pane ID to `session_file_path`,
    /// pruning older session log files if the directory exceeds `max_files`.
    pub fn record_command(&self, command: &str) -> std::io::Result< Option<PathBuf> > {
        if !self.enabled || command.trim().is_empty() {
            return Ok(None);
        }
        fs::create_dir_all(&self.base_dir)?;
        let ts = format_iso_utc_now();
        let escaped = command.replace('\n', "\\n");
        let record_line = format!(
            "[{} | session={} | pane={}] {}\n",
            ts, self.session_id, self.pane_id, escaped
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.session_file_path)?;
        file.write_all(record_line.as_bytes())?;
        file.flush()?;

        let _ = self.prune_excess_files();
        Ok(Some(self.session_file_path.clone()))
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
                    "bilterm-commands-pane{}-{}-{ts}.sh",
                    self.pane_id, self.session_id
                ))
            }
        };

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out = String::new();
        out.push_str(&format!(
            "# Bilterm Exported Command History\n# Session: {}\n# Terminal Pane: #{}\n# Exported At: {}\n\n",
            self.session_id,
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

    /// Rotates/prunes oldest `bilterm-history-*.log` files in `base_dir` when total count exceeds `max_files`.
    pub fn prune_excess_files(&self) -> std::io::Result<usize> {
        let Ok(entries) = fs::read_dir(&self.base_dir) else {
            return Ok(0);
        };
        let mut files: Vec<(PathBuf, SystemTime)> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                if !name.starts_with("bilterm-history-") || !name.ends_with(".log") {
                    return None;
                }
                let mtime = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(UNIX_EPOCH);
                Some((e.path(), mtime))
            })
            .collect();

        if files.len() <= self.max_files {
            return Ok(0);
        }

        // Sort oldest first by modification time, then filename
        files.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let to_remove = files.len() - self.max_files;
        let mut removed = 0;
        for (path, _) in files.into_iter().take(to_remove) {
            if path != self.session_file_path && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_store_persists_metadata_exports_temp_file_and_prunes_old_logs() {
        let tmp_root = std::env::temp_dir().join(format!(
            "bilterm-hist-unit-{}",
            generate_session_id()
        ));
        let _ = fs::remove_dir_all(&tmp_root);

        let mut store = HistoryStore::new(1, "sess-100", tmp_root.clone(), true, 2);
        let written = store
            .record_command("tail -f /dev/null")
            .unwrap()
            .expect("written path");
        assert!(written.exists());
        let contents = fs::read_to_string(&written).unwrap();
        assert!(contents.contains("session=sess-100"));
        assert!(contents.contains("pane=1"));
        assert!(contents.contains("tail -f /dev/null"));

        // Create 2 more session files with max_files = 2 -> oldest should be pruned
        let store2 = HistoryStore::new(2, "sess-101", tmp_root.clone(), true, 2);
        store2.record_command("echo second").unwrap();
        let store3 = HistoryStore::new(3, "sess-102", tmp_root.clone(), true, 2);
        store3.record_command("echo third").unwrap();

        let remaining: Vec<_> = fs::read_dir(&tmp_root).unwrap().flatten().collect();
        assert_eq!(remaining.len(), 2);

        // Export commands to temporary file
        let exported = store
            .export_to_temp_file(&["ls -la".to_string(), "cargo test".to_string()], None)
            .unwrap();
        assert!(exported.exists());
        let exp_text = fs::read_to_string(&exported).unwrap();
        assert!(exp_text.contains("# Session: sess-100"));
        assert!(exp_text.contains("cargo test"));

        let _ = fs::remove_file(&exported);
        let _ = fs::remove_dir_all(&tmp_root);
    }
}
