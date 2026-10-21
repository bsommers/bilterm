---
phase: quick
plan: 20261020-tab-completion-and-smooth-floating-prompt
type: execute
wave: 1
depends_on: []
files_modified:
  - src/pane/completion.rs
  - src/pane/mod.rs
  - src/pty/line_discipline.rs
  - src/pty/mod.rs
  - src/input/mod.rs
  - src/ui/mod.rs
  - src/app.rs
  - tests/tab_completion_and_streaming.rs
autonomous: true
---

# Quick Plan: Tab Completion & Smooth Floating Prompt Over High-Throughput Streaming (`tail -f /dev/random`)

## Objective
1. Add full **Tab Completion (`Tab` / `Shift+Tab`)** to the `FloatingCommandPrompt` supporting:
   -Filesystem path completion (relative, absolute `/dev/ran...`, `~/` home expansion, and live child shell `/proc/<pid>/cwd` resolution)
   - `$PATH` executable and shell builtin completion on command tokens
   - `$ENV` variable completion
   - Bilterm `:colon` command completion (`:prompt`, `:split`, `:deck`, etc.)
   - Longest-common-prefix (LCP) expansion + interactive `Tab` / `Shift+Tab` candidate cycling with an in-prompt candidate bar.
2. Ensure the `FloatingCommandPrompt` **floats smoothly over high-throughput scrolling backgrounds** like `tail -f /dev/random`:
   - Fix `LineDisciplineState::requires_raw_passthrough()` so streaming cooked-mode foreground commands (`ECHO && ICANON`, e.g. `tail -f`, `ping`, `watch`, `yes`) keep the floating command prompt visible (`CommandPrompt` mode) instead of hiding it.
   - Bound per-frame PTY chunk draining (`MAX_EVENTS_PER_POLL`) and coalesce `refresh_line_discipline()` to once per poll so `tail -f /dev/random` never starves the Tokio render/input loop.
   - Sanitize `OutputCanvas` cells in `render_output_canvas` against C0/C1 control codes (`0x00..0x1F`, `0x7F..0x9F`), format/bidi characters, and right-edge wide-character overflow so random binary streams cannot corrupt the host terminal cursor or tear the floating prompt during differential rendering.

## Tasks
1. **RED (TDD)**: Create `tests/tab_completion_and_streaming.rs` testing:
   - Filesystem path completion (`/dev/ran` -> `/dev/random`, relative paths, directory trailing `/`)
   - Multi-match LCP + `Tab` / `Shift+Tab` cycling
   - Colon command completion (`:pr` -> `:prompt `)
   - Streaming foreground process (`foreground_pgid != shell_pgid` with `ECHO && ICANON`) keeping `is_prompt_visible() == true`
   - High-throughput random binary byte stream (`tail -f /dev/random` simulation with C0/C1/wide/escape bytes) preserving the floating command prompt intact and sanitizing all canvas cells.
2. **GREEN**: Implement `src/pane/completion.rs`, wire `Tab`/`BackTab` in `src/pane/mod.rs`, `src/input/mod.rs`, and `src/ui/mod.rs`, and harden `src/pty/line_discipline.rs`, `src/pty/mod.rs`, `src/ui/mod.rs`, and `src/app.rs`.
3. **VERIFY**: Run `cargo test` and `cargo clippy --all-targets -- -D warnings`.
