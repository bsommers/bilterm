# Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config - Context

**Gathered:** 2026-10-20
**Status:** Ready for planning
**Mode:** Autonomous

<domain>
## Phase Boundary

Phase 4 layers authentic **IBM 3270 Mainframe** and **DOS Borland Turbo Vision (Turbo C / Turbo Pascal IDE)** visual chrome, customizable glyph packs, cross-hair cursors, modular status bars, and declarative configuration (`~/.config/bilterm/config.toml` + `.ini` theme import) across Bilterm (`UI-01..05`, `CFG-01`):
1. **Borland Turbo Vision Window Chrome & Translucent Drop Shadows (`UI-01`)**: Active composite panes and floating prompts render double-line box-drawing borders (`╔═╗`), inactive panes render single-line borders (`┌─┐`), floating prompts and stacked windows cast a 2-column right / 1-row bottom translucent character-cell drop shadow (dimming underlying foreground/background colors while preserving underlying text characters!), window number badges (`[1]`, `[2]`), and clickable border widgets (`[▪]` close, `[↕]` zoom/maximize, `◢` resize corner).
2. **Retro Mainframe & DOS Color Themes (`UI-02`)**: Built-in presets for **Borland Turbo C / Turbo Pascal** (`turbo-blue`, `turbo-cyan`), **IBM 3270 Mainframe** (`ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`), and custom user-defined RGB/indexed palettes.
3. **Customizable OIA & Borland DOS Function-Key Status Bars (`UI-03`)**: Per-pane IBM 3270 Operator Information Area (OIA) status bar (`READY`, `!ECHO LOCK`, `ALT SCR`, `SCROLL`, `INPUT: CMD BOX` vs `RAW PTY`, `[PIN:TOP/BOT/FLT]`, `[BLK k/N]`, `Z:k/N`, `R:rr C:ccc`) plus the bottom Borland DOS Function-Key hotkey bar (`F1 Help  F2 SplitH  F3 SplitV  F5 Zoom  F6 Next  F10 Quake  F12 Raw`).
4. **Unicode, CP437, Dingbats & Nerd Font Glyph Packs (`UI-04`)**: Configurable `GlyphSet` presets (`unicode`, `cp437`, `nerd-font`, `ascii`) with strict `unicode-width` cell-alignment enforcement so decorative badges (`◄`, `►`, `▲`, `▼`, `▪`, `↕`, `◢`, `╬`, ``, ``) never misalign borders.
5. **IBM 3270 Full-Pane Cross-Hair Cursor & Caret Styles (`UI-05`)**: Non-destructive software cross-hair ruler tinting all cells in the active pane sharing `cursor_row` or `cursor_col` with the theme's cross-hair tint color, plus selectable caret shapes (`SolidBlock`, `BlinkingHollowBlock`, `Underscore`, `VerticalBar`).
6. **Declarative Configuration (`CFG-01`)**: `BiltermConfig` loaded from `~/.config/bilterm/config.toml` (with `parse_ini_theme` support for `.ini` theme files) configuring default prompt anchor/height, theme preset, glyph pack, cursor style, cross-hair toggle, drop-shadow toggle, and status bar visibility.

</domain>
