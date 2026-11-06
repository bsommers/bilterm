---
phase: 51-ui-tweaks-and-command-parity
plan: 10
subsystem: api
tags: [rpc, mcp, agents-list, agents-demo, UIT-01, UIT-03]
requires: ["51-01", "51-02"]
provides:
  - "RPC/HTTP/ctl methods agents.list and agents.demo"
  - "MCP tools bilterm_agents_list and bilterm_agents_demo (tools/list = 13)"
key-files:
  created:
    - src/api/rpc_agents.rs
    - tests/phase51_api_parity.rs
  modified:
    - src/api/rpc.rs
    - src/api/mcp.rs
    - src/api/mod.rs
    - tests/v110_v120_v130_automation_api_mcp_playbooks.rs
    - tests/api_mcp_resources.rs
    - tests/api_error_paths.rs
    - tests/phase50_1_api.rs
decisions:
  - "agents.list with args but no op (or op list) is rejected (invalid agents.list parameter: args need an op)"
  - "the wm.command result shape is rebuilt in a private helper in rpc_agents.rs; the wm.command arm is untouched"
status: complete
plan_head_before: 0d52cae2f4fc6ee98d42a2ff788b91c9621eed06
plan_head_after: 39c7f3a3bd0254f263964d82aec27eaa007aece7
commits: 1
actuals:
  tokens: 11000
  tasks: 2
  commits: 1
---

# Phase 51 Plan 10: agents.list / agents.demo RPC and MCP Summary

Named `agents.list` and `agents.demo` methods (RPC, HTTP `POST /rpc`, JSON-RPC TCP/socket, `bilterm ctl`) and MCP tools `bilterm_agents_list` / `bilterm_agents_demo`, all lowering to `:agents list [op args]` / `:agents demo [stop]` through `parse_colon_command` and `apply_with_policy(.., Confined)`.

Commit: `39c7f3a` (both tasks in one commit, tests written first).

## For plan 51-11 (docs)

- New RPC methods: `agents.list` `{op?, args?}`, `agents.demo` `{stop?}`. Added to `RPC_METHODS` (now 25 entries).
- MCP tool count: 13 (was 11 after Phase 50.1; plus two, appended after `bilterm_find`).
- Closed `op` set: `open close toggle dismiss blur select jump zoom pin unpin scale filter dock float move resize rect size scroll`; absent or `list` is the data read (also opens the window, D-03).
- Tokens: `[A-Za-z0-9+\-_.#%:]`, 1..=32 chars, no leading `:`, at most 8; `args` is an array of strings/integers or one whitespace-separated string. Rejections are `invalid agents.list parameter: <reason>` / `invalid agents.demo parameter: <reason>` (JSON-RPC -32000; MCP `isError: true`).
- Result: the `wm.command` object (status, command, active_pane_id, pane_count, optional data) plus `agents_list` {active, pinned, focused, docked, filter, selected, row_count, scale_pct} or `demo` {running, endpoints, spawned}.
- `agents.demo {"stop":true}` needs no write gate (D-01): it never touches configuration and works with `[api] allow_config_write = false`; it only closes demo-spawned panes (51-03).

## Gates

- `cargo test -q --test phase51_api_parity`: 12 passed.
- `cargo test -q --test api_conformance --test v110_v120_v130_automation_api_mcp_playbooks --test mcp_interactive_navigation --test api_mcp_resources --test api_error_paths --test phase50_1_api`: all ok.
- `cargo clippy -q --all-targets -- -D warnings`: clean.
- `scripts/verify.sh` (full): `PASS 80/80 70s`.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock tests/api_conformance.rs`: clean. `rpc.rs` diff: 4 additions, 0 removals. `rpc_agents.rs`: 166 lines.

## Deviations from Plan

**1. [Rule 3 - Blocking] `tests/phase50_1_api.rs` also pins the tool count.** `tools_list_has_eleven_tools_with_find_last` asserted `tools.len() == 11` and `find` last. Updated to 13 and `tools[10]` is `bilterm_find` (file outside `files_modified`; required to keep the suite green).

**2. [Test adaptation] `agents_demo_start_stop` does not assert `demo.running == true` after start.** 51-03 (demo bookkeeping) is not in this base, so `running` stays false; the test pins the object shape and the parity with `wm.command`. Once 51-03 merges, `running` becomes true after `agents.demo {}` without any change here.

Other pre-existing 51-05/51-03 stubs (no-op `pin`, etc.) do not matter: every parity test compares the named path to the colon path.

## Known Stubs

None in this plan.

## Threat Flags

None. T-51-25 mitigated (closed op set, charset, caps, no leading colon; 19 bad payloads plus two injection payloads tested with no state change), T-51-26 unchanged capability (stop only closes demo-spawned panes, 51-03), T-51-27 caps, T-51-28 summaries hold flags, counts and ids only, T-51-SC no dependency change.

## Self-Check: PASSED

`src/api/rpc_agents.rs`, `tests/phase51_api_parity.rs` exist; commit `39c7f3a` is an ancestor of HEAD.
