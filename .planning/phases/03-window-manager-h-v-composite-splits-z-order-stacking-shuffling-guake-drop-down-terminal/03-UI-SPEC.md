# Phase 3: Window Manager — H/V Composite Splits, Z-Order Stacking/Shuffling & Guake Drop-Down Terminal - UI Design Specification

**Phase:** 3
**UI Hint:** yes

## 1. Multi-Pane Layout & Painter's Algorithm Z-Order Compositing

```text
+--------------------------------------------------------------------------------+
| ╔═[Q] QUAKE QUICK TERM [~] (Alt+` to hide) ════════════════════════════[▲][▼]═╗ | <- Pass 3: Guake Drop-Down
| ║ ==> htop                                                                    ║ |    Overlay (if visible)
| ╚═════════════════════════════════════════════════════════════════════════════╝ |
| ┌─[1] PANE #1 ───────────────────────┐╔═[2] PANE #2 (ACTIVE) ─────────────────╗ | <- Pass 1/2: Tiled Splits
| │ ╔═╣ BILTERM 3270 // CMD [TOP] ╠══╗ │║ ╔═╣ BILTERM 3270 // CMD [TOP] ╠═[▲][▼]║ |    or Z-Order Stacked
| │ ║ ==> ls                         ║ │║ ║ ==> cargo test                      ║ |    CompositePanes
| │ ╚════════════════════════════════╝ │║ ╚═════════════════════════════════════╝ |
| │ file1.rs                           │║ running 27 tests...                     |
| │ 3270-OIA│ READY │ INPUT: CMD BOX   │║ 3270-OIA│ READY │ INPUT: CMD BOX │Z:2/2 |
+--------------------------------------------------------------------------------+
```

## 2. Z-Order Stacking & Deck Shuffling (`WM-02`)
- **Z-Order Stack (`z_stack: Vec<PaneId>`)**:
  - Bottom of deck: `z_stack[0]`, Top of deck (focused): `z_stack[z_stack.len() - 1]`.
  - **Rotate Top-to-Bottom (`Alt+Tab` / `F6` / `:deck next`)**: Moves the current top pane to the bottom of the deck (`0`) and promotes the next pane to the top.
  - **Rotate Bottom-to-Top (`Alt+Shift+Tab` / `:deck prev`)**: Pulls the bottom pane (`0`) to the top of the deck.
  - **Selective Raise (`Alt+1`..`Alt+9` / Mouse Click / `:focus <id>`)**: Plucks `pane_id` from its current position in `z_stack` and pushes it to the top (`z_stack.push(pane_id)`).
- **OIA Stack Telemetry**: Displays `Z:k/N` (where `k` is the 1-based z-order rank and `N` is total panes) when multiple panes exist.

## 3. Wide-Character Boundary Sanitization (`WM-04`)
- Before rendering any overlay (`FloatingCommandPrompt`, overlapping `CompositePane` in Stack/Cascade mode, or `QuickTerminal`), `occlude_rect(buf, rect)` scans every row `y in rect.y..rect.y + rect.height`:
  1. **Left boundary (`rect.x`)**: If `rect.x > buf.area.x`, inspects `buf[(rect.x - 1, y)]`. If `UnicodeWidthStr::width(symbol) >= 2`, replaces `buf[(rect.x - 1, y)]` symbol with `" "` so the wide character's left half does not bleed into `rect.x`.
  2. **Right boundary (`rect.x + rect.width`)**: If `rect.x + rect.width < buf.area.x + buf.area.width` and `rect.width > 0`, inspects `buf[(rect.x + rect.width - 1, y)]`. If `UnicodeWidthStr::width(symbol) >= 2`, replaces `buf[(rect.x + rect.width, y)]` symbol with `" "` so no orphaned right-half continuation cell remains outside the right border.
  3. Invokes `Clear.render(rect, buf)`.
