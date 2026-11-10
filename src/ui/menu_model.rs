//! Top menu bar model (`KEY-01`, `KEY-03`, Phase 63-02).
//!
//! The data lives here (not in `modals.rs`): categories with a mnemonic, items with a label, the
//! `:colon` command they run and a mnemonic. Labels never embed key text; the accelerator shown at
//! the right of an item is computed from the live key tables ([`dropdown_hints`]) so it is correct
//! for the OS, the surface and the active profile.
//!
//! Existing items keep their positions (tests and docs address them by index); new items are
//! appended.

use crate::action::{Action, WindowModeOp};
use crate::colon_command::parse_colon_command;
use crate::input::keyprofile::EffectiveKeymap;
use crate::wm::WindowManager;

/// One menu entry.
#[derive(Clone, Copy, Debug)]
pub struct MenuItem {
    pub label: &'static str,
    pub command: &'static str,
    /// Letter that picks the item while its category is open (case-insensitive, unique per category).
    pub mnemonic: char,
}

/// One pull-down.
#[derive(Clone, Copy, Debug)]
pub struct MenuCategory {
    pub title: &'static str,
    /// Letter that opens the category (`Alt+letter` or `Shift+letter` while the menu is open).
    pub mnemonic: char,
    pub items: &'static [MenuItem],
}

const fn mi(label: &'static str, command: &'static str, mnemonic: char) -> MenuItem {
    MenuItem { label, command, mnemonic }
}

pub const MENUBAR_CATEGORIES: &[MenuCategory] = &[
    MenuCategory {
        title: "≡ System",
        mnemonic: 'S',
        items: &[
            mi("3270 Inspector", ":inspector", 'I'),
            mi("New Session", ":session new", 'N'),
            mi("Save Session", ":session save", 'S'),
            mi("Restore Session", ":session restore", 'R'),
            mi("Session Processes & CWD...", ":session info", 'P'),
            mi("Toggle Auto-Relaunch Apps", ":session processes", 'A'),
            mi("Quit", ":quit", 'Q'),
        ],
    },
    MenuCategory {
        title: "Window",
        mnemonic: 'W',
        items: &[
            mi("Split Horizontal", ":split h", 'H'),
            mi("Split Vertical", ":split v", 'V'),
            mi("Zoom Toggle", ":zoom", 'Z'),
            mi("Stack Windows", ":layout stack", 'S'),
            mi("Window Theme Browser...", ":window theme list", 'B'),
            mi("Customize Window Colors...", ":window menu", 'C'),
            mi("Next Window Theme", ":window theme next", 'N'),
            mi("Next Window Outline", ":window outline next", 'U'),
            mi("Toggle Bold Border", ":window bold toggle", 'D'),
            mi("Reset Window Style", ":window reset", 'R'),
            mi("Tile Windows & Panels", ":arrange grid", 'P'),
            mi("Tile Windows", ":layout tile", 'T'),
            mi("Toggle Stack/Tile", ":layout cycle", 'G'),
            mi("Window Mode", ":wmode", 'M'),
            mi("Copy Coordinate", ":coords copy", 'O'),
        ],
    },
    MenuCategory {
        title: "Prompt",
        mnemonic: 'P',
        items: &[
            mi("Pin Top", ":prompt top", 'T'),
            mi("Pin Bottom", ":prompt bottom", 'B'),
            mi("Fixed / Float", ":prompt split", 'F'),
            mi("Cycle Float/Fix/Term", ":mode cycle", 'C'),
        ],
    },
    MenuCategory {
        title: "View",
        mnemonic: 'V',
        items: &[
            mi("3270 Field Hints", ":hints", 'F'),
            mi("Vi Copy Mode", ":copy", 'C'),
            mi("Rectangular Select", ":rect", 'R'),
            mi("Toggle Rect Mouse Mode", ":rect mode", 'M'),
            mi("Cross-Hair Ruler", ":crosshair", 'H'),
            mi("DOS F-Key Bar", ":fkeybar", 'K'),
            mi("Agent Traffic", ":agents toggle", 'A'),
            mi("Find Agents", ":find agent", 'G'),
            mi("Find Sessions", ":find session", 'S'),
            mi("Find Command History", ":find history", 'Y'),
            mi("Agents List", ":agents list toggle", 'L'),
            mi("Agent Demo", ":agents demo", 'E'),
            mi("Stop Agent Demo", ":agents demo stop", 'P'),
            mi("Fonts Window", ":fonts toggle", 'N'),
            mi("Command Palette Window", ":commands toggle", 'W'),
            mi("Clipboard Viewer", ":clipboard toggle", 'B'),
            mi("Traffic Monitor", ":traffic toggle", 'T'),
            mi("Next Coordinate Badge Style", ":coords cycle", 'X'),
            mi("Context Bus Quick List", ":context list", 'Q'),
            mi("Agent Job Queue", ":agent queue list", 'J'),
            mi("Mission Control Squad HUD [Z]", ":agents hud toggle", 'Z'),
            mi("Swarm Status [7]", ":swarm status", '7'),
            mi("Agent Turn Replay [8]", ":agent replay show", '8'),
            mi("Script Studio", ":script toggle", 'U'),
            mi("Drawer Peek Preview", ":drawer preview toggle", 'V'),
            mi("Drawer Rows: 1 line", ":drawer rows 1", '1'),
            mi("Drawer Rows: 2 lines", ":drawer rows 2", '2'),
            mi("Voice Dictation Toggle", ":voice toggle", 'I'),
            mi("Browser Canvas", ":browser", 'O'),
            mi("Video Canvas", ":video", 'D'),
        ],
    },
    MenuCategory {
        title: "Theme",
        mnemonic: 'T',
        items: &[
            mi("Borland Turbo Blue", ":theme turbo-blue", 'B'),
            mi("Borland Turbo Cyan", ":theme turbo-cyan", 'C'),
            mi("IBM 3278 Phosphor Green", ":theme 3278", 'G'),
            mi("IBM 3279 Amber CRT", ":theme 3279", 'A'),
        ],
    },
    MenuCategory {
        title: "Tmux",
        mnemonic: 'X',
        items: &[
            mi("Broadcast Input", ":broadcast", 'B'),
            mi("Read-Only Lock", ":lock", 'L'),
            mi("Quake Terminal", ":quake", 'Q'),
            mi("Raw Passthrough", ":raw", 'R'),
            mi("SSH tmux link...", ":ssh-tmux prefill", 'S'),
            mi("Reconnect link", ":ssh-tmux reconnect", 'E'),
            mi("Disconnect link", ":ssh-tmux disconnect", 'D'),
            mi("Link info", ":ssh-tmux info", 'I'),
        ],
    },
    MenuCategory {
        title: "Help",
        mnemonic: 'H',
        items: &[
            mi("Help Reference", ":help", 'H'),
            mi("3270 Inspector", ":inspector", 'I'),
            mi("List Sessions", ":session list", 'L'),
            mi("Close Help", ":help", 'C'),
            mi("Keyboard Conflicts...", ":keys check", 'K'),
        ],
    },
    MenuCategory {
        title: "Daemon",
        mnemonic: 'D',
        items: &[
            mi("List Daemons", ":daemon list", 'L'),
            mi("Daemon Status", ":daemon status", 'S'),
            mi("Start Daemon", ":daemon start", 'T'),
            mi("Stop Daemon", ":daemon stop", 'O'),
            mi("Switch Daemon", ":daemon switch default", 'W'),
            mi("Prune Daemons", ":daemon prune", 'P'),
        ],
    },
    MenuCategory {
        title: "Web",
        mnemonic: 'E',
        items: &[
            mi("Web Server Status", ":web status", 'S'),
            mi("Open Web UI", ":web open", 'O'),
            mi("Stop Web Server", ":web stop", 'T'),
        ],
    },
];

/// Chords handled before the tables are consulted, so `hint_for_action` cannot find them.
const ROUTED_OUTSIDE_TABLES: &[(&str, &str)] = &[(":quit", "Ctrl+Q")];

/// Index of the category whose mnemonic is `c` (case-insensitive).
pub fn category_by_mnemonic(c: char) -> Option<usize> {
    MENUBAR_CATEGORIES.iter().position(|cat| cat.mnemonic.eq_ignore_ascii_case(&c))
}

/// Index of the item of `category` whose mnemonic is `c` (case-insensitive).
pub fn item_by_mnemonic(category: usize, c: char) -> Option<usize> {
    MENUBAR_CATEGORIES.get(category)?.items.iter().position(|item| item.mnemonic.eq_ignore_ascii_case(&c))
}

/// Byte index of the mnemonic letter inside `text` (case-insensitive), for underlining.
pub fn mnemonic_index(text: &str, mnemonic: char) -> Option<usize> {
    text.char_indices().find(|(_, c)| c.eq_ignore_ascii_case(&mnemonic)).map(|(i, _)| i)
}

/// The action an item runs, normalised for accelerator lookup (`:wmode` toggles, but the keys that
/// reach Window mode all enter it).
pub fn menu_action(command: &str) -> Option<Action> {
    let parsed = parse_colon_command(command)?.ok()?;
    Some(match parsed {
        Action::WindowMode(_) => Action::WindowMode(WindowModeOp::Enter),
        Action::MenuOpen { .. } => Action::MenuOpen { category: None },
        other => other,
    })
}

/// Accelerator text of one item under `km` (Primary chord > leader sequence > Window-mode
/// sequence > any other chord), with a short fallback for keys routed outside the tables.
pub fn item_hint(km: &EffectiveKeymap, leader_label: &str, item: &MenuItem) -> Option<String> {
    if let Some(action) = menu_action(item.command)
        && let Some(hint) = km.hint_for_action_with_leader(&action, leader_label)
    {
        return Some(hint);
    }
    ROUTED_OUTSIDE_TABLES.iter().find(|(c, _)| *c == item.command).map(|(_, h)| (*h).to_string())
}

/// Accelerators of every item of `category` for the live workspace (profile, env and leader key).
pub fn dropdown_hints(wm: &WindowManager, category: usize) -> Vec<Option<String>> {
    let Some(cat) = MENUBAR_CATEGORIES.get(category.min(MENUBAR_CATEGORIES.len() - 1)) else {
        return Vec::new();
    };
    let km = wm.keymap();
    let leader = wm.config().leader_key.clone();
    cat.items.iter().map(|item| item_hint(&km, &leader, item)).collect()
}
