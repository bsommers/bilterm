//! Atomic Workspace & Session Tree Checkpointing (`PERS-01`, `PERS-02`).
//!
//! Persists the full [`SessionTree`], per-session layout geometry, pane working directories,
//! font scales, scroll modes, floating prompt geometry, command history, and scrollback tails
//! using [`crate::secure_fs`] (`0700` directories + atomic `0600` sibling `.tmp` rename).
//!
//! Enforces strict layout-mode geometry separation so restoring a `"tiled"` workspace never
//! contaminates `ZStack` custom floating window coordinates.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::secure_fs::{
    ensure_private_dir, private_dir_in, private_runtime_dir, validate_plain_file_name,
    write_private_file,
};
use crate::wm::session_tree::SessionTree;

/// Current schema version for [`WorkspaceJournal`].
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

/// Validates that `name` is a safe single-component journal slot identifier (`SEC-R01`, `SEC-R03`).
pub fn validate_safe_name(name: &str) -> io::Result<()> {
    validate_plain_file_name(name).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid journal slot name: {name:?}"),
        )
    })
}

/// Per-pane checkpoint record stored inside a [`SessionJournalRecord`] (`PERS-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneJournalRecord {
    pub pane_id: usize,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shell_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_command: Option<String>,
    pub font_scale_pct: u16,
    pub scroll_mode: String,
    pub prompt_anchor: String,
    pub prompt_offset_y: u16,
    pub prompt_height: u16,
    #[serde(default)]
    pub command_history: Vec<String>,
    #[serde(default)]
    pub scrollback_tail: Vec<String>,
}

/// Per-session checkpoint record stored inside a [`WorkspaceJournal`] (`PERS-01`, `PERS-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionJournalRecord {
    pub session_name: String,
    /// Workspace arrangement mode: `"tiled"` or `"stacked"`.
    pub layout_mode: String,
    pub active_pane_id: usize,
    pub theme_name: String,
    /// `SplitTree` percentage ratios (`10..=90`) used exclusively in `"tiled"` mode.
    #[serde(default)]
    pub split_ratios: Vec<u16>,
    /// Custom `ZStack` `(pane_id, x, y, width, height)` rectangles used exclusively in `"stacked"` mode.
    #[serde(default)]
    pub z_stack_rects: Vec<(usize, u16, u16, u16, u16)>,
    #[serde(default)]
    pub panes: Vec<PaneJournalRecord>,
}

/// Full multi-session workspace and [`SessionTree`] checkpoint (`PERS-01`, `PERS-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceJournal {
    pub version: u32,
    pub timestamp_epoch_secs: u64,
    pub active_session: String,
    pub session_tree: SessionTree,
    #[serde(default)]
    pub sessions: Vec<SessionJournalRecord>,
}

impl Default for WorkspaceJournal {
    fn default() -> Self {
        Self::new("main", SessionTree::new(), Vec::new())
    }
}

impl WorkspaceJournal {
    /// Constructs a new [`WorkspaceJournal`] stamped with the current UNIX epoch time.
    pub fn new(
        active_session: impl Into<String>,
        session_tree: SessionTree,
        sessions: Vec<SessionJournalRecord>,
    ) -> Self {
        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            version: JOURNAL_SCHEMA_VERSION,
            timestamp_epoch_secs,
            active_session: active_session.into(),
            session_tree,
            sessions,
        }
    }

    /// Resolves and ensures the default `0700` journal directory inside the private runtime dir.
    pub fn default_journal_dir() -> io::Result<PathBuf> {
        let runtime = private_runtime_dir()?;
        private_dir_in(&runtime, "journals")
    }

    /// Atomically writes this journal to `<journal_dir>/<slot_name>.toml` and `<journal_dir>/latest.toml` (`PERS-01`).
    ///
    /// Uses [`ensure_private_dir`] (`0700`) and [`write_private_file`] (`0600` sibling `.tmp` + `rename`)
    /// so an interrupted write or power loss never leaves a half-written checkpoint.
    pub fn save_atomic(&self, journal_dir: &Path, slot_name: &str) -> io::Result<PathBuf> {
        validate_safe_name(slot_name)?;
        ensure_private_dir(journal_dir)?;

        // Sanitize records prior to serialization so "tiled" sessions never persist stale ZStack rects.
        let mut clean = self.clone();
        for session in &mut clean.sessions {
            session.z_stack_rects = Self::sanitized_z_stack_rects_for_mode(session);
        }

        let payload = toml::to_string_pretty(&clean)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;

        let file_name = if slot_name.ends_with(".toml") {
            slot_name.to_string()
        } else {
            format!("{slot_name}.toml")
        };
        let slot_path = journal_dir.join(file_name);
        write_private_file(&slot_path, payload.as_bytes())?;

        let latest_path = journal_dir.join("latest.toml");
        if latest_path != slot_path {
            write_private_file(&latest_path, payload.as_bytes())?;
        }
        Ok(slot_path)
    }

    /// Loads and deserializes a [`WorkspaceJournal`] from `<journal_dir>/<slot_name>.toml` (`PERS-02`).
    pub fn load_from_dir(journal_dir: &Path, slot_name: &str) -> io::Result<Self> {
        validate_safe_name(slot_name)?;
        ensure_private_dir(journal_dir)?;

        let file_name = if slot_name.ends_with(".toml") {
            slot_name.to_string()
        } else {
            format!("{slot_name}.toml")
        };
        let slot_path = journal_dir.join(file_name);
        let raw = fs::read_to_string(&slot_path)?;
        let mut journal: Self = toml::from_str(&raw)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;

        for session in &mut journal.sessions {
            session.z_stack_rects = Self::sanitized_z_stack_rects_for_mode(session);
        }
        Ok(journal)
    }

    /// Enforces strict `SplitTree` vs `ZStack` separation: returns an empty vector whenever
    /// `record.layout_mode` is `"tiled"` so `SplitTree` tiles are never contaminated by custom `ZStack` rects.
    pub fn sanitized_z_stack_rects_for_mode(
        record: &SessionJournalRecord,
    ) -> Vec<(usize, u16, u16, u16, u16)> {
        if record.layout_mode.trim().eq_ignore_ascii_case("tiled") {
            Vec::new()
        } else {
            record.z_stack_rects.clone()
        }
    }

    /// Returns clamped `SplitTree` ratios (`10..=90`) for `"tiled"` mode.
    pub fn sanitized_split_ratios_for_mode(record: &SessionJournalRecord) -> Vec<u16> {
        record
            .split_ratios
            .iter()
            .map(|&r| r.clamp(10, 90))
            .collect()
    }
}

impl super::WindowManager {
    /// Handles `:session processes|cmd|shell|cwd|relaunch|info` per-pane process/shell/CWD persistence commands.
    pub(super) fn execute_session_pane_state_command(
        &mut self,
        verb: &str,
        parts: &[&str],
        subcmd: &str,
    ) -> bool {
        match verb {
            "processes" | "apps" | "restore-processes" | "save-apps" => {
                let next = match parts.get(1).copied().map(|s| s.to_ascii_lowercase()) {
                    Some(v) if matches!(v.as_str(), "on" | "true" | "1" | "yes") => true,
                    Some(v) if matches!(v.as_str(), "off" | "false" | "0" | "no") => false,
                    _ => !self.config.session_restore_processes,
                };
                self.config.session_restore_processes = next;
                self.maybe_autosave_layout();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Session process & command auto-relaunch: {} (shell + running command + CWD saved per pane)",
                    if next { "ON" } else { "OFF" }
                )));
                true
            }
            "cmd" | "command" | "app" | "program" => {
                if parts.len() < 2 {
                    let cur = self
                        .active_pane()
                        .resolve_running_command()
                        .unwrap_or_else(|| "(none)".to_string());
                    let pid = self.active_pane_id();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Pane #{pid} saved/running command: {cur}"
                    )));
                    return true;
                }
                let raw_cmd = subcmd
                    .trim_start()
                    .strip_prefix(verb)
                    .unwrap_or("")
                    .trim();
                let pid = self.active_pane_id();
                if matches!(
                    raw_cmd.to_ascii_lowercase().as_str(),
                    "clear" | "none" | "off" | "reset"
                ) {
                    self.active_pane_mut().prompt_mut().running_command = None;
                    self.maybe_autosave_layout();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Cleared saved running command for Pane #{pid}"
                    )));
                } else {
                    self.active_pane_mut().prompt_mut().running_command =
                        Some(raw_cmd.to_string());
                    self.maybe_autosave_layout();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Set saved running command for Pane #{pid}: {raw_cmd}"
                    )));
                }
                true
            }
            "shell" => {
                if parts.len() < 2 {
                    let pid = self.active_pane_id();
                    let sh = self.active_pane().shell_program().to_string();
                    let args = self.active_pane().shell_args().join(" ");
                    let suffix = if args.is_empty() {
                        String::new()
                    } else {
                        format!(" {args}")
                    };
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Pane #{pid} shell: {sh}{suffix}"
                    )));
                    return true;
                }
                let prog = parts[1].to_string();
                let args: Vec<String> = parts[2..].iter().map(|s| (*s).to_string()).collect();
                let pid = self.active_pane_id();
                self.active_pane_mut()
                    .set_shell_program(prog.clone(), args.clone());
                self.maybe_autosave_layout();
                let args_str = if args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", args.join(" "))
                };
                self.active_pane_mut().set_status_message(Some(format!(
                    "Set Pane #{pid} session shell: {prog}{args_str}"
                )));
                true
            }
            "cwd" | "dir" | "workdir" => {
                if parts.len() < 2 {
                    let pid = self.active_pane_id();
                    let cwd = self.active_pane().resolve_cwd();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Pane #{pid} working directory: {}",
                        cwd.display()
                    )));
                    return true;
                }
                let raw_path = parts[1..].join(" ");
                let path = std::path::PathBuf::from(raw_path.trim());
                let pid = self.active_pane_id();
                self.active_pane_mut().set_tracked_cwd(Some(path.clone()));
                self.maybe_autosave_layout();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Set Pane #{pid} session working directory: {}",
                    path.display()
                )));
                true
            }
            "relaunch" | "restart-apps" => {
                let mut count = 0usize;
                for pane in self.panes.values_mut() {
                    if !pane.is_read_only()
                        && let Some(cmd) = pane.resolve_running_command()
                        && !Self::is_unsafe_auto_relaunch_command(&cmd)
                    {
                        let payload = format!("{cmd}\n");
                        let _ = pane.write_to_pty(payload.as_bytes());
                        count += 1;
                    }
                }
                self.active_pane_mut().set_status_message(Some(format!(
                    "Relaunched running command(s) in {count} pane(s)"
                )));
                true
            }
            "info" | "inspect" | "status" | "panes" => {
                let sess = self.config.history_session.clone();
                let auto_proc = if self.config.session_restore_processes {
                    "ON"
                } else {
                    "OFF"
                };
                let mut summary_parts = Vec::new();
                let mut popup_lines = vec![
                    format!(
                        "SESSION: '{sess}' | Layout={:?} | Panes={} | RestoreProcesses={auto_proc}",
                        self.layout_mode,
                        self.panes.len()
                    ),
                    "─".repeat(66),
                ];
                for (&pid, p) in &self.panes {
                    let sh = p.shell_program();
                    let args = p.shell_args().join(" ");
                    let sh_full = if args.is_empty() {
                        sh.to_string()
                    } else {
                        format!("{sh} {args}")
                    };
                    let cwd = p.resolve_cwd().to_string_lossy().into_owned();
                    let cmd = p
                        .resolve_running_command()
                        .unwrap_or_else(|| "(idle)".to_string());
                    summary_parts.push(format!("#{pid}[sh={sh_full}, cwd={cwd}, cmd={cmd}]"));
                    popup_lines.push(format!(
                        "  Pane #{pid} ({}) | Shell: {sh_full} | CWD: {cwd} | Cmd: {cmd}",
                        p.title()
                    ));
                }
                self.open_mcp_popup_window("SESSION PANE STATE (SHELL / CWD / COMMAND)", popup_lines);
                self.active_pane_mut().set_status_message(Some(format!(
                    "Session '{sess}' (processes={auto_proc}): {}",
                    summary_parts.join(" ; ")
                )));
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn sample_journal() -> WorkspaceJournal {
        let mut tree = SessionTree::new();
        tree.branch("main", "workers").unwrap();

        let pane = PaneJournalRecord {
            pane_id: 1,
            title: "Bash #1".to_string(),
            cwd: Some("/tmp".to_string()),
            shell: Some("/bin/bash".to_string()),
            shell_args: Vec::new(),
            running_command: Some("htop".to_string()),
            font_scale_pct: 110,
            scroll_mode: "smooth".to_string(),
            prompt_anchor: "floating".to_string(),
            prompt_offset_y: 2,
            prompt_height: 4,
            command_history: vec!["cargo test -q".to_string()],
            scrollback_tail: vec!["test result: ok.".to_string()],
        };

        let tiled_session = SessionJournalRecord {
            session_name: "main".to_string(),
            layout_mode: "tiled".to_string(),
            active_pane_id: 1,
            theme_name: "turbo-blue".to_string(),
            split_ratios: vec![50, 65],
            // Deliberately populate z_stack_rects on a tiled session to verify sanitization!
            z_stack_rects: vec![(1, 4, 2, 60, 18)],
            panes: vec![pane.clone()],
        };

        let stacked_session = SessionJournalRecord {
            session_name: "workers".to_string(),
            layout_mode: "stacked".to_string(),
            active_pane_id: 1,
            theme_name: "3278".to_string(),
            split_ratios: vec![50],
            z_stack_rects: vec![(1, 6, 3, 70, 20)],
            panes: vec![pane],
        };

        WorkspaceJournal::new("main", tree, vec![tiled_session, stacked_session])
    }

    #[test]
    fn journal_atomic_save_and_load_roundtrip_and_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_dir = tmp.path().join("journals");
        let journal = sample_journal();

        let saved_path = journal.save_atomic(&journal_dir, "checkpoint_1").unwrap();
        assert!(saved_path.exists());
        assert!(journal_dir.join("latest.toml").exists());

        #[cfg(unix)]
        {
            let dir_md = fs::symlink_metadata(&journal_dir).unwrap();
            assert_eq!(dir_md.permissions().mode() & 0o777, 0o700);
            let file_md = fs::symlink_metadata(&saved_path).unwrap();
            assert_eq!(file_md.permissions().mode() & 0o777, 0o600);
        }

        let loaded = WorkspaceJournal::load_from_dir(&journal_dir, "checkpoint_1").unwrap();
        assert_eq!(loaded.active_session, "main");
        assert_eq!(
            loaded.session_tree.subtree_session_names("main"),
            vec!["main", "workers"]
        );
        assert_eq!(loaded.sessions.len(), 2);
        // Tiled session must have its z_stack_rects stripped
        assert!(loaded.sessions[0].z_stack_rects.is_empty());
        assert_eq!(loaded.sessions[0].split_ratios, vec![50, 65]);
        // Stacked session retains its custom z_stack_rects
        assert_eq!(loaded.sessions[1].z_stack_rects, vec![(1, 6, 3, 70, 20)]);
    }

    #[test]
    fn journal_rejects_traversal_slot_names() {
        let tmp = tempfile::tempdir().unwrap();
        let journal_dir = tmp.path().join("journals");
        let journal = sample_journal();

        for bad in &["", "..", "../escape", "sub/slot", "a\\b"] {
            let err = journal.save_atomic(&journal_dir, bad).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput);
        }
    }
}
