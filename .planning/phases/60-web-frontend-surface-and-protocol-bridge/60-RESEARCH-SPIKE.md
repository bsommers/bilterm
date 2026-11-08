# Phase 60 Research Spike: Multi-Surface Web Frontend Surface & `@agent-manager/protocol` Bridge

**Date:** 2026-11-07  
**Status:** Complete  
**Requirements Covered:** `WEB-FE-01` (RFC 6455 WS Protocol Bridge), `WEB-FE-02` (View Model & REST Adapters), `WEB-FE-03` (Multi-Client View State Isolation), `WEB-FE-04` (Static SPA Confinement & Surface Parity)

---

## 1. Executive Summary

Bilterm provides a resilient systems engine for PTY management, VT emulation, 3270 OIA/Turbo Vision rendering, and automation control via JSON-RPC 2.0 and MCP. In parallel, `/home/bill/src/ui/agent-manager-fe` provides a modern React + Vite + Tailwind web dashboard utilizing `@xterm/xterm` for terminal visualization and `@agent-manager/protocol` v1 for agent and session lifecycle management.

Prior to Phase 60, Bilterm exposed only raw line-delimited JSON-RPC over Unix/TCP sockets, an SSE endpoint on `/events` that steals events from other consumers, and lacked browser-accessible WebSocket (RFC 6455) streaming, web view model projection, and static SPA hosting.

This research spike investigates and establishes the exact protocol contract, architectural boundaries, and implementation strategy to turn Bilterm into a first-class backend for `agent-manager-fe` and any web client without breaking existing TUI/GUI guarantees (`ARCH-R01..04`, `SEC-R01..04`, `UNI-R01`).

---

## 2. `@agent-manager/protocol` v1 Contract Analysis

The canonical contract defined in `/home/bill/src/ui/agent-manager-fe/packages/protocol/src/` (`@agent-manager/protocol` v1, `CONTRACT_VERSION = 1`) defines the wire schema across WebSockets and HTTP.

### 2.1 WebSocket Protocol (`GET /ws`)

#### A. Handshake & Lifecycle
1. **Client Handshake (`hello`)**:
   ```typescript
   {
     type: 'hello';
     contract: 1;
     token?: string;
     epoch?: string;
     resumeFrom?: Record<string, number>; // agentId -> lastSeq
     client?: { name: string; version: string };
   }
   ```
2. **Server Handshake Acknowledgment (`hello_ack`)**:
   ```typescript
   {
     type: 'hello_ack';
     contract: 1;
     epoch: string;
     server: { name: string; version: string };
     auth: { required: boolean };
     limits: {
       maxFrameBytes: number;        // 1,048,576 (1 MiB)
       maxInputBytes: number;        // 65,536 (64 KiB)
       maxBroadcastTargets: number;  // 64
     };
     backends: BackendInfo[];
   }
   ```
3. **Initial Roster Snapshot (`agent:init_list`)**:
   Immediately follows `hello_ack` (and on unauthenticated connections before `hello` if auth is not required):
   ```typescript
   {
     type: 'agent:init_list';
     agents: Agent[];
   }
   ```
4. **Heartbeat (`ping` / `pong`)**:
   Client sends `{"type": "ping", "t": <timestamp_ms>}` every 15s when idle; server replies `{"type": "pong", "t": <timestamp_ms>}`.

#### B. Client-to-Server Commands
- `agent:input`: `{ type: 'agent:input', agentId: string, data: string }`
- `agent:resize`: `{ type: 'agent:resize', agentId: string, cols: number, rows: number }`
- `agent:create`: `{ type: 'agent:create', name: string, mode: AgentMode, targetHost: string, initialCmd?: string, backend?: string, params?: Record<string, string|number|boolean>, reqId?: string }`
- `agent:kill`: `{ type: 'agent:kill', agentId: string, killSession?: boolean, confirm?: boolean, reqId?: string }`
- `agent:update`: `{ type: 'agent:update', agentId: string, name?: string, tags?: string[], group?: string, reqId?: string }`
- `agent:broadcast`: `{ type: 'agent:broadcast', targetAgentIds: string[], data: string, reqId?: string }`
- `agent:history`: `{ type: 'agent:history', agentId: string, beforeSeq?: number, afterSeq?: number, limit?: number, reqId?: string }`
- `agent:approve`: `{ type: 'agent:approve', agentId: string, gateId: string, decision: 'approve'|'deny', reqId?: string }`

#### C. Server-to-Client Delta Events
- `agent:added`: `{ type: 'agent:added', agent: Agent }`
- `agent:removed`: `{ type: 'agent:removed', agentId: string, reason?: 'killed'|'exited'|'gone'|'backend_down' }`
- `agent:updated`: `{ type: 'agent:updated', agentId: string, patch: Partial<Agent> }`
- `agent:status_change`: `{ type: 'agent:status_change', agentId: string, status: AgentStatus, reason?: string, exitCode?: number }`
- `agent:output`: `{ type: 'agent:output', agentId: string, data: string, seq?: number, replay?: boolean }`
- `agent:gap`: `{ type: 'agent:gap', agentId: string, fromSeq: number, toSeq: number }`
- `agent:history_chunk`: `{ type: 'agent:history_chunk', agentId: string, chunks: HistoryChunk[], hasMore: boolean, reqId?: string }`
- `agent:ack`: `{ type: 'agent:ack', reqId: string, op: AckOp, ok: true, agent?: Agent, results?: BroadcastResult[] }`
- `agent:error`: `{ type: 'agent:error', reqId?: string, agentId?: string, code: ErrorCode, message: string, retryable: boolean, detail?: unknown }`
- `agent:meta`: `{ type: 'agent:meta', agentId: string, scope: MetaScope, data: ... }`
- `agent:gate`: `{ type: 'agent:gate', agentId: string, gateId: string, prompt: string, expiresAt?: number }`

#### D. Sequencing & Replay Mechanics
- Every `agent:output` chunk receives a strictly monotonic 1-based `seq: u64` per pane.
- Bilterm retains a per-pane ring buffer (64 KiB – 256 KiB) of chunk records: `{ seq: u64, ts: u64, data: String }`.
- When a client reconnects with `resumeFrom[agentId] = last_seen_seq`:
  - If `last_seen_seq` is within retained history, replay missing chunks with `replay: true`.
  - If `last_seen_seq` is older than the oldest retained chunk, emit `agent:gap { fromSeq: last_seen_seq + 1, toSeq: oldest_seq - 1 }` followed by replaying retained chunks.
  - Streaming UTF-8 boundary integrity is guaranteed so multi-byte UTF-8 code points are never sliced across frame boundaries.

---

### 2.2 REST Endpoints Expected by Web Frontends

In addition to the WebSocket endpoint, `agent-manager-fe` relies on REST endpoints under `/api/*` (and legacy root aliases):

| Endpoint | Method | Request / Query | Response Body | Bilterm Source |
|---|---|---|---|---|
| `/health` / `/api/health` | `GET` | - | `{ "ok": true, "contract": 1, "epoch": "<epoch_id>", "backends": [{"id": "bilterm", "up": true}], "agents": <count> }` | Process uptime + `panes.len()` |
| `/api/agents` / `/agents` | `GET` | - | `{ "ok": true, "agents": Agent[] }` | `WorkspaceViewModelDto::agents` |
| `/api/agents/:id` / `/agents/:id` | `GET` | - | `{ "ok": true, "agent": Agent }` | Pane lookup by ID/name |
| `/api/agents/:id/logs` / `/logs/:id` | `GET` | `beforeSeq?`, `afterSeq?`, `limit?` (default 500, max 2000) | `{ "ok": true, "lines": LogLine[], "hasMore": bool }` | Per-pane output ring buffer converted to lines |
| `/api/search` / `/search` | `POST` / `GET` | `{ "q": string, "caseSensitive"?: bool, "limit"?: number }` | `{ "ok": true, "results": SearchHit[], "nextCursor"?: string, "truncated"?: bool }` | `find_results_json` / line search |
| `/tokens.css` | `GET` | - | `text/css` (`:root { --color-canvas: #...; ... }`) | Phase 45 `DesignTokens::for_profile()` |
| `/rpc` | `POST` | JSON-RPC 2.0 (`workspace.view_model`) | JSON-RPC 2.0 Response (`result: WorkspaceViewModelDto`) | `src/api/rpc_dispatch.rs` |

---

### 2.3 Static SPA Hosting & Security Confinement (`SEC-R01`)

To allow Bilterm to serve the built `agent-manager-fe` single-page application directly without requiring Node.js or a separate HTTP server in production:
1. **CLI & Config Flag**: `--web-dir <path>` (e.g. `--web-dir /home/bill/src/ui/agent-manager-fe/dist`).
2. **Directory Traversal Defense (`SEC-R01`)**:
   - Resolve canonical path via `std::fs::canonicalize(web_dir)`.
   - For every incoming request `GET /path/to/file`:
     - Clean relative segments (`.` and `..`).
     - Reject NUL bytes (`\0`) and invalid UTF-8 sequences.
     - Canonicalize candidate path and verify `candidate.starts_with(&canonical_web_dir)`.
3. **SPA `index.html` Fallback**:
   - If requested path does not map to an existing file and does not have a static asset file extension (e.g. `/agents`, `/settings`, `/workspace`), serve `index.html` with `200 OK` and `Content-Type: text/html`.
4. **CORS Configuration for Local Vite Development**:
   - CLI flag `--cors-origin <origin>` (e.g. `--cors-origin http://localhost:5173`).
   - When configured, emit:
     - `Access-Control-Allow-Origin: <origin>`
     - `Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS`
     - `Access-Control-Allow-Headers: Content-Type, Authorization, Idempotency-Key`
   - Handle `OPTIONS` preflight requests cleanly with `204 No Content`.

---

## 3. Data Mapping: Bilterm Core to `@agent-manager/protocol`

| Protocol Field | Bilterm Native Origin | Mapping Semantics |
|---|---|---|
| `Agent.id` | `PaneId` (`usize`) / Alias | Projected as string (e.g. `"pane-1"`, `"agent-builder"`). |
| `Agent.name` | `pane.custom_title` or `agent_mail` name | Human-readable pane label or agent identity. |
| `Agent.mode` | `pane.remote_target` (`ssh`, `tmux`, local) | Returns `'tmux' \| 'ssh'` or custom mode. |
| `Agent.targetHost` | `pane.remote_target.host` | Hostname or `"localhost"` for local PTYs. |
| `Agent.status` | `AgentState` (`Working`, `Blocked`, `Idle`, `Done`, `Errored`) | `Working` -> `running`, `Idle` -> `idle`, `Blocked` -> `thinking`, `Errored` -> `error`. |
| `Agent.unreadOutput`| `pane.has_unseen_output` / notification flag | Boolean indicator. |
| `Agent.layout` | `FloatingWindowGeometry` or BSP leaf `Rect` | Projected `{ isFloating, x, y, width, height, zIndex }`. |
| `Agent.capabilities`| Pane features | `['input', 'resize', 'kill', 'history', 'broadcast']`. |
| `Agent.createdAt` | `pane.created_at_ms` | Timestamp in epoch milliseconds. |
| `Agent.lastActivity`| `pane.last_output_ms` | Timestamp in epoch milliseconds. |

---

## 4. Architectural Findings & Key Seams

1. **RFC 6455 Handshake & Framing Without Heavy Dependencies**:
   - The WebSocket handshake requires computing:
     `Sec-WebSocket-Accept = Base64(SHA-1(Sec-WebSocket-Key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))`.
   - `crate::vt::codec::base64_encode` already exists in Bilterm.
   - A pure, self-contained, 45-line RFC 3174 SHA-1 digest implementation in `src/api/ws.rs` avoids adding unvetted external dependencies, preserving compile speed and security.
   - Text framing (RFC 6455 Section 5.2) with unmasked server-to-client frames and client-to-server 4-byte XOR unmasking is straightforward, robust, and cleanly testable in isolation.
2. **Headless PTY Output Pump**:
   - In headless / daemon mode, `WindowManager::poll_all_ptys()` must be regularly driven by the HTTP / WebSocket server loop so PTY output is ingested and pushed to connected web clients without waiting for local TUI draw ticks.
3. **Multi-Client State Decoupling (`ARCH-R02`)**:
   - The local TUI/GUI operator's focused pane and viewport must remain untouched when a remote web client focuses a different pane or scrolls back through history.
   - Solved cleanly by introducing `ClientViewState` and `ClientViewRegistry` in `src/api/client_view.rs`.

---

## 5. Risk Assessment & Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| **Buffer Exhaustion on High PTY Output** | High | Cap per-pane output ring buffer at 256 KiB; emit `agent:gap` if a slow client falls behind. |
| **Directory Traversal via Static Server** | Critical | Strict canonical path verification under `SEC-R01`; reject any path resolving outside `--web-dir`. |
| **WebSocket Memory Leak** | Medium | Bounded channels (`mpsc::channel(256)`); drop disconnected client senders on write failure. |
| **State Mutation Races** | Medium | All write operations lower to canonical `Action` and dispatch via `WindowManager::apply` through the existing plane queue. |
