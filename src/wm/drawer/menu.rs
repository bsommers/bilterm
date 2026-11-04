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

/// One menu entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigatorMenuEntry {
    NewSession,
    NewWindow,
    SplitHorizontal,
    SplitVertical,
    Rename,
    Close,
}

impl NavigatorMenuEntry {
    /// Entries in display order; `:drawer menu pick <n>` counts from 1 in this order.
    pub const ALL: [Self; 6] =
        [Self::NewSession, Self::NewWindow, Self::SplitHorizontal, Self::SplitVertical, Self::Rename, Self::Close];

    pub const fn label(self) -> &'static str {
        match self {
            Self::NewSession => "New session",
            Self::NewWindow => "New window here",
            Self::SplitHorizontal => "Split horizontal",
            Self::SplitVertical => "Split vertical",
            Self::Rename => "Rename",
            Self::Close => "Close",
        }
    }

    /// The operation the entry applies (its key twin: `N`, `a`, `A`, `r`, `x`).
    pub fn op(self) -> DrawerNavOp {
        match self {
            Self::NewSession => DrawerNavOp::NewSession,
            Self::NewWindow => DrawerNavOp::AddWindow(None),
            Self::SplitHorizontal => DrawerNavOp::AddPane(SplitDirection::Horizontal),
            Self::SplitVertical => DrawerNavOp::AddPane(SplitDirection::Vertical),
            Self::Rename => DrawerNavOp::RenamePrompt,
            Self::Close => DrawerNavOp::Close,
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
                    Some(entry) => self.apply_drawer_nav(entry.op()),
                    None => false,
                }
            }
            _ => false,
        }
    }
}
