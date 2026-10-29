# Bilterm Session Checkpoint (`v2.1.0` + Post-Merge Hardening & Docs Sync)

- **Timestamp**: 2026-10-28T20:01:00-04:00
- **Branch & Commit**: `main` at `7817cd2` (synced with `origin/main`)
- **Installed Binary**: `~/.local/bin/bilterm` & `~/.cargo/bin/bilterm` on Linux; `/Users/bill/.local/bin/bilterm` & `/Users/bill/Applications/Bilterm.app` on macOS (`bilterm 2.1.0`)

## 1. Completed in This Session
1. **`v2.0.0` (`4c52454`) — Full Window Layout Detach/Re-Attach & Complete `<F1>` / `:help` Catalog**:
   - Full multi-pane window layout detach/re-attach (`SplitTree` + `ZStack` `FloatingWindowGeometry` + prompt state + visible screen lines) via `Ctrl+X D`, `:detach [name]`, `:attach [name]`, and `bilterm attach [name]` (`-a`).
   - Complete `<F1>` / `:help` reference (`52+` `:colon` commands across all subsystems, including all `:mcp` subcommands), Command Palette catalog (`src/ui/help_catalog.rs`), and `<Tab>` completions (`src/pane/completion.rs`).
2. **Modular Documentation & `README.md` Shortening (`f24e222`, `19390c2`)**:
   - Shortened `README.md` (`235 -> 101` lines) with `## About`, created `docs/VISUAL_GALLERY.md`, and synchronized `docs/README.md`, `docs/running.md`, `docs/user-guide/06-keyboard-mouse-and-command-reference.md`, and `docs/DEVELOPER_AND_AGENT_GUIDE.md` for `v2.1.0`.
3. **`v2.1.0` (`f361b9b`) — Phases 35 & 36 (Executed in Parallel via Subagents)**:
   - **Phase 35 (`DAEM-03..04`)**: Live background OS process daemonization (`src/daemon/process.rs`, `bilterm --daemon-serve --session <name>`, private `0600` Unix socket `daemon-<session>.sock`, stale socket cleanup on `ConnectionRefused`, `bilterm ls` / `bilterm sessions`, and `"session.save"` / `"session.restore"` JSON-RPC 2.0 handlers; verified in `tests/live_daemon_detach_attach.rs`).
   - **Phase 36 (`MCP-03..04`)**: Interactive mouse & keyboard navigation in `:mcp` console (`src/mcp_console/mod.rs`, `src/ui/mcp_ui.rs`, `McpRowAction`: `FocusPane`, `SessionNode`, `ApplyTheme`, `Up`/`Down`/`j`/`k`/`Enter` + mouse click row activation, `r`=Refresh, `f`=Freeze/Thaw, `n`=Branch, `m`=Float↔Push, `d`=Edge; verified in `tests/mcp_interactive_navigation.rs`).
4. **Pulled & Verified macOS / Claude Code / `agy` Hardening (`0feac6f..7817cd2`)**:
   - Guarded Kitty Keyboard Protocol (`CSI < u` / `CSI = u`) stack pops (`src/vt/mod.rs`, `src/vt/scan.rs`).
   - Fixed shifted printable characters under Kitty disambiguate mode (`src/input/encode.rs`).
   - Fixed inline raw-mode TUI prompt submission with bracketed paste (`\x1b[200~...\x1b[201~\r` in `src/pane/mod.rs`).
   - Added `.zshenv`/`.zprofile` sourcing in `ZDOTDIR` wrapper and `UTF-8` `LANG` fallback in GUI mode (`src/pty/shell_integration.rs`, `src/pty/mod.rs`).

## 2. Verification State
- `cargo clippy -q --all-targets -- -D warnings`: `0` warnings.
- Unit & integration test suites passing (`184` lib unit tests + all integration suites including `live_daemon_detach_attach` and `mcp_interactive_navigation`).
- All `src/**/*.rs` files `<= 800` lines (`ARCH-R04`) and `src/input/mod.rs::handle_wm_key_event` `<= 80` lines (`ARCH-R01`).

## 3. Exact Next Steps (When Resuming After Restart)
Ready to plan & execute items **3 and 4** from our roadmap backlog:
- **Option 3 (Phase 37)**: Cross-Platform Release Packaging & Distribution (`cargo-dist` / GitHub Actions release binaries for Linux `x86_64`/`aarch64`, macOS universal `.app` bundle / Homebrew formula, and Windows `.exe` installer).
- **Option 4 (Phase 38)**: Live PTY Stream Multiplexing Across Daemon Re-Attach (attaching multiple concurrent TUI/GUI clients to the same running `daemon-<session>.sock` PTY master streams without respawning child shells).
