//! Keyboard handling of the top menu bar (`KEY-01`, Phase 63-02).
//!
//! The menu opens from the keyboard through `Action::MenuOpen` (leader `m`, `Primary+;`,
//! `:menu open`); this module drives it once open. Keys: `Left`/`Right`/`Tab`/`BackTab` change the
//! category (wrapping over the whole table), `Up`/`Down`/`Home`/`End` the item, `Enter` runs it, `Esc`
//! closes. A letter picks the item with that mnemonic in the open category and, when no item has
//! it, opens the category with that mnemonic; `Alt+letter` and `Shift+letter` always address the
//! category level.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use crate::error::Result;
use crate::ui::menu_model::{MENUBAR_CATEGORIES, category_by_mnemonic, item_by_mnemonic};
use crate::wm::{Modal, WindowManager};

/// Where a key moves the cursor, computed without touching the workspace.
#[derive(Debug, PartialEq, Eq)]
enum MenuStep {
    Close,
    Goto { category: usize, item: usize },
    Run { category: usize, item: usize },
    Ignore,
}

fn wrap(category: usize, delta: isize) -> usize {
    let n = MENUBAR_CATEGORIES.len() as isize;
    (category as isize + delta).rem_euclid(n) as usize
}

fn step_for(key: KeyEvent, category: usize, item: usize) -> MenuStep {
    let category = category.min(MENUBAR_CATEGORIES.len() - 1);
    let last_item = MENUBAR_CATEGORIES[category].items.len().saturating_sub(1);
    let item = item.min(last_item);
    let goto_cat = |category: usize| MenuStep::Goto { category, item: 0 };
    match key.code {
        KeyCode::Esc => MenuStep::Close,
        KeyCode::Left | KeyCode::BackTab => goto_cat(wrap(category, -1)),
        KeyCode::Right | KeyCode::Tab => goto_cat(wrap(category, 1)),
        KeyCode::Up => MenuStep::Goto { category, item: item.saturating_sub(1) },
        KeyCode::Down => MenuStep::Goto { category, item: (item + 1).min(last_item) },
        KeyCode::Home => MenuStep::Goto { category, item: 0 },
        KeyCode::End => MenuStep::Goto { category, item: last_item },
        KeyCode::Enter => MenuStep::Run { category, item },
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            let category_level = key.modifiers.contains(KeyModifiers::ALT) || c.is_ascii_uppercase();
            if !category_level && let Some(i) = item_by_mnemonic(category, c) {
                return MenuStep::Run { category, item: i };
            }
            match category_by_mnemonic(c) {
                Some(cat) => goto_cat(cat),
                None => MenuStep::Ignore,
            }
        }
        _ => MenuStep::Ignore,
    }
}

/// Handles a key while the menu bar is open. Always consumes the key.
pub(super) fn handle_menubar_key(wm: &mut WindowManager, key: KeyEvent) -> Result<Option<AppAction>> {
    let Some((category, item)) = wm.menubar_state() else { return Ok(None) };
    match step_for(key, category, item) {
        MenuStep::Close => wm.close_modal(),
        MenuStep::Goto { category, item } => {
            if let Modal::Menubar { category: c, item: i } = wm.modal_mut() {
                *c = category;
                *i = item;
            }
        }
        MenuStep::Run { category, item } => {
            return super::modal::execute_menubar_selection(wm, category, item).map(Some);
        }
        MenuStep::Ignore => {}
    }
    Ok(Some(AppAction::Handled))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrows_wrap_over_the_whole_table() {
        let n = MENUBAR_CATEGORIES.len();
        assert_eq!(step_for(k(KeyCode::Right), n - 1, 3), MenuStep::Goto { category: 0, item: 0 });
        assert_eq!(step_for(k(KeyCode::Left), 0, 3), MenuStep::Goto { category: n - 1, item: 0 });
    }

    #[test]
    fn item_mnemonic_beats_category_and_uppercase_addresses_categories() {
        // `S` is both the System category and "Stack Windows" in Window.
        let window = category_by_mnemonic('W').unwrap();
        assert_eq!(step_for(k(KeyCode::Char('s')), window, 0), MenuStep::Run { category: window, item: 3 });
        let up = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT);
        assert_eq!(step_for(up, window, 0), MenuStep::Goto { category: 0, item: 0 });
    }
}
