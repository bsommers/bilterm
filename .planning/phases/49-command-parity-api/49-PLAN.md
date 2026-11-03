---
phase: 49-command-parity-api
type: plan
status: IN PROGRESS (49-01, 49-02, 49-03, 49-05, 49-06 done 2026-11-02; 49-04 and 49-07 pending; API docs packet with agy)
requirements: [API-01]
---
# Phase 49: Command parity and a full public API

Goal: every user-visible capability is (1) a `:colon` command, (2) lowered to one
`Action` through `WindowManager::apply`, (3) reachable from MCP, and (4) exposed by a
documented, versioned API. NOTHING is removed; Legacy behaviour is byte-identical.

## Audit (2026-11-02, v2.6.1)

Layers today: keys/mouse/colon -> `Action` (115 variants) -> `WindowManager::apply`.
RPC (`src/api/rpc.rs`, JSON-RPC 2.0 over Unix socket / `--listen 127.0.0.1:7370`) has
~20 methods; MCP (`src/api/mcp.rs`) has 10 tools. `bilterm_colon_command` / `wm.command`
already run ANY command the parser accepts, so MCP reaches everything that is a colon command.

Gaps:
1. **Actions with no colon form** (key-only): CrosshairCycle, FocalRadiusResize,
   LayoutCycle, PromptMoveVertical, PromptResizeHeight, PromptResizeWidth,
   SearchPromptPrefill. Not reachable from MCP at all.
2. **Input handlers that mutate state without an Action** (found by grep in `src/input/`):
   toggle_help, toggle_zoom, toggle_minimize_pane, toggle_maximize_modal,
   toggle_quick_terminal, set_layout_mode, set_drawer_visibility/layout/edge, toggle_drawer,
   resize_drawer, move_selected_mcp_item, toggle_freeze_selected_mcp_node, set_modal_rect,
   menubar category open, exit-guard state. Each is invisible to colon/MCP/replay.
   (Some may already be reached via an Action too; 49-02 classifies each.)
3. **Config is not a command**: ~97 `BiltermConfig` fields; only some have `:` setters, none
   have `get`. Session-only commands (`:appearance :mode :panel :chrome`) cannot be persisted.
4. **No query surface**: colon commands return only a status string. MCP/RPC cannot read
   back appearance, mode, drawer state, config values, keymap, or the command catalog.
5. **Three duplicated command lists** (parser, `COLON_COMMANDS` completion,
   `PALETTE_CATALOG`) drift apart (caused the Ctrl+X palette bug in v2.6.0).
6. **MCP is thin**: 10 tools, stringly typed, no resources, no per-command schema, errors are
   all `-32601`. RPC has no versioning, capabilities handshake, or auth story documented
   beyond socket permissions.

## Work breakdown (serial; Claude owns Action/apply wiring per gate G3)

- **49-01 Audit** DONE (this file).
- **49-02 Colon parity.** Add colon forms for the 7 key-only Actions
  (`:crosshair cycle`, `:focal radius +4`, `:layout cycle`, `:prompt move up|down`,
  `:prompt height +1`, `:prompt width +4`, `:search <text>`); route each input-handler
  mutation in gap 2 through a new or existing `Action`. Gate test: every `Action` variant
  has a colon string AND every keymap entry's Action round-trips parse(format(action)).
- **49-03 Single command registry.** One table (name, aliases, args schema, description,
  Action builder, palette sample) generating parser dispatch, Tab completion, Ctrl+X palette
  and help. Existing drift tests become trivially true. Legacy command names unchanged.
- **49-04 Query commands.** `:get <key>`, `:set <key> <value>`, `:config list`,
  `:state` (appearance, mode, drawer, layout, focus), `:commands` returns the catalog.
  Commands return a structured `CommandOutcome {status, message, data: JsonValue}`
  instead of only a string. Opt-in persistence: `:appearance save`, `:config save`.
- **49-05 MCP completeness.** One MCP tool per registry command group generated from the
  registry (typed JSON schema), plus MCP resources (`bilterm://state`, `.../config`,
  `.../commands`, `.../pane/<id>`), plus granular errors (invalid params -32602, not found).
  Keep the 10 existing tools unchanged. Test: iterate the registry, call each via MCP.
- **49-06 Public API spec.** Versioned (`api.version`, `api.capabilities`), documented
  endpoint set in `docs/API.md` + machine-readable `docs/api/openapi.json` /
  JSON-RPC schema: workspace, pane, session, config, appearance, drawer, fleet, events,
  commands. Optional loopback HTTP+SSE transport mirroring JSON-RPC (events stream),
  token auth for non-Unix-socket transports, `secure_fs` for token file (0600).
  Decision needed before building the HTTP transport (see Open questions).
- **49-07 Conformance + docs.** Contract test suite that drives every endpoint over
  Unix socket, TCP and MCP; `bilterm ctl <method>` covers all endpoints; update
  AGENTS.md / developer guide / CHANGELOG; release.

## Rules
- Legacy parity: new commands are additive; existing syntax and output unchanged.
- No file over ~1000 lines; `handle_wm_key_event <= 80`; runtime files via `secure_fs`.
- `scripts/verify.sh` PASS and a parity test per step; update PLAN/ROADMAP/STATE/CHECKPOINT.
- Outward-facing: network listeners stay loopback-only and opt-in; no new default ports.

## Open questions for the user
1. Add an HTTP+SSE transport (new dependency, loopback + token), or keep JSON-RPC over
   Unix socket/TCP + MCP only?
2. Should `:set`/`:config save` be allowed over MCP/RPC by default, or behind a
   `[api] allow_config_write = true` switch (recommended: switch, default off)?
3. Session-only appearance commands: persist automatically, or only on `:appearance save`?

## Progress log (2026-11-02)
- 49-02 DONE: 7 key-only Actions now have colon forms (`:crosshair cycle`, `:layout cycle`, `:focal radius`, `:prompt move|resize-height|resize-width`, `:search prefill`). Uncovered input mutations: minimize-pane by mouse, MCP-console freeze/move (need Actions); modal/drawer drag geometry stays mouse-only.
- 49-03 DONE: `src/command_registry.rs` = one catalog (name, subargs, summary, alias_of, examples, shortcut) joined from completion/palette/help + EXTRA; 5 drift tests; `bilterm://commands` reads it. The parser stays hand-written (Legacy parity).
- 49-05 DONE: MCP resources (state/config/commands/pane) and -32602/-32601/-32603 codes.
- 49-06 DONE (transport): `--headless --http 127.0.0.1:PORT`, bearer token 0600, Host/Origin checks, limits. Open: `/events` shares the `events.poll` queue; token generator is Unix-only.
- Decisions (user): HTTP+SSE AND JSON-RPC both; config write gated by `[api] allow_config_write` default off; persist only on explicit `:appearance save` / `:config save` (assumed).
- Agy packet: .planning/dispatch/49-doc.md (docs/API.md) - user-run, not yet run.
