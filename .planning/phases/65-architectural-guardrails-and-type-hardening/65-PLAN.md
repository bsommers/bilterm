---
phase: 65-architectural-guardrails-and-type-hardening
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/daemon/mod.rs
  - src/daemon/types.rs
  - src/daemon/supervisor.rs
  - src/pane/mod.rs
  - src/pane/transport.rs
  - src/input/mouse.rs
  - tests/phase65_guardrails.rs
autonomous: true
requirements:
  - GUARD-01
  - GUARD-02
  - GUARD-03
recommended_model: sonnet
must_haves:
  truths:
    - "SignalablePid newtype strictly rejects PID 0, negative PIDs, PID 1, and values > i32::MAX at construction time."
    - "No low-level signaling function (libc::kill) can accept raw integers; all call sites must supply a validated SignalablePid."
    - "PaneTransport enum explicitly decouples local PtySession from remote bridge and disconnected states in CompositePane."
    - "CompositePane cannot write to a local PTY while in Remote transport mode, and cannot tap remote input while in Local transport mode."
    - "A mouse click on an inactive or unfocused window/pane is consumed strictly as a focus-and-raise event, preventing click bleed-through to child PTY, canvas, or SGR mouse handlers."
  artifacts:
    - path: "src/daemon/types.rs"
      provides: "SignalablePid type, TryFrom/FromStr implementations, validation logic, and unit tests"
    - path: "src/pane/transport.rs"
      provides: "PaneTransport enum (Local, Remote, None), I/O delegation, and invariant guards"
    - path: "src/input/mouse.rs"
      provides: "Focus isolation logic for inactive window mouse events"
    - path: "tests/phase65_guardrails.rs"
      provides: "Integration and regression tests for GUARD-01, GUARD-02, and GUARD-03"
---

# Phase 65 Plan: Architectural Guardrails & Type-Safe Domain Hardening (GUARD-01..03)

## Objective

Fortify critical architectural boundaries and eliminate class-wide bug categories via type-safe domain modeling and event isolation:
1. Establish a `SignalablePid` newtype that makes sending signals (`kill(2)`) to `PID 0`, negative PIDs, `PID 1`, or invalid integers impossible at compile time and runtime (`GUARD-01`).
2. Decouple `CompositePane` transport state into an explicit `PaneTransport` enum (`Local(PtySession)`, `Remote(RemotePaneBridge)`, `None`), eliminating ad-hoc boolean flags and multi-field synchronization risks (`GUARD-02`).
3. Enforce focus isolation in `src/input/mouse.rs` such that mouse clicks on an unfocused/inactive window or pane are consumed exclusively as a focus-and-raise event without bleeding through to the underlying terminal canvas, child PTY, selection drag, or SGR mouse protocol (`GUARD-03`).

---

## Detailed Requirements

### GUARD-01: `SignalablePid` Domain Newtype
- **Problem**: Raw integers (`i32`, `u32`, `i64`) are currently used to represent PIDs across process supervisors, lockfile parsers, and cleanup routines. In POSIX `kill(pid, sig)`:
  - `pid == 0` signals all processes in the caller's process group.
  - `pid == -1` broadcasts the signal to every process the caller has permission to send signals to (system-wide blast).
  - `pid < -1` signals the entire process group `|pid|`.
  - `pid == 1` targets `init` / `systemd`.
  Accidental underflow, sentinel `-1`, or uninitialized `0` passing into `libc::kill` risks crashing the entire user session or daemon group.
- **Specification**:
  - Implement `SignalablePid` wrapping a validated positive integer: `1 < pid <= i32::MAX as u32`.
  - Provide safe constructors:
    - `SignalablePid::new(pid: u32) -> Result<Self, InvalidPidError>`
    - `SignalablePid::from_raw(pid: i64) -> Result<Self, InvalidPidError>`
    - `TryFrom<i32>`, `TryFrom<u32>`, `TryFrom<i64>`
  - Expose accessors:
    - `.as_raw() -> u32`
    - `.as_pid_t() -> libc::pid_t`
  - Re-route all `libc::kill` invocations (in `src/daemon/supervisor.rs` and other process management sites) to require `SignalablePid`.
  - Replace `parse_daemon_pid(raw: i64) -> Option<u32>` with `parse_daemon_pid(raw: i64) -> Option<SignalablePid>`.

### GUARD-02: Remote Pane Transport Decoupling (`PaneTransport`)
- **Problem**: `CompositePane` currently models transport with loosely coupled fields:
  - `pty: Option<PtySession>`
  - `remote_input_tap: bool`
  - `remote_input: Vec<u8>`
  - `link_banner: Option<LinkBanner>`
  This permits contradictory states (e.g. `remote_input_tap == true` with an active `pty`, or writing to a closed PTY). Byte delivery logic branches with mutable conditionals inside `write_to_pty`.
- **Specification**:
  - Define `PaneTransport` enum:
    ```rust
    pub enum PaneTransport {
        Local(PtySession),
        Remote(RemotePaneBridge),
        None,
    }
    ```
    where `RemotePaneBridge` encapsulates `remote_input` buffer, `link_banner`, and tap metrics.
  - Encapsulate transport actions behind unified methods on `PaneTransport`:
    - `write_bytes(&mut self, bytes: &[u8]) -> io::Result<()>`
    - `resize(&mut self, cols: u16, rows: u16) -> io::Result<()>`
    - `is_remote(&self) -> bool`
    - `is_local(&self) -> bool`
  - Refactor `CompositePane` to hold `transport: PaneTransport`, eliminating independent boolean flags.

### GUARD-03: Focus Isolation in `src/input/mouse.rs`
- **Problem**: In `src/input/mouse.rs`, clicking an inactive window/pane raises and focuses the pane via `wm.focus_and_raise(hit_id)`, but subsequent checks only swallow clicks on the pane status bar or prompt chrome row. Clicks on the canvas fall through to `route()` and `handle_pane_mouse_event()`. This causes:
  - Inadvertent selection drags in the newly focused window.
  - Unintended SGR mouse click reports sent to full-screen applications (`vim`, `htop`, `less`, `fzf`) running in the inactive window.
  - Accidental command execution or cursor repositioning in the prompt.
- **Specification**:
  - When `!was_active` on a `MouseEventKind::Down(MouseButton::Left | MouseButton::Right)`, the click MUST be consumed exclusively for window focus/raise.
  - Return `Ok(AppAction::Handled)` immediately after `wm.focus_and_raise(hit_id)` and resizing/updating z-order, preventing event propagation to canvas/selection/SGR handlers.
  - Window border manipulations (e.g., clicking close `[X]`, zoom `[□]`, minimize `[_]`, or frame drag borders) continue to be handled by window management controls.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-PID-01** | Denial of Service / Process Termination | Malformed or negative lockfile PID (`-1`) causes `libc::kill(-1, sig)`, sending signals to all processes in the user's session. | `SignalablePid` rejects all values `<= 1`, preventing system-wide or process-group signal broadcast. |
| **T-PID-02** | System Instability | Calling `kill(1, sig)` targets `init` / `PID 1`. | `SignalablePid` requires `pid > 1`, completely eliminating init process signaling. |
| **T-PID-03** | Integer Overflow / Type Confusion | Unchecked cast of `i64` or `u32` exceeding `i32::MAX` to `pid_t` results in negative PID due to sign extension. | `SignalablePid` enforces `pid <= i32::MAX as u32` before permitting conversion to `libc::pid_t`. |
| **T-TRN-01** | Information Disclosure / Misdirected I/O | Shell commands or credentials meant for a remote SSH/tmux session are mistakenly written to a local PTY due to desynchronized `remote_input_tap` flag. | `PaneTransport` sum type ensures mutual exclusivity: a pane is either `Local` or `Remote`, never both. |
| **T-MOU-01** | Unintended Input Injection / Clickjacking | Clicking an unfocused pane to switch focus triggers an unintended button click, selection drag, or SGR escape sequence in a background application. | Mouse focus isolation swallows the initial click on inactive windows, ensuring it only activates the window. |

---

## Technical Design & Wave Breakdown

### Wave 1: Type-Safe PID Signaling (`GUARD-01`)
1. In `src/daemon/types.rs`:
   - `struct SignalablePid(u32);`
   - `enum DaemonError { InvalidPid(i64), ... }`
   - Implement `TryFrom<i32>`, `TryFrom<u32>`, `TryFrom<i64>`, `Display`, `Debug`, `Copy`, `Clone`, `Eq`, `PartialEq`, `Hash`.
   - Implement helper methods: `pub fn as_raw(&self) -> u32`, `pub fn as_pid_t(&self) -> libc::pid_t`.
2. Update `src/daemon/supervisor.rs`:
   - Refactor `parse_daemon_pid` to return `Option<SignalablePid>`.
   - Update `check_pid_alive(pid: u32) -> bool` and provide `check_signalable_pid_alive(pid: SignalablePid) -> bool`.
   - Update `kill_daemon_graceful(pid: SignalablePid, ...)` and direct `libc::kill` call sites.
3. Unit test coverage in `src/daemon/types.rs` covering:
   - `-1`, `-99`, `0`, `1` -> rejected with expected error variant.
   - `2`, `1000`, `i32::MAX` -> accepted.
   - `i32::MAX as i64 + 1`, `u32::MAX` -> rejected with InvalidPid.

### Wave 2: Remote Pane Transport Decoupling (`GUARD-02`)
1. Create `src/pane/transport.rs`:
   - Define `RemotePaneBridge` containing `remote_input: Vec<u8>`, `link_banner: Option<LinkBanner>`.
   - Define `PaneTransport`:
     ```rust
     pub enum PaneTransport {
         Local(PtySession),
         Remote(RemotePaneBridge),
         None,
     }
     ```
   - Implement methods:
     - `pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()>`
     - `pub fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()>`
     - `pub fn is_remote(&self) -> bool`
     - `pub fn is_local(&self) -> bool`
     - `pub fn remote_bridge_mut(&mut self) -> Option<&mut RemotePaneBridge>`
2. Refactor `CompositePane`:
   - Replace `pty: Option<PtySession>`, `remote_input_tap: bool`, `remote_input: Vec<u8>`, and `link_banner: Option<LinkBanner>` with `transport: PaneTransport`.
   - Update `write_to_pty`, `drain_remote_input`, `set_remote_input_tap`, `link_banner` accessors.
   - Ensure clean delegation without behavioral regression.

### Wave 3: Mouse Focus Isolation (`GUARD-03`) & Comprehensive Verification
1. In `src/input/mouse.rs`:
   - Identify when `hit_focus` points to an unfocused/inactive pane (`!was_active`).
   - After `wm.focus_and_raise(hit_id)`, if `mouse.kind` is `MouseEventKind::Down(_)`, consume the event immediately:
     ```rust
     if !was_active && matches!(mouse.kind, MouseEventKind::Down(_)) {
         return Ok(AppAction::Handled);
     }
     ```
   - Ensure title bar / frame controls (close, minimize, zoom, border resize) are preserved where appropriate while canvas interactions are isolated.
2. Integration & Verification:
   - Add `tests/phase65_guardrails.rs` to verify:
     - Unit tests for `SignalablePid` and invalid signal rejection.
     - `PaneTransport` state separation and I/O isolation.
     - Simulated mouse click on inactive pane verifying focus transition without SGR or selection event leakage.
   - Run `scripts/verify.sh` to confirm 100% test pass rate and clean clippy report.

---

## Verification Gates

- [ ] **Gate 1 (Unit & Invariants)**: `SignalablePid` unit tests exhaustively cover negative values, 0, 1, and overflow.
- [ ] **Gate 2 (Decoupling)**: `CompositePane` contains no loose `remote_input_tap` booleans or unbundled PTY options.
- [ ] **Gate 3 (Focus Isolation)**: Inactive-window click tests confirm zero SGR/selection/prompt bleed-through on activation.
- [ ] **Gate 4 (Regression & Quality)**: `scripts/verify.sh` passes with zero compiler warnings, zero clippy warnings, and zero test failures.
- [ ] **Gate 5 (Architecture & Modularity)**: All modified files adhere to `ARCH-R04` (`<= 1000` lines per file).
