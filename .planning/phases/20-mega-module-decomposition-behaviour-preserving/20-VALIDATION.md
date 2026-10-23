---
phase: 20
slug: mega-module-decomposition-behaviour-preserving
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-23
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` + `#[tokio::test]` + `insta` snapshot assertions |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test --lib <module>` (`vt`, `pane`, `colon_command`, `input`, `api`, `ui`, `wm`) |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~15 seconds incremental |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib <module>` and `cargo clippy --all-targets -- -D warnings`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite + line-count gate + cycle gate must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|----------|-----------|-------------------|--------|
| 20-01-01 | 01 | 1 | ARCH-R04 | Extract `vt/{codec,graphics,detect,recording}.rs` & move `cell_to_ratatui_style` to `ui/` | unit + integration | `cargo test --lib vt:: && cargo test --test terminal_torture_and_stress` | ⬜ pending |
| 20-01-02 | 01 | 1 | ARCH-R04 | Extract `vt/scan.rs`, `vt/tests.rs`, `vt/scrollback_tests.rs`; define `ARCH-R01..05` in `REQUIREMENTS.md` | unit + line check | `cargo test --lib vt:: && awk 'FNR==1{f=0} ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/vt/*.rs` | ⬜ pending |
| 20-02-01 | 02 | 1 | ARCH-R04 | Extract `pane/{prompt_editor,modes,history_store_tests}.rs` | unit | `cargo test --lib pane:: && awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/pane/prompt_editor.rs src/pane/modes.rs src/pane/history_store.rs` | ⬜ pending |
| 20-02-02 | 02 | 1 | ARCH-R04 | Move `colon_command` to `src/colon_command/{mod,parse,execute}.rs` and eliminate `pane ↔ wm` cycle | unit + integration | `cargo test --lib colon_command:: && test $(grep -rn "crate::wm\|crate::colon_command" src/pane/ | wc -l) -eq 0` | ⬜ pending |
| 20-03-01 | 03 | 2 | ARCH-R04 | Split `src/input/mod.rs` into `encode.rs`, `mouse.rs`, `modal.rs`, `keymap.rs`, `mod.rs` | unit + line check | `cargo test --lib input:: && awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/input/*.rs` | ⬜ pending |
| 20-04-01 | 04 | 2 | ARCH-R04 | Extract `api/{json,playbook,mcp}.rs` from `src/api/mod.rs` | unit + integration | `cargo test --lib api:: && cargo test --test v110_v120_v130_automation_api_mcp_playbooks` | ⬜ pending |
| 20-04-02 | 04 | 2 | ARCH-R04 | Extract `api/{rpc,server,tests}.rs` and thin `src/api/mod.rs` | unit + integration | `cargo test --lib api:: && cargo test --test phase19_security_cli && awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/api/*.rs` | ⬜ pending |
| 20-05-01 | 05 | 3 | ARCH-R04 | Split `ui/mod.rs` (`modals.rs`, `status_bar.rs`) and `wm/mod.rs` (`session.rs`) so all `src/**/*.rs` <= 800 lines | full suite + line gate | `cargo test && cargo clippy --all-targets -- -D warnings && awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' $(find src -name '*.rs')` | ⬜ pending |
| 20-05-02 | 05 | 3 | ARCH-R04 | Update `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md` for Phase 20 completion | grep | `grep -q "ARCH-R04.*Complete" .planning/REQUIREMENTS.md` | ⬜ pending |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: every task has automated test + line-count verification
- [x] Wave 0 complete (existing 138 unit + 10 integration test targets cover all decomposed modules)
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-23
