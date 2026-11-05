---
phase: 50-agent-aware-workspace-navigator
reviewed: 2026-11-03T00:00:00Z
depth: standard
files_reviewed: 48
files_reviewed_list:
  - src/wm/agent_state.rs
  - src/wm/traffic.rs
  - src/wm/agents_window.rs
  - src/wm/agents_geometry.rs
  - src/wm/agents_cmd.rs
  - src/wm/drawer.rs
  - src/wm/drawer/nav.rs
  - src/wm/drawer/menu.rs
  - src/wm/drawer/navigator.rs
  - src/wm/mod.rs
  - src/wm/v2_runtime.rs
  - src/wm/session_tree.rs
  - src/wm/z_stack.rs
  - src/colon_command/execute_agents.rs
  - src/colon_command/parse_agents.rs
  - src/colon_command/parse_visual.rs
  - src/colon_command/execute_wm.rs
  - src/colon_command/parse.rs
  - src/colon_command/mod.rs
  - src/action.rs
  - src/api/rpc.rs
  - src/config/agents.rs
  - src/config/mod.rs
  - src/input/agents_window.rs
  - src/input/drawer_navigator.rs
  - src/input/gui_route.rs
  - src/input/keymap.rs
  - src/input/modal.rs
  - src/input/mouse.rs
  - src/input/mouse_wm.rs
  - src/input/mod.rs
  - src/ui/agents_ui.rs
  - src/ui/agents_gui.rs
  - src/ui/graph_raster.rs
  - src/ui/drawer/navigator.rs
  - src/ui/drawer.rs
  - src/ui/renderer.rs
  - src/ui/typography.rs
  - src/ui/gui_font/linux_x11.rs
  - src/vt/codec.rs
  - src/pane/completion.rs
  - src/daemon/mod.rs
  - src/mcp_console/mod.rs
  - src/command_registry.rs
  - src/ui/help_catalog.rs
  - src/ui/mod.rs
  - scripts/check_classic_goldens.sh
  - scripts/check_phase50_doc_coverage.sh
findings:
  critical: 1
  warning: 7
  info: 9
  total: 17
status: issues_found
---

# Phase 50: Code Review Report

**Reviewed:** 2026-11-03
**Depth:** standard
**Files Reviewed:** 48 (tests and snapshots excluded)
**Status:** issues_found

## Summary

Phase 50 adds the agent state tracker, traffic bus, Navigator drawer, AGENT TRAFFIC window (TUI and GUI) and
the `:agents` / `:drawer` command families. Positive checks that held up:

- `handle_wm_key_event` is 48 lines (ARCH-R01 limit 80). The phase added no logic to it.
- All new user-triggerable operations lower to `Action` and run through `WindowManager::apply` (`ColonCommand` is an alias of `Action`).
- Pane removal goes through `remove_pane`. No new file I/O was added, so no `secure_fs` rules apply.
- The compose line carries no text, has no colon form, and `ComposeSubmit` only sets a status message.
- Labels from pane titles, session names and agent names go through `sanitize_display`. The identity token is restricted to `[A-Za-z0-9._-]`.

The main defect is that the compose gate can be bypassed through the paste path. The window is also an input trap
when it is invisible, scaled-window geometry uses the wrong coordinate space, and the snoop view drifts under
ring eviction.

## Critical Issues

### CR-01: Paste bypasses the agents window input capture and writes to the pane beneath (outward write path from compose)

**Status:** fixed (8e7673d): `handle_wm_paste_event` routes Event::Paste through the agents window capture; only keyword entry accepts text.

**File:** `src/app.rs:142-144` (consumer), `src/input/agents_window.rs:23-61` (capture contract)
**Issue:** `handle_agents_window_key` promises that "keys never leak into the pane below". `Event::Paste` is routed
unconditionally to `handle_paste_event(self.wm.active_pane_mut(), ..)` and never consults
`wm.agents_window().captures_input()`. While the window owns the keyboard (compose line, keyword entry, graph),
a bracketed paste, which also arrives from the GUI backends through `gui_macos.rs:536`, is delivered to the
active pane:
- In `RawPtyPassthrough` mode the text is written to the PTY (bracketed, or directly when single-line).
- In `CommandPrompt` mode it is inserted into the hidden pane prompt.

This is exactly the outward write path from the `send as me` compose stub that D-09 forbids. The user believes
they are typing into a disabled compose box, and the text goes to a shell or agent instead. Pasting into the
keyword filter has the same problem.
**Fix:**
```rust
Event::Paste(text) => {
    if self.wm.agents_window().captures_input() {
        // Keyword entry accepts pasted text; every other window pane swallows it.
        if self.wm.agents_window().snoop.keyword_entry.is_some() {
            for c in text.chars().filter(|c| !c.is_control()) {
                let _ = self.wm.apply(Action::Agents(AgentsOp::KeywordEdit(KeywordEdit::Push(c))));
            }
        }
    } else {
        let _ = handle_paste_event(self.wm.active_pane_mut(), &text)?;
    }
}
```
Add a regression test in `tests/phase50_agents_window.rs` that opens the window, focuses compose, sends
`Event::Paste` and asserts that the active pane's PTY and prompt are unchanged.

## Warnings

### WR-01: Invisible agents window still captures the whole keyboard (input trap)

**Status:** fixed (f9eed86): `WindowManager::agents_window_captures_input()` requires the window to be drawn.

**File:** `src/wm/agents_window.rs:497-499`, `src/input/modal.rs:334-339`
**Issue:** `captures_input()` only checks `active && (!pinned || focused)`. `agents_window_rect()` returns `None`
(window not drawn) when the workspace is smaller than 30x10, and `agents_pane_rects()` returns `None` for tiny
rects. After a terminal resize below the minimum, the window is invisible but `handle_agents_window_key` still
consumes every key except the leader. The mouse handler correctly returns `None` in that state, so keyboard and
mouse disagree. Only Esc, which is unclaimed in the invisible state, recovers.
**Fix:** Make the capture predicate visibility-aware at the call site, for example:
```rust
if wm.agents_window().captures_input()
    && wm.agents_window_rect(wm.current_workspace_area().1).is_some() { ... }
```
or add `WindowManager::agents_window_captures_input()` that includes the rect check, and use it in `modal.rs`.

### WR-02: Window geometry operations use the unscaled workspace while the window renders and hit-tests in the scaled (virtual) space

**Status:** fixed (c217d05): `agents_workspace()` uses the scaled virtual area; `snoop_view_rows` uses it too.

**File:** `src/wm/agents_geometry.rs:228-230,238-250,253-290,325-341`, `src/wm/agents_window.rs:883-887`
**Issue:** The renderer (`renderer.rs`), the mouse handler (`panel_space`) and `agents_gui` all work in
`panel_virtual_area(ws, Agents)` when `agents_scale_pct != 100`. But `agents_workspace()` returns
`current_workspace_area().1`, which is unscaled. The following therefore clamp and compute against the wrong area at scale != 100:
- `set_agents_rect`
- `move_agents_window`
- `resize_agents_window`
- `set_agents_size`
- `nudge_agents_node`
- `snoop_view_rows`, which feeds `keep_snoop_selection_visible` and `scroll_snoop`

Consequences at 150-200%:
- Shift/Alt+arrows start from the wrong default rect, so the window jumps.
- A stored rect can lie outside the virtual area and be clipped by `intersection`.
- The visible conversation row count differs from the count used for scroll and selection, so the selected line scrolls out of view.
- At 75% the user cannot use the extra virtual area.

**Fix:** Derive the workspace from the same helper the renderer uses:
```rust
fn agents_workspace(&self) -> Rect {
    let ws = self.current_workspace_area().1;
    self.panel_virtual_area(ws, crate::ui::typography::PanelTarget::Agents)
}
```
and use `agents_workspace()` in `snoop_view_rows` too.

### WR-03: Non-follow conversation view drifts when the traffic ring evicts old events

**Status:** fixed (1322c65): `SnoopState.scroll_seq` anchors the scrolled view on a bus sequence number.

**File:** `src/wm/agents_window.rs:418-425,467-474`, `src/wm/traffic.rs:153-156`
**Issue:** `SnoopState.scroll` is an index counted from the oldest filtered line, and `SnoopState` documents
"new traffic never moves the view". Once the bus holds `TRAFFIC_CAP` (512) events, every new event evicts index 0,
so the same index now names a newer line and the history view creeps forward. This also happens when a filter
changes the line set. `selected_seq` correctly uses sequence numbers, but the scroll anchor does not.
**Fix:** Anchor the scrolled view on a bus sequence number (`scroll_seq: Option<u64>` = seq of the bottom
visible line) and resolve it to an index in `snoop_view_range` callers. Fall back to the oldest line when that
seq has been evicted.

### WR-04: Untrusted names collapse into one graph node, and sanitisation misses invisible/format characters

**Status:** fixed (6a7de68): injective node keys (hash suffix for altered names) and `sanitize_display` drops invisible/format characters.

**File:** `src/wm/agents_window.rs:42-51`, `src/vt/codec.rs:187-199`
**Issue:**
1. `NodeKey::from_endpoint` builds keys from `sanitize_display(name, 64)`, so distinct endpoints merge into one
   node and one edge set. Examples are two session or agent names that differ only in dropped control or bidi
   characters, or that share a 64-char prefix (the second becomes `…`-terminated, but all longer names with the
   same first 63 chars are identical). In a security-oriented traffic view this lets an endpoint hide its
   traffic inside another's node, which matters once Phase 52 adds `Agent(String)` endpoints.
2. `sanitize_display` drops C0, DEL, C1, bidi embedding/override/isolate controls, LRM/RLM and BOM. It still
   passes zero-width and invisible format characters: U+200B-U+200D, U+2060-U+2064, U+061C (ALM, a bidi mark),
   U+2028/U+2029 and the Unicode tag block U+E0000-E007F. An agent or session name can therefore be visually
   identical to another name, which defeats the label-spoof mitigation (T-50-04).

**Fix:** Extend the drop list in `sanitize_display` with the ranges above. For keys, use an injective encoding
of the raw name (for example a hash of the full raw string, or the raw name escaped with `escape_default`),
and keep the sanitised, truncated form for the label only.

### WR-05: Navigator row builder performs filesystem I/O on every call, and rows are rebuilt many times per frame and per event

**Status:** fixed (20536d8): saved-session list snapshotted in `WorkspaceDrawer`, invalidated by every `apply` and every 32 ticks.

**File:** `src/wm/drawer/navigator.rs:170-174` (`list_sessions`), called via `drawer_rows()` from
`src/ui/drawer/navigator.rs:171-193` and `src/input/drawer_navigator.rs:45-75`
**Issue:** `navigator_drawer_rows()` calls `self.list_sessions()`, which calls `resolve_session_dir()` and
`read_dir`. `navigator_layout()` calls `drawer_rows()`, and each of `navigator_dot_cells`, `navigator_comms_cells`,
`navigator_fold_marker_cells`, `navigator_plus_cells`, `navigator_footer_buttons`, `navigator_row_at`,
`navigator_menu_layout`, `selected_row_has_comms` and `drawer_open_comms` calls `navigator_layout()`. One render
therefore performs at least 6-8 directory scans, and a mouse click performs several more. During animation
(`ANIM_INTERVAL` = 120 ms) this repeats continuously. Because the directory can change between calls, the renderer
and the hit-test can see different row lists, so `row_index` can point at a different row than the one drawn.
`resolve_session_dir` may also create the directory from a render path.
**Fix:** Snapshot the saved-session list once per tick (cache it in `WorkspaceDrawer`, refreshed from
`update_drawer_telemetry` or on session create/rename/delete). Have the layout helpers take one `rows` snapshot
per frame or event instead of recomputing it.

### WR-06: One-key and one-click destructive pane close with no confirmation, next to adjacent single-cell targets

**Status:** fixed (45eb5eb): closing a Working pane from the Navigator needs a second close within ~5 s (no generic confirm modal exists; the review offered this alternative).

**File:** `src/input/keymap.rs` (`x`/`Delete` bindings), `src/wm/drawer/menu.rs:42,69`, `src/ui/drawer/navigator.rs:359-402`
**Issue:** `DrawerNavOp::Close` runs `remove_pane` on the selected pane, which kills the process. It is reachable by
`x`/`Delete`, the footer `x` button and the right-click menu `Close` entry. The footer buttons are single-cell
targets on a 2-cell pitch (`r`, `x`, `@` side by side). Now that rows show Working agents, a mis-click or
a stray `x` terminates a long-running agent session with no prompt, even though panes under exit-guard policy
get a confirmation elsewhere.
**Fix:** Route navigator close through the existing exit-guard or confirm path when the pane is `Working` or has
a running command. Alternatively require a second press (`x` twice, or a menu confirm) for non-idle panes.

### WR-07: Agents window keyboard handler runs before all modal handling

**Status:** fixed (045ac33): modals get keys before the agents window and the window is drawn below modals (TUI path).

**File:** `src/input/modal.rs:334-339`, `src/ui/mod.rs:730`
**Issue:** `handle_agents_window_key` is the first thing in `handle_wm_modal_key`, ahead of Help, Inspector,
Palette, MenuBar and ExitGuard. The window is also rendered after `render_exit_guard_modal`, so it is drawn on top
of that modal. If a pane exits and raises `Modal::ExitGuard` while the window is open, the guard is hidden
and its y/n/Enter keys go to the agents window instead. Enter then triggers `Activate`/`ComposeSubmit`, and
the guard cannot be answered until the window is dismissed (Esc on a pinned window only releases focus).
**Fix:** Check `wm.modal().is_some()` (or the specific modal kinds) before routing keys to the agents window,
and render modals above the window. Alternatively auto-release window focus when a modal opens.

## Info

### IN-01: `ComposeGate` text advertises a config key that does not exist

**File:** `src/wm/agents_window.rs:903`
**Issue:** The text says `[api] allow_agent_messaging`, but no such key exists in `ApiConfig`. This is
user-visible and misleading.
**Fix:** Say "agent messaging arrives with Phase 52" without naming a key, or add the key (default false) when Phase 52 lands.

### IN-02: `split_panes` can panic and its bounds disagree with `SPLITTER_RANGE`

**File:** `src/ui/agents_ui.rs:58-67`
**Issue:** `graph_w.clamp(1.min(w), w.saturating_sub(2))` panics (`min > max`) when `inner.width == 1`. It is `pub`
and only guarded by `agents_pane_rects` (width >= 6). The percent clamp `10..=90` differs from the setter
range `20..=80` (`agents_geometry.rs:18`).
**Fix:** Use `graph_w.min(max).max(min)` or early-return for small widths, and share a single range constant.

### IN-03: `u16` arithmetic can overflow for very wide workspaces

**File:** `src/wm/agents_window.rs:566-571`
**Issue:** `workspace_area.width * 80 / 100` (and the height equivalent) overflows `u16` above 819 columns or
caps rows, which wraps in release and panics in debug. High-DPI GUI grids can approach that.
**Fix:** `(u32::from(w) * 80 / 100) as u16`.

### IN-04: `drawer_rename_selected` always reports success

**File:** `src/wm/drawer/nav.rs:267-274`
**Issue:** `self.apply(Action::SessionTreeCommand(..)).is_ok()` is true even when the rename fails, because
`rename_session` reports its error through a status message and `apply` still returns `Ok`. The `changed`
return of `apply_drawer_nav` is therefore wrong for failed renames. `rename_session` also renames only the
on-disk and profile state, not the `SessionTree` node the navigator displays.
**Fix:** Call `rename_session` directly and propagate its `Result`, and rename the tree node there as well.

### IN-05: `sanitize_display` buffers the whole input before capping

**File:** `src/vt/codec.rs:187-199` (hot caller `src/wm/traffic.rs:141`)
**Issue:** `collect::<Vec<char>>()` over the full string allocates 4x the payload before truncation. The traffic
bus calls it on whole `pane.exec` / broadcast commands, which arrive over the API.
**Fix:** `raw.chars().filter_map(..).take(max_chars + 1)`, then decide on ellipsis from the count.

### IN-06: Raw command text is retained and shown in the conversation view

**File:** `src/api/rpc.rs:306-320,458-468`, `src/wm/traffic.rs:213-227`
**Issue:** Previews of `pane.exec`, broadcast and `send_keys` payloads are stored (160 chars each, 512 events),
rendered in the window, copyable to the system clipboard, and captured by workspace recording (REC-02).
Redaction applies only when the target pane is at a password prompt. Secrets typed inline
(`export TOKEN=...`) are retained in the clear. This is within the recorded T-50-11..13 acceptance but worth a
documented caveat or a `traffic.preview = off` option.
**Fix:** Document it, or add a config switch that stores only `bytes` and `kind`.

### IN-07: Agent state derives from untrusted screen text

**File:** `src/wm/agent_state.rs:102-134`
**Issue:** Any program output matching a profile `blocked` or `working` phrase in the last 8 rows sets the displayed
state, and `Blocked` drives `NextAttention` focus jumps. Output (for example `cat` of a hostile file) can force
attention or mask a real Blocked state. This is inherent to the heuristic design, but it is not stated in the user docs.
**Fix:** Add a short note to the Navigator chapter's profile section.

### IN-08: Layering and size

**File:** `src/wm/agents_geometry.rs:12-13`, `src/wm/agents_window.rs:886`, `src/wm/agents_window.rs` (1081 lines)
**Issue:** `wm::agents_geometry` and `wm::agents_window` import `ui::agents_ui` and `ui::graph_raster` (`split_panes`,
`snoop_lines_area`, `node_px`, `label_cell`), while `ui` imports `wm`. That creates a wm<->ui dependency cycle,
and geometry used by hit-testing lives in the renderer. `agents_window.rs` exceeds the ARCH-R04 ~800-1000 line
target (layout engine, selection model and snoop logic in one file). `ui/drawer/navigator.rs:766` also indexes
`buf[(x, y)]` unchecked, where every other path uses `cell_mut`.
**Fix:** Move `split_panes`, `snoop_lines_area` and the cell-geometry helpers into a `wm`-level layout module
that both layers use. Split `agents_window.rs` into `agents_layout.rs` and `agents_snoop.rs`. Use `cell_mut`
for the dot write.

### IN-09: Script gates are brittle

**File:** `scripts/check_classic_goldens.sh:15-19`, `scripts/check_phase50_doc_coverage.sh:8-9`
**Issue:**
- The golden gate fails permanently once any classic golden is legitimately updated (it demands exactly one commit per snapshot), and it fails on any dirty `tests/snapshots/`.
- The doc gate hardcodes `BASE=0915792`, which predates the phase plan commit `091299a`. The "new drawer verbs" set therefore depends on a research-only commit. It also silently uses `|| true` around the `grep -vxF` filter, so an empty base verb list would mark every verb new.
- `modifiers_from_x11_state` is now applied to every X11 press, release and motion event (`linux_x11.rs:697-745`), whereas Win32 and macOS were not updated. Ctrl/Alt/Shift-click behaviour (for example Alt rect-select) now differs between backends.

**Fix:** Compare golden content to a recorded hash manifest instead of commit counts. Read the base from the phase
metadata (or `git merge-base`). Apply the modifier mapping on the other GUI backends, or document the divergence.

---

_Reviewed: 2026-11-03_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
