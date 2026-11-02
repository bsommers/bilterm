# Phase 46 Design: Shell, Display Modes, Navigator, Pane Chrome

Status: DRAFT for review. No code for this section has been written.
Governing rules: Phase 44 DECISIONS.md, ROADMAP Phase 46 "Harmony rule", user rule
"we are taking NOTHING away from the old looks. just adding new looks and capabilities."

## 0. Already done in Phase 46 (commits on main)
- Non-Legacy profiles default to the bundled JetBrains Mono (c5a95f9).
- `:appearance legacy|obsidian|midnight|arctic|liquid` (2456157). Session only, not saved.
- A missing config file starts on Obsidian; `BiltermConfig::default()` stays Legacy (1fe1227).
These are not the ROADMAP's 46-01/02/03 plans. The ROADMAP plan names stay unused; the work below maps onto them in section 7.

## 1. Principle: Legacy renders exactly as today
Every new behavior is gated on `AppearanceProfile::resolve(cfg) != Legacy`. In Legacy, `DisplayMode`
is ignored (treated as Standard) and the existing flags (`show_top_menubar`, `show_dos_fkey_bar`,
`is_framed()` frames, Borland titles, drawer) behave exactly as now. Test: a Legacy snapshot with
`display_mode = zen` is byte-identical to one with `standard`.

## 2. Display modes (`:mode standard|minimal|zen`)
`DisplayMode` already exists in `src/ui/design/config.rs` as `Standard | Focus | Zen`. The ROADMAP says
`minimal`; the code says `Focus`. Proposal: keep the enum variant `Focus` and accept `minimal` as an alias
in `:mode` and in serde (`#[serde(alias = "minimal")]`). No existing config breaks.

| Element | Standard | Minimal (Focus) | Zen |
|---|---|---|---|
| Top bar (workspace pills) | shown | shown only while a pane is focused and a split/stack exists | hidden |
| Pane frame and title | frame; title on the focused pane only | frame hairline, no title text | none |
| Status bar (OIA) | shown | shown | hidden, except transient notices (section 4) |
| Input row | normal | normal | one-line input row, always visible |
| Drawer, palette, modals | available | available | available (opened on demand, drawn over the terminal) |

"All actions still reachable" (ROADMAP criterion 1) holds because every action lowers to `Action`:
keybindings, `:` commands, and the command palette all keep working with no chrome on screen.

## 3. Where the code changes
`WorkspaceWidget::render` in `src/ui/mod.rs` (lines ~575-700) already computes `top_offset`, `bottom_offset`,
and `framed`, then draws frames and bars. Plan:
1. Add a pure function `ShellLayout::compute(area, cfg) -> ShellLayout` in a new `src/ui/shell.rs`
   returning `{ top_bar: Option<Rect>, workspace: Rect, status: Option<Rect>, input: Option<Rect>, mode }`.
   It reads `AppearanceProfile::resolve` and `display_mode`. For Legacy it reproduces today's
   `top_offset`/`bottom_offset` math exactly (unit-tested against the current formula).
2. `WorkspaceWidget::render` takes its rects from `ShellLayout` instead of the inline math. Legacy output unchanged (snapshot tests prove it).
3. Frame/title policy: a `PaneChromeSpec { frame: FrameKind, title: TitleKind, radius, border, padding }`
   chosen from mode + `[appearance]` overrides; the existing Borland path stays the Legacy branch.
4. The top bar reuses the `Navigator` model from section 5.
Size budget: `shell.rs` ~300 lines. `ui/mod.rs` is at 781 lines, so only the rect math moves out; net shrink.

## 4. Zen specifics (Phase 44 decision: keeps a one-line input)
- Layout: terminal content fills everything except one bottom row for the input line.
- Transient notices (status messages, bell, errors): shown in that same row for ~3 s, then revert to the input. No separate status bar, so Zen never needs more than one row.
- Entry and exit: `:mode zen`, `:mode standard`, plus a toggle key. Proposed `Leader z` (to confirm there is no clash in `keymap.rs`).
- A one-time hint on entering Zen shows how to leave (`:mode standard` or the toggle key), so a user can never be stranded.
- Edge case: terminals under 3 rows tall fall back to Standard behavior with no input row (nothing hidden that would make the UI unusable).

## 5. Navigator (top bar) and the existing session system
Requirement 3: "Existing workspace/session system extended, not duplicated."
- Model: `Navigator { items: Vec<NavItem{ id, label, kind: Window|Pane|Session, state: Active|Busy|Attention|Idle }> }`,
  built read-only from the existing `WindowManager` pane list, z-stack ranks and the saved-session list that the drawer already uses. No new state store.
- Rendering: pills in the top bar for Obsidian/Midnight/Arctic/Liquid. Clicking a pill and `Leader 1..9` call the existing `Action::FocusPane`. No new focus path.
- The drawer (Sessions/Windows/Panes) stays as the detailed view. The top bar is the compact summary of the same data.
- Overflow: pills collapse to `n+` when the width is short.

## 6. Pane chrome options (`[appearance]`)
Existing fields already carry most of it: `border` (None/Hairline/Bold), `density`, `corner_radius_px` (GUI only), `opacity_pct`, `glow_pct`.
Additions (all optional, serde default, ignored in Legacy):
- `title = focused | all | none` (default `focused`, per the Phase 44 decision)
- `focus_ring = true|false`: accent color on the focused pane border, from `ColorRole::Accent`
- `padding = 0|1` cells inside the frame (derived from `density` by default; override only if wanted)
Key delivery is unchanged: chrome is drawn from `pane_chrome(id)` and never intercepts input.
Contextual pane actions (46-03 in the ROADMAP): a hover or focus-visible strip (split, zoom, close, mirror, lock) rendered on the pane title row, each button lowering to the same `Action` as its key. Terminal-mode only where mouse events exist; always keyboard-reachable through the palette.

## 7. Work breakdown (serial, one commit each, verify.sh after each)
| Step | Content | Model | ROADMAP plan |
|---|---|---|---|
| 46-04 | `DisplayMode` alias `minimal`, `:mode` command (Action + parse + complete + help), session-only like `:appearance` | sonnet | 46-01 |
| 46-05 | `ShellLayout` extracted with Legacy-parity tests; render uses it | sonnet | 46-01 |
| 46-06 | Standard/Minimal/Zen layouts + Zen input row and notices | sonnet | 46-01 |
| 46-07 | `Navigator` + top-bar pills (non-Legacy only) | sonnet | 46-02 |
| 46-08 | Pane chrome options (`title`, `focus_ring`, `padding`) | sonnet | 46-02 |
| 46-09 | Contextual pane action strip + integration tests | sonnet | 46-03 |
Checks per step: `scripts/verify.sh`; Legacy snapshots unchanged; new snapshot tests for each mode in Obsidian.

## 8. Questions for review
1. `minimal`: alias for `Focus` (my pick, no breaking change), or rename the enum to `Minimal`?
2. Zen toggle key: `Leader z` OK, or do you want a different binding?
3. Should `:mode` and `:appearance` persist to `config.toml`? The code has no config write path today, so it is a new `secure_fs` atomic write if you want it. My pick: yes, in a later step, opt-in via `:appearance save`.
4. Top bar in Minimal: hide whenever only one pane exists (my pick), or always show?
5. Zen with multiple panes: show only the focused pane (my pick, others stay alive in the background), or keep the tiling visible without chrome?
