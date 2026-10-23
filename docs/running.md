# Running Bilterm: Shells, Tab Completions, Configuration & CLI Reference

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md)

## Table of Contents

- [1. Prerequisites & Installation](#1-prerequisites--installation)
- [2. Shells & Tab Command Completions](#2-shells--tab-command-completions)
  - [Which Shell Does Bilterm Run?](#which-shell-does-bilterm-run)
  - [How Does Tab Command Completion Work?](#how-does-tab-command-completion-work)
- [3. Configuration Reference (`~/.config/bilterm/config.toml`)](#3-configuration-reference-configbiltermconfigtoml)
- [4. Environment Variables & Exit Codes](#4-environment-variables--exit-codes)

---

## 1. Prerequisites & Installation

### System Requirements
- **Rust**: `1.88+` (Edition 2024)
- **OS**: Linux (primary POSIX PTY), macOS, or Windows (`ConPTY`)

### Build & Run
```bash
cargo build --release
./target/release/bilterm
```

### CLI Options (`v1.4.0`)
```bash
bilterm                                         # Launch interactive TUI multiplexer
bilterm --mcp                                   # Run Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio
bilterm --headless --listen 127.0.0.1:7370      # Run headless JSON-RPC 2.0 automation daemon
bilterm ctl 127.0.0.1:7370 workspace.status     # Query or drive a running Bilterm instance via JSON-RPC 2.0
bilterm --playbook <playbook.toml>              # Execute a declarative TOML automation playbook and exit
bilterm --gui-dump <path.ppm>                   # Render GPU/GUI framebuffer snapshot (Netpbm P6 .ppm)
bilterm --completions bash|zsh|fish              # Generate shell completions for bash, zsh, or fish
bilterm --man                                   # Print roff(7) manual page (docs/bilterm.1)
bilterm --version                               # Print version (bilterm 1.4.0)
bilterm --help                                  # Print CLI usage summary
```

---

## 2. Shells & Tab Command Completions

### Which Shell Does Bilterm Run?

Bilterm is **shell-agnostic**: it spawns your real interactive system shell inside a pseudoterminal (`portable-pty`) for every Composite Pane and for the Guake drop-down terminal (`src/pty/mod.rs`, `resolve_default_shell_for_os`):

| Platform | Environment Variable Checked | Fallback Shell | Supported Shells |
|----------|------------------------------|----------------|------------------|
| **Linux** | `$SHELL` | `/bin/sh` | `bash`, `zsh`, `fish`, `nu`, `dash`, `ksh`, `sh` |
| **macOS** | `$SHELL` | `/bin/sh` | `zsh` (macOS default), `bash`, `fish`, `nu`, `sh` |
| **Windows** | `%COMSPEC%` | `powershell.exe` | `powershell.exe`, `pwsh.exe`, `cmd.exe`, `wsl.exe` |

To launch Bilterm with a specific shell for a session, set `SHELL` when invoking `bilterm`:

```bash
SHELL=/bin/zsh ./target/release/bilterm
SHELL=/usr/bin/fish ./target/release/bilterm
```

---

### How Does Tab Command Completion Work?

Bilterm supports **two complementary `<Tab>` completion modes** depending on whether you are typing in the **Command Prompt Box (`INPUT: CMD BOX`)** or directly in **Raw PTY Mode (`INPUT: RAW PTY`)**:

#### Mode 1: Built-in Context-Aware Completion in `INPUT: CMD BOX` (`src/pane/completion.rs`)

When typing inside Bilterm's floating or split command prompt (`==> `), pressing **`Tab`** (or **`Shift+Tab`** to cycle backwards) invokes Bilterm's zero-latency completion engine (`compute_completions`):

1. **`:colon` Commands & Sub-Arguments**:
   - If the line begins with `:`, Bilterm completes all built-in multiplexer commands (`:prompt`, `:split`, `:vsplit`, `:layout`, `:deck`, `:focus`, `:zoom`, `:quake`, `:theme`, `:outline`, `:border`, `:shadows`, `:glyphs`, `:cursor`, `:crosshair`, `:fkeybar`, `:history`, `:block`, `:yank`, `:clear`, `:raw`, `:help`, `:quit`).
   - Pressing `Tab` after a command space (e.g., `:prompt <Tab>`, `:outline <Tab>`, `:theme <Tab>`, `:shadows <Tab>`, `:history <Tab>`, `:crosshair <Tab>`) completes valid sub-arguments (`split-top`, `split-bottom`, `double`, `rounded`, `dotted`, `tinted`, `save`, `max-files`, `full`, etc.).
2. **Command Position (First Token on a Line or After `|` / `;` / `&&`)**:
   - **Explicit Paths**: If the token starts with `./`, `../`, `/`, or `~/`, Bilterm completes matching filesystem entries relative to the child shell's live working directory.
   - **Shell Builtins & Keywords**: Matches standard shell builtins (`cd`, `echo`, `export`, `source`, `alias`, `unalias`, `history`, `jobs`, `fg`, `bg`, `kill`, `pwd`, `read`, `set`, `unset`, `shift`, `test`, `time`, `trap`, `type`, `ulimit`, `umask`, `wait`, ` which`, `sudo`, `env`, `exec`, `eval`, `exit`, `return`, `true`, `false`).
   - **`$PATH` Executables**: Scans every directory in your `$PATH` environment variable, verifying POSIX executable permission bits (`mode & 0o111 != 0` on Unix), and deduplicates matching binary names.
3. **Argument Position (Subsequent Tokens)**:
   - **Live Working Directory Tracking**: Resolves relative paths against the child shell's actual current working directory — tracked automatically via `OSC 7` (`\x1b]7;file://hostname/path\x07`) and Linux `/proc/<shell_pid>/cwd` inspection!
   - **Tilde (`~/`) Expansion**: Expands `~/` against `$HOME` while preserving the `~/` prefix in your command buffer.
   - **Directory Traversal**: Appends `/` to completed directories without a trailing space so you can press `Tab` repeatedly to drill down nested directory trees.
4. **Environment Variables (`$VAR` and `${VAR}`)**:
   - Any token starting with `$` or `${` completes against all live environment variables in the process environment (`$PATH`, `$HOME`, `${XDG_CONFIG_HOME}`, etc.).
5. **Longest Common Prefix (LCP) & Visual Candidate Ribbon**:
   - **Single Match**: Immediately completes the token in-place (appending a trailing space for commands/files, or `/` for directories).
   - **Multiple Matches**: Expands the **Longest Common Prefix (LCP)** and displays a bottom ribbon inside the prompt box:
     ```text
     TAB: cargo  [cargo-clippy]  cargo-fmt  cargo-nextest
     ```
     Press **`Tab`** / **`Shift+Tab`** to cycle through candidates in-place, any character key to commit the selection and keep typing, or **`Esc`** to dismiss the ribbon.

#### Mode 2: Native Shell Completion in `INPUT: RAW PTY` (`F12` / `Ctrl+G`)

If you toggle `INPUT: RAW PTY` (via **`F12`**, **`Ctrl+G`**, or `:raw`), pressing **`Tab`** sends raw `0x09` (`\t`) directly to your underlying `$SHELL` PTY, triggering your shell's native programmable completion (`zsh` `compsys`, `bash-completion`, `fish` completions, or `fzf` tab widgets).

---

## 3. Configuration Reference (`~/.config/bilterm/config.toml`)

| Key | Type | Allowed Values | Default |
|-----|------|----------------|---------|
| `theme` | `string` | `"turbo-blue"`, `"turbo-cyan"`, `"ibm-3278-green"`, `"ibm-3279-amber"`, `"ibm-mono"` | `"turbo-blue"` |
| `glyphs` | `string` | `"unicode"`, `"cp437"`, `"nerd-font"`, `"ascii"` | `"unicode"` |
| `cursor_style` | `string` | `"solid-block"`, `"blinking-hollow-block"`, `"underscore"`, `"vertical-bar"` | `"solid-block"` |
| `window_outline` | `string` | `"double"`, `"solid"`, `"rounded"`, `"thick"`, `"dotted"`, `"dashed"`, `"quadruple"`, `"ascii"` | `"double"` |
| `prompt_outline` | `string` | `"double"`, `"solid"`, `"rounded"`, `"thick"`, `"dotted"`, `"dashed"`, `"quadruple"`, `"ascii"` | `"double"` |
| `shadow_style` | `string` | `"none"`, `"translucent"`, `"theme-tinted"`, `"solid-half-block"` | `"translucent"` |
| `drop_shadows` | `bool` | `true`, `false` | `false` |
| `crosshair_cursor` | `bool` | `true`, `false` | `false` |
| `crosshair_fullscreen` | `bool` | `true`, `false` | `false` |
| `show_dos_fkey_bar` | `bool` | `true`, `false` | `false` |
| `history_enabled` | `bool` | `true`, `false` | `true` |
| `history_max_files` | `usize` | `1..=10000` | `25` |
| `history_dir` | `string` (optional) | Directory path | `None` |
| `default_prompt_anchor` | `string` | `"top"`, `"bottom"`, `"floating"`, `"split-top"`, `"split-bottom"` | `"top"` |
| `default_prompt_height` | `u16` | `3..=12` | `3` |
| `show_menu_bar` | `bool` | `true`, `false` | `false` |
| `crt_scanlines` | `bool` | `true`, `false` | `false` |
| `triggers` | `array of strings` | Case-insensitive output highlight patterns | `[]` |
| `[keybindings]` | `table` | Custom keychord -> `:command` mappings (e.g. `"Ctrl+T" = ":split v"`) | `{}` |
| `[remotes.<name>]` | `table` | Named SSH / `herdr`-style remote host profiles (`destination`, `port`, `identity_file`, `tmux_session`, `rpc_endpoint`, `jump_host`) | `{}` |

---

## 4. Environment Variables & Exit Codes

| Variable / Code | Description |
|-----------------|-------------|
| `SHELL` | Default POSIX shell spawned on Linux/macOS (fallback: `/bin/sh`) |
| `COMSPEC` | Default shell spawned on Windows (fallback: `powershell.exe`) |
| `TMUX` | Auto-detected for outer `tmux` coexistence and `DCS tmux;` clipboard passthrough |
| `XDG_CONFIG_HOME` | Override base directory for `bilterm/config.toml` |
| `XDG_STATE_HOME` | Override base directory for `bilterm/history/*.log` session history files |
| `RUST_LOG` | Tracing filter written to `./bilterm.log` (e.g., `RUST_LOG=bilterm=debug`) |
| **Exit Code `0`** | Clean exit (`:quit`, `:q`, `Ctrl+Q`, or last pane closed) |
| **Exit Code `1`** | Initialization or fatal runtime I/O error (after `TerminalRestoreGuard` restores host terminal state) |
