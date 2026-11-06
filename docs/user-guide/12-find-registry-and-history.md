# Chapter 12: Find: Agents, Sessions & Command History

[← Chapter 11: Navigator & Agent Traffic](11-navigator-sidebar-and-agent-traffic.md) | [User Guide Home](README.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)

---

The **Find window** is one fuzzy, ranked search over three things: the agents and panes that are open now, the workspace sessions (live and saved on disk), and the commands you have run (captured in this run, plus the `history.log` files on disk). It opens with **`Ctrl+Alt+F`**, from the View menu, from the palette, or with `:find agent|session|history`. The same search is available to scripts over RPC (`find.*`) and to AI clients over MCP (`bilterm_find`).

This chapter describes what the window shows, where each result came from, what is never stored, and what it cannot see yet. The gaps are listed in full in [Known limits (G1 to G10)](#9-known-limits-g1-to-g10). Read them before you rely on Find for remote work.

> **Entry chord.** `Ctrl+Alt+F` is the only keyboard entry. There is no leader chord: `<L> ?` is already Help. `Ctrl+Alt+Shift+F` is accepted as a twin.

---

## 1. What It Finds

| Kind | Opened by | Searches | Rows |
|------|-----------|----------|------|
| **Agents** (`agent`) | `Ctrl+Alt+F`, `:find agent [text]`, View > Find Agents | Agents and panes that are open in this run | Agent rows (`agent:<session>/<pane>`) with state and time since the state changed |
| **Sessions** (`session`) | `Ctrl+Alt+F` then `Tab`, `:find session [text]`, View > Find Sessions | Live sessions first, then saved workspace sessions and history-only sessions on disk | Session rows (`session:<name>`) and the panes inside them (`pane:<session>/<pane>`) |
| **History** (`history`) | `Ctrl+Alt+F` then `Tab` `Tab`, `:find history [text]`, View > Find Command History | Commands captured in this run, then the `history.log` files on disk | History rows (`history:<session>/<seq>`): the command, its cwd when trusted, its exit code when known |

Rows are grouped in this order: **live**, then **saved session**, then **history log** (see [section 3](#3-reading-a-result-and-its-source)). Within a group the fuzzy score orders the rows, so a live match appears above a saved match with the same score.

An empty query lists everything in the kind, capped at 200 rows. Typing filters the list. The candidates are gathered once per open and re-ranked in memory, so typing does not re-read the disk.

---

## 2. Commands and the Reserved Words

`:find` has three **reserved first words**: `agent`, `session` and `history` (the plurals `agents` and `sessions` are also accepted). Any other `:find` argument keeps its old meaning, which is a search of the active pane's scrollback.

| Command | What it does |
|---------|--------------|
| `:find agent [text]` | Open the Find window on the Agents kind, with `text` as the query if given |
| `:find session [text]` | Open the Find window on the Sessions kind |
| `:find history [text]` | Open the Find window on the History kind |
| `:find <text>` | **Unchanged.** Scrollback search for `<text>`, exactly as before |
| `:find next` / `:find prev` / `:find off` | **Unchanged.** Step through or clear scrollback search highlights |
| `:search <word>` | **Unchanged.** The way to search the scrollback for a word that collides with a reserved first word |

The first word decides. `:find session of the day` opens the window with the query `of the day`. To search the scrollback for the word "session", use `:search session`.

Typing `:find agent` in the floating prompt and pressing `Enter` opens the window the same way `Ctrl+Alt+F` does. A query longer than 128 characters is cut to 128.

---

## 3. Reading a Result and Its Source

Every row carries a **source**: the origin, the name of the workspace session it belongs to, and, for saved data, the path it was read from. The source is shown in three places:

- **The row tag**, at the right edge of the row, is one of:
  - `[live]`: the row describes something open in this run.
  - `[saved:<name>]`: the row came from a saved workspace session file. `<name>` is the session name.
  - `[log:<name>]`: the row came from a `history.log` file. `<name>` is the session name.
- **The source line** under the list shows the selected row's source in full: `source: <origin> <name> <path>`. The origin is `live`, `saved-session` or `history-log`. `<path>` is shown for disk rows.
- **The status message** after `:find` summarises the result, for example `Find history "cargo": 2 results, 1 masked (1 live, 0 saved, 1 log)`. The breakdown counts the rows returned.

A worked example: `:find history cargo`, with one command typed in this run and one older command in the saved session `dev`.

```
Find history "cargo"
  cargo test -q                                 [live]
  cargo fmt --check                             [log:dev]
  1 masked                                       1/2
  source: history-log dev /home/me/.local/state/bilterm/sessions/dev/history.log
```

A session that is both live and saved appears once. The highest-priority origin (live, then saved, then log) is kept in `source`, and the others are listed in `also_in`.

The **masked** count is shown as `<n> masked` in the footer and in the status message. It is a number only. Masked text is never displayed, returned or logged. See [section 4](#4-privacy-what-is-never-stored-d-u2).

---

## 4. Privacy: What Is Never Stored (D-U2)

Find indexes commands, so the rules for what it keeps matter. They are summarised here; the shapes are listed under [Secret-looking commands](#secret-looking-commands).

**Never stored or indexed**, from this version onward:

- **Secret-looking commands.** A command that matches one of the shapes below is not written to `history.log` and does not enter the Find index. It is still kept in the in-memory prompt history, so `Ctrl+R` and command-block navigation work as before.
- **Password-prompt input.** Text you type when the pane is showing a password prompt (for example `[sudo] password` or `Password:`) is sent to the program and recorded nowhere. It is not in the prompt history, the command blocks, `history.log` or the Find index.

**Masked on read.** `history.log` files that already exist on disk are read through the same detection. A line that looks secret is dropped from the results and counted as `n masked`. The file is not rewritten.

**Known residual: saved-session files.** Secret-looking commands stay in the in-memory prompt history, so `Ctrl+R` can still find them. A saved-session file written by `:session save` or by autosave can therefore still contain such a line. Find masks those lines on read and counts them, but it does not change the file on disk. Edit or delete the saved file yourself if it must be clean.

**Detection is heuristic, not a guarantee.** It is a fixed list of patterns, not a secret scanner. It does not catch secrets in files you `cat`, in program output, in raw-mode input (gap G4), or in any shape not listed below. Plain words such as `grep -r password src/` or `git commit -m "rotate token"` are not treated as secrets.

### Secret-looking commands

A command is treated as secret-looking when it matches one of these shapes. Names and header names are matched case-insensitively. The values shown are placeholders.

| Shape | Example |
|-------|---------|
| An assignment whose name contains `password`, `passwd`, `secret`, `token`, `apikey`, `api_key`, `api-key`, `credential`, `private_key` or `access_key`, with a non-empty value | `export API_TOKEN=<value>` |
| A secret flag followed by a value or `=`: `--password`, `--passwd`, `--token`, `--secret`, `--api-key`, `--apikey`, `--auth-token`, `--client-secret`, `--access-key`, `--passphrase` | `curl --token=<value> …` |
| `-p<value>` attached, for `mysql`, `mariadb`, `mysqldump` or `mysqladmin`; or `sshpass -p <value>` | `mysql -p<value> db` |
| `Authorization:`, `Proxy-Authorization:` or `X-Api-Key:` headers, or `Bearer <token>` | `curl -H "Authorization: <value>" …` |
| URL user information `scheme://user:password@host` | `git clone https://user:<value>@host/repo` |
| Token shapes: `ghp_`, `gho_`, `ghs_`, `github_pat_` followed by enough characters; `sk-`; `AKIA` followed by 16 upper-case letters or digits; `xox[abprs]-`; `AIza`; a JSON Web Token (`eyJ….eyJ….`); a `-----BEGIN … PRIVATE KEY-----` block | (token-shaped strings) |
| `openssl … -passin pass:` or `-passout pass:`; `gpg --passphrase`; `chpasswd`; `htpasswd -b`; `echo` or `printf` piped into `sudo -S` | `echo <value> \| sudo -S ls` |

Known misses: `openssl -passin env:VAR`, secrets held in files, and any form not in this table. Known false positives: any `token=` or `password=` word, even inside a quoted message, and `Bearer` followed by eight or more characters in prose.

---

## 5. Keys and Mouse

The window is modal while it is open. Printable characters always edit the query. Every key below has a `:finder` twin (see [section 6](#6-finder-verbs)).

### Keys

| Key | Action | `:finder` twin |
|-----|--------|----------------|
| `Ctrl+Alt+F` | Open the window (Agents kind), or close it when it is open | `:find agent` / `:finder close` |
| `Tab` / `Shift+Tab` | Next / previous kind (Agents, Sessions, History) | `:finder kind next` / `:finder kind prev` |
| *printable* | Add a character to the query | `:finder query <text>` |
| `Backspace` | Delete the last query character | — |
| `Ctrl+U` | Clear the query | `:finder query` (no text) |
| `Up` / `Down` | Move the selection by one row | `:finder move -1` / `:finder move +1` |
| `PageUp` / `PageDown` | Move by 10 rows | `:finder move -10` / `:finder move +10` |
| `Home` / `End` | First / last row | `:finder move top` / `:finder move bottom` |
| `Enter` | Jump to the selected row (see [Jump semantics](#jump-semantics)) | `:finder enter` |
| `Ctrl+Y` | For a history row, put its command into the prompt (never runs it) | `:finder prefill` |
| `Esc` | Close the window | `:finder close` |

### Mouse

| Gesture | Action |
|---------|--------|
| View menu: **Find Agents (Ctrl+Alt+F)**, **Find Sessions**, **Find Command History** | Open the window on that kind |
| Click a kind tab `[Agents]` `[Sessions]` `[History]` | Switch kind |
| Click `[clear]` at the end of the query row | Clear the query |
| Click a row | Select it **and jump** to it. One click is enough |
| Hover over a row | Move the selection to it |
| Wheel over the window, between the tabs and the footer | Move the selection by 3 rows |
| Footer chips `[Enter jump]` `[Tab kind]` `[^Y prefill]` `[Esc close]` | The same as the key on the chip |

Typing has no mouse path; the mouse equivalents are the tabs, the `[clear]` chip and row clicks.

### Jump semantics

`Enter`, a row click and `:finder enter` do the same thing. What happens depends on the row:

- **Live agent or pane row:** the pane is focused and raised, and the window closes. If the pane has gone, the status says `pane #N is gone` and the window stays open.
- **Live command row (history):** the pane that ran the command is focused and scrolled to the newest matching command block. If that block has scrolled out of scrollback, the status says so and nothing else changes.
- **Saved or log command row:** the command is placed in the active prompt. It is **never run**: it is not sent to the PTY and not submitted, and the prompt history is unchanged. If the prompt is hidden, the status says so and nothing is placed.
- **Session row, live:** the window closes.
- **Session row, saved:** the saved session is switched to. The outgoing session is autosaved first, and the status says so (or says that autosave is off).
- **Session row, history only** (a `history.log` with no saved layout): the status says `session <name> has history only (no saved layout)` and the window stays open.
- **Pane row inside a saved session:** the session is switched to, then the pane is focused if its id is still present.
- **`metadata_only` row** (an unattached `:ssh-tmux` label): the pane is focused, and the status says that no remote tmux is attached.
- **No row selected:** the status says `Find: no result selected`.

---

## 6. `:finder` Verbs

`:finder` drives the window while it is open. These verbs have **no palette entry**: the palette closes before it runs a command, so a `:finder` verb run from there would have nothing to act on. They are the keyboard and scripting twins of the keys above.

| Verb | Meaning |
|------|---------|
| `:finder kind agent\|session\|history\|next\|prev` | Switch kind (`agents` and `sessions` are accepted) |
| `:finder query <text>` | Replace the query. With no text it clears the query |
| `:finder move <+n\|-n\|top\|bottom>` | Move the selection |
| `:finder select <n>` | Select row `n` (0-based) |
| `:finder enter` | Jump to the selected row |
| `:finder prefill` | Put the selected command into the prompt |
| `:finder close` | Close the window |

A bare `:finder` is a usage error. Query text is capped at 128 characters.

---

## 7. Automation: RPC, MCP and `bilterm ctl`

The Find search is also available to scripts and AI clients. These calls are **read-only**: they do not open the window, move focus or change the modal state, and they are not gated by `[api] allow_config_write`.

| Surface | Call | Parameters |
|---------|------|------------|
| JSON-RPC | `find.agents`, `find.sessions`, `find.history` | `query` (string, optional, capped at 128 characters); `limit` (integer, optional, default 50, range 1 to 200) |
| MCP | `bilterm_find` | `kind` (`agent`, `session` or `history`, required); `query`; `limit` |
| `bilterm ctl` | `bilterm ctl find.history '{"query":"cargo","limit":10}'` | The same parameters as RPC |
| `:find` data | `wm.command` with `:find history cargo` | Opens the window and stashes the same JSON as its data |

Every surface returns the same envelope. Each row carries its `source`, and the envelope carries `masked`. Values below are illustrative:

```json
{"kind":"history","query":"cargo","total":2,"returned":2,"truncated":false,"masked":1,
 "rows":[
   {"id":"history:main/12","kind":"history","row":"history","label":"cargo test -q",
    "detail":"cwd /home/me/src/app","score":812,
    "source":{"kind":"live","name":"main","path":null},"also_in":[],
    "transport":"local","host":null,"tmux_session":null,"tmux_window":null,"tmux_pane":null,
    "tags":[],"metadata_only":false,"pane":1,"session":"main",
    "seq":12,"command":"cargo test -q","exit_code":0,"duration_ms":1210},
   {"id":"history:dev/3","kind":"history","row":"history","label":"cargo fmt --check",
    "detail":"cwd /home/me/src/app","score":640,
    "source":{"kind":"history-log","name":"dev","path":"/home/me/.local/state/bilterm/sessions/dev/history.log"},
    "also_in":[],"transport":"local","host":null,"tmux_session":null,"tmux_window":null,
    "tmux_pane":null,"tags":[],"metadata_only":false,"pane":null,"session":"dev",
    "seq":3,"command":"cargo fmt --check","exit_code":null,"duration_ms":null}
 ]}
```

Field notes:

- `source.kind` is `live`, `saved-session` or `history-log`. `source.path` is `null` for live rows.
- `masked` is a count. Masked text never appears in `command`, `label`, `detail` or anywhere else.
- `exit_code` is `null` when no exit status was observed (see G3).
- `truncated: true` means more rows matched than were returned. `total` is the number that matched.
- `row` is `agent`, `session`, `pane` or `history`. The session kind returns both `session` and `pane` rows.

The MCP tool returns the same JSON as text. A missing or unknown `kind` is an invalid-parameters error (`-32602`).

---

## 8. SSH and tmux Coverage

Find sees what Bilterm itself did in a pane. For SSH and tmux that is less than the remote session holds. This table states what each case gives you today.

| Case | What Find shows | What Find does not show | Gap |
|------|-----------------|-------------------------|-----|
| A local pane, command typed in the command box | History row with `cwd` (OSC 7 reported, or the tracked local cwd) and the exit code when OSC 133 reports one | Commands typed in raw mode | P-01, G4 |
| `:ssh user@host` pane | Pane row with transport `ssh` and `host`. Commands typed in the command box are recorded with `host` and `transport: ssh` | The remote cwd and exit code, unless the remote shell itself reports OSC 7 and OSC 133. Raw-mode input | G3, G8, G4 |
| `:ssh-tmux host [session]` pane | Active pane with transport `ssh-tmux`, host, session, and live link state | When not connected, row is `metadata_only` | Resolved in Phase 51.1 (formerly G2) |
| `tmux -CC` bridge panes (transport `tmux`) | When a control stream is attached, the tmux session, window (`@2` or `@2:name`) and tmux pane | Multiple simultaneous links (one link at a time) | Resolved in Phase 51.1 (formerly G1, P-02) |
| A `tmux` or `ssh` you run by hand inside a local pane | Whatever the local shell recorded, as local commands | Input typed inside the program (raw mode). The pane is **not** an SSH pane | G4, G5 |
| Restored session with SSH panes | Local shell panes | The remote target is lost on restore | G6 |
| Headless daemon (`--headless`) | Rows for the panes the daemon holds | Live SSH or tmux PTYs: the daemon holds none | G7 |

`pane` ids are unique only within a run, so a saved row's `pane` is not a jump target (G6). Use `session` and `id` to identify saved rows.

---

## 9. Known Limits (G1 to G10)

These are the documented gaps of the current build.

- **G1: Resolved in Phase 51.1.** The `tmux -CC` bridge is attached via `:ssh-tmux <user@host> [session]` with persistent auto-reconnect and hydration. Only one tmux link at a time in this version.
- **G2: Resolved in Phase 51.1.** `:ssh-tmux` spawns a real PTY-backed `ssh -tt` session running `tmux -CC` with keepalive options, login-shell wrapper, interactive host-key/password prompts, and automatic reconnect with exponential backoff.
- **G3: Remote OSC 133 and OSC 7 are never requested.** Bilterm does not send a shell-integration bootstrap to a remote host. A remote exit code and cwd need a remote shell that already emits OSC 133 (exit status) and OSC 7 (cwd).
- **G4: Raw-mode input is not recorded.** Input in `INPUT: RAW PTY` mode, input to full-screen programs, and remote shell input that bypasses the command box are not captured. Anything typed before attach is not recorded either.
- **G5: A local pane where you typed `ssh host` is not an SSH pane.** Find does not infer a transport from command text.
- **G6: `pane_id` is unique per run only, and journal restore loses `remote_target`.** A restored SSH pane becomes a local shell.
- **G7: The headless daemon holds no live SSH or tmux PTYs.**
- **G8: A remote pane's cwd is shown only when OSC 7 reported it.** For an SSH pane the local directory is never used as its cwd.
- **G9: The tmux format variables `pane_command_*` are not used.** They need a recent tmux, so the running command of a tmux pane is not reported this phase.
- **G10: A tmux window name is known only after a `%window-renamed` notification.** There is no `list-windows` reply to correlate with.

---

## 10. Bounds

| Bound | Value | Applies to |
|-------|-------|------------|
| Captured commands kept in memory | 2048 (ring) | This run's command ring |
| Characters per stored command | 512 | Every stored command (display-safe copy) |
| Characters per name, label, title or host | 64 | Labels in every row |
| Rows returned or rendered | 200 | Maximum `limit`; the window list |
| Default `limit` | 50 | RPC and MCP |
| Characters per query | 128 | Typed query, `:find` argument, RPC and MCP `query` |
| Newest `history.log` lines read | 20 000 | Each `history.log` read |
| `history.log` tail window | 16 MiB | Larger files are read from the tail |
| Saved file size limit | 2 MiB | Each saved session file (larger files are skipped) |
| Saved sessions and history directories inspected | 64 | Disk scan |
| Panes per session | 64 | Pane rows of each session |
| Queued captured commands per pane | 64 | Between two event-loop ticks |

Symbolic links and non-regular files in the session and history directories are not followed. Session names that fail validation are skipped. Oversized and unreadable files are skipped and do not stop the search.

---

## See Also

- [Chapter 6: Keyboard, Mouse & Command Reference](06-keyboard-mouse-and-command-reference.md): the `:find` and `:finder` rows.
- [Chapter 5: tmux Integration](05-tmux-integration.md): the tmux modes and their limits.
- [Chapter 10: Security Architecture](10-security-and-best-practices.md): filesystem confinement and RPC security.
- [API reference](../API.md): the `find.*` methods and the `bilterm_find` tool.

---

[← Chapter 11: Navigator & Agent Traffic](11-navigator-sidebar-and-agent-traffic.md) | [User Guide Home](README.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)
