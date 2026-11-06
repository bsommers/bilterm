# Phase 52 Frozen Contract: Agent-to-Agent Messaging & Coordination (AGT-01..03, UNI-R01)

Authored for Phase 52 of Bilterm under `AGENTS.md` conventions (`ARCH-R01` unified Action dispatch, `ARCH-R02` encapsulated state on `WindowManager`, `ARCH-R04` modular source files <= 800-1000 lines, `UNI-R01` surface uniformity, `SEC-R01..04` private filesystem operations via `secure_fs`).

Every Phase 52 plan reads this contract. Plans 52-01, 52-02, and 52-03 build against these frozen definitions.

---

## 1. Architectural Decisions

- **D-01 Isolated In-Memory Mailboxes**: Each named agent or pane possesses an ordered, bounded mailbox storing structured `AgentMessage`s in memory on `WindowManager`. Messages survive pane redraws, window repositioning, and layout switches.
- **D-02 Unified Action Lowering (`ARCH-R01`)**: All user interactions (keyboard, `:agent` colon commands, palette, RPC, HTTP, MCP, `bilterm ctl`) lower to `Action::AgentMail(AgentMailOp)` executed through `WindowManager::apply(&mut self, action: Action) -> Result<AppAction>`.
- **D-03 Stable Naming & Addressing**: Panes are addressed either by stable alias (`:agent name <name>`, env `BILTERM_AGENT`) or by pane id (`#<id>`). Registered names are validated using single-component safe identifier checks (alphanumeric, `-`, `_`, length 1..64).
- **D-04 Strict Safety & Opt-In Policy (`SEC-R01..04`)**:
  - Gated off by default behind `[api] allow_agent_messaging = false`. Can be enabled in config or dynamically via `:agent enable on` / `wm.set_agent_messaging_enabled(true)`.
  - Attempting to send or broadcast while disabled returns an immediate explicit error: `"Agent messaging is disabled (enable via :agent enable on or [api] allow_agent_messaging = true)"`.
  - Unknown recipient returns explicit `Err("Unknown agent '<to>'")`; silent drops are forbidden.
  - Payloads are size-capped (`MAX_BODY_BYTES = 8192`), mailboxes are queue-capped (`MAX_INBOX_DEPTH = 64`), and senders are rate-limited (`MAX_MSGS_PER_WINDOW = 32`).
  - Loop guard: recursive `reply_to` message chains increment `hop_count`. If `hop_count > MAX_HOPS` (8), delivery is rejected with `Err("Agent message loop guard triggered (hop_count > 8)")`.
- **D-05 Delivery Modes (`AgentDeliveryMode`)**:
  - `Mailbox`: Message is appended to the recipient's inbox and increments unread badge.
  - `Nudge`: Message is appended to the inbox AND, **only if** the recipient pane is idle/waiting, NOT in `!ECHO` password lock (`!pane.is_password_prompt()`), and NOT in alternate screen (`!pane.is_alternate_screen()`), a sanitized one-line notification (`# [mail from <from> #<id>]: <short_preview>\n`) is injected into the pane PTY. If the pane is busy, locked, or in alt screen, the injection is refused and the message is safely stored in the mailbox with status `"queued in mailbox (nudge suppressed: pane busy/locked)"`.
  - `File`: The message body is written atomically with `0600` permissions under a private runtime directory (`agent-mail/<to>/msg-<id>.txt`) using `secure_fs`. The file path is recorded in `AgentMessage.file_path`, and if idle, the nudge references the file path.
- **D-06 Unread Telemetry & Event Bus Integration**:
  - Unread mail counts appear as badges (`✉ N` or `[M:N]`) in the Navigator drawer and the `AGENTS` window (`AgentsListRow`).
  - Delivery emits `"agent.message"` events on the automation event bus (`/events`, `events.poll`).
  - `agent.wait` allows synchronous blocking on incoming replies or acks with a timeout.

---

## 2. Message Schema & Enums

### 2.1 `AgentMessage`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentMessage {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub body: String,
    pub kind: String,
    pub reply_to: Option<u64>,
    pub hop_count: u8,
    pub timestamp_ms: u64,
    pub acked: bool,
    pub file_path: Option<String>,
}
```

### 2.2 `AgentDeliveryMode`
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDeliveryMode {
    #[default]
    Mailbox,
    Nudge,
    File,
}
```

### 2.3 `AgentMailOp`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentMailOp {
    SetName {
        pane_id: Option<usize>,
        name: String,
    },
    Send {
        from: Option<String>,
        to: String,
        body: String,
        kind: String,
        delivery: AgentDeliveryMode,
        reply_to: Option<u64>,
    },
    Broadcast {
        from: Option<String>,
        subtree: Option<String>,
        body: String,
        kind: String,
    },
    Inbox {
        agent: Option<String>,
        unread_only: bool,
    },
    Ack {
        agent: Option<String>,
        message_id: Option<u64>,
    },
    List,
    Enable(Option<bool>),
}
```

---

## 3. Limits & Constants

```rust
pub const MAX_BODY_BYTES: usize = 8192;
pub const MAX_INBOX_DEPTH: usize = 64;
pub const MAX_HOPS: u8 = 8;
pub const MAX_MSGS_PER_WINDOW: usize = 32;
pub const RATE_WINDOW_SECS: u64 = 60;
pub const NUDGE_PREVIEW_CAP: usize = 60;
```

---

## 4. Colon Command Grammar

- `:agent name <name>`: Register name for the active pane.
- `:agent name <pane_id> <name>`: Register name for the specified pane.
- `:agent send <to> <body...>`: Send message to `<to>` with default `mailbox` delivery.
- `:agent send --nudge <to> <body...>`: Send with `nudge` delivery.
- `:agent send --file <to> <body...>`: Send with `file` delivery.
- `:agent broadcast <body...>`: Broadcast message to all registered agents.
- `:agent broadcast --tree <subtree> <body...>`: Broadcast to agents in a specific subtree.
- `:agent inbox`: View active pane / agent inbox.
- `:agent inbox <name>`: View inbox for agent `<name>`.
- `:agent ack <id>`: Acknowledge message `<id>`.
- `:agent ack all`: Acknowledge all messages in active agent's inbox.
- `:agent ack <name> <id>`: Acknowledge message `<id>` for agent `<name>`.
- `:agent list`: List registered agents and unread counts.
- `:agent enable on`: Enable agent messaging.
- `:agent enable off`: Disable agent messaging.
- `:agent enable`: Query whether agent messaging is enabled.

---

## 5. JSON-RPC & MCP Surface Parity

### 5.1 JSON-RPC Methods
- `agent.name`: `{"pane_id"?: number, "name": string}` -> `{"status": "ok", "pane_id": number, "name": string}`
- `agent.send`: `{"from"?: string, "to": string, "body": string, "kind"?: string, "delivery"?: "mailbox"|"nudge"|"file", "reply_to"?: number}` -> `{"status": "sent"|"queued", "message_id": number, ...}`
- `agent.broadcast`: `{"from"?: string, "subtree"?: string, "body": string, "kind"?: string}` -> `{"status": "broadcast", "recipients": [string], "count": number}`
- `agent.inbox`: `{"agent"?: string, "unread_only"?: boolean}` -> `{"agent": string, "messages": [AgentMessage], "total": number, "unread": number}`
- `agent.ack`: `{"agent"?: string, "message_id"?: number}` -> `{"status": "acked", "count": number}`
- `agent.list`: `{}` -> `{"enabled": boolean, "agents": [{"name": string, "pane_id": number, "unread": number, "state": string}]}`
- `agent.wait`: `{"agent": string, "timeout_ms"?: number, "reply_to"?: number}` -> `{"status": "received"|"timeout", "message"?: AgentMessage}`

### 5.2 MCP Tool
- `bilterm_agent_mail`: Unified MCP tool taking action parameter (`"send"`, `"broadcast"`, `"inbox"`, `"ack"`, `"list"`, `"name"`, `"wait"`) with corresponding arguments.

### 5.3 MCP Resource
- `bilterm://agent/{name}/inbox`: Dynamic resource returning current unread/read messages formatted as JSON.

---

## 6. Verification Requirements

- Minimum 16 tests in `tests/phase52_agent_messaging.rs`.
- Two-agent round trip.
- Unknown recipient error.
- Gating off by default -> error; gating on -> succeeds.
- Loop guard (`MAX_HOPS = 8`).
- Rate limiting and inbox depth cap (`MAX_INBOX_DEPTH = 64`).
- Nudge refusal during `ALT_SCREEN` and `!ECHO` lock.
- Private file creation (`0600`) via `secure_fs`.
- Subtree broadcast.
- MCP and RPC parity.
- Unread badge rendering in `AGENTS` window and Navigator.
