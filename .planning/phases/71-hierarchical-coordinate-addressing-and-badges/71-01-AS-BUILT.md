> As-built record of the first COORD implementation (PR #57). The authoritative plan is `71-PLAN.md`.

---
phase: 71-hierarchical-coordinate-addressing-and-badges
plan: 01
type: execute
wave: 1
depends_on: [65]
files_modified:
  - src/wm/coordinates.rs
  - src/action.rs
  - src/colon_command/
  - src/ui/
  - src/input/mouse_wm.rs
  - src/api/
  - tests/phase71_hierarchical_coordinates.rs
autonomous: true
requirements:
  - COORD-01
  - COORD-02
  - COORD-03
  - COORD-04
  - UNI-R01
recommended_model: sonnet
must_haves:
  truths:
    - "Every window has a live, fully-populated 5-tuple coordinate D:(W,S,Y,P) via WindowManager::coord_of."
    - "TargetCoord::parse accepts every form in the grammar below and rejects every listed error case with CoordParseError."
    - "Title-bar badge is Off by default, so classic goldens (GOLDENS OK 11) stay byte-identical."
    - "Every coordinate command is reachable from colon, palette, menu, and JSON-RPC via Action::Coords(CoordsOp)."
  artifacts:
    - path: "src/wm/coordinates.rs"
      provides: "TargetCoord parse/format/to_path, coord_of, resolve_coord, CoordDisplayStyle names"
    - path: "tests/phase71_hierarchical_coordinates.rs"
      provides: "Parser, formatter, resolver, badge, and surface tests for COORD-01..04"
---

# Phase 71 Plan: 5-Tuple Hierarchical Coordinate Addressing & Window Bar Badge — D:(W,S,Y,P)

## Objective

Give every Bilterm window a stable, human-readable address on the hierarchy
`Daemon D -> Workspace W -> Session S -> Window Y -> Pane P (optional)`, let users
and clients parse and resolve such addresses, show the focused window's address in
its title bar, and expose the same operations on every UI and API surface.

The contract in `src/wm/coordinates.rs` is already committed as a stub
(`9ce97d2`, `a9e6455`). Type shapes and function signatures there are FIXED.
Bodies are `todo!()` and are filled in by this phase. The authoritative semantics are
the COORD shared-semantics spec (`coord-spec.md` in the session scratchpad); this plan
restates it and does not extend it.

---

## Requirements

### COORD-01: `TargetCoord` type and live `coord_of`
- `TargetCoord { daemon, workspace, session, window, pane }`, each `Option<String>`; `None` = wildcard.
- Live values, deterministic:
  - daemon: `"d0"` (always, for the local instance).
  - workspace: `"w1"` (one workspace per instance).
  - session: the active session name, i.e. the WindowManager config `history_session` as a raw string.
  - window: `"y{PaneId}"` (e.g. `y2`).
  - pane: `Some("%N")` when the window mirrors a remote tmux pane with a known `%pane_id`; otherwise `None`.
- `WindowManager::coord_of(id) -> Option<TargetCoord>` returns a fully-populated coordinate for the window, except `pane`, which follows the rule above. Returns `None` for an unknown pane id.

### COORD-02: Parser and formatter

**Grammar (EBNF, case-insensitive for keywords and labels; whitespace allowed around every token):**

```ebnf
coord         = ws , ( daemon_tuple | tuple | labelled | path ) , ws ;

daemon_tuple  = "(" , ws , daemon_seg , ws , ":" , ws , "(" , positional , ")" , ws , ")" ;
tuple         = "(" , positional , ")" ;
positional    = component , ( "," , component ) , [ "," , component ] ;     (* 3 or 4 items *)
component     = ws , ( token ) , ws ;

labelled      = "(" , label_item , { "," , label_item } , ")" ;             (* any subset, any order *)
label_item    = ws , label , ws , ":" , ws , token , ws ;
label         = "wksp" | "sess" | "win" | "pane" | "daemon" | "d" ;

path          = segment , { sep , segment } ;
sep           = ":" | "/" ;                                                   (* '/' == ':' *)
segment       = daemon_seg | workspace_seg | session_seg | window_seg | pane_seg | token ;
daemon_seg    = "d" , digits ;
workspace_seg = "w" , digits ;
session_seg   = "s" , digits ;
window_seg    = ( "y" | "#" ) , digits ;
pane_seg      = ( "p" | "%" ) , digits ;
token         = { any char except "," ":" "/" "(" ")" whitespace } ;
digits        = digit , { digit } ;
ws            = { whitespace } ;
```

**Accepted forms (normative examples):**

| Input | Parsed as |
|---|---|
| `(d0:(w1,s1,y2,p1))` | daemon tuple, all five levels |
| `(d0:(w1,s1,y2))` | daemon tuple, no pane |
| `(w1,s1,y2,p1)` / `(w1,s1,y2)` | positional compact tuple |
| `(Wksp: w1, Sess: main, Win: y2, Pane: %3)` | verbose labelled; `Daemon:`/`D:` also accepted |
| `d0:w1:s1:y2:p1` | colon path, prefix-classified |
| `w1/s1/y2` | slash path (same as colon) |
| `w12:p6` | Herdr form: workspace 12, pane 6 (window absent) |
| `s1:y2` | session 1, window 2 |
| `#2`, `y2`, `Y2` | window 2 (normalized to `y2`) |
| `%3` | pane `%3` (window absent) |
| `2` | bare number: window `y2` |

**Normalization rules:**
- Window normalizes `#2`, `y2`, `Y2` to `y2`.
- Pane keeps `%3` or `p6`, lowercased.
- Daemon, workspace, and session are stored as given, trimmed.

**Path classification:**
- If every segment is prefix-classified (`dN`, `wN`, `sN`, `yN`/`#N`, `pN`/`%N`), assign each segment to its level.
- The order must be non-decreasing in hierarchy (D, W, S, Y, P). Otherwise it is an error.
- Otherwise use positional assignment by segment count:
  - 5 segments: D:W:S:Y:P
  - 4 segments: W:S:Y:P
  - 3 segments: W:S:Y
  - 2 segments: S:Y
  - 1 segment: window

**Error cases (`CoordParseError`; each must have a test):**
- empty or whitespace-only input
- unbalanced parentheses
- more than 5 components
- duplicate level (in a path or a labelled form)
- prefix-classified segments out of hierarchy order
- unknown label in the labelled form
- malformed digits (e.g. `y`, `w-1`, `p` with no number)

**Formatting (`TargetCoord::format(style)`; `None` levels render as `*`):**

| Style | Example with pane | Example without pane |
|---|---|---|
| Verbose | `(Wksp: w1, Sess: main, Win: y2, Pane: %3)` | `(Wksp: w1, Sess: main, Win: y2)` |
| Compact | `(w1,main,y2,%3)` | `(w1,main,y2)` |
| DaemonTuple | `(d0:(w1,main,y2,%3))` | `(d0:(w1,main,y2))` |
| Off | `None` | `None` |

- `to_path()` returns the canonical colon path, e.g. `d0:w1:main:y2` or `d0:w1:main:y2:%3`. Unspecified levels are omitted.
- Round-trip: `parse(format(c, s)) == c` for Compact, DaemonTuple, and Verbose whenever all levels are set.

### COORD-03: `CoordDisplayStyle` and title-bar badge
- `CoordDisplayStyle` variants are `Verbose`, `Compact`, `DaemonTuple`, `Off` (default). `from_name` accepts `verbose|compact|daemon|daemon-tuple|off` case-insensitively. `name` returns `verbose|compact|daemon|off`.
- Badge: right-aligned inside the window title bar, left of the `[_][□][X]` controls, one space gap. The text comes from `format(coord_style())` for the focused window.
- `Off` draws nothing. This keeps classic goldens byte-identical, which `scripts/check_classic_goldens.sh` must report as `GOLDENS OK 11`.
- If the badge would overlap the title text or the controls, it is dropped entirely. It is never truncated.
- Title-bar click hit-testing (`detect_title_bar_badge_click` in `src/input/mouse_wm.rs`) must not regress. Clicks on the badge are consumed and do not reach the window controls.
- With `Off`, the render path must return before calling `format` or `coord_of`, so the badge code is inert until style is changed.

### COORD-04: `:coords` command and resolver
- `CoordsOp` (in `src/wm/coordinates.rs`): `Show`, `Style(CoordDisplayStyle)`, `Locate(String)`, `Copy`.
- `Show` reports the focused window's coordinate in the status bar and sets `command_data` to JSON `{"path": ..., "compact": ...}`.
- `Style` calls `set_coord_style`.
- `Locate` resolves the argument with `resolve_coord` and focuses the first match. No match is an error.
- `Copy` copies the focused window's canonical `to_path()` to the clipboard.
- Colon forms: `:coords`, `:coords verbose|compact|daemon|off`, `:coords locate <coord>`, `:coords copy`. A bad argument is a parse error that lists the valid options.
- `WindowManager::resolve_coord(&TargetCoord) -> CoordResolution`:
  - `None` levels are wildcards.
  - `daemon` must equal `d0` if set. `workspace` must equal `w1` if set. `session` must equal the active session if set.
  - `window` `yN` matches PaneId N.
  - Herdr compatibility: if `window` is `None` and `pane` is `pN`, treat it as window N.
  - Pane `%N` matches the window whose tmux pane id is `%N`.
  - Results are ascending by PaneId. Only workspace panes are matched, not quick terminal or utility overlays, unless they are in `panes()`.

### UNI-R01: Uniform surfaces

Every coordinate operation lowers to `Action::Coords(CoordsOp)` and executes through `WindowManager::apply` (`src/colon_command/execute_wm.rs`). The surface matrix below is the authoritative list of what must be reachable.

| Operation | Colon | Palette | Menu | Keybinding | JSON-RPC | MCP |
|---|---|---|---|---|---|---|
| Show | `:coords` | Coordinates: Show entry | not in spec | n/a | `coords.format` (focused pane, default) | via RPC |
| Style | `:coords verbose\|compact\|daemon\|off` | one entry per style | View -> Coordinates: Verbose / Compact / Daemon / Off | n/a | none in spec; `coords.format` takes `style` for formatting only | via RPC |
| Locate | `:coords locate <coord>` | Coordinates: Locate entry | not in spec | n/a | `coords.resolve` | via RPC |
| Copy | `:coords copy` | Coordinates: Copy entry | Window -> Copy Coordinate | n/a | none in spec; path available from `coords.format` | via RPC |

Notes:
- "n/a" means the keybinding surface is deliberately not added in this phase. Do not invent keybindings.
- "not in spec" means the spec defines no menu entry for that operation. Do not add one.
- "via RPC" means MCP exposes the same operations through the JSON-RPC methods (`bilterm --mcp`).
- JSON-RPC methods:
  - `coords.resolve` takes `{"coord": "<text>"}` and returns `{"panes": [2], "coords": ["d0:w1:main:y2"]}`.
  - `coords.format` takes `{"pane_id"?: n, "style"?: "compact"}`, where `pane_id` defaults to the focused pane, and returns `{"text": "(w1,main,y2)", "path": "d0:w1:main:y2"}`.
  - Parse errors become JSON-RPC errors.
- Help catalog and Tab completion entries are added for each colon form.
- Open question for the implementer: `coords.resolve` and `coords.format` are read-only. They may call `coord_of` and `resolve_coord` directly rather than going through `Action`. The `action_reachability` test decides which is required.

---

## Threat Model & Security Considerations

| Threat ID | Category | Description | Mitigation |
|---|---|---|---|
| T-COO-01 | Input handling | Untrusted coordinate text from RPC or colon input causes panic or unbounded allocation. | Parser caps component count at 5 and rejects oversize input. It never panics on any input. |
| T-COO-02 | Misdirected action | `:coords locate` focuses an unintended window through lenient matching. | Resolver matches only the levels that are set. Wildcards apply only to `None`. A zero-match `Locate` is an error, not a fallback. |
| T-COO-03 | Information disclosure | Badge or `command_data` leaks session names into screenshots or logs by default. | Badge defaults to `Off`. Session name appears only when the user opts in. |

---

## Technical Design & Wave Breakdown

### Wave 1: Core addressing (COORD-01, COORD-02, resolver half of COORD-04)
- Owner: `src/wm/coordinates.rs`.
- Implement `coord_of`, `TargetCoord::parse`, `format`, `to_path`, `CoordDisplayStyle::from_name` and `name`, and `resolve_coord`.
- Keep the stub signatures unchanged. Only function bodies change.
- Tests in `tests/phase71_hierarchical_coordinates.rs`:
  - every accepted form in the table above
  - every error case
  - round-trip for the three formats
  - resolver wildcard and Herdr cases
  - `coord_of` through `WindowManager::new_headless` and `CompositePane::new_headless`

### Wave 2 (parallel with Wave 1, against fixed signatures): badge and surfaces
- Badge (COORD-03). Owner: `src/ui/*` (title-bar render) and `src/input/mouse_wm.rs` (badge hit-test only).
- Surfaces (COORD-04, UNI-R01). Owners:
  - `src/action.rs`: add `Action::Coords(CoordsOp)`.
  - `src/colon_command/*`: colon parse and `execute_wm` arm.
  - `src/api/*`: `coords.resolve`, `coords.format`.
  - Menu, palette, and help catalog definitions. Locate them with codegraph or `grep -rn "Copy" src/ui`. Do not add new menu structure beyond the matrix.
- Runtime tests need Wave 1 bodies. Code can be written before that, but tests cannot pass until Wave 1 lands.

### Wave 3: Integration and gates
- Run the verification commands below.
- Confirm no golden change, `handle_wm_key_event` still at or under 80 lines, and no new bare field access on `WindowManager` or `CompositePane`.

### File ownership

| Path | Task | Wave |
|---|---|---|
| `src/wm/coordinates.rs` | COORD-01, COORD-02, resolver | 1 |
| `src/action.rs` | `Action::Coords` variant | 2 |
| `src/colon_command/*` | colon forms, `execute_wm` arm | 2 |
| `src/ui/*` | title-bar badge, menu and palette entries | 2 |
| `src/input/mouse_wm.rs` | badge click hit-test only | 2 |
| `src/api/*` | `coords.resolve`, `coords.format` | 2 |
| `tests/phase71_hierarchical_coordinates.rs` | all new tests | 1, 2 |

No other files are touched. Do not edit `tests/snapshots/*`. A golden diff is a failure, not something to re-record.

### Architecture gates
- ARCH-R01: `src/input/mouse.rs::handle_wm_key_event` stays at or under 80 lines. Any new key path goes through `Action`, not inline logic.
- ARCH-R02: new `WindowManager` state (the `coord_style` field) stays private. Read and write it only through `coord_style()` and `set_coord_style()`. Pane removal stays on `remove_pane`.
- ARCH-R04: every `src/**/*.rs` file targets roughly 800–1000 lines. If `src/wm/coordinates.rs` grows past that, split it into `src/wm/coordinates/{parse,format,resolve}.rs` with `mod.rs` re-exports. The public signatures stay the same.

---

## Verification

Primary gate (run from the worktree root):

```bash
./scripts/verify.sh --only clippy unit t:phase71_hierarchical_coordinates t:action_reachability t:ui_goldens
scripts/check_classic_goldens.sh   # must print: GOLDENS OK 11
```

- `verify.sh` prints one line. PASS means done. On FAIL, read only `target/verify/<stage>/fail.txt`, then `log`, per `AGENTS.md` (PERF-CTX).
- `GOLDENS OK 11` is required. Any other output is a failure.
- Deliver through a feature branch and PR. Never push to `main` directly.
- Do not commit or push as part of planning. Execution commits follow the `committing-and-releasing-cleanly` flow.

### Acceptance criteria
1. Every grammar example in COORD-02 parses to the stated value.
2. Every error case returns `CoordParseError`. None panics.
3. `format` and `parse` round-trip for Compact, DaemonTuple, and Verbose with all levels set.
4. `Off` is the default. With `Off`, the classic goldens are byte-identical.
5. Every row of the UNI-R01 matrix is reachable from the listed surfaces. `n/a` cells are asserted absent.
6. `:coords locate` with no match returns an error and changes no focus.
