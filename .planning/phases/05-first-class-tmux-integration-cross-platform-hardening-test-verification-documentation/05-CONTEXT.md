# Phase 5: First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation - Context

**Gathered:** 2026-10-20
**Status:** Ready for planning
**Mode:** Autonomous

<domain>
## Phase Boundary

Phase 5 completes Bilterm's v1 milestone across six requirements (`TMUX-01`, `TMUX-02`, `PLAT-01`, `QUAL-01`, `QUAL-02`, `QUAL-03`):
1. **Outer `$TMUX` Coexistence & `DCS tmux;` Passthrough (`TMUX-01`)**: Detects when Bilterm runs inside an outer `tmux` session (`$TMUX` environment variable), avoids `Ctrl+B` prefix collisions (using `Alt+` chords and `:command` palette), wraps outbound `OSC 52` clipboard escape sequences in `\x1bPtmux;\x1b<seq>\x1b\\` passthrough envelopes (doubling any inner `\x1b` bytes per the `tmux` passthrough specification), and displays `[TMUX]` telemetry in the OIA status bar.
2. **`tmux -CC` Control Mode Backend & CLI Synchronization (`TMUX-02`)**: Parses `tmux -CC` control mode notifications (`%output %<id> <octal-escaped-payload>`, `%layout-change`, `%window-add`, `%session-changed`, `%begin`/`%end` blocks), unescapes `tmux` octal sequences (`\015\012`, `\033`, `\\`), routes `%output %pane_id` bytes directly into the matching Bilterm `CompositePane`'s `VtParser`, and formats `send-keys -t %<id> -H <hex>` / `split-window` / `resize-pane` commands so every `tmux` pane gains Bilterm's local `FloatingCommandPrompt` overlay.
3. **Cross-Platform Parity (`PLAT-01`)**: Ensures clean compilation and behavior across **Linux** (POSIX `openpty` + `tcgetpgrp`/`tcgetattr`), **macOS** (BSD PTY + `tcgetpgrp`/`tcgetattr`), and **Windows** (`ConPTY` via `portable-pty`, `pwsh.exe`/`cmd.exe` default shell detection, and `#[cfg(windows)]` `OSC 133` fallback in `line_discipline.rs`).
4. **Comprehensive Test Verification (`QUAL-01`)**: Unit tests, Ratatui `TestBackend` + `insta` snapshot tests, and headless PTY integration tests covering every subsystem (`pty`, `vt`, `pane`, `wm`, `ui`, `config`, `tmux`, `input`, `error`).
5. **Technical Architecture Documentation & ADRs (`QUAL-02`)**: Complete system architecture documentation in `docs/architecture/OVERVIEW.md` and Architecture Decision Records (`ADR-001` through `ADR-005`) in `docs/architecture/adr/`.
6. **Extensive Multi-Chapter User Manual (`QUAL-03`)**: Complete User Manual in `docs/user-guide/` (`index.md`, `01-getting-started.md`, `02-floating-command-prompt.md`, `03-window-manager-and-z-stack.md`, `04-themes-glyphs-and-config.md`, `05-tmux-integration.md`, `06-keyboard-mouse-and-command-reference.md`) plus an updated repository `README.md`.

</domain>
