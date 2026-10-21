# Chapter 2: The Command Prompt, Tab Completion, History & Command Blocks

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 1: Getting Started](01-getting-started.md) | [Chapter 3: Window Manager →](03-window-manager-and-z-stack.md)

## Table of Contents

- [1. Positioning, Non-Overlapping Split Mode & Resizable Width/Height](#1-positioning-non-overlapping-split-mode--resizable-widthheight)
- [2. Running / Pending Command Banner](#2-running--pending-command-banner)
- [3. Scrollable Multi-Line Command Prompt & Live Syntax Highlighting](#3-scrollable-multi-line-command-prompt--live-syntax-highlighting)
- [4. Command History Pop-up Window, Session Persistence & Temp-File Export](#4-command-history-pop-up-window-session-persistence--temp-file-export)
- [5. `OSC 133` Command Block Jumping & `OSC 52` Clipboard](#5-osc-133-command-block-jumping--osc-52-clipboard)

---

## 1. Positioning, Non-Overlapping Split Mode & Resizable Width/Height

Every `CompositePane` includes a dedicated command prompt that can either **float over** the scrolling `OutputCanvas` or **partition the pane with zero overlap**, and can be resized both vertically and horizontally via **keyboard**, **mouse**, or **`:command`**:

| Action | Keyboard | Mouse | `:command` |
|--------|----------|-------|------------|
| Pin to **Top** (Floating IBM 3270 Default) | `Alt+T` | Click `[▲]` badge on top-right border, or drag top border to row 0 | `:prompt top` |
| Pin to **Bottom** (Floating) | `Alt+B` | Click `[▼]` badge on top-right border, or drag top border to bottom | `:prompt bottom` |
| **Non-Overlapping Split** (Command + Output with zero overlap) | `Alt+S` | — | `:prompt split-top` / `:prompt split-bottom` / `:prompt split` |
| **Free-Float** vertically & horizontally | `Alt+Up` / `Alt+Down` | Drag the prompt's top border (`╔═╣ BILTERM 3270 ... ╠═╗`) | `:prompt float <row>` |
| **Resize Height** (`3..12` rows) | `Alt+Plus` / `Alt+Minus` | Drag the prompt's bottom border (`╚═══╝`) up or down | `:prompt height <3..12>` |
| **Resize Width** (`16..cols` or full width) | `Alt+]` / `Alt+[` | Drag the prompt's **left border**, **right border**, or **bottom-right corner (`◢`)** | `:prompt width <cols\|full>` |

---

## 2. Running / Pending Command Banner

When you submit a command with `Enter` (for example, `tail -f /dev/null` or `tail -f /dev/random`), Bilterm keeps the command visible in a dedicated banner at the top of the command prompt window while giving you a clean input line below it:
- **`⟳ RUNNING:  tail -f /dev/null`** — Displayed in bright yellow/cyan while a foreground subcommand is actively executing in the PTY.
- **`● LAST CMD:  ...`** — Displayed after submission until the next command or `Ctrl+C` clears it.
- Pressing `Ctrl+C` while a foreground command is streaming sends `SIGINT` (`0x03`) to the PTY and clears the running banner.

---

## 3. Scrollable Multi-Line Command Prompt & Live Syntax Highlighting

- **Insert Newline**: Press `Shift+Enter` or `Alt+Enter` to write multi-line scripts (`==> ` on line 1, `... ` on continuation lines).
- **Scroll Prompt Content**:
  - When a command has more lines than the current prompt box height, a visual scrollbar (`█`/`░`) appears on the right border of the prompt.
  - Scroll the prompt viewport vertically using **`Shift+PageUp` / `Shift+PageDown`** or the **mouse scroll wheel** while hovering over the prompt box.
  - Long single-line commands automatically scroll horizontally to follow the cursor.
- **Tab Completion (`Tab` / `Shift+Tab`)**:
  - Completes `:colon` commands and sub-arguments (`:prompt`, `:outline`, `:shadows`, `:theme`, `:history`, etc.), shell builtins, `$PATH` executables, `$ENV` / `${ENV}` variables, and filesystem paths (`./`, `/`, `~/`, and the child shell's live working directory via `OSC 7` or `/proc/<pid>/cwd`).
  - Multiple matches open a live `TAB:` completion ribbon at the bottom of the prompt; press `Tab` / `Shift+Tab` to cycle and `Esc` to dismiss.

---

## 4. Command History Pop-up Window, Session Persistence & Temp-File Export

### Interactive History Modes
1. **Borland Command History Pop-up Window (`Alt+P` or `:history`)**:
   - Opens a centered Borland-style modal dialog listing all commands executed in the active pane along with its unique **Session ID** and **Persistence Status (`PERSIST:ON/OFF`)**.
   - Use `Up`/`Down` (or `k`/`j`) to select a command, `Enter` to load it into the prompt, **`s`** to export the pane's command history to a temporary `.sh` script file, or `Esc`/`q` to close.
2. **Fuzzy History Search (`Ctrl+R`)**:
   - Opens the inline `nucleo-matcher` reverse-i-search overlay with matched characters highlighted in **Bold Underlined Yellow**.

### Automatic Session Persistence & File Rotation
By default (`history_enabled = true`), Bilterm automatically persists every submitted command to disk (`$XDG_STATE_HOME/bilterm/history/` or `~/.local/state/bilterm/history/`):
- Each log file is named `bilterm-history-<session_id>-pane-<pane_id>.log` (where `<session_id>` combines epoch timestamp, PID, and sequence counter).
- Each line records:
  ```text
  [2026-10-20T21:45:00Z | session=1792532700-4120-1 | pane=1] tail -f /dev/null
  ```
- **Commands & Config Controls**:
  - `:history save [optional/path.sh]` — Save current pane commands out to a temporary file (or custom path) and display the path in the status bar.
  - `:history off` / `:history on` — Toggle command history persistence on or off at runtime.
  - `:history max-files <N>` — Set the maximum number of retained session history log files (oldest files are automatically pruned).

---

## 5. `OSC 133` Command Block Jumping & `OSC 52` Clipboard

- **Jump to Previous / Next Command Block**: Press `Alt+PageUp` / `Alt+PageDown` (or `:block prev` / `:block next`) to jump directly between prior commands in scrollback.
- **Yank to System Clipboard (`OSC 52`)**: Run `:yank` to copy the selected command block (or screen contents) via `OSC 52` (`DCS tmux;` wrapped when inside `tmux`).

---

[← Chapter 1: Getting Started](01-getting-started.md) | [User Guide Home](README.md) | [Chapter 3: Window Manager →](03-window-manager-and-z-stack.md)
