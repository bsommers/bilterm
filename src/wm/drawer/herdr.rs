//! `herdr` drawer style: compact workspace > session > pane tree (`FLT-01`, Phase 50 D-02).
//!
//! The classic drawer (`classic_drawer_rows` in the parent module) is untouched. This module owns the
//! style dispatcher [`WindowManager::drawer_rows`], the herdr row builder and the per-style width
//! helpers on [`DrawerConfig`].

use serde::{Deserialize, Serialize};

use super::{DrawerConfig, DrawerRow, DrawerRowKind};
use crate::wm::WindowManager;

/// Visual style of the slide-out drawer (`drawer.style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrawerStyle {
    /// Telemetry-rich classic rows (default, byte-identical to pre-Phase-50 output).
    #[default]
    Classic,
    /// Compact one-line-per-item workspace > session > pane tree.
    Herdr,
}

impl DrawerStyle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Herdr => "herdr",
        }
    }
    pub const fn toggle(self) -> Self {
        match self {
            Self::Classic => Self::Herdr,
            Self::Herdr => Self::Classic,
        }
    }
}

/// Tree level of a herdr drawer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HerdrLevel {
    Workspace,
    Session,
    Pane,
    /// Quick terminal and fleet nodes.
    Other,
}

/// Herdr-only metadata attached to a [`DrawerRow`] (`None` for every classic row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HerdrRowMeta {
    pub depth: u8,
    pub level: HerdrLevel,
    pub collapsed: bool,
    pub has_children: bool,
    /// `true` for the active session and its live panes; `false` for saved sessions.
    pub live: bool,
    /// Pane count rollup (live count for the active session, stored count for saved ones).
    pub pane_count: usize,
}

impl DrawerConfig {
    /// Drawer width in columns for the active style (herdr keeps its own width).
    pub fn effective_width_cols(&self) -> u16 {
        match self.style {
            DrawerStyle::Herdr => self.herdr_width_cols,
            DrawerStyle::Classic => self.width_cols,
        }
    }

    /// Writes `cols` to the width field owned by the active style (never crosses styles).
    pub fn set_width_cols_for_style(&mut self, cols: u16) {
        match self.style {
            DrawerStyle::Herdr => self.herdr_width_cols = cols,
            DrawerStyle::Classic => self.width_cols = cols,
        }
    }
}

impl WindowManager {
    /// Builds the drawer rows for the configured [`DrawerStyle`]; the classic body lives in
    /// `classic_drawer_rows` (parent module) and is unchanged.
    pub fn drawer_rows(&self) -> Vec<DrawerRow> {
        match self.config.drawer.style {
            DrawerStyle::Classic => self.classic_drawer_rows(),
            DrawerStyle::Herdr => self.herdr_drawer_rows(),
        }
    }

    /// Herdr rows: the active workspace followed by one row per live pane.
    pub(crate) fn herdr_drawer_rows(&self) -> Vec<DrawerRow> {
        let mut rows = Vec::new();
        let ids = self.ordered_pane_ids();
        let name = self.config.history_session.clone();
        rows.push(herdr_row(
            DrawerRowKind::SessionHeader(name.clone()),
            name,
            false,
            HerdrRowMeta {
                depth: 0,
                level: HerdrLevel::Workspace,
                collapsed: false,
                has_children: !ids.is_empty(),
                live: true,
                pane_count: ids.len(),
            },
        ));
        let top_id = self.z_stack.top();
        let qt_visible = self.quick_terminal.visible;
        for id in ids {
            let Some(pane) = self.panes.get(&id) else { continue };
            rows.push(herdr_row(
                DrawerRowKind::Pane(id),
                pane.title().to_string(),
                !qt_visible && top_id == Some(id),
                HerdrRowMeta {
                    depth: 1,
                    level: HerdrLevel::Pane,
                    collapsed: false,
                    has_children: false,
                    live: true,
                    pane_count: 0,
                },
            ));
        }
        rows
    }
}

/// Builds a herdr row with every classic-only field neutral.
fn herdr_row(kind: DrawerRowKind, title: String, is_focused_pane: bool, meta: HerdrRowMeta) -> DrawerRow {
    DrawerRow {
        kind,
        connector: "",
        focus_rank_badge: String::new(),
        spinner: ' ',
        sparkline: String::new(),
        cmd_badge: String::new(),
        mode_badges: Vec::new(),
        transport_badges: Vec::new(),
        title,
        is_focused_pane,
        herdr: Some(meta),
    }
}
