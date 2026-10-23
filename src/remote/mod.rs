//! First-Class SSH Domains, Remote `tmux -CC` over SSH & `herdr`-Style Central Fleet Management (`v1.4.0`).
//!
//! Enables Bilterm to centrally manage both **local composite panes** and **remote SSH / `tmux -CC` / JSON-RPC peer sessions**:
//! 1. **Direct SSH Composite Panes (`:ssh <user@host>`)**: Spawns SSH sessions with OpenSSH `ControlMaster` multiplexing,
//!    remote `OSC 133` prompt-marker bootstrap, and a live `🌐 SSH:<host>` badge in the 3270 OIA status bar.
//! 2. **Remote Persistent `tmux -CC` over SSH (`:ssh-tmux <user@host> [session]`)**: Streams remote `tmux -CC` control mode
//!    over SSH into [`crate::tmux::TmuxControlBridge`], hydrating remote panes as local `CompositePane`s.
//! 3. **`herdr`-Style Fleet Registry (`FleetRegistry`, `:remote add|list|exec|disconnect`)**: Centrally tracks local panes,
//!    SSH panes, remote `tmux -CC` sessions, and federated `bilterm --headless` JSON-RPC nodes with `@tag` selector fan-out.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{BiltermError, Result};
use crate::pane::PaneId;

/// Declarative remote host profile configurable in `~/.config/bilterm/config.toml` under `[remotes.<name>]`
/// or registered dynamically via `:remote add <name> <user@host> [@tag1,@tag2]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteHostProfile {
    /// Logical alias for the remote host (e.g. `"prod-db"`, `"gpu-1"`).
    pub name: String,
    /// SSH destination (`user@hostname` or `hostname`).
    pub destination: String,
    /// Optional custom SSH port (`-p <port>`).
    #[serde(default)]
    pub port: Option<u16>,
    /// Optional SSH private key path (`-i <identity_file>`).
    #[serde(default)]
    pub identity_file: Option<String>,
    /// Enables OpenSSH `ControlMaster=auto` and `ControlPersist=60s` for instant multi-pane SSH multiplexing.
    #[serde(default = "default_true")]
    pub control_master: bool,
    /// Injects a non-invasive POSIX `OSC 133` prompt-marker bootstrap over SSH so command blocks and exit codes work remotely.
    #[serde(default = "default_true")]
    pub remote_osc133_bootstrap: bool,
    /// Optional fleet tags (e.g. `["prod", "db"]`) for `@tag` selector targeting.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional remote `bilterm --headless --listen <addr>` JSON-RPC endpoint if federated.
    #[serde(default)]
    pub rpc_endpoint: Option<String>,
}

fn default_true() -> bool {
    true
}

pub const CONTROL_PATH_LEAF: &str = "ssh-%C";
pub const CONTROL_DIR_MAX_BYTES: usize = 40;

/// Validates that an SSH destination does not contain shell injection, options, or control chars.
pub fn validate_destination(dest: &str) -> Result<()> {
    if dest.is_empty() {
        return Err(BiltermError::Config(format!(
            "invalid SSH destination {dest:?}: empty"
        )));
    }
    if dest.starts_with('-') {
        return Err(BiltermError::Config(format!(
            "invalid SSH destination {dest:?}: leading '-' not allowed"
        )));
    }
    if let Some(ch) = dest.chars().find(|c| c.is_whitespace() || c.is_control()) {
        return Err(BiltermError::Config(format!(
            "invalid SSH destination {dest:?}: contains invalid character {ch:?}"
        )));
    }
    Ok(())
}

impl RemoteHostProfile {
    pub fn new(name: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            destination: destination.into(),
            port: None,
            identity_file: None,
            control_master: true,
            remote_osc133_bootstrap: true,
            tags: Vec::new(),
            rpc_endpoint: None,
        }
    }

    fn push_common_args(&self, args: &mut Vec<String>, control_dir: Option<&Path>) {
        if self.control_master
            && let Some(dir) = control_dir
            && let Some(s) = dir.to_str()
        {
            if s.len() <= CONTROL_DIR_MAX_BYTES
                && !s.contains(char::is_whitespace)
                && !s.contains('%')
            {
                args.push("-o".to_string());
                args.push("ControlMaster=auto".to_string());
                args.push("-o".to_string());
                args.push("ControlPersist=60s".to_string());
                args.push("-o".to_string());
                args.push(format!("ControlPath={s}/{CONTROL_PATH_LEAF}"));
            } else {
                tracing::warn!(
                    "ssh control_dir {:?} unsafe or exceeds {} bytes; ControlMaster disabled",
                    s,
                    CONTROL_DIR_MAX_BYTES
                );
            }
        }
        if let Some(port) = self.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }
        if let Some(id_file) = &self.identity_file {
            args.push("-i".to_string());
            args.push(id_file.clone());
        }
    }

    /// Builds the `ssh` CLI arguments for spawning an interactive remote shell pane.
    ///
    /// Validates destination against injection attacks, pushes `--` before the destination,
    /// and uses `<control_dir>/ssh-%C` for ControlPath when a safe directory is provided.
    pub fn build_ssh_args(&self, control_dir: Option<&Path>) -> Result<Vec<String>> {
        validate_destination(&self.destination)?;
        let mut args = vec!["-tt".to_string()];
        self.push_common_args(&mut args, control_dir);
        args.push("--".to_string());
        args.push(self.destination.clone());
        Ok(args)
    }

    /// Builds the `ssh` CLI arguments for attaching to a persistent remote `tmux -CC` Control Mode session over SSH.
    ///
    /// Validates destination against injection attacks, pushes `--` before destination,
    /// and keeps the remote tmux command as the final argument.
    pub fn build_ssh_tmux_cc_args(
        &self,
        session_name: &str,
        control_dir: Option<&Path>,
    ) -> Result<Vec<String>> {
        validate_destination(&self.destination)?;
        let mut args = vec!["-T".to_string()];
        self.push_common_args(&mut args, control_dir);
        args.push("--".to_string());
        args.push(self.destination.clone());
        let sanitized_session: String = session_name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let sess = if sanitized_session.is_empty() {
            "bilterm"
        } else {
            &sanitized_session
        };
        args.push(format!("tmux -CC new-session -A -s {sess}"));
        Ok(args)
    }
}

/// Returns a one-line POSIX shell snippet that configures `OSC 133` prompt/command-exit markers
/// and `OSC 7` working directory reporting on a remote SSH host without modifying remote dotfiles.
pub fn remote_osc133_bootstrap_command() -> &'static str {
    " PROMPT_COMMAND='__bt_ec=$?; printf \"\\033]133;D;%d\\007\\033]7;file://%s%s\\007\\033]133;A\\007\" \"$__bt_ec\" \"$(hostname)\" \"$PWD\"'; printf '\\033]133;A\\007'\n"
}

/// Connection transport type for a node managed by Bilterm's `FleetRegistry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetNodeKind {
    /// A direct SSH session attached to a local `CompositePane`.
    SshPane {
        pane_id: PaneId,
        destination: String,
    },
    /// A persistent remote `tmux -CC` session over SSH mapped to one or more local `CompositePane`s.
    SshTmux {
        destination: String,
        session_name: String,
        pane_ids: Vec<PaneId>,
    },
    /// A registered remote host profile or federated `bilterm --headless` JSON-RPC peer.
    HostProfile {
        destination: String,
        rpc_endpoint: Option<String>,
    },
}

/// Runtime entry in Bilterm's `herdr`-style central fleet manager (`v1.4.0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNode {
    pub name: String,
    pub kind: FleetNodeKind,
    pub tags: Vec<String>,
    pub connected: bool,
    pub last_dispatched_command: Option<String>,
}

/// Target selector for `herdr`-style fleet command fan-out (`:remote exec <selector> <cmd>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetSelector {
    /// Targets every unlocked local and remote pane plus registered fleet nodes (`all` or `*`).
    All,
    /// Targets only local panes (`local`).
    LocalOnly,
    /// Targets all remote SSH / `tmux -CC` panes and nodes (`remote`).
    RemoteOnly,
    /// Targets nodes/panes carrying a specific tag (`@prod`, `@db`).
    Tag(String),
    /// Targets a specific node name or host destination (`prod-1` or `ubuntu@10.0.0.5`).
    Node(String),
}

impl FleetSelector {
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        match trimmed {
            "all" | "*" => Self::All,
            "local" => Self::LocalOnly,
            "remote" | "ssh" => Self::RemoteOnly,
            s if s.starts_with('@') && s.len() > 1 => Self::Tag(s[1..].to_string()),
            other => Self::Node(other.to_string()),
        }
    }
}

/// Central `herdr`-style registry managing local and remote SSH / `tmux -CC` / federated sessions (`v1.4.0`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetRegistry {
    pub nodes: BTreeMap<String, FleetNode>,
    pub profiles: BTreeMap<String, RemoteHostProfile>,
}

impl FleetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a named remote host profile (`:remote add <name> <user@host> [@tag1,@tag2]`).
    pub fn register_profile(&mut self, profile: RemoteHostProfile) {
        let name = profile.name.clone();
        let node = FleetNode {
            name: name.clone(),
            kind: FleetNodeKind::HostProfile {
                destination: profile.destination.clone(),
                rpc_endpoint: profile.rpc_endpoint.clone(),
            },
            tags: profile.tags.clone(),
            connected: profile.rpc_endpoint.is_some(),
            last_dispatched_command: None,
        };
        self.profiles.insert(name.clone(), profile);
        self.nodes.insert(name, node);
    }

    /// Registers an active SSH-backed `CompositePane` in the central fleet registry.
    pub fn register_ssh_pane(
        &mut self,
        name: impl Into<String>,
        pane_id: PaneId,
        destination: impl Into<String>,
        tags: Vec<String>,
    ) {
        let name_str = name.into();
        let dest_str = destination.into();
        self.nodes.insert(
            name_str.clone(),
            FleetNode {
                name: name_str,
                kind: FleetNodeKind::SshPane {
                    pane_id,
                    destination: dest_str,
                },
                tags,
                connected: true,
                last_dispatched_command: None,
            },
        );
    }

    /// Registers a remote `tmux -CC` over SSH session and its hydrated `CompositePane` IDs.
    pub fn register_ssh_tmux(
        &mut self,
        name: impl Into<String>,
        destination: impl Into<String>,
        session_name: impl Into<String>,
        pane_ids: Vec<PaneId>,
        tags: Vec<String>,
    ) {
        let name_str = name.into();
        self.nodes.insert(
            name_str.clone(),
            FleetNode {
                name: name_str,
                kind: FleetNodeKind::SshTmux {
                    destination: destination.into(),
                    session_name: session_name.into(),
                    pane_ids,
                },
                tags,
                connected: true,
                last_dispatched_command: None,
            },
        );
    }

    /// Disconnects and removes a remote node by name.
    pub fn disconnect(&mut self, name: &str) -> bool {
        self.nodes.remove(name).is_some()
    }

    /// Resolves which `FleetNode` names match `selector`.
    pub fn matching_nodes(&self, selector: &FleetSelector) -> Vec<String> {
        self.nodes
            .values()
            .filter(|node| match selector {
                FleetSelector::All | FleetSelector::RemoteOnly => true,
                FleetSelector::LocalOnly => false,
                FleetSelector::Tag(tag) => node
                    .tags
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(tag)),
                FleetSelector::Node(target) => {
                    node.name.eq_ignore_ascii_case(target)
                        || match &node.kind {
                            FleetNodeKind::SshPane { destination, .. }
                            | FleetNodeKind::SshTmux { destination, .. }
                            | FleetNodeKind::HostProfile { destination, .. } => {
                                destination.eq_ignore_ascii_case(target)
                            }
                        }
                }
            })
            .map(|n| n.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_destination_rejects_injection() {
        for bad in &[
            "",
            "-oProxyCommand=touch /tmp/pwn",
            "-p",
            "host\nProxyCommand",
            "host name",
            "host\u{0007}",
            "host\u{0000}",
        ] {
            assert!(
                validate_destination(bad).is_err(),
                "expected destination {:?} to be rejected",
                bad
            );
        }

        for good in &[
            "ubuntu@10.0.0.42",
            "gpu-node",
            "user@host.example.com",
            "[::1]",
        ] {
            assert!(
                validate_destination(good).is_ok(),
                "expected destination {:?} to be accepted",
                good
            );
        }
    }

    #[test]
    fn builders_reject_dash_destination() {
        let prof = RemoteHostProfile::new("evil", "-oProxyCommand=evil");
        assert!(prof.build_ssh_args(None).is_err());
        assert!(prof.build_ssh_tmux_cc_args("sess", None).is_err());
    }

    #[test]
    fn ssh_args_place_double_dash_before_destination() {
        let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
        let args = prof.build_ssh_args(None).unwrap();
        let len = args.len();
        assert!(len >= 2);
        assert_eq!(args[len - 2], "--");
        assert_eq!(args[len - 1], "ubuntu@10.0.0.42");
    }

    #[test]
    fn tmux_args_place_double_dash_before_destination() {
        let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
        let args = prof.build_ssh_tmux_cc_args("main", None).unwrap();
        let len = args.len();
        assert!(len >= 3);
        assert_eq!(args[len - 3], "--");
        assert_eq!(args[len - 2], "ubuntu@10.0.0.42");
        assert_eq!(args[len - 1], "tmux -CC new-session -A -s main");
    }

    #[test]
    fn control_path_uses_private_dir() {
        let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
        let dir = Path::new("/run/user/1000/bilterm");
        let args = prof.build_ssh_args(Some(dir)).unwrap();
        assert!(args.contains(&format!("ControlPath={}/ssh-%C", dir.display())));

        let no_dir_args = prof.build_ssh_args(None).unwrap();
        assert!(!no_dir_args.iter().any(|a| a.contains("Control")));
    }

    #[test]
    fn control_path_skipped_for_unsafe_dir() {
        let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
        // Contains space
        let bad_space = Path::new("/run/user/1000/bad dir");
        let args = prof.build_ssh_args(Some(bad_space)).unwrap();
        assert!(!args.iter().any(|a| a.contains("Control")));

        // Contains %
        let bad_percent = Path::new("/run/user/1000/bad%dir");
        let args = prof.build_ssh_args(Some(bad_percent)).unwrap();
        assert!(!args.iter().any(|a| a.contains("Control")));

        // Longer than 40 bytes
        let long_path_str = "/run/user/1000/this_path_is_way_too_long_for_sun_path_limits";
        assert!(long_path_str.len() > 40);
        let bad_long = Path::new(long_path_str);
        let args = prof.build_ssh_args(Some(bad_long)).unwrap();
        assert!(!args.iter().any(|a| a.contains("Control")));
    }
}
