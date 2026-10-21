# Chapter 1: Getting Started & Core Concepts

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [Chapter 2: Command Prompt →](02-floating-command-prompt.md)

## Table of Contents

- [1. Building & Launching Bilterm](#1-building--launching-bilterm)
- [2. What is a Composite Window Pane?](#2-what-is-a-composite-window-pane)
- [3. Automatic Full-Screen TUI (`vim`, `htop`) & Password (`sudo`) Protection](#3-automatic-full-screen-tui-vim-htop--password-sudo-protection)

---

## 1. Building & Launching Bilterm

```bash
cargo build --release
./target/release/bilterm
```

Bilterm detects your platform's default interactive shell (`$SHELL` on Linux/macOS, or `%COMSPEC%` / `powershell.exe` on Windows) and spawns an interactive PTY session inside **Composite Pane `#1`**.

---

## 2. What is a Composite Window Pane?

In traditional terminal emulators, your command prompt is etched into the same character stream as command output. In **Bilterm**, inspired by classic **IBM 3270 mainframe terminals**, every terminal pane is a **Composite Window Pane** composed of three synchronized layers:

1. **Full-Height `OutputCanvas`**: Spans 100% of the pane's height (minus 1 row for the OIA status bar). Commands run here with full 24-bit TrueColor, 256-color, and VT100/ANSI fidelity, scrolling smoothly *underneath* the floating prompt.
2. **Overlaid `FloatingCommandPrompt`**: A movable, pinnable, resizable multi-line command editor (`==> `) with live shell syntax highlighting, Tab completion, and `Ctrl+R` fuzzy history search. Moving, pinning, or hiding this prompt **never** sends `SIGWINCH` to your shell, eliminating reflow jitter. You can also switch between **Floating Mode** and **Non-Overlapping Split Mode** (`Alt+S` / `:prompt split-top | split-bottom`).
3. **IBM 3270 Operator Information Area (`OIA Status Bar`)**: A 1-row telemetry bar at the bottom of each pane showing system state (`● READY`, `⟳ RUNNING`, `⇳ SCROLL`, `▣ ALT SCR`, `♦ !ECHO LOCK`), active input routing (`INPUT: CMD BOX` vs `INPUT: RAW PTY`), prompt pin mode (`PIN:TOP`, `PIN:BOT`, `PIN:FLT`, `PIN:SPL-T`, `PIN:SPL-B`), output volume (`≣ OUT:0L/0B`), and live cursor coordinates (`⌖ R:01 C:001`).

---

## 3. Automatic Full-Screen TUI (`vim`, `htop`) & Password (`sudo`) Protection

- **Alternate Screen Auto-Hide (`DECSET 47/1047/1049`)**: When you launch `vim`, `nvim`, `htop`, `less`, or `fzf`, Bilterm automatically hides the floating command prompt, switches to `INPUT: RAW PTY`, and routes all keystrokes directly to the full-screen application. When the application exits, the floating command prompt automatically reappears.
- **Password Prompt Lock (`!ECHO LOCK`)**: When `sudo`, `ssh`, or `gpg` disables terminal echo (`termios !ECHO`), Bilterm immediately hides the floating command prompt, displays `♦ !ECHO LOCK` in red on the OIA status bar, and routes your password keystrokes directly to the PTY so secrets are never echoed or stored in prompt history.
- **Manual Toggle (`Ctrl+G` or `F12`)**: Press `Ctrl+G` or `F12` (or run `:raw`) at any time to manually toggle between `INPUT: CMD BOX` (command prompt visible) and `INPUT: RAW PTY` (command prompt hidden, direct PTY input).

---

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [Chapter 2: Command Prompt →](02-floating-command-prompt.md)
