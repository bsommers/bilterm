---
phase: 01-core-pty-vt100-engine-single-compositepane-prototype
plan: 01
subsystem: pty-vt100
tags: [rust, ratatui, portable-pty, vt100-ctt, decset-1049, osc-133]

requires: []
provides:
  - "Cross-platform PTY session lifecycle with deduplicated SIGWINCH resizing (PtySession)"
  - "Panic and signal-safe terminal restoration guard (TerminalRestoreGuard)"
  - "VT100/ANSI/TrueColor state machine with DECSET 47/1047/1049 and OSC 133 marker tracking (VtParser)"
affects: [01-02, 01-03, phase-2, phase-3]

tech-stack:
  added: [ratatui, crossterm, portable-pty, vt100-ctt, tokio, libc, color-eyre, thiserror, tracing, unicode-width, insta]
  patterns: ["Deduplicated PTY resize guarding against SIGWINCH thrashing", "Pre-scanned DECSET 47/1047/1049 and OSC 133 control sequence hooks"]

key-files:
  created: [Cargo.toml, src/lib.rs, src/error.rs, src/pty/mod.rs, src/vt/mod.rs, tests/pty_lifecycle.rs]
  modified: [.gitignore]

key-decisions:
  - "Wrapped vt100-ctt::Parser in VtParser with explicit DECSET 47/1047/1049 and OSC 133;A/B/C/D tracking"
  - "Guarded PtySession::resize with current_size comparison so moving/hiding overlays emits zero SIGWINCH"

patterns-established:
  - "PtySession::sigwinch_count atomic counter for deterministic verification of zero-SIGWINCH invariants"

requirements-completed: [PTY-01, PTY-02]

coverage:
  - id: D1
    description: "Cross-platform PTY lifecycle and deduplicated SIGWINCH resize"
    requirement: PTY-01
    verification:
      - kind: integration
        ref: "tests/pty_lifecycle.rs#pty_spawns_shell_executes_command_and_deduplicates_sigwinch"
        status: pass
    human_judgment: false
  - id: D2
    description: "VT100/ANSI/TrueColor and DECSET 47/1047/1049 + OSC 133 state machine"
    requirement: PTY-02
    verification:
      - kind: unit
        ref: "src/vt/mod.rs#tracks_decset_1049_1047_and_47_alternate_screen"
        status: pass
    human_judgment: false

duration: 2min
completed: 2026-10-19
status: complete
---

# Phase 1 Plan 01: Core PTY & VT100 Engine Summary

**Cross-platform `portable-pty` session manager with deduplicated `SIGWINCH` resizing, `TerminalRestoreGuard` panic recovery, and `VtParser` supporting 24-bit TrueColor, scrollback, `DECSET 47/1047/1049` alternate screen detection, and `OSC 133` markers.**

## Performance

- **Duration:** 2 min
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Created `Cargo.toml` and core library scaffolding (`src/lib.rs`, `src/error.rs`, `src/pty/mod.rs`, `src/vt/mod.rs`).
- Implemented `PtySession` with dedicated non-blocking reader thread, `sigwinch_count` tracking, and deduplicated `resize(rows, cols)`.
- Implemented `VtParser` with 16/256/TrueColor cell styling, scrollback navigation, `DECSET 47/1047/1049` alternate screen detection, and `OSC 133;A/B/C/D` command block tracking.
- Verified with unit tests and headless shell integration test (`tests/pty_lifecycle.rs`).

## Files Created/Modified
- `Cargo.toml` - Workspace crate manifest and dependencies
- `src/error.rs` - `BiltermError` and `TerminalRestoreGuard`
- `src/pty/mod.rs` - `PtyConfig`, `PtyEvent`, and `PtySession`
- `src/vt/mod.rs` - `VtParser`, `Osc133Phase`, `CommandBlockRecord`, `cell_to_ratatui_style`
- `tests/pty_lifecycle.rs` - Headless PTY shell execution and `SIGWINCH` deduplication test

## Decisions Made
- Combined `vt100-ctt` screen state with an explicit control sequence scanner for `DECSET 47/1047/1049` and `OSC 133;A/B/C/D` so all three alternate screen modes and semantic shell prompt markers are tracked deterministically.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## Next Phase Readiness
- Ready for Plan `01-02` (three-signal line-discipline inspector, `CompositePane` zero-`SIGWINCH` geometry, and modal input router).
