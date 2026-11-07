//! Interactive TUI Rename & Move Dialog state and input handlers (`SES-01..05`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::ui::rename_dialog_ui::{
    RenameDialogHit, hit_test_rename_dialog, rename_dialog_rect,
};
use crate::wm::drawer::{DrawerRow, DrawerRowKind};
use crate::wm::window_manage::{
    BulkWindowOp, InlineRenameState, SessionManageOp, WindowManageOp,
};
use crate::wm::{ rect_contains, WindowManager};

/// Semantic operation performed when the interactive dialog is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameDialogKind {
    /// Renames a window (optionally scoped to a specific session).
    Window {
        pane_id: PaneId,
        session: Option<String>,
    },
    /// Renames a session.
    Session {
        old_name: String,
    },
    /// Moves a window to a target session.
    MoveWindow {
        pane_id: PaneId,
        switch: bool,
    },
    /// Moves all currently selected windows to a target session.
    BulkMove,
    /// Prepends a prefix to all currently selected windows.
    BulkRenamePrefix,
}

/// Runtime state of the interactive TUI Rename / Move popup dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameDialogState {
    pub kind: RenameDialogKind,
    pub title: String,
    pub subtitle: String,
    pub current_input: String,
    /// Character index of the editing caret (`0..=current_input.chars().count()`).
    pub cursor: usize,
}

impl RenameDialogState {
    pub fn char_len(&self) -> usize {
        self.current_input.chars().count()
    }

    fn byte_index_of_char(&self, char_idx: usize) -> usize {
        self.current_input
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.current_input.len())
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.char_len() >= 64 {
            return;
        }
        let cursor = self.cursor.min(self.char_len());
        let byte_idx = self.byte_index_of_char(cursor);
        self.current_input.insert(byte_idx, ch);
        self.cursor = cursor + 1;
    }

    pub fn backspace(&mut self) {
        let cursor = self.cursor.min(self.char_len());
        if cursor == 0 {
            return;
        }
        let start = self.byte_index_of_char(cursor - 1);
        let end = self.byte_index_of_char(cursor);
        self.current_input.replace_range(start..end, "");
        self.cursor = cursor - 1;
    }

    pub fn delete(&mut self) {
        let len = self.char_len();
        let cursor = self.cursor.min(len);
        if cursor >= len {
            return;
        }
        let start = self.byte_index_of_char(cursor);
        let end = self.byte_index_of_char(cursor + 1);
        self.current_input.replace_range(start..end, "");
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.min(self.char_len()).saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let len = self.char_len();
        self.cursor = (self.cursor + 1).min(len);
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.char_len();
    }

    pub fn clear(&mut self) {
        self.current_input.clear();
        self.cursor = 0;
    }

    pub fn delete_word(&mut self) {
        let cursor = self.cursor.min(self.char_len());
        if cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.current_input.chars().collect();
        let mut new_cur = cursor;
        while new_cur > 0 && chars[new_cur - 1].is_whitespace() {
            new_cur -= 1;
        }
        while new_cur > 0 && !chars[new_cur - 1].is_whitespace() && chars[new_cur - 1] != '-' && chars[new_cur - 1] != '_' {
            new_cur -= 1;
        }
        if new_cur == cursor {
            new_cur = cursor.saturating_sub(1);
        }
        let start = self.byte_index_of_char(new_cur);
        let end = self.byte_index_of_char(cursor);
        self.current_input.replace_range(start..end, "");
        self.cursor = new_cur;
    }
}

impl WindowManager {
    /// Returns the active rename/move dialog state if open.
    pub fn rename_dialog(&self) -> Option<&RenameDialogState> {
        self.window_manage.rename_dialog.as_ref()
    }

    /// Returns a mutable reference to the active rename/move dialog state if open.
    pub fn rename_dialog_mut(&mut self) -> Option<&mut RenameDialogState> {
        self.window_manage.rename_dialog.as_mut()
    }

    /// Returns `true` when the interactive rename/move popup dialog is open.
    pub fn rename_dialog_open(&self) -> bool {
        self.window_manage.rename_dialog.is_some()
    }

    /// Resolves `(session_name, current_title)` for `target_id` across active, background, and saved sessions.
    pub(crate) fn resolve_pane_session_and_title(
        &self,
        target_id: PaneId,
        session_hint: Option<&str>,
    ) -> (Option<String>, String) {
        if let Some(hint) = session_hint {
            if hint == self.config.history_session
                && let Some(p) = self.panes.get(&target_id)
            {
                return (Some(hint.to_string()), p.title().to_string());
            }
            if let Some(bg) = self.window_manage.background_sessions.get(hint)
                && let Some(p) = bg.panes.get(&target_id)
            {
                return (Some(hint.to_string()), p.title().to_string());
            }
            if let Some(snap) = self.load_session_snapshot_any(hint)
                && let Some(p) = snap.panes.iter().find(|p| p.id == target_id)
            {
                let t = p.custom_name.clone().unwrap_or_else(|| p.title.clone());
                return (Some(hint.to_string()), t);
            }
        }

        if let Some(p) = self.panes.get(&target_id) {
            return (
                Some(self.config.history_session.clone()),
                p.title().to_string(),
            );
        }
        for (sess, bg) in &self.window_manage.background_sessions {
            if let Some(p) = bg.panes.get(&target_id) {
                return (Some(sess.clone()), p.title().to_string());
            }
        }
        for sess in self.list_sessions() {
            if let Some(snap) = self.load_session_snapshot_any(&sess)
                && let Some(p) = snap.panes.iter().find(|p| p.id == target_id)
            {
                let t = p.custom_name.clone().unwrap_or_else(|| p.title.clone());
                return (Some(sess), t);
            }
        }
        (session_hint.map(ToOwned::to_owned), format!("PANE #{target_id}"))
    }

    /// Opens the interactive Window Rename dialog for `pane_id` (defaulting to selected drawer pane or active pane).
    pub fn open_window_rename_dialog(&mut self, pane_id: Option<PaneId>) {
        let rows = self.drawer_rows();
        let drawer_pane = self.selected_drawer_pane_id();
        let target_id = pane_id
            .or(drawer_pane)
            .unwrap_or_else(|| self.active_pane_id());
        let hint = if pane_id.is_none() || pane_id == drawer_pane {
            self.selected_navigator_session(&rows)
        } else {
            None
        };
        let (session, current_name) =
            self.resolve_pane_session_and_title(target_id, hint.as_deref());
        let cursor = current_name.chars().count();

        self.window_manage.inline_rename = Some(InlineRenameState {
            pane_id: target_id,
            original_name: current_name.clone(),
            current_input: current_name.clone(),
        });
        self.window_manage.rename_dialog = Some(RenameDialogState {
            kind: RenameDialogKind::Window {
                pane_id: target_id,
                session,
            },
            title: format!("RENAME WINDOW #{target_id}"),
            subtitle: format!("New name for Window #{target_id} (current: '{current_name}'):"),
            current_input: current_name,
            cursor,
        });
    }

    /// Opens the interactive Session Rename dialog for `session_name` (defaulting to selected drawer session or active session).
    pub fn open_session_rename_dialog(&mut self, session_name: Option<String>) {
        let old_name = session_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.selected_drawer_session_name());
        let cursor = old_name.chars().count();
        self.window_manage.inline_rename = None;
        self.window_manage.rename_dialog = Some(RenameDialogState {
            kind: RenameDialogKind::Session {
                old_name: old_name.clone(),
            },
            title: format!("RENAME SESSION [{old_name}]"),
            subtitle: format!("New name for session '{old_name}':"),
            current_input: old_name,
            cursor,
        });
    }

    /// Opens the interactive Move Window to Session dialog.
    pub fn open_window_move_dialog(&mut self, pane_id: Option<PaneId>, switch: bool) {
        let target_id = pane_id
            .or_else(|| self.selected_drawer_pane_id())
            .unwrap_or_else(|| self.active_pane_id());
        let default_target = self.default_other_session_name();
        let cursor = default_target.chars().count();
        self.window_manage.inline_rename = None;
        self.window_manage.rename_dialog = Some(RenameDialogState {
            kind: RenameDialogKind::MoveWindow {
                pane_id: target_id,
                switch,
            },
            title: format!("MOVE WINDOW #{target_id} TO SESSION"),
            subtitle: format!("Target session name for Window #{target_id}:"),
            current_input: default_target,
            cursor,
        });
    }

    /// Opens the interactive Bulk Move Windows dialog.
    pub fn open_bulk_move_dialog(&mut self) {
        let count = self.window_manage.selected_windows.len();
        let default_target = self.default_other_session_name();
        let cursor = default_target.chars().count();
        self.window_manage.inline_rename = None;
        self.window_manage.rename_dialog = Some(RenameDialogState {
            kind: RenameDialogKind::BulkMove,
            title: format!("BULK MOVE ({count} SELECTED)"),
            subtitle: "Target session name for selected windows:".to_string(),
            current_input: default_target,
            cursor,
        });
    }

    /// Opens the interactive Bulk Rename Prefix dialog.
    pub fn open_bulk_rename_prefix_dialog(&mut self) {
        let count = self.window_manage.selected_windows.len();
        self.window_manage.inline_rename = None;
        self.window_manage.rename_dialog = Some(RenameDialogState {
            kind: RenameDialogKind::BulkRenamePrefix,
            title: format!("BULK RENAME PREFIX ({count} SELECTED)"),
            subtitle: "Prefix to prepend to selected window names:".to_string(),
            current_input: String::new(),
            cursor: 0,
        });
    }

    /// Opens either the Window Rename dialog or Session Rename dialog depending on the selected Drawer row.
    pub fn open_drawer_rename_dialog(&mut self) {
        let rows = self.drawer_rows();
        match rows.get(self.drawer.selected_index) {
            Some(DrawerRow {
                kind: DrawerRowKind::Pane(pane_id),
                ..
            }) => {
                self.open_window_rename_dialog(Some(*pane_id));
            }
            Some(DrawerRow {
                kind: DrawerRowKind::SessionHeader(s),
                ..
            }) => {
                self.open_session_rename_dialog(Some(s.clone()));
            }
            _ => {
                self.open_session_rename_dialog(None);
            }
        }
    }

    fn default_other_session_name(&self) -> String {
        let cur = self.config.history_session.trim();
        for s in self.ordered_sessions_with_active() {
            if s != cur {
                return s;
            }
        }
        for node in &self.daemon_state.session_tree.nodes {
            if node.name != cur {
                return node.name.clone();
            }
        }
        String::new()
    }

    fn clear_prefilled_rename_prompt(&mut self) {
        let text = self.active_pane().prompt().text();
        if text.starts_with(":drawer rename")
            || text.starts_with(":window rename")
            || text.starts_with(":session rename")
            || text.starts_with(":window move")
            || text.starts_with(":window bulk")
        {
            let prompt = self.active_pane_mut().prompt_mut();
            prompt.set_text("");
            prompt.force_command_prompt = false;
        }
    }

    /// Handles a key event while the rename/move dialog is open. Returns `true` if consumed.
    pub fn handle_rename_dialog_key(&mut self, key: KeyEvent) -> bool {
        if self.window_manage.rename_dialog.is_none() {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.cancel_rename_dialog();
                return true;
            }
            KeyCode::Enter => {
                let _ = self.commit_rename_dialog();
                return true;
            }
            KeyCode::Backspace => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.backspace();
                }
            }
            KeyCode::Delete => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.delete();
                }
            }
            KeyCode::Left => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.move_left();
                }
            }
            KeyCode::Right => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.move_right();
                }
            }
            KeyCode::Home => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.move_home();
                }
            }
            KeyCode::End => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.move_end();
                }
            }
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    match c.to_ascii_lowercase() {
                        'u' => d.clear(),
                        'w' => d.delete_word(),
                        'a' => d.move_home(),
                        'e' => d.move_end(),
                        'c' | 'g' => {
                            self.cancel_rename_dialog();
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                    d.insert_char(c);
                }
            }
            _ => {}
        }
        if let Some(new_input) = self
            .window_manage
            .rename_dialog
            .as_ref()
            .map(|d| d.current_input.clone())
            && let Some(ir) = self.window_manage.inline_rename.as_mut()
        {
            ir.current_input = new_input;
        }
        true
    }

    /// Handles a mouse event while the rename/move dialog is open. Returns `true` if consumed.
    pub fn handle_rename_dialog_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        if self.window_manage.rename_dialog.is_none() {
            return false;
        }
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let rect = rename_dialog_rect(area);
            if !rect_contains(rect, mouse.column, mouse.row) {
                self.cancel_rename_dialog();
                return true;
            }
            match hit_test_rename_dialog(rect, mouse.column, mouse.row) {
                Some(RenameDialogHit::Close) | Some(RenameDialogHit::CancelButton) => {
                    self.cancel_rename_dialog();
                }
                Some(RenameDialogHit::SaveButton) => {
                    let _ = self.commit_rename_dialog();
                }
                Some(RenameDialogHit::ClearButton) => {
                    if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                        d.clear();
                    }
                    if let Some(ir) = self.window_manage.inline_rename.as_mut() {
                        ir.current_input.clear();
                    }
                }
                Some(RenameDialogHit::InputField(col_offset)) => {
                    if let Some(d) = self.window_manage.rename_dialog.as_mut() {
                        d.cursor = col_offset.min(d.char_len());
                    }
                }
                None => {}
            }
        }
        true
    }

    /// Commits the current input of the interactive rename/move dialog.
    pub fn commit_rename_dialog(&mut self) -> Result<String, String> {
        let Some(dialog) = self.window_manage.rename_dialog.take() else {
            return Err("No rename dialog open".to_string());
        };
        self.window_manage.inline_rename = None;
        self.clear_prefilled_rename_prompt();

        let input = dialog.current_input.trim().to_string();
        if input.is_empty() {
            let err = "Input cannot be empty".to_string();
            self.active_pane_mut().set_status_message(Some(err.clone()));
            return Err(err);
        }

        let res = match dialog.kind {
            RenameDialogKind::Window { pane_id, session } => {
                self.rename_window_in_session(Some(pane_id), session.as_deref(), &input)
            }
            RenameDialogKind::Session { old_name } => self.session_manage_apply(
                &SessionManageOp::Rename {
                    old_name: Some(old_name),
                    new_name: input,
                },
            ),
            RenameDialogKind::MoveWindow { pane_id, switch } => self.window_manage_apply(
                &WindowManageOp::Move {
                    window_id: Some(pane_id),
                    target_session: input,
                    switch,
                },
            ),
            RenameDialogKind::BulkMove => self.window_manage_apply(&WindowManageOp::Bulk(
                BulkWindowOp::Move {
                    target_session: input,
                },
            )),
            RenameDialogKind::BulkRenamePrefix => self.window_manage_apply(&WindowManageOp::Bulk(
                BulkWindowOp::RenamePrefix { prefix: input },
            )),
        };

        match &res {
            Ok(msg) => self.active_pane_mut().set_status_message(Some(msg.clone())),
            Err(err) => self.active_pane_mut().set_status_message(Some(err.clone())),
        }
        res
    }

    /// Cancels and closes the interactive rename/move dialog without mutating state.
    pub fn cancel_rename_dialog(&mut self) {
        self.window_manage.rename_dialog = None;
        self.window_manage.inline_rename = None;
        self.clear_prefilled_rename_prompt();
        self.active_pane_mut()
            .set_status_message(Some("Dialog canceled".to_string()));
    }
}
