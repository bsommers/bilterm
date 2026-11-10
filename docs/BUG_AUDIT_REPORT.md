# Comprehensive Audit of Recent Bugs, Regressions, and Bugfixes in Bilterm

**Audit Target:** Phases 55–64+, Daemon Supervisor & PID 0 issues, `:ssh-tmux` remote pane bugs, Input/Keybinding regressions, and CI/Cross-platform build breaks.  
**Git History Reviewed:** Commits `a833fdf` through `7aaa11b` (including merges #31, #33, #34, #35, #40 and branches `fix/daemon-complete`, `fix/ssh-tmux-complete`, `feat/phase-61..64`).

---

## Executive Summary

Recent feature development across Phases 55 through 64 expanded Bilterm into persistent SSH/tmux remote multiplexing, daemon supervision, dynamic traffic monitoring, multi-mode windowing, and rich clipboard/browser canvases. While velocity was high, this phase boundary surfaced critical regressions:
1. **Remote `:ssh-tmux` Panes:** Spawned redundant local shells, doubled keystrokes/commands, deadlocked or dropped input on idle session attach, and failed prefill if modals/copy mode were active.
2. **Daemon Supervisor & PID 0:** Headless daemons failed to lock/register, `ensure_started` synthesized `PID 0` entries that were unsignalable, `stop_daemon` could kill unrelated processes on PID reuse or target process group 0, and runtime directory resolution drifted across execution contexts.
3. **Input & Keybinding Regressions:** Key-repeat toggled modals uncontrollably, the since-retired Meta+M menu chord collided with macOS compose characters and Claude Code chords, compose/AltGr was unconditionally rewritten into `ESC <letter>`, focus clicks inadvertently cycled prompt chrome, and zoomed pane state diverged from window focus.
4. **CI & Cross-Platform Breaks:** Test harnesses and clipboard helpers broke on macOS and Windows due to ungated Linux POSIX PTY types and X11 MIME symbols, while hardcoded tool-count constants broke tests when new capabilities landed.

---

## 1. Bugs Introduced by Poor Specification

These defects occurred because requirements, ADRs, or phase plans were underspecified, failed to document protocol states, or lacked explicit edge-case definitions.

### 1.1 Remote-Only Panes vs Local Shell PTYs (`:ssh-tmux`)
* **Commits:** `e48bd27` (red), `c098821` (green), [`src/wm/tmux_remote.rs`](file:///home/bill/src/ai/bilterm/src/wm/tmux_remote.rs), [`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs)
* **Underlying Gap in Spec:**
  Phase 51.1 specified that `:ssh-tmux` attaches to a remote `tmux -CC` session over SSH. However, the specification never defined the **execution model of a remote pane**. It implicitly assumed a pane is always backed by a local PTY (`portable_pty`). When `split_focused_remote_pane` was called in live TUI/GUI mode, it invoked standard pane spawning, launching `/bin/bash` locally while forwarding keystrokes over the SSH transport.
* **Consequence:** Keystrokes were written to both the local shell and the remote SSH link. Commands submitted via the floating prompt executed both remotely in tmux and locally on the user's host machine!

### 1.2 Idle Remote Session Attach Protocol (`tmux -CC`)
* **Commits:** `c098821`, [`src/tmux/list_panes.rs`](file:///home/bill/src/ai/bilterm/src/tmux/list_panes.rs), [`src/wm/tmux_link.rs`](file:///home/bill/src/ai/bilterm/src/wm/tmux_link.rs), [`src/wm/tmux_io.rs`](file:///home/bill/src/ai/bilterm/src/wm/tmux_io.rs)
* **Underlying Gap in Spec:**
  The `tmux -CC` integration spec assumed that upon attaching (`tmux -CC new-session -A -s <name>`), tmux always emits `%layout-change` followed by `%output`. However, when attaching to an **idle session** (no running process producing output, layout unchanged), tmux sends only the handshake greeting (`%begin ... %end`).
  Because the spec did not define the idle handshake lifecycle, Bilterm waited indefinitely for `%layout-change` to map the primary pane. Unmapped primary panes rejected all user input with `"Link down"` errors.
* **Specification Fix Needed:** Define attach protocol state machine with explicit fallbacks: if no layout change is received within attach greeting, provisionally bind primary pane to `%0` and issue `list-panes -s -F "#{window_id} #{pane_id}"` (`ReplyKind::ListPanes`) to reconcile actual pane IDs.

### 1.3 Ignored Configuration in Classic Drawer (`FIX-01`)
* **Commits:** `d4d3f29`, `e8ab1fd`
* **Underlying Gap in Spec:**
  `:drawer rows 2` was added in an earlier phase, but the specification was ambiguous regarding whether it applied to all drawer styles or only the `Navigator` style. The `Classic` drawer renderer silently ignored the `row_lines` setting, leaving users with 1-line truncated rows while reporting success.

### 1.4 Ambiguous Zoom Toggle Semantics Under Shuffling (`FIX-02`)
* **Commits:** `4e92499`, [`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs)
* **Underlying Gap in Spec:**
  The spec defined "Window -> Zoom" as toggling `zoomed_pane: Option<PaneId>`. It failed to specify the invariant when the active window changes while zoomed (e.g. via deck shuffle, drawer selection, or Alt+Tab). Because `zoomed_pane` stored a static `PaneId`, shifting focus left the old pane zoomed on screen while input went to the newly focused hidden pane. When the user attempted to un-zoom via the menu, `toggle_zoom()` saw `zoomed_pane != active_pane` and re-zoomed the new active pane rather than un-zooming.
* **Specification Fix Needed:** Zoom is not an identity pointer; it is a **display mode** (`effective_zoomed = if is_zoomed { z_stack.top() } else { None }`).

---

## 2. Bugs Preventable by Better Skills or Agent Practices

These defects arose not from algorithmic impossibility, but from lapses in engineering discipline, testing habits, or platform awareness by implementing agents.

### 2.1 Lack of TDD / Test-First Discipline on Complex Protocols
* **Evidence:** In `feat/phases-51.1`, the agent implemented SSH-tmux integration directly against happy-path mocks. It wasn't until commit `e48bd27` (`test(ssh-tmux): red-phase TDD regression suite`) that an agent wrote 6 explicit failure cases—**5 of which immediately failed on `main`**:
  1. `connect_ssh_tmux_never_spawns_local_pty_even_when_not_headless` (FAILED: local bash PTY existed)
  2. `no_double_characters_on_keystrokes_or_prompt_submission` (FAILED: double write)
  3. `idle_remote_tmux_session_maps_pane_hydrates_and_delivers_commands_on_same_tick` (FAILED: unmapped pane)
  4. `list_panes_reconciles_non_zero_first_pane_id_and_multi_pane_without_local_pty` (FAILED: no list-panes sent)
  5. `prefill_works_from_normal_raw_copy_linkraw_and_modal_states` (FAILED: failed when modals open)
* **Agent Practice Failure:** Had the `test-driven-development` skill been strictly followed during Phase 51.1 (writing the harness tests before touching [`src/wm/tmux_remote.rs`](file:///home/bill/src/ai/bilterm/src/wm/tmux_remote.rs)), none of these five regressions would have entered `main`.

### 2.2 Incomplete Cross-Platform Awareness & Leaky `#[cfg]` Annotations
* **Commits:**
  - `069a883`: `fix(tests): gate PtyConfig/PtySession helper with cfg(target_os = linux) in phase64_traffic_hooks`
  - `2f069bf`: `fix(clipboard): gate classify_mime_targets with unix non-macos cfg for cross-platform clippy`
  - `0d9839b`: `fix(verify): make scripts/verify.sh run under macOS /bin/bash 3.2`
  - `7b5cb76`: `fix(release): make build-release-assets.sh Linux-only and build the tag (#24)`
* **Agent Practice Failure:**
  Agents repeatedly wrote helper code or test harnesses assuming Linux is the only target:
  - In `phase64_traffic_hooks.rs`, an agent instantiated `PtyConfig { program: "sh" }` using a helper only available on Linux, breaking macOS/Windows builds.
  - In `src/clipboard.rs`, X11-specific MIME classification was compiled for macOS targets, causing unused code and platform compilation failures.
  - In `scripts/verify.sh`, an agent used modern Bash 4+ syntax, breaking on macOS where default `/bin/bash` is version 3.2.
* **Remedy:** Agents must verify conditional compilation using `cargo check --target aarch64-apple-darwin` and `cargo check --target x86_64-pc-windows-msvc` (or cross-platform clippy) before committing.

### 2.3 Fragile Hardcoded Magic Numbers in Global Test Suites
* **Commit:** `eefddf1` (`fix(64-03): clippy and tool-count test constants`)
* **Files:** [`tests/api_error_paths.rs`](file:///home/bill/src/ai/bilterm/tests/api_error_paths.rs), [`tests/phase51_api_parity.rs`](file:///home/bill/src/ai/bilterm/tests/phase51_api_parity.rs), [`tests/phase51_agents_list_cmd.rs`](file:///home/bill/src/ai/bilterm/tests/phase51_agents_list_cmd.rs)
* **Agent Practice Failure:**
  Tests asserted hardcoded constants like `assert_eq!(tools.len(), 24)` and `assert_eq!(appended[cat], 11)`. Whenever Phase 63 or Phase 64 added a new tool (e.g. `bilterm_keys`, `bilterm_traffic`), existing tests in unrelated suites broke.
* **Remedy:** Assert semantic properties (e.g. `assert!(tools.iter().any(|t| t.name == "bilterm_find"))`) rather than fixed global counts.

### 2.4 Gaps in Surface Uniformity (UNI-R01) Verification
* **Commits:** `3297e1e`, `50a020b` (`feat(daemon-ui): floating DAEMON SUPERVISOR manager window (TUI+GUI), Daemon menu fixes`)
* **Agent Practice Failure:**
  When Phase 55 delivered daemon management, it implemented colon commands (`:daemon list|start|stop`) and RPC/MCP, but neglected the interactive floating window and menu dropdown hooks. When the menu was clicked, nothing rendered because dropdowns were skipped when `TopBarKind::Pills` was active.
  The feature was declared "complete" despite violating UNI-R01 (mouse + key + menu + colon parity).

---

## 3. Bugs Preventable by Architectural & Design Patterns

These bugs were symptoms of structural weaknesses: improper state lifecycles, missing type invariants, or tight coupling across abstraction boundaries.

### 3.1 Primitive Obsession: Untyped PIDs & Unsafe Signal Dispatch
* **Commits:** `a833fdf` (PR #19), `d857966`
* **Files:** [`src/daemon/supervisor.rs`](file:///home/bill/src/ai/bilterm/src/daemon/supervisor.rs), [`src/daemon/registry.rs`](file:///home/bill/src/ai/bilterm/src/daemon/registry.rs)
* **Architectural Flaw:**
  PIDs were stored as raw integers (`i64` in JSON, cast to `u32` or `pid_t`).
  - Casting `-1` or `0` to `pid_t` in Unix `libc::kill(pid, sig)` signals **all processes in the current process group** (`pid == 0`) or **all processes the caller has permission to signal** (`pid == -1`).
  - If a daemon died and its PID was recycled by the OS, `stop_daemon` would read the stale lockfile and kill an innocent third-party process.
* **Architectural Remedy Applied:**
  1. Introduction of `parse_daemon_pid(raw: i64) -> Option<u32>` enforcing `1 < pid <= i32::MAX`.
  2. **Non-blocking flock check:** Before signaling, `stop_daemon` calls `claim_unheld_lock(&lock_path)`. If the lock can be acquired, the daemon is dead; the lockfile is purged without signaling.

### 3.2 Split Brain: Lockfile vs Socket vs Registry Triplication (PID 0 Defect)
* **Commits:** `d857966`, `50a020b`
* **Files:** [`src/main.rs`](file:///home/bill/src/ai/bilterm/src/main.rs), [`src/daemon/registry.rs`](file:///home/bill/src/ai/bilterm/src/daemon/registry.rs), [`src/daemon/supervisor.rs`](file:///home/bill/src/ai/bilterm/src/daemon/supervisor.rs)
* **Architectural Flaw:**
  The daemon architecture maintained three separate filesystem representations of a daemon:
  1. An exclusive lockfile (`daemon-<name>.lock`) containing a JSON record with PID.
  2. A Unix domain socket (`daemon-<name>.sock`).
  3. A registry entry (`daemons/<name>.json`).
  
  When `bilterm --daemon` spawned, `src/main.rs` never called `acquire_lock` or `register_current_daemon`. As a result:
  - Sockets existed on disk without lockfiles or registry entries.
  - When `DaemonRegistry::list_active` crawled the directory, it synthesized fallback records for bare sockets with `pid: 0`.
  - The UI displayed daemons with `PID 0`, and any attempt to stop them failed because PID 0 was rejected as unsignalable.
* **Design Pattern Recommendation:**
  Adopt a single-source-of-truth lock-and-register lifecycle guard (`RegistrationGuard`). The daemon cannot bind its socket until it holds the lock, and the lock record *is* the registry.

### 3.3 Conflation of Local vs Remote PTY I/O Channels
* **Commits:** `c098821`, [`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/wm/tmux_remote.rs`](file:///home/bill/src/ai/bilterm/src/wm/tmux_remote.rs)
* **Architectural Flaw:**
  `CompositePane` conflated "owning a terminal canvas" with "owning a local OS process PTY". In `CompositePane::write_to_pty`, the code unconditionally invoked `self.pty.write_all(bytes)`. When remote input tapping was introduced, it tapped the input *in addition* to writing to the PTY, causing character duplication.
* **Design Pattern Recommendation:**
  Refactor `CompositePane` to use a polymorphic or enum-based backend:
  ```rust
  pub enum PaneTransport {
      LocalPty(PtySession),
      RemoteBridge { link_id: LinkId, pane_id: usize },
      Synthetic(VirtualBuffer),
  }
  ```
  A pane backed by `RemoteBridge` does not allocate an OS PTY file descriptor at all.

### 3.4 Conflating Window Focus Transition with Content Interaction
* **Commit:** `7e0fac9` (`fix(61-03)`)
* **Files:** [`src/input/mouse.rs`](file:///home/bill/src/ai/bilterm/src/input/mouse.rs), [`src/wm/z_stack.rs`](file:///home/bill/src/ai/bilterm/src/wm/z_stack.rs), [`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs)
* **Architectural Flaw:**
  In `handle_wm_mouse_event`:
  1. Clicking an unfocused window called `focus_and_raise(hit)`.
  2. But the event was not consumed by the focus change—it fell through to `handle_pane_mouse_event`.
  3. If the user clicked near the top of an unfocused window to activate it, the click simultaneously hit the OIA prompt button and toggled prompt mode (`Floating` -> `Fixed`)!
  4. Furthermore, `z_stack.raise()` called `clear_inactive_transient_copy_cursors()`, destroying the active Vi copy-mode selection of any pane losing focus.
* **Design Pattern Recommendation:**
  **Two-tier event consumption**: A click that changes window focus (`was_active == false`) must be consumed by the window manager as a focus event, never leaking down to pane chrome actions unless explicitly configured. Pane selection/copy states must be owned by the pane and preserved across focus shifts.

### 3.5 Global Premature Key Normalization Breaking Compose & IME
* **Commits:** `56a0f38`, `c08be03`, [`src/input/normalize.rs`](file:///home/bill/src/ai/bilterm/src/input/normalize.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs)
* **Architectural Flaw:**
  `normalize_macos_option_key` was executed unconditionally in `src/input/mod.rs:67` before routing to raw PTY passthrough. On Linux and Windows, characters produced by AltGr or compose keys (`ø`, `ß`, `å`, `ç`) were intercepted, converted to `Alt+<letter>`, and transmitted to child programs as `ESC <letter>`, making non-US keyboard layouts unusable.
* **Design Pattern Recommendation:**
  Input must pass through a strict layered filter:
  1. Host OS text composition / IME (yields printable UTF-8 characters).
  2. WM Leader & Modal Key Router (Window mode, Which-Key).
  3. Global WM Hotkeys (only unambiguous modifiers: `Primary` = `Cmd` on macOS, `Ctrl+Shift` on Linux/Win).
  4. Foreground Terminal Stream (PTY / Remote Link).

---

## Actionable Recommendations & Guardrails

| Area | Concrete Recommendation | Enforcing Mechanism |
| :--- | :--- | :--- |
| **Specifications** | **Protocol State Diagrams in Plans:** Any phase touching network/IPC protocols (`tmux -CC`, daemons, RPC) must document all connection states (idle, connecting, reattaching, error) in the phase plan before code is written. | GSD `gsd-plan-phase` requirement gate |
| **Agent Practices** | **Mandatory Red-Phase TDD for Regressions:** Any bugfix task must check in or prove a failing regression test (like `tests/ssh_tmux_tdd_regression.rs`) before applying fixes. | `test-driven-development` skill adherence |
| **Cross-Platform** | **Cross-Target Clippy in Local Verification:** Expand `scripts/verify.sh` to execute `cargo check --target aarch64-apple-darwin` to catch OS-gated symbols before pushing. | `scripts/verify.sh` pre-commit gate |
| **Architecture** | **Newtype Domain Validation:** Replace raw integer PIDs with `ValidatedPid` / `SignalablePid` newtypes that cannot represent `0`, negative numbers, or `1`. | Compile-time type safety (`src/daemon/types.rs`) |
| **Architecture** | **Pane Transport Decoupling:** Separate `CompositePane` rendering canvas from OS PTY allocation via an explicit `PaneTransport` enum to prevent accidental local process execution in remote panes. | Refactor `src/pane/mod.rs` |
| **UI Event Model** | **Focus Isolation:** Enforce that focus-raising mouse clicks are swallowed by the WM and never dispatch to intra-pane chrome controls. | `src/input/mouse.rs` invariant test |
