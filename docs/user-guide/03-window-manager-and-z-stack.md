# Chapter 3: Multi-Pane Window Manager, Z-Order Stacking, Mouse Frame Resizing & Guake Quick Terminal

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 2: Command Prompt](02-floating-command-prompt.md) | [Chapter 4: Themes & Config →](04-themes-glyphs-and-config.md)

## Table of Contents

- [1. Splitting Composite Panes Horizontally & Vertically (`WM-01`)](#1-splitting-composite-panes-horizontally--vertically-wm-01)
- [2. All-Border & All-Corner Mouse Frame Resizing + Per-Pane Font Scaling (`v1.12.0`)](#2-all-border--all-corner-mouse-frame-resizing--per-pane-font-scaling-v1120)
- [3. Z-Order Window Stacking, Deck Shuffling & Selective Raise (`WM-02`)](#3-z-order-window-stacking-deck-shuffling--selective-raise-wm-02)
- [4. Persistent Guake / Quake-Style Drop-Down Quick Terminal (`WM-05`)](#4-persistent-guake--quake-style-drop-down-quick-terminal-wm-05)
- [5. Slide-Out Sessions/Windows/Panes & Fleet Drawer (`F7` / `:drawer`)](#5-slide-out-sessionswindowspanes--fleet-drawer-f7--drawer)
- [6. Workspace Session Serialization, Multi-Pane Broadcast & 3270 Field Lock (`ECO-02`..`ECO-04`)](#6-workspace-session-serialization-multi-pane-broadcast--3270-field-lock-eco-02eco-04)

---

## 1. Splitting Composite Panes Horizontally & Vertically (`WM-01`)

Every `CompositePane` (complete with its own PTY `OutputCanvas` and its own movable `FloatingCommandPrompt`) can be recursively split into binary space-partitioned (`SplitTree`) layouts:
- **Horizontal Split (Top / Bottom)**: Press `F2`, `Alt+H`, `Ctrl+B "` / `Ctrl+X -`, or run `:split h` (or `:sp`).
- **Vertical Split (Left / Right)**: Press `Alt+V`, `Ctrl+B %` / `Ctrl+X |`, or run `:split v` (or `:vsplit` / `:vs`).
- **Close Focused Pane**: Press `Alt+W`, `Ctrl+B w`, click `[X]` on the pane's top-left title bar, or run `:close`.
- **Zoom / Maximize Focused Pane**: Press `F5`, `Alt+Z`, `Ctrl+B z`, click `[▲]`/`[▼]` on the pane's top-right title bar, or run `:zoom`.

---

## 2. All-Border & All-Corner Mouse Frame Resizing + Per-Pane Font Scaling (`v1.12.0`)

- **Drag Any Edge or Corner with the Mouse**:
  - **Edges (`Top`, `Bottom`, `Left`, `Right`)**: Dragging any interior shared border moves the corresponding `SplitTree` divider directly; dragging an outer pane border adjusts the opposite divider so every frame edge is responsive. In `Stacked` Z-order mode, dragging any edge resizes that side while anchoring the opposite edge.
  - **Corners (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`)**: Dragging any of the 4 corners (`╔`, `╗`, `╚`, `╝`) resizes both horizontal and vertical dimensions simultaneously.
- **Keyboard Resize & Equalize**:
  - Use `Ctrl+Alt+Left/Right/Up/Down` (or `Alt+Shift+Left/Right/Up/Down` or `:resize left|right|up|down [N]`) to resize by keyboard.
  - Run `:equalize` (or `:resize reset` / `<Leader> =`) to reset all split dividers to 50/50.
- **Independent Per-Pane Font Scaling (`Ctrl++`, `Ctrl+-`, `Ctrl+0`, `Ctrl+Wheel`)**:
  - Press **`Ctrl++`** / **`Ctrl+-`** / **`Ctrl+0`** (or hold **`Ctrl`** and scroll the mouse wheel over any pane) to scale **only that pane's** font size (`50%..=300%`).
  - Zooming a pane's font automatically recalculates its `cols × rows` PTY grid (so a pane at `80%` font scale fits `1.25×` more columns and rows, while a pane at `125%` scale renders larger text with proportionally fewer columns and rows), working seamlessly even inside interactive full-screen TUI apps like `nmon`, `htop`, and `vim`.

---

## 3. Z-Order Window Stacking, Deck Shuffling & Selective Raise (`WM-02`)

Inspired by classic **Borland Turbo Vision (Turbo C / Turbo Pascal)** overlapping windows, Bilterm maintains a **Z-Order Deck (`ZStack`)** across all open composite panes:
- **Toggle Tiled vs. Stacked Cascade Layout**: Press `Alt+M` (`<Leader> M`) or run `:layout stack` / `:layout tile`.
- **Shuffle Deck Top-to-Bottom (`Next Window`)**: Press `Alt+Tab`, `F6`, `<Leader> n`, or run `:deck next`. Sends the current top window to the bottom of the deck (`Z:1/N`) and promotes the next window to the top (`Z:N/N`).
- **Shuffle Deck Bottom-to-Top (`Prev Window`)**: Press `Alt+Shift+Tab`, `<Leader> p`, or run `:deck prev`. Pulls the bottom-most window to the top of the deck.
- **Selective Raise by Window Number**: Press `Alt+1` through `Alt+9` (`<Leader> 1..9`) or run `:focus <id>` to immediately pluck pane `#1..#9` from anywhere in the stack and raise it to the front.
- **Selective Raise & Move by Mouse**: Click anywhere inside an occluded window to raise it to the top of the z-order stack and focus it; drag its top title bar to reposition it across the workspace.

---

## 4. Persistent Guake / Quake-Style Drop-Down Quick Terminal (`WM-05`)

Need to run a quick "by-the-way" command without disrupting your tiled or stacked workspace?
- Press **`F10`**, **`Alt+\``** (Alt+Backtick), **`Ctrl+\`**, **`Ctrl+Space`**, or run **`:quake`** to toggle the top-anchored **Quake Quick Terminal (`[Q] QUAKE QUICK TERM [~]`)**.
- When hidden, its underlying shell and PTY session remain **100% persistent** in the background — press `F10` or `Alt+\`` again at any time to slide it back down with your running process and command history intact.

---

## 5. Slide-Out Sessions/Windows/Panes & Fleet Drawer (`F7` / `:drawer`)

- Press **`F7`**, **`Alt+Shift+D`**, **`<Leader> d`**, or run **`:drawer`** to toggle the **`herdr`-style Slide-Out Drawer** displaying all active sessions, windows, composite panes (with live process status and geometry), and registered remote SSH fleet nodes.
- Dock the drawer to any screen edge (`:drawer left | right | top | bottom`), switch between `Overlay` (floating with DOS drop shadow) and `Push` (shrinking tiled workspace so zero cells are occluded via `:drawer push | float`), or set visibility (`:drawer fixed | autohide | ondemand`).
- **Interactive Session Creation & Reordering in the Drawer**:
  - Press **`N`** inside the focused `<F7>` Drawer to create and switch to a new session immediately.
  - Press **`Shift+Up`** / **`Alt+Up`** / **`K`** or **`Shift+Down`** / **`Alt+Down`** / **`J`** inside the focused `<F7>` Drawer to move the selected session up or down in the session list order (persisted to `session-order.json`).

---

## 6. Workspace Session Serialization, Multi-Pane Broadcast & 3270 Field Lock (`ECO-02`..`ECO-04`)

- **Create, Name/Rename, Reorder, Save & Restore Multi-Pane Workspaces (`:session`)**:
  - `:session new [name]` (or `:new-session [name]`, `bilterm new [name]`, `bilterm -n [name]`) saves the outgoing session and starts a fresh 1-pane session (auto-named `session-2`, `session-3`, ... when `[name]` is omitted).
  - `:session blank [name]` (or `:session clean [name]`, `bilterm blank [name]`, `bilterm -B`) starts a clean 1-pane blank session without restoring prior multi-pane state.
  - `:session rename [old_name] <new_name>` (or `:rename-session <new_name>`) renames the active or specified session in place while preserving its position in the session list.
  - `:session move [name] <up|down|top|bottom|1..N>` and `:session reorder <s1> <s2> ...` (or `:session swap <a> <b>`) reorder sessions in `:session list` and the `<F7>` Drawer.
  - `:session save <name>` serializes the active layout (`Tile` vs `Stack`), `SplitTree` ratios, `ZStack` order, pane titles, working directories, `PromptAnchor`, prompt dimensions, read-only locks, and command histories to `$XDG_STATE_HOME/bilterm/sessions/<name>.toml`.
  - `:session restore <name>` reconstructs the saved workspace layout and panes; `:session list` displays all saved session profiles in your custom order; `:session delete <name>` deletes a saved session.
- **Synchronized Multi-Pane Input Broadcast (`:broadcast [on|off]`)**:
  - Displays a `📡 BCAST` badge in the OIA status bar and fans out every command submitted in the focused prompt to **all unlocked panes** simultaneously.
- **Read-Only Protected 3270 Field Lock (`Alt+L` / `:lock [on|off]`)**:
  - Locks a pane against accidental command submissions or raw keystrokes (`🔒 PROT` badge in OIA) and excludes it from `:broadcast` fan-out while still allowing `:colon` commands.
- **Top Borland Turbo Vision Menu Bar (`:menubar [on|off]`)**:
  - Enables a 1-row top pull-down menu bar (`≡ System  Window  Prompt  View  Theme  Tmux  Help`) navigable by mouse click or keyboard arrows.

---

[← Chapter 2: Command Prompt](02-floating-command-prompt.md) | [User Guide Home](README.md) | [Chapter 4: Themes & Config →](04-themes-glyphs-and-config.md)
