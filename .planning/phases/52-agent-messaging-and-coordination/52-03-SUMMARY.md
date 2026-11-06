# Summary 52-03: Surface Parity, API/MCP, Unread Badges & Verification Gate

## Completed
1. **Surface Parity**:
   - `Action::AgentMail(AgentMailOp)` wired in `src/action.rs` with `colon_form_or_summary`, `colon_for`, and verified in `every_action_is_reachable_from_keymap_colon_or_rpc`.
   - `:agent` colon commands parsed (`parse_agent_mail.rs`) and executed (`execute_agent_mail.rs`) for verbs: `name`, `send`, `broadcast`, `inbox`, `ack`, `list`, `enable`.
   - Tab completion wired in `src/pane/completion.rs` for `:agent` verbs and flags (`--nudge`, `--file`, `--unread`, `--tree`).
   - Palette & command registry wired in `src/command_registry.rs` and `src/ui/help_catalog.rs`.

2. **JSON-RPC & MCP Integration**:
   - 7 RPC methods implemented in `src/api/rpc.rs`: `agent.name`, `agent.send`, `agent.broadcast`, `agent.inbox`, `agent.ack`, `agent.list`, `agent.wait`.
   - MCP tool `bilterm_agent_mail` registered and schema-defined in `src/api/mcp.rs`.
   - MCP dynamic resource `bilterm://agent/<name>/inbox` implemented in `src/api/mcp_resources.rs` (supporting list and read).
   - Automation event `AutomationEvent::AgentMessage` emitted on message dispatch.

3. **UI Unread Badges**:
   - `AgentsListRow` enriched with `unread_mail: usize`.
   - `AGENTS` window (`src/ui/agents_list_ui.rs`) renders `✉N` badge in amber/yellow when `unread_mail > 0`.
   - Navigator drawer rows (`src/wm/drawer.rs`) render `✉ {unread_mail}` in mode badges when `unread_mail > 0`.

4. **Integration Test Suite**:
   - `tests/phase52_agent_messaging.rs`: 17 integration tests covering two-agent round-trip, unknown recipient error, opt-in gating, loop guard (hop count > 8), queue depth cap (64), sender rate limit (32/min), oversized body rejection, nudge refusal on alternate screen / password lock, file delivery with `0600` permissions, broadcast delivery, MCP tool/resource parity, drawer/agents unread badges, and `agent.wait` timeout/reply.

5. **Verification**:
   - `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
   - `scripts/verify.sh`: `PASS 109/109 64s`.
