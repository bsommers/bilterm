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
    /// Enables OpenSSH `ControlMaster=auto` and `ControlPersist=10s` for instant multi-pane SSH multiplexing.
    #[serde(default = "default_true")]
    pub control_master: bool,
    /// Optional POSIX `OSC 133` prompt-marker bootstrap over SSH (disabled by default to avoid pre-login echo).
    #[serde(default)]
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

/// Extracts `(destination, port)` from `ssh` argv: the first token after `--` (validated) and
/// the value of `-p N` / `-pN` (Phase 64 traffic attribution; never used to build commands).
pub fn ssh_destination_from_args(args: &[String]) -> Option<(String, Option<u16>)> {
    let sep = args.iter().position(|a| a == "--")?;
    let dest = args.get(sep + 1)?;
    validate_destination(dest).ok()?;
    let mut port = None;
    let mut i = 0;
    while i < sep {
        let a = args[i].as_str();
        if a == "-p" {
            port = args.get(i + 1).and_then(|v| v.parse::<u16>().ok());
            i += 1;
        } else if let Some(v) = a.strip_prefix("-p") {
            port = v.parse::<u16>().ok().or(port);
        }
        i += 1;
    }
    Some((dest.clone(), port))
}

impl RemoteHostProfile {
    pub fn new(name: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            destination: destination.into(),
            port: None,
            identity_file: None,
            control_master: true,
            remote_osc133_bootstrap: false,
            tags: Vec::new(),
            rpc_endpoint: None,
        }
    }

    fn push_common_args(&self, args: &mut Vec<String>, control_dir: Option<&Path>, is_link: bool) {
        push_keepalive_args(args);
        if is_link {
            args.push("-o".to_string());
            args.push("ControlMaster=no".to_string());
            args.push("-o".to_string());
            args.push("ControlPath=none".to_string());
        } else if self.control_master
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
                args.push("ControlPersist=10s".to_string());
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
        self.push_common_args(&mut args, control_dir, false);
        args.push("--".to_string());
        args.push(self.destination.clone());
        Ok(args)
    }

    /// Builds the `ssh` CLI arguments for attaching to a persistent remote `tmux -CC` Control Mode session over SSH.
    ///
    /// Validates destination against injection attacks, pushes `--` before destination,
    /// and wraps the remote `tmux -CC` command in a login shell (`$SHELL -lc`) so non-interactive
    /// `sshd` sessions discover Homebrew/MacPorts installations of tmux on macOS (csh/tcsh logins
    /// are unsupported).
    pub fn build_ssh_tmux_cc_args(
        &self,
        session_name: &str,
        control_dir: Option<&Path>,
    ) -> Result<Vec<String>> {
        validate_destination(&self.destination)?;
        let mut args = vec!["-tt".to_string()];
        self.push_common_args(&mut args, control_dir, true);
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
        args.push(format!(
            "exec $SHELL -lc 'exec env PATH=\"$PATH:/opt/homebrew/bin:/usr/local/bin:/opt/local/bin\" tmux -CC new-session -A -s {sess}'"
        ));
        Ok(args)
    }
}

fn push_keepalive_args(args: &mut Vec<String>) {
    args.push("-o".to_string());
    args.push("ServerAliveInterval=15".to_string());
    args.push("-o".to_string());
    args.push("ServerAliveCountMax=3".to_string());
    args.push("-o".to_string());
    args.push("TCPKeepAlive=yes".to_string());
    args.push("-o".to_string());
    args.push("ConnectTimeout=15".to_string());
}

/// Redacts sensitive parameters (identity files, ControlPath sockets) from an SSH argv for safe logging.
///
/// This is the only sanctioned way to log an SSH argv in `bilterm` (see Phase 51.1 CONTRACT section 11).
pub fn redact_ssh_argv(args: &[String]) -> String {
    let mut redacted = Vec::with_capacity(args.len());
    let mut mask_next_as_key = false;
    for arg in args {
        if mask_next_as_key {
            redacted.push("<key>".to_string());
            mask_next_as_key = false;
            continue;
        }
        if arg == "-i" {
            redacted.push("-i".to_string());
            mask_next_as_key = true;
        } else if let Some((prefix, _)) = arg.split_once("ControlPath=") {
            redacted.push(format!("{prefix}ControlPath=<private>"));
        } else {
            redacted.push(arg.clone());
        }
    }
    redacted.join(" ")
}

/// Returns a one-line POSIX shell snippet that configures `OSC 133` prompt/command-exit markers
/// and `OSC 7` working directory reporting on a remote SSH host without modifying remote dotfiles.
pub fn remote_osc133_bootstrap_command() -> &'static str {
    " PROMPT_COMMAND='__bt_ec=$?; printf \"\\033]133;D;%d\\007\\033]7;file://%s%s\\007\\033]133;A\\007\" \"$__bt_ec\" \"$(hostname)\" \"$PWD\"'; printf '\\033]133;A\\007'\n"
}

/// Discovers non-wildcard `Host` aliases from `~/.ssh/config` for `:ssh` `<Tab>` completion.
pub fn discover_ssh_config_hosts() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return Vec::new();
    };
    let path = std::path::PathBuf::from(home).join(".ssh").join("config");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut hosts = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut toks = trimmed.split_whitespace();
        if let Some(kw) = toks.next()
            && kw.eq_ignore_ascii_case("host")
        {
            for alias in toks {
                if !alias.contains('*')
                    && !alias.contains('?')
                    && !alias.starts_with('!')
                    && validate_destination(alias).is_ok()
                    && !hosts.iter().any(|h| h == alias)
                {
                    hosts.push(alias.to_string());
                }
            }
        }
    }
    hosts
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

    /// Removes any fleet node references to a closed/reaped `pane_id`.
    pub fn remove_pane_references(&mut self, target_id: PaneId) {
        self.nodes.retain(|_, node| match &mut node.kind {
            FleetNodeKind::SshPane { pane_id, .. } => *pane_id != target_id,
            FleetNodeKind::SshTmux { pane_ids, .. } => {
                pane_ids.retain(|&pid| pid != target_id);
                !pane_ids.is_empty()
            }
            FleetNodeKind::HostProfile { .. } => true,
        });
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
        assert!(args[len - 1].contains("-s main"));
        // Phase 51.1: wraps in login shell with macOS Homebrew path
        assert!(args[len - 1].starts_with("exec $SHELL -lc '"));
        assert!(args[len - 1].ends_with("-s main'"));
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

    #[test]
    fn ssh_destination_from_args_parses_port_and_destination() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            ssh_destination_from_args(&v(&["-tt", "-p", "2222", "--", "bill@prod-db", "tmux"])),
            Some(("bill@prod-db".to_string(), Some(2222)))
        );
        assert_eq!(
            ssh_destination_from_args(&v(&["--", "host"])),
            Some(("host".to_string(), None))
        );
        assert_eq!(ssh_destination_from_args(&v(&["-tt", "host"])), None);
        assert_eq!(ssh_destination_from_args(&v(&["-tt", "--"])), None);
        assert_eq!(ssh_destination_from_args(&v(&["--", "-oProxyCommand=x"])), None);
    }
}
