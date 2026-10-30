# Chapter 4: Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Configuration

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 3: Window Manager](03-window-manager-and-z-stack.md) | [Chapter 5: tmux Integration →](05-tmux-integration.md)

## Table of Contents

- [1. Selectable Window & Pane Border Outlines](#1-selectable-window--pane-border-outlines)
- [2. Themeable & Selectable DOS Drop Shadows (`UI-01`)](#2-themeable--selectable-dos-drop-shadows-ui-01)
- [3. How to Turn On Full-Screen vs. Pane-Local Cross-Hair Cursors (`UI-05`)](#3-how-to-turn-on-full-screen-vs-pane-local-cross-hair-cursors-ui-05)
- [4. Operator Information Area (OIA) Status Bar Icons & Telemetry](#4-operator-information-area-oia-status-bar-icons--telemetry)
- [5. Declarative Configuration (`~/.config/bilterm/config.toml`) (`CFG-01`)](#5-declarative-configuration-configbiltermconfigtoml-cfg-01)

---

## 1. Selectable Window & Pane Border Outlines

Customize the border outline style of workspace windows and the command prompt independently or together using `:outline` / `:border` or `config.toml`:

| Style Name | Top-Left / Horizontal / Top-Right | Command |
|------------|-----------------------------------|---------|
| `double` (Default) | `╔═══╗` | `:outline double` |
| `single` / `thin` / `solid` | `┌───┐` | `:outline thin` |
| `rounded` | `╭───╮` | `:outline rounded` |
| `thick` / `bold` / `heavy` | `┏━━━┓` | `:outline thick` |
| `dotted` | `┌┈┈┈┐` | `:outline dotted` |
| `dashed` | `┌┄┄┄┐` | `:outline dashed` |
| `quadruple` | `▛▀▀▀▜` | `:outline quadruple` |
| `ascii` | `+---+` | `:outline ascii` |

Target only windows or only the command prompt via:
- `:outline window <style>`
- `:outline prompt <style>`
- `:outline bold [on|off]` — render every window frame in bold weight (`window_outline_bold = true` in `config.toml`).

### Per-Window Theme, Outline & Bold Overrides

Each window can use its own theme and frame style, independent of the global settings. Commands apply to the focused window:

| Command | Effect |
|---------|--------|
| `:window theme <name>` / `:theme window <name>` | Built-in preset (`dracula`, `3278`, …) or imported `.itermcolors`/TOML/YAML name or path |
| `:window theme next` / `prev` / `reset` | Cycle presets for this window, or revert to the global theme |
| `:window outline <style>` / `:outline pane <style>` | Per-window frame style (any style from the table above) |
| `:window outline next` / `reset` | Cycle styles for this window, or revert to the global outline |
| `:window bold [on\|off\|toggle\|reset]` | Per-window bold border |
| `:window reset` / `:window show` | Clear all overrides for this window, or show its current style |

These actions are also in the top menu bar under **Window** (*Next Window Theme*, *Next Window Outline*, *Toggle Bold Border*, *Reset Window Style*). Per-window overrides last for the current run only and are not saved with sessions yet.

### Window Title-Bar Controls

Each window has three controls in its upper-right corner: **`[_]` minimize**, **`[□]` maximize/zoom**, and **`[X]` close**. The close `X` is shown in red. The CP437 glyph pack uses Turbo Vision's `[↑]` for zoom, and the ASCII pack uses `[+]`.

---

## 2. Themeable & Selectable DOS Drop Shadows (`UI-01`)

Floating prompts, stacked windows, the History Pop-up, the `<F1>` Help modal, and the Guake overlay cast a **2-column right / 1-row bottom DOS drop shadow**. Switch shadow modes live via `:shadows <mode>`:
- `:shadows translucent` — Dims underlying foreground/background cells to `DarkGray` on `Black` without erasing the characters underneath.
- `:shadows tinted` — Tints underlying cells using the active theme's `shadow_fg` and `shadow_bg` palette colors (customizable in `theme.ini`).
- `:shadows solid` — Renders classic opaque DOS block shadows (`█`).
- `:shadows off` / `:shadows none` — Disables drop shadows.

---

## 3. How to Turn On Full-Screen vs. Pane-Local Cross-Hair Cursors (`UI-05`)

Bilterm supports the classic **IBM 3270 horizontal + vertical cross-hair cursor ruler** in two modes:

1. **Keyboard Hotkey (`Alt+X`)**:
   - Press **`Alt+X`** once: Enables **Pane-Local Cross-Hairs** (horizontal and vertical ruler lines span the active pane's canvas).
   - Press **`Alt+X`** a second time: Switches to **Full-Screen Cross-Hairs** (horizontal and vertical ruler lines extend edge-to-edge across the entire terminal screen).
   - Press **`Alt+X`** a third time: Turns cross-hairs **Off**.
2. **Colon Commands**:
   - `:crosshair full` (or `:crosshair fullscreen`) — Turn on **full-screen** vertical & horizontal cross-hairs.
   - `:crosshair pane` (or `:crosshair on`) — Turn on **pane-local** vertical & horizontal cross-hairs.
   - `:crosshair off` — Turn off cross-hairs.
3. **`config.toml`**:
   ```toml
   crosshair_cursor = true
   crosshair_fullscreen = true
   ```

---

## 4. Operator Information Area (OIA) Status Bar Icons & Telemetry

Each composite pane's bottom 1-row **3270-OIA** status bar displays:
- **System State Icon & Badge**:
  - `● READY` (Green) — Shell ready at prompt
  - `⟳ RUNNING` (Cyan) — Foreground command streaming (`tail -f`, build, etc.)
  - `⇳ SCROLL` (Cyan) — Viewing scrollback history
  - `▣ ALT SCR` (Yellow) — Full-screen interactive TUI (`vim`, `htop`) active
  - `♦ !ECHO LOCK` (Red) — Password prompt (`sudo`, `ssh`) detected; floating prompt hidden & protected
- **Input Routing & Pin Mode**: `INPUT: CMD BOX` vs. `INPUT: RAW PTY`, and `PIN:TOP`, `PIN:BOT`, `PIN:FLT`, `PIN:SPL-T`, or `PIN:SPL-B`.
- **Accurate `⌖ R:xx C:xxx` Counter**:
  - In `INPUT: CMD BOX` mode, displays the live 1-based row and column of your command prompt cursor (`R:01 C:001` as you type or move across lines).
  - In `INPUT: RAW PTY` mode, displays the live 1-based row and column of the underlying VT100 terminal cursor.
- **Output Length Counter (`≣ OUT:<lines>L/<bytes>`)**:
  - Tracks total output lines and formatted byte volume processed by the pane (e.g. `≣ OUT:142L/8.4K` or `≣ OUT:12500L/1.2M`).

---

## 5. Cursor Customization (Shapes, Custom Characters, Blink Modes & Colors) (`UI-05`)

Bilterm supports full runtime and declarative customization of the command prompt & canvas cursor via **`:cursor`** or `~/.config/bilterm/config.toml`:

```text
# Open the interactive Cursor Style, Blink & Color Selector popup:
:cursor

# Shapes: block (█), hollow (▯), underline / underscore (_), bar / beam (▏), custom <char>
:cursor block
:cursor hollow
:cursor underline
:cursor bar
:cursor custom ◆
:cursor custom ❯

# Blink modes: blink (500ms), solid / steady (always visible), rapid (250ms), auto
:cursor blink
:cursor solid
:cursor rapid

# Colors: named presets (theme, green, amber, cyan, yellow, white, red, magenta, blue, orange, pink) or #RRGGBB hex
:cursor color amber
:cursor color #00ff66
:cursor color theme

# Compound one-liner commands (combine shape + blink + color in any order):
:cursor hollow blink amber
:cursor underline solid cyan
:cursor custom ◆ solid #ff55ff
:cursor reset
```

---

## 6. Interactive Text File & Theme Directory Browser (`:browse`, `:theme import`)

Import single theme files, entire directories (automatically scanning `schemes/`, `themes/`, `iterm2/`, `alacritty/`, `kitty/`, and `wezterm/` subdirectories), wildcard globs, or browse the filesystem interactively:

```text
# Open the interactive Text File & Theme Browser in the current or specified directory:
:browse
:files ~/styling/iterm2colors
:theme browse

# Import a directory (auto-discovers subdirectories like schemes/ and previews all imported themes):
:theme import /home/bill/styling/iterm2colors

# Import a wildcard glob or single .itermcolors / .toml / .yaml file (supports <Tab> path completion):
:theme import ~/styling/iterm2colors/schemes/*.itermcolors
:theme import "~/styling/iterm2colors/schemes/3024 Night.itermcolors"

# Initialize & inspect ~/.config/bilterm/config.toml and ~/.config/bilterm/themes/:
:theme config
```

Inside the **`:browse`** modal:
- **`Up` / `Down`** (or mouse wheel / click): Select directory or file
- **`Enter`** (or **`Right`** on a folder): Enter directory, import/apply theme file (`.itermcolors`, `.toml`, `.yaml`), or preview text file
- **`Backspace` / `Left` / `-`**: Navigate to parent directory (`..`)
- **Type any characters**: Live case-insensitive filename filter (`Backspace` clears filter first)
- **`Esc`**: Close browser

---

## 7. Declarative Configuration (`~/.config/bilterm/config.toml`) (`CFG-01`)

Bilterm automatically creates `~/.config/bilterm/config.toml` and `~/.config/bilterm/themes/` on first launch (or via `:theme config`).

```toml
theme = "ibm-3279-amber"
glyphs = "unicode"
cursor_style = "blinking-hollow-block" # block, hollow-block, blinking-block, blinking-hollow-block, underscore, blinking-underscore, bar, blinking-bar
cursor_blink = "blink"                 # auto, blink, solid, rapid
# cursor_char = "◆"                    # optional custom single-character cursor glyph
# cursor_color = "#FFB000"             # optional named color ("amber", "green", "cyan") or "#RRGGBB" hex
window_outline = "double"
prompt_outline = "rounded"
shadow_style = "theme-tinted"
crosshair_cursor = true
crosshair_fullscreen = true
drop_shadows = true
show_dos_fkey_bar = true
layout_profile = "default"
layout_autosave = true
history_enabled = true
history_max_files = 25          # retained session directories
history_session = "default"     # or: bilterm --session <name>
history_max_entries = 100000
history_load_limit = 10000
# history_dir = "/custom/path/to/bilterm-history"
default_prompt_anchor = "split-top"
default_prompt_height = 3
```

### Scrollback Memory (`[scrollback]`)

Scrollback is sized from the host's RAM instead of a fixed line count. Every pane starts at
`initial_lines`; roughly every 2 s (and on split, close, and resize) a pane that is at least 90%
full doubles its capacity while the shared budget has room. An active pane may borrow budget
that idle panes are not using. When total usage exceeds the budget, or available memory drops
below `low_memory_pct` of total RAM, the largest panes shrink toward an equal share, never
below `min_lines`. Panes on the alternate screen (`vim`, `htop`) are resized after they exit it.

```toml
[scrollback]
memory_budget = "auto"   # 10% of RAM (min 64 MiB): ~19 GiB on 192 GiB, ~1.6 GiB on 16 GiB
max_lines = "auto"       # or a hard per-pane cap, e.g. 1000000
min_lines = 2000
initial_lines = 10000
low_memory_pct = 10
```

Each line costs about `columns x 32 + 64` bytes, so 1.6 GiB holds roughly 650,000 lines of an
80-column pane. Run `:scrollback` to see the active pane's used/capacity lines and memory, and
`:scrollback clear` to drop its history.

---

[← Chapter 3: Window Manager](03-window-manager-and-z-stack.md) | [User Guide Home](README.md) | [Chapter 5: tmux Integration →](05-tmux-integration.md)
