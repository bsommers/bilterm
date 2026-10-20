---
phase: 2
slug: floating-prompt-ergonomics-triple-modality-intra-pane-controls-command-blocks
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-20
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` + `insta` (`ratatui::backend::TestBackend`) |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all-targets` |
| **Estimated runtime** | ~3 seconds |

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 2-01-01 | 01 | 1 | PANE-04 | — | Multi-line editing, undo/redo, and shell syntax highlighting | unit | `cargo test pane::highlight::` | ✅ | ⬜ pending |
| 2-01-02 | 01 | 1 | PANE-04 | — | Fuzzy command history search via `nucleo-matcher` (`Ctrl+R`) | unit | `cargo test pane::fuzzy_history::` | ✅ | ⬜ pending |
| 2-02-01 | 02 | 2 | PTY-04, PANE-05 | T-2-01 | Bracketed paste sanitization (`DECSET 2004`), `OSC 52` clipboard, and `OSC 133` command block jumping | unit | `cargo test vt::` | ✅ | ⬜ pending |
| 2-02-02 | 02 | 2 | PANE-02, PANE-03 | — | Triple-modality prompt move/pin/resize (keyboard, mouse drag/click, and `:command` parser) | unit | `cargo test input::` | ✅ | ⬜ pending |
| 2-03-01 | 03 | 3 | PANE-02, PANE-03, PANE-04, PANE-05 | — | `TestBackend` + `insta` snapshots of multi-line highlighted prompt, fuzzy search overlay, and command block jump | snapshot | `cargo test --test phase2_prompt_ergonomics` | ✅ | ⬜ pending |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-20
