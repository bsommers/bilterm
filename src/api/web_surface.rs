//! Shared web-surface runtime state (`WEB-FE-01..04`, `UNI-R01`, Phase 60).
//!
//! One [`WebSurfaceState`] (cheap `Arc` clone) is held by the `WindowManager` and by the HTTP/WS
//! server, so `:web status`, `web.status` RPC, the `bilterm_web_surface` MCP tool and the live
//! server all read the same facts: enabled, port, url, connected clients, web dir, CORS origin.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use super::client_view::ClientViewRegistry;
use super::json::JsonValue;

/// Default port the web UI is documented on (`agent-manager-fe` connects to `ws://localhost:8088/ws`).
pub const DEFAULT_WEB_PORT: u16 = 8088;

#[derive(Debug)]
struct Shared {
    enabled: AtomicBool,
    stop_requested: AtomicBool,
    port: AtomicU16,
    web_dir: Mutex<Option<PathBuf>>,
    cors_origin: Mutex<Option<String>>,
}

/// Handle on the process's web surface; clones share state.
#[derive(Clone, Debug)]
pub struct WebSurfaceState {
    shared: Arc<Shared>,
    clients: ClientViewRegistry,
}

impl Default for WebSurfaceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Point-in-time copy of [`WebSurfaceState`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSurfaceStatus {
    pub enabled: bool,
    pub port: u16,
    pub url: String,
    pub active_clients: usize,
    pub web_dir: Option<String>,
    pub cors_origin: Option<String>,
}

impl WebSurfaceStatus {
    pub fn to_json(&self) -> JsonValue {
        let opt = |v: &Option<String>| v.clone().map_or(JsonValue::Null, JsonValue::String);
        JsonValue::obj([
            ("enabled", JsonValue::Bool(self.enabled)),
            ("port", JsonValue::Number(i64::from(self.port))),
            ("url", JsonValue::String(self.url.clone())),
            ("active_clients", JsonValue::Number(self.active_clients as i64)),
            ("web_dir", opt(&self.web_dir)),
            ("cors_origin", opt(&self.cors_origin)),
        ])
    }

    /// One-line human summary for the status bar.
    pub fn summary(&self) -> String {
        if self.enabled {
            format!(
                "Web UI: listening on {} ({} client{})",
                self.url,
                self.active_clients,
                if self.active_clients == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Web UI: not running (port {}; start with `bilterm --headless --http 127.0.0.1:{}`)",
                self.port, self.port
            )
        }
    }
}

impl WebSurfaceState {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                enabled: AtomicBool::new(false),
                stop_requested: AtomicBool::new(false),
                port: AtomicU16::new(DEFAULT_WEB_PORT),
                web_dir: Mutex::new(None),
                cors_origin: Mutex::new(None),
            }),
            clients: ClientViewRegistry::new(),
        }
    }

    /// Per-connection client view registry (`WEB-FE-03`).
    pub fn clients(&self) -> &ClientViewRegistry {
        &self.clients
    }

    /// Marks the server as listening on `port` and clears any pending stop request.
    pub fn mark_listening(&self, port: u16) {
        self.shared.port.store(port, Ordering::SeqCst);
        self.shared.stop_requested.store(false, Ordering::SeqCst);
        self.shared.enabled.store(true, Ordering::SeqCst);
    }

    pub fn mark_stopped(&self) {
        self.shared.enabled.store(false, Ordering::SeqCst);
    }

    /// Asks a running server to stop accepting connections (`:web stop`).
    pub fn request_stop(&self) {
        self.shared.stop_requested.store(true, Ordering::SeqCst);
        self.shared.enabled.store(false, Ordering::SeqCst);
    }

    pub fn is_stop_requested(&self) -> bool {
        self.shared.stop_requested.load(Ordering::SeqCst)
    }

    /// Sets the port used by the next start (`:web port <N>`); `Err` for port 0.
    pub fn set_port(&self, port: u16) -> Result<(), String> {
        if port == 0 {
            return Err("web port must be 1..=65535".to_string());
        }
        if self.is_enabled() {
            return Err(format!(
                "web UI is running on port {}; `:web stop` first",
                self.port()
            ));
        }
        self.shared.port.store(port, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.shared.enabled.load(Ordering::SeqCst)
    }

    pub fn port(&self) -> u16 {
        self.shared.port.load(Ordering::SeqCst)
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port())
    }

    pub fn set_web_dir(&self, dir: Option<PathBuf>) {
        if let Ok(mut g) = self.shared.web_dir.lock() {
            *g = dir;
        }
    }

    pub fn web_dir(&self) -> Option<PathBuf> {
        self.shared.web_dir.lock().ok().and_then(|g| g.clone())
    }

    pub fn set_cors_origin(&self, origin: Option<String>) {
        if let Ok(mut g) = self.shared.cors_origin.lock() {
            *g = origin;
        }
    }

    pub fn cors_origin(&self) -> Option<String> {
        self.shared.cors_origin.lock().ok().and_then(|g| g.clone())
    }

    pub fn status(&self) -> WebSurfaceStatus {
        WebSurfaceStatus {
            enabled: self.is_enabled(),
            port: self.port(),
            url: self.url(),
            active_clients: self.clients.len(),
            web_dir: self.web_dir().map(|p| p.to_string_lossy().into_owned()),
            cors_origin: self.cors_origin(),
        }
    }
}
