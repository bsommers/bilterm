//! Navigator right-click context menu: transient state and entry table (Phase 50-15, `FLT-01`).
//!
//! The menu is opened, dismissed and picked through [`DrawerNavOp::MenuOpen`], [`DrawerNavOp::MenuClose`]
//! and [`DrawerNavOp::MenuPick`], so a mouse gesture and a `:drawer menu` command change the same state.
//! Each entry applies the [`DrawerNavOp`] of its key twin; the menu adds no operation of its own.

use super::nav::DrawerNavOp;
use crate::wm::{SplitDirection, WindowManager};

/// An open context menu; `anchor` is the screen cell it was opened at (`None`: at the selected row).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigatorMenu {
    anchor: Option<(u16, u16)>,
}

impl NavigatorMenu {
    pub(super) fn at(anchor: Option<(u16, u16)>) -> Self {
        Self { anchor }
    }

    /// Screen cell the menu was opened at, `None` when it follows the selected row.
    pub fn anchor(&self) -> Option<(u16, u16)> {
        self.anchor
    }
}

impl super::WorkspaceDrawer {
    /// `true` while the navigator right-click menu is open (raw state; see `WindowManager::navigator_menu`).
    pub fn navigator_menu_open(&self) -> bool {
        self.nav_menu.is_some()
    }
}

/// One menu entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigatorMenuEntry {
    NewSession,
    NewWindow,
    SplitHorizontal,
    SplitVertical,
    Rename,
    Close,
    MoveWindow,
    BulkMove,
    BulkClose,
    BulkTile,
}

impl NavigatorMenuEntry {
    /// Entries in display order; `:drawer menu pick <n>` counts from 1 in this order.
    pub const ALL: [Self; 10] = [
        Self::NewSession,
        Self::NewWindow,
        Self::SplitHorizontal,
        Self::SplitVertical,
        Self::Rename,
        Self::Close,
        Self::MoveWindow,
        Self::BulkMove,
        Self::BulkClose,
        Self::BulkTile,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::NewSession => "New session",
            Self::NewWindow => "New window here",
            Self::SplitHorizontal => "Split horizontal",
            Self::SplitVertical => "Split vertical",
            Self::Rename => "Rename",
            Self::Close => "Close",
            Self::MoveWindow => "Move to session...",
            Self::BulkMove => "Bulk move...",
            Self::BulkClose => "Bulk close",
            Self::BulkTile => "Bulk tile",
        }
    }

    /// The operation the entry applies (its key twin: `N`, `a`, `A`, `r`, `x`).
    pub fn op(self) -> Option<DrawerNavOp> {
        match self {
            Self::NewSession => Some(DrawerNavOp::NewSession),
            Self::NewWindow => Some(DrawerNavOp::AddWindow(None)),
            Self::SplitHorizontal => Some(DrawerNavOp::AddPane(SplitDirection::Horizontal)),
            Self::SplitVertical => Some(DrawerNavOp::AddPane(SplitDirection::Vertical)),
            Self::Rename => Some(DrawerNavOp::RenamePrompt),
            Self::Close => Some(DrawerNavOp::Close),
            _ => None,
        }
    }

    pub fn apply(self, wm: &mut WindowManager) -> bool {
        match self {
            Self::NewSession => wm.apply_drawer_nav(DrawerNavOp::NewSession),
            Self::NewWindow => wm.apply_drawer_nav(DrawerNavOp::AddWindow(None)),
            Self::SplitHorizontal => wm.apply_drawer_nav(DrawerNavOp::AddPane(SplitDirection::Horizontal)),
            Self::SplitVertical => wm.apply_drawer_nav(DrawerNavOp::AddPane(SplitDirection::Vertical)),
            Self::Rename => wm.apply_drawer_nav(DrawerNavOp::RenamePrompt),
            Self::Close => wm.apply_drawer_nav(DrawerNavOp::Close),
            Self::MoveWindow => {
                let target_pane = wm.selected_drawer_pane_id();
                wm.drawer_mut().focused = false;
                let prefill = match target_pane {
                    Some(id) => format!(":window move {id} "),
                    None => ":window move ".to_string(),
                };
                let prompt = wm.active_pane_mut().prompt_mut();
                prompt.manual_raw_override = false;
                prompt.force_command_prompt = true;
                prompt.set_text(&prefill);
                wm.open_window_move_dialog(target_pane, false);
                true
            }
            Self::BulkMove => {
                wm.drawer_mut().focused = false;
                let prompt = wm.active_pane_mut().prompt_mut();
                prompt.manual_raw_override = false;
                prompt.force_command_prompt = true;
                prompt.set_text(":window bulk move ");
                wm.open_bulk_move_dialog();
                true
            }
            Self::BulkClose => {
                let res = wm.window_manage_apply(&crate::wm::window_manage::WindowManageOp::Bulk(
                    crate::wm::window_manage::BulkWindowOp::Close,
                ));
                if let Err(e) = res {
                    wm.active_pane_mut().set_status_message(Some(e));
                }
                true
            }
            Self::BulkTile => {
                let res = wm.window_manage_apply(&crate::wm::window_manage::WindowManageOp::Bulk(
                    crate::wm::window_manage::BulkWindowOp::Tile,
                ));
                if let Err(e) = res {
                    wm.active_pane_mut().set_status_message(Some(e));
                }
                true
            }
        }
    }
}

impl WindowManager {
    /// The open navigator context menu, `None` when closed or the navigator drawer is not showing.
    pub fn navigator_menu(&self) -> Option<&NavigatorMenu> {
        self.drawer.nav_menu.as_ref().filter(|_| {
            self.config.drawer.style == super::DrawerStyle::Navigator && self.is_drawer_effectively_visible()
        })
    }

    /// Applies a menu op; returns `true` when it changed state.
    pub(super) fn apply_navigator_menu(&mut self, op: DrawerNavOp) -> bool {
        match op {
            DrawerNavOp::MenuOpen(anchor) => {
                self.drawer.nav_menu = Some(NavigatorMenu::at(anchor));
                true
            }
            DrawerNavOp::MenuClose => self.drawer.nav_menu.take().is_some(),
            DrawerNavOp::MenuPick(i) => {
                self.drawer.nav_menu = None;
                match NavigatorMenuEntry::ALL.get(i) {
                    Some(entry) => entry.apply(self),
                    None => false,
                }
            }
            _ => false,
        }
    }
}
