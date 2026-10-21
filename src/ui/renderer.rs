//! Abstract UI & Window Layer Renderer Seam (`ARCH-03`).
//!
//! Decouples Bilterm's window manager (`WindowManager`, `SplitTree`, `ZStack`, `QuickTerminal`)
//! and virtual terminal core (`libghostty-vt` / `VtParser` -> [`GridSnapshot`]) from the concrete
//! GUI/TUI toolkit so the same state can drive:
//! - **Ratatui + Crossterm** (in-terminal TUI mode)
//! - **GPUI** (Zed's hardware-accelerated GPU window framework)
//! - **`winit` + `egui`** (immediate-mode native desktop GUI)

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::pane::PaneId;
use crate::ui::WorkspaceWidget;
use crate::vt::snapshot::GridSnapshot;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Point-in-time frame snapshot of a single [`CompositePane`](crate::pane::CompositePane) for GPU/GUI renderers (`GPUI` / `egui`).
#[derive(Clone, Debug)]
pub struct PaneFrameSnapshot {
    pub id: PaneId,
    pub title: String,
    pub outer_rect: Rect,
    pub canvas_rect: Rect,
    pub prompt_rect: Option<Rect>,
    pub status_bar_rect: Rect,
    pub active: bool,
    pub z_rank: Option<(usize, usize)>,
    pub grid: GridSnapshot,
}

/// Complete point-in-time workspace frame snapshot consumed by external UI layers (`GPUI`, `egui`, or headless tests).
#[derive(Clone, Debug)]
pub struct WorkspaceFrameSnapshot {
    pub viewport: Rect,
    pub layout_mode: WorkspaceLayoutMode,
    pub panes: Vec<PaneFrameSnapshot>,
    pub quick_terminal: Option<PaneFrameSnapshot>,
    pub help_visible: bool,
}

impl WorkspaceFrameSnapshot {
    /// Captures a complete point-in-time [`WorkspaceFrameSnapshot`] from `wm` at `viewport`.
    pub fn capture(wm: &WindowManager, viewport: Rect) -> Self {
        let workspace_area = if wm.config.show_dos_fkey_bar && viewport.height > 2 {
            Rect {
                x: viewport.x,
                y: viewport.y,
                width: viewport.width,
                height: viewport.height - 1,
            }
        } else {
            viewport
        };

        let rects = wm.compute_workspace_rects(workspace_area);
        let top_id = wm.z_stack.top();
        let qt_visible = wm.quick_terminal.visible;

        let mut panes = Vec::with_capacity(rects.len());
        for (id, outer) in rects {
            let Some(pane) = wm.panes.get(&id) else {
                continue;
            };
            let active = !qt_visible && top_id == Some(id);
            let canvas_rect = pane.canvas_rect(outer);
            let prompt_rect = pane.is_prompt_visible().then(|| pane.prompt_rect(outer));
            let status_bar_rect = pane.status_bar_rect(outer);
            panes.push(PaneFrameSnapshot {
                id,
                title: pane.title.clone(),
                outer_rect: outer,
                canvas_rect,
                prompt_rect,
                status_bar_rect,
                active,
                z_rank: wm.z_stack.rank_of(id),
                grid: pane.vt.snapshot(),
            });
        }

        let quick_terminal = if qt_visible {
            let qt_outer = wm.quick_terminal.overlay_rect(workspace_area);
            let qt_pane = &wm.quick_terminal.pane;
            Some(PaneFrameSnapshot {
                id: qt_pane.id,
                title: qt_pane.title.clone(),
                outer_rect: qt_outer,
                canvas_rect: qt_pane.canvas_rect(qt_outer),
                prompt_rect: qt_pane
                    .is_prompt_visible()
                    .then(|| qt_pane.prompt_rect(qt_outer)),
                status_bar_rect: qt_pane.status_bar_rect(qt_outer),
                active: true,
                z_rank: None,
                grid: qt_pane.vt.snapshot(),
            })
        } else {
            None
        };

        Self {
            viewport,
            layout_mode: wm.layout_mode,
            panes,
            quick_terminal,
            help_visible: wm.help_visible,
        }
    }
}

/// Trait implemented by any UI/Window rendering backend (`Ratatui`, `GPUI`, `winit + egui`).
pub trait WorkspaceFrameRenderer {
    type Surface;

    /// Renders the current state of `wm` onto `surface` within `viewport`.
    fn render_workspace(&mut self, wm: &WindowManager, viewport: Rect, surface: &mut Self::Surface);
}

/// Default immediate-mode TUI renderer backed by `ratatui::buffer::Buffer`.
#[derive(Debug, Default)]
pub struct RatatuiWorkspaceRenderer;

impl WorkspaceFrameRenderer for RatatuiWorkspaceRenderer {
    type Surface = Buffer;

    fn render_workspace(&mut self, wm: &WindowManager, viewport: Rect, surface: &mut Self::Surface) {
        WorkspaceWidget::new(wm).render(viewport, surface);
    }
}
