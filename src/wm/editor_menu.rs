//! Editor menu bar execution: `EditorOp::Menu` (open, close, run an item) and the keyboard and
//! mouse entry points. Menu items never act directly: the keys and clicks only build
//! `Action::Editor(EditorOp::Menu(..))` and call `WindowManager::apply` (`ARCH-R01`, `UNI-R01`).

use super::WindowManager;
use crate::action::Action;
use crate::api::JsonValue;
use crate::editor::menu::{MENUS, MenuKey, MenuState, menu_by_name};
use crate::editor::{EditorOp, MenuCmd, MenuOp, Mode};

impl WindowManager {
    /// Executes `EditorOp::Menu`.
    pub(super) fn editor_menu_op(&mut self, op: MenuOp) {
        let Some(id) = self.editor_target() else {
            return self.editor_reply_pub(false, "no editor window (:edit [path], :scratch, or :artifact [name])".into());
        };
        match op {
            MenuOp::Open(name) => {
                let idx = name.as_deref().and_then(menu_by_name).unwrap_or(0);
                if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
                    ed.focused = true;
                    ed.mode = Mode::Edit;
                    ed.menu.open_menu(idx);
                }
                self.menu_done(id, format!("menu {}", MENUS[idx].title.to_ascii_lowercase()));
            }
            MenuOp::Close => {
                if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
                    ed.menu.close();
                }
                self.menu_done(id, "menu closed".into());
            }
            MenuOp::Do(cmd) => {
                if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
                    ed.menu.close();
                }
                self.menu_cmd(id, cmd);
            }
        }
    }

    fn menu_done(&mut self, id: crate::pane::PaneId, message: String) {
        let mut extra = vec![("pane", JsonValue::Number(id as i64))];
        if let Some(p) = self.panes.get_mut(&id) {
            if let Some(ed) = p.editor() {
                extra.push(("editor", JsonValue::parse(&ed.status_json()).unwrap_or(JsonValue::Null)));
            }
            p.paint_editor();
        }
        self.editor_reply(true, message, extra);
    }

    fn editor_reply_pub(&mut self, ok: bool, message: String) {
        self.editor_reply(ok, message, vec![]);
    }

    fn menu_cmd(&mut self, id: crate::pane::PaneId, cmd: MenuCmd) {
        // Items that are an existing editor verb reuse it.
        let delegated = match cmd {
            MenuCmd::New => Some(EditorOp::Open { path: None }),
            MenuCmd::Revert => Some(EditorOp::Revert),
            MenuCmd::Broadcast => Some(EditorOp::Broadcast { note: None }),
            MenuCmd::Close => Some(EditorOp::Close { force: false }),
            MenuCmd::Undo => Some(EditorOp::Undo),
            MenuCmd::Redo => Some(EditorOp::Redo),
            MenuCmd::ToggleCheck => Some(EditorOp::Check),
            MenuCmd::Wrap => Some(EditorOp::Wrap(None)),
            MenuCmd::Render => Some(EditorOp::Render(None)),
            MenuCmd::Preview => Some(EditorOp::Preview),
            _ => None,
        };
        if let Some(op) = delegated {
            return self.apply_editor_op(op);
        }
        let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) else { return };
        ed.message = None;
        match cmd {
            MenuCmd::Save => ed.save_or_prompt(false),
            MenuCmd::SaveAs => ed.mode = Mode::SaveAs(String::new()),
            MenuCmd::Cut => {
                let t = ed.buf.cut();
                if !t.is_empty() {
                    crate::clipboard::set_system_clipboard(&t);
                }
            }
            MenuCmd::Copy => ed.copy(),
            MenuCmd::Paste => match crate::clipboard::get_system_clipboard() {
                Some(t) => ed.paste(&t),
                None => ed.message = Some("clipboard is empty".into()),
            },
            MenuCmd::SelectAll => ed.buf.select_all(),
            MenuCmd::Find => ed.mode = Mode::Find(ed.search.clone()),
            MenuCmd::FindNext => ed.find_report(true),
            MenuCmd::FindPrev => ed.find_report(false),
            MenuCmd::Goto => ed.mode = Mode::Goto(String::new()),
            MenuCmd::Keys => ed.menu.dialog = true,
            _ => {}
        }
        let msg = ed.message.clone().unwrap_or_else(|| cmd.name().to_string());
        self.menu_done(id, msg);
    }

    /// Menu keys of the focused editor, or `F10` / `Alt+letter` to open it. `None` = not ours.
    pub(super) fn editor_menu_key(&mut self, id: crate::pane::PaneId, key: crossterm::event::KeyEvent) -> Option<bool> {
        let ed = self.panes.get_mut(&id)?.editor_mut()?;
        if ed.menu.is_active() {
            let out = ed.menu.key(key);
            self.after_editor_change();
            if let MenuKey::Run(c) = out {
                let _ = self.apply(Action::Editor(EditorOp::Menu(MenuOp::Do(c))));
            }
            return Some(true);
        }
        if matches!(ed.mode, Mode::Edit)
            && let Some(i) = MenuState::opener(key)
        {
            let name = MENUS[i].title.to_ascii_lowercase();
            let _ = self.apply(Action::Editor(EditorOp::Menu(MenuOp::Open(Some(name)))));
            return Some(true);
        }
        None
    }

    /// Runs the item a mouse click picked in an editor's drop-down (see `menu_click`).
    pub fn editor_menu_pending(&mut self) {
        let due: Vec<_> = self.panes.values_mut().filter_map(|p| p.editor_mut()?.menu.pending.take()).collect();
        for cmd in due {
            let _ = self.apply(Action::Editor(EditorOp::Menu(MenuOp::Do(cmd))));
        }
    }
}
