//! Per-web-client view state isolation (`WEB-FE-03`, `ARCH-R02`, Phase 60).
//!
//! Every WebSocket connection gets its own [`ClientViewState`]: viewport size, focused pane and
//! per-pane scroll offsets. Updating it never touches the host `WindowManager`, so a browser tab
//! focusing or scrolling a pane cannot move the local TUI/GUI operator's focus, nor another tab's.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::pane::PaneId;

/// Most clients tracked at once; registration beyond this is refused (bounded memory).
pub const MAX_WEB_CLIENTS: usize = 64;
/// Most per-pane scroll offsets one client may hold.
pub const MAX_SCROLL_ENTRIES: usize = 1024;

/// View state owned by one web client.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientViewState {
    pub client_id: String,
    pub viewport_cols: u16,
    pub viewport_rows: u16,
    pub focused_pane_id: Option<PaneId>,
    pub scroll_offsets: HashMap<PaneId, usize>,
    pub active_modal: Option<String>,
    pub last_seen_epoch_ms: u64,
    /// Last `agent:resize` requested per pane by this client (viewport only; the host PTY is untouched).
    pub pane_sizes: HashMap<PaneId, (u16, u16)>,
}

/// Thread-safe registry of [`ClientViewState`]s, cheap to clone (shared `Arc`).
#[derive(Clone, Debug, Default)]
pub struct ClientViewRegistry {
    inner: Arc<RwLock<HashMap<String, ClientViewState>>>,
    next_id: Arc<AtomicU64>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl ClientViewRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a fresh connection id (`web-1`, `web-2`, ...).
    pub fn allocate_id(&self) -> String {
        format!("web-{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Registers `client_id` with a default view; `false` when the registry is full.
    pub fn register(&self, client_id: &str) -> bool {
        let Ok(mut map) = self.inner.write() else {
            return false;
        };
        if !map.contains_key(client_id) && map.len() >= MAX_WEB_CLIENTS {
            return false;
        }
        map.entry(client_id.to_string()).or_insert_with(|| ClientViewState {
            client_id: client_id.to_string(),
            last_seen_epoch_ms: now_ms(),
            ..ClientViewState::default()
        });
        true
    }

    pub fn unregister(&self, client_id: &str) {
        if let Ok(mut map) = self.inner.write() {
            map.remove(client_id);
        }
    }

    fn with_client(&self, client_id: &str, f: impl FnOnce(&mut ClientViewState)) -> bool {
        let Ok(mut map) = self.inner.write() else {
            return false;
        };
        match map.get_mut(client_id) {
            Some(st) => {
                f(st);
                st.last_seen_epoch_ms = now_ms();
                true
            }
            None => false,
        }
    }

    pub fn update_viewport(&self, client_id: &str, cols: u16, rows: u16) -> bool {
        self.with_client(client_id, |s| {
            s.viewport_cols = cols;
            s.viewport_rows = rows;
        })
    }

    /// Sets this client's focused pane only; the host `WindowManager` focus is never changed.
    pub fn set_focus(&self, client_id: &str, pane_id: Option<PaneId>) -> bool {
        self.with_client(client_id, |s| s.focused_pane_id = pane_id)
    }

    pub fn set_scroll(&self, client_id: &str, pane_id: PaneId, offset: usize) -> bool {
        self.with_client(client_id, |s| {
            if s.scroll_offsets.len() < MAX_SCROLL_ENTRIES || s.scroll_offsets.contains_key(&pane_id) {
                s.scroll_offsets.insert(pane_id, offset);
            }
        })
    }

    pub fn set_pane_size(&self, client_id: &str, pane_id: PaneId, cols: u16, rows: u16) -> bool {
        self.with_client(client_id, |s| {
            if s.pane_sizes.len() < MAX_SCROLL_ENTRIES || s.pane_sizes.contains_key(&pane_id) {
                s.pane_sizes.insert(pane_id, (cols, rows));
            }
        })
    }

    pub fn set_active_modal(&self, client_id: &str, modal: Option<String>) -> bool {
        self.with_client(client_id, |s| s.active_modal = modal)
    }

    pub fn get(&self, client_id: &str) -> Option<ClientViewState> {
        self.inner.read().ok()?.get(client_id).cloned()
    }

    pub fn len(&self) -> usize {
        self.inner.read().map_or(0, |m| m.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Sorted ids of all registered clients.
    pub fn client_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .inner
            .read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        ids.sort();
        ids
    }
}
