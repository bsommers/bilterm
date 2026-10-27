# Chapter 1: Getting Started & Core Concepts

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [Chapter 2: Command Prompt →](02-floating-command-prompt.md)

## Table of Contents

- [1. Building & Launching Bilterm (TUI & Native `--gui` Mode)](#1-building--launching-bilterm-tui--native---gui-mode)
- [2. What is a Composite Window Pane?](#2-what-is-a-composite-window-pane)
- [3. Per-Pane Independent Font Scaling & All-Border Mouse Frame Resizing (`v1.12.0`)](#3-per-pane-independent-font-scaling--all-border-mouse-frame-resizing-v1120)
- [4. Automatic Full-Screen TUI (`vim`, `htop`, `nmon`) & Password (`sudo`) Protection](#4-automatic-full-screen-tui-vim-htop-nmon--password-sudo-protection)

---

## 1. Building & Launching Bilterm (TUI & Native `--gui` Mode)

```bash
# Option A: Plain binary installer (Linux / macOS)
./scripts/install.sh            # installs to ~/.local/bin (or pass --cargo, --global, --prefix <DIR>)
bilterm -v

# Option B: Fresh clone developer setup (installs C toolchain, rustup, Rust 1.90, builds & installs)
./scripts/setup.sh --yes --build --install

# Option C: Windows PowerShell setup or plain binary install
powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1 -Yes -Build -Install
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1

# Option D: Direct Cargo build & launch
cargo build --release
./target/release/bilterm

# Option E: Native Desktop GUI Window with Custom Font & Size (X11 / Win32 GDI / Cocoa)
bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16
bilterm --gui --gui-font "IBM 3270" --gui-font-size 18 --theme ibm-3278-green
```

Bilterm detects your platform's default interactive shell (`$SHELL` on Linux/macOS, or `%COMSPEC%` / `powershell.exe` on Windows) and spawns an interactive PTY session inside **Composite Pane `#1`**.

![Bilterm Multi-Pane Composite Workspace & SSH Fleet](../assets/screenshot-workspace.svg)

---

## 2. What is a Composite Window Pane?

In traditional terminal emulators, your command prompt is etched into the same character stream as command output. In **Bilterm**, inspired by classic **IBM 3270 mainframe terminals**, every terminal pane is a **Composite Window Pane** composed of three synchronized layers:

1. **Full-Height `OutputCanvas`**: Spans 100% of the pane's height (minus 1 row for the OIA status bar). Commands run here with full 24-bit TrueColor, 256-color, and VT100/ANSI fidelity, scrolling smoothly *underneath* the floating prompt.
2. **Overlaid `FloatingCommandPrompt`**: A movable, pinnable, resizable multi-line command editor (`==> `) with live shell syntax highlighting, Tab completion, and `Ctrl+R` fuzzy history search. Moving, pinning, or hiding this prompt **never** sends `SIGWINCH` to your shell, eliminating reflow jitter. You can also switch between **Floating Mode**, **Non-Overlapping Split Mode** (`Alt+S` / `:prompt split-top | split-bottom`), and **Regular Terminal Mode** (`F4` / `:mode terminal`).
3. **IBM 3270 Operator Information Area (`OIA Status Bar`)**: A 1-row telemetry bar at the bottom of each pane showing system state (`● READY`, `⟳ RUNNING`, `⇳ SCROLL`, `▣ ALT SCR`, `♦ !ECHO LOCK`), active input routing (`INPUT: CMD BOX` vs `INPUT: RAW PTY`), prompt pin mode (`PIN:TOP`, `PIN:BOT`, `PIN:FLT`, `PIN:SPL-T`, `PIN:SPL-B`), output volume (`≣ OUT:0L/0B`), and live cursor coordinates (`⌖ R:01 C:001`).

---

## 3. Per-Pane Independent Font Scaling & All-Border Mouse Frame Resizing (`v1.12.0`)

- **Per-Pane Independent Font Zoom (`Ctrl++`, `Ctrl+-`, `Ctrl+0`, `Ctrl+MouseWheel`)**:
  - Each composite pane maintains its own font scale (`50%..=300%`) and font family (`:font jetbrains 120%`, `:font vga 14pt`).
  - Zooming a pane's font in or out dynamically recalculates that pane's character grid (`cols × rows`) to fit its pixel/cell frame and sends `TIOCSWINSZ` (`SIGWINCH`) to that pane's child PTY so programs like `nmon`, `htop`, and `vim` reflow cleanly inside that pane alone.
- **All-Border & All-Corner Mouse Frame Drag Resizing**:
  - Grab **any** pane border (`Top`, `Bottom`, `Left`, `Right`) or **any** corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`) with the mouse to resize tiled splits or overlapping Z-Stack windows smoothly in 2-pane, 3-pane, or N-pane layouts.

![Bilterm Per-Pane Independent Font Scaling & 3-Way Split](../assets/screenshot-per-pane-scaling.svg)

---

## 4. Automatic Full-Screen TUI (`vim`, `htop`, `nmon`) & Password (`sudo`) Protection

- **Alternate Screen Auto-Hide (`DECSET 47/1047/1049`)**: When you launch `vim`, `nvim`, `htop`, `nmon`, `less`, or `fzf`, Bilterm automatically hides the floating command prompt, switches to `INPUT: RAW PTY`, and routes keystrokes directly to the full-screen application (while still keeping global WM chords like `Ctrl++`/`Ctrl+-` font zoom, `Alt+1..9` focus, and mouse border resizing active). When the application exits, the floating command prompt automatically reappears.
- **Password Prompt Lock (`!ECHO LOCK`)**: When `sudo`, `ssh`, or `gpg` disables terminal echo (`termios !ECHO`), Bilterm immediately hides the floating command prompt, displays `♦ !ECHO LOCK` in red on the OIA status bar, and routes your password keystrokes directly to the PTY so secrets are never echoed or stored in prompt history.
- **Manual Toggle (`Ctrl+G` or `F12`)**: Press `Ctrl+G` or `F12` (or run `:raw`) at any time to manually toggle between `INPUT: CMD BOX` (command prompt visible) and `INPUT: RAW PTY` (command prompt hidden, direct PTY input).

---

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [Chapter 2: Command Prompt →](02-floating-command-prompt.md)
