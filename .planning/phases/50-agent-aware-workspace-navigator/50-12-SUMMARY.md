---
phase: 50-agent-aware-workspace-navigator
plan: 12
subsystem: ui
tags: [navigator, drawer, comms-indicator, traffic, keymap]
status: complete

requires:
  - phase: 50-05
    provides: "partners_of, traffic bus"
  - phase: 50-10
    provides: "NodeKey, open_agents_window_on_edge, apply_agent_filter"
  - phase: 50-15
    provides: "handle_navigator_drawer_mouse, navigator_row_at"
provides:
  - "CommsIndicator {Pair, FanOut} + NavigatorRowMeta.comms / comms_active"
  - "NavigatorDrawerState.comms_focus on WorkspaceDrawer.navigator"
  - "DrawerNavOp::OpenComms (:drawer comms) and CommsFocus(bool) (:drawer comms-focus on|off)"
  - "DrawerKeyScope::NavigatorComms; rows Right, o, Enter, Left, Esc in NAVIGATOR_DRAWER_BINDINGS"
  - "ui::drawer::navigator::navigator_comms_cells hit-test helper"
affects: [50-14]

actuals:
  tokens: 40000
  tasks: 2
  commits: 1
plan_head_before: 2fd68f42310ecf6d43e45da5df7533d67e220362
plan_head_after: 0ffdbc93d28a5a30c2234afce8df4746ff93a9fa

key-files:
  created: []
  modified:
    - src/wm/drawer/navigator.rs
    - src/wm/drawer/nav.rs
    - src/wm/drawer/menu.rs
    - src/wm/drawer.rs
    - src/wm/agent_state.rs
    - src/ui/drawer/navigator.rs
    - src/input/drawer_navigator.rs
    - src/input/keymap.rs
    - src/colon_command/parse_visual.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - tests/phase50_navigator_drawer.rs

key-decisions:
  - "Comms focus follows the selection: apply_drawer_nav clears comms_focus whenever it no longer equals the selected row, so moving away and back does not re-arm it"
  - "The whole indicator (glyph plus partner label) is the click target, not just the glyph cell"
  - "tick_agent_ui treats a flowing comms glyph like a Working spinner (same 120 ms throttle); the check covers all pane rows, not only scrolled-into-view ones"
---

# Phase 50 Plan 12: Navigator comms indicator Summary

Each live Navigator pane row now shows who it is talking to: `⇄you` for exactly one partner within 60 s, `⋔3` for several, right-aligned. The glyph alternates `⇄`/`⇆` while the last event is under 1.5 s old, and click, `o`, Right+Enter or `:drawer comms` all open the traffic window on that conversation.

## What changed

- `navigator.rs`: `CommsIndicator`, `comms`/`comms_active` on row meta, computed from `partners_of(id, 60 s, tracker last update)` so rendering stays deterministic. Partner label is `sanitize_display(label, 6)`. `NavigatorDrawerState { comms_focus }` plus `navigator_comms_focused()` and `navigator_comms_active()`.
- `ui/drawer/navigator.rs`: `comms_fit` is the single layout routine for the renderer and `navigator_comms_cells`. Truncation order: title first (down to indent + dot + `#id`), then the partner label, never the glyph. ASCII pack: `<>`/`><`, `>>`+count. Focused glyph is underlined and inverted.
- `nav.rs`: `OpenComms` (Pair -> edge focus + pair filter; FanOut -> pane select + agent filter; no indicator -> select pane) and `CommsFocus(bool)`.
- `keymap.rs` / `drawer_navigator.rs`: new `NavigatorComms` scope (active only while the glyph has focus). Right is a claim guard: on a row without comms it returns `None` so the classic hscroll / leave-drawer behaviour is unchanged. Mouse click on the indicator lowers to `Row(i)` then `OpenComms`.
- `agent_state.rs`: `tick_agent_ui` redraws at the 120 ms cadence while any pane row has a flowing glyph.
- Parser, completion (`comms`, `comms-focus `), help rows (both tables plus the keys line).

## Tests

10 `comms_` tests (tracer + 9 behaviour) pass, all via real key events, mouse events and colon commands. `keys_colon_equivalents_match_keys` now sets up traffic and glyph focus for `NavigatorComms` rows (otherwise Esc/Left/Enter would be compared against a state where they are not claimed) and `observe` records the agents window and comms focus.

Verification: `scripts/verify.sh` PASS 58/58; plan-scoped stages PASS; `scripts/check_classic_goldens.sh` prints `GOLDENS OK 11`; `drawer_classic_parity` 11/11; the `wm.` allow-list gate on `drawer_navigator.rs` is clean; `Cargo.toml`/`Cargo.lock` unchanged.

## Deviations from Plan

**1. [Rule 3 - Blocking] Tasks 1 and 2 share one commit.** The comms ops, keymap rows, `drawer.rs` field and renderer are interleaved across the same files, and the tracer alone would not compile without the `navigator` field. Commit `0ffdbc9` holds both tasks.

**2. [Rule 3] `src/wm/drawer.rs` line cap not met.** Acceptance says `wc -l <= 1034`; the file was already 1044 at the start of this plan (50-15 added to the 1032 cap). I moved `WorkspaceDrawer::navigator_menu_open` into `drawer/menu.rs` and added the single `navigator` field line plus widened the existing `pub use navigator::{...}` line to export `CommsIndicator`: now 1040 lines. The 1200-line hard backstop holds. Remaining 6 lines would need moving 50-15's `nav_menu` into the new state struct.

**3. [Rule 1] Clippy.** `>= y + 1` and `% 2 == 0` rewritten (`>`, `is_multiple_of`).

## Unverified

Nothing was run in a desktop session. The GUI and TUI draw the indicator from the same cell buffer (`phase50_gui_render` passes), but real-window click and animation behaviour is unobserved.

## Known Stubs

None.

## Threat Flags

None. T-50-28 (label sanitised, kind decided by producer; U+202E test passes), T-50-29 (aggregation bounded by the 512-event ring, redraw throttled to 120 ms), T-50-SC (no dependency changes) are mitigated as planned.

## Self-Check: PASSED

Commit `0ffdbc9` is an ancestor of HEAD; all modified files exist; `comms_` tests, full verify and goldens pass.
