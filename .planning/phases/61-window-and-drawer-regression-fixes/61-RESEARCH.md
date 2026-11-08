# Phase 61: Window & Drawer Regression Fixes - Research

**Researched:** 2026-11-08
**Domain:** Rust TUI/GUI window manager (ratatui/crossterm): drawer row layout, zoom state, per-pane UI state across focus changes
**Confidence:** HIGH for FIX-02 and the main FIX-03 causes (both reproduced headless); MEDIUM for FIX-01 (mechanism proven, user's runtime style not observable)

<user_constraints>
## User Constraints (from CONTEXT.md)

No `61-CONTEXT.md` exists (phase directory was empty; `/gsd-discuss-phase` was not run). Constraints come from the bug report, `.planning/REQUIREMENTS.md` (FIX-01..03, lines 258-262) and project rules (`CLAUDE.md`, `AGENTS.md`).

### Locked Decisions
- None recorded. Treat the three requirement texts in REQUIREMENTS.md as locked scope.

### Claude's Discretion
- Root-cause selection, fix design, test design, plan split.

### Deferred Ideas (OUT OF SCOPE)
- None recorded.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FIX-01 | Classic (legacy) drawer: `:drawer rows 2` renders two-line rows; row-lines setting discoverable/toggleable from the drawer | Classic path ignores `row_lines` by design; fix = per-style row-lines + in-drawer chip/key/menu (see FIX-01) |
| FIX-02 | Menu Window -> Zoom toggles correctly; input never falls through to panes beneath | `zoomed_pane` is a fixed id that does not follow focus; `toggle_zoom` re-zooms instead of un-zooming (reproduced) |
| FIX-03 | Switching/toggling stacked windows preserves prompt type, input mode and UI state | Focusing click also executes the OIA/prompt chrome action (reproduced); copy mode cleared on focus change (reproduced); prompt mode not persisted in snapshots |
</phase_requirements>

## Summary

All three bugs are real defects in the current tree, but only two were reproduced end-to-end. Method: scratch crates under the session scratchpad depend on `bilterm` by path (no repo edits) and drive `WindowManager::new_headless` through the public key/mouse entry points. Output quoted below is from those runs. [VERIFIED: headless runs this session]

FIX-01: no code regression exists. `git log -S"row_lines"` returns only commits 1412846 (introduction, 2026-11-04), 3d8e8d8 (help-table move) and 31cd42c (Phase 56, adds `session_name` and background-session rows). None touch the Classic render/metrics path. Two-line rows have only ever existed for the Navigator style (any look) and, accidentally, for the Liquid Glass look of the Classic style through an all-or-nothing auto rule. `tests/navigator_two_line_rows.rs` (9 tests) passes on HEAD, and the user's real `~/.config/bilterm/config.toml` parses to `style=Navigator row_lines=2`. So "everything on one line" is the Classic drawer style (long single-line telemetry rows), which silently ignores `:drawer rows`. REQUIREMENTS.md FIX-01 itself says "Classic (legacy) drawer mode".

FIX-02: `zoomed_pane` stores a fixed pane id while input always goes to `z_stack.top()`. Any focus change while zoomed (deck shuffle Alt+Tab/F6, drawer select, `focus_and_raise`) leaves the zoomed pane on screen but sends keys to a hidden pane; and `toggle_zoom()` then re-zooms the new top instead of un-zooming, so Menu -> Window -> Zoom "does nothing" visibly. Reproduced.

FIX-03: three independent state-loss defects: (a) the click that focuses an inactive pane is not consumed and also runs the OIA/prompt-chrome action on it (cycles prompt mode Floating -> Fixed; reproduced); (b) `clear_inactive_transient_copy_cursors` drops copy mode of any pane losing focus (reproduced); (c) `SavedPaneState` has no prompt-mode/raw-override field, so snapshot/restore paths reset Regular Terminal mode (code-read). Deck/focus switching itself preserves prompt mode, text and scroll offset in memory (verified), so a separate defect is not hiding there.

**Primary recommendation:** FIX-01 add a style-owned `classic_row_lines` (0=auto, 1, 2) honoured by `drawer_vertical_layout_metrics`, plus an in-drawer chip/key/menu; FIX-02 derive effective zoom from `z_stack.top()` and make `toggle_zoom` a true toggle; FIX-03 swallow the focusing click on OIA/prompt chrome, stop clearing inactive copy mode, persist raw-override in `SavedPaneState`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Drawer row height / step | `ui::drawer` (layout metrics) | `input::mouse_wm` (hit-test) | `drawer_vertical_layout_metrics` is the single source for render and mouse (src/input/mouse_wm.rs:873,912,928) |
| Row-lines config + command | `wm::drawer` config, `wm::drawer::nav` setter | `colon_command` parse/execute | `Action::DrawerRows` -> `set_drawer_row_lines` |
| Zoom state | `WindowManager` (wm/mod.rs) | `compute_workspace_rects` consumers (render, mouse, resize) | One state field, many readers; must be derived from focus |
| Click-to-focus + chrome clicks | `input::mouse::handle_wm_mouse_event` | `pane` OIA/prompt handlers | Ordering of focus vs action is decided in the WM mouse router |
| Copy-mode / prompt-mode state | `CompositePane` (per pane) | `WindowManager` focus helpers must not reset it | State belongs to the pane; WM only changes `z_stack` |
| Persistence of pane mode | `wm::session` `SavedPaneState` | `layout_profile`, `window_manage_bulk` | Three producers/consumers of the same struct |

## Project Constraints (from CLAUDE.md / AGENTS.md)

- Verification: run `scripts/verify.sh` (or `--quick`), read the single output line; on FAIL read only `target/verify/<stage>/fail.txt` then `log`. Always `-q` on cargo. [CITED: CLAUDE.md]
- ARCH-R04: `src/**/*.rs` ~800-1000 lines soft, 1200 hard test backstop. Current sizes: `src/input/mouse.rs` 972, `src/wm/mod.rs` 1048, `src/ui/drawer.rs` 510, `src/wm/drawer.rs` 655, `src/wm/z_stack.rs` 747 (`wc -l` this session). Net additions to mouse.rs/wm/mod.rs must stay small (< ~60 lines) or split into a new file.
- ARCH-R01: every user-triggerable operation lowers to `Action` and runs via `WindowManager::apply`; `handle_wm_key_event` stays <= 80 lines. Every new `Action` variant must be added to `src/action.rs::tests::every_action_is_reachable_from_keymap_colon_or_rpc`. This phase should add NO new `Action` variants (reuse `DrawerRows(u8)`, `ZoomToggle`).
- ARCH-R02: `WindowManager`/`CompositePane` internals stay private; mutate via methods.
- UNI-R01: each capability needs mouse, key, colon/palette/help/RPC paths all reaching the same `Action`/state, with a parity test or an entry in `uniformity_exceptions`.
- SEC-R01..04: any new persisted field goes through the existing `secure_fs` writers (no new file writes planned).
- Subagent rule (user global): planning/research agents use `sonnet`/`haiku`; no opus.

## Standard Stack

No new dependencies. Everything uses the existing stack: `ratatui 0.30.2`, `crossterm 0.29.0`, `serde`+`toml`, plus `insta` for snapshot tests if wanted. [CITED: AGENTS.md stack section / CLAUDE.md]

### Package Legitimacy Audit
No external packages are installed by this phase. Scratch research crates used only path deps and already-resolved registry crates (`ratatui`, `crossterm`) with `--offline`. Packages removed (SLOP): none. Packages flagged (SUS): none.

## Architecture Patterns

### System Architecture Diagram

```
 key / mouse / menu / :colon / RPC
        |
        v
 input::handle_wm_key_event --+          input::mouse::handle_wm_mouse_event
        |                     |                    |
        |  Action             |                    | 1. hit_test_pane (uses compute_workspace_rects)
        v                     |                    | 2. focus_and_raise(hit)   <-- FIX-03a: click still continues below
 WindowManager::apply(Action) |                    | 3. OIA/prompt chrome handler -> cycle_prompt_mode / set_prompt_anchor
   ZoomToggle -> toggle_zoom  |                    | 4. forward to pane
   DrawerRows -> set_drawer_row_lines              v
        |                                    CompositePane (prompt/copy/scroll state, per pane)
        v
 compute_workspace_rects(area)  <-- FIX-02: zoomed_pane (stale id) vs z_stack.top() (input target)
        |
        v
 ui::WorkspaceWidget / render_workspace_drawer
        Classic: drawer_vertical_layout_metrics(cfg,h,n) -> (hdr,ftr,step)   <-- FIX-01: step ignores drawer.row_lines
        Navigator: navigator_layout_metrics + row_height(detail)
```

### Recommended file layout for new code
```
tests/phase61_drawer_rows.rs              # FIX-01
tests/phase61_zoom_follows_focus.rs       # FIX-02
tests/phase61_window_state_preserved.rs   # FIX-03
```

### Anti-Patterns to Avoid
- Second source of row geometry: render and mouse must keep calling `drawer_vertical_layout_metrics` (signature is used by 3 test files; do not change it).
- Fixing zoom by hooking every `raise` call site: derive from `z_stack.top()` instead (one place).
- Making Classic honour the existing `row_lines` (default 2): that silently changes every Classic user's drawer and breaks Classic parity goldens. Use a separate style-owned field (precedent: `set_width_cols_for_style`, src/wm/drawer/navigator.rs "never crosses styles").

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Two-line Classic card text | New formatter | `format_liquid_drawer_card` (src/ui/drawer.rs:84) | Already renders title line + telemetry line per row kind |
| Row hit-testing | New geometry | `drawer_vertical_layout_metrics` (src/ui/drawer.rs:23) | Shared with mouse_wm; step flows through automatically |
| Zoom follow-focus hooks | Per-call-site sync | Derived `effective_zoomed()` | Covers shuffle, focus_and_raise, `:focus`, drawer, mouse in one place |
| Pane mode persistence | New file/format | Extend `SavedPaneState` with `#[serde(default)]` field | Existing evolvable struct (src/wm/session.rs:22) |

## Runtime State Inventory

Not a rename/refactor phase. One persisted-data note: adding `classic_row_lines` to `[drawer]` and a field to `SavedPaneState` are additive with `#[serde(default)]`; old config and session TOMLs keep loading. Stored data: none to migrate. Live service config: none. OS-registered state: none. Secrets/env: none. Build artifacts: none.

## FIX-01 Root cause: Classic drawer ignores `row_lines`

### Evidence
- `row_lines` doc: `/// Navigator style only: \`2\` (default) draws pane rows over two lines` [VERIFIED: src/wm/drawer.rs:111-115].
- Only readers of the setting: `navigator_row_lines()` (src/wm/drawer/navigator.rs:145) used in `push_panes`/`push_bg_panes` (navigator.rs:324,367) and `navigator_layout_metrics` (src/ui/drawer/navigator.rs:32-34). The Classic path `drawer_vertical_layout_metrics` never reads `cfg.drawer`: [VERIFIED: src/ui/drawer.rs:23-43, quoted] `let step = if cfg.liquid_glass && inner_height >= 14 && item_count.saturating_mul(2) <= avail { 2 } else { 1 };` and the non-modern branch returns `(1, footer_h, 1)`.
- Documented behaviour: "Classic drawer style ignores `row_lines`." [VERIFIED: docs/user-guide/11-navigator-sidebar-and-agent-traffic.md:63]. Commit 1412846 message: "Classic style is untouched."
- Setter writes the field regardless of style but its status message is style-blind: `Drawer rows: {lines} (navigator style)` [VERIFIED: src/wm/drawer/nav.rs:411-416]. In Classic the command "succeeds" and changes nothing visible.
- The `:drawer` usage error omits `rows` [VERIFIED: src/colon_command/parse_visual.rs, usage string ends `...|style|minimize|maximize|size <n>]`]; Classic title/header chips are `[D:..] [M:..] [P:..] [+Win] [+Sess]` (src/ui/drawer.rs:~165-205): no rows indicator and no way to switch style from the drawer.
- Navigator works on HEAD: `cargo test -q --test navigator_two_line_rows` -> 9 passed. User config: scratch `BiltermConfig::load` printed `OK style=Navigator row_lines=2`. [VERIFIED: runs this session]
- Phase 56 suspect cleared: diff of 31cd42c in `src/wm/drawer.rs`/`navigator.rs`/`ui/drawer*` adds only the `preview` module, `session_name`, and background-session rows; no change to metrics, `row_height`, or the renderer.
- Environment facts: installed `~/.local/bin/bilterm` is 3.0.1 (built 2026-11-06 21:20), `~/.cargo/bin/bilterm` is 2.8.0 (2026-11-05 06:56, predates two-line rows), and a long-running `bilterm --gui` (pid 3465828, started 2026-11-06 20:10) is alive. A stale binary/process is a possible contributor but cannot be proven from here.

### Ranked hypotheses
1. **(most likely) Runtime drawer style is Classic.** Classic rows are one long telemetry line (`formatted_line`), matching "everything is on one line"; `:drawer rows 2` is a no-op there. Confidence MEDIUM (mechanism HIGH; user's runtime style unobservable).
2. **Liquid Glass look cliff.** The only Classic 2-line path is `cfg.liquid_glass && inner_height >= 14 && item_count*2 <= avail` (src/ui/drawer.rs:32). It is all-or-nothing: one extra row flips the whole drawer to one line. Phase 53/56 added background-session rows to the Classic list (src/wm/drawer.rs:621-626, `extra_sessions.push(bg_name.clone())`), which can tip it. Would explain "worked 2-3 days ago" without the user ever having been in Navigator. Confidence LOW-MEDIUM.
3. **Stale/other binary or old running GUI process.** LOW.

One design fixes 1 and 2: an explicit, style-owned Classic row-lines value with no "must all fit" rule (scrolling already handles overflow: `list_items_cap = (list_rows_px / row_step).max(1)`).

### Decision: how row_lines applies
- Navigator keeps `row_lines` (default 2) exactly as is.
- Classic gets `classic_row_lines: u8` (0 = auto/current behaviour, 1 = force one line, 2 = force two lines), default 0, so Classic and Liquid goldens are untouched until the user opts in.
- `:drawer rows 1|2` writes the field owned by the ACTIVE style (same "never crosses styles" rule as width), and the status message names the style: `Drawer rows: 2 (classic style)` / `(navigator style)`.
- Explicit 2 in Classic renders via `format_liquid_drawer_card` for any look, vertical edges only, when `inner_height` can hold at least one 2-line item plus header/footer; horizontal edges ignore it (documented).

### Discoverability (UNI-R01 paths; no new Action)
| Surface | Path |
|---------|------|
| Mouse | Classic drawer title/header chip `[R:2]` (click toggles 1<->2); menu View items |
| Keys | A free drawer-focused key in `src/input/modal.rs` drawer match (taken: q,_,-,*,M,J,K,N,b,B,>,<,u,U,P,s,S,r,R,:,j,k,h,l,g,G,v,|,%,x,z,d,m,p,a,L,n,+,c,w; digits 1/2 appear unused: executor must grep before choosing) |
| Commands | existing `:drawer rows [1\|2]`, palette and help lines (update text: no longer "Navigator drawer") |
| API | existing `Action::DrawerRows` over RPC/MCP/`bilterm ctl` (unchanged) |
| Menu | Add two View-menu entries (`:drawer rows 1`, `:drawer rows 2`) in `MENUBAR_CATEGORIES` (src/ui/modals.rs:~381) |

Tests that enumerate menu items (`tests/phase51_integration.rs:310`, `tests/agents_launch_affordances.rs:80`, `tests/phase50_1_parity.rs`) iterate generically via `menubar_category_item_count`, so adding items is safe, but dropdown height changes: re-run them.

## FIX-02 Root cause: zoom id is decoupled from focus

### Evidence
- Zoom stored as an id and focus is `z_stack.top()`: [VERIFIED: src/wm/mod.rs:925-935]
  ```
  pub fn toggle_zoom(&mut self) {
      let Some(top_id) = self.z_stack.top() else { return; };
      if self.zoomed_pane == Some(top_id) { self.zoomed_pane = None; } else { self.zoomed_pane = Some(top_id); }
  ```
  `active_pane_id()` = `self.z_stack.top().unwrap_or(1)` [VERIFIED: src/wm/mod.rs:716-718]. Rendering and hit-testing use `compute_workspace_rects`, which returns only `(zoomed_id, area)` when `zoomed_pane` is set [VERIFIED: src/wm/mod.rs:961-964].
- Nothing clears or follows zoom on focus change: `focus_and_raise` (wm/mod.rs:873), `shuffle_deck_next/prev` (~895-921) never touch `zoomed_pane`; the only clearers are remove/split/minimize/session swaps (grep of `zoomed_pane`).
- Reproduction (scratch crate, 2 panes, `Action::ZoomToggle` then `shuffle_deck_next`):
  ```
  start top=2 zoomed=None
  after zoom top=2 zoomed=Some(2) rects=[(2, 80x24)]
  after shuffle top=1 zoomed=Some(2) rects=[(2, 80x24)]     <- pane 2 drawn, keys go to pane 1
  after zoom2 top=1 zoomed=Some(1) rects=[(1, 80x24)]        <- "toggle" re-zooms, never un-zooms
  after zoom3 top=1 zoomed=None
  ```
  This is exactly "mouse and keys project down to the pane beneath" plus "Zoom toggle does not work". The same mismatch arises from drawer selection, `:focus`, leader digits, attention jumps, mouse focus on a utility window, anything calling `focus_and_raise` or shuffle.
- The menu path itself is sound: `execute_menubar_selection` closes the modal, parses `":zoom"` (from "Zoom Toggle (F5)", src/ui/modals.rs:392) and calls the same `Action::ZoomToggle` -> `toggle_zoom` (src/input/modal_palette.rs:10-22; src/colon_command/execute_wm.rs:78). Menu, F5, Alt+Z, leader z, titlebar badge (mouse_wm.rs:384) and the Navigator Zoom button (ops.rs:216) all share it, so one fix covers every surface.

### Fix approach
1. Add private `fn effective_zoomed(&self) -> Option<PaneId>`: `self.zoomed_pane.and(self.z_stack.top()).filter(|id| self.panes.contains_key(id))`. Semantics: zoom is a mode ("monocle") that follows focus. Use it in `compute_workspace_rects` and in the public `zoomed_pane()` getter (so the existing tests/getter stay truthful).
2. `toggle_zoom`: `if self.zoomed_pane.is_some() { None } else { Some(top) }` (a real toggle), then reflow + autosave as today.
3. Because raise/shuffle change the visible pane without `resize_all`, call `self.reflow_default_viewport()` from the shuffle/`focus_and_raise` paths only if sizes can differ (zoomed pane == full area, so the newly visible pane needs the full-area viewport). Simplest: in `focus_and_raise`, shuffle next/prev: `if self.zoomed_pane.is_some() { self.reflow_default_viewport(); }`.
4. Do not touch `session.rs`/`layout_profile.rs`: the stale stored id is harmless because readers derive from top.
Alternative considered and rejected: unzoom on focus change (tmux default). Follow-focus keeps "stacked carousel while maximized" usable and needs no UX relearning; flag as an Open Question.
Edge: `quick_terminal.visible` makes `focus()` = Quick while `toggle_zoom` still targets `z_stack.top()`; acceptable (zoom applies to the workspace deck), covered by a test asserting no panic.

## FIX-03 Root causes: state loss on focus/switch

User wording: "prompt type changes" when switching/toggling stacked windows. Verified in-memory behaviour (scratch crate, Tiled and Stacked): after Alt+Tab key, F6, `focus_and_raise`, `focus_direction`, `toggle_layout_mode`, quake and drawer toggles, `prompt_mode()`, prompt text and scrollback offset of each pane were unchanged. So the prompt-type change comes from the following, in rank order.

### F3-a (HIGH, reproduced): the focusing click is not consumed
[VERIFIED: src/input/mouse.rs:839-842] `if matches!(mouse.kind, MouseEventKind::Down(_)) && let Focus::Pane(hit_id) = hit_focus { wm.focus_and_raise(hit_id); }` then the same event continues to the pane handlers: OIA bar `handle_oia_status_bar_mouse` (mouse.rs:58-125; routing cell calls `pane.cycle_prompt_mode()` at :110; pin cell calls `set_prompt_anchor` at :120) and the prompt top-border controls (mouse.rs:~237-253, 331-335).
Reproduction (Tiled, 2 panes, top=2; click on the OIA routing cell of inactive pane 1 at (21,22), then of active pane 2):
```
initial                      top=2 | #1[Floating anchor=Top] #2[Floating anchor=Top]
click inactive #1 OIA routing top=1 | #1[Fixed anchor=SplitTop] #2[Floating anchor=Top]
click active #2 OIA routing  top=2 | #1[Fixed anchor=SplitTop] #2[Fixed anchor=SplitTop]
```
Switching windows by clicking anywhere on an inactive window's status bar (very easy in tiled/stacked layouts) both focuses it and cycles/changes its prompt mode or anchor.
Fix: before `focus_and_raise`, record `was_active`; when the pane was NOT active and the Down(Left) lands on the pane's OIA row or on prompt chrome controls (top border), raise/focus and return `AppAction::Handled` without running the chrome action. Text-area clicks keep today's click-through (selection start) behaviour. Title-bar badges (minimize/zoom/close in `handle_wm_frame_border_mouse_down`) stay unchanged: they are explicit window controls.

### F3-b (HIGH, reproduced): copy mode is dropped on focus change
[VERIFIED: src/wm/z_stack.rs:650-661] `clear_inactive_transient_copy_cursors` sets `copy_mode_mut().active = false` for every non-active pane that has no selection/rect mode; called from `focus_and_raise`, `shuffle_deck_next`, `shuffle_deck_prev` (src/wm/mod.rs:884,903,919). Reproduction: pane 2 in copy mode, Alt+Tab -> `#2 copy=false`.
The renderer already draws the copy cursor only for the active pane (`if active && pane.copy_mode().active`, src/ui/mod.rs:463), and keys go only to the active pane, so the clearing is not needed for correctness. Fix: delete the function and its 3 call sites.

### F3-c (MEDIUM, code-read): prompt mode not persisted
`SavedPaneState` carries `prompt_anchor`, height, width, x_offset, input, scroll_mode, read_only... but no `manual_raw_override`/mode [VERIFIED: src/wm/session.rs:22-71 read in full]; restore only assigns `pane.prompt_mut().anchor = sp.prompt_anchor` etc. (src/wm/layout_profile.rs:578-583). Producers: src/wm/session.rs:393 and src/wm/window_manage_bulk.rs:321; consumer: layout_profile.rs ~578. Any path that rebuilds panes from `SavedPaneState` (session restore, layout-profile switch, undo snapshots) resets Regular Terminal mode to Floating/Fixed. In-memory session switch (`switch_to_session_preserving_panes`) keeps live panes and is unaffected (verified by reading window_manage_bulk.rs:437-480). Fix: add `#[serde(default, skip_serializing_if = "is_false")] pub raw_passthrough: bool` set from `prompt().manual_raw_override`, applied on restore.

### F3-d (MEDIUM, reproduced): scroll offset reset on column reflow
`VtParser::set_size` rebuilds the parser from `primary_stream_log` when columns change (src/vt/mod.rs:310-320), which resets `scrollback()` to 0. Reproduction: pane scrolled by 10, `toggle_layout_mode` -> `sb=0`. Triggered by any layout change (stack/tile, drawer push, font zoom), not by deck switching. Fix: capture `scrollback_offset()` before reflow and `set_scrollback_offset(min(old, capacity))` after. Low-risk, isolated to src/vt/mod.rs.

### F3-e (open, not planned): inactive prompt outline restyle
`render_floating_prompt` draws an inactive pane's prompt with `OutlineStyle::Solid` when `prompt_outline == Double` (`else if !active && cfg.prompt_outline == OutlineStyle::Double`, src/ui/status_bar.rs:110), and also the active pane while in copy mode (`self.active && !self.pane.copy_mode().active`, src/ui/mod.rs:170). In Legacy look this is visible as the prompt "type" changing between double and single border when switching windows. It is deliberate rendering, so changing it needs a user decision (Open Question 2).

## Common Pitfalls

### Pitfall 1: Changing `drawer_vertical_layout_metrics` signature
Used by `src/input/mouse_wm.rs` (3x) and tests `input_mouse_wm_coverage_panels.rs:538`, `bilterm_live_driver_suite.rs:187`, `drawer_behaviour_coverage.rs:701,800,819`. Keep `(cfg, inner_height, item_count)`; read `cfg.drawer.classic_row_lines` inside.

### Pitfall 2: Classic 2-line breaking scroll/hit math
`list_items_cap = list_rows_px / row_step`, row index from `(mouse.row - list_start_y) / row_step`. Because both renderer and mouse already divide by `step`, only the metrics change; verify with a click on the second line of row k selecting row k.

### Pitfall 3: Double reflow/flicker on zoom follow
Calling `resize_all` per focus change is fine (it is idempotent: `resize_viewport` returns early if size unchanged, src/pane/mod.rs:451). Only do it when zoomed.

### Pitfall 4: Swallowing the whole focusing click
Swallow only OIA row and prompt-chrome hits. Swallowing all first clicks would break click-to-focus-then-select and existing mouse tests; run `tests/input_mouse_wm_coverage_*.rs`.

### Pitfall 5: Copy-mode leak
After removing the clearing, verify keys typed while another pane is active never reach the inactive pane's copy handler (`handle_key_event` runs on the active pane only) and that the OIA of the inactive pane shows its COPY state truthfully.

## Code Examples

### FIX-02 core (illustrative)
```rust
// Source: src/wm/mod.rs (current toggle_zoom at :925) - target shape
fn effective_zoomed(&self) -> Option<PaneId> {
    self.zoomed_pane.and(self.z_stack.top()).filter(|id| self.panes.contains_key(id))
}
pub fn zoomed_pane(&self) -> Option<PaneId> { self.effective_zoomed() }
pub fn toggle_zoom(&mut self) {
    let Some(top_id) = self.z_stack.top() else { return };
    self.zoomed_pane = if self.zoomed_pane.is_some() { None } else { Some(top_id) };
    self.reflow_default_viewport();
    self.maybe_autosave_layout();
}
```

### FIX-03a core (illustrative)
```rust
// Source: src/input/mouse.rs ~:839 - target shape
let was_active = matches!(hit_focus, Focus::Pane(id) if id == wm.active_pane_id());
if matches!(mouse.kind, MouseEventKind::Down(_)) && let Focus::Pane(hit_id) = hit_focus {
    wm.focus_and_raise(hit_id);
}
// later, once pane_inner/on_pane_status_bar are known:
if !was_active && matches!(pane_mouse.kind, MouseEventKind::Down(MouseButton::Left))
    && (on_pane_status_bar || on_prompt_chrome_row) {
    return Ok(AppAction::Handled);   // focus only; no mode/anchor change
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Classic drawer single-line telemetry rows | Navigator two-line rows (`row_lines`) | 2026-11-04 (1412846) | Classic never got the setting |
| Zoom id fixed at toggle time | Derive from focus (this phase) | Phase 61 | Removes hidden-pane input |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The user's runtime drawer style is Classic (or Liquid Glass look) | FIX-01 | If they are really in Navigator and still see one line, a different bug exists; plan 61-01 includes a diagnostic status line (`Drawer rows: N (style)`) that exposes it immediately |
| A2 | "Prompt type changes" refers to prompt mode/anchor changing on the focusing click (F3-a), or to the double/solid outline (F3-e) | FIX-03 | If it is the outline restyle only, F3-a/b/c still fix real defects but the visible symptom needs the F3-e decision |
| A3 | Follow-focus zoom is the desired UX (vs unzoom-on-switch) | FIX-02 | Tests encode follow-focus; changing is a one-line policy swap |
| A4 | Digits `1`/`2` are free in the Classic drawer key match | FIX-01 | Executor greps `src/input/modal.rs` before choosing; fall back to another free key |
| A5 | The 3.0.1 installed binary / running GUI process do not carry the user-visible symptom | FIX-01 | User may simply need to rebuild/reinstall; verify with `bilterm --version` and a fresh launch after fix |

## Open Questions

1. **Which look/style is the user in when rows are one line?**
   - Known: config says Navigator; Classic ignores `row_lines`; Navigator verified working headless.
   - Unclear: runtime style/look. Recommendation: ship the status-message diagnostic and ask the user to run `:drawer style` after the fix; plan handles both Classic and Liquid cases.
2. **Should an inactive window's prompt keep a double outline?** (F3-e) Recommendation: leave as is unless the user confirms this is the "prompt type" change; if so, make it config-driven (`inactive_prompt_outline = "keep"`).
3. **Zoom + focus change: follow or unzoom?** Recommendation: follow (planned).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo/rustc (edition 2024 crate) | build/tests | yes | builds ran this session | - |
| `scripts/verify.sh` | gates | yes (repo) | - | `cargo test -q` per test file |
| Display/GPU | none (headless tests only) | n/a | - | - |

No missing dependencies.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness, integration tests under `tests/*.rs` (one `t:<name>` verify stage per file) |
| Config file | none (Cargo.toml) |
| Quick run command | `cargo test -q --test phase61_drawer_rows` (per file) / `scripts/verify.sh --quick` |
| Full suite command | `scripts/verify.sh` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FIX-01 | Classic + `:drawer rows 2` -> `drawer_vertical_layout_metrics(..).2 == 2`; rendered row k has telemetry on line 2; `rows 1` back to 1; default (0) unchanged; Navigator unaffected; click on 2nd line selects row k; status names style; chip/key/menu reach same state | integration | `cargo test -q --test phase61_drawer_rows` | Wave 0 (new) |
| FIX-02 | zoom + `shuffle_deck_next`: `zoomed_pane()==Some(active)` and rects == [(active, area)]; `ZoomToggle` after focus change un-zooms; menu Window->Zoom via key Enter and mouse toggles twice; hit_test at centre == active | integration | `cargo test -q --test phase61_zoom_follows_focus` | Wave 0 (new) |
| FIX-03 | click on inactive OIA routing cell focuses only (modes unchanged); active-pane click still cycles; copy mode survives Alt+Tab round trip; raw_passthrough survives snapshot->restore; scroll offset survives reflow | integration | `cargo test -q --test phase61_window_state_preserved` | Wave 0 (new) |

### Sampling Rate
- Per task commit: the plan's single test file (`cargo test -q --test <file>`).
- Per wave merge: `scripts/verify.sh --quick`.
- Phase gate: full `scripts/verify.sh` PASS before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `tests/phase61_drawer_rows.rs`, `tests/phase61_zoom_follows_focus.rs`, `tests/phase61_window_state_preserved.rs` (new files; headless via `WindowManager::new_headless(rows, cols)`, isolate XDG dirs as `tests/navigator_two_line_rows.rs::isolate_xdg` does).
- [ ] Existing regression suites to re-run each wave: `navigator_two_line_rows`, `drawer_behaviour_coverage`, `input_mouse_wm_coverage_panels`, `bilterm_live_driver_suite`, `phase51_*`, `phase56_*`, `src/input/tests.rs` (zoom at :238-247), `src/wm/tests.rs` (zoom reap at :72-105).

## Security Domain

`security_enforcement` not explicitly disabled; applicable controls are small.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | `:drawer rows` parser accepts only `1`/`2` (src/colon_command/parse_visual.rs); new config field clamped like `navigator_row_lines()` |
| V2/V3/V4/V6 | no | no auth, session, access control or crypto touched |
| V12 Files | minimal | new `SavedPaneState` field rides existing `secure_fs` writers; `#[serde(default)]` keeps old files loadable |

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Hidden-pane input (keys to an invisible window) | Spoofing/Tampering | FIX-02: input target always equals the visible zoomed pane (a real security benefit: users could type secrets into a pane they cannot see) |
| Out-of-range config value | Tampering | treat anything not 1/2 as auto/2 (as `navigator_row_lines`) |

## Sources

### Primary (HIGH confidence)
- In-repo source read this session: src/ui/drawer.rs, src/ui/drawer/navigator.rs, src/wm/drawer.rs, src/wm/drawer/{navigator,nav,ops}.rs, src/wm/mod.rs, src/wm/z_stack.rs, src/wm/session.rs, src/wm/layout_profile.rs, src/wm/window_manage_bulk.rs, src/input/{mouse,mouse_wm,modal_palette}.rs, src/pane/mod.rs, src/vt/mod.rs, src/ui/{mod,status_bar,modals}.rs, docs/user-guide/11-navigator-sidebar-and-agent-traffic.md.
- git history: `git log -S"row_lines"`, `git show 1412846 / 31cd42c / 3d8e8d8`.
- Headless reproduction crates (scratchpad `exp`, `cfgcheck`; not committed) and `cargo test -q --test navigator_two_line_rows` (9 passed).

### Secondary / Tertiary
- None. No web research was needed; all findings are first-party code.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH (unchanged, no new deps)
- Architecture / FIX-02 / FIX-03a-b: HIGH (reproduced)
- FIX-01 user-runtime attribution: MEDIUM (mechanism proven, style unobservable)
- FIX-03c: MEDIUM (code-read, not run through restore)

**Research date:** 2026-11-08
**Valid until:** 2026-12-08 (stable code; re-check if Phases 62-64 land first, they touch input/copy mode)
