---
phase: 03-window-manager-h-v-composite-splits-z-order-stacking-shuffling-guake-drop-down-terminal
plan: 02
subsystem: wm-coordinator-quick-terminal-sgr-mouse
tags: [rust, window-manager, guake-quick-terminal, sgr-1006-mouse]
provides:
  - Persistent Guake/Quake-style top-anchored `QuickTerminal` (`src/wm/quick_terminal.rs`)
  - Multi-pane `WindowManager` coordinator (`src/wm/mod.rs`)
  - SGR (1006) local mouse coordinate translation (`encode_sgr_mouse_local`) and multi-pane `:command`/keyboard routing
affects: [src/wm/quick_terminal.rs, src/wm/mod.rs, src/pane/colon_command.rs, src/input/mod.rs]
tech-stack:
  added: []
  patterns: [Persistent Drop-Down Overlay, Reverse Z-Order Mouse Hit-Testing, SGR 1006 Coordinate Translation]
key-files:
  created: [src/wm/quick_terminal.rs, src/wm/mod.rs]
  modified: [src/pane/colon_command.rs, src/input/mod.rs, src/lib.rs]
key-decisions:
  - "Assigned PaneId 0 to the persistent QuickTerminal so toggling visibility via Alt+` / Ctrl+Space / :quake never terminates its underlying PTY session"
requirements-completed: [WM-01, WM-02, WM-03, WM-05]
duration: 4min
completed: 2026-10-20
---

# Phase 3 Plan 02: Multi-Pane WindowManager, Guake Drop-Down Terminal & SGR Mouse Routing Summary

**Implemented `QuickTerminal` (`src/wm/quick_terminal.rs`), `WindowManager` (`src/wm/mod.rs`), and SGR 1006 local mouse translation.**
