//! Key handling for the TRAFFIC MONITOR window (`TMON-04`, Phase 64-03). Table driven.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::AppAction;
use crate::action::Action;
use crate::traffic::GroupKey;
use crate::wm::WindowManager;
use crate::wm::traffic_monitor::{RateCol, TrafficMonitorOp as Op};

const N: KeyModifiers = KeyModifiers::NONE;
const A: KeyModifiers = KeyModifiers::ALT;
const S: KeyModifiers = KeyModifiers::SHIFT;

/// In-window keys while the filter box is NOT focused.
pub const TRAFFIC_MONITOR_BINDINGS: &[(KeyModifiers, KeyCode, Op)] = &[
    (N, KeyCode::Tab, Op::CycleGroup(1)),
    (N, KeyCode::BackTab, Op::CycleGroup(-1)),
    (S, KeyCode::BackTab, Op::CycleGroup(-1)),
    (N, KeyCode::Char('s'), Op::CycleSort(1)),
    (N, KeyCode::Char('S'), Op::ToggleSortDir),
    (N, KeyCode::Char('1'), Op::RateCol(RateCol::R1)),
    (N, KeyCode::Char('2'), Op::RateCol(RateCol::R10)),
    (N, KeyCode::Char('3'), Op::RateCol(RateCol::R60)),
    (N, KeyCode::Char('l'), Op::CycleLocality),
    (N, KeyCode::Char('p'), Op::Pause(None)),
    (N, KeyCode::Char('/'), Op::FilterFocus(true)),
    (N, KeyCode::Enter, Op::Drill),
    (N, KeyCode::Backspace, Op::DrillUp),
    (N, KeyCode::Char('x'), Op::Reset),
    (N, KeyCode::Char('o'), Op::Oia(None)),
    (N, KeyCode::Char('c'), Op::Classify(None)),
    (N, KeyCode::Char('P'), Op::Pin(None)),
    (N, KeyCode::Char('D'), Op::Dock(None)),
    (N, KeyCode::Up, Op::Step(-1)),
    (N, KeyCode::Char('k'), Op::Step(-1)),
    (N, KeyCode::Down, Op::Step(1)),
    (N, KeyCode::Char('j'), Op::Step(1)),
    (N, KeyCode::PageUp, Op::Page(-1)),
    (N, KeyCode::PageDown, Op::Page(1)),
    (N, KeyCode::Home, Op::First),
    (N, KeyCode::End, Op::Last),
    (N, KeyCode::Esc, Op::Dismiss),
    (N, KeyCode::Char('q'), Op::Dismiss),
    (A, KeyCode::Left, Op::Resize { dw: -2, dh: 0 }),
    (A, KeyCode::Right, Op::Resize { dw: 2, dh: 0 }),
    (A, KeyCode::Up, Op::Resize { dw: 0, dh: -1 }),
    (A, KeyCode::Down, Op::Resize { dw: 0, dh: 1 }),
    (S, KeyCode::Left, Op::Move { dx: -2, dy: 0 }),
    (S, KeyCode::Right, Op::Move { dx: 2, dy: 0 }),
    (S, KeyCode::Up, Op::Move { dx: 0, dy: -1 }),
    (S, KeyCode::Down, Op::Move { dx: 0, dy: 1 }),
];

/// Pure key-to-Action translation for the window when it captures input.
pub fn traffic_monitor_action(key: KeyEvent, filter_focused: bool) -> Option<Action> {
    let act = |op| Some(Action::TrafficMonitor(op));
    if filter_focused {
        return match key.code {
            KeyCode::Esc | KeyCode::Enter => act(Op::FilterFocus(false)),
            KeyCode::Backspace => act(Op::FilterBackspace),
            KeyCode::Tab => act(Op::CycleGroup(1)),
            KeyCode::BackTab => act(Op::CycleGroup(-1)),
            KeyCode::Up => act(Op::Step(-1)),
            KeyCode::Down => act(Op::Step(1)),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => act(Op::FilterClear),
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                act(Op::FilterChar(c))
            }
            _ => None,
        };
    }
    let mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    TRAFFIC_MONITOR_BINDINGS
        .iter()
        .find(|(m, c, _)| *m == mods && *c == key.code)
        .map(|(_, _, op)| Action::TrafficMonitor(op.clone()))
}

/// Group-by shortcut used by tests and docs: the group a `Tab` from `from` selects.
pub fn next_group(from: GroupKey) -> GroupKey {
    crate::wm::traffic_monitor::data::GROUP_NAMES
        .iter()
        .map(|(_, g)| *g)
        .cycle()
        .skip_while(|g| *g != from)
        .nth(1)
        .unwrap_or(from)
}

/// Applies `key` to `wm` when the window captures input.
pub(super) fn handle_traffic_monitor_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let action = traffic_monitor_action(key, wm.traffic_monitor().filter_focused)?;
    Some(wm.apply(action).unwrap_or(AppAction::Handled))
}
