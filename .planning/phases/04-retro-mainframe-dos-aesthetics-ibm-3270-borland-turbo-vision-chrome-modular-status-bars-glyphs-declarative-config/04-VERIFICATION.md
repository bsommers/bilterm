---
phase: 04-retro-mainframe-dos-aesthetics-ibm-3270-borland-turbo-vision-chrome-modular-status-bars-glyphs-declarative-config
status: passed
verified: 2026-10-20
requirements_verified:
  - UI-01
  - UI-02
  - UI-03
  - UI-04
  - UI-05
  - CFG-01
score: 5/5
---

# Phase 4 Verification: Retro Mainframe & DOS Aesthetics — IBM 3270 + Borland Turbo Vision Chrome, Modular Status Bars, Glyphs & Declarative Config

## Goal-Backward Verification Summary

**Phase Goal:** As a retro-computing enthusiast, I want to customize Borland Turbo Vision window borders/shadows, CGA/VGA and IBM 3270 phosphor palettes, cross-hair cursors, Unicode/CP437/Nerd Font glyph sets, and modular OIA/F-key status bars via `config.toml`, so that Bilterm delivers an authentic mainframe and DOS IDE aesthetic tailored to my workflow.

**Verdict:** `status: passed` (5/5 observable truths verified, 6/6 requirements verified, 43/43 automated tests passing, 0 clippy warnings).

## Requirement Traceability

| Requirement | Description | Status | Verification Test |
|-------------|-------------|--------|-------------------|
| `UI-01` | Borland Turbo Vision frames (`╔═╗` / `┌─┐`), 2-col/1-row translucent drop shadows, `[1]` badges, `[▪]`/`[↕]`/`◢` widgets | **VERIFIED** | `ui::chrome::tests`, `snapshot_borland_turbo_vision_shadows_and_fkey_bar` |
| `UI-02` | Borland Turbo C/Pascal & IBM 3270 (3278 Green, 3279 Amber, Mono) color themes | **VERIFIED** | `ui::theme::tests`, `snapshot_ibm_3279_amber_cp437_crosshair_hollow_caret` |
| `UI-03` | Customizable IBM 3270 OIA status bar and Borland DOS Function-Key bar | **VERIFIED** | `snapshot_borland_turbo_vision_shadows_and_fkey_bar` |
| `UI-04` | Unicode, CP437, Dingbats, and Nerd Font glyph packs with strict cell-width alignment | **VERIFIED** | `ui::glyphs::tests::all_glyph_packs_preserve_strict_cell_width_alignment` |
| `UI-05` | IBM 3270 full-pane cross-hair cursor ruler and selectable caret shapes | **VERIFIED** | `ui::cursor::tests`, `snapshot_ibm_3279_amber_cp437_crosshair_hollow_caret` |
| `CFG-01` | Declarative `~/.config/bilterm/config.toml` and `.ini` theme file import | **VERIFIED** | `config::tests::parses_toml_config_and_ini_theme_overrides` |
