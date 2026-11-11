# Chapter 13: The Traffic Monitor

The TRAFFIC MONITOR is a floating, pinnable, dockable utility window that shows how many **bytes**
cross every I/O boundary of Bilterm: shell PTYs, `ssh` and `tmux -CC` links, the JSON-RPC, HTTP,
WebSocket and MCP surfaces, the host terminal and the clipboard helper. It never stores or shows
payload bytes, only counts and sanitised labels.

> Not to be confused with **Agent Traffic** (`:agents traffic`, chapter 11), which shows who messaged
> whom. The Traffic Monitor shows bytes.

## 1. Concepts

- **Wire vs Payload.** A *wire* channel is the physical link (an ssh connection, a socket). A *payload*
  channel is the decoded stream of one pane over that link (for example a tmux pane). Connection,
  Origin and Locality groups count wire channels; Session, Window and Pane groups count payload
  channels. Nothing is double counted.
- **Locality.** `L` local (PTY, tmux subprocess, clipboard helper), `LB` loopback (RPC, HTTP, MCP on
  127.0.0.1) and `NET` network (ssh, remote tmux, remote web clients, STT providers).
- **Groups.** Connection, Session, Window, Pane, Origin, Locality, Direction (rx and tx rows).
- **Columns.** NAME, LOC, RX/s, TX/s, 10s, 1m (combined rates), TOT RX, TOT TX, PEAK (best second in the
  last minute), ESC% (escape bytes as a share of rx, live while the window is open), EV/s, SPARK
  (last seconds, one bar per second). Narrow windows drop the least important columns first.

## 2. Opening it

| Surface | How |
|---------|-----|
| Key | `Ctrl+Alt+T` (global, also in raw passthrough and over alt-screen apps), Leader `Ctrl+X $` |
| Menu | View > Traffic Monitor (Ctrl+Alt+T) |
| Colon | `:traffic`, `:traffic toggle`, `:traffic open`, `:traffic close`, alias `:netmon` |
| API | `traffic.window {action:"open"}`, MCP `bilterm_traffic` with `action:"window"` |

(`Ctrl+X #` already means ArrangeGrid, so the leader chord is `$`.)

## 3. Commands

- `:traffic pin` / `:traffic unpin` keep the window open when it loses focus.
- `:traffic dock` / `:traffic float` tile it into the `:arrange` strip or release it; moving or
  resizing releases the dock.
- `:traffic group <connection|session|window|pane|origin|locality|direction>` (also `next`, `prev`).
- `:traffic sort <rate|rate10|rate1m|total|peak|name> [asc|desc]`; `:traffic sort reverse` flips.
- `:traffic rate <1s|10s|1m>` highlights and sorts by a rate column.
- `:traffic filter <text>` (max 64 characters, control characters removed), `:traffic clear-filter`.
- `:traffic locality <all|local|loopback|network>` (or `next`).
- `:traffic pause` freezes the table, `:traffic resume` resumes. Paused tables show `PAUSED`.
- `:traffic drill` descends into the selected row (Connection to Pane, Origin to Connection, Session
  to Window); `:traffic up` returns, with a history of up to 4 levels.
- `:traffic show [n]` stores the table as structured data (identical to RPC `traffic.snapshot`).
- `:traffic reset` zeroes counters; `:traffic enable` / `:traffic disable` switch metering;
  `:traffic classify on|off` toggles escape classification; `:traffic oia on|off` toggles the status
  bar indicator.
- `:traffic scale <75..200>` sets the GUI scale; `:traffic move <dx> <dy>`, `:traffic resize <dw> <dh>`,
  `:traffic rect <x> <y> <w> <h>`, `:traffic size auto` place the window.

## 4. In-window keys

`Tab`/`BackTab` group, `s` sort key, `S` direction, `1` `2` `3` rate column, `l` locality, `p` pause,
`/` filter (type, `Enter`/`Esc` to leave), `Enter` drill, `Backspace` up, `x` reset, `o` OIA,
`c` classify, `P` pin, `D` dock, `j k Up Down PgUp PgDn Home End` navigate, `q` closes, `Esc` hands the keyboard back to the pane and leaves the window open
(`Ctrl+Alt+O` cycles the keyboard between the pane and the open side windows).
`Shift+arrows` move and `Alt+arrows` resize.

## 5. Mouse

Title-bar badges `[o][□][×]` pin, dock, close; click a group tab, a column header (click again to
flip), a locality chip, a row (click the selected row to drill), the footer chips; wheel scrolls
(`Ctrl`+wheel scales in the GUI); drag the title to move and any border or corner to resize.

## 6. RPC and MCP

`traffic.snapshot {group_by, sort, descending, locality, filter, window, limit}` returns
`{at_ms, enabled, totals, rows[], truncated}`; `traffic.history {key, group_by, n}`;
`traffic.window {action, value}`; `traffic.reset`; `traffic.config {set:{enabled|classify|oia_indicator}}`.
MCP exposes all of them as the single tool `bilterm_traffic` (`action`: `snapshot`, `history`,
`window`, `reset`, `config`). `bilterm ctl traffic.snapshot` works from the shell. The MCP stdio
loop counts its own bytes on an `mcp` channel.

Mutating verbs (`reset`, `enable`, `disable`, `classify`, `oia`, and `traffic.config set`) are refused
for API callers unless `[api] allow_config_write = true`; local callers are never gated.

## 7. Privacy

Only counters and sanitised labels (max 48 characters) are kept. Host names appear as `ssh:<host>:<port>`;
set `[traffic] label_hosts = false` to show `host-<n>` instead. No payload is ever stored.

## 8. Surface uniformity

| Capability | Mouse | Key | Command | API |
|------------|-------|-----|---------|-----|
| Open / close | menu, `[×]` | `Ctrl+Alt+T`, `Ctrl+X $`, `q` | `:traffic toggle` | `traffic.window` |
| Pin / dock | `[o]` `[□]` | `P` `D` | `:traffic pin`, `:traffic dock` | `traffic.window` |
| Group | tabs | `Tab` | `:traffic group` | `traffic.snapshot group_by` |
| Sort | header click | `s` `S` | `:traffic sort` | `traffic.snapshot sort` |
| Filter | filter box, chips | `/` `l` | `:traffic filter`, `:traffic locality` | `traffic.snapshot` |
| Pause / drill | chips, row click | `p` `Enter` | `:traffic pause`, `:traffic drill` | `traffic.window` |
| Reset / config | footer chip | `x` `c` `o` | `:traffic reset` | `traffic.reset`, `traffic.config` (gated) |
| Read | row selection | | `:traffic show` | `traffic.snapshot`, `traffic.history` |

Exceptions: geometry verbs have no palette row; the gated verbs above.
