# Bilterm — Claude & AI Coding Agent Instructions

Please read **[`AGENTS.md`](AGENTS.md)** and **[`docs/DEVELOPER_AND_AGENT_GUIDE.md`](docs/DEVELOPER_AND_AGENT_GUIDE.md)** first. Together they document:

1. **Crate & Library Stack (`Cargo.toml`)**: `ratatui 0.30.2`, `crossterm 0.29.0`, `portable-pty 0.9.0`, `tokio 1.44+`, `vt100-ctt 0.17.1` (default VT engine; optional `libghostty-vt` via feature `ghostty-vt`), `serde`+`toml`, `nucleo-matcher`, and `insta`.
2. **Complete 224-File Source Map (`src/**/*.rs`) & Architectural Gates**:
   - Every `src/**/*.rs` file SHOULD target **~800–1000 lines** (`ARCH-R04`, a soft goal; hard test backstop 1200). Split when it improves modularity/readability, not to hit a number.
   - `src/input/mod.rs::handle_wm_key_event` MUST stay **`<= 80` lines** (`ARCH-R01`).
   - All user-triggerable commands/keybindings/RPC calls lower to `crate::action::Action` and execute via `WindowManager::apply(Action)` (`src/colon_command/execute_wm.rs`).
   - All `WindowManager` and `CompositePane` internal fields are private (`ARCH-R02`); pane removal goes exclusively through `WindowManager::remove_pane(id)` and command submission goes exclusively through `CompositePane::submit_command(&mut self)`.
   - All runtime directories and files use `src/secure_fs.rs` (`0700` dirs, `0600` atomic files, `validate_safe_name`).
3. **Inspectability & Debugging**:
   - Live TUI: Press **`F9`** (`:inspector`) for the 3270 Terminal Inspector modal.
   - Trace logs: `RUST_LOG=bilterm=debug cargo run` writes to `bilterm.log`.
   - Live RPC / MCP: `bilterm ctl workspace.status` or `bilterm --mcp`.
   - Deterministic headless unit tests: `WindowManager::new_headless(cols, rows)` + `CompositePane::new_headless(id, title, cols, rows)`.

## Verification (token-cheap)
Run `scripts/verify.sh`; read its single output line. PASS = done. FAIL = read only `target/verify/<stage>/fail.txt`, then `log`. Details in `AGENTS.md` (PERF-CTX).
