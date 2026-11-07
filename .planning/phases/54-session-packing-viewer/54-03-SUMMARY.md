---
phase: 54-session-packing-viewer
plan: 03
subsystem: session map input, drag-and-drop, surface parity
tags: [drag-and-drop, keyboard-workflow, parity, rpc, mcp, VIEW-03, VIEW-04, UNI-R01]
requires: [54-01, 54-02]
provides:
  - "plan_session_map_mouse, hit_test_session_map (src/input/session_map.rs)"
  - "SESSION_MAP_BINDINGS, handle_session_map_key (src/input/session_map_keys.rs)"
  - "rpc_session_map, bilterm_session_map (src/api/rpc_session_map.rs, src/api/rpc.rs, src/api/mcp.rs)"
affects: []
key-files:
  created: [src/input/session_map.rs, src/input/session_map_keys.rs, src/api/rpc_session_map.rs, tests/phase54_drag_drop_parity.rs]
  modified: [src/input/mod.rs, src/input/modal.rs, src/input/mouse.rs, src/input/keymap.rs, src/input/gui_route.rs, src/api/mod.rs, src/api/rpc.rs, src/api/mcp.rs, src/pane/completion.rs, src/command_registry.rs, src/ui/help_catalog.rs]
decisions:
  - "Pure mouse planner and declarative key handler for zero-side-effect gesture resolution"
  - "Full surface uniformity across Mouse, Keyboard, Colon Commands (:sessions map / :packing), and Automation JSON-RPC (session.map) / MCP"
  - "Invalid drop gestures safely cancel and preserve state invariants"
status: complete
---

# Phase 54 Plan 03: Mouse Drag-and-Drop, Keyboard Equivalents & Surface Parity Summary

Implemented mouse drag-and-drop hit-testing and planning (`src/input/session_map.rs`, 295 lines), keyboard workflow (`src/input/session_map_keys.rs`, 89 lines), JSON-RPC and MCP exposure (`src/api/rpc_session_map.rs`, 109 lines), and surface parity test suite (`tests/phase54_drag_drop_parity.rs`, 4 passed).

## Gates (verbatim)

- `cargo test -q --test phase54_drag_drop_parity`: 4 passed
- `cargo test -q --lib`: 319 passed
- `cargo clippy -q --all-targets -- -D warnings`: clean (rc 0)
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `handle_wm_key_event`: 53 lines (<= 80)
