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
8. [GUI rendering](#gui-rendering)
9. [Assumptions and limits](#assumptions-and-limits)

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
| `:drawer style [classic\|navigator\|toggle]` | | | Switch drawer style |
| `:drawer rows 1\|2` | | | Lines per pane row in Navigator style (`drawer.row_lines`) |

`:drawer size`, `toggle`, `focus`, the dock edges and the pinning modes are unchanged; see [Chapter 3](03-window-manager-and-z-stack.md).

### `:agents` verbs

| Command | Key | Mouse | Effect |
| :--- | :--- | :--- | :--- |
| `:agents list` | | | Agent state per pane (data over RPC, see [API](../API.md)) |
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
| `:agents demo` | | | Fill the window with synthetic `[demo]` pane-to-pane events (splits to three panes if needed; memory only, nothing is sent to any pane) to try the graph and conversation |
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

**Conversation (right).** The bus events in order, newest last, as `<from> -> <to> [<kind>] <preview>`. Kinds are `input`, `exec`, `keys`, `broadcast` (and `message`, reserved for Phase 51). Previews are sanitised and capped at 160 characters; text sent to a pane in a password prompt shows `[redacted]`, and copy keeps it redacted. The view follows new lines until you scroll up; `f` / `End` / the follow marker resume. Filters (agent, pair, keyword) show as chips in a header row; small panes drop the header, buttons and chips progressively. `y` or `[copy]` copies the selected line to the clipboard.

**Compose line.** `send as me` is shown but **disabled**: nothing typed there is stored or sent, and no pane or bus event is produced. It becomes available only with the Phase 51 permission gate.

**Window management.** Move (`Shift+arrows`, drag title), resize (`Alt+arrows`, drag edges and corners), maximize (`m`, badge), close (`Esc` unless pinned), pin (`*`, the `◇` / `◆` badge), splitter (`<` / `>`, drag), panel scale (`[` / `]`, `Ctrl` + wheel, GUI surface). A **pinned** window stays open and keeps its place; `Esc` then only returns the keyboard to the panes, and `:agents unpin` restores close-on-`Esc`. Pinned or not, the window never blocks the panes: click one (or run `:agents blur`) and keys go to it again.

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

- **Compose is disabled.** The `send as me` line (and `[send]`) is a stub until Phase 51 adds the permission gate and agent messaging.
- **The graph is star-shaped until Phase 51.** Phase 50 sees only input broadcast, `:mcp broadcast`, and `pane.exec` / `pane.send_keys`; pane-to-pane agent messaging arrives with Phase 51.
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
