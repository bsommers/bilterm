# Chapter 3: Multi-Pane Window Manager, Z-Order Stacking & Guake Quick Terminal

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 2: Command Prompt](02-floating-command-prompt.md) | [Chapter 4: Themes & Config →](04-themes-glyphs-and-config.md)

## Table of Contents

- [1. Splitting Composite Panes Horizontally & Vertically (`WM-01`)](#1-splitting-composite-panes-horizontally--vertically-wm-01)
- [2. Z-Order Window Stacking, Deck Shuffling & Selective Raise (`WM-02`)](#2-z-order-window-stacking-deck-shuffling--selective-raise-wm-02)
- [3. Persistent Guake / Quake-Style Drop-Down Quick Terminal (`WM-05`)](#3-persistent-guake--quake-style-drop-down-quick-terminal-wm-05)

---

## 1. Splitting Composite Panes Horizontally & Vertically (`WM-01`)

Every `CompositePane` (complete with its own PTY `OutputCanvas` and its own movable `FloatingCommandPrompt`) can be recursively split:
- **Horizontal Split (Top / Bottom)**: Press `F2`, `Alt+H`, or run `:split h` (or `:sp`).
- **Vertical Split (Left / Right)**: Press `Alt+V` or run `:split v` (or `:vsplit` / `:vs`).
- **Close Focused Pane**: Press `Alt+W` or run `:close`.
- **Zoom / Maximize Focused Pane**: Press `F5`, `Alt+Z`, or run `:zoom`.

---

## 2. Z-Order Window Stacking, Deck Shuffling & Selective Raise (`WM-02`)

Inspired by classic **Borland Turbo Vision (Turbo C / Turbo Pascal)** overlapping windows, Bilterm maintains a **Z-Order Deck (`ZStack`)** across all open composite panes:
- **Toggle Tiled vs. Stacked Cascade Layout**: Press `Alt+M` or run `:layout stack` / `:layout tile`.
- **Shuffle Deck Top-to-Bottom (`Next Window`)**: Press `Alt+Tab`, `F6`, or run `:deck next`. Sends the current top window to the bottom of the deck (`Z:1/N`) and promotes the next window to the top (`Z:N/N`).
- **Shuffle Deck Bottom-to-Top (`Prev Window`)**: Press `Alt+Shift+Tab` or run `:deck prev`. Pulls the bottom-most window to the top of the deck.
- **Selective Raise by Window Number**: Press `Alt+1` through `Alt+9` or run `:focus <id>` to immediately pluck pane `#1..#9` from anywhere in the stack and raise it to the front.
- **Selective Raise by Mouse Click**: Click anywhere inside an occluded window to raise it to the top of the z-order stack and focus it.

---

## 3. Persistent Guake / Quake-Style Drop-Down Quick Terminal (`WM-05`)

Need to run a quick "by-the-way" command without disrupting your tiled or stacked workspace?
- Press **`F10`**, **`Alt+\``** (Alt+Backtick), **`Ctrl+Space`**, or run **`:quake`** to toggle the top-anchored **Quake Quick Terminal (`[Q] QUAKE QUICK TERM [~]`)**.
- When hidden, its underlying shell and PTY session remain **100% persistent** in the background — press `F10` or `Alt+\`` again at any time to slide it back down with your running process and command history intact.

---

[← Chapter 2: Command Prompt](02-floating-command-prompt.md) | [User Guide Home](README.md) | [Chapter 4: Themes & Config →](04-themes-glyphs-and-config.md)
