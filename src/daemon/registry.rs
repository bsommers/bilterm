//! Live Background Daemon Discovery & Metadata Registry (`DAEM-06`, `DAEM-07`, `SEC-R01..04`).
//!
//! Maintains `<runtime_dir>/daemons/<name>.json` records with automatic dead-peer pruning
//! and cross-daemon state reporting.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::json::JsonValue;
use crate::daemon::supervisor::{check_pid_alive, now_ms};
use crate::secure_fs::{ensure_private_dir, validate_safe_name, write_private_file};

/// Metadata record for a live background daemon instance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonRegistryEntry {
    /// Unique instance ID, e.g. `"daem_01jcb89z..."`.
    pub id: String,
    /// Validated safe name, e.g. `"default"`, `"worker-1"`.
    pub name: String,
    /// OS process ID.
    pub pid: u32,
    /// `CARGO_PKG_VERSION` of the running daemon binary.
    pub version: String,
    /// Epoch timestamp when the daemon started (milliseconds).
    pub started_at_ms: u64,
    /// Path to the Unix domain socket.
    pub socket_path: PathBuf,
    /// Number of active sessions hosted by this daemon.
    pub sessions_count: usize,
    /// Total panes running across all sessions.
    pub panes_count: usize,
    /// Number of registered agents.
    pub agents_count: usize,
    /// Epoch timestamp of the last heartbeat (milliseconds).
    pub last_heartbeat_ms: u64,
}

impl DaemonRegistryEntry {
    /// Returns true if this entry corresponds to the current process.
    pub fn is_current(&self) -> bool {
        self.pid == std::process::id()
    }

    /// Serializes entry to compact JSON.
    pub fn to_json(&self) -> String {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), JsonValue::String(self.id.clone()));
        map.insert("name".to_string(), JsonValue::String(self.name.clone()));
        map.insert("pid".to_string(), JsonValue::Number(self.pid as i64));
        map.insert(
            "version".to_string(),
            JsonValue::String(self.version.clone()),
        );
        map.insert(
            "started_at_ms".to_string(),
            JsonValue::Number(self.started_at_ms as i64),
        );
        map.insert(
            "socket_path".to_string(),
            JsonValue::String(self.socket_path.to_string_lossy().into_owned()),
        );
        map.insert(
            "sessions_count".to_string(),
            JsonValue::Number(self.sessions_count as i64),
        );
        map.insert(
            "panes_count".to_string(),
            JsonValue::Number(self.panes_count as i64),
        );
        map.insert(
            "agents_count".to_string(),
            JsonValue::Number(self.agents_count as i64),
        );
        map.insert(
            "last_heartbeat_ms".to_string(),
            JsonValue::Number(self.last_heartbeat_ms as i64),
        );
        JsonValue::Object(map).to_json_string()
    }

    /// Deserializes an entry from JSON text.
    pub fn from_json(s: &str) -> Result<Self, String> {
        let val = JsonValue::parse(s).ok_or_else(|| "invalid json in registry entry".to_string())?;
        let id = val
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("missing id")?
            .to_string();
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("missing name")?
            .to_string();
        let pid = val
            .get("pid")
            .and_then(|v| v.as_i64())
            .ok_or("missing or invalid pid")? as u32;
        let version = val
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or("missing version")?
            .to_string();
        let started_at_ms = val
            .get("started_at_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u64;
        let socket_path = val
            .get("socket_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .ok_or("missing socket_path")?;
        let sessions_count = val
            .get("sessions_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        let panes_count = val
            .get("panes_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        let agents_count = val
            .get("agents_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as usize;
        let last_heartbeat_ms = val
            .get("last_heartbeat_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u64;

        Ok(Self {
            id,
            name,
            pid,
            version,
            started_at_ms,
            socket_path,
            sessions_count,
            panes_count,
            agents_count,
            last_heartbeat_ms,
        })
    }
}

/// Registry manager for discovering, registering, and pruning daemon metadata files.
pub struct DaemonRegistry;

impl DaemonRegistry {
    /// Resolves `<base_dir>/daemons/<name>.json`.
    pub fn entry_path_in(base_dir: &Path, name: &str) -> io::Result<PathBuf> {
        validate_safe_name(name).map_err(|e| {
            io::Error::new(ErrorKind::InvalidInput, format!("invalid daemon name: {e}"))
        })?;
        let dir = base_dir.join("daemons");
        ensure_private_dir(&dir)?;
        Ok(dir.join(format!("{name}.json")))
    }

    /// Registers or updates metadata for a daemon in `<base_dir>/daemons/<name>.json` atomically (0600).
    pub fn register(base_dir: &Path, entry: &DaemonRegistryEntry) -> io::Result<()> {
        let path = Self::entry_path_in(base_dir, &entry.name)?;
        let json = entry.to_json();
        write_private_file(&path, json.as_bytes())
    }

    /// Updates the heartbeat timestamp and counts for a registered daemon.
    pub fn update_heartbeat(
        base_dir: &Path,
        name: &str,
        sessions: usize,
        panes: usize,
        agents: usize,
    ) -> io::Result<()> {
        let path = Self::entry_path_in(base_dir, name)?;
        let content = fs::read_to_string(&path)?;
        let mut entry = DaemonRegistryEntry::from_json(&content)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;

        entry.sessions_count = sessions;
        entry.panes_count = panes;
        entry.agents_count = agents;
        entry.last_heartbeat_ms = now_ms();

        write_private_file(&path, entry.to_json().as_bytes())
    }

    /// Scans `<base_dir>/daemons/*.json`, verifies process liveness, prunes dead entries,
    /// and returns all active daemons sorted by name.
    pub fn list_active(base_dir: &Path) -> Vec<DaemonRegistryEntry> {
        let dir = base_dir.join("daemons");
        let Ok(read_dir) = fs::read_dir(dir) else {
            return Vec::new();
        };

        let mut active = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(item) = DaemonRegistryEntry::from_json(&content) else {
                continue;
            };

            if check_pid_alive(item.pid) {
                active.push(item);
            } else {
                // Prune dead peer
                let _ = fs::remove_file(&path);
                if fs::symlink_metadata(&item.socket_path).is_ok() {
                    let _ = fs::remove_file(&item.socket_path);
                }
            }
        }

        active.sort_by(|a, b| a.name.cmp(&b.name));
        active
    }

    /// Unregisters daemon `name`, removing its `.json` file.
    pub fn unregister(base_dir: &Path, name: &str) -> io::Result<()> {
        let path = Self::entry_path_in(base_dir, name)?;
        if fs::symlink_metadata(&path).is_ok() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Prunes all dead daemons in `<base_dir>/daemons/`, returning the count of pruned entries.
    pub fn prune_dead(base_dir: &Path) -> usize {
        let dir = base_dir.join("daemons");
        let Ok(read_dir) = fs::read_dir(dir) else {
            return 0;
        };

        let mut count = 0;
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(item) = DaemonRegistryEntry::from_json(&content) else {
                continue;
            };

            if !check_pid_alive(item.pid) {
                let _ = fs::remove_file(&path);
                if fs::symlink_metadata(&item.socket_path).is_ok() {
                    let _ = fs::remove_file(&item.socket_path);
                }
                count += 1;
            }
        }
        count
    }
}
