---
phase: 51-ui-tweaks-and-command-parity
plan: 04
subsystem: command_registry / ui help_catalog / pane completion / api
tags: [palette, parity, exclusions, UIT-02, UIT-01, UIT-03, UIT-04, UNI-R01]
requires: ["51-01", "51-02"]
provides:
  - "PALETTE_EXCLUSIONS + closed reason vocabulary + CommandInfo.palette_exclusions"
  - "probe sweep and anti-rot tests in src/command_registry.rs"
  - "223 new PALETTE_CATALOG entries, new HELP_LINES rows, completion rows for drawer/arrange/agents"
  - "bilterm://commands carries palette_exclusions per command"
affects: [51-11, 51-13]
key-files:
  created:
    - tests/phase51_palette_parity.rs
  modified:
    - src/command_registry.rs
    - src/api/mcp_resources.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
decisions:
  - "equivalent spellings with no Action equality (:profile single|dual-v|dual-h|main-side|quad|cascade, :layout main-side, :border, :raise, :session-rename) are short-alias exclusions rather than duplicate entries"
  - "sweep treats a needs-argument probe as covered by Action equality OR by a palette entry that starts with the probe"
status: complete
base_sha: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 55eee7979c6f298ca10debd0421bc465ca197331
commits: 4
actuals:
  tokens: 60000
  tasks: 3
  commits: 4
---

# Phase 51 Plan 04: Palette parity Summary

Every colon verb is now a palette entry that parses to the same `Action`, or a `PALETTE_EXCLUSIONS` row with a reason from a closed vocabulary; a sweep test over every Tab-completion probe fails when a new verb has neither, and the exclusions are visible over MCP.

Commits: `6804dc9` (Task 1 sweep, exclusions, JSON), `ec89e63` (Task 2 completion rows), `0caca83` (Task 3 entries, help, tests), `55eee79` (clippy fix).

## Authoritative gap list (first sweep run)

First run with the exclusion table filled for existing verbs: 205 probes uncovered. After the Task 2 completion rows (drawer values, arrange, agents list/demo stop): 242. After Task 3: 0. The list covered the 11 verbs of research A (`focus`, `arrange`, `yank`, `clear`, `raw`, `glyphs`, `shadows`, `snazzy`, `glass`, `style`, `rename-session`) plus the sub-verb gaps. Without any exclusions the raw probe count was 331 uncovered.

## Verbs added to completion rows (for plan 51-11 doc gate)

- `:agents`: `demo stop`, `list open`, `list close`, `list toggle`, `list dismiss`, `list blur`, `list select `, `list jump`, `list zoom`, `list pin`, `list unpin`, `list scale `, `list filter `, `list dock`, `list float`, `list move `, `list resize `, `list rect `, `list size `, `list scroll `.
- `:drawer`: `style ` replaced by `style navigator`, `style classic`; `rows ` by `rows 1`, `rows 2`; added `minimize`, `maximize`, `pin`, `unpin`, `scale `, `mode `, `edge `. `:navigator` row: `style navigator`, `style classic`.
- New rows: `:arrange` (grid, columns, rows, main-side, main-top, stack-grid, float-grid) and `:auto-arrange` (EXTRA alias row).

## Gates

- `cargo test -q --lib command_registry`: 9 passed. `cargo test -q --lib help_catalog`: 5 passed. `cargo test -q --test phase51_palette_parity`: 6 passed. `cargo test -q --lib action::tests::every_completion_entry_is_a_command_the_parser_recognises`: 1 passed.
- `cargo test -q --test api_conformance --test v110_v120_v130_automation_api_mcp_playbooks`: 8 + 4 passed (no skip rows needed).
- `scripts/verify.sh` (full): `PASS 80/80 55s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`; `cargo test -q --test drawer_classic_parity`: 11 passed.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean. `src/input/modal.rs` untouched.
- Acceptance greps: `PALETTE_EXCLUSIONS` in command_registry.rs 9; `palette_exclusions` in mcp_resources.rs 1; Find-modal reason string 1; `":drawer style navigator"` in help_catalog.rs 1; `":agents demo stop"` 2; `help_catalog.rs` 903 lines.

## Deviations from Plan

1. **[Doc] `:window theme ` count is 7, not 6.** `:window theme prev` (a cycle verb, not one of the 13 presets) got a palette entry because it is a real probe with no other cover; no preset entry was added.
2. **[Doc] `cargo test -q --lib pane::completion` runs 0 tests** (src/pane/completion.rs has no test module); the completion drift guard `action::tests::every_completion_entry_is_a_command_the_parser_recognises` and `command_registry` tests were run instead. No test pinning the old completion counts needed editing.
3. **[Doc] `:finder` bare head also excluded** (with the Find-modal reason) in addition to the seven sub-verbs; `:find agent|session|history` resolve to the 50.1-09 entries.
4. **[Doc] Demo stop spot test is weaker than planned.** The pane-count restore and `agents_demo_summary().running` belong to plan 51-03 (`demo_stop` is still the seam stub in this base); the test asserts the verbs run and stop never adds panes. 51-03's own tests own the restore.
5. **[Rule 1] clippy `collapsible_if`** in the sweep helper fixed (commit 55eee79).
6. help_catalog.rs ends at 903 lines (plan soft target 900; under the 1000 limit, no split).
7. Stale exclusions removed by the anti-rot test during Task 3 (`:session cmd`, `:rename-session`, `:renamesession`; Action equality already covers them).

## Known Stubs

None added by this plan. `:agents demo stop` entries run the 51-03 seam stub until 51-03 merges.

## Threat Flags

None. T-51-10 mitigated (`:mcp derez` and `:mcp shutdown --confirm` are `destructive` exclusions; the spot test skips host-side-effect verbs); T-51-11 mitigated (sweep plus `every_palette_entry_with_complete_args_parses_ok`); T-51-SC: no dependency change.

## Follow-ups

- Enter-prefill for fill-in palette entries (`src/input/modal.rs`) needs separate sign-off (D-02); `palette_enter_fill_in_entries_unchanged` pins today's behavior.
- `:arrange <pattern>` has no key path (contract exception 4).

## Self-Check: PASSED

`tests/phase51_palette_parity.rs` exists; commits `6804dc9`, `ec89e63`, `0caca83`, `55eee79` are ancestors of HEAD.
