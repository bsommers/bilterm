---
phase: 04-retro-mainframe-dos-aesthetics-ibm-3270-borland-turbo-vision-chrome-modular-status-bars-glyphs-declarative-config
plan: 02
subsystem: borland-chrome-crosshair-cursor-fkey-bar
tags: [rust, ratatui, borland-turbo-vision, drop-shadow, ibm-3270-crosshair, fkey-bar]
provides:
  - Borland Turbo Vision window frame (`render_borland_frame`) and translucent 2-col/1-row drop shadow (`render_drop_shadow`) in `src/ui/chrome.rs`
  - IBM 3270 full-pane cross-hair cursor (`render_crosshair_cursor`) and selectable carets (`CaretStyle`, `render_caret`) in `src/ui/cursor.rs`
  - Borland DOS Function-Key bar (`render_dos_fkey_bar`) in `src/ui/status_bar.rs`
affects: [src/ui/chrome.rs, src/ui/cursor.rs, src/ui/status_bar.rs]
tech-stack:
  added: []
  patterns: [Translucent Character-Cell Drop Shadow, Software Full-Pane Cross-Hair Cursor Ruler, DOS Function-Key Bar]
key-files:
  created: [src/ui/chrome.rs, src/ui/cursor.rs, src/ui/status_bar.rs]
  modified: []
key-decisions:
  - "Implemented translucent character-cell drop shadows (`DarkGray` on `Black`) that preserve underlying character glyphs rather than erasing them"
requirements-completed: [UI-01, UI-03, UI-05]
duration: 3min
completed: 2026-10-20
---

# Phase 4 Plan 02: Borland Chrome, Translucent Drop Shadows, Cross-Hair Cursor & DOS F-Key Bar Summary

**Implemented `render_borland_frame`/`render_drop_shadow` (`src/ui/chrome.rs`), `render_crosshair_cursor`/`render_caret` (`src/ui/cursor.rs`), and `render_dos_fkey_bar` (`src/ui/status_bar.rs`).**
