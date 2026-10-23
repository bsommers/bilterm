# Session Checkpoint (architecture-review P0 fixes)

**Updated:** `2026-10-22`
**Branch:** `fix/architecture-review-p0` (from `main` @ `b480b9e`; uncommitted work)
**Previous checkpoint (v1.5.0):** see `git show b480b9e:.planning/CHECKPOINT.md`

## 1. User decisions (verbatim)
- After the architecture review (`.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md`), user said: **"yes. do all 1,2,3"**, meaning:
  1. Fix the P0 items (P0.1–P0.4), each with a regression test.
  2. Run a dependency audit.
  3. Break the P1 items into GSD phases (ROADMAP).
- Nothing has been committed or pushed. Committing still needs explicit user approval.

## 2. Progress
- [x] P0.1 PTY truncation removed (`src/pane/mod.rs`: new `ingest_pty_events` seam + test `large_pty_bursts_are_processed_without_dropping_mode_changes`). Red → green verified.
- [x] P0.2 Config loaded at startup (`BiltermConfig::default_path`/`load`, `WindowManager::apply_config`, errors shown in OIA).
- [x] P0.3 Soundness: `GhosttyCAbiTerminal::with_vtable` now private + SAFETY invariant. **Open:** `ghostty-vt` feature still doesn't build (needs `libghostty-vt` 0.2.2 dep + Zig; user decision); marked EXPERIMENTAL in Cargo.toml.
- [x] P0.4 RPC daemon: actor-owned plane, per-connection tasks, 1 MiB line cap, JSON depth 128. Live repro passes.
- [x] `tokio` → 1.54.1. **Open:** `time` fix (0.3.47) needs MSRV 1.88 > declared 1.86 (user decision).
- [x] P1 → ROADMAP phases 19–23 (Plans TBD via `/gsd-plan-phase`).
- Verified: clippy clean, 104 tests passed, 0 failed.
- [x] Audit run (`cargo audit`): 2 vulns (`time 0.3.45` → `>=0.3.47`; `tokio 1.54.0` → `>=1.54.1`, yanked). Warnings: `lru 0.12.5` (unsound) and `paste` (unmaintained) come via `vt100-ctt → ratatui 0.29`; `lru 0.16.4` (unsound) via `ratatui-core`. `vt100-ctt` duplicates ratatui (0.29 + 0.30).
- [ ] Apply `cargo update -p time -p tokio`.
- [ ] P1 → GSD phases 6+ in `.planning/ROADMAP.md`.

## 3. Resume verification command
`cargo test && cargo clippy --all-targets -- -D warnings && cargo audit`
