# Bilterm User Guide (`v1.12.0`)

[← Back to Project README](../../README.md) | [Starter Packs (`starter-packs/`)](../../starter-packs/README.md)

Welcome to the official User Guide for **Bilterm (`v1.12.0`)** — the Rust + Ratatui terminal multiplexer, native desktop GUI terminal (`--gui`), composite-pane shell wrapper, remote SSH fleet manager, and AI-automatable terminal emulator inspired by **IBM 3270 Mainframe Terminals** and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)**.

> **Quick Interactive Help (`<F1>`)**: At any time inside Bilterm, press **`F1`** or run **`:help`** to open the scrollable Borland Turbo Vision Help Modal directly on screen (`Up`/`Down`/`PageUp`/`PageDown` to scroll, `F1`/`Esc` to close).

![Bilterm Multi-Pane Composite Workspace & SSH Fleet](../assets/screenshot-workspace.svg)

---

## 10-Chapter User Guide Index

1. **[Chapter 1: Getting Started & Core Concepts](01-getting-started.md)**
   - Building & launching (TUI and `--gui`), Composite Window Pane architecture (`OutputCanvas` + `FloatingCommandPrompt` + `OIA Status Bar`), per-pane independent font scaling (`v1.12.0`), all-border mouse resizing, Alternate Screen (`DECSET 1049`) auto-hide, and `!ECHO LOCK` password protection.
2. **[Chapter 2: The Command Prompt, Tab Completion, History & Command Blocks](02-floating-command-prompt.md)**
   - Top/Bottom/Floating/Split positioning (`Alt+T`, `Alt+B`, `Alt+S`), resizable width (`Alt+[`/`Alt+]`, mouse border drag) & height (`Alt+Plus`/`Alt+Minus`), scrollable multi-line editing (`Shift+PageUp`/`Shift+PageDown`), running command banner (`⟳ RUNNING:`), Tab completion (`Tab`/`Shift+Tab`), Borland Command History Pop-up (`Alt+P` / `:history`), session persistence, and `OSC 133` block jumping.
3. **[Chapter 3: Multi-Pane Window Manager, Z-Order Stacking, Mouse Frame Resizing & Guake Quick Terminal](03-window-manager-and-z-stack.md)**
   - Horizontal/Vertical binary splits (`F2`/`Alt+H`, `Alt+V`), all-border & all-corner mouse frame drag resizing (`v1.12.0`), independent per-pane font scaling (`Ctrl++`/`Ctrl+-`), Borland Turbo Vision overlapping Z-order stack (`Alt+M`), deck shuffling (`F6`/`Alt+Tab`), selective raise (`Alt+1..9`, mouse click), Slide-Out Drawer (`F7` / `:drawer`), and the persistent Guake/Quake drop-down terminal (`F10` / `Alt+\`` / `:quake`).
4. **[Chapter 4: Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Config](04-themes-glyphs-and-config.md)**
   - 8 border outline styles (`:outline`), 4 DOS drop-shadow modes (`:shadows`), 10 retro & modern palettes (`:theme`), 4 glyph packs (`:glyphs`), pane-local vs. full-screen IBM 3270 cross-hairs (`Alt+X` / `:crosshair`), OIA status bar telemetry, and `config.toml` / `.ini` configuration.
5. **[Chapter 5: First-Class `tmux` Integration, SSH Domains & `herdr` Fleet Management](05-tmux-integration.md)**
   - Outer `$TMUX` coexistence (`DCS tmux;` passthrough for `OSC 52` clipboard), `tmux -CC` Control Mode synchronization, `:ssh` composite panes, `:ssh-tmux` persistent sessions, and `:remote` fleet federation.
6. **[Chapter 6: Complete CLI, Keyboard, Mouse & `:command` Reference](06-keyboard-mouse-and-command-reference.md)**
   - Exhaustive tables of all CLI flags and subcommands, function keys (`F1`–`F12`), `Ctrl+B`/`Ctrl+X` leader chords, `Alt+` chords, per-pane font scaling shortcuts, mouse drag/click/scroll gestures, and every `:colon` command across all subsystems.
7. **[Chapter 7: Troubleshooting & FAQ](07-troubleshooting-and-faq.md)**
   - Diagnostic log inspection (`bilterm.log`), per-pane font zoom inside `nmon`/`htop`, multi-pane mouse resizing, terminal recovery, password prompt routing, CJK/Emoji border alignment, and common questions.
8. **[Chapter 8: Extended CLI, Native `--gui` Mode & Typography Examples](08-extended-cli-and-gui-examples.md)**
   - Copy-pasteable CLI recipes for `--gui --gui-font "..." --gui-font-size 16`, per-pane font scaling (`Ctrl++`/`Ctrl+-`), headless JSON-RPC daemons (`--headless`, `bilterm ctl`), MCP AI server (`--mcp`), TOML playbooks (`bilterm run`), and high-DPI charts (`bilterm plot`).
9. **[Chapter 9: Real-World Use Cases, Workflows & Architecture Ideas](09-use-cases-and-workflows.md)**
   - 9 comprehensive architectures & workflows: Multi-Agent AI Coding Swarms, Remote Agents over SSH, Remote Development (`:ssh-tmux`), Robot Scripting & Deterministic QA, CI/CD Verification Harnesses, Daily Power-User Operations, Polyglot Developer Layouts, Live Escape-Sequence Debugging (`F9`), and Custom Per-Pane Font Layouts (`nmon`).
10. **[Chapter 10: Security Architecture & Operational Best Practices](10-security-and-best-practices.md)**
    - Defense-in-depth engineering reference covering 3-Signal `!ECHO LOCK`, `/dev/random` flood & escape-trap sanitization, clipboard paste-injection guards, private `0700`/`0600` filesystem confinement, Unix socket `SO_PEERCRED` + bearer-token RPC security, and SSH argument-injection prevention.

---

See **[`docs/user-guide/README.md`](README.md)** for the full single-page guide with screenshots, starter packs, and configuration tables.
