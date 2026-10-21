# Phase 4: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config - UI Design Specification

**Phase:** 4
**UI Hint:** yes

## 1. Borland Turbo Vision Chrome & Translucent Character-Cell Drop Shadows (`UI-01`)

```text
╔═[▪]═╣ [1] PANE #1 (Z:2/2) ╠═════════════════════════════════════════[▲][▼][↕]═╗
║ $ ls -la                                                                      ║░░
║ ╔═╣ BILTERM 3270 // CMD [TOP] ╠═══════════════════════════════════════[▲][▼]╗ ║░░
║ ║ ==> cargo test --all-targets                                              ║░░║░░
║ ╚══════════════════════════════════════════════════════════════════════════◢╝░░║░░
║   ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░║░░
║ 3270-OIA│ READY │ INPUT: CMD BOX │ PIN:TOP │ R:02 C:001                       ║░░
╚══════════════════════════════════════════════════════════════════════════════◢╝░░
  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
F1 Help  F2 SplitH  F3 SplitV  F5 Zoom  F6 Next  F10 Quake  F12 Raw     [TURBO-BLUE]
```

- **Translucent Character-Cell Drop Shadow**:
  - Casts a **2-column right shadow** (`x = rect.right()..rect.right() + 2`, `y = rect.y + 1..rect.bottom() + 1`) and **1-row bottom shadow** (`x = rect.x + 2..rect.right() + 2`, `y = rect.bottom()`).
  - Preserves the underlying character glyph in `buf[(x, y)]` while mutating its `Style` to `fg(Color::DarkGray).bg(Color::Black).remove_modifier(Modifier::BOLD)` (authentic Borland Turbo Vision attribute-byte `0x08` shadow effect!).
- **Clickable Window Border Widgets**:
  - Top-left: `[▪]` close button
  - Top-right: `[▲][▼]` prompt pin buttons and `[↕]` zoom/maximize button
  - Bottom-right corner: `◢` resize corner glyph

## 2. Color Theme Presets (`UI-02`)

| Theme Preset | Surface BG | Active Border FG | Inactive Border FG | Title FG | Crosshair BG | OIA BG |
|--------------|------------|------------------|--------------------|----------|--------------|--------|
| `turbo-blue` (Default) | `#001C38` | `#00FF66` | `#55FFFF` | `#FFFF55` | `#002B55` | `#000080` |
| `turbo-cyan` | `#008080` | `#FFFF55` | `#FFFFFF` | `#FFFF55` | `#005555` | `#004040` |
| `ibm-3278-green` | `#051405` | `#00FF66` | `#008833` | `#55FF55` | `#003300` | `#002200` |
| `ibm-3279-amber` | `#180C00` | `#FFB000` | `#996600` | `#FFD700` | `#331900` | `#241200` |
| `ibm-mono` | `#000000` | `#FFFFFF` | `#888888` | `#FFFFFF` | `#222222` | `#1A1A1A` |
