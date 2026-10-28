# Agent Autonomy & Super-Permissive Configuration Guide (`Linux` & `macOS`)

> **Purpose:** Reference and drop-in configuration templates for running **Antigravity (`agy` / Antigravity 2.0)**, **Claude Code**, **Gemini CLI**, and **OpenGSD** in 100% hands-off, zero-prompt autonomous mode ("Dark Software Factory" / `yolo` mode), distilled from the `800+` historical command approvals in `~/.gemini/antigravity-cli/settings.json` and multi-session Linux + macOS development of Bilterm.

---

## 1. Why Agents Still Prompt Even After Hundreds of "Always Allow" Approvals

Inspection of `~/.gemini/antigravity-cli/settings.json` (`770+` accumulated entries) revealed three root causes of repeated confirmation prompts:

1. **Over-Specific Literal Command Approvals**:
   - Approving individual invocations in the UI records exact argument strings such as:
     - `"command(cargo build -q --release)"`
     - `"command(cargo clippy -q --all-features --all-targets -- -D warnings)"`
   - When an agent later runs `cargo clippy -q --all-targets -- -D warnings` (without `--all-features`) or `cargo test -q --test v2_phases_30_to_34_integration`, it misses the literal match and prompts again.
2. **Compound Shell Pipelines (`&&`, `||`, `;`, `|`)**:
   - A compound command like `shopt -s globstar && awk 'ENDFILE { ... }' src/**/*.rs` or `cargo build -q --release && install -m 0755 ...` requires **every** constituent binary/builtin (`shopt`, `awk`, `cargo`, `install`) or a full-string regex (`command(.*)`) to be permitted.
3. **Subagent Inheritance**:
   - Parallel subagents (`invoke_subagent` with `Model: "flash"` or `"inherit"`) evaluate `run_command` against the same host permission policy. If a subagent uses a shell builtin (`shopt`, `printf`, `set`, `true`, `test`) not in `permissions.allow`, the subagent blocks waiting for human approval.

---

## 2. Antigravity CLI (`agy`) & Antigravity 2.0 Super-Permissive Setup

### Option A: Maximum Autonomy (Zero Prompts for Any Command)

In **`~/.gemini/antigravity-cli/settings.json`** (both Linux and macOS), set `"allowNonWorkspaceAccess": true` and include `"command(.*)"` alongside base command prefixes in `"permissions.allow"`:

```json
{
  "allowNonWorkspaceAccess": true,
  "toolExecutionPolicy": "always-proceed",
  "artifactReviewMode": "always-proceed",
  "nonWorkspaceFileAccess": "allow",
  "internetAccessPolicy": "allow",
  "permissions": {
    "allow": [
      "command(.*)",
      "command(cargo)",
      "command(rustc)",
      "command(rustup)",
      "command(git)",
      "command(gh)",
      "command(shopt)",
      "command(awk)",
      "command(sed)",
      "command(rg)",
      "command(grep)",
      "command(find)",
      "command(ls)",
      "command(cat)",
      "command(head)",
      "command(tail)",
      "command(sort)",
      "command(wc)",
      "command(diff)",
      "command(sha256sum)",
      "command(shasum)",
      "command(install)",
      "command(cp)",
      "command(mv)",
      "command(rm)",
      "command(mkdir)",
      "command(touch)",
      "command(ln)",
      "command(chmod)",
      "command(tar)",
      "command(zip)",
      "command(unzip)",
      "command(curl)",
      "command(jq)",
      "command(python3)",
      "command(python)",
      "command(uv)",
      "command(pytest)",
      "command(node)",
      "command(npm)",
      "command(npx)",
      "command(nix)",
      "command(just)",
      "command(zig)",
      "command(codegraph)",
      "command(graphify)",
      "command(bilterm)",
      "command(~/.local/bin/bilterm)",
      "command(~/.cargo/bin/bilterm)",
      "command(./target/release/bilterm)",
      "command(./target/debug/bilterm)",
      "command(bash)",
      "command(sh)",
      "command(zsh)",
      "command(env)",
      "command(which)",
      "command(pwd)",
      "command(echo)",
      "command(printf)",
      "command(true)",
      "command(set)",
      "command(exit)"
    ],
    "deny": [
      "command(rm -rf /)",
      "command(rm -rf ~)",
      "command(git push --force origin main)",
      "command(git reset --hard origin/main)"
    ]
  }
}
```

### Option B: Antigravity 2.0 Desktop App UI Settings

In **Antigravity 2.0 → Left Sidebar → Settings**:

| Setting | Super-Permissive Value | Purpose |
| :--- | :--- | :--- |
| **Tool Execution Policy** | `always-proceed` | Runs all `run_command` calls (including inside subagents) without interactive confirmation. |
| **Artifact Review Mode** | `always-proceed` | Prevents plans/artifacts from pausing execution waiting for a "Proceed" click. |
| **Non-Workspace File Access** | `allow` | Allows installing binaries to `~/.local/bin/bilterm`, `~/.cargo/bin/bilterm`, or `~/Applications/Bilterm.app`, and reading shared skills in `~/.gemini/`. |
| **Internet Access Policy** | `allow` | Allows `gh pr`, `git fetch/pull/push`, `cargo`, and web documentation lookups. |
| **Terminal Sandbox** | `off` (or per-project `off`) | Allows `portable-pty` PTY allocation (`openpty`/`forkpty`), Unix domain sockets (`0600`), and `cargo` builds without sandbox device errors. |

---

## 3. Consolidated Command Foundation (Distilled from 770+ Session Approvals)

Every command approved across the Bilterm, Flowscope, OntoLogic, Maiors, and GSD sessions collapses into the following **canonical prefix allowlist**. Using bare command prefixes (`command(<binary>)`) in `permissions.allow` matches any flags or arguments passed to that binary:

| Category | Canonical `permissions.allow` Entries | Replaces N Literal Entries |
| :--- | :--- | :--- |
| **Rust & Cargo Toolchain** | `command(cargo)`, `command(rustc)`, `command(rustup)`, `command(cargo-zigbuild)`, `command(cargo-hack)`, `command(cargo-nextest)` | Replaces 95+ literal `cargo test/clippy/build/check/run/fmt` entries |
| **Git & GitHub CLI** | `command(git)`, `command(gh)` | Replaces 80+ literal `git status/add/commit/push/pull/fetch/diff/log/tag/worktree/checkout` entries |
| **Shell Builtins & Text Processing** | `command(shopt)`, `command(awk)`, `command(sed)`, `command(rg)`, `command(grep)`, `command(find)`, `command(sort)`, `command(head)`, `command(tail)`, `command(wc)`, `command(cat)`, `command(printf)`, `command(echo)`, `command(set)`, `command(true)`, `command(bash)`, `command(sh)` | Prevents prompts on `ARCH-R04` line-ceiling checks (`shopt -s globstar && awk ...`) and scripted loops |
| **Filesystem & Release Packaging** | `command(install)`, `command(cp)`, `command(mv)`, `command(rm)`, `command(mkdir)`, `command(chmod)`, `command(ln)`, `command(touch)`, `command(tar)`, `command(sha256sum)`, `command(shasum)`, `command(ls)`, `command(pwd)`, `command(which)` | Replaces 110+ literal file/packaging entries (`install -m 0755 ...`, `chmod +x ...`) |
| **macOS App Bundle Tools** | `command(sips)`, `command(iconutil)`, `command(codesign)`, `command(otool)`, `command(plutil)`, `command(open)`, `command(defaults)`, `command(xcrun)` | Enables zero-prompt `Bilterm.app` `.icns` icon generation and bundle installation on macOS |
| **OpenGSD & Node Orchestration** | `command(node)`, `command(gsd-tools)`, `command(npm)`, `command(npx)` | Replaces 180+ literal `node .../gsd-tools.cjs ...` entries |
| **Python, Benchmarks & Eval** | `command(python3)`, `command(python)`, `command(uv)`, `command(pytest)`, `command(ruff)`, `command(poetry)`, `command(.venv/bin/python)`, `command(.venv/bin/pytest)`, `command(.venv/bin/pip)` | Replaces 220+ literal Python test/eval/benchmark entries |
| **Code Intelligence & Build Systems** | `command(codegraph)`, `command(graphify)`, `command(nix)`, `command(just)`, `command(zig)`, `command(gradle)`, `command(./gradlew)`, `command(javac)`, `command(gcc)`, `command(docker)`, `command(kubectl)`, `command(helm)` | Covers all workspace indexers, Nix flakes, Zig cross-builds, and container tools |

---

## 4. Claude Code Super-Permissive Setup (`.claude/settings.json`)

For agents running under Claude Code (or hybrid Claude/GSD sessions), place the following in `.claude/settings.local.json` (or globally in `~/.claude/settings.json`):

```json
{
  "env": {
    "SWE_SKILLS_CHECKPOINT_GLOBS": ".planning/CHECKPOINT.md:.planning/STATE.md:docs/plans/*checkpoint*.md:CHECKPOINT.md"
  },
  "permissions": {
    "defaultMode": "bypassPermissions",
    "allow": [
      "Bash(*)",
      "Read(*)",
      "Edit(*)",
      "Write(*)",
      "MultiEdit(*)",
      "Glob(*)",
      "Grep(*)",
      "LS(*)",
      "Task(*)",
      "WebFetch(*)",
      "WebSearch(*)",
      "mcp__*"
    ],
    "deny": [
      "Bash(rm -rf /)",
      "Bash(git push --force origin main)"
    ]
  }
}
```

---

## 5. OpenGSD Autonomous Workflow Configuration (`.planning/config.json`)

To prevent OpenGSD phase/milestone runners (`/gsd-autonomous`, `/gsd-execute-phase`, `/gsd-plan-phase`) from pausing for human confirmation between waves or phases, verify these keys in [`.planning/config.json`](../.planning/config.json):

```json
{
  "mode": "yolo",
  "parallelization": true,
  "commit_docs": true,
  "workflow": {
    "auto_advance": true,
    "skip_discuss": true,
    "human_verify_mode": "none",
    "use_worktrees": false
  }
}
```

---

## 6. Agent Execution Best Practices to Avoid Triggering Prompts

Even with a permissive allowlist, agents should follow these command hygiene rules:

1. **Prefer Native Tools Over Shell Commands for File Operations**:
   - Always use `read_file`, `edit_file`, and `write_file` instead of `cat << 'EOF'`, `sed -i`, or `python3 -c` file rewrites. Native file tools never trigger shell command prompts.
2. **Start Commands with an Allowlisted Binary Name**:
   - Instead of leading with shell variable assignments (`FOO=bar cargo test -q`) or subshells (`(cd foo && cargo test)`), invoke `cargo test -q` directly (or `env FOO=bar cargo test -q` with `command(env)` allowlisted) so prefix matchers immediately recognize the leading token.
3. **Never Use Shell `sleep` Loops**:
   - Use the native `schedule` tool (`DurationSeconds`) for polling/timers rather than `sleep 30 && gh pr list`.
