---
phase: 50-herdr-style-left-sidebar-agent-aware-workspace-navigator
plan: 01
subsystem: testing
tags: [insta, snapshot, golden, drawer, parity, ratatui]
status: complete

requires:
  - phase: 46
    provides: GUI headless frame pattern, display modes
provides:
  - "11 committed classic-drawer goldens (symbols + style digest; GUI quad/glyph/rgba digest)"
  - "scripts/check_classic_goldens.sh immutability gate"
affects: [50-02, 50-03, 50-04, 50-05, 50-06, 50-07, 50-08, 50-09, 50-10, 50-11, 50-12, 50-13]

actuals:
  tokens: 16000
  tasks: 2
  commits: 3
plan_head_before: 091299a6becce4cbd0491b57bc54c84414c3f59c
plan_head_after: f45dee258e2cd64aea40f2a8bf86d950d31f8bce

tech-stack:
  added: []
  patterns:
    - "Style digest: FNV-1a 64 over symbol|fg|bg|modifier per cell, appended to the snapshot text"
    - "ENV_LOCK + temp XDG_STATE_HOME isolation for drawer tests (list_sessions reads saved sessions)"

key-files:
  created:
    - tests/drawer_classic_parity.rs
    - scripts/check_classic_goldens.sh
    - tests/snapshots/drawer_classic_parity__legacy_left.snap
    - tests/snapshots/drawer_classic_parity__legacy_right.snap
    - tests/snapshots/drawer_classic_parity__legacy_top.snap
    - tests/snapshots/drawer_classic_parity__legacy_bottom.snap
    - tests/snapshots/drawer_classic_parity__modern_left.snap
    - tests/snapshots/drawer_classic_parity__liquid_glass_left.snap
    - tests/snapshots/drawer_classic_parity__maximized_left.snap
    - tests/snapshots/drawer_classic_parity__overlay_left.snap
    - tests/snapshots/drawer_classic_parity__session_tree_left.snap
    - tests/snapshots/drawer_classic_parity__quick_terminal_fleet_left.snap
    - tests/snapshots/drawer_classic_parity__gui_surface_left.snap
  modified: []

key-decisions:
  - "Goldens were generated on the untouched src/ tree and must never be regenerated in Phase 50 (D-02)"
  - "Quick-terminal golden pins the fleet group with the quake overlay hidden, because the visible overlay occludes the left drawer"

requirements-completed: [FLT-01]

coverage:
  - id: D1
    description: "Classic drawer output (cells and styles) pinned for every edge and look by committed goldens"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "cargo test -q --test drawer_classic_parity (11 passed, repeated runs deterministic)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Immutability gate proves each golden committed once and clean"
    requirement: FLT-01
    verification:
      - kind: other
        ref: "scripts/check_classic_goldens.sh -> GOLDENS OK 11"
        status: pass
    human_judgment: false
---

# Phase 50 Plan 01: Classic Drawer Parity Goldens Summary

**11 insta goldens (cell symbols plus an FNV-1a style digest, and a GUI quad/glyph/rgba digest) pinning today's classic slide-out drawer, with a git-history gate script, all created before any Phase 50 `src/` edit.**

## Performance

- Tasks: 2 (1 tracer, 1 auto)
- Files: 13 created, 0 modified; `src/`, `Cargo.toml`, `Cargo.lock` untouched
- Commits: 3

## Accomplishments

- `tests/drawer_classic_parity.rs` renders a deterministic two-pane fixture (explicit OSC 2 titles, no `poll_all_ptys()`, temp XDG dir) through `WorkspaceWidget` and snapshots symbols plus a style digest.
- Matrix: legacy left/right/top/bottom (edges set via the real `:drawer` colon path), `:modern on`, `:liquid on`, maximized (`DrawerSize(u16::MAX)`), `:drawer float` overlay, session tree (`[SAVED]` and `[↳ default]` badges), fleet profile row, and a GUI surface frame.
- `scripts/check_classic_goldens.sh` fails unless at least 11 goldens each have exactly one commit, `tests/snapshots/` is clean, and no `.snap.new` exists. Prints `GOLDENS OK 11`.

## Task Commits

1. Task 1 (tracer: legacy_left golden): `d8b761d`
2. Task 2 (full matrix and gate script): `1f391cc`
3. Clippy fix (needless `&mut` in GUI test): `f45dee2`

## Deviations from Plan

**1. [Rule 1 - Bug] Clippy needless mutable borrow**
- **Found during:** verification (`scripts/verify.sh` clippy stage)
- **Issue:** `render_workspace_with_cell_size` takes `&WindowManager`; the test passed `&mut wm`.
- **Fix:** pass `&wm`. Snapshot unchanged.
- **Commit:** `f45dee2`

**2. [Plan fallback] Quick terminal row not pinned**
- **Found during:** Task 2, `quick_terminal_fleet_left`
- **Issue:** a visible quake overlay covers the top 40% of the screen and fully occludes the left drawer, so the QuickTerminal drawer row cannot appear in a full-frame golden.
- **Fix:** the test toggles quake on and off through `Action::QuakeToggle` (asserting visibility), and pins the fleet group (`register_profile` is public, so the "no public insert path" fallback was not needed). The QuickTerminal drawer row is therefore NOT covered by a classic golden.
- **Commit:** `1f391cc`

**3. Branch and process note:** execution was first blocked by the protected-branch guard on `main`; the user then directed work onto branch `gsd/phase-50-herdr-sidebar`. All commits are on that branch.

**Total deviations:** 1 auto-fixed (Rule 1), 1 plan fallback, 1 process note. **Impact:** none on the parity oracle except the uncovered QuickTerminal row.

## Known Stubs

None.

## Threat Flags

None. T-50-01/02 mitigated via the gate script, temp XDG isolation, OSC 2 titles and `assert_no_host_paths`; T-50-SC: no dependency changes.

## Verification

- `cargo test -q --test drawer_classic_parity`: 11 passed, repeated runs deterministic
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `scripts/verify.sh`: `PASS 52/52`
- `git diff --stat 091299a..HEAD -- src Cargo.toml Cargo.lock`: empty

## Next Phase Readiness

Plans 50-02..50-13 can use `cargo test -q --test drawer_classic_parity` and `scripts/check_classic_goldens.sh` as their classic-parity gate. Note: the gate script requires goldens not to be amended in later commits.

## Self-Check: PASSED

All 13 created files exist; commits d8b761d, 1f391cc, f45dee2 are ancestors of HEAD.
