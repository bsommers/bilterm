---
phase: 20-mega-module-decomposition-behaviour-preserving
plan: 04
subsystem: api
tags: [refactor, decomposition, api, json-rpc, mcp, playbook, server]
requires:
  - phase: 20-02
    provides: "top-level crate::colon_command module"
  - phase: 20-03
    provides: "decomposed crate::input module"
provides:
  - "src/api/json.rs (287 lines): JsonValue RFC 8259 AST and recursive parser with MAX_JSON_DEPTH"
  - "src/api/playbook.rs (712 lines): AutomationEvent, Playbook, PlaybookStep, PlaybookReport, feed_output_and_collect_events, run_playbook_with_policy, run_playbook, and snapshot/fleet unit tests"
  - "src/api/mcp.rs (260 lines): handle_mcp_line, mcp_tools_list, run_mcp_stdio_server_with_plane, run_mcp_stdio_server, parse_key_chord_str"
  - "src/api/rpc.rs (781 lines): AutomationControlPlane, SnapshotPolicy, validate_snapshot_name, handle_jsonrpc_line, dispatch_method, workspace_status_json"
  - "src/api/server.rs (593 lines): ListenEndpoint, ListenConfig, validate_listen_config, send_rpc_request_with_token, run_headless_server, serve_jsonrpc_tcp, serve_jsonrpc_unix"
  - "src/api/tests.rs (539 lines): JSON parser depth, daemon concurrency, listen policy, bearer token, and Unix socket unit tests"
  - "src/api/mod.rs (21 lines): thin module dispatcher re-exporting all public API symbols"
affects: [20-05]
tech-stack:
  added: []
  patterns: ["focused submodule decomposition with pub use re-exports preserving crate::api::* paths"]
key-files:
  created:
    - src/api/json.rs
    - src/api/playbook.rs
    - src/api/mcp.rs
    - src/api/rpc.rs
    - src/api/server.rs
    - src/api/tests.rs
  modified:
    - src/api/mod.rs
key-decisions:
  - "Split the 855-line unit test suite between src/api/tests.rs (539 lines: JSON parser, server/socket/auth/listen policy tests) and src/api/playbook.rs::tests (snapshot confinement, MCP/RPC playbook confinement, and fleet registration tests) so every file stays <= 800 lines."
  - "Placed parse_key_chord_str in src/api/mcp.rs (re-exported via pub use mcp::*) to keep src/api/rpc.rs at 781 lines."
patterns-established:
  - "Cohesive separation of JSON AST (json.rs), JSON-RPC control plane (rpc.rs), MCP stdio server (mcp.rs), playbook/event engine (playbook.rs), and network/Unix socket listener transport (server.rs)."
requirements-completed: [ARCH-R04]
duration: 6min
completed: 2026-10-23
---

# Phase 20 Plan 04: `api` Module Decomposition Summary

**Decomposed the 3,124-line `src/api/mod.rs` monolith into `json`, `rpc`, `mcp`, `playbook`, `server`, and `tests` submodules, all `<= 800` lines with zero behavioral changes**

## Performance

- **Duration:** 6 min
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Extracted `JsonValue` RFC 8259 parser/serializer and `MAX_JSON_DEPTH` into `src/api/json.rs` (287 lines).
- Extracted `AutomationEvent`, `Playbook`, `PlaybookStep`, `PlaybookReport`, and `AutomationControlPlane` playbook/event methods into `src/api/playbook.rs` (712 lines, including snapshot confinement and fleet unit tests).
- Extracted MCP protocol handlers (`handle_mcp_line`, `mcp_tools_list`, `run_mcp_stdio_server*`) and `parse_key_chord_str` into `src/api/mcp.rs` (260 lines).
- Extracted `AutomationControlPlane`, `SnapshotPolicy`, `validate_snapshot_name`, `handle_jsonrpc_line`, `dispatch_method`, and `workspace_status_json` into `src/api/rpc.rs` (781 lines).
- Extracted `ListenEndpoint`, `ListenConfig`, `validate_listen_config`, `send_rpc_request*`, `run_headless_server`, and TCP/Unix socket listener loops into `src/api/server.rs` (593 lines).
- Extracted server/socket/token/listen policy unit tests into `src/api/tests.rs` (539 lines) and reduced `src/api/mod.rs` to a 21-line re-export hub.

## Task Commits

1. **Task 1 & Task 2: Split `src/api/mod.rs` into `json`, `playbook`, `mcp`, `rpc`, `server`, and `tests`** - pending commit

## Verification

- `awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/api/*.rs` -> exit 0 (max file: `src/api/rpc.rs` at 781 lines)
- `cargo test --lib api::` -> 31 passed
- `cargo test --test phase19_security_cli --test v110_v120_v130_automation_api_mcp_playbooks` -> 11 passed
- `cargo clippy --all-targets -- -D warnings` -> 0 warnings
