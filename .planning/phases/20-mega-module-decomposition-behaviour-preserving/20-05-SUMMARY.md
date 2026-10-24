---
phase: 20-mega-module-decomposition-behaviour-preserving
plan: 05
subsystem: ui, wm
tags: [refactor, decomposition, ui, wm, modals, status-bar, session]
requires:
  - phase: 20-01
    provides: "decomposed crate::vt module and cell_to_ratatui_style in crate::ui"
  - phase: 20-02
    provides: "decomposed crate::pane and crate::colon_command modules"
  - phase: 20-03
    provides: "decomposed crate::input module"
  - phase: 20-04
    provides: "decomposed crate::api module"
provides:
  - "src/ui/modals.rs (728 lines): palette, history popup, unsafe paste, F1 help, Borland top menubar + dropdown, and F9 3270 inspector renderers"
  - "src/ui/status_bar.rs (728 lines): DOS function-key bar, floating command prompt renderer, and IBM 3270 OIA status bar renderers"
  - "src/ui/mod.rs (707 lines): CompositePaneWidget, render_output_canvas, WorkspaceWidget, buffer_to_snapshot_string, cell_to_ratatui_style"
  - "src/wm/session.rs (584 lines): SavedPaneState, SavedWorkspaceSession, session save/restore/list, SSH/fleet pane orchestration, and scrollback rebalancing"
  - "src/wm/mod.rs (632 lines): WindowManager core split/z-stack/focus/zoom/mouse geometry and hit-testing"
  - "All 58 .rs files under src/ verified <= 800 lines; ARCH-R04 marked Complete"
affects: [21, 22, 23]
tech-stack:
  added: []
  patterns: ["global <= 800 line module decomposition with zero behavioral changes"]
key-files:
  created:
    - src/ui/modals.rs
    - src/wm/session.rs
  modified:
    - src/ui/status_bar.rs
    - src/ui/mod.rs
    - src/wm/mod.rs
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
key-decisions:
  - "Placed render_floating_prompt, build_completion_bar_line, and render_fuzzy_search_contents alongside render_dos_fkey_bar and render_oia_status_bar* in src/ui/status_bar.rs (728 lines) so src/ui/mod.rs (707 lines), src/ui/modals.rs (728 lines), and src/ui/status_bar.rs (728 lines) are all <= 800 lines."
  - "Extracted SavedPaneState, SavedWorkspaceSession, session save/restore/list, SSH/fleet pane spawning, and scrollback rebalancing + unit tests into src/wm/session.rs (584 lines), bringing src/wm/mod.rs to 632 lines."
patterns-established:
  - "Hard <= 800 line ceiling enforced across every .rs source file in src/."
requirements-completed: [ARCH-R04]
duration: 7min
completed: 2026-10-23
---

# Phase 20 Plan 05: `ui` & `wm` Decomposition and Phase 20 Verification Summary

**Decomposed `src/ui/mod.rs` (2,058 lines) and `src/wm/mod.rs` (1,173 lines) so all 58 `.rs` files under `src/` are `<= 800` lines, verified `pane ↔ wm` cycle elimination, and passed the full test and `insta` snapshot suite**

## Performance

- **Duration:** 7 min
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Extracted modal and Borland top menubar renderers into `src/ui/modals.rs` (728 lines).
- Moved `render_oia_status_bar`, `render_oia_status_bar_themed`, `format_osc9_gauge`, and `render_floating_prompt` into `src/ui/status_bar.rs` (728 lines) with `pub use` re-exports in `src/ui/mod.rs` (707 lines).
- Extracted `SavedPaneState`, `SavedWorkspaceSession`, session save/restore/list, SSH/fleet pane methods, and scrollback rebalancing into `src/wm/session.rs` (584 lines), reducing `src/wm/mod.rs` to 632 lines.
- Verified global line count ceiling across all 58 `.rs` files in `src/` (maximum: `src/vt/mod.rs` at 800 lines).
- Marked `ARCH-R04` Complete in `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, and `.planning/STATE.md`.

## Verification

- `awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' $(find src -name '*.rs')` -> exit 0
- `test $(grep -rn "crate::wm\|crate::colon_command" src/pane/ | wc -l) -eq 0` -> exit 0
- `cargo clippy --all-targets -- -D warnings` -> 0 warnings
- `cargo test` -> all 138 unit tests and all integration + `insta` snapshot suites passed
