---
phase: 04-retro-mainframe-dos-aesthetics-ibm-3270-borland-turbo-vision-chrome-modular-status-bars-glyphs-declarative-config
plan: 01
subsystem: theme-glyphs-config
tags: [rust, ratatui, themes, glyphs, toml, ini]
provides:
  - Retro color theme engine (`ThemePreset`, `ThemePalette` in `src/ui/theme.rs`)
  - Unicode/CP437/Nerd Font/ASCII glyph registry (`GlyphPack`, `GlyphSet` in `src/ui/glyphs.rs`)
  - Declarative TOML + INI theme configuration (`BiltermConfig`, `parse_ini_theme` in `src/config/mod.rs`)
affects: [src/ui/theme.rs, src/ui/glyphs.rs, src/config/mod.rs]
tech-stack:
  added: [serde, toml]
  patterns: [Theme Palette Registry, Cell-Width Validated Glyph Packs, Declarative TOML/INI Config]
key-files:
  created: [src/ui/theme.rs, src/ui/glyphs.rs, src/config/mod.rs]
  modified: [src/lib.rs]
key-decisions:
  - "Validated every GlyphSet preset with unicode-width to guarantee deterministic cell alignment across Unicode, CP437, Nerd Font, and ASCII"
requirements-completed: [UI-02, UI-04, CFG-01]
duration: 3min
completed: 2026-10-20
---

# Phase 4 Plan 01: Retro Theme Engine, Glyph Registry & Declarative Config Summary

**Implemented `ThemePreset`/`ThemePalette` (`src/ui/theme.rs`), `GlyphPack`/`GlyphSet` (`src/ui/glyphs.rs`), and `BiltermConfig`/`parse_ini_theme` (`src/config/mod.rs`).**
