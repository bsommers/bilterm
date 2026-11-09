//! Mouse selection routing policy for panes running nested TUIs with mouse tracking (`COPY-01`).

use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

/// Local selection mode when the mouse event is handled locally by Bilterm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSelect {
    /// Normal character-stream selection.
    Stream,
    /// Rectangular block selection.
    Block,
}

/// Destination for a mouse event targeting a terminal pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseRoute {
    /// Forward the mouse event to the child PTY process (encoded as SGR/DECSET).
    ToChild,
    /// Handle the mouse event locally within Bilterm as text selection.
    Local(LocalSelect),
    /// Handle as clipboard paste (Phase 57 Shift+Right/Middle click).
    Paste,
}

/// User configuration policy for which modifiers override child mouse reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverridePolicy {
    #[default]
    ShiftAndAlt,
    ShiftOnly,
    AltOnly,
    Off,
}

impl OverridePolicy {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "shift_alt" | "shiftandalt" | "all" => OverridePolicy::ShiftAndAlt,
            "shift" | "shiftonly" => OverridePolicy::ShiftOnly,
            "alt" | "altonly" => OverridePolicy::AltOnly,
            "off" | "none" => OverridePolicy::Off,
            _ => OverridePolicy::ShiftAndAlt,
        }
    }

    pub fn allows_shift(self) -> bool {
        matches!(self, OverridePolicy::ShiftAndAlt | OverridePolicy::ShiftOnly)
    }

    pub fn allows_alt(self) -> bool {
        matches!(self, OverridePolicy::ShiftAndAlt | OverridePolicy::AltOnly)
    }
}

/// Pure routing decision for a mouse event.
///
/// Rules evaluated in order:
/// 1. `Shift + Down(Right|Middle)` -> `Paste` (preserves Phase 57 paste bypass).
/// 2. If copy mode is already active, a drag is in progress, or rect mode is active -> `Local`.
/// 3. Scroll events are never overridden by `Shift`/`Alt` -> `ToChild`.
/// 4. Sticky per-pane mouse select toggle -> `Local(Stream)`.
/// 5. `Alt` held and permitted by policy -> `Local(Block)`.
/// 6. `Shift` held and permitted by policy -> `Local(Stream)`.
/// 7. Otherwise -> `ToChild`.
#[allow(clippy::too_many_arguments)]
pub fn route(
    mods: KeyModifiers,
    kind: MouseEventKind,
    tracking: bool,
    _raw_passthrough: bool,
    copy_active: bool,
    rect_mode: bool,
    drag_in_progress: bool,
    sticky: bool,
    policy: OverridePolicy,
) -> MouseRoute {
    // 1. Shift + Right/Middle click -> Paste
    if mods.contains(KeyModifiers::SHIFT)
        && matches!(
            kind,
            MouseEventKind::Down(MouseButton::Right | MouseButton::Middle)
        )
    {
        return MouseRoute::Paste;
    }

    // 2. Active selection/drag/rect continuation
    if copy_active || drag_in_progress || rect_mode {
        return if rect_mode {
            MouseRoute::Local(LocalSelect::Block)
        } else {
            MouseRoute::Local(LocalSelect::Stream)
        };
    }

    // 3. Scroll events are never overridden into selection
    if matches!(kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
        return MouseRoute::ToChild;
    }

    // 4. Sticky per-pane override
    if sticky {
        return MouseRoute::Local(LocalSelect::Stream);
    }

    // 5. Alt modifier override -> Block selection
    if mods.contains(KeyModifiers::ALT) && policy.allows_alt() {
        return MouseRoute::Local(LocalSelect::Block);
    }

    // 6. Shift modifier override -> Stream selection
    if mods.contains(KeyModifiers::SHIFT) && policy.allows_shift() {
        return MouseRoute::Local(LocalSelect::Stream);
    }

    // 7. If child owns mouse tracking, forward to child; otherwise default to local selection
    if tracking {
        MouseRoute::ToChild
    } else {
        MouseRoute::Local(LocalSelect::Stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_priorities() {
        // Shift + Right click pastes
        assert_eq!(
            route(
                KeyModifiers::SHIFT,
                MouseEventKind::Down(MouseButton::Right),
                true,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::Paste
        );

        // Plain drag over tracking child goes to child
        assert_eq!(
            route(
                KeyModifiers::NONE,
                MouseEventKind::Drag(MouseButton::Left),
                true,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::ToChild
        );

        // Plain drag over non-tracking child is local
        assert_eq!(
            route(
                KeyModifiers::NONE,
                MouseEventKind::Drag(MouseButton::Left),
                false,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::Local(LocalSelect::Stream)
        );

        // Shift drag selects stream locally
        assert_eq!(
            route(
                KeyModifiers::SHIFT,
                MouseEventKind::Drag(MouseButton::Left),
                true,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::Local(LocalSelect::Stream)
        );

        // Alt drag selects block locally
        assert_eq!(
            route(
                KeyModifiers::ALT,
                MouseEventKind::Drag(MouseButton::Left),
                true,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::Local(LocalSelect::Block)
        );

        // Sticky toggle makes plain drag local stream
        assert_eq!(
            route(
                KeyModifiers::NONE,
                MouseEventKind::Drag(MouseButton::Left),
                true,
                true,
                false,
                false,
                false,
                true,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::Local(LocalSelect::Stream)
        );

        // Scroll events go to child even with Shift
        assert_eq!(
            route(
                KeyModifiers::SHIFT,
                MouseEventKind::ScrollUp,
                true,
                true,
                false,
                false,
                false,
                false,
                OverridePolicy::ShiftAndAlt,
            ),
            MouseRoute::ToChild
        );
    }
}
