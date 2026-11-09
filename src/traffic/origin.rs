//! Typed origin taxonomy, locality and label sanitising (std only).

use crate::traffic::LABEL_CAP;

/// Where a channel's bytes come from or go to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    /// Shell child on a local PTY.
    LocalPty,
    /// Pane running `ssh` inside a PTY.
    Ssh { host: String, port: u16 },
    /// Phase 51.1 `ssh -tt ... tmux -CC` link (wire) and its decoded panes (payload).
    TmuxLink { dest: String, session: String },
    /// Legacy local `tmux -CC` subprocess.
    TmuxLocal,
    /// JSON-RPC unix socket.
    RpcUnix,
    /// JSON-RPC TCP socket; `remote` when the peer is non-loopback.
    RpcTcp { remote: bool },
    /// Loopback HTTP request/response traffic.
    Http,
    /// Server-sent events stream.
    Sse,
    /// WebSocket web client (`web-3`).
    Ws { client: String },
    /// MCP stdio.
    Mcp,
    /// The terminal Bilterm itself runs in.
    HostTty,
    /// OS clipboard helper process.
    ClipboardOs,
    /// Speech-to-text provider connection.
    Stt { host: String },
}

/// How far traffic travels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Locality {
    Local,
    Loopback,
    Network,
}

impl Locality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Loopback => "loopback",
            Self::Network => "network",
        }
    }
}

/// Double-count guard: which views a channel contributes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    /// Connection / origin / total views only.
    Wire,
    /// Session / window / pane views only.
    Payload,
    /// Both families (e.g. a local PTY pane).
    Both,
}

impl Role {
    pub fn is_wire(self) -> bool {
        matches!(self, Self::Wire | Self::Both)
    }
    pub fn is_payload(self) -> bool {
        matches!(self, Self::Payload | Self::Both)
    }
}

/// Coarse boundary family derived from an [`Origin`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Surface {
    Pty,
    Control,
    Clipboard,
    HostTty,
}

/// Opaque id allocated per distinct [`ConnectionKey`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u32);

/// Normalised, bounded string form of an [`Origin`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionKey(pub String);

impl ConnectionKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    h == "localhost" || h.starts_with("127.") || h == "::1" || h == "[::1]"
}

impl Origin {
    pub fn locality(&self) -> Locality {
        match self {
            Self::LocalPty
            | Self::HostTty
            | Self::RpcUnix
            | Self::Mcp
            | Self::ClipboardOs
            | Self::TmuxLocal => Locality::Local,
            Self::Http | Self::Sse | Self::Ws { .. } | Self::RpcTcp { remote: false } => {
                Locality::Loopback
            }
            Self::Ssh { .. } | Self::TmuxLink { .. } | Self::RpcTcp { remote: true } => {
                Locality::Network
            }
            Self::Stt { host } => {
                if is_loopback_host(host) {
                    Locality::Loopback
                } else {
                    Locality::Network
                }
            }
        }
    }

    pub fn surface(&self) -> Surface {
        match self {
            Self::LocalPty | Self::Ssh { .. } | Self::TmuxLink { .. } | Self::TmuxLocal => {
                Surface::Pty
            }
            Self::ClipboardOs => Surface::Clipboard,
            Self::HostTty => Surface::HostTty,
            _ => Surface::Control,
        }
    }

    pub fn class_label(&self) -> &'static str {
        match self {
            Self::LocalPty => "local",
            Self::Ssh { .. } => "ssh",
            Self::TmuxLink { .. } => "tmux-link",
            Self::TmuxLocal => "tmux",
            Self::RpcUnix | Self::RpcTcp { .. } => "rpc",
            Self::Http | Self::Sse => "http",
            Self::Ws { .. } => "ws",
            Self::Mcp => "mcp",
            Self::HostTty => "tty",
            Self::ClipboardOs => "clip",
            Self::Stt { .. } => "stt",
        }
    }

    /// Normalised connection key: all local PTYs share `local`, each ssh `host:port` is one key.
    pub fn connection_key(&self) -> ConnectionKey {
        let s = match self {
            Self::LocalPty => "local".to_string(),
            Self::Ssh { host, port } => format!("ssh:{}:{port}", label::sanitize(host)),
            Self::TmuxLink { dest, session } => {
                format!("tmux-link:{}/{}", label::sanitize(dest), label::sanitize(session))
            }
            Self::TmuxLocal => "tmux-local".to_string(),
            Self::RpcUnix => "rpc-unix".to_string(),
            Self::RpcTcp { remote } => {
                if *remote { "rpc-tcp:remote" } else { "rpc-tcp" }.to_string()
            }
            Self::Http => "http".to_string(),
            Self::Sse => "sse".to_string(),
            Self::Ws { client } => format!("ws:{}", label::sanitize(client)),
            Self::Mcp => "mcp".to_string(),
            Self::HostTty => "tty".to_string(),
            Self::ClipboardOs => "clip".to_string(),
            Self::Stt { host } => format!("stt:{}", label::sanitize(host)),
        };
        ConnectionKey(s)
    }
}

/// Label sanitising: printable ASCII only, capped at [`LABEL_CAP`].
pub mod label {
    use super::LABEL_CAP;

    /// Strips control and non-ASCII characters and caps the result at [`LABEL_CAP`] chars.
    pub fn sanitize(s: &str) -> String {
        s.chars()
            .filter(|c| (' '..='~').contains(c))
            .take(LABEL_CAP)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locality_map() {
        use Locality::*;
        for o in [
            Origin::LocalPty,
            Origin::HostTty,
            Origin::RpcUnix,
            Origin::Mcp,
            Origin::ClipboardOs,
            Origin::TmuxLocal,
        ] {
            assert_eq!(o.locality(), Local);
        }
        for o in [
            Origin::Http,
            Origin::Sse,
            Origin::Ws { client: "web-1".into() },
            Origin::RpcTcp { remote: false },
            Origin::Stt { host: "localhost".into() },
            Origin::Stt { host: "127.0.0.1".into() },
        ] {
            assert_eq!(o.locality(), Loopback, "{o:?}");
        }
        for o in [
            Origin::Ssh { host: "h".into(), port: 22 },
            Origin::TmuxLink { dest: "d".into(), session: "s".into() },
            Origin::RpcTcp { remote: true },
            Origin::Stt { host: "api.example.com".into() },
        ] {
            assert_eq!(o.locality(), Network, "{o:?}");
        }
    }

    #[test]
    fn keys_normalised() {
        assert_eq!(Origin::LocalPty.connection_key().as_str(), "local");
        let k = Origin::Ssh { host: "prod-db".into(), port: 22 }.connection_key();
        assert_eq!(k.as_str(), "ssh:prod-db:22");
        let k = Origin::TmuxLink { dest: "bastion".into(), session: "work".into() };
        assert_eq!(k.connection_key().as_str(), "tmux-link:bastion/work");
    }

    #[test]
    fn sanitize_strips_and_caps() {
        let s = label::sanitize(&"a\x1b[31mb\nc".repeat(40));
        assert!(!s.contains('\x1b') && !s.contains('\n'));
        assert_eq!(s.chars().count(), LABEL_CAP);
    }
}
