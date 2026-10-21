---
phase: quick
plan: 20261020-tab-completion-and-smooth-floating-prompt
status: complete
completed: 2026-10-20
---

# Quick Task Summary: Tab Completion & Smooth Floating Prompt Over `tail -f /dev/random`

## Delivered
1. **Tab Completion Engine ([`src/pane/completion.rs`](file:///home/bill/src/ai/bilterm/src/pane/completion.rs), [`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Completes `:colon` commands (`:prompt`, `:split`, `:deck`, `:layout`, `:block`, etc.) and subarguments.
   - Completes shell builtins and `$PATH` executables in command position.
   - Completes `$ENV` and `${ENV}` variables.
   - Completes relative, absolute (`/dev/ran...` -> `/dev/random`), and `~/` filesystem paths resolved against the child shell's live `/proc/<pid>/cwd` directory.
   - Expands Longest Common Prefix (LCP) on first `Tab`, displays an in-prompt `TAB:` candidate ribbon bar when multiple matches exist, and cycles forward/backward via `Tab` / `Shift+Tab`.
2. **Smooth Floating Prompt Over High-Throughput Streaming ([`src/pty/line_discipline.rs`](file:///home/bill/src/ai/bilterm/src/pty/line_discipline.rs), [`src/vt/mod.rs`](file:///home/bill/src/ai/bilterm/src/vt/mod.rs), [`src/pty/mod.rs`](file:///home/bill/src/ai/bilterm/src/pty/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [`src/app.rs`](file:///home/bill/src/ai/bilterm/src/app.rs))**:
   - Fixed `LineDisciplineState::requires_raw_passthrough()` so cooked-mode streaming foreground commands (`echo_enabled && canonical_mode`, e.g. `tail -f /dev/random`, `ping`, `watch`, `yes`) keep the `FloatingCommandPrompt` visible on top of the scrolling background.
   - Added `is_binary_noise_chunk` + `sanitize_binary_noise_chunk` in `VtParser::process` so random binary streams (`tail -f /dev/random`) cannot falsely trigger `DECSET 47/1047/1049` alternate screen mode or lock the VT100 scroll region (`DECSTBM`).
   - Added `is_safe_terminal_cluster` cell sanitization in `render_output_canvas` to block C0/C1 control codes, bidi/format characters, and right-edge wide-character wrapping from corrupting the host terminal's cursor during differential rendering.
   - Bounded PTY channel buffering (`32` chunks), coalesced per-frame byte processing (`32 KB` tail cap), and configured `MissedTickBehavior::Skip` for locked 60 FPS responsiveness and instant `Ctrl+C` (`SIGINT`) interruption.

## Verification
- `cargo test && cargo clippy --all-targets -- -D warnings`: 52/52 tests passing (31 unit + 21 integration/snapshot), 0 warnings.
