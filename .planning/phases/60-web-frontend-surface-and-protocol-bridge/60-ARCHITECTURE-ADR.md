# Phase 60 Architecture Decision Record (ADR): Multi-Surface Web Frontend Backend & `@agent-manager/protocol` Bridge

**Date:** 2026-11-07  
**Status:** Approved  
**Context:** Phase 60 (`WEB-FE-01..04`)  
**Deciders:** Core Engineering Team, AGY Agentic Compiler Engine

---

## 1. Context and Problem Statement

Bilterm is designed as a multi-surface terminal multiplexer and automation platform. While JSON-RPC 2.0 (Unix socket / TCP) and Model Context Protocol (stdio MCP) serve backend daemons and AI coding tools, web-based control surfaces such as `/home/bill/src/ui/agent-manager-fe` require:
1. Low-latency, full-duplex WebSocket communication (RFC 6455) for interactive PTY streaming and lifecycle events.
2. Direct conformance with the wire contract defined by `@agent-manager/protocol` v1 (`CONTRACT_VERSION = 1`).
3. An atomic, single-flight view model projection (`WorkspaceViewModelDto`) containing the complete layout tree, active agents, utility windows, and theme design tokens.
4. Client view state isolation (`ARCH-R02`), preventing multiple remote browser tabs from clobbering each other's or the local TUI/GUI operator's focused pane and viewport.
5. Production-grade static asset hosting under strict filesystem confinement (`SEC-R01`), serving the bundled SPA frontend alongside the API.

How should Bilterm architect this web surface without introducing massive third-party web frameworks, violating module size limits (`ARCH-R04`), or creating conflicts during parallel multi-agent development?

---

## 2. Decision Drivers

- **Modularity & Size Ceilings (`ARCH-R04`)**: Every new module under `src/api/` must stay well below 1,000 lines (target ~200–350 lines).
- **Encapsulated Workspace Invariants (`ARCH-R02`)**: External client operations must never bypass `WindowManager::apply(Action)` or mutate private WM state directly.
- **Surface Uniformity (`UNI-R01`)**: Every capability exposed over WebSockets/HTTP must also be accessible via TUI/GUI keys, colon commands (`:web`), and MCP tools (`bilterm_web_surface`).
- **Filesystem Security Confinement (`SEC-R01`)**: Static file serving must prevent path traversal attacks (`..`), symlink escapes, and unauthorized directory indexing.
- **Zero Heavy Web Framework Dependencies**: Maintain instant build times and hermetic execution without pulling in bloated dependencies like `actix-web`, `axum`, or `tungstenite`.
- **Parallel Multi-Agent Autonomy**: Decouple the Rust backend work in `bilterm` from the TypeScript frontend work in `agent-manager-fe` so two AI agents (AGY and Claude Code) can build in parallel without file collisions.

---

## 3. Architecture & Module Decomposition

The Web Frontend Backend is organized into four cohesive submodules under `src/api/`, integrated cleanly into the existing HTTP server (`src/api/http.rs`):

```
                                  +-----------------------------+
                                  | Browser / Vite Dev Server   |
                                  | (agent-manager-fe)          |
                                  +--------------+--------------+
                                                 |
                       HTTP / REST / Static      |  WebSocket (/ws)
                                                 v
+---------------------------------------------------------------------------------------+
| bilterm HTTP Server (src/api/http.rs)                                                 |
|                                                                                       |
|  +---------------------------+        +--------------------------+                    |
|  | Static & SPA Server       |        | RFC 6455 WS Bridge       |                    |
|  | (src/api/static_serve.rs) |        | (src/api/ws.rs)          |                    |
|  | - SEC-R01 Path Guard      |        | - SHA-1 Upgrade Handshake|                    |
|  | - SPA index.html Fallback |        | - Unmask / Text Framing  |                    |
|  | - CORS Allowlist          |        | - Monotonic seq + Replay |                    |
|  +-------------+-------------+        +-------------+------------+                    |
|                |                                    |                                 |
|                +------------------+                 |                                 |
|                                   v                 v                                 |
|                        +---------------------------------------+                      |
|                        | View Model & REST Adapters            |                      |
|                        | (src/api/view_model.rs)               |                      |
|                        | - WorkspaceViewModelDto               |                      |
|                        | - /tokens.css (DesignTokens Export)   |                      |
|                        | - REST: /agents, /logs, /search       |                      |
|                        +------------------+--------------------+                      |
|                                           |                                           |
|                                           v                                           |
|                        +---------------------------------------+                      |
|                        | Multi-Client View Isolation           |                      |
|                        | (src/api/client_view.rs)              |                      |
|                        | - ClientViewState & Registry          |                      |
|                        | - Isolated Viewports & Focus          |                      |
|                        +------------------+--------------------+                      |
+-------------------------------------------|-------------------------------------------+
                                            v
                         +---------------------------------------+
                         | AutomationControlPlane &              |
                         | WindowManager (src/api/rpc.rs)        |
                         | - Action::* Dispatch                  |
                         | - poll_all_ptys Headless Pump         |
                         +---------------------------------------+
```

### 3.1 `src/api/ws.rs` (~350 lines): RFC 6455 WebSocket Bridge
- **Handshake Upgrade**:
  - Intercepts HTTP `GET /ws` requests with `Upgrade: websocket` and `Connection: Upgrade`.
  - Calculates RFC 6455 `Sec-WebSocket-Accept`:
    $$\text{Accept} = \text{Base64}\Big(\text{SHA-1}\big(\text{Sec-WebSocket-Key} + \text{"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"}\big)\Big)$$
  - Emits HTTP `101 Switching Protocols` and transitions TCP stream into full-duplex binary/text framing.
- **Framing & Zero-Copy Protocol Dispatch**:
  - Decodes client text frames, applying 4-byte masking unmasking inline.
  - Implements ping/pong frames (`0x9` / `0xA`) and clean close negotiation (`0x8`).
- **Sequencing & Replay (`WEB-FE-01`)**:
  - Maintains per-pane 1-based monotonic counter (`seq: u64`).
  - Holds an in-memory ring buffer of recent chunks (`64 KiB – 256 KiB`) per pane.
  - Replays chunks upon `resumeFrom` reconnection; emits `agent:gap` when client gap exceeds retained window.
- **Message Dispatch**:
  - Translates `@agent-manager/protocol` client messages (`agent:input`, `agent:resize`, `agent:create`, `agent:kill`, `agent:broadcast`) into canonical `Action` commands queued to `AutomationControlPlane`.
  - Broadcasts server delta events (`agent:added`, `agent:removed`, `agent:output`, `agent:status_change`) to all active subscriber sockets.

### 3.2 `src/api/view_model.rs` (~300 lines): View Model Projection & REST Adapters
- **`WorkspaceViewModelDto`**:
  - Aggregates the complete layout tree:
    - `SplitTree`: Recursive BSP layout nodes with split orientation and proportions.
    - `ZStack`: Floating window rectangles, z-index ordering, and zoomed pane state.
    - `Agents`: Array of active agents mapped directly to `@agent-manager/protocol` `Agent` structs.
    - `Navigator`: Hierarchical session, window, and pane tree.
    - `UtilityWindows`: State of Script Studio, Session Map, Clipboard Viewer, and File Browser.
    - `DesignTokens`: Theme color tokens serialized as hex strings.
- **`to_css_variables()` Method (`WEB-FE-02`)**:
  - Formats Phase 45 `DesignTokens` into CSS custom properties:
    ```css
    :root {
      --color-canvas: #090d16;
      --color-surface: #0f172a;
      --color-surface-raised: #1e293b;
      --color-overlay: #334155;
      --color-border-idle: #334155;
      --color-border-focus: #38bdf8;
      --color-text-primary: #f1f5f9;
      --color-text-muted: #94a3b8;
      --color-accent: #38bdf8;
      --color-accent-on: #0f172a;
      --color-success: #4ade80;
      --color-warning: #facc15;
      --color-danger: #f87171;
      --color-scrim: rgba(0, 0, 0, 0.55);
    }
    ```
  - Served directly at `GET /tokens.css` with `Content-Type: text/css`.
- **REST Adapters**:
  - Implements lightweight handlers for `GET /api/agents`, `GET /api/agents/:id`, `GET /api/agents/:id/logs`, and `POST /api/search` reusing existing `find_results_json` logic.

### 3.3 `src/api/client_view.rs` (~200 lines): Client Viewport & Focus Isolation
- **`ClientViewState` (`WEB-FE-03`)**:
  - Tracks individual web client state:
    - `client_id: String` (assigned upon WebSocket handshake or session cookie).
    - `viewport: Rect` (dimensions of the browser's terminal container).
    - `focused_pane_id: Option<PaneId>` (pane focused within this specific web tab).
    - `scroll_offsets: HashMap<PaneId, usize>` (client-specific scrollback positions).
    - `active_overlays: Vec<String>` (modals or drawers toggled by this client).
- **`ClientViewRegistry`**:
  - Thread-safe store (`Arc<RwLock<HashMap<String, ClientViewState>>>`).
  - Ensures a browser user selecting a pane or scrolling history never shifts focus on the local TUI screen or inside other connected browser sessions (`ARCH-R02`).

### 3.4 `src/api/static_serve.rs` (~250 lines): Static SPA Server & CORS Confinement
- **`SEC-R01` Filesystem Confinement (`WEB-FE-04`)**:
  - Canonicalizes `--web-dir` root at startup.
  - Validates all request paths: canonicalizes the target and asserts `target.starts_with(&root)`.
  - Rejects path traversal (`..`), NUL bytes, and invalid encodings with `403 Forbidden` / `404 Not Found`.
- **SPA Fallback Routing**:
  - If a path lacks an extension and does not exist on disk, serves `index.html` with `200 OK` to support client-side React routing.
- **CORS Allowlist**:
  - When `--cors-origin` is provided, attaches `Access-Control-Allow-Origin`, methods, and headers to responses, and handles `OPTIONS` preflights.
- **Privacy Masking**:
  - Hooks into `crate::pane::redact::classify` to scrub sensitive credentials before streaming output when requested.

---

## 4. Parallel Multi-Agent Execution Contract

To enable simultaneous execution by AGY (`bilterm` backend) and Claude Code (`agent-manager-fe` frontend):

```
+-----------------------------------------------------------------------------------------+
| PARALLEL EXECUTION BOUNDARY                                                             |
+----------------------------------------------------+------------------------------------+
| bilterm (Rust Backend)                             | agent-manager-fe (TS Frontend)     |
| Owner: AGY Compiler Engine                         | Owner: Claude Code                 |
| Workspace: /home/bill/src/ai/bilterm               | Workspace: /home/bill/src/ui/...   |
+----------------------------------------------------+------------------------------------+
| Responsibilities:                                  | Responsibilities:                  |
| - Implement RFC 6455 WS upgrade in `src/api/ws.rs` | - Connect to `ws://localhost:8088/ws`|
| - Emit `@agent-manager/protocol` v1 frames         | - Render active agents from WS list|
| - Implement `GET /tokens.css` from `DesignTokens`  | - Load dynamic styles via /tokens.css|
| - Implement static SPA serving with `SEC-R01`      | - Test build output via `npm run build`|
| - Maintain `<1,000` LOC per file constraint        | - Retain `@agent-manager/protocol` |
+----------------------------------------------------+------------------------------------+
| Shared Golden Contract:                                                                 |
| - Wire schema frozen in `packages/protocol/src/`                                       |
| - Integration tests run against loopback `127.0.0.1:8088`                               |
| - Zero shared code repositories; communicate strictly over wire protocol              |
+-----------------------------------------------------------------------------------------+
```

---

## 5. Surface Uniformity (`UNI-R01`)

Web server management is fully accessible across all Bilterm surfaces:
1. **Keys**: Chords declared in `src/input/keymap_tables.rs`.
2. **Colon Command**: `:web [status|open|port|stop]` in `src/colon_command/parse_web.rs` and `src/colon_command/execute_web.rs`.
3. **Command Palette**: `Web: Open Web UI`, `Web: Server Status` in fuzzy palette.
4. **Top Menu**: `Tools -> Web UI...`.
5. **JSON-RPC**: `web.status`, `web.open`, `workspace.view_model`.
6. **MCP**: `bilterm_web_surface` in `src/api/mcp.rs`.

---

## 6. Consequences & Invariants

### Positive
- Fully conforms to `@agent-manager/protocol` v1 without running a Node.js gateway.
- Preserves `ARCH-R04` with four modular files under 350 lines each.
- Guarantees `ARCH-R02` state encapsulation via `ClientViewRegistry`.
- Enables single-binary distribution serving both TUI/GUI and web UI.

### Neutral
- In-memory ring buffer consumes ~256 KiB RAM per active pane (bounded and configurable).
- SHA-1 implementation in Rust adds ~45 lines to `src/api/ws.rs` without any external crate.
