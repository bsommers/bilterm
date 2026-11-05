# Seed: Scripting language

**Status**: Backlog seed (not scheduled, no phase assigned)
**Depends on**: Phase 49 (Command Parity & Full Public API) as the foundation. Phase 49 should precede any scripting work.

## Idea

A scripting language for Bilterm: automation, layout recipes, agent orchestration.

## Candidate approach

A thin layer over the existing `:` command / RPC vocabulary (every command already lowers to `Action` via `WindowManager::apply`). A script runs the same verbs the user types or an agent calls over RPC/MCP.

## Constraints

- Do NOT create a second command vocabulary. Anything a script can do must be an existing `:` command / RPC method; new capability goes into the command set first (Phase 49 parity), then becomes scriptable for free.
- Respect existing gating (`[api]` permissions, exit guard, sanitised names, `secure_fs` for any script storage).

## Before any design

Run `/gsd-spec-phase` first to settle the purpose (automation vs layout recipes vs agent orchestration) before choosing syntax. The purpose decides whether this is a command-list format, a small expression language, or an embedded interpreter.

## Open questions

- Which of the three purposes comes first?
- Where do scripts live (config dir, per-workspace, per-session)?
- Error model: abort, continue, or transactional with undo (see Phase 52 undo-safe semantics)?
