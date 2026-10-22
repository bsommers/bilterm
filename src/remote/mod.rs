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

use serde::{Deserialize, Serialize};

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

    /// Builds the `ssh` CLI arguments for spawning an interactive remote shell pane.
    pub fn build_ssh_args(&self) -> Vec<String> {
        let mut args = vec!["-tt".to_string()];
        if self.control_master {
            args.push("-o".to_string());
            args.push("ControlMaster=auto".to_string());
            args.push("-o".to_string());
            args.push("ControlPersist=60s".to_string());
            args.push("-o".to_string());
            args.push("ControlPath=/tmp/bilterm-ssh-%r@%h:%p".to_string());
        }
        if let Some(port) = self.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }
        if let Some(id_file) = &self.identity_file {
            args.push("-i".to_string());
            args.push(id_file.clone());
        }
        args.push(self.destination.clone());
        args
    }

    /// Builds the `ssh` CLI arguments for attaching to a persistent remote `tmux -CC` Control Mode session over SSH.
    pub fn build_ssh_tmux_cc_args(&self, session_name: &str) -> Vec<String> {
        let mut args = vec!["-T".to_string()];
        if self.control_master {
            args.push("-o".to_string());
            args.push("ControlMaster=auto".to_string());
            args.push("-o".to_string());
            args.push("ControlPersist=60s".to_string());
            args.push("-o".to_string());
            args.push("ControlPath=/tmp/bilterm-ssh-%r@%h:%p".to_string());
        }
        if let Some(port) = self.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }
        if let Some(id_file) = &self.identity_file {
            args.push("-i".to_string());
            args.push(id_file.clone());
        }
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
        args
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
