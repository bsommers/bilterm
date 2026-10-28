# Bilterm Documentation Hub (`v2.1.0`)

[← Back to Project README](../README.md) | [Visual Gallery (`docs/VISUAL_GALLERY.md`)](VISUAL_GALLERY.md) | [Starter Packs (`starter-packs/`)](../starter-packs/README.md)

Welcome to the comprehensive documentation suite for **Bilterm (`v2.1.0`)** — the IBM 3270 Mainframe, DOS Borland Turbo Vision, & TRON Master Control Program (`MCP`) inspired TUI multiplexer, native desktop GUI terminal (`--gui`), composite-pane shell wrapper, remote SSH fleet manager, and AI-automatable terminal emulator written in Rust + Ratatui.

---

## Documentation Index

| Document | Purpose & Scope |
|----------|-----------------|
| **[`docs/VISUAL_GALLERY.md`](VISUAL_GALLERY.md)** | **Visual Gallery & SVG Screen Renders** — Borland Turbo Blue workspace, per-pane independent font scaling (`120%`/`80%`/`100%`), 4-pane Multi-Agent AI Swarm, IBM 3278 Phosphor Green (`F9` Inspector), and IBM 3279 Amber (`Ctrl+P` Command Palette) |
| **[`docs/user-guide/README.md`](user-guide/README.md)** | **Complete 10-Chapter Interactive User Manual** covering every CLI flag, keybinding, mouse gesture, `:colon` / `:mcp` command, use case, and security best practice |
| **[`docs/user-guide/06-keyboard-mouse-and-command-reference.md`](user-guide/06-keyboard-mouse-and-command-reference.md)** | **Complete CLI, Keyboard, Mouse & `:command` Reference** — exhaustive tables of leader chords, global hotkeys, mouse frame/status-bar controls, and `:colon` / `:mcp` directives |
| **[`docs/user-guide/08-extended-cli-and-gui-examples.md`](user-guide/08-extended-cli-and-gui-examples.md)** | **Extended CLI, Native `--gui` & Typography Cookbook** — `bilterm --gui`, `bilterm attach`, per-pane `Ctrl++`/`Ctrl+-` font scaling, `--headless`, `bilterm ctl`, `--mcp`, `bilterm run`, and `bilterm plot` |
| **[`docs/user-guide/09-use-cases-and-workflows.md`](user-guide/09-use-cases-and-workflows.md)** | **Real-World Use Cases & Workflows** — Multi-Agent AI Coding Swarms, Remote Agents, Remote Development (`:ssh-tmux`), Robot Scripting & QA, Incident War Rooms, and Custom Per-Pane Font Layouts |
| **[`docs/user-guide/10-security-and-best-practices.md`](user-guide/10-security-and-best-practices.md)** | **Security Architecture & Operational Best Practices** — 3-Signal `!ECHO LOCK`, `/dev/random` escape-trap sanitization, paste-injection guards, `0700`/`0600` confinement, Unix socket `SO_PEERCRED`, and bearer-token RPC auth |
| **[`starter-packs/README.md`](../starter-packs/README.md)** | **5 Ready-to-Run Starter Packs** — Multi-Agent Swarm, Remote Dev & Fleet, Robot QA Harness, Sysadmin Incident War Room, and Retro CRT Workstation profiles |
| **[`docs/DEVELOPER_AND_AGENT_GUIDE.md`](DEVELOPER_AND_AGENT_GUIDE.md)** | **Developer, Maintainer & AI Agent Engineering Guide** — complete crate dependency stack, modular source tree (`<= 800` lines gate), `Action` pipeline, encapsulated invariants, live `<F9>` Inspector / `bilterm.log` / `bilterm ctl` debugging playbook |
| **[`docs/AGENT_AUTONOMY_AND_PERMISSIONS.md`](AGENT_AUTONOMY_AND_PERMISSIONS.md)** | **Agent Autonomy & Super-Permissive Configuration Guide (`Linux` & `macOS`)** — drop-in `settings.json` / `.claude/settings.local.json` / `.planning/config.json` templates for zero-prompt autonomous execution |
| **[`docs/LESSONS_LEARNED_COMPOUNDED.md`](LESSONS_LEARNED_COMPOUNDED.md)** | **Compounded Engineering & Architecture Lessons Learned (`macOS × Linux × v2.0`)** — Unified 10-domain synthesis matrix and 15 architectural invariants (`INV-01..15`) |
| **[`docs/LESSONS_LEARNED.md`](LESSONS_LEARNED.md)** | **Engineering & Architecture Lessons Learned (Sourced from macOS)** — macOS `Option` dead-key normalization, Powerline/PUA geometric glyphs, dual-plane selection & occlusion, and `BlockPage` (`MORE...`/`HOLDING`) scrolling |
| **[`docs/LESSONS_LEARNED_LINUX.md`](LESSONS_LEARNED_LINUX.md)** | **Engineering Lessons Learned (Sourced from Linux)** — `Ord::clamp` viewport inversion guards, per-pane font scaling + `TIOCSWINSZ`, `RawPtyPassthrough` WM chord interception, and zero-header X11/Xft `dlopen` native GUI |
| **[`docs/purpose.md`](purpose.md)** | Problem statement, IBM 3270 + Borland Turbo Vision design philosophy, threat model, and security boundaries (`!ECHO LOCK`, `sanitize_stream_traps`, `occlude_rect`) |
| **[`docs/architecture.md`](architecture.md)** | Full subsystem breakdown, 3-pass Ratatui compositor, 3-signal line-discipline state machine, SSH/Fleet Federation, JSON-RPC 2.0 / MCP Automation Plane, `WorkspaceRecorder`, and Mermaid/ASCII diagrams |
| **[`docs/running.md`](running.md)** | Prerequisites, installation, CLI & Automation, shell resolution (`$SHELL` / `%COMSPEC%`), `<Tab>` completion internals, `config.toml`, and exit codes |
| **[`docs/how-to.md`](how-to.md)** | Step-by-step task recipes and troubleshooting matrix |
| **[`docs/diagrams/README.md`](diagrams/README.md)** | Multi-format visual diagram catalog (**ASCII** `.txt`, **Mermaid** `.mmd`, and **native draw.io** `.drawio` XML) |
| **[`docs/architecture/OVERVIEW.md`](architecture/OVERVIEW.md)** | Architecture Decision Records (`ADR-001` through `ADR-007`) |
| **[`docs/TEST_COVERAGE.md`](TEST_COVERAGE.md)** | Test coverage report (**`83.45%` line coverage**, 250+ passing tests) and battle/stress/performance benchmarks |
| **[`docs/launch-presentation-and-pitch-script.md`](launch-presentation-and-pitch-script.md)** | 12-slide technical deck specification and 5-minute product launch video script |
| **[`CHANGELOG.md`](../CHANGELOG.md)** | Semantic versioning release history (`v0.1.0` through `v2.0.0`) |

---

## Complete CLI Options Summary

| Command | Description |
|---------|-------------|
| `bilterm` | Launch interactive IBM 3270 × Borland Turbo Vision TUI multiplexer (auto-resumes detached session) |
| `bilterm attach [session]` (`-a` / `--attach`) | Re-attach & restore saved/detached multi-pane window layout intact |
| `bilterm -G` / `--gui` | Launch standalone native desktop GUI window (X11 / Win32 GDI / Cocoa) |
| `bilterm --gui-font <NAME>` (`-F` / `--font`) | Set font family or preset (`"JetBrains Mono"`, `"IBM 3270"`, `"Fira Code"`, `"vga"`, etc.) |
| `bilterm --gui-font-size <PT>` (`--font-size`) | Set base font size in points (`6..=72`, default `16`) |
| `bilterm --gui-font-file <PATH>` (`--font-file`) | Load custom `.ttf` / `.otf` font file directly from disk |
| `bilterm -T` / `--theme <THEME>` | Override color palette (`turbo-blue`, `ibm-3278-green`, `ibm-3279-amber`, `tron-legacy`, `dracula`, etc.) |
| `bilterm -m` / `--mcp` | Run native Model Context Protocol (MCP `2024-11-05`) server over `stdio` |
| `bilterm -d` / `--headless [-l <addr>]` | Run headless JSON-RPC daemon (default private `0600` Unix socket; loopback TCP via `-l 127.0.0.1:PORT`) |
| `bilterm -r` / `--allow-remote` | Allow listening on non-loopback TCP addresses (requires `-t` / `--token-file`) |
| `bilterm -t` / `--token-file <path>` | Path to `0600` file containing bearer token (or `BILTERM_TOKEN_FILE`) |
| `bilterm -S` / `--snapshot-dir <path>` | Directory for confined workspace snapshots (default: `$XDG_STATE_HOME/bilterm/snapshots`) |
| `bilterm ctl [-t <path>] [endpoint] <method> [json]` | Send JSON-RPC 2.0 command (`workspace.status`, `pane.exec`, `fleet.exec`, etc.) |
| `bilterm plot [kind] [csv] [-o <file>] [-b <mode>]` | Plot `hires`, `line`, `bar`, `scatter`, `candlestick`, or `heatmap` inline or export `.svg`/`.ppm` |
| `bilterm run <file.toml>` (`-p` / `--playbook`) | Execute declarative multi-step TOML robot playbook with assertions |
| `bilterm -g` / `--gui-dump <path.ppm\|.svg>` | Render workspace via `GuiWorkspaceRenderer` to a `.ppm` or `.svg` framebuffer |
| `bilterm -s` / `--session <name>` | Use named persistent command-history & workspace session |
| `bilterm -c` / `--config <file.toml>` | Load configuration from custom TOML file path |
| `bilterm -C` / `--completions <bash\|zsh\|fish>` | Print shell completion script to `stdout` |
| `bilterm -M` / `--man` | Print standalone `roff(7)` manual page (`docs/bilterm.1`) |
| `bilterm -v` / `--version` (`-V`) | Print version information (`bilterm 2.0.0`) |
| `bilterm -h` / `--help` | Print CLI help summary |

---

## Complete Shortcuts & `:colon` / `:mcp` Reference Summary

| Action | Shortcut (`<L>` = `Ctrl+B` or `Ctrl+X`) | `:command` |
|--------|----------|------------|
| **Help / Command Palette / 3270 Inspector** | `F1` (`<L> ?`) / `Ctrl+P` (`<L> :`) / `F9` (`<L> i`) | `:help` / `:palette` / `:inspector` |
| **Detach & Leave Window Layout Intact / Re-Attach** | `Ctrl+X D` / Exit Guard `[d]` | `:detach [name]` / `:attach [name]` |
| **MCP Root Console (Floating vs. Push-Out)** | `m` (Float↔Push), `d` (Edge), `Esc` (Close) | `:mcp [status \| tree \| float \| push [edge] \| spawn \| graft \| broadcast \| freeze \| thaw \| derez --confirm \| checkpoint \| shutdown --confirm]` |
| **Exit Guard & In-Place Shell Respawn** | `[r]` Respawn, `[d]` Detach, `[q]` Quit | `:exit-guard [guard \| respawn \| detach \| off]` / `:respawn-pane` |
| **Split Horizontal / Split Vertical / Zoom** | `F2` (`<L> -` / `Alt+H`) / `<L> \|` (`Alt+V`) / `F5` (`<L> z`) | `:split h` / `:split v` / `:zoom` |
| **Per-Pane Font Zoom In / Out / Reset** | `Ctrl++` / `Ctrl+-` / `Ctrl+0` / `Ctrl+Wheel` | `:fontsize [+ \| - \| reset \| <pt> \| <pct>%]` / `:font` |
| **Resize Pane Frame / Equalize Splits** | Drag ANY border or corner / `Ctrl+Alt+Arrows` / `<L> =` | `:resize <left\|right\|up\|down\|reset>` / `:equalize` |
| **Toggle Tiled vs. Z-Stack / Shuffle Deck** | `<L> M` (`Alt+M`) / `F6` (`<L> o` / `Alt+Tab`) / `<L> 1..9` | `:layout stack\|tile` / `:deck next\|prev` / `:focus <N>` |
| **Guake Quick Terminal / Raw PTY Toggle** | `F10` (`Ctrl+\` / `Alt+\``) / `F12` (`Ctrl+G`) | `:quake` / `:raw` |
| **Cycle Floating / Fixed / Regular Terminal** | `F4` (`<L> e` / `Alt+E`) / Click OIA `READY` or `CMD BOX` | `:mode [float \| fixed \| terminal \| cycle]` |
| **Prompt Pin Top / Bottom / Non-Overlapping Split**| `<L> t` (`Alt+T`) / `<L> b` / `<L> s` (`Alt+S`) | `:prompt top \| bottom \| split-top \| split-bottom` |
| **Mainframe Page-Hold (`MORE...`) & Smooth Scroller** | `<L> PgDn` / `PF8` / `PA2` / `Space` / Click OIA `SCRL` | `:scroll [smooth \| page \| instant \| toggle \| next \| prev]` / `:more` |
| **3270 Field Hints / Vi Copy / Canvas Search** | `F3` (`<L> f`) / `F8` (`Alt+C`) / `<L> /` (`Alt+/`) | `:hints` / `:copy` / `:search <query>` |
| **Universal Themes (22 Presets + Import/Export)** | — | `:theme <preset \| list \| import <path> \| export [file.toml]>` |
| **Focal Spotlight / Focal Zoom Lens** | `Alt+O` (`<L> L`) / `Alt+Shift+O` / `Alt+}` & `Alt+{` | `:spotlight [circle\|ellipse] [fuzzy\|pixel]` / `:focalzoom [1.25x..3x]` |
| **Slide-Out Drawer & Session Tree** | `F7` (`<L> d`) | `:drawer [toggle\|left\|right\|top\|bottom\|float\|push]` / `:session [save\|restore\|list\|tree\|branch\|graft\|checkpoint]` |
| **Tektronix 4010/4014 & Graphical Canvas** | `\x1b[?38h` / `OSC 1337;Canvas=...` | `:canvas [hires\|tek\|mode\|res\|plot\|image\|export\|clear]` |
| **SSH Pane / Remote `tmux -CC` / Fleet Exec** | — | `:ssh <user@host> [port]` / `:ssh-tmux <user@host> [sess]` / `:remote [list\|add\|exec\|disconnect]` |
