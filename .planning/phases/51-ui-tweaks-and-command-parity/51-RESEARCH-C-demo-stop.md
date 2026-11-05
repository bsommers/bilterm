# Phase 51 Research C: `:agents demo stop` (UIT-01)

**Researched:** 2026-11-05
**Scope:** Read-only. No source changed. Worktree: `/home/bill/src/ai/bilterm/.claude/worktrees/integ`
**Confidence:** HIGH for code-cited facts. One LOW item (PTY child termination, see Assumptions).

## Summary

`:agents demo` today adds 13 `[demo]` Message events between three panes, splitting the workspace if it has fewer than three. Nothing records which panes it created, so stop can't safely close them. The `[demo]` marker exists only in preview text, and the session topology keeps demo pairs after ring eviction, so stop needs a second purge.

**Primary recommendation:** Store `{endpoints, spawned}` pane ids in a private `WindowManager` field, with methods in a `wm` submodule. Close only `spawned` panes that still exist, via `remove_pane`. Remove ring events by `Message` kind, `"[demo] "` prefix, and demo endpoint ids. Purge the matching topology pairs. Make re-running demo replace its events rather than double them.

## Current Behaviour (verified)

- **Demo script and split loop.** `DEMO_SCRIPT` has 13 entries (`src/colon_command/execute_agents_topology.rs:151-165`). The split loop runs at most twice, from `split_focused` until `pane_count() >= 3` (`execute_agents_topology.rs:174-181`). IDs are the first three panes in `BTreeMap` order, `wm.panes().keys().take(3)` (`:182`). Pre-existing panes can be among them.
- **Event recording.** Each event is recorded via `wm.record_traffic(Pane(f), Pane(t), TrafficKind::Message, text, at)` (`:187-192`, call at `:191`). `record_traffic` redacts when the target pane is in a password prompt, replacing the preview with `"[redacted]"` (`src/wm/traffic.rs:231-234`, const at `:26`). A redacted demo event would lose its `[demo] ` prefix.
- **Status text.** `status()` writes to `active_pane_mut()` (`execute_agents_topology.rs:22-24`). The demo status string is at `:194-197`.
- **Ring and cap.** `TRAFFIC_CAP = 512` (`traffic.rs:22`). `record` pushes to a `VecDeque` and evicts from the front (`traffic.rs:147-160`). `next_seq` never decrements (`:139-140`, `latest_seq` at `:170-172`).
- **Event schema.** `TrafficEvent { seq, at, from, to, kind, preview, bytes, redacted }` (`traffic.rs:91-108`). There is no demo flag. `TrafficKind::Message` is documented as "Reserved for Phase 51." (`traffic.rs:72-73`). ADR-008 says Message is reserved for Phase 52 (`docs/architecture/adr/ADR-008-agent-traffic-bus.md:33`). The source comment is stale after the renumbering in commit `63480cf`.
- **Topology.** `Topology` is keyed by `(NodeKey, NodeKey)` (`src/wm/agents_topology.rs:115-120`). It is never trimmed by ring eviction (`:1-10`). It has `record`, `iter`, `pair`, `len`, and `dropped`, but no removal method (`:124-184`).
- **Graph scene.** Edges come from the topology (`src/wm/agents_window.rs:673-681`). Nodes come from ring events and topology endpoints (`:668-676`). A closed demo pane therefore still shows as a dead `pane:N` node with its edge unless the topology is purged.
- **Only pane-to-pane producer.** `TrafficKind::Message` and Pane-to-Pane recording are produced only by demo (`execute_agents_topology.rs:191`, the only `TrafficKind::Message` producer found by grep). Other producers use User, Api, or Mcp as the source (`src/api/rpc.rs:467` Api to Pane; `src/wm/v2_runtime.rs:54-57`). The `rpc.rs:318` call was not read individually. So Pane-to-Pane pairs are demo-only today.
- **Pane close.** `remove_pane(id)` refuses when `panes.len() <= 1` or the id is missing, and returns `false` (`src/wm/mod.rs:650-666`, guard at `:651`). It also updates split tree, z-stack, zoom, and fleet references. `close_focused_pane` closes the z-stack top, which may be a user pane (`:669-678`). Do not use it for stop.
- **Pane id allocation.** `split_focused` allocates `next_pane_id` and increments (`src/wm/mod.rs:615-618`). Two sites reset or recompute it: `create_new_session` sets it to `2` (`src/wm/session_manager.rs:234`) and `apply_saved_session_snapshot` sets `max_id + 1` (`src/wm/layout_profile.rs:611`). IDs can therefore be reused after a session or layout restore. `insert_tmux_pane` uses `.max(...)` (`src/wm/v2_runtime.rs:79-81`) and does not reuse IDs.
- **Agents window state.** `AgentsWindowState` has public fields including `selection` and `snoop` (`src/wm/agents_window.rs:384-405`). `SnoopState` has `filter_agent`, `filter_pair`, and `selected_seq` (`:441-456`). `agents_window_mut()` is public (`:577`), so colon_command code can reset these without new accessors.
- **Snoop fallback.** `snoop_selected_seq` falls back to the newest line when the selected seq is absent (`:907-913`).
- **Parse path.** `:agents demo` parses only with `parts.len() == 2` (`src/colon_command/parse_agents.rs:207`). `:agents demo stop` falls through to `Err(USAGE)` today. USAGE is at `parse_agents.rs:8`.
- **Dispatch.** `AgentsOp::Demo` routes at `src/colon_command/execute_agents.rs:29`. `execute_window_op` has an exhaustive "Handled by the caller" list that includes `AgentsOp::Demo` (`execute_agents.rs:157-178`, entry at `:161`). Adding a variant requires editing that list or the build fails.
- **API path.** `wm.command` parses any colon command and applies it with `SnapshotPolicy::Confined` (`src/api/rpc.rs:478-493`). No per-verb allowlist was seen on that path. `:agents topology export` is refused over the API (`execute_agents_topology.rs:54-56`), but demo has no API gate.
- **Palette.** `:agents demo` has no PALETTE_CATALOG row. It appears only in HELP_LINES at `src/ui/help_catalog.rs:95`. The `:agents` palette rows are at `:293-321`, inside PALETTE_CATALOG (`:220-489`). `command_registry::catalog` takes the summary from `palette.first()` for the head (`src/command_registry.rs:123-128`). New palette rows must come after `:agents list` (`help_catalog.rs:293`).
- **Parity test gap.** `agents_op_colon_form` has an arm for Demo (`tests/phase50_agents_window.rs:514`). `sample_ops()` (`:596-673`) has no `AgentsOp::Demo`, so demo is not covered by the completion/help parity test.
- **Doc script baseline.** I ran `scripts/check_phase50_doc_coverage.sh` read-only. It printed `DOCS OK 57`. Count math: 31 `:agents` row verbs plus 23 new `:drawer` verbs (versus base `0915792`) plus 3 phrases is 57. The `:agents` row is at `src/pane/completion.rs:479`.

## Design Recommendations

### 1. Where to store demo pane ids
- Add a private field to `WindowManager` (`src/wm/mod.rs`, after `agents_window` at `:135-136`), initialised next to `agents_window: agents_window::AgentsWindowState::default()` (`:216`). Private, not `pub(crate)`, per ARCH-R02.
- Struct: `AgentsDemoState { endpoints: Vec<PaneId>, spawned: Vec<PaneId> }`, `Default`.
  - `endpoints`: the three ids demo sent between.
  - `spawned`: only the ids created by this demo, computed as keys not in the pre-run `panes()` set.
- Put the methods in a new `src/wm/agents_demo.rs` (declared from `wm/mod.rs`). Descendants of `wm` may access private fields. `colon_command` may not, so the executor should only call methods and format status.
- Suggested methods: `start_agents_demo` and `stop_agents_demo`, each returning a small report struct (counts and a `was_running` flag).

### 2. Closing only demo-created panes
- Iterate `spawned` in reverse creation order. For each id:
  - If `wm.pane(id).is_none()`, skip it (already closed by the user).
  - Else call `wm.remove_pane(id)`. If it returns `false`, count it as kept (last pane).
- Never close endpoints that were pre-existing. Never call `close_focused_pane`.
- Clear `spawned` and `endpoints` after stop.

### 3. Removing only demo events from the ring
Add `TrafficBus::retain_events(pred) -> usize` in `src/wm/traffic.rs`. It should use `VecDeque::retain` and return the number removed. It must not change `next_seq`, `dropped`, or `latest_seq`. Predicate:

```
e.kind == TrafficKind::Message
  && e.preview.starts_with("[demo] ")
  && matches!((&e.from, &e.to), (Pane(a), Pane(b)) if endpoints.contains(a) && endpoints.contains(b))
```

- Demo texts all start with `"[demo] "` (`execute_agents_topology.rs:152-164`).
- `TrafficKind` derives `PartialEq` (`traffic.rs:62`).
- Do not add a `demo` field to `TrafficEvent`. ADR-008 says the schema is costly to change (`ADR-008...:18`). If a schema change is wanted anyway, it needs an ADR update.
- **Required demo change:** record with `record_traffic_with(..., false, at)` (`traffic.rs:239-249`) instead of `record_traffic`. Demo text is synthetic and not sensitive. Without this, a password-prompt endpoint produces `"[redacted]"` previews that the prefix predicate cannot match.

### 4. Purging topology
Add `Topology::remove_pair(&mut self, a: &NodeKey, b: &NodeKey) -> bool` in `src/wm/agents_topology.rs`, mirroring `pair()` (`:166-169`). Remove the three endpoint pairs (0-1, 1-2, 0-2). Only remove pairs where both endpoints are demo endpoints. Pane-Pane pairs are demo-only today (see Current Behaviour), so this is safe now. If Phase 51/52 adds real Pane-Pane traffic, add a per-pair demo-only flag first.

### 5. Clean up stale UI references
After closing panes and purging events:
- If `agents_window.selection.node` or `.edge` refers to a removed `pane:<id>`, reset `selection` to default.
- If `snoop.filter_agent` or `snoop.filter_pair` refers to a removed key, clear it.
- `selected_seq` is handled by the newest-line fallback (`agents_window.rs:907-913`).

### 6. No demo and demo twice
- **Stop with no demo:** `AgentsDemoState` is empty. Report "No agent demo to stop", touch nothing, and do not scan the ring.
- **Demo run twice (today):** the second run does not split, since `pane_count() >= 3`. It reuses the same three panes and appends 13 more events. The ring then holds 26 demo events and topology counts double. Demo does not currently guard against this.
- **Recommended:** if `AgentsDemoState` is non-empty, first purge old events and topology pairs (do not close panes), then re-emit. Merge `spawned` (`old ∪ new`), never overwrite it with an empty list. Otherwise panes from the first run leak, because the second run spawns nothing.
- **Alternative:** refuse with "Demo already running; :agents demo stop first". This is simpler but less friendly.

### 7. API exposure
`wm.command` and MCP `bilterm_colon_command` will accept `:agents demo stop` with no gate, matching demo. Stop closes panes, which is a new capability over RPC. Recommendation: allow it, because it only touches demo-marked state. Refusing is also viable, following the `export` pattern at `execute_agents_topology.rs:54-56`. This is a planner decision. If allowed, add a line to `docs/API.md` near the Agent State and Topology sections (`:883-923`).

## Files That Must Change

| File | Line(s) | Change |
|---|---|---|
| `src/wm/agents_cmd.rs` | `:81-83` | Add `DemoStop` variant after `Demo`, with doc comment. |
| `src/colon_command/parse_agents.rs` | `:8`, `:207` | Add `Some("demo") if parts.len() == 3 && parts[2] == "stop"` → `DemoStop`. Add `Some("demo")` usage error fallback. Update USAGE to `demo [stop]`. No test pins USAGE. |
| `src/colon_command/execute_agents.rs` | `:29`, `:157-178` | Add `AgentsOp::DemoStop => super::execute_agents_topology::demo_stop(wm)`. Add `\| AgentsOp::DemoStop` to the exhaustive no-op list in `execute_window_op`. |
| `src/colon_command/execute_agents_topology.rs` | `:151-198` | `demo()`: capture pre-run ids, record spawned, use `record_traffic_with(..., false, at)`, replace-on-rerun. Add `demo_stop()` that formats status only. |
| `src/wm/mod.rs` | `:135-136`, `:216`, `:650-666` (no change to remove_pane) | Add private `agents_demo` field and initialiser. Declare `mod agents_demo;`. |
| `src/wm/agents_demo.rs` | new (~150 lines) | `AgentsDemoState`, `start_agents_demo`, `stop_agents_demo`. |
| `src/wm/session_manager.rs` | `:195-234` | Clear `agents_demo` in `create_new_session` (pane-id reuse). |
| `src/wm/layout_profile.rs` | `:506-611` | Clear `agents_demo` in `apply_saved_session_snapshot` (pane-id reuse). |
| `src/wm/traffic.rs` | `:21-26`, `:112-203` | Add `retain_events`. |
| `src/wm/agents_topology.rs` | `:115-184` | Add `remove_pair`. |
| `src/pane/completion.rs` | `:479` | Add `"demo stop"` entry to `:agents` row. Do not use brackets (see Pitfalls). |
| `src/ui/help_catalog.rs` | `:95` (HELP_LINES), after `:293` (PALETTE_CATALOG) | Add HELP_LINES row `(":agents demo stop", ...)`. Add PALETTE rows `(":agents demo", ...)` and `(":agents demo stop", "—", ...)` after `:agents list`. |
| `tests/phase50_agents_window.rs` | `:514`, `:596-673` | Add `AgentsOp::DemoStop => ":agents demo stop"` arm. Add `AgentsOp::Demo` and `AgentsOp::DemoStop` to `sample_ops()`. |
| `tests/agents_launch_affordances.rs` | `:113-124` | Add stop tests here, or in a new `tests/phase51_demo_stop.rs`. |
| `docs/user-guide/11-navigator-sidebar-and-agent-traffic.md` | after `:252` | Add row `:agents demo stop` (exact string required by the script). |
| `docs/API.md` | `:883-923` | Optional: mention `:agents demo stop` over RPC/MCP. |
| `scripts/check_phase50_doc_coverage.sh` | none | No edit. Count changes from 57 to 58 automatically via the completion row. |

## Test Plan

1. **Parse:** `:agents demo stop` gives `DemoStop`. `:agents demo x` and `:agents demo stop x` give usage errors.
2. **Pane count:** `fixture(120,40)` has 2 panes. Demo splits once, to 3. Stop returns to 2. Pre-existing ids are still present.
3. **Ring:** after stop, no `[demo] ` preview remains. A user `Input` event recorded before or after demo survives.
4. **Topology:** after stop, `traffic().topology().pair(&NodeKey::pane(a), &NodeKey::pane(b))` is `None` for each demo pair.
5. **No demo:** stop reports "No agent demo" and leaves panes and ring unchanged.
6. **Demo twice:** 26 events become 13 after the second demo (replace semantics). Stop closes the spawned pane, so no leak.
7. **User closed a spawned pane first:** stop skips it without error.
8. **Last-pane guard:** if only one pane remains, stop keeps it and reports it.
9. **Redaction:** with a pane in password-prompt state as a demo endpoint, demo events are still removed by stop.
10. **Parity:** `sample_ops` includes Demo and DemoStop. Palette has demo and demo stop rows. `parse_colon_command(":agents demo stop")` succeeds (palette parse test).
11. **Doc script:** `bash scripts/check_phase50_doc_coverage.sh` prints `DOCS OK 58`.
12. **Headless:** use `WindowManager::new_headless` (no PTY).

Run via `scripts/verify.sh` (per AGENTS.md).

## Pitfalls

1. **`close_focused_pane` closes the wrong pane.** It uses the z-stack top, which may be a user pane. Close by stored id only (`src/wm/mod.rs:669-678`).
2. **Overwriting `spawned` on re-run leaks panes.** Merge, do not replace (Design 6).
3. **Redaction hides demo events from the prefix match.** Use `record_traffic_with(..., false, ...)` in demo (Design 3).
4. **Pane id reuse.** Session restore (`layout_profile.rs:611`) and `create_new_session` (`session_manager.rs:234`) can reuse ids. Clear `agents_demo` there, or a stale id could match a user pane.
5. **Topology is not in the ring.** Ring-only cleanup leaves dead `pane:N` nodes in the graph (`agents_window.rs:673-681`).
6. **`remove_pane` refuses the last pane** (`mod.rs:651`). Report it; do not treat `false` as a failure.
7. **Palette summary ordering.** New `:agents` palette rows must come after `:agents list` (`command_registry.rs:126`).
8. **Doc-script regex.** `row_verbs` uses `\[[^]]*\]` (`check_phase50_doc_coverage.sh`). Brackets in a completion entry break it. `"demo stop"` is safe.
9. **Predicate collision.** Phase 51/52 messages may begin with `[demo]`. The predicate requires `Message` kind and demo endpoint ids to avoid that.
10. **Schema change needs an ADR.** Do not add fields to `TrafficEvent` without updating ADR-008.
11. **Stale phase comments.** `traffic.rs:43`, `:72-73`, and `wm/agents_cmd.rs:1-4` say "Phase 51" for messaging. ADR-008 says Phase 52. Fix these in the same change if touched.

## Open Questions

1. **API policy:** allow `:agents demo stop` over RPC/MCP, or refuse like export? (Design 7)
2. **Demo twice:** replace (recommended) or refuse? (Design 6)
3. **Redaction:** should demo stop redact its own synthetic text? (Design 3 changes it to not redact.)

## Assumptions Log

| # | Claim | Basis | Risk if wrong |
|---|---|---|---|
| A1 | `CompositePane` drop terminates the child shell, so `remove_pane` does not leak processes. | `impl Drop for PtySession` exists at `src/pty/mod.rs:285`. Its body was not read. [ASSUMED] | Demo panes could leave shells running after stop. Check the Drop body, or reuse the same path as `:close-pane`. |
| A2 | Removing arbitrary events from the ring is handled by the snoop view. | `SnoopState` doc at `agents_window.rs:448-454` describes seq-anchored scroll (WR-03). Not tested. | A scrolled-back view could jump. Add a test. |
| A3 | Pane-to-Pane pairs are demo-only today. | grep of `record_traffic*` call sites. `rpc.rs:318` was not read. | Topology purge could remove real pairs. Confirm before Phase 51/52 ships messaging. |
