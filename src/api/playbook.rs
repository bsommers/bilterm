//! Real-Time Event Subscription Bus & Declarative Robot Playbooks (`AutomationEvent`, `Playbook`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::json::JsonValue;
use super::rpc::{AutomationControlPlane, SnapshotPolicy, extract_vt_row_text};
use crate::error::{BiltermError, Result};

/// Structured automation event emitted by the Bilterm Control Plane (`v1.3.0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationEvent {
    CommandSubmitted {
        pane_id: usize,
        command: String,
    },
    CommandCompleted {
        pane_id: usize,
        command: String,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    TriggerMatched {
        pane_id: usize,
        pattern: String,
        row: u16,
        line: String,
    },
    NotificationReceived {
        pane_id: usize,
        title: String,
        body: String,
    },
    EchoLockChanged {
        pane_id: usize,
        locked: bool,
    },
    AlternateScreenChanged {
        pane_id: usize,
        active: bool,
    },
}

impl AutomationEvent {
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::CommandSubmitted { pane_id, command } => JsonValue::obj([
                ("type", JsonValue::String("command_submitted".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("command", JsonValue::String(command.clone())),
            ]),
            Self::CommandCompleted {
                pane_id,
                command,
                exit_code,
                duration_ms,
            } => JsonValue::obj([
                ("type", JsonValue::String("command_completed".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("command", JsonValue::String(command.clone())),
                (
                    "exit_code",
                    exit_code.map_or(JsonValue::Null, |c| JsonValue::Number(i64::from(c))),
                ),
                (
                    "duration_ms",
                    duration_ms.map_or(JsonValue::Null, |d| JsonValue::Number(d as i64)),
                ),
            ]),
            Self::TriggerMatched {
                pane_id,
                pattern,
                row,
                line,
            } => JsonValue::obj([
                ("type", JsonValue::String("trigger_matched".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("pattern", JsonValue::String(pattern.clone())),
                ("row", JsonValue::Number(i64::from(*row))),
                ("line", JsonValue::String(line.clone())),
            ]),
            Self::NotificationReceived {
                pane_id,
                title,
                body,
            } => JsonValue::obj([
                (
                    "type",
                    JsonValue::String("notification_received".to_string()),
                ),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("title", JsonValue::String(title.clone())),
                ("body", JsonValue::String(body.clone())),
            ]),
            Self::EchoLockChanged { pane_id, locked } => JsonValue::obj([
                ("type", JsonValue::String("echo_lock_changed".to_string())),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("locked", JsonValue::Bool(*locked)),
            ]),
            Self::AlternateScreenChanged { pane_id, active } => JsonValue::obj([
                (
                    "type",
                    JsonValue::String("alternate_screen_changed".to_string()),
                ),
                ("pane_id", JsonValue::Number(*pane_id as i64)),
                ("active", JsonValue::Bool(*active)),
            ]),
        }
    }
}

/// Declarative automation playbook (`bilterm --playbook <file.toml>`) (`v1.3.0`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Playbook {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub steps: Vec<PlaybookStep>,
}

/// Single step in a declarative automation playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookStep {
    /// Action type: `"colon"`, `"exec"`, `"send_keys"`, `"feed_output"`,
    /// `"assert_screen_contains"`, `"assert_trigger_fired"`, `"assert_exit_code"`, or `"export_ppm"`.
    pub action: String,
    #[serde(default)]
    pub pane_id: Option<usize>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub expected: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub path: Option<String>,
}

/// Result of executing a declarative automation playbook.
#[derive(Debug, Clone)]
pub struct PlaybookReport {
    pub name: String,
    pub passed: bool,
    pub steps_completed: usize,
    pub failure_reason: Option<String>,
    pub events: Vec<AutomationEvent>,
}

impl Playbook {
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        toml::from_str(raw).map_err(|e| BiltermError::Config(format!("invalid playbook TOML: {e}")))
    }
}

impl AutomationControlPlane {
    /// Feeds raw VT/ANSI output bytes into a pane and emits structured `AutomationEvent` notifications.
    pub fn feed_output_and_collect_events(
        &mut self,
        pane_id: usize,
        bytes: &[u8],
    ) -> std::result::Result<(), String> {
        let triggers = self.wm.config.triggers.clone();
        let pane = self
            .wm
            .panes
            .get_mut(&pane_id)
            .ok_or_else(|| format!("pane #{pane_id} not found"))?;
        let prev_alt = pane.vt().is_alternate_screen();
        let prev_blocks = pane.vt().command_blocks().len();
        let prev_prev_exit = pane
            .vt()
            .command_blocks()
            .last()
            .and_then(|b| b.exit_code);
        let prev_notifs = pane.vt().notifications().len();

        pane.process_pty_output(bytes);

        let now_alt = pane.vt().is_alternate_screen();
        let new_notifs: Vec<(String, String)> = pane
            .vt()
            .notifications()
            .iter()
            .skip(prev_notifs)
            .map(|n| (n.title.clone(), n.body.clone()))
            .collect();
        let completed_block = pane.vt().command_blocks().last().and_then(|b| {
            if (pane.vt().command_blocks().len() > prev_blocks || b.exit_code != prev_prev_exit)
                && b.exit_code.is_some()
            {
                Some((b.command.clone(), b.exit_code, b.duration_ms))
            } else {
                None
            }
        });

        let mut matched_triggers = Vec::new();
        if !triggers.is_empty() {
            let (rows, _) = pane.vt().size();
            for r in 0..rows {
                let row_str = extract_vt_row_text(pane.vt(), r);
                let row_lower = row_str.to_ascii_lowercase();
                for pat in &triggers {
                    if !pat.is_empty() && row_lower.contains(&pat.to_ascii_lowercase()) {
                        matched_triggers.push((pat.clone(), r, row_str.clone()));
                    }
                }
            }
        }

        if now_alt != prev_alt {
            self.push_event(AutomationEvent::AlternateScreenChanged {
                pane_id,
                active: now_alt,
            });
        }
        for (title, body) in new_notifs {
            self.push_event(AutomationEvent::NotificationReceived {
                pane_id,
                title,
                body,
            });
        }
        if let Some((command, exit_code, duration_ms)) = completed_block {
            self.push_event(AutomationEvent::CommandCompleted {
                pane_id,
                command,
                exit_code,
                duration_ms,
            });
        }
        for (pattern, row, line) in matched_triggers {
            self.push_event(AutomationEvent::TriggerMatched {
                pane_id,
                pattern,
                row,
                line,
            });
        }
        Ok(())
    }

    /// Executes a declarative `Playbook` step-by-step with the given `SnapshotPolicy`.
    pub fn run_playbook_with_policy(
        &mut self,
        playbook: &Playbook,
        policy: SnapshotPolicy,
    ) -> std::result::Result<PlaybookReport, String> {
        let mut completed = 0;
        for (idx, step) in playbook.steps.iter().enumerate() {
            let step_num = idx + 1;
            match step.action.as_str() {
                "colon" => {
                    let cmd = step
                        .command
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'colon' requires 'command'"))?;
                    self.dispatch_method(
                        "wm.command",
                        &JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]),
                    )?;
                }
                "exec" => {
                    let cmd = step
                        .command
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'exec' requires 'command'"))?;
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    self.dispatch_method(
                        "pane.exec",
                        &JsonValue::obj([
                            ("pane_id", JsonValue::Number(pane_id as i64)),
                            ("command", JsonValue::String(cmd.to_string())),
                        ]),
                    )?;
                }
                "send_keys" => {
                    let arr: Vec<JsonValue> =
                        step.keys.iter().cloned().map(JsonValue::String).collect();
                    self.dispatch_method(
                        "pane.send_keys",
                        &JsonValue::obj([("keys", JsonValue::Array(arr))]),
                    )?;
                }
                "feed_output" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let data = step.data.as_deref().unwrap_or("");
                    self.feed_output_and_collect_events(pane_id, data.as_bytes())?;
                }
                "assert_screen_contains" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let expected = step.expected.as_deref().unwrap_or("");
                    let pane = self
                        .wm
                        .panes
                        .get(&pane_id)
                        .ok_or_else(|| format!("step {step_num}: pane #{pane_id} not found"))?;
                    let (rows, _) = pane.vt().size();
                    let found =
                        (0..rows).any(|r| extract_vt_row_text(pane.vt(), r).contains(expected));
                    if !found {
                        return Err(format!(
                            "step {step_num}: assertion failed — pane #{pane_id} screen does not contain '{expected}'"
                        ));
                    }
                }
                "assert_trigger_fired" => {
                    let pat = step.pattern.as_deref().unwrap_or("");
                    let fired = self.events.iter().any(|ev| {
                        matches!(
                            ev,
                            AutomationEvent::TriggerMatched { pattern, .. }
                            if pattern.eq_ignore_ascii_case(pat)
                        )
                    });
                    if !fired {
                        return Err(format!(
                            "step {step_num}: assertion failed — trigger '{pat}' did not fire"
                        ));
                    }
                }
                "assert_exit_code" => {
                    let pane_id = step.pane_id.unwrap_or_else(|| self.wm.active_pane_id());
                    let expected_code = step.exit_code.unwrap_or(0);
                    let pane = self
                        .wm
                        .panes
                        .get(&pane_id)
                        .ok_or_else(|| format!("step {step_num}: pane #{pane_id} not found"))?;
                    let actual = pane.vt().command_blocks().last().and_then(|b| b.exit_code);
                    if actual != Some(expected_code) {
                        return Err(format!(
                            "step {step_num}: assertion failed — expected exit_code {expected_code}, got {actual:?}"
                        ));
                    }
                }
                "export_ppm" => {
                    let path = step
                        .path
                        .as_deref()
                        .ok_or_else(|| format!("step {step_num}: 'export_ppm' requires 'path'"))?;
                    match policy {
                        SnapshotPolicy::Trusted => {
                            self.write_snapshot_trusted(Path::new(path))?;
                        }
                        SnapshotPolicy::Confined => {
                            self.write_snapshot_confined(path)?;
                        }
                    }
                }
                other => {
                    return Err(format!("step {step_num}: unknown playbook action '{other}'"));
                }
            }
            completed += 1;
        }
        Ok(PlaybookReport {
            name: playbook.name.clone(),
            passed: true,
            steps_completed: completed,
            failure_reason: None,
            events: self.events.iter().cloned().collect(),
        })
    }

    /// Executes a declarative `Playbook` step-by-step with `SnapshotPolicy::Trusted`.
    pub fn run_playbook(
        &mut self,
        playbook: &Playbook,
    ) -> std::result::Result<PlaybookReport, String> {
        self.run_playbook_with_policy(playbook, SnapshotPolicy::Trusted)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::api::{default_snapshot_dir_from, validate_snapshot_name};

    #[test]
    fn snapshot_name_validation() {
        for bad in [
            "",
            ".",
            "..",
            "../x.ppm",
            "/tmp/x.ppm",
            "a/../../x.ppm",
            "sub/x.ppm",
            "a\\b.ppm",
            "x\0.ppm",
        ] {
            assert!(
                validate_snapshot_name(bad).is_err(),
                "expected rejection of '{bad}'"
            );
        }
        for good in ["frame.ppm", "frame-01.ppm"] {
            assert!(
                validate_snapshot_name(good).is_ok(),
                "expected acceptance of '{good}'"
            );
        }
    }

    fn check_no_leaked_file(dir: &Path, bad_name: &str) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    check_no_leaked_file(&p, bad_name);
                } else {
                    assert_ne!(
                        p.file_name().and_then(|n| n.to_str()),
                        Some(bad_name),
                        "found leaked file: {}",
                        p.display()
                    );
                }
            }
        }
    }

    #[test]
    fn snapshot_confined_rejects_traversal_and_writes_nothing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());

        for bad in [
            "../x.ppm",
            "/tmp/x.ppm",
            "a/../../x.ppm",
            "sub/x.ppm",
            "",
            ".",
            "..",
        ] {
            let req = JsonValue::obj([
                ("jsonrpc", JsonValue::String("2.0".to_string())),
                ("id", JsonValue::Number(1)),
                ("method", JsonValue::String("workspace.snapshot".to_string())),
                (
                    "params",
                    JsonValue::obj([("ppm_path", JsonValue::String(bad.to_string()))]),
                ),
            ])
            .to_json_string();

            let resp = plane.handle_jsonrpc_line(&req);
            assert!(
                resp.contains("ppm_path must be a file name within the snapshot directory"),
                "bad path '{bad}' response: {resp}"
            );
        }

        check_no_leaked_file(temp_dir.path(), "x.ppm");
    }

    #[test]
    #[cfg(unix)]
    fn snapshot_confined_refuses_planted_symlink() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        std::fs::create_dir_all(&snaps_dir).unwrap();

        let victim = temp_dir.path().join("victim");
        std::fs::write(&victim, b"keep").unwrap();

        let evil = snaps_dir.join("evil.ppm");
        std::os::unix::fs::symlink(&victim, &evil).unwrap();

        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("workspace.snapshot".to_string())),
            (
                "params",
                JsonValue::obj([("ppm_path", JsonValue::String("evil.ppm".to_string()))]),
            ),
        ])
        .to_json_string();

        let resp = plane.handle_jsonrpc_line(&req);
        assert!(
            resp.contains("ppm_path must be a file name within the snapshot directory")
                || resp.contains("error"),
            "response: {resp}"
        );
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
    }

    #[test]
    fn snapshot_confined_writes_inside_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("workspace.snapshot".to_string())),
            (
                "params",
                JsonValue::obj([("ppm_path", JsonValue::String("frame.ppm".to_string()))]),
            ),
        ])
        .to_json_string();

        let resp = plane.handle_jsonrpc_line(&req);
        assert!(!resp.contains("error"), "got error response: {resp}");

        let target = snaps_dir.join("frame.ppm");
        assert!(target.exists());
        let contents = std::fs::read(&target).unwrap();
        assert!(contents.starts_with(b"P6\n"), "missing P6 header");
    }

    #[test]
    fn snapshot_without_ppm_path_is_status_only() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("workspace.snapshot".to_string())),
        ])
        .to_json_string();

        let resp = plane.handle_jsonrpc_line(&req);
        assert!(resp.contains("pane_count"), "got: {resp}");
        assert!(!snaps_dir.exists());
    }

    #[test]
    fn default_snapshot_dir_from_chain() {
        use std::ffi::OsStr;
        let s1 = default_snapshot_dir_from(Some(OsStr::new("/s")), None).unwrap();
        assert_eq!(s1, PathBuf::from("/s/bilterm/snapshots"));

        let s2 = default_snapshot_dir_from(None, Some(OsStr::new("/h"))).unwrap();
        assert_eq!(s2, PathBuf::from("/h/.local/state/bilterm/snapshots"));

        assert_eq!(default_snapshot_dir_from(None, None), None);
    }

    #[test]
    fn mcp_snapshot_path_is_confined() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());

        let pb_toml = r#"
name = "escape_test"
version = "1.0.0"
[[steps]]
action = "export_ppm"
path = "../escape.ppm"
"#;
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("tools/call".to_string())),
            (
                "params",
                JsonValue::obj([
                    (
                        "name",
                        JsonValue::String("bilterm_run_playbook".to_string()),
                    ),
                    (
                        "arguments",
                        JsonValue::obj([("toml", JsonValue::String(pb_toml.to_string()))]),
                    ),
                ]),
            ),
        ])
        .to_json_string();

        let resp = plane.handle_mcp_line(&req).unwrap();
        assert!(
            resp.contains("\"isError\":true") || resp.contains("error"),
            "response: {resp}"
        );
        assert!(!temp_dir.path().join("escape.ppm").exists());
    }

    #[test]
    fn rpc_playbook_export_is_confined() {
        let temp_dir = tempfile::tempdir().unwrap();
        let snaps_dir = temp_dir.path().join("snaps");
        let mut plane =
            AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snaps_dir.clone());

        let abs_path = temp_dir.path().join("abs.ppm");
        let pb_bad = format!(
            r#"
name = "abs_test"
version = "1.0.0"
[[steps]]
action = "export_ppm"
path = "{}"
"#,
            abs_path.display()
        );

        let req_bad = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("playbook.run".to_string())),
            ("params", JsonValue::obj([("toml", JsonValue::String(pb_bad))])),
        ])
        .to_json_string();

        let resp_bad = plane.handle_jsonrpc_line(&req_bad);
        assert!(
            resp_bad.contains("error"),
            "expected error response: {resp_bad}"
        );
        assert!(!abs_path.exists());

        let pb_good = r#"
name = "good_test"
version = "1.0.0"
[[steps]]
action = "export_ppm"
path = "pb.ppm"
"#;
        let req_good = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(2)),
            ("method", JsonValue::String("playbook.run".to_string())),
            (
                "params",
                JsonValue::obj([("toml", JsonValue::String(pb_good.to_string()))]),
            ),
        ])
        .to_json_string();

        let resp_good = plane.handle_jsonrpc_line(&req_good);
        assert!(
            !resp_good.contains("error"),
            "expected success: {resp_good}"
        );
        assert!(snaps_dir.join("pb.ppm").exists());
    }

    #[test]
    fn fleet_register_rejects_option_destination() {
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("fleet.register".to_string())),
            (
                "params",
                JsonValue::obj([
                    ("name", JsonValue::String("bad_node".to_string())),
                    (
                        "destination",
                        JsonValue::String("-oProxyCommand=evil".to_string()),
                    ),
                ]),
            ),
        ])
        .to_json_string();

        let resp = plane.handle_jsonrpc_line(&req);
        assert!(resp.contains("error"), "expected error response: {resp}");
        assert!(!plane.wm.fleet.nodes.contains_key("bad_node"));
    }

    #[test]
    fn fleet_register_accepts_valid_destination() {
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let req = JsonValue::obj([
            ("jsonrpc", JsonValue::String("2.0".to_string())),
            ("id", JsonValue::Number(1)),
            ("method", JsonValue::String("fleet.register".to_string())),
            (
                "params",
                JsonValue::obj([
                    ("name", JsonValue::String("good_node".to_string())),
                    (
                        "destination",
                        JsonValue::String("ubuntu@10.0.0.42".to_string()),
                    ),
                ]),
            ),
        ])
        .to_json_string();

        let resp = plane.handle_jsonrpc_line(&req);
        assert!(!resp.contains("error"), "expected success: {resp}");
        assert!(plane.wm.fleet.nodes.contains_key("good_node"));
    }
}
