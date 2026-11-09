# Chapter 11: The Navigator Sidebar & Agent Traffic Window

[← Chapter 10: Security & Best Practices](10-security-and-best-practices.md) | [User Guide Home](README.md)

The **Navigator** is an optional style of the slide-out drawer (`F7` / `:drawer`). Instead of the classic telemetry rows it shows a compact workspace > session > pane tree where every pane carries a live **agent state dot** (working, blocked, idle, done, errored), per-workspace counts, and a small **communication glyph** when a pane is talking to another pane, to you, or to the automation API. From any pane row you can open the **AGENT TRAFFIC window**: a node graph of who talks to whom, next to a conversation (snoop) view of what was sent.

The Navigator changes nothing about the classic drawer: with the default style the drawer renders and behaves exactly as before.

> The Navigator was previously called the "herdr sidebar". `herdr` remains only as a hidden, deprecated alias (`drawer.style = "herdr"`, `herdr_width_cols`, `:drawer style herdr`).

## Table of Contents

1. [Enabling the navigator sidebar](#enabling-the-navigator-sidebar)
2. [Reading the sidebar](#reading-the-sidebar)
3. [Keyboard](#keyboard)
4. [Mouse](#mouse)
5. [Commands](#commands)
6. [Communication indicators](#communication-indicators)
7. [Agent traffic window](#agent-traffic-window)
8. [Agents list window](#agents-list-window)
9. [Tiling floating panels](#tiling-floating-panels)
10. [Command palette coverage](#command-palette-coverage)
11. [Surface uniformity](#surface-uniformity)
12. [GUI rendering](#gui-rendering)
13. [Assumptions and limits](#assumptions-and-limits)

---

## Enabling the navigator sidebar

The default stays `classic`. Switch to the Navigator in any of three ways:

```toml
# ~/.config/bilterm/config.toml
[drawer]
style = "navigator"          # "classic" (default) or "navigator"
navigator_width_cols = 28    # 16-100, default 28
row_lines = 2                # pane rows: 2 (default) = title + user@host path line, 1 = compact
```

```text
:drawer style navigator      # switch now (also: classic, toggle)
:set drawer.style navigator  # same through the generic config path
:drawer rows 1               # compact single-line pane rows (2 restores the two-line layout)
```

Then open the drawer with `F7`, `Ctrl+X d`, or `:drawer`. `:drawer focus` gives it the keyboard. The width is `navigator_width_cols`; change it with `{` / `}`, `:drawer widen <+-n>`, or by dragging the drawer edge. See [Chapter 4](04-themes-glyphs-and-config.md#7-declarative-configuration-configbilterm-configtoml-cfg-01) for the config tables and `[agents.<name>]` profiles.

## Reading the sidebar

The tree has three levels. Workspace and session rows take one line; with the default `row_lines = 2` each pane row takes two (set `row_lines = 1` or run `:drawer rows 1` for one line per item):

```text
▾ work                 ⠋2 ▲1 ○3      workspace row, with state counts
  ▾ build              ⠋2            session row
    ⠋ #3 cargo watch   ⇄you          pane row, line 1: dot, #id, title, comms glyph
      bill@dev  ~/src/app            pane row, line 2: user@host  path
  › ▲ #5 codex         ⋔3
      bill@dev  ~/src/api
▸ notes                ○1            folded workspace
```

- **Second line**: a shell-prompt style `user@host  path`, indented under the `#id`. A local pane shows your login, the short host name and the pane's live working directory (`~` for your home). A pane attached to a remote host (`:ssh`, `:ssh-tmux`) shows that destination and only the directory its shell reported over `OSC 7`. A pane that exposes neither shows no second line. The whole two-line row is one click target and is highlighted as a unit; the dot, `[+]`, fold marker and comms glyph stay on line 1. Classic drawer style ignores `row_lines`.

- **Fold markers**: `▾` open, `▸` folded (ASCII pack `-` / `+`). Workspaces are the roots of the session tree; saved (not running) sessions appear with a pane rollup.
- **Current pane**: marked with `›` (ASCII `>`).
- **Workspace counts**: non-zero states per workspace, in the order blocked, errored, working, done, idle.
- **Titles** are sanitised and truncated to fit the width; the comms glyph is never truncated before the title and partner label.

State dot per pane (derived only from signals Bilterm already tracks; no agent needs to cooperate):

| State | Unicode | ASCII | Colour token | Meaning |
| :--- | :---: | :---: | :--- | :--- |
| working | animated braille spinner (`⠋ ...`) | `*` | success | a command is running and produced output in the last 1.5 s, or a profile `working` pattern is on screen |
| blocked | `▲` | `!` | warning | password / `!ECHO LOCK` prompt, profile `blocked` pattern on screen, or an unread notification in an unfocused pane |
| idle | `○` | `-` | muted | nothing running, or a running command that has been silent for more than 1.5 s |
| done | `●` | `+` | accent | the pane exited with code 0, or a command finished successfully and you have not looked yet |
| errored | `✗` | `x` | danger | non-zero exit, or a failed command you have not looked yet |

Blocked, errored and done are the **attention** states. `!` (key, footer button, or leader `!`) jumps to the next pane needing attention: blocked first, then errored, then done.

A footer button bar at the bottom of the drawer (when it is at least 4 rows tall) shows `* ! N + r x @ agents`. Each label is the key of the same character, and the last button is labelled `@ agents` so the Agent Traffic window is easy to find: it **toggles** the window (open on the selected pane row, close when open). Buttons that do not fit are dropped from the right (the `@ agents` text needs a 24-column inner width). See [Mouse](#mouse).

## Keyboard

Leader chords (`<L>` is `Ctrl+B` or `Ctrl+X`; inside an outer `$TMUX` only `Ctrl+X`):

| Chord | Command | Effect |
| :--- | :--- | :--- |
| `<L> !` | `:drawer attention` | Focus the next pane needing attention |
| `<L> a` | `:agents zoom` | Open the AGENT TRAFFIC window |
| `Ctrl+Alt+A` | `:agents toggle` | Open the AGENT TRAFFIC window, or close it when open (all of `F1`..`F12` are already bound, so this is a global chord) |
| `Ctrl+Alt+N` | `:agents list toggle` | Open the AGENTS list window, or close it when open. Works while a full-screen program owns the pane (the same reserved path as `Ctrl+Alt+A`) |
| `<L> A` | `:agents list toggle` | The same toggle as a leader chord, for hosts that swallow `Ctrl+Alt+N` |
| `<L> y` | `:agents demo` | Start the agent traffic demo (see [Agent traffic window](#agent-traffic-window)) |
| `<L> Y` | `:agents demo stop` | Stop the demo: close only the panes it created and remove its traffic |

### Navigator drawer (drawer focused, `drawer.style = navigator`)

Table `NAVIGATOR_DRAWER_BINDINGS` in `src/input/keymap.rs`. Keys it does not claim keep their classic meaning (`q`, `*`, `[`, `]`, `d`, `m`, `p`, `J`/`K`, ...). `a` and `A` shadow the classic arrange keys in navigator style only (`:arrange grid` and `:arrange stack-grid` stay available).

| Key | Operation |
| :--- | :--- |
| `j` / `Down`, `k` / `Up` | Next / previous row |
| `Enter`, `Space` | Focus the selected pane (or fold a workspace/session row) |
| `!` | Jump to the next pane needing attention |
| `e` | Fold / unfold the selected session |
| `E`, `C` | Expand all / collapse all (a second `C` on a fully folded tree unfolds) |
| `N` | New session |
| `n`, `c`, `+`, `w` | New pane (split the focused pane) |
| `a` | Add a window to the selected session |
| `A` | Split the selected pane |
| `r`, `R` | Rename the selected session (opens the prompt) |
| `x`, `Delete` | Close the selected pane |
| `PageUp` / `PageDown` | Move the selection by -5 / +5 rows |
| `{` / `}` | Narrow / widen the drawer by 2 columns |
| `Right` | Focus the comms glyph of the selected row (only on a row that shows one) |
| `o` | Open the traffic window on the selected row's conversation |

While the comms glyph has focus: `Enter` opens the traffic window on it, `Left` or `Esc` releases it.

### Agent traffic window (while the window has the keyboard)

The window is not modal. It takes the keyboard when it opens and whenever you click inside it; a click on a pane, focusing another pane (leader keys, `Alt+Tab`) or `:agents blur` gives the keyboard back while the window stays open. Table `AGENTS_WINDOW_BINDINGS`, by scope. The leader key is never consumed. Keys the table does not claim are swallowed only while the window has the keyboard.

| Scope | Keys | Operation |
| :--- | :--- | :--- |
| Any | `Esc` | Close the window (when pinned: release the keyboard only) |
| Any | `Shift+arrows` | Move the window one cell |
| Any | `Alt+arrows` | Resize the window one cell |
| Any | `*` | Pin / unpin |
| Any | `[` / `]` | Panel scale -10 / +10 (GUI surface) |
| Any | `m` | Maximize / restore |
| Any | `<` / `>` | Move the graph/conversation splitter -5 / +5 percent |
| Any | `Ctrl+O` | Cycle the keyboard: graph, conversation, compose (`F6` keeps its global deck meaning) |
| Graph | arrows | Move the node cursor |
| Graph | `Tab` / `Shift+Tab` | Cycle the selected node's edges |
| Graph | `Enter` | Focus the selected pane node and leave the window |
| Graph | `g` | Grab the selected node (see below) |
| Graph | `u` | Unpin the selected node |
| Grab | arrows | Nudge the node one cell (pins it) |
| Grab | `g`, `Enter`, `Esc` | Drop the node |
| Conversation | `j` / `k`, `Down` / `Up` | Select next / previous line |
| Conversation | `y` | Copy the selected line |
| Conversation | `PageUp`, `PageDown`, `Home` | Scroll the history |
| Conversation | `End`, `f` | Follow new lines (`End` sets follow, `f` toggles) |
| Conversation | `a` / `p` | Filter to the selected line's agent / pair |
| Conversation | `/` | Keyword entry (type, `Enter` applies, `Esc` cancels, `Backspace` edits) |
| Conversation | `c` | Clear all filters |
| Conversation | `A` / `P` / `T` | Remove the agent / pair / text filter |
| Compose | `Enter` | Submit (disabled, see [Assumptions and limits](#assumptions-and-limits)) |

## Mouse

### Navigator drawer

| Gesture | Operation |
| :--- | :--- |
| Click a row | Select and focus that pane |
| Click a fold marker | Fold / unfold that workspace or session |
| Click `[+]` after a workspace/session name | Add a window to it; `[+]` beside the `WORKSPACES` title creates a session |
| Right-click a row | Select it and open a menu at the pointer: New session, New window here, Split horizontal, Split vertical, Rename, Close. `Esc`, any key or a click elsewhere dismisses it; right-click outside the drawer still pastes |
| Footer buttons `* ! N + r x @` | Pin, jump to attention, new session, new pane, rename, close pane, open the traffic window on the selected pane |
| Title badges | Minimize / maximize / close the drawer |
| Wheel | Move the selection by 3 rows |
| Drag the drawer edge | Resize (`navigator_width_cols`) |
| `Ctrl` + wheel over the drawer | Scale the drawer panel (GUI surface only, see [GUI rendering](#gui-rendering)) |
| Click the comms glyph | Open the traffic window on that conversation |

### Agent traffic window

| Gesture | Operation |
| :--- | :--- |
| Drag the title bar | Move the window |
| Drag an edge or corner | Resize (the top edge is the title bar; its four corners resize) |
| Title badges `[□]` / `[X]` | Maximize / close |
| Pin badge `◇` / `◆` in the title | Pin / unpin the window |
| Drag the splitter | Change the graph/conversation split (20 to 80 percent) |
| Click a node | Select it; a second click on a pane node focuses that pane and leaves the window |
| Drag a node | Move and pin it at that position |
| Right-click a node | Unpin it |
| Click an edge | Select it; `Enter` or a second click filters the conversation to that pair and focuses it |
| Click a conversation line | Select it |
| Wheel over the conversation | Scroll the history (scrolling up stops following) |
| Header buttons, chips, follow marker | Agent / pair / keyword filters, clear, follow; a chip's `×` removes that filter |
| `[copy]` | Copy the selected line |
| `[send]` | Press the (disabled) compose line; nothing is sent |
| Click outside the window | Release the keyboard; the click goes on to the pane under it |
| Click inside an unfocused window | Take the keyboard back |
| `Ctrl` + wheel over the window | Scale this window (GUI surface only) |

Mouse events over the window never reach the drawer or panes below it; events outside the window pass through to the panes as usual. The `:mcp` console is drawn above the window, but the window sees mouse events first, so an MCP panel overlapping the window receives no clicks in the overlap.

When both floating windows are open, the AGENTS list window handles mouse events before the traffic window. An **open menu-bar dropdown outranks both**: a click on an item it covers runs that item, and neither window takes the click.

## Commands

All of these lower to the same internal action as their key and mouse twin. `:navigator` is an alias of `:drawer` for the verbs below. **`:drawer close` still minimizes the drawer** (classic meaning, unchanged); **`:drawer close-pane` closes the selected pane**. `:drawer expand` without `all` still maximizes the drawer.

### `:drawer` verbs (Navigator)

| Command | Key | Mouse | Effect |
| :--- | :--- | :--- | :--- |
| `:drawer attention` | `!`, `<L> !` | footer `!` | Next pane needing attention |
| `:drawer fold` | `e` | click fold marker | Fold / unfold the selected session |
| `:drawer enter` | `Enter`, `Space` | click row | Focus the selected row |
| `:drawer next` | `j`, `Down` | wheel | Select next row |
| `:drawer prev` | `k`, `Up` | wheel | Select previous row |
| `:drawer row <n>` | | click row n | Select row n (1-based) |
| `:drawer scroll <+-n>` | `PageUp` / `PageDown` | wheel (3 rows) | Move the selection n rows |
| `:drawer widen <+-n>` | `{` / `}` | drag edge | Resize the drawer |
| `:drawer new-session` | `N` | footer `N`, `[+]` by title, menu | New session |
| `:drawer new-pane` | `n` `c` `+` `w` | footer `+` | Split the focused pane |
| `:drawer rename [name]` | `r`, `R` | footer `r`, menu | Rename the selected session (prompt without a name) |
| `:drawer close-pane` | `x`, `Delete` | footer `x`, menu | Close the selected pane |
| `:drawer expand all` | `E` | | Unfold everything |
| `:drawer collapse all` | `C` | | Fold to workspace rows (again unfolds) |
| `:drawer add window [session]` | `a` | `[+]`, menu | Add a window to the selected or named session |
| `:drawer add pane [h\|v]` | `A` | menu | Split the selected pane |
| `:drawer menu` | | right-click row | Open the context menu |
| `:drawer menu close` | `Esc` | click away | Close the menu |
| `:drawer menu pick <n>` | | click entry | Run menu entry n |
| `:drawer comms` | `o`, `Enter` on glyph | click glyph | Open the traffic window on the selected row's conversation |
| `:drawer comms-focus on\|off` | `Right` / `Left`, `Esc` | | Focus / release the comms glyph |
| `:drawer style classic` | | | Classic drawer style (telemetry rows) |
| `:drawer style navigator` | | | Navigator drawer style (the workspace > session > pane tree) |
| `:drawer style [classic\|navigator\|toggle]` | | | Switch drawer style (no argument or `toggle` flips it) |
| `:drawer rows 1` | | | One line per pane row in Navigator style (`drawer.row_lines = 1`) |
| `:drawer rows 2` | | | Two lines per pane row in Navigator style, the default (title plus `user@host  path`) |
| `:drawer minimize` | | title minimize badge | Minimize the drawer (size 0), the same as `:drawer close` |
| `:drawer maximize` | | title maximize badge | Maximize the drawer, the same as `:drawer expand` without `all` |
| `:drawer pin` | | | Fixed visibility: the drawer stays open |
| `:drawer unpin` | | | On-demand visibility: the drawer hides when it is not needed |
| `:drawer mode float` / `push` | | | Overlay (drawn over the panes) or Push (reflows the panes); `mode` with no argument toggles |
| `:drawer edge left` / `right` / `top` / `bottom` | | | Dock the drawer to an edge, the same as `:drawer left`, `right`, `top`, `bottom` |
| `:drawer preview` / `:drawer preview toggle` | `v` (Navigator), `P` (Classic), `Alt+P`, `Ctrl+Alt+P`, `<L> P` | hover row (120 ms), middle-click row, footer `[peek]` | Toggle non-destructive window peek preview |
| `:drawer preview on` / `:drawer preview off` | `Esc` (off) | mouse-leave drawer (off) | Enable or dismiss the active peek preview |
| `:drawer preview commit` | `Enter` | left-click row | Commit the previewed pane or session as the active focus |
| `:drawer preview auto` | | | Toggle hover-to-preview auto mode (`drawer.preview_on_hover`) |
| `:drawer preview row` / `:drawer preview pane` / `:drawer preview session` | | | Preview a specific 1-based drawer row, pane id (`:drawer preview pane <id>`), or session name (`:drawer preview session <name>`) |

`:drawer size`, `toggle`, `focus`, the dock edges and the pinning modes are unchanged; see [Chapter 3](03-window-manager-and-z-stack.md).

### `:agents` verbs

| Command | Key | Mouse | Effect |
| :--- | :--- | :--- | :--- |
| `:agents list` | | | Agent state per pane (data over RPC, see [API](../API.md)). It also opens the AGENTS list window; bare `:agents`, `:agents ls` and `:agents status` do the same |
| `:agents list open` / `close` / `toggle` | `Ctrl+Alt+N`, leader `A` | menu View > Agents List; title `[X]` | Open, close or toggle the AGENTS list window |
| `:agents list dismiss` | `Esc` | title minimize badge | Close, or release the keyboard when pinned |
| `:agents list blur` | | click outside the window | Release the keyboard without closing |
| `:agents list select up` / `down` / `first` / `last` / `page-up` / `page-down` | `Up`/`k`, `Down`/`j`, `Home`, `End`, `PageUp`, `PageDown` | wheel (scrolls) | Move the row selection |
| `:agents list select <pane-id>` | | click a row | Select a row by pane id (`#3` is accepted) |
| `:agents list jump [pane-id]` | `Enter` | click the selected row again | Focus the pane and leave the list |
| `:agents list zoom [pane-id]` | `z` | right-click a row, chip `[zoom]` | Open the traffic window on the pane |
| `:agents list pin` / `:agents list unpin` / `:agents list pin toggle` | `*` (toggle) | title pin badge | Pin, unpin, or toggle the pin on the list window |
| `:agents list scale +n` / `-n` | `]` / `[` (+10 / -10) | `Ctrl` + wheel (±10) | Step the window's GUI scale |
| `:agents list scale <75..200>` | | | Set the window's GUI scale to an absolute value (colon and API only) |
| `:agents list filter all` / `agents` / `toggle` | `f` (toggle) | chip `[filter]` | Show every tracked pane, only panes that match an agent profile or report a state, or toggle |
| `:agents list dock` / `dock toggle` / `float` | `d` (toggle) | title dock badge, chip `[dock]` | Dock into the tiling strip, toggle, or float the window |
| `:agents list move <dx> <dy>` | `Shift`+arrows | drag the title bar | Move the window by cells |
| `:agents list resize <dw> <dh>` | `Alt`+arrows | drag an edge or corner | Resize the window by cells |
| `:agents list rect <x> <y> <w> <h>` | | | Set the window rectangle (clamped to the workspace) |
| `:agents list size <cols>x<rows>` / `size auto` | | | Set the window size, or the default size |
| `:agents list scroll <±n>` | | wheel | Scroll the row list by n rows (`PageUp`/`PageDown` page it) |
| `:agents zoom [pane-id]` | `<L> a` | footer `@`, comms glyph | Open the traffic window (optionally on a pane) |
| `:agents toggle [pane-id]` | `Ctrl+Alt+A` | menu View > Agent Traffic, footer `@ agents` | Open the window (optionally on a pane), or close it when it is open |
| `:agents close` | | title `[X]` | Close the window |
| `:agents dismiss` | `Esc` | | Close, or release the keyboard when pinned |
| `:agents blur` | | click a pane | Release the keyboard without closing (the window is not modal) |
| `:agents select <left\|right\|up\|down\|node-key>` | arrows | click node | Move the node cursor, or select a node such as `pane:2` |
| `:agents edge <next\|prev\|a b>` | `Tab` / `Shift+Tab` | click edge | Cycle the node's edges, or select the edge between two nodes |
| `:agents enter` | `Enter` | second click on a node | Focus the selected pane and leave the window |
| `:agents move <dx> <dy>` | `Shift+arrows` | drag title | Move the window |
| `:agents resize <dw> <dh>` | `Alt+arrows` | drag edge/corner | Resize the window |
| `:agents rect <x> <y> <w> <h>` | | | Set the window rectangle (clamped to the workspace, minimum 40x12) |
| `:agents size <cols>x<rows>\|auto` | | | Set the window size, or the default centred size |
| `:agents max` | `m` | title `[□]` | Maximize / restore |
| `:agents split <20..80\|+n\|-n>` | `<` / `>` | drag splitter | Graph/conversation split |
| `:agents grab [on\|off]` | `g` | | Grab mode for the selected node |
| `:agents nudge <left\|right\|up\|down>` | arrows in grab mode | | Move the selected node one cell and pin it |
| `:agents node-pin <node-key> <x> <y>` | | drag node | Pin a node at a permille position (0..1000) |
| `:agents node-unpin [node-key]` | `u` | right-click node | Release a pinned node |
| `:agents pin [toggle]` | `*` | pin badge | Pin the window open |
| `:agents unpin` | `*` | pin badge | Unpin |
| `:agents scale <75..200\|+n\|-n>` | `[` / `]` | `Ctrl` + wheel | Panel scale of the window (GUI surface) |
| `:agents focus [graph\|snoop\|compose]` | `Ctrl+O` | click a pane | Move the keyboard |
| `:agents line <+n\|-n\|seq>` | `j` / `k` | click a line | Select a conversation line |
| `:agents copy` | `y` | `[copy]` | Copy the selected line |
| `:agents scroll <±n\|top>` | `PageUp`, `PageDown`, `Home` | wheel | Scroll the history |
| `:agents follow [on\|off]` | `f`, `End` | follow marker | Follow new lines |
| `:agents fade [secs]` | | | Seconds (1..3600, default 60) for an edge to fade to its floor; no argument shows it. Same as `agents_window.edge_fade_secs` |
| `:agents demo` | `<L> y` | menu View > Agent Demo | Fill the window with synthetic `[demo]` pane-to-pane events (splits to three panes if needed; memory only, nothing is sent to any pane) to try the graph and conversation. Running it again replaces the earlier demo events and demo topology pairs |
| `:agents demo stop` | `<L> Y` | menu View > Stop Agent Demo | Close only the panes the demo created, remove its `[demo]` events and its topology pairs. Not behind `[api] allow_config_write` over the API |
| `:agents topology` | | | Which pairs communicated this session (data over RPC / MCP) |
| `:agents topology export <name>` | | | Write the topology to `<name>.json` in the private runtime directory (local only) |
| `:agents filter agent [label]` | `a` | header button | Filter to a node (no label: the selected line's sender) |
| `:agents filter pair [a b]` | `p` | `Enter`/click on an edge | Filter to a pair (no nodes: the selected line's pair) |
| `:agents filter text <keyword>` | `/` then type | header `/` button | Case-insensitive keyword filter (capped at 64 characters) |
| `:agents filter clear` | `c` | header clear button | Remove all filters |
| `:agents filter remove <agent\|pair\|text>` | `A` / `P` / `T` | chip `×` | Remove one filter |

Filters are combined with AND; there is no regex. There is deliberately no `:agents send` or `compose` command.

## Communication indicators

Each live pane row shows who it is exchanging traffic with, right-aligned:

| Glyph | ASCII | Meaning |
| :---: | :---: | :--- |
| `⇄you` | `<>you` | exactly one partner within the last 60 s (partner label, up to 6 characters) |
| `⋔3` | `>>3` | several partners (the count) |

While the last event is under 1.5 s old the pairing glyph alternates `⇄` / `⇆` (ASCII `<>` / `><`), and the fan-out glyph pulses; the drawer redraws at most every 120 ms for this. Partners are other panes, `you` (input broadcast), `api` (automation API: `pane.exec`, `pane.send_keys`), or `mcp` (the `:mcp` Master Control Program console).

Click the glyph, press `o`, press `Right` then `Enter`, or run `:drawer comms` to open the traffic window on that conversation: a pairing opens on the edge with the pair filter applied, a fan-out selects the pane with an agent filter. On a row with no indicator these just select the pane.

## Agent traffic window

Open it with `<L> a`, `Ctrl+Alt+A`, `:agents zoom`, `:agents toggle`, the top menu (View > Agent Traffic), the footer `@ agents` button, or a comms glyph; `Ctrl+Alt+A`, `:agents toggle`, the menu entry and the footer button close it again when it is open. It is an ordinary, non-modal window: it floats above the panes and drawer and below the `:mcp` console, other panes and keys keep working while it is open, and it only receives keys while it has focus (see above), default 80% x 70% of the workspace (minimum 40x12, nothing is drawn below 30x10).

**Graph (left).** One node per live pane plus the endpoints seen on the bus (`you`, `api`, `mcp`, `session:<name>`); labels are `you`, `#<id>`, `@<session>`. Edges are drawn as curves between nodes that exchanged traffic; nodes are coloured by agent state. An edge traces the real conversation: **the more often and the more recently a pair talked, the brighter its edge.** Each event adds a weight that falls linearly from 1 (now) to 0 (one fade window ago), and the brightness is that sum `s` mapped through `s / (s + 1.5)`, so a chatty pair glows and a one-off message is dim. After the fade window (`:agents fade`, default 60 s) the edge decays to a fixed floor but **never disappears once the pair has spoken**. The terminal (braille) and GUI (vector) renderers use the same value; the selected edge keeps the accent colour and edges that carried one of the newest events also get flow particles in the GUI. The layout is deterministic and stable: a topology that has not changed does not wobble between frames, and nodes you pin (drag, nudge, `:agents node-pin`) stay where you put them. Above 64 nodes the layout falls back to a grid and keeps live panes.

**Communication topology.** Independent of the 512-event conversation history, Bilterm keeps a bounded record (at most 256 pairs, least recently active dropped first) of every pair that communicated in this session: event count, first and last event (bus sequence number and age). `:agents topology` reports it (also over RPC `wm.command`, MCP `bilterm_colon_command` and `bilterm ctl`, as data, read-only) and `:agents topology export <name>` writes it as JSON to `<private runtime dir>/agent-topology/<name>.json` (directory `0700`, file `0600`; `<name>` is letters, digits, `-`, `_`, `.`, at most 64, not starting with a dot). Export is refused for API callers. The record holds node labels, counts and times only, never message content, and it is in memory only: it ends with the session unless you export it.

**Node and edge cursor.** Arrow keys move the cursor to the nearest node in that direction; `Tab` / `Shift+Tab` step through the selected node's edges. `Enter` on a pane node focuses it. **Grab mode** (`g`): arrows nudge the selected node one terminal cell and pin it; `g`, `Enter` or `Esc` drops it. `u` or right-click unpins.

**Conversation (right).** The bus events in order, newest last, as `<from> -> <to> [<kind>] <preview>`. Kinds are `input`, `exec`, `keys`, `broadcast` (and `message`, reserved for Phase 52). Previews are sanitised and capped at 160 characters; text sent to a pane in a password prompt shows `[redacted]`, and copy keeps it redacted. The view follows new lines until you scroll up; `f` / `End` / the follow marker resume. Filters (agent, pair, keyword) show as chips in a header row; small panes drop the header, buttons and chips progressively. `y` or `[copy]` copies the selected line to the clipboard.

**Compose line.** `send as me` is shown but **disabled**: nothing typed there is stored or sent, and no pane or bus event is produced. It becomes available only with the Phase 52 permission gate.

**Window management.** Move (`Shift+arrows`, drag title), resize (`Alt+arrows`, drag edges and corners), maximize (`m`, badge), close (`Esc` unless pinned), pin (`*`, the `◇` / `◆` badge), splitter (`<` / `>`, drag), panel scale (`[` / `]`, `Ctrl` + wheel, GUI surface). A **pinned** window stays open and keeps its place; `Esc` then only returns the keyboard to the panes, and `:agents unpin` restores close-on-`Esc`. Pinned or not, the window never blocks the panes: click one (or run `:agents blur`) and keys go to it again.

## Agents list window

The AGENTS list is a floating window with one row per tracked pane: its agent state, its name, its pane id, and how long it has been in that state. Use it to see every pane at once, select one, and jump to it. It is separate from the AGENT TRAFFIC window. The two windows can both be open, but only one owns the keyboard at a time.

**Rows.** By default every tracked pane is listed, in pane-id order. The agents filter (`f`, the `[filter]` chip, or `:agents list filter agents`) narrows the list to panes that match an `[agents.<name>]` profile or report their state explicitly. No profile ships built in (see [Assumptions and limits](#assumptions-and-limits)), so until you add one the agents filter can show no rows. The rows refresh as the agent states change.

**Opening and closing.** Open it with `Ctrl+Alt+N`, leader `A`, View > Agents List in the menu, `:agents list` (also `:agents list open`, bare `:agents`, `:agents ls` or `:agents status`), or the `agents.list` API method. Opening it gives it the keyboard. The same toggle chord closes it, as do `:agents list close` and the title's close badge `[X]`. `Esc` closes it unless it is pinned.

**Keys** (while the list has the keyboard; table `AGENTS_LIST_BINDINGS` in `src/input/agents_list_keys.rs`):

| Key | Operation |
| :--- | :--- |
| `Up` / `k`, `Down` / `j` | Select the previous / next row |
| `PageUp`, `PageDown` | Select a page up / down |
| `Home`, `End` | Select the first / last row |
| `Enter` | Focus the selected pane and leave the list (jump) |
| `z` | Open the traffic window on the selected pane (zoom) |
| `f` | Toggle the agents filter |
| `*` | Pin / unpin the window |
| `[`, `]` | Panel scale -10 / +10 (GUI surface) |
| `Shift`+arrows | Move the window one cell |
| `Alt`+arrows | Resize the window one cell |
| `d` | Dock into the tiling strip / float |
| `Esc` | Close, or release the keyboard when pinned |
| `Ctrl+Alt+N` | Close (the same toggle as outside the list) |

The leader key is never consumed. Other keys are swallowed while the list has the keyboard, so typing does not reach the pane underneath. Click a pane, or run `:agents list blur`, to hand the keyboard back while the list stays open.

**Mouse** (table in `src/input/agents_list.rs`):

| Gesture | Operation |
| :--- | :--- |
| Click a row | Select it. Clicking the selected row again, while the list has the keyboard, jumps to that pane. In an unfocused list the click takes the keyboard and selects; it never jumps |
| Right-click a row | Open the traffic window on that pane (zoom) |
| Title badges: pin, minimize, dock, close | Pin / unpin; dismiss (same as `Esc`); dock / float; close |
| Footer chips `[filter]`, `[dock]`, `[zoom]` | Toggle the filter; dock / float; open the traffic window on the selection |
| Drag the title bar | Move the window (this releases any docking) |
| Drag an edge or corner | Resize the window (this releases any docking) |
| Wheel | Scroll three rows |
| `Ctrl` + wheel | Panel scale -10 / +10 (GUI surface only) |
| Press outside the window while it has the keyboard | Release the keyboard; the press then goes on to whatever is underneath |

**Pin.** A pinned list stays open, and `Esc` then only releases the keyboard. Closing the window clears the pin.

**Scale and default placement.** The list has its own panel scale, 75 to 200 percent, on the GUI surface, set by `[` / `]`, `Ctrl` + wheel, `:agents list scale`, or `:agents list scale <75..200>`. Its default size is 36 columns wide (or the workspace width when that is narrower), tall enough for its rows and never more than 70 percent of the workspace height (at least 6 rows). It sits one column in from the right edge and one row below the top. Any move, resize, rectangle or size change releases the dock.

**Docking.** `d`, the dock badge, the `[dock]` chip and `:agents list dock` move the window into the strip described in [Tiling floating panels](#tiling-floating-panels). If there is no room, the window stays floating and `:agents list dock` reports `No room to dock the agents list`. `float` or `:agents list float` puts it back.

**Text and GUI.** In the terminal the window is drawn as cells: a frame with the pin badge and the title buttons, a `STATE NAME PANE SINCE` header, one row per pane with its state dot, the selected row in reverse video, the active pane marked, a scroll indicator when the rows overflow, and the footer chips. In GUI mode the same layout is drawn at the window's own scale. Pane names come from running commands and titles, so they are sanitised before they are shown.

**Verbs.** Every key and gesture above has a colon twin, listed in [Commands](#commands) under `:agents list`. The window is also available over the API (`agents.list`, `bilterm_agents_list`); see [API](../API.md).

## Tiling floating panels

`:arrange` (and `:auto-arrange`), and the `:layout` forms `grid`, `columns`, `rows`, `main-side`, `main-top`, `stack-grid` and `float-grid`, now also tile the open floating panels. The menu entry Window > Tile Windows & Panels runs `:arrange grid`. Each arrange does three things:

- **Docks the open panels.** The AGENTS list and the AGENT TRAFFIC window are placed in a strip beside the panes. When the workspace is at least 100 columns wide the strip is on the right, `clamp(width / 3, 28, 48)` columns wide, with the open panels stacked at equal height. Otherwise the strip is along the bottom, `max(6, height / 3)` rows tall, with the panels side by side.
- **Switches an open `:mcp` console to Push layout** (its existing reflowing mode).
- **Re-flows the panes** into the space that is left.

The strip is recomputed from the live workspace, so a terminal resize keeps the panels docked. If the panes would be left with less than 20 by 6 cells, nothing docks and the panels stay floating. After `:arrange` the status line shows the arrange result, not the no-room notice.

Moving, resizing, closing or floating a docked panel releases its space in the strip. The docked state is not written to layout profiles or to config. With no list, traffic window or `:mcp` console open, arrange behaves as it did before.

Unchanged by this: `:layout tile`, `:layout stack`, `:layout cycle` and their keys `Cmd+L` / `Ctrl+Shift+L` (these switch the tiled and stacked modes and never dock panels), and the drawer's own Push or Overlay setting.

## Command palette coverage

Every colon verb must be reachable from the command palette (`Ctrl+P`, `Alt+Space`, or leader `Space`, then type). Each verb either has a palette entry that runs the same action, or a row in `PALETTE_EXCLUSIONS` (`src/command_registry.rs`) with a reason. A test sweeps every Tab-completion probe and fails when a new verb has neither, so the rule is checked on every change.

The reasons come from a closed list:

| Reason | Meaning |
| :--- | :--- |
| `free-text` | Needs free text that a palette entry cannot supply |
| `host-or-path` | Needs a host name or a path |
| `entity-id` | Needs a specific pane, session or other id |
| `numeric` | Needs an exact number; use the drag or the step keys instead |
| `combinatorial` | Too many combinations to list |
| `destructive` | Has effects a palette click should not trigger |
| `reachable-via-picker` | Covered by a picker (the 14 `:window theme <preset>` forms are in the theme picker) |
| `short-alias` | Another spelling of an entry that is already listed |

Exclusions are visible to agents: each command in the `bilterm://commands` resource carries its `palette_exclusions`, so automation can see why a form has no palette entry. Pressing `Enter` on a fill-in entry (for example `:ssh `) still runs the bare verb, as before; prefilling the text is a follow-up that needs its own sign-off.

## Surface uniformity

Every capability in this chapter must behave the same on every surface, the rule in AGENTS.md convention 6 (`UNI-R01`). The table gives the path for each Phase 51 capability. Any colon form also runs over RPC and MCP through `wm.command` and `bilterm_colon_command`.

| Capability | Mouse | Key (GUI and TUI) | Colon | Palette and help | RPC, HTTP, `bilterm ctl` | MCP |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Demo start | menu View > Agent Demo | leader `y` | `:agents demo` | yes | `agents.demo` | `bilterm_agents_demo` |
| Demo stop | menu View > Stop Agent Demo | leader `Y` | `:agents demo stop` | yes | `agents.demo` with `stop: true` | `bilterm_agents_demo` with `stop: true` |
| Palette coverage | click an entry | `Ctrl+P`, `Alt+Space`, leader `Space` | `:palette`, `:commands` | the table itself | `bilterm://commands` (`palette_exclusions`) | resource |
| Open, close, toggle the list | menu View > Agents List; `[X]` badge | `Ctrl+Alt+N`, leader `A` | `:agents list open`, `close`, `toggle` | yes | `agents.list` | `bilterm_agents_list` |
| Dismiss, blur | minimize badge; click outside | `Esc` | `:agents list dismiss`, `blur` | yes | `agents.list` | `bilterm_agents_list` |
| Select, step, page, first, last | click a row; wheel | `Up` / `k`, `Down` / `j`, `PageUp`, `PageDown`, `Home`, `End` | `:agents list select ...` | yes | `agents.list` | `bilterm_agents_list` |
| Jump to the pane | click the selected row again | `Enter` | `:agents list jump [id]` | yes | `agents.list` | `bilterm_agents_list` |
| Zoom to the traffic window | right-click a row; chip `[zoom]` | `z` | `:agents list zoom [id]` | yes | `agents.list` | `bilterm_agents_list` |
| Pin | pin badge | `*` | `:agents list pin`, `unpin`, `pin toggle` | yes | `agents.list` | `bilterm_agents_list` |
| Scale | `Ctrl` + wheel | `[`, `]` | `:agents list scale ...` | yes | `agents.list` | `bilterm_agents_list` |
| Filter | chip `[filter]` | `f` | `:agents list filter ...` | yes | `agents.list` | `bilterm_agents_list` |
| Move, resize, rect, size | drag the title or an edge | `Shift`+arrows, `Alt`+arrows | `:agents list move`, `resize`, `rect`, `size` | `rect` is excluded (`numeric`); the rest yes | `agents.list` | `bilterm_agents_list` |
| Scroll | wheel | `PageUp`, `PageDown` (as `page`) | `:agents list scroll <±n>` | yes | `agents.list` | `bilterm_agents_list` |
| Dock and float | dock badge; chip `[dock]` | `d` | `:agents list dock`, `dock toggle`, `float` | yes | `agents.list` | `bilterm_agents_list` |
| Tile everything | menu Window > Tile Windows & Panels | none (exception 4) | `:arrange grid` and the other patterns | yes | `wm.command ":arrange grid"` | `bilterm_colon_command` |

The recorded exceptions, each with its reason:

1. **Explicit pane ids** (`:agents list select <id>`, `jump <id>`, `zoom <id>`): no key. `Up`, `Down` and `Enter` act on the selection. Reason: `entity-id`.
2. **`size` and `rect`**: no key, because they take numbers. The mouse does the same job through the title drag and the edge drag, which send `rect`. Reason: `numeric`.
3. **Explicit-value forms** (`pin`, `unpin`, `filter all|agents`, `dock`, `float`, and the absolute `scale <75..200>`): the keys and the mouse send the toggle form. The explicit forms are colon, RPC and palette only.
4. **`:arrange <pattern>`**: no key chord, because the pattern is text. The keyboard path is the palette (`Ctrl+P`, then type `arrange`). The menu entry Window > Tile Windows & Panels runs `:arrange grid`. A key for arrange would need its own action in a later change.
5. **Dismiss and page**: the mouse twin of `dismiss` is the minimize badge, and the mouse twin of `page` is the wheel (`scroll`).
6. **`:agents list status` and the bare forms** (`:agents`, `:agents ls`, `:agents status`, `:agents list`): these read the agent data and open the window. They have no separate key; the window's toggle is `Ctrl+Alt+N` or leader `A`.

## GUI rendering

In GUI mode (`--gui`) the same scene is drawn with vector graphics instead of cells, with no new dependency:

- the agents graph is one anti-aliased, hi-DPI RGBA texture with smooth **curved edges**;
- active edges carry three **flow particles**, and each node has a state-coloured **glow**;
- the Navigator state dots are smooth **vector icons**, with the working icon and blocked icon animated;
- textures are capped (an oversized window skips the texture rather than allocating gigabytes).

The terminal UI shows the braille/cell version of the same graph and the glyph dots from [Reading the sidebar](#reading-the-sidebar).

**Panel scale and `Ctrl` + wheel (runtime scope chosen in Phase 50 plan 13: Linux/X11 fully wired).** The drawer, the `:mcp` console and the agents window each have their own panel scale (75 to 200 percent) on the GUI surface.

| GUI runtime | `Ctrl` + wheel over a panel | `[` / `]`, `:drawer scale`, `:mcp scale`, `:agents scale` | Mouse modifiers (`Shift`/`Alt`/`Ctrl`) |
| :--- | :--- | :--- | :--- |
| Linux (X11) | scales that panel; `Ctrl` + wheel elsewhere still zooms the pane font | work | delivered to mouse handlers |
| Windows | **still zooms the whole terminal font** | work | **empty** (not delivered) |
| macOS | **still zooms the whole terminal font** (`Cmd`/`Ctrl`) | work | **empty** (not delivered) |

On Windows and macOS panel scale is therefore reachable only through `[` / `]` and the `... scale` commands. The headless GUI surface used by the test suite behaves like the X11 row.

## Assumptions and limits

Known gaps, stated plainly:

- **Compose is disabled.** The `send as me` line (and `[send]`) is a stub until Phase 52 adds the permission gate and agent messaging.
- **The graph is star-shaped until Phase 52.** Phase 50 sees only input broadcast, `:mcp broadcast`, and `pane.exec` / `pane.send_keys`; pane-to-pane agent messaging arrives with Phase 52.
- **History is the last 512 events**, kept in memory only: nothing is persisted, logged, or published on `/events` (see [API](../API.md)).
- **Silent long builds read as idle.** A running command that prints nothing for more than 1.5 s drops from working to idle unless a profile `working` pattern matches.
- **Windows and macOS GUI runtimes**: `Ctrl` + wheel over a panel still zooms the font and mouse events carry empty modifiers (see [GUI rendering](#gui-rendering)).
- **Not verified in a real window.** The X11 GUI visuals (vector icons, curved edges, particles, glow, comms-glyph animation) and the `Ctrl` + wheel panel scale were covered by automated tests on the rendering and routing paths only; they have not been seen in a real GUI window. Windows and macOS were compile-checked only.
- **`src/wm/drawer.rs` is 1040 lines**, above the ~1000-line soft target (hard backstop 1200). A follow-up should move the context-menu state into its own struct.

Assumptions behind the behaviour (from the Phase 50 research log):

- **A1** The Navigator layout (workspace > session > pane, one line per item) was described by the user and not read from another product.
- **A2** The thresholds are tuned guesses: working holds for 1.5 s after the last output byte, a working/idle flip needs 500 ms of dwell, and profile patterns scan the last 8 visible rows.
- **A3** No agent profiles ship built in. Without a `[agents.<name>]` profile only generic signals are used; example patterns in [Chapter 4](04-themes-glyphs-and-config.md) are unverified.
- **A4** Silent long builds read as idle (above).
- **A5** The default drawer width is 28 columns (`navigator_width_cols`, 16 to 100).
- **A6** Key choices (`j`/`k`, `e`, `E`, `C`, `N`, `a`, `A`, `o`, `Ctrl+O`, leader `!` / `a`) are proposals chosen to avoid existing bindings; each has a command twin, so every operation is reachable without its key.
- **A7** "Workspace" maps to the root of the session tree.
- **A8** State and comms glyphs are assumed single-width; the ASCII glyph pack (`:glyphs ascii`) is the fallback for fonts where they are not.

---

[← Chapter 10: Security & Best Practices](10-security-and-best-practices.md) | [User Guide Home](README.md)
