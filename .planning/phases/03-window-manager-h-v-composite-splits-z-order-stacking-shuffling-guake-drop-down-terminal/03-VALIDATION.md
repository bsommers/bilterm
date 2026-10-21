---
phase: 3
slug: window-manager-h-v-composite-splits-z-order-stacking-shuffling-guake-drop-down-terminal
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-20
---

# Phase 3 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `insta 1.42` + `ratatui::backend::TestBackend` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all-targets && cargo clippy --all-targets -- -D warnings` |

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Automated Command | Status |
|---------|------|------|-------------|-------------------|--------|
| 03-01-01 | 01 | 1 | `WM-01`, `WM-02`, `WM-04` | `cargo test --lib wm` | ⬜ pending |
| 03-02-01 | 02 | 2 | `WM-03`, `WM-05` | `cargo test --lib` | ⬜ pending |
| 03-03-01 | 03 | 3 | `WM-01..05` | `cargo test --test phase3_window_manager` | ⬜ pending |
