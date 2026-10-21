# Chapter 4: Retro Mainframe & DOS Aesthetics, Themes, Glyphs & Configuration

## 1. Borland Turbo Vision Chrome & Translucent Drop Shadows (`UI-01`)

- **Active vs. Inactive Frames**: Active windows render classic Borland double-line borders (`╔═╗`); inactive windows render single-line borders (`┌─┐`), complete with `[1]` window number badges, `[↕]` zoom and `[▪]` close widgets, and a `◢` resize corner.
- **Translucent Character-Cell Drop Shadows**: Enable via `:shadows on` or `drop_shadows = true` in `config.toml`. Floating prompts, stacked windows, and the Guake overlay cast a **2-column right / 1-row bottom translucent shadow** (`DarkGray` on `Black`) that dims underlying text while keeping the characters beneath legible.

---

## 2. Color Theme Presets (`UI-02`)

Switch themes live at any time via `:theme <name>`:
- `:theme turbo-blue` — Borland Turbo C / Turbo Pascal Royal Blue + Yellow/Cyan/Green (Default)
- `:theme turbo-cyan` — Classic Borland Turbo Vision Cyan dialog palette
- `:theme 3278` (`ibm-3278-green`) — IBM 3278 Model 2 Phosphor Green mainframe palette
- `:theme 3279` (`ibm-3279-amber`) — IBM 3279 / 3290 Gas-Plasma Amber mainframe palette
- `:theme mono` (`ibm-mono`) — High-contrast monochrome mainframe palette

---

## 3. IBM 3270 Cross-Hair Cursor Ruler & Custom Carets (`UI-05`)

- **Cross-Hair Cursor Ruler**: Run `:crosshair on` to enable a full-pane IBM 3270 horizontal + vertical cross-hair cursor ruler that non-destructively tints the active cursor row and column across the pane.
- **Caret Shapes**: Switch software caret shapes via `:cursor <style>`:
  - `:cursor block` — Inverted solid block (`█`)
  - `:cursor hollow` — Blinking hollow block (`▯`)
  - `:cursor underscore` — Classic DOS underscore (`_`)
  - `:cursor bar` — Vertical beam (`▏`)

---

## 4. Unicode, CP437 & Nerd Font Glyph Packs (`UI-04`)

Switch glyph packs live via `:glyphs <pack>`:
- `:glyphs unicode` — Standard Unicode box-drawing and geometric dingbats (`[▲][▼]`, `[▪]`, `[↕]`, `◢`, `==> `)
- `:glyphs cp437` — Classic IBM PC Code Page 437 symbols (`♦=> `, `[■]`, `╬`, `║`)
- `:glyphs nerd-font` — Powerline & Nerd Font symbols (`❯=> `, ``)
- `:glyphs ascii` — Pure 7-bit ASCII fallback (`[^][v]`, `[x]`, `[+]`, `/`)

---

## 5. Declarative Configuration (`~/.config/bilterm/config.toml`) (`CFG-01`)

```toml
theme = "ibm-3279-amber"
glyphs = "cp437"
cursor_style = "blinking-hollow-block"
crosshair_cursor = true
drop_shadows = true
show_dos_fkey_bar = true
default_prompt_anchor = "top"
default_prompt_height = 3
```
