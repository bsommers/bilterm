---
phase: 1
slug: core-pty-vt100-engine-single-compositepane-prototype
status: approved
shadcn_initialized: false
preset: none
created: 2026-10-19
---

# Phase 1 — UI Design Contract

> Visual and interaction contract for Phase 1 TUI (`ratatui 0.30.2` + `crossterm 0.29.0`).

---

## Design System

| Property | Value |
|----------|-------|
| Tool | Ratatui 0.30.2 (`Buffer` immediate-mode compositor) |
| Preset | IBM 3270 / Borland Turbo Vision Hybrid TUI |
| Component library | `ratatui::widgets` (`Block`, `Borders`, `Clear`, `Paragraph`) |
| Icon library | Unicode Box Drawing (`╔═╗`, `┌─┐`), ASCII/CP437 indicators |
| Font | Host terminal monospace grid (cell-aligned 1x1 / 2x1 wide-char) |

---

## Spacing Scale

Declared values (TUI character-cell units mapped to 4px logical grid equivalents):

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px (1 col) | Inner prompt horizontal padding, status bar segment delimiter |
| sm | 8px (2 cols) | Drop shadow horizontal offset, status badge padding |
| md | 16px (3 rows) | Default `FloatingCommandPrompt` height (1 top border + 1 input line + 1 bottom border) |
| lg | 24px (100% - 1 row) | Full-height `OutputCanvas` area (`pane_rect.height.saturating_sub(1)`) |
| xl | 32px (1 row bottom) | Operator Information Area (OIA) bottom status bar |
| 2xl | 48px | Reserved for multi-line prompt expansion |
| 3xl | 64px | Full terminal viewport |

Exceptions: TUI operates on integer `(col, row)` terminal cells (`u16`). `OutputCanvas` always spans `pane_rect.height - 1` rows so moving/hiding the floating prompt never triggers `SIGWINCH`.

---

## Typography

| Role | Size | Weight | Line Height |
|------|------|--------|-------------|
| Body (`OutputCanvas` PTY cells) | 1 cell | Regular / ANSI Bold (`Modifier::BOLD`) | 1 row |
| Label (`OiaStatusBar` segments) | 1 cell | Bold (`Modifier::BOLD`) | 1 row |
| Heading (`FloatingCommandPrompt` title) | 1 cell | Bold (`Modifier::BOLD`) | 1 row |
| Display (Active cursor / mode badge) | 1 cell | Reversed (`Modifier::REVERSED`) | 1 row |

---

## Color

| Role | Value | Usage |
|------|-------|-------|
| Dominant (60%) | `#000000` / Terminal Default | `OutputCanvas` background & ANSI/TrueColor PTY cells |
| Secondary (30%) | `#0000AA` (Borland Blue) / `#002200` (3270 Green) | `FloatingCommandPrompt` surface & `OiaStatusBar` background |
| Accent (10%) | `#00FF66` (3278 Phosphor Green) / `#FFFF55` (Borland Yellow) | Active `FloatingCommandPrompt` double border (`╔═╗`) & `CMD BOX` mode badge |
| Destructive | `#FF5555` | `X SYSTEM` / `!ECHO (LOCKED)` password/raw-mode lock indicator |

Accent reserved for: Active `FloatingCommandPrompt` border/title and OIA active routing indicator (`INPUT: CMD BOX` vs `RAW PTY`).

---

## Copywriting Contract

| Element | Copy |
|---------|------|
| Primary CTA | `[Enter] Execute Command` / `[Ctrl+G] Toggle Raw PTY` |
| Empty state heading | `═══╣ BILTERM 3270 // COMMAND PROMPT ╠═══` |
| Empty state body | `Type a shell command and press Enter (Ctrl+G: Raw PTY, Ctrl+Q: Quit)` |
| Error state | `X SYSTEM — PTY process terminated or I/O error` |
| Destructive confirmation | `Quit Bilterm: Press Ctrl+Q to close active shell session` |

---

## UI Considerations

Applicable state considerations resolved: 5 covered, 0 backstop, 0 unresolved

| Category | Element(s) | Status | Resolution / Reason |
|----------|------------|--------|---------------------|
| empty | `FloatingCommandPrompt` & `OutputCanvas` | ✅ covered | Empty floating prompt renders bordered box with title `BILTERM` and cursor at `(0, 0)` over the live PTY canvas without occluding status bar |
| populated | `OutputCanvas` & `FloatingCommandPrompt` | ✅ covered | `OutputCanvas` renders VT100/ANSI/TrueColor cells across 100% of canvas height (`pane_height - 1`) while `FloatingCommandPrompt` uses `widgets::Clear` to overlay cleanly on top |
| loading / executing | `OiaStatusBar` & `InputRoutingMode` | ✅ covered | During command execution or `DECSET 47/1047/1049` alternate screen (`vim`/`htop`), `FloatingCommandPrompt` auto-hides and `OiaStatusBar` displays `RAW PTY` / `ALT SCREEN` |
| error / password lock | `InputRoutingMode::RawPtyPassthrough` | ✅ covered | When `!ECHO` (`sudo`/`ssh` password prompt) or foreground PGID mismatch is detected, input routes directly to PTY without echoing cleartext in `FloatingCommandPrompt` and OIA displays `!ECHO` / `RAW PTY` |
| overflow / resize | `OutputCanvas` & `FloatingCommandPrompt` | ✅ covered | Moving or hiding `FloatingCommandPrompt` never alters PTY `Winsize` (`SIGWINCH` count = 0); host terminal resize clamps prompt rect within viewport bounds |

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| shadcn official | none (Rust Ratatui TUI) | not required |

---

## Checker Sign-Off

- [x] Dimension 1 Copywriting: PASS
- [x] Dimension 2 Visuals: PASS
- [x] Dimension 3 Color: PASS
- [x] Dimension 4 Typography: PASS
- [x] Dimension 5 Spacing: PASS
- [x] Dimension 6 Registry Safety: PASS

**Approval:** approved 2026-10-19
