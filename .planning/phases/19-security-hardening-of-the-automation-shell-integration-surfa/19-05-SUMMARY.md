---
phase: 19-security-hardening-of-the-automation-shell-integration-surfa
plan: 05
subsystem: snapshot-and-fleet
tags: [rust, security, snapshot-confinement, ppm, playbook, fleet-register, remote-add]

requires: [19-02, 19-03, 19-04]
provides:
  - "snapshot_dir configuration on AutomationControlPlane with default_snapshot_dir_from resolving XDG/HOME/runtime"
  - "Strict validate_snapshot_name rejecting path traversal, slashes, backslashes, empty, dot, and dotdot"
  - "write_snapshot_confined opening with O_NOFOLLOW in canonical snapshot directory"
  - "write_snapshot_trusted for operator CLI playbook execution"
  - "SnapshotPolicy enum (Trusted vs Confined) differentiating CLI playbook runs from RPC/MCP runs"
  - "run_mcp_stdio_server_with_plane allowing external plane dependency injection"
  - "validate_destination enforcement on fleet.register RPC method and :remote add colon command"
affects: [19-06, 19-07]

tech-stack:
  added: []
  patterns: ["Path canonicalization and O_NOFOLLOW file confinement", "Dual-policy playbook snapshot export"]

key-files:
  created: []
  modified: [src/api/mod.rs, src/pane/colon_command.rs]

key-decisions:
  - "workspace.snapshot ppm_path must be a single file name confined to the snapshot directory"
  - "Direct RPC/MCP workspace.snapshot and playbook.run requests use SnapshotPolicy::Confined"
  - "CLI --playbook executions retain unconfined SnapshotPolicy::Trusted behavior"
  - "SSH destination validation enforced at both RPC fleet.register and interactive :remote add"

patterns-established:
  - "All client-submitted file export requests strictly require leaf file names and enforce parent directory canonicalization"

requirements-completed: [SEC-R02, SEC-R04]

duration: 12min
completed: 2026-10-23
status: complete
---

# Phase 19 Plan 05: Snapshot Confinement & Remote Registration Hardening Summary

**Confined `workspace.snapshot` `ppm_path` for RPC/MCP clients to a designated snapshot directory with path traversal and symlink guards (SEC-R02), decoupled trusted CLI playbooks from confined RPC playbooks, and validated SSH destinations across RPC and interactive `:remote add` (SEC-R04).**

## Performance & Verification

- **Tests added:** 10 comprehensive unit and integration tests in `src/api/mod.rs` and `src/pane/colon_command.rs`:
  - `snapshot_name_validation`
  - `snapshot_confined_rejects_traversal_and_writes_nothing`
  - `snapshot_confined_refuses_planted_symlink`
  - `snapshot_confined_writes_inside_dir`
  - `snapshot_without_ppm_path_is_status_only`
  - `default_snapshot_dir_from_chain`
  - `mcp_snapshot_path_is_confined`
  - `rpc_playbook_export_is_confined`
  - `fleet_register_rejects_option_destination`
  - `fleet_register_accepts_valid_destination`
  - `remote_add_rejects_option_destination`
- **Suite status:** 138 unit tests + all integration tests passing cleanly; `cargo clippy --all-targets -- -D warnings` 0 warnings.
