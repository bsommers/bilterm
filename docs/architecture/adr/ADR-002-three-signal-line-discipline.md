# ADR-002: Three-Signal Line-Discipline & Password Lock Detection

- **Status:** Accepted
- **Date:** 2026-10-20

## Context
A floating command prompt that buffers keystrokes locally until `Enter` poses a severe security and usability hazard if the underlying PTY enters a password prompt (`sudo`, `ssh`, `gpg`) or an interactive sub-REPL (`python`, `node`, `git add -p`) that does not switch to the `DECSET 1049` alternate screen.

## Decision
`src/pty/line_discipline.rs` combines three orthogonal signals before routing any keystroke:
1. **Signal 1 (`OSC 133` Shell Integration)**: Tracks `OSC 133;C` (command executing) vs `A/B/D` (shell at prompt).
2. **Signal 2 (`tcgetpgrp` Foreground Process Group)**: Compares `libc::tcgetpgrp(master_fd)` against the shell's PID to detect active foreground child processes.
3. **Signal 3 (`tcgetattr` `termios` Inspection)**: Inspects `c_lflag & libc::ECHO`. Whenever `!ECHO` is detected (`echo_enabled == false`), Bilterm immediately hides the floating prompt, displays `!ECHO LOCK` in the OIA status bar, and routes keystrokes directly to the PTY without buffering them in memory.

## Consequences
- `sudo` and `ssh` passwords are never echoed in cleartext inside `FloatingPromptState`.
- Interactive REPLs and pagers automatically receive raw keystrokes without requiring manual mode switching, while `Ctrl+G` / `F12` remains available as an instant manual override.
