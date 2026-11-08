//! Multi-session and multi-window clipboard & yank-ring registry (`CLIP-02`, Phase 57-01).

use std::time::{SystemTime, UNIX_EPOCH};

use crate::pane::redact::{Exclusion, classify as classify_secret};
use crate::wm::PaneId;

/// Maximum entries retained in the workspace-wide [`ClipboardRegistry`].
pub const MAX_CLIPBOARD_ENTRIES: usize = 200;

pub use crate::clipboard::ClipboardSource;

/// MIME classification for a [`ClipboardEntry`] (`CLIP-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardMime {
    TextPlain,
    MediaRef { mime: String, bytes_len: usize },
}

impl ClipboardMime {
    pub fn label(&self) -> &str {
        match self {
            Self::TextPlain => "text/plain",
            Self::MediaRef { mime, .. } => mime.as_str(),
        }
    }

    pub fn is_media(&self) -> bool {
        matches!(self, Self::MediaRef { .. })
    }
}

/// Filter tab scope in the `CLIPBOARD` utility window (`CLIP-02`, `CLIP-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ClipboardFilterScope {
    /// Show all entries across all sessions and windows.
    #[default]
    All,
    /// Show entries belonging to the selected/active session.
    Session,
    /// Show entries belonging to the selected/active window (pane).
    Window,
    /// Show only pinned entries (`pinned == true`).
    Pinned,
    /// Show only image/video media passthrough entries (`ClipboardMime::MediaRef`).
    Media,
}

impl ClipboardFilterScope {
    pub const ALL_TABS: [Self; 5] = [
        Self::All,
        Self::Session,
        Self::Window,
        Self::Pinned,
        Self::Media,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Session => "Session",
            Self::Window => "Window",
            Self::Pinned => "Pinned",
            Self::Media => "Media",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "all" | "*" => Some(Self::All),
            "session" | "sess" => Some(Self::Session),
            "window" | "win" | "pane" => Some(Self::Window),
            "pinned" | "pin" | "pins" => Some(Self::Pinned),
            "media" | "image" | "images" | "video" => Some(Self::Media),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Session,
            Self::Session => Self::Window,
            Self::Window => Self::Pinned,
            Self::Pinned => Self::Media,
            Self::Media => Self::All,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::All => Self::Media,
            Self::Session => Self::All,
            Self::Window => Self::Session,
            Self::Pinned => Self::Window,
            Self::Media => Self::Pinned,
        }
    }
}

/// Single entry stored in the [`ClipboardRegistry`] (`CLIP-02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: u64,
    pub session_name: String,
    pub pane_id: Option<PaneId>,
    pub window_title: String,
    pub source: ClipboardSource,
    pub mime: ClipboardMime,
    pub content: String,
    pub char_count: usize,
    pub line_count: usize,
    pub timestamp_epoch_secs: u64,
    pub pinned: bool,
    pub redacted: bool,
}

impl ClipboardEntry {
    /// Returns a single-line snippet suitable for list display, masking secrets when `redacted`.
    pub fn masked_snippet(&self, max_chars: usize) -> String {
        if self.redacted {
            let preview = format!(
                "••• [REDACTED SECRET · {} chars, {} lines]",
                self.char_count, self.line_count
            );
            return preview.chars().take(max_chars).collect();
        }
        let flat = self.content.replace('\r', "").replace('\n', " ↵ ");
        let mut out: String = flat.chars().take(max_chars).collect();
        if flat.chars().count() > max_chars && max_chars > 1 {
            out.pop();
            out.push('…');
        }
        out
    }

    /// Returns multi-line preview lines for the bottom detail pane, masking secrets when `redacted`.
    pub fn masked_preview_lines(&self, max_lines: usize, max_cols: usize) -> Vec<String> {
        if self.redacted {
            return vec![
                format!(
                    "••• [REDACTED SECRET · {} chars, {} lines]",
                    self.char_count, self.line_count
                ),
                "Secret material is masked in UI/RPC and excluded from disk persistence.".to_string(),
            ];
        }
        let mut lines = Vec::new();
        for raw_line in self.content.lines().take(max_lines.max(1)) {
            let clean = raw_line.replace('\r', "");
            let clipped: String = clean.chars().take(max_cols.max(1)).collect();
            lines.push(clipped);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }
}

/// Workspace-wide bounded clipboard & yank history registry (`CLIP-02`).
#[derive(Clone, Debug)]
pub struct ClipboardRegistry {
    entries: Vec<ClipboardEntry>,
    next_id: u64,
    max_entries: usize,
}

impl Default for ClipboardRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            max_entries: MAX_CLIPBOARD_ENTRIES,
        }
    }

    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get_by_id(&self, id: u64) -> Option<&ClipboardEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Records a text clipboard entry (newest at index 0), deduplicating identical `(session_name, pane_id, content)`
    /// while preserving `pinned` status and classifying secrets via [`crate::pane::redact::classify`].
    pub fn record_text(
        &mut self,
        session_name: impl Into<String>,
        pane_id: Option<PaneId>,
        window_title: impl Into<String>,
        source: ClipboardSource,
        content: impl Into<String>,
    ) -> u64 {
        let content = content.into();
        if content.is_empty() {
            return 0;
        }
        let session_name = session_name.into();
        let window_title = window_title.into();
        let redacted = matches!(classify_secret(&content), Some(Exclusion::Secret));
        let char_count = content.chars().count();
        let line_count = content.lines().count().max(1);
        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut preserved_pin = false;
        let mut reused_id = None;
        if let Some(pos) = self.entries.iter().position(|e| {
            !e.mime.is_media()
                && e.session_name == session_name
                && e.pane_id == pane_id
                && e.content == content
        }) {
            let prev = self.entries.remove(pos);
            preserved_pin = prev.pinned;
            reused_id = Some(prev.id);
        }

        let id = reused_id.unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            id
        });

        self.entries.insert(
            0,
            ClipboardEntry {
                id,
                session_name,
                pane_id,
                window_title,
                source,
                mime: ClipboardMime::TextPlain,
                content,
                char_count,
                line_count,
                timestamp_epoch_secs,
                pinned: preserved_pin,
                redacted,
            },
        );
        self.evict_over_capacity();
        id
    }

    /// Records a media passthrough event (`Ctrl+V` when `image/*` or `video/*` is on the system clipboard).
    pub fn record_media(
        &mut self,
        session_name: impl Into<String>,
        pane_id: Option<PaneId>,
        window_title: impl Into<String>,
        mime: impl Into<String>,
        bytes_len: usize,
    ) -> u64 {
        let mime_str = mime.into();
        let session_name = session_name.into();
        let window_title = window_title.into();
        let summary = if bytes_len > 0 {
            format!("[Media passthrough: {mime_str} ({bytes_len} bytes) -> raw Ctrl+V (0x16)]")
        } else {
            format!("[Media passthrough: {mime_str} -> raw Ctrl+V (0x16)]")
        };
        let char_count = summary.chars().count();
        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.entries.insert(
            0,
            ClipboardEntry {
                id,
                session_name,
                pane_id,
                window_title,
                source: ClipboardSource::MediaPassthrough,
                mime: ClipboardMime::MediaRef {
                    mime: mime_str,
                    bytes_len,
                },
                content: summary,
                char_count,
                line_count: 1,
                timestamp_epoch_secs,
                pinned: false,
                redacted: false,
            },
        );
        self.evict_over_capacity();
        id
    }

    fn evict_over_capacity(&mut self) {
        while self.entries.len() > self.max_entries {
            if let Some(pos) = self.entries.iter().rposition(|e| !e.pinned) {
                self.entries.remove(pos);
            } else {
                self.entries.pop();
            }
        }
    }

    pub fn toggle_pin(&mut self, id: u64) -> Option<bool> {
        let entry = self.entries.iter_mut().find(|e| e.id == id)?;
        entry.pinned = !entry.pinned;
        Some(entry.pinned)
    }

    pub fn delete_entry(&mut self, id: u64) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Clears unpinned entries matching `scope` (pinned entries survive `:clipboard clear`).
    pub fn clear_unpinned(
        &mut self,
        scope: ClipboardFilterScope,
        active_session: &str,
        active_pane_id: PaneId,
    ) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| {
            if e.pinned {
                return true;
            }
            match scope {
                ClipboardFilterScope::All => false,
                ClipboardFilterScope::Session => e.session_name != active_session,
                ClipboardFilterScope::Window => e.pane_id != Some(active_pane_id),
                ClipboardFilterScope::Pinned => true,
                ClipboardFilterScope::Media => !e.mime.is_media(),
            }
        });
        before.saturating_sub(self.entries.len())
    }

    /// Filters entries by `scope`, optional `session_filter`, optional `pane_filter`, and case-insensitive `search_query`.
    pub fn filtered_entries(
        &self,
        scope: ClipboardFilterScope,
        session_filter: Option<&str>,
        pane_filter: Option<PaneId>,
        active_session: &str,
        active_pane_id: PaneId,
        search_query: &str,
    ) -> Vec<&ClipboardEntry> {
        let q = search_query.trim().to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                match scope {
                    ClipboardFilterScope::All => {}
                    ClipboardFilterScope::Session => {
                        let target_sess = session_filter.unwrap_or(active_session);
                        if e.session_name != target_sess {
                            return false;
                        }
                    }
                    ClipboardFilterScope::Window => {
                        let target_pane = pane_filter.unwrap_or(active_pane_id);
                        if e.pane_id != Some(target_pane) {
                            return false;
                        }
                    }
                    ClipboardFilterScope::Pinned => {
                        if !e.pinned {
                            return false;
                        }
                    }
                    ClipboardFilterScope::Media => {
                        if !e.mime.is_media() {
                            return false;
                        }
                    }
                }
                if let Some(sess) = session_filter
                    && scope != ClipboardFilterScope::Session
                    && e.session_name != sess
                {
                    return false;
                }
                if let Some(pid) = pane_filter
                    && scope != ClipboardFilterScope::Window
                    && e.pane_id != Some(pid)
                {
                    return false;
                }
                if !q.is_empty() {
                    let hay_content = if e.redacted {
                        String::new()
                    } else {
                        e.content.to_ascii_lowercase()
                    };
                    let hay_sess = e.session_name.to_ascii_lowercase();
                    let hay_title = e.window_title.to_ascii_lowercase();
                    let hay_src = e.source.badge().to_ascii_lowercase();
                    let hay_mime = e.mime.label().to_ascii_lowercase();
                    if !hay_content.contains(&q)
                        && !hay_sess.contains(&q)
                        && !hay_title.contains(&q)
                        && !hay_src.contains(&q)
                        && !hay_mime.contains(&q)
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    pub fn distinct_sessions(&self) -> Vec<String> {
        let mut out = Vec::new();
        for e in &self.entries {
            if !out.contains(&e.session_name) {
                out.push(e.session_name.clone());
            }
        }
        out
    }

    pub fn distinct_windows(&self) -> Vec<(PaneId, String)> {
        let mut out = Vec::new();
        for e in &self.entries {
            if let Some(pid) = e.pane_id
                && !out.iter().any(|(id, _)| *id == pid)
            {
                out.push((pid, e.window_title.clone()));
            }
        }
        out
    }
}
