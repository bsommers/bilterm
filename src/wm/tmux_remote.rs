//! Remote-pane helpers for `:ssh-tmux`: pty-less splits, prompt prefill, provisional `%0` mapping.

use crate::error::Result;
use crate::pane::PaneId;
use crate::tmux::hydrate::ReplyKind;
use crate::tmux::link::LinkState;
use crate::wm::{SplitDirection, WindowManager};

/// Command that asks the remote tmux server for every pane (reply: `@win %pane name` lines).
pub(crate) const LIST_PANES_CMD: &str = "list-panes -s -F \"#{window_id} #{pane_id} #{window_name}\"";

impl WindowManager {
    /// Splits the focused pane like `split_focused`, but the new pane never owns a local PTY:
    /// remote (ssh-tmux) panes are fed by the link and must not spawn or write to a local shell.
    pub fn split_focused_remote_pane(&mut self, direction: SplitDirection) -> Result<PaneId> {
        let prev = std::mem::replace(&mut self.headless_mode, true);
        let res = self.split_focused(direction);
        self.headless_mode = prev;
        res
    }

    /// `:ssh-tmux prefill`: puts the active pane into a clean command-prompt state with
    /// `:ssh-tmux ` typed, whatever mode it was in (modal, copy mode, raw passthrough, ...).
    pub(crate) fn prefill_ssh_tmux_prompt(&mut self) {
        self.close_modal();
        let pane = self.active_pane_mut();
        if pane.copy_mode().active {
            pane.exit_copy_mode();
        }
        if pane.hints_mode().active {
            pane.exit_hints_mode();
        }
        pane.clear_canvas_search();
        pane.prompt_mut().cancel_fuzzy_search();
        pane.prompt_mut().history_popup_open = false;
        pane.set_link_raw_input(false);
        let prompt = pane.prompt_mut();
        prompt.manual_raw_override = false;
        prompt.force_command_prompt = true;
        prompt.set_text(":ssh-tmux ");
        pane.set_status_message(Some("Type <user@host> [session] and press Enter".to_string()));
    }

    /// While connected with the link's primary pane still unmapped (idle remote session: no
    /// `%layout-change`/`%output`), binds the EXISTING primary pane to its previous tmux id (or
    /// `%0`) and returns that id. `None` when nothing needs (or can) be mapped.
    pub(crate) fn ensure_provisional_primary(&mut self) -> Option<usize> {
        let primary = self.tmux_link.primary_pane?;
        if !self.panes.contains_key(&primary)
            || !self.tmux_bridge.bilterm_to_tmux.is_empty()
            || self.tmux_bridge.primary() != Some(primary)
        {
            return None;
        }
        let mut bridge = std::mem::take(&mut self.tmux_bridge);
        let tmux = bridge.map_provisional_primary(self);
        self.tmux_bridge = bridge;
        tmux
    }

    /// Applies a `list-panes` reply to the bridge, never creating local PTYs.
    pub(crate) fn apply_list_panes_reply(&mut self, lines: &[String]) {
        let mut bridge = std::mem::take(&mut self.tmux_bridge);
        bridge.apply_list_panes(self, lines);
        self.tmux_bridge = bridge;
    }

    /// Idle remote sessions emit no layout/output, so after the greeting nothing is ever mapped:
    /// bind the primary provisionally and ask tmux for its panes (once per attach).
    pub(crate) fn discover_idle_panes(&mut self) {
        if !matches!(self.tmux_link.machine.state(), LinkState::Connected { .. })
            || self.tmux_bridge.blocks_closed() == 0
            || !self.tmux_bridge.bilterm_to_tmux.is_empty()
            || self.tmux_link.aux.notes.contains_key("list_panes_queued")
        {
            return;
        }
        self.tmux_link.aux.notes.insert("list_panes_queued".to_string(), "1".to_string());
        let _ = self.ensure_provisional_primary();
        self.queue_tmux_command(LIST_PANES_CMD, ReplyKind::ListPanes);
    }
}
