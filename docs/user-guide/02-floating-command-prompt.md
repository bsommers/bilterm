# Chapter 2: The Floating Command Prompt & Command Blocks

## 1. Triple-Modality Positioning & Resizing

Every `CompositePane` has its own independent `FloatingCommandPrompt` that can be positioned and resized using **keyboard shortcuts**, **mouse drag-and-drop**, or **`:command` execution**:

| Action | Keyboard | Mouse | `:command` |
|--------|----------|-------|------------|
| Pin to **Top** (IBM 3270 Default) | `Alt+T` | Click `[▲]` badge on prompt top-right border, or drag top border to row 0 | `:prompt top` |
| Pin to **Bottom** | `Alt+B` | Click `[▼]` badge on prompt top-right border, or drag top border to bottom | `:prompt bottom` |
| **Free-Float** vertically | `Alt+Up` / `Alt+Down` | Drag the prompt's top border (`╔═╣ BILTERM 3270 ... ╠═╗`) up or down | `:prompt float <row>` |
| **Resize Height** (`3..12` rows) | `Alt+Plus` / `Alt+Minus` | Drag the prompt's bottom border (`╚═══╝`) up or down | `:prompt height <3..12>` |

---

## 2. Multi-Line Command Editing & Live Shell Syntax Highlighting

- **Insert Newline**: Press `Shift+Enter` or `Alt+Enter` to split the command across multiple lines (`==> ` on line 1, `... ` on continuation lines). The floating box automatically expands vertically to fit multi-line scripts.
- **Execute Command**: Press `Enter` to submit the entire multi-line buffer to the underlying PTY shell.
- **Live Syntax Highlighting**:
  - **Commands** (`git`, `cargo`, `grep`): Bold Phosphor Green (`#55FF55`)
  - **Flags** (`-la`, `--all-targets`): Bright Cyan (`#55FFFF`)
  - **Pipes & Operators** (`|`, `&&`, `||`, `;`, `>`): Bold Yellow (`#FFFF55`)
  - **Quoted Strings** (`"..."`, `'...'`): Amber (`#FFB000`)
  - **Variables** (`$USER`, `${PATH}`): Magenta (`#FF55FF`)
  - **Unclosed Quotes**: Red Underlined (`#FF5555`)
- **Readline / Emacs Editing & Undo/Redo**:
  - `Ctrl+Z`: Undo last edit
  - `Ctrl+K`: Kill from cursor to end of line
  - `Ctrl+W`: Kill previous word
  - `Ctrl+U`: Clear entire prompt buffer
  - `Ctrl+Y`: Yank (paste) last killed text
  - `Alt+Left` / `Alt+Right`: Move by word

---

## 3. Interactive Fuzzy History Search (`Ctrl+R`)

Press `Ctrl+R` inside the floating command prompt to open the `nucleo-matcher` fuzzy history search overlay:
- Type any subsequence (e.g. `ctall` to match `cargo test --all-targets`).
- Matched characters are highlighted in **Bold Underlined Yellow**.
- Press `Up`/`Down` or `Ctrl+P`/`Ctrl+N` to cycle through ranked matches.
- Press `Enter` to load the selected command into the editor, or `Esc` / `Ctrl+G` to cancel.

---

## 4. `OSC 133` Command Block Jumping & `OSC 52` Clipboard

- **Jump to Previous Command Block**: Press `Alt+PageUp` or run `:block prev`. The OIA status bar displays `BLK k/N` (`e.g. BLK 1/4`) and scrolls the underlying `OutputCanvas` directly to that command's invocation row.
- **Jump to Next Command Block**: Press `Alt+PageDown` or run `:block next`.
- **Yank to System Clipboard (`OSC 52`)**: Run `:yank` to encode the focused command block (or screen contents) as an `OSC 52` Base64 clipboard sequence (`\x1b]52;c;<base64>\x07`) sent to your host terminal (and automatically wrapped in `DCS tmux;` when inside `tmux`).
