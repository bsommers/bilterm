---
phase: 50-agent-aware-workspace-navigator
plan: 13
subsystem: ui
tags: [gui, x11, mouse-modifiers, ctrl-wheel, panel-scale]
status: complete

requires:
  - phase: 50-16
    provides: "PanelTarget scale, set_gui_surface, panel_space"
provides:
  - "src/input/gui_route.rs: route_gui_wheel, GuiWheelRoute, modifiers_from_x11_state (pure, all OSes)"
  - "X11 runner: GUI surface enabled, real mouse modifiers, Ctrl+wheel over drawer/:mcp/agents window scales that panel"
  - "Win32 and macOS runners mark the WindowManager as a GUI surface"
affects: [50-14]

actuals:
  tokens: 30000
  tasks: 3
  commits: 2
plan_head_before: 6fc2cc06d28e22562431cbaaf4b97afc8b254c5e
plan_head_after: 2fdb7f66ee6b3d997d02d6d64c3493369e8283d6

key-files:
  created:
    - src/input/gui_route.rs
    - tests/phase50_gui_runtime.rs
  modified:
    - src/input/mod.rs
    - src/ui/gui_font/linux_x11.rs
    - src/ui/gui_window.rs
    - src/ui/gui_macos.rs

key-decisions:
  - "Decision record (Task 1, blocking-human): the repo owner chose `fix-linux` via AskUserQuestion. X11 is fully wired; Win32 and macOS only get set_gui_surface(true)."
  - "route_gui_wheel hit-tests each panel in its own scaled panel space, so scaled panels route correctly"

---

# Phase 50 Plan 13: GUI runtime scope (fix-linux) Summary

On real X11 GUI windows, Ctrl+wheel over the Navigator drawer, `:mcp` console or agents window now scales that panel. Ctrl+wheel anywhere else still zooms the font. Shift/Alt/Ctrl now reach mouse handlers. Win32 and macOS only got the one-line GUI-surface enablement.

## Decision record (Task 1)

Option `fix-linux`, chosen by the repo owner at the blocking-human gate. Scope: X11 fully fixed (modifiers, Ctrl+wheel panel routing, panel scale enabled). Win32 and macOS: `set_gui_surface(true)` only.

## What changed

- `src/input/gui_route.rs`: `route_gui_wheel(wm, col, row, up, mods, viewport)` returns `Forward(MouseEvent)` (no Ctrl, or Ctrl over a panel, CONTROL kept) or `FontZoom { up }` (Ctrl elsewhere). `modifiers_from_x11_state` maps ShiftMask/ControlMask/Mod1Mask.
- `linux_x11.rs`: `app.wm.set_gui_surface(true)` once at loop start; every MouseEvent (down, up, motion, wheel) now carries `modifiers_from_x11_state(state)`; buttons 4/5 go through `route_gui_wheel`, `FontZoom` keeps the previous focus-under-pointer + `FontZoomIn/Out` code.
- `gui_window.rs` (`#[cfg(windows)]`) and `gui_macos.rs`: `set_gui_surface(true)` at runner start. `[` / `]` and `:... scale` now apply there.
- `tests/phase50_gui_runtime.rs`: 7 `gui_runtime_` tests (drawer forward+scale, agents window scale, pane font zoom, plain wheel modifiers, X11 mask mapping, scaled-panel hit in panel space, platform-neutral purity).

## Known gap (for plan 50-14 docs)

On Windows and macOS, Ctrl+wheel (Ctrl/Cmd on macOS) over a panel still zooms the whole terminal font, and mouse events still carry empty modifiers. Panel scale is reachable there only via `[` / `]` and `:panel scale` / `:drawer scale`. A future change should route those two runners through `route_gui_wheel` (Win32: `WM_MOUSEWHEEL` Ctrl branch + MK_SHIFT/MK_CONTROL/GetKeyState; macOS: scroll branch).

## Cross-target compile check

`cargo check -q --lib --target x86_64-pc-windows-gnu` and `--target aarch64-apple-darwin` both succeed (exit 0), before and after the Win32/macOS edit (pre-existing unrelated warnings in `src/api/http.rs` on the Windows target). Baseline was taken after the X11 task commit, which touches no code compiled on those targets besides the pure `gui_route` module.

## Verification

- `cargo test --test phase50_gui_runtime`: 7 passed. `cargo clippy --all-targets -- -D warnings`: clean.
- `scripts/verify.sh`: PASS 58/58. `scripts/check_classic_goldens.sh`: GOLDENS OK 11. `Cargo.toml`/`Cargo.lock` unchanged.
- Acceptance greps: `set_gui_surface(true)` count is 1 in each of linux_x11.rs, gui_window.rs, gui_macos.rs; `route_gui_wheel` in linux_x11.rs (2 occurrences: import and call).

## Unverified (needs a desktop session)

- Real X11 window behaviour (Ctrl+wheel over drawer / agents window in `bilterm --gui`): not run here; covered only by the pure routing tests.
- Windows and macOS launch: compile-checked only; the plan's human-check applies only under `fix-all`, which was not chosen.

## Deviations from Plan

- Cargo check baseline on the unmodified tree was not captured separately from Task 2 (see above); both targets compile.
- Otherwise none. No stubs, no new threat surface.

## Self-Check: PASSED
