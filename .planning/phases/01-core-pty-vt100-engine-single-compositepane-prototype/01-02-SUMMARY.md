---
phase: 01-core-pty-vt100-engine-single-compositepane-prototype
plan: 02
subsystem: pane-routing
tags: [rust, termios, tcgetpgrp, osc-133, composite-pane, input-routing]

requires:
  - phase: 01-core-pty-vt100-engine-single-compositepane-prototype
    provides: "PtySession and VtParser (01-01)"
provides:
  - "Three-signal line-discipline inspector (OSC 133 + tcgetpgrp + termios !ECHO/!ICANON)"
  - "CompositePane state and zero-SIGWINCH geometry manager"
  - "Modal keyboard router preventing cleartext password leaks during !ECHO"
affects: [01-03, phase-2, phase-3]

tech-stack:
  added: []
  patterns: ["Three-signal line-discipline inspection (OSC 133 + tcgetpgrp + tcgetattr)", "Zero-SIGWINCH overlay geometry"]

key-files:
  created: [src/pty/line_discipline.rs, src/pane/mod.rs, src/input/mod.rs]
  modified: [src/pty/mod.rs, src/lib.rs]

key-decisions:
  - "Evaluated 3-signal heuristic before every keystroke so sudo/ssh !ECHO prompts route straight to the PTY without storing characters in FloatingPromptState"
  - "Decoupled FloatingCommandPrompt anchor/visibility from OutputCanvas dimensions so prompt movement emits zero SIGWINCH"

patterns-established:
  - "CompositePane::effective_routing_mode unifying DECSET 47/1047/1049, 3-signal line discipline, and manual raw toggle"

requirements-completed: [PTY-02, PTY-03, PANE-01]

coverage:
  - id: D1
    description: "Three-signal line-discipline detection and password-safe input routing"
    requirement: PTY-03
    verification:
      - kind: unit
        ref: "src/input/mod.rs#password_keystrokes_in_no_echo_mode_never_leak_into_floating_prompt"
        status: pass
    human_judgment: false
  - id: D2
    description: "CompositePane zero-SIGWINCH geometry and DECSET 1049 prompt auto-hide"
    requirement: PANE-01
    verification:
      - kind: unit
        ref: "src/pane/mod.rs#moving_or_hiding_prompt_never_resizes_canvas_or_emits_sigwinch"
        status: pass
    human_judgment: false

duration: 2min
completed: 2026-10-20
status: complete
---

# Phase 1 Plan 02: Line-Discipline Inspector, CompositePane Geometry & Modal Input Router Summary

**Three-signal line-discipline detector (`OSC 133` + `tcgetpgrp` + `termios !ECHO`), `CompositePane` zero-`SIGWINCH` geometry manager, and modal keyboard router.**

## Performance

- **Duration:** 2 min
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Implemented `src/pty/line_discipline.rs` combining `OSC 133` state, POSIX `tcgetpgrp`, and `tcgetattr` (`ECHO`/`ICANON`) with real Unix PTY termios tests.
- Implemented `src/pane/mod.rs` with `CompositePane`, `FloatingPromptState`, `PromptAnchor` (`Top`, `Bottom`, `Floating`), zero-`SIGWINCH` viewport resizing, and automatic prompt hiding on `DECSET 47/1047/1049` or `!ECHO`.
- Implemented `src/input/mod.rs` routing keys between `FloatingPromptState` and raw VT100/xterm PTY byte encoding.

## Files Created/Modified
- `src/pty/line_discipline.rs` - `LineDisciplineState`, `InputRoutingMode`, `inspect_pty_fd`
- `src/pane/mod.rs` - `CompositePane`, `FloatingPromptState`, `PromptAnchor`
- `src/input/mod.rs` - `AppAction`, `handle_key_event`, `encode_key_to_pty_bytes`

## Decisions Made
- Handled modern interactive shells (`bash` readline / `zsh` ZLE) that disable `ICANON` at their own prompt by comparing `foreground_pgid` against `shell_pgid` alongside `OSC 133` and `ECHO`.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## Next Phase Readiness
- Ready for Plan `01-03` (3-pass Ratatui `CompositePaneWidget` compositor, async Tokio `App` loop, and `TestBackend` + `insta` snapshot tests).
