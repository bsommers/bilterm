---
status: diagnosed
goal: find_root_cause_only
trigger: "ssh-tmux session to Mac quits early though it should be persistent; reconnects but buggy on reconnect"
created: 2026-11-05
updated: 2026-11-05
---

# ssh-tmux early quit / buggy reconnect (read-only diagnosis, no source edited)

## TL;DR

Persistence over SSH is not implemented in the production path. `:ssh-tmux` never starts ssh or tmux.
It splits a local shell pane, labels it `TMUX-SSH:<dest>/<session>` and prints
"Attached remote tmux -CC session". Nothing on the Mac is persistent unless the user ran tmux there by hand.
The real connection is whatever the user typed in that pane (plain `ssh`, no keepalives).
The `tmux -CC` bridge exists but is unreachable from the UI and is also broken on several counts.

## Verified facts (evidence)

- `:ssh-tmux` -> `ColonCommand::SshTmuxConnect` -> `WindowManager::connect_ssh_tmux`
  (src/colon_command/execute_wm.rs:358-359, src/wm/session.rs:247-281).
  - session.rs:262 `split_focused` -> src/wm/mod.rs:621-632 spawns the default local `$SHELL` (`PtyConfig::default()`).
  - session.rs:264-270 only sets the title/badge/status message; session.rs:277-279 registers a fleet node.
  - No `ssh`, no `tmux`, no `TmuxControlSubprocess` is started.
- `build_ssh_tmux_cc_args` (src/remote/mod.rs:138-160), `TmuxControlSubprocess::spawn_attach_or_new`
  (src/tmux/mod.rs:347), `from_channels` and `attach_tmux_control` (src/tmux/mod.rs:411) have no production callers.
  The only user is tests/vt_worker_and_snapshot_seam.rs:282. `poll_tmux_control` (wm/mod.rs:717) therefore never has a subprocess in real use.
- User's saved sessions (~/.local/state/bilterm/sessions/main.toml:508-513, 589-593, dual-h.toml:30-34, test270.toml:30-34):
  - Pane #12 has `shell="/bin/bash"`, `running_command="ssh <user@mac>"`, `remote_target="<user@mac>/bilterm"`,
    title `bigmac.local: gitlab` (set by the Mac shell prompt).
  - So the pattern is: `:ssh-tmux user@mac bilterm` -> local bash pane -> user types `ssh user@mac` by hand. No tmux is visible in saved scrollback (no tmux status bar string found).
  - Pane #13 scrollback shows repeated `ssh user@mac` with "Last login: ... from 192.168.1.245" at roughly 20:18:40, 20:21:17, 20:22:08, 20:23:02, 21:04:16, 07:17:32, 07:41:40.
    These are many short or repeated logins, consistent with sessions ending/being re-made often. Not conclusive on cause.
- ~/.config/bilterm/config.toml has no `[remotes.*]`. ~/.ssh has only keys, **no `~/.ssh/config`** -> no ServerAliveInterval/ControlPersist tuning from the user.
- ~/.local/state/bilterm/bilterm.log is **0 bytes** (created Oct 26): no tracing output was captured, so the log gives no information.
  Logging only happens with `RUST_LOG=bilterm=debug` (per CLAUDE.md), and nothing in the ssh/tmux path logs exit reasons anyway.
- Direct experiment (isolated socket, tmux 3.6, no tty): `tmux -CC new-session -A -s x </dev/null` prints
  `tcgetattr failed: Inappropriate ioctl for device`, rc=1. `-C` (single) works without a tty.
  Both `spawn_attach_or_new` (piped stdin, src/tmux/mod.rs:356-361, stderr -> /dev/null) and
  `build_ssh_tmux_cc_args` (`ssh -T`, src/remote/mod.rs:144, then bare `tmux -CC ...` at :157) hit this and would exit immediately with the reason swallowed.

## Ranked hypotheses

1. **(Most likely, confirmed by code + user data) The "persistent tmux" is not there.**
   `:ssh-tmux` is a local-shell facade (session.rs:247-281). The user's ssh is a plain `ssh` typed in the pane, so any drop, sleep or exit ends the remote session.
   The status text "Attached remote tmux -CC session" (session.rs:267-269, execute_wm.rs:359-362) is misleading and hides this.
   Falsify: user's Mac `tmux ls` shows no `bilterm` session, or the pane has no tmux status bar.

2. **Plain ssh drops: no keepalives anywhere.**
   `push_common_args` (src/remote/mod.rs:87-115) and `build_ssh_args` (:127) add no `ServerAliveInterval`/`ServerAliveCountMax`/`TCPKeepAlive`.
   The user's manual `ssh` has none either (no ~/.ssh/config). A Mac that sleeps, drops Wi-Fi, or has an idle-timing router kills the TCP flow.
   ssh then prints `client_loop: send disconnect: Broken pipe` / `Connection to ... closed by remote host` and exits, or freezes until the kernel notices.
   Falsify: user sees one of those messages, or `ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3` survives the same period.
   Mac-side check: `pmset -g | grep -i -E "sleep|womp|tcpkeepalive"`; `sudo pmset -a tcpkeepalive 1`, `caffeinate` test.

3. **Pane silently disappears when its PTY child exits (looks like "quit").**
   Reader thread emits `PtyEvent::Exited` on EOF/err (src/pty/mod.rs:161-175); `poll_pty_events` sets `exited` (src/pane/mod.rs:695-696);
   `poll_all_ptys` calls `remove_pane` for any exited pane while >1 panes exist (src/wm/mod.rs:697-706).
   `hold_exited_shell` (src/pane/mod.rs:748, SAFE-01 "SHELL EXITED" banner) has **no non-test callers**
   (grep: only tests/phase50_agent_state.rs, input_modal_guards_coverage.rs, v2_runtime tests), so the default `exit_protection = Guard` (src/config/mod.rs:238) never applies to exited children.
   Result: no exit code, no ssh error text, scrollback gone. If it is the last pane, `all_panes_exited` ends the whole app (src/app.rs:76).
   Falsify: reproduce with `ssh` pane killed from the Mac (`pkill -f sshd:`) and observe pane vanishes with no banner.

4. **Reconnect = session restore / detach-attach, which does not reconnect anything.**
   `:detach` (src/wm/v2_runtime.rs:209-222) saves layout TOML only; PTYs die with the process.
   `restore_session` -> `apply_saved_session_snapshot` (src/wm/session.rs:543, src/wm/layout_profile.rs:504-552) respawns `shell`/`shell_args` as a fresh local process,
   then **re-applies `remote_target`, title (`bigmac.local: gitlab`) and replays saved scrollback as live output** (layout_profile.rs:550-562).
   `running_command` is not re-run. The pane looks still connected (remote title/badge/old screen) but is a local shell.
   For real `:ssh` panes the saved `shell`/`shell_args` are the ssh argv, so restore re-ssh's, but the ControlPersist master or NAT state may be stale (hypothesis 5).
   `respawn_shell_in_place` (src/pane/mod.rs:759-780) behaves the same: new local shell for `:ssh-tmux` panes, re-ssh for `:ssh` panes, with old badge/title kept.

5. **Stale ControlMaster on reconnect (`:ssh` panes only).**
   `ControlMaster=auto` + `ControlPersist=60s` (src/remote/mod.rs:96-102) with no ServerAlive means a half-dead master stays reusable for 60s after the last client exits
   (and indefinitely while clients are attached). A reconnect inside that window multiplexes onto the dead TCP and hangs or errors
   (`mux_client_request_session: read from master failed: Broken pipe`). All panes to the same host share one master, so they die together.
   Falsify: `ls $XDG_RUNTIME_DIR/bilterm/ssh-*`, `ssh -O check -o ControlPath=...`; retry with `-o ControlMaster=no`.

6. **Latent bugs if the real `tmux -CC` bridge is wired later (and likely what the user expects):**
   a. `-CC` needs a tty on the client side. `ssh -T` (remote/mod.rs:144) and piped local stdio (tmux/mod.rs:356-361) give `tcgetattr failed`; stderr is `/dev/null` so the reason is lost. Use `ssh -tt` or `-C` (single).
   b. Remote command is bare `tmux ...` (remote/mod.rs:157). On macOS Homebrew tmux (`/opt/homebrew/bin`, `/usr/local/bin`) is not on the non-interactive sshd PATH -> `tmux: command not found`, exit 127, looks like an instant quit.
   c. No `ServerAlive*`, no reconnect loop, no backoff. EOF without `%exit` just yields `Disconnected` -> `tmux_control = None` (tmux/mod.rs:449-451), silently. `%exit [reason]` is parsed (tmux/mod.rs:141-148) but the reason is dropped and `bridge.exited` is never read.
   d. `TmuxControlBridge::new()` pre-seeds `%0 -> bilterm pane 1` (tmux/mod.rs:250-251). On reattach to an existing session whose panes are not `%0`, `%output %0` can write into the user's local pane #1, and maps are never cleared on exit/reattach (stale ids, duplicate `TMUX %N` panes).
   e. `WindowClose` (tmux/mod.rs:58) is parsed but not handled (handle_control_line :272-299) -> panes never removed when remote windows close.
   f. `take_outbound_commands` (tmux/mod.rs:333) has no caller: keystrokes, splits and `refresh-client -C` are never written to `cmd_tx`. Nothing sends `capture-pane`/`list-windows` on attach, so pre-existing screen content/cwd/scrollback is not hydrated after reattach.
   g. Remote-side tmux options to check on the Mac: `exit-empty`, `destroy-unattached`, `detach-on-destroy`, `exit-unattached` (all default off/on safely, but `~/.tmux.conf` may differ); `new-session -A -s` on a server with `exit-empty on` and no other session will die with its last window.

7. **Lower probability / ruled out:** no idle timeout, inactivity detach, or SIGHUP handling in bilterm (grep of src/daemon, src/pty, src/wm for idle/inactivity/auto_detach/SIGHUP: nothing).
   The `bilterm daemon` module is protocol/state only; panes' PTYs live in the client process (src/daemon/mod.rs header, v2_runtime.rs:209).
   Bilterm itself only handles Ctrl+C (`tokio::signal::ctrl_c`, src/app.rs:94-96); if bilterm's own terminal sends SIGHUP, all ssh children die at once.

## What to ask the user (cheapest discriminators first)

1. Exact steps/commands: what they typed after `:ssh-tmux ...`; do they run `tmux`/`tmux attach` on the Mac by hand? Does the pane show a green tmux status bar?
2. Right after the "quit": on the Mac run `tmux ls` (and `tmux -V`, `which tmux`). Does the session still exist?
3. The last lines of the pane before it ended, or whether the pane just vanished (hypothesis 3). Any `client_loop`/`Broken pipe`/`closed by remote host`/`Connection reset` text.
4. How long until it quits (always ~N minutes = idle/NAT/sleep; random = Wi-Fi/sleep). Does the Mac sleep or Wi-Fi roam? Do all ssh panes die at the same instant (shared master or sleep)?
5. What "reconnects" means: bilterm restart + `:attach`/session restore, `:respawn-pane`/Enter on exit guard, or retyping `ssh`? And what is wrong after: stale title/old screen, blank pane, hung login, duplicate panes?
6. Run with logging: `RUST_LOG=bilterm=debug bilterm 2>bilterm-debug.log` (current ~/.local/state/bilterm/bilterm.log is empty), and try `ssh -vv -o ServerAliveInterval=15 -o ServerAliveCountMax=3 user@mac 2>ssh.log` in a pane (redact hostnames/keys before sharing).
7. Mac side: `pmset -g`, `sudo log show --last 10m --predicate 'process == "sshd"'`, `~/.tmux.conf`.

## Proposed fix plan (headless-testable first)

Do these in order; each step has a regression test that needs no network or real Mac.

1. **Tell the truth in the UI (small, high value).** `connect_ssh_tmux` must not claim "Attached" until a transport exists. Either implement step 2 or change the message/title to "(local shell; run ssh/tmux manually)". Test: `WindowManager::new_headless` + `connect_ssh_tmux` -> assert pane status message does not contain "Attached" unless a control subprocess is attached.
2. **Wire the real path.** Spawn `ssh -tt [-o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes] -- dest 'exec "${SHELL:-/bin/sh}" -lc "tmux -CC new-session -A -s NAME"'` (login shell so Homebrew PATH works) inside a `CompositePane` PTY, DCS-1000p aware, or use `-C` over a pipe. Feed lines to `TmuxControlSubprocess::from_channels` for testing.
   Unit tests: extend `build_ssh_tmux_cc_args` tests (src/remote/mod.rs:436+) to assert `-tt`, ServerAlive options, and login-shell wrapping; test that `ssh_args` for `:ssh` also contain ServerAlive* (src/remote/mod.rs:127).
3. **Bridge hardening (all headless, driven through `from_channels`):**
   - `%exit reason` and EOF-without-exit -> store `TmuxDisconnect{reason, at}`, show status message, keep panes marked "(disconnected)" instead of dropping silently.
   - Replace the hard-coded `new()` seed with an empty map plus an explicit local-pane association; test that `%output %5` on a fresh bridge never touches local pane 1.
   - Clear/reconcile maps on reconnect (diff `%layout-change` pane sets), handle `WindowClose` by removing mapped panes (`WindowManager::remove_pane`), no duplicate `TMUX %N` panes on repeated attach.
   - Pump `take_outbound_commands` into `cmd_tx` each tick; on (re)attach enqueue `list-windows`/`capture-pane -p -e -S - -t %N` to hydrate; test that after a simulated disconnect + new `from_channels` attach, pane count equals remote pane count and content is restored.
   - Reconnect supervisor with capped exponential backoff (1s,2s,4s...30s) as a pure state machine (`next_delay(attempt)`), fed a fake clock; test the backoff and the "no reconnect after user-initiated detach" rule.
4. **Pane exit visibility.** Call `hold_exited_shell` (or at least set a status message with exit code and last 10 lines) from `poll_all_ptys` for panes whose `remote_target().is_some()` instead of silently `remove_pane` (src/wm/mod.rs:697-706). Test: headless pane with remote_target gets `PtyEvent::Exited` -> pane persists with held banner.
5. **Restore honesty.** In `apply_saved_session_snapshot` do not carry `remote_target`/remote title onto a pane whose spawned program is the local shell, or mark it "(not connected)"; optionally offer to re-run `running_command`. Test with a saved session fixture containing `remote_target` and `shell="/bin/bash"`.
6. **ControlMaster hygiene.** Add `ServerAliveInterval=15`, `ServerAliveCountMax=3`, `TCPKeepAlive=yes` in `push_common_args`; consider `ControlPersist=10s` or `ControlMaster=no` for reconnect. Test via the existing arg-builder tests.
7. **Observability.** `tracing::info!` in connect/exit/poll paths so `bilterm.log` records: ssh argv (redacted), pane exit (code/signal), `%exit` reason, reconnect attempts.

## Files of interest

- src/wm/session.rs:247-281 (connect_ssh_tmux), :181-244 (spawn_ssh_pane), :543 (restore_session)
- src/remote/mod.rs:87-160 (ssh arg builders; no keepalive; `-T`; bare tmux)
- src/tmux/mod.rs:234-454 (bridge, subprocess, poll; unreachable from UI)
- src/wm/mod.rs:615-646, 680-722 (split_focused, poll_all_ptys/remove_pane)
- src/pane/mod.rs:738-780 (exit/hold/respawn), src/pty/mod.rs:154-175 (reader exit)
- src/wm/layout_profile.rs:504-562, src/wm/v2_runtime.rs:209-222 (restore/detach)
- User data (read-only, hosts redacted): ~/.local/state/bilterm/sessions/{main,dual-h,test270}.toml; ~/.local/state/bilterm/bilterm.log (empty)
