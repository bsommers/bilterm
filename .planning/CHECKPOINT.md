# Session Checkpoint (architecture-review follow-ups)

**Updated:** `2026-10-22`
**Branch:** `fix/architecture-review-p0` (pushed to origin @ `e018ad7`)

## 1. User decisions (verbatim)
- "1 - yes" → raised MSRV to 1.88 and bumped `time` (done, `e1ea4a1`).
- "2 - wire-up ghostty-vt feature if it helps or provides some solid footing for future work" → done (`e018ad7`). Needs Rust 1.90+ and Zig 0.15.2; its build script git-clones Ghostty, so the first build needs network.
- "3 - commit and push" → done. Branch pushed; no PR or merge to main yet.
- "execute this as autonomously as possible … conserve tokens, pass minimal sufficient context to subagents, compact or clear …"
- New request: "persistent session-segmented command histories that survive individual session runs" and "massive scrollback buffers … based on system resources (192 GB vs 16 GB) … expand or contract … save RAM when not needed … dynamic, scaled allocations".

## 2. Done
- History merged `3dbb528`; scrollback merged `721e8cf`. Both pushed to `origin/fix/architecture-review-p0`; worktrees removed. Verified: clippy clean, 127 tests passed, 0 failed.

## 3. Next steps
- Optional: open a PR or merge into `main`. Not requested yet.
- Scrollback follow-ups:
  - Rebuild runs on the UI thread, so growing or resizing a pane with about 1M lines can stall the UI. Move it to the worker.
  - Within the 2 s rebalance window a flood can drop the oldest lines.
  - DECSTBM isn't restored after a rebuild.
  - The worker and ghostty paths still use fixed capacities.
  - The 64 KB regression test was never run against the old code to watch it fail.

## 4. Known open items
- `cargo fmt --check` already failed across the repo before this work started (left as-is).
- The `zig_0_15` attribute in `flake.nix` hasn't been checked with nix.
- A palette-indexed underline colour maps to `None` in the Ghostty backend.

- History follow-ups: old per-run log files are not migrated; two processes sharing one session can lose a line during compaction.
- Main checkout has user-owned uncommitted changes (`docs/README.md`, `docs/launch-presentation-and-pitch-script.md`); don't touch them.
