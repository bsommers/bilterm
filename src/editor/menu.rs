//! MS-DOS EDIT style menu bar of the editor window: File / Edit / Search / View / Help.
//!
//! Model and keys only (painting is in `menu_render`). Every item that does something names a
//! [`MenuCmd`]; the window manager runs it as `Action::Editor(EditorOp::Menu(MenuOp::Do(cmd)))`,
//! so the keyboard, the mouse, `:editor menu ...`, RPC, MCP and the CLI share one path
//! (`UNI-R01`). Items the editor cannot do yet are listed disabled (`cmd: None`), never faked.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What a menu item does. The wire names (`:editor menu do <name>`) are in [`MenuCmd::NAMES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    New,
    Save,
    SaveAs,
    Revert,
    Broadcast,
    Close,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    ToggleCheck,
    Find,
    FindNext,
    FindPrev,
    Goto,
    Wrap,
    Render,
    Preview,
    Keys,
}

impl MenuCmd {
    pub const NAMES: &'static [(MenuCmd, &'static str)] = &[
        (Self::New, "new"),
        (Self::Save, "save"),
        (Self::SaveAs, "save-as"),
        (Self::Revert, "revert"),
        (Self::Broadcast, "broadcast"),
        (Self::Close, "close"),
        (Self::Undo, "undo"),
        (Self::Redo, "redo"),
        (Self::Cut, "cut"),
        (Self::Copy, "copy"),
        (Self::Paste, "paste"),
        (Self::SelectAll, "select-all"),
        (Self::ToggleCheck, "toggle-check"),
        (Self::Find, "find"),
        (Self::FindNext, "find-next"),
        (Self::FindPrev, "find-prev"),
        (Self::Goto, "goto"),
        (Self::Wrap, "wrap"),
        (Self::Render, "render"),
        (Self::Preview, "preview"),
        (Self::Keys, "keys"),
    ];

    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(c, _)| *c == self).map_or("", |(_, n)| n)
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(_, n)| *n == s).map(|(c, _)| *c)
    }
}

/// One row of a drop-down. `label == "-"` is a separator.
pub struct Item {
    pub label: &'static str,
    /// Letter that picks the item while its menu is open.
    pub mnemonic: char,
    /// Shortcut shown right-aligned (empty when the item has none).
    pub hotkey: &'static str,
    /// `None` = not implemented yet (shown disabled).
    pub cmd: Option<MenuCmd>,
    pub hint: &'static str,
}

pub struct Menu {
    pub title: &'static str,
    pub mnemonic: char,
    pub items: &'static [Item],
}

const fn it(label: &'static str, mnemonic: char, hotkey: &'static str, cmd: Option<MenuCmd>, hint: &'static str) -> Item {
    Item { label, mnemonic, hotkey, cmd, hint }
}
const SEP: Item = it("-", ' ', "", None, "");

pub const MENUS: &[Menu] = &[
    Menu {
        title: "File",
        mnemonic: 'f',
        items: &[
            it("New", 'n', "", Some(MenuCmd::New), "Open a new empty buffer in a new editor window"),
            it("Open...", 'o', "", None, "Not available yet: use :edit <path> from the prompt (Ctrl+])"),
            it("Save", 's', "Ctrl+S", Some(MenuCmd::Save), "Write the buffer to its file (asks for a name when it has none)"),
            it("Save As...", 'a', "Ctrl+O", Some(MenuCmd::SaveAs), "Write the buffer under a new file name"),
            it("Revert", 'r', "", Some(MenuCmd::Revert), "Reload the file from disk, discarding edits"),
            it("Broadcast to agents", 'b', "", Some(MenuCmd::Broadcast), "Sync this artifact and notify named agents"),
            SEP,
            it("Close window", 'c', "", Some(MenuCmd::Close), "Close this editor window (a second close discards unsaved changes)"),
        ],
    },
    Menu {
        title: "Edit",
        mnemonic: 'e',
        items: &[
            it("Undo", 'u', "Ctrl+Z", Some(MenuCmd::Undo), "Undo the last edit"),
            it("Redo", 'r', "Ctrl+Y", Some(MenuCmd::Redo), "Redo the last undone edit"),
            SEP,
            it("Cut", 't', "Ctrl+K", Some(MenuCmd::Cut), "Move the selection to the clipboard (Ctrl+X is the leader key)"),
            it("Copy", 'c', "Ctrl+C", Some(MenuCmd::Copy), "Copy the selection to the clipboard"),
            it("Paste", 'p', "Ctrl+V", Some(MenuCmd::Paste), "Insert the clipboard at the cursor"),
            it("Select all", 'a', "Ctrl+A", Some(MenuCmd::SelectAll), "Select the whole buffer"),
            SEP,
            it("Toggle check", 'k', "Ctrl+Space", Some(MenuCmd::ToggleCheck), "Toggle a Markdown task checkbox ([ ] / [x]) on the current line"),
        ],
    },
    Menu {
        title: "Search",
        mnemonic: 's',
        items: &[
            it("Find...", 'f', "Ctrl+F", Some(MenuCmd::Find), "Search forward for text"),
            it("Find next", 'n', "Ctrl+N", Some(MenuCmd::FindNext), "Jump to the next match"),
            it("Find previous", 'p', "Ctrl+P", Some(MenuCmd::FindPrev), "Jump to the previous match"),
            it("Go to line...", 'g', "Ctrl+G", Some(MenuCmd::Goto), "Move the cursor to a line number"),
            it("Replace...", 'r', "", None, "Not available yet (planned: :editor replace)"),
        ],
    },
    Menu {
        title: "View",
        mnemonic: 'v',
        items: &[
            it("Soft wrap", 'w', "Ctrl+L", Some(MenuCmd::Wrap), "Toggle soft wrapping of long lines"),
            it("Rendered view", 'r', "", Some(MenuCmd::Render), "Toggle rendered Markdown / interactive checklist reading view"),
            it("Browser preview", 'p', "", Some(MenuCmd::Preview), "Open or refresh live HTML/JS/Markdown preview in a browser pane"),
            it("Line numbers", 'l', "", None, "Not available yet"),
        ],
    },
    Menu {
        title: "Help",
        mnemonic: 'h',
        items: &[it("Keys...", 'k', "", Some(MenuCmd::Keys), "Show the editor key reference")],
    },
];

/// Resolves a menu by (case-insensitive) title for `:editor menu open <name>`.
pub fn menu_by_name(name: &str) -> Option<usize> {
    MENUS.iter().position(|m| m.title.eq_ignore_ascii_case(name))
}

pub fn menu_by_mnemonic(c: char) -> Option<usize> {
    MENUS.iter().position(|m| m.mnemonic == c.to_ascii_lowercase())
}

/// Bar column span `[x0, x1)` of each title: one blank each side of the word.
pub fn title_spans() -> Vec<(usize, usize)> {
    let mut x = 0;
    MENUS
        .iter()
        .map(|m| {
            let w = m.title.chars().count() + 2;
            let s = (x, x + w);
            x += w;
            s
        })
        .collect()
}

/// Result of feeding a key to an active menu.
#[derive(Debug, PartialEq, Eq)]
pub enum MenuKey {
    /// Consumed; nothing to run.
    Consumed,
    /// Consumed; run this item (the menu is closed already).
    Run(MenuCmd),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    /// Index of the open drop-down (`None` = closed).
    pub open: Option<usize>,
    pub item: usize,
    /// The Help > Keys dialog is showing.
    pub dialog: bool,
    /// Set by a mouse click; the window manager runs it as an action.
    pub pending: Option<MenuCmd>,
}

fn is_sep(it: &Item) -> bool {
    it.label == "-"
}

impl MenuState {
    pub fn is_active(&self) -> bool {
        self.open.is_some() || self.dialog
    }

    pub fn open_menu(&mut self, idx: usize) {
        self.dialog = false;
        self.open = Some(idx.min(MENUS.len() - 1));
        self.item = self.step(self.open.unwrap_or(0), 0, 1);
    }

    pub fn close(&mut self) {
        self.open = None;
        self.dialog = false;
    }

    /// First selectable (non-separator) item at or after `from` in direction `dir`, wrapping.
    fn step(&self, menu: usize, from: usize, dir: isize) -> usize {
        let items = MENUS[menu].items;
        let n = items.len() as isize;
        let mut i = from as isize;
        for _ in 0..items.len() {
            i = i.rem_euclid(n);
            if !is_sep(&items[i as usize]) {
                return i as usize;
            }
            i += dir;
        }
        0
    }

    fn pick(&mut self, menu: usize, item: usize) -> MenuKey {
        match MENUS[menu].items[item].cmd {
            Some(c) => {
                self.close();
                MenuKey::Run(c)
            }
            None => MenuKey::Consumed,
        }
    }

    /// Keys while the menu or the keys dialog is up. Everything is consumed.
    pub fn key(&mut self, key: KeyEvent) -> MenuKey {
        if key.kind == KeyEventKind::Release {
            return MenuKey::Consumed;
        }
        if self.dialog {
            self.dialog = false;
            return MenuKey::Consumed;
        }
        let Some(menu) = self.open else { return MenuKey::Consumed };
        let m = key.modifiers;
        let alt = m.contains(KeyModifiers::ALT);
        let n = MENUS.len();
        let last = MENUS[menu].items.len() - 1;
        match key.code {
            KeyCode::Esc | KeyCode::F(10) => self.close(),
            KeyCode::Left => self.open_menu((menu + n - 1) % n),
            KeyCode::Right | KeyCode::Tab => self.open_menu((menu + 1) % n),
            KeyCode::BackTab => self.open_menu((menu + n - 1) % n),
            KeyCode::Up => self.item = self.step(menu, (self.item + last) % (last + 1), -1),
            KeyCode::Down => self.item = self.step(menu, (self.item + 1) % (last + 1), 1),
            KeyCode::Home => self.item = self.step(menu, 0, 1),
            KeyCode::End => self.item = self.step(menu, last, -1),
            KeyCode::Enter => return self.pick(menu, self.item),
            KeyCode::Char(c) if alt => {
                if let Some(i) = menu_by_mnemonic(c) {
                    self.open_menu(i);
                }
            }
            KeyCode::Char(c) if !m.contains(KeyModifiers::CONTROL) => {
                let lc = c.to_ascii_lowercase();
                if let Some(i) = MENUS[menu].items.iter().position(|it| !is_sep(it) && it.mnemonic == lc) {
                    self.item = i;
                    return self.pick(menu, i);
                }
            }
            _ => {}
        }
        MenuKey::Consumed
    }

    /// The menu to open for a key while the menu is closed: `F10` (first menu) or `Alt+letter`.
    pub fn opener(key: KeyEvent) -> Option<usize> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        match (key.code, key.modifiers) {
            (KeyCode::F(10), KeyModifiers::NONE) => Some(0),
            (KeyCode::Char(c), KeyModifiers::ALT) => menu_by_mnemonic(c),
            _ => None,
        }
    }
}
