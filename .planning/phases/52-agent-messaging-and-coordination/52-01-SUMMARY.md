# Plan 52-01 Summary: Specification, Contract, and Safety Foundations

## Accomplishments
- Authored `52-CONTRACT.md` detailing the frozen specification for Phase 52 Agent-to-Agent Messaging & Coordination (`AGT-01..03`).
- Defined `AgentMessage`, `AgentDeliveryMode`, and `AgentMailOp` structures.
- Established addressing rules (`:agent name <n>`, `BILTERM_AGENT` env, `#<pane_id>`).
- Specified delivery modes (`Mailbox`, `Nudge`, `File`) with safety invariants (no nudges during `ALT_SCREEN` or `!ECHO` password lock; atomic `0600` files).
- Formalized safety controls: `[api] allow_agent_messaging` default `false`, loop guard (`MAX_HOPS = 8`), rate limiter (`MAX_MSGS_PER_WINDOW = 32`), queue depth (`MAX_INBOX_DEPTH = 64`), and payload cap (`MAX_BODY_BYTES = 8192`).
- Defined surface parity across `:agent` commands, `Action::AgentMail`, RPC methods (`agent.*`), MCP tools (`bilterm_agent_mail`), MCP resources (`bilterm://agent/<name>/inbox`), and unread badges.
