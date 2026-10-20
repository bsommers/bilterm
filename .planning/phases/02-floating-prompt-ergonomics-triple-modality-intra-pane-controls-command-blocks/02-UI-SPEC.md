---
phase: 2
slug: floating-prompt-ergonomics-triple-modality-intra-pane-controls-command-blocks
status: approved
shadcn_initialized: false
preset: none
created: 2026-10-20
---

# Phase 2 — UI Design Contract

> Visual and interaction contract for Phase 2 Floating Prompt Ergonomics, Triple-Modality Controls & Command Blocks.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | Ratatui 0.30 (`Buffer` compositor) |
| Preset | IBM 3270 / Borland Turbo Vision Hybrid TUI |
| Component library | `ratatui::widgets` (`Block`, `BorderType`, `Clear`, `Paragraph`) |
| Icon library | Box Drawing + `[▲]` Top Pin, `[▼]` Bottom Pin, `[↕]` Resize handle |
| Font | Host terminal monospace grid |

---

## Spacing Scale

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px (1 col) | Prompt prefix and border widget spacing |
| sm | 8px (2 cols) | Horizontal margin of floating prompt |
| md | 16px (3..12 rows) | Dynamic / user-resizable `FloatingCommandPrompt` height |
| lg | 24px | Full-height `OutputCanvas` underneath |
| xl | 32px (1 row) | Bottom OIA status bar |

Exceptions: TUI cell coordinates (`u16`).

---

## Typography

| Role | Size | Weight | Line Height |
|------|------|--------|-------------|
| Command Token | 1 cell | Bold (`Modifier::BOLD`) | 1 row |
| Flag / String / Variable Token | 1 cell | Regular | 1 row |
| Error Token (unclosed quote) | 1 cell | Underlined (`Modifier::UNDERLINED`) | 1 row |
| Fuzzy Search Header | 1 cell | Bold Reversed | 1 row |

---

## Color

| Role | Value | Usage |
|------|-------|-------|
| Dominant (60%) | `#000000` / Reset | `OutputCanvas` cells |
| Secondary (30%) | `#001C38` | `FloatingCommandPrompt` background |
| Accent (10%) | `#00FF66` / `#FFFF55` | Active border (`╔═╗`), command token highlight, fuzzy search match highlight |
| Destructive | `#FF5555` | Syntax error token (unclosed string/quote) & non-zero command block exit status |

Accent reserved for: Active prompt border, valid command executable token, and active fuzzy-history match.

---

## Copywriting Contract

| Element | Copy |
|---------|------|
| Primary CTA | `[Enter] Run  [Shift+Enter] Newline  [Ctrl+R] History  [:cmd] Control` |
| Empty state heading | `╣ BILTERM 3270 // CMD [TOP] [▲][▼] ╠` |
| Empty state body | `==> ` |
| Error state | `Unknown :command (try :prompt top|bottom|float <y>|height <h>, :block prev|next, :clear, :q)` |
| Destructive confirmation | `:quit closes the active Bilterm session` |

---

## UI Considerations

Applicable state considerations resolved: 5 covered, 0 backstop, 0 unresolved

| Category | Element(s) | Status | Resolution / Reason |
|----------|------------|--------|---------------------|
| empty | `FloatingCommandPrompt` | ✅ covered | Renders single-line `==> ` prompt with `[▲][▼]` pin widgets |
| populated (multi-line) | `FloatingCommandPrompt` | ✅ covered | Multi-line shell input auto-expands or scrolls within prompt box with live token syntax highlighting |
| fuzzy-search | `FloatingCommandPrompt` (`Ctrl+R`) | ✅ covered | Renders `SEARCH (Ctrl+R): <query> -> <best match>` in the floating prompt title/footer using `nucleo-matcher` |
| mouse-drag-resize | `FloatingCommandPrompt` border | ✅ covered | Dragging top border moves prompt (`PromptAnchor::Floating`); dragging bottom border resizes prompt height; clicking `[▲]`/`[▼]` pins Top/Bottom |
| command-blocks | `OutputCanvas` & `OiaStatusBar` | ✅ covered | `Alt+[` / `Alt+]` or `:block prev`/`next` jumps scrollback to recorded `OSC 133` / command block boundaries and shows `[BLK k/N]` in OIA |

---

## Checker Sign-Off

- [x] Dimension 1 Copywriting: PASS
- [x] Dimension 2 Visuals: PASS
- [x] Dimension 3 Color: PASS
- [x] Dimension 4 Typography: PASS
- [x] Dimension 5 Spacing: PASS
- [x] Dimension 6 Registry Safety: PASS

**Approval:** approved 2026-10-20
