---
phase: 50-herdr-style-left-sidebar-agent-aware-workspace-navigator
plan: 02
subsystem: ui
tags: [drawer, herdr, ratatui, session-tree, sanitize]
status: complete

requires:
  - phase: 50-01
    provides: classic drawer goldens and immutability gate
provides:
  - "DrawerStyle (classic|herdr) config: drawer.style, drawer.herdr_width_cols (default 28)"
  - "WindowManager::drawer_rows style dispatcher and herdr_drawer_rows (workspace > session > pane tree)"
  - "DrawerRow.herdr: Option<HerdrRowMeta> (depth, level, collapsed, has_children, live, pane_count)"
  - "render_herdr_drawer, herdr_layout_metrics, HerdrGlyphs (unicode/ascii)"
  - "crate::vt::sanitize_display(raw, max_chars)"
affects: [50-03, 50-04, 50-05, 50-06, 50-07, 50-10, 50-11, 50-12]

actuals:
  tokens: 9100
  tasks: 2
  commits: 3
plan_head_before: 8e3529b09e6643db20c92849fe3daaf9ef4fcb9c
plan_head_after: f4d9eb025f29005a9dd043d33d33e58bbfe8c8db

tech-stack:
  added: []
  patterns:
    - "Style dispatch seam: classic body renamed classic_drawer_rows unchanged; dispatcher + herdr logic in child module src/wm/drawer/herdr.rs"
    - "Renderer early-return branch to ui::drawer::herdr before any classic statement"
    - "DrawerConfig::effective_width_cols / set_width_cols_for_style keep per-style widths isolated"

key-files:
  created:
    - src/wm/drawer/herdr.rs
    - src/ui/drawer/herdr.rs
    - tests/phase50_herdr_drawer.rs
  modified:
    - src/wm/drawer.rs
    - src/ui/drawer.rs
    - src/vt/codec.rs

key-decisions:
  - "Roots = tree nodes with no (existing) parent that are explicit (same rule as classic: has parent/children or name != main), active, or saved; an active session absent from the tree is listed first as an orphan root"
  - "sanitize_display result is at most max_chars chars in total, the ellipsis counting as one of them"
  - "Rollup (Np) is shown on workspace/session rows when live or pane_count > 0; saved sessions without a tree node get pane_count 0 and no rollup"

requirements-completed: [FLT-01]

coverage:
  - id: D1
    description: "drawer.style = herdr renders a one-line-per-item workspace > session > pane tree with fold, saved rollup, current-pane marker"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_herdr_drawer.rs (13 tests: 2 style_, 11 tree_) all pass"
        status: pass
    human_judgment: false
  - id: D2
    description: "Titles sanitised (C0/C1/bidi/zero-width stripped, capped) before layout"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tree_sanitizes_titles + 6 sanitize_display_ unit tests in src/vt/codec.rs"
        status: pass
    human_judgment: false
  - id: D3
    description: "Classic drawer output byte-identical"
    requirement: FLT-01
    verification:
      - kind: other
        ref: "scripts/check_classic_goldens.sh -> GOLDENS OK 11; cargo test --test drawer_classic_parity 11 passed"
        status: pass
    human_judgment: false
---

# Phase 50 Plan 02: Herdr Drawer Tree Tracer Summary

**`drawer.style = herdr` (or `:set drawer.style herdr`) switches the slide-out drawer to a compact, 28-column, one-line-per-item workspace > session > pane tree with sanitised titles, fold state, saved-session pane rollups and an ASCII glyph fallback, while all 11 classic goldens stay byte-identical.**

## Performance

- Duration: about 5 min of agent wall time (tracer, RED, GREEN)
- Tasks: 2 (1 tracer, 1 tdd auto); files: 3 created, 3 modified
- Commits: 3 (`41c94fc` tracer, `4a41ce2` RED tests, `f4d9eb0` GREEN)

## Accomplishments

- Tracer: config (`DrawerStyle`, `herdr_width_cols`), row dispatcher, renderer early-return, and 3 end-to-end tests; classic parity re-verified before expanding.
- Full tree: depth-first walk over the session tree (visited set, depth cap 8), live panes under the active session only, collapsed nodes hide descendants and panes, quick terminal and fleet rows appended as `Other`.
- Renderer: marker column, indent, fold glyph, width-aware truncation with ellipsis, right-aligned `Np` rollup, optional footer hint; single line per row even under liquid glass; Top/Bottom edges render the same vertical list inside the horizontal rect.
- Width: herdr resizes (`:drawer size N`) write `herdr_width_cols` only; `width_cols` untouched.
- `sanitize_display` in `src/vt/codec.rs` (re-exported through `pub use codec::*`).

## Deviations from Plan

None - plan executed exactly as written. Notes:

- The task 1 commit was made right after a test run of `drawer_behaviour_coverage` that showed 6 failures. They are pre-existing environment dependence, not regressions: those older tests do not isolate `XDG_STATE_HOME` and count rows from the developer's real `~/.local/state/bilterm/sessions`. With an isolated XDG dir (and under `scripts/verify.sh`, which isolates it) they pass (42 + 10 passed). The task 1 commit itself was verified green via `scripts/verify.sh` afterwards.
- Test fixtures for nesting use the tree `main > build > default` (active session `default`) because changing `history_session` makes `default` appear as an additional saved root via `list_sessions()`.

## Known Stubs

None. Known limitation by design: mouse hit-testing (`src/input/mouse.rs`) still uses the classic row metrics, so clicks in a herdr-style drawer are not yet mapped; keys and mouse for herdr are scheduled for 50-06.

## Threat Flags

None. T-50-03 (sanitise before cell write), T-50-04 (64-char title cap, depth cap 8, visited set) and T-50-05 (width clamps unchanged in `drawer_rect`/`resize_drawer`) mitigated; T-50-SC: no dependency changes.

## Verification

- `scripts/verify.sh` (full): `PASS 53/53`
- `scripts/verify.sh --only clippy unit t:drawer_classic_parity t:phase50_herdr_drawer t:slide_out_drawer_and_telemetry t:drawer_behaviour_coverage t:session_lessons_and_regression_suite`: `PASS 53/53`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `src/wm/drawer.rs`: 1029 lines (cap 1030); `git diff Cargo.toml Cargo.lock`: clean
- Acceptance greps: `classic_drawer_rows` 1, `pub fn drawer_rows` 0 in drawer.rs / 1 in herdr.rs, `herdr: None` 6, `render_herdr_drawer` 1 in ui/drawer.rs

## Next Phase Readiness

Ready for 50-03 (parallel) and 50-04: `HerdrRowMeta` is ready to take state fields; `herdr_layout_metrics` and `HerdrGlyphs` are the hooks for state dots, key/mouse handling and comms glyphs.

## Self-Check: PASSED

Created files exist (`src/wm/drawer/herdr.rs`, `src/ui/drawer/herdr.rs`, `tests/phase50_herdr_drawer.rs`); commits 41c94fc, 4a41ce2, f4d9eb0 are ancestors of HEAD.
