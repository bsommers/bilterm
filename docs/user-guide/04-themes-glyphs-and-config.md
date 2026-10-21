# Chapter 4: Retro Mainframe & DOS Aesthetics, Outlines, Drop Shadows, Cross-Hairs & Configuration

## 1. Selectable Window & Pane Border Outlines

Customize the border outline style of workspace windows and the command prompt independently or together using `:outline` / `:border` or `config.toml`:

| Style Name | Top-Left / Horizontal / Top-Right | Command |
|------------|-----------------------------------|---------|
| `double` (Default) | `╔═══╗` | `:outline double` |
| `solid` | `┌───┐` | `:outline solid` |
| `rounded` | `╭───╮` | `:outline rounded` |
| `thick` | `┏━━━┓` | `:outline thick` |
| `dotted` | `┌┈┈┈┐` | `:outline dotted` |
| `dashed` | `┌┄┄┄┐` | `:outline dashed` |
| `quadruple` | `▛▀▀▀▜` | `:outline quadruple` |
| `ascii` | `+---+` | `:outline ascii` |

Target only windows or only the command prompt via:
- `:outline window <style>`
- `:outline prompt <style>`

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

## 5. Declarative Configuration (`~/.config/bilterm/config.toml`) (`CFG-01`)

```toml
theme = "ibm-3279-amber"
glyphs = "unicode"
cursor_style = "blinking-hollow-block"
window_outline = "double"
prompt_outline = "rounded"
shadow_style = "tinted"
crosshair_cursor = true
crosshair_fullscreen = true
drop_shadows = true
show_dos_fkey_bar = true
history_enabled = true
history_max_files = 25
# history_dir = "/custom/path/to/bilterm-history"
default_prompt_anchor = "split-top"
default_prompt_height = 3
```
