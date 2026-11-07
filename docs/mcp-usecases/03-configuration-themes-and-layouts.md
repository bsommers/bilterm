# 03 — Configuration, Themes, Shaders, and Layouts via MCP

Bilterm exposes its entire visual compositor, appearance profile system, typography engine, and layout manager through `:colon` commands (`bilterm_colon_command` / `wm.command`), dedicated MCP tools (`bilterm_workspace_fontsize`, `bilterm_canvas`), and read-only MCP resources (`bilterm://state`, `bilterm://config`, `bilterm://commands`).

---

## 1. Inspecting Runtime Config & State (`:state`, `:get`, `:config list`)

Read queries always succeed over MCP and JSON-RPC regardless of the `[api] allow_config_write` setting, returning structured JSON inside the `data` field of `wm.command`:

### Via MCP Resource (`resources/read`)

```json
{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{
  "uri":"bilterm://config"
}}
```

### Via `bilterm_colon_command` (`:state`, `:get <key>`, `:config list`)

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":state"}
}}
```

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":get appearance.profile"}
}}
```

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":config list"}
}}
```

### The Config Write Gate (`[api] allow_config_write`)

For security (`SEC-R01`), config-mutating commands (`:set <key> <value>`, `:config save`, `:appearance save`) invoked over RPC, HTTP, MCP, or synthetic `pane.send_keys` (`api_origin == true`) are **refused by default** unless enabled in `~/.config/bilterm/config.toml`:

```toml
[api]
allow_config_write = true
```

*(Note: The `[api]` section itself is never modifiable over the API).* When `allow_config_write = true` is set, agents can modify keys and persist them:

```bash
bilterm ctl wm.command '{"command":":set theme dracula"}'
bilterm ctl wm.command '{"command":":appearance save"}'
bilterm ctl wm.command '{"command":":config save"}'
```

---

## 2. Appearance Profiles, Display Modes & Visual Styles

Bilterm supports 5 high-level appearance profiles, 3 display modes, and composable visual style flags:

### Appearance Profiles (`:appearance`)

| Command | Profile Description |
|---------|---------------------|
| `:appearance legacy` | Classic IBM 3270 + Borland Turbo Vision retro chrome |
| `:appearance obsidian` | Modern minimal dark obsidian chrome |
| `:appearance midnight` | Deep midnight blue modern chrome |
| `:appearance arctic` | High-clarity arctic slate chrome |
| `:appearance liquid` | Refractive Liquid Glass borders with ambient halo |

### Display Modes (`:mode`) & Chrome Policies (`:chrome`)

```bash
# Switch display mode (for non-Legacy profiles)
bilterm ctl wm.command '{"command":":mode standard"}'
bilterm ctl wm.command '{"command":":mode minimal"}'
bilterm ctl wm.command '{"command":":mode zen"}'

# Configure pane title bar visibility & focus accent ring
bilterm ctl wm.command '{"command":":chrome title focused"}'   # all | focused | none | auto
bilterm ctl wm.command '{"command":":chrome ring on"}'         # on | off

# Configure visual style flags (:style, :snazzy, :glass, :modern, :liquid)
bilterm ctl wm.command '{"command":":style snazzy-glass"}'     # classic | snazzy | glass | snazzy-glass | modern | liquid | cycle
```

---

## 3. Color Themes, Per-Window Chrome & Glyph Packs

### Built-In 14 Theme Presets (`:theme <preset>`)

Defined in `src/ui/theme.rs` (`ThemePreset::from_name`):

| Category | Theme Names / Aliases |
|----------|-----------------------|
| **Borland / DOS** | `turbo-blue` (`borland`, `blue`), `turbo-cyan` (`cyan`) |
| **IBM Mainframe** | `ibm-3278-green` (`3278`, `green`), `ibm-3279-amber` (`3279`, `amber`), `ibm-mono` (`mono`) |
| **Cyber / Sci-Fi** | `cyberpunk-neon` (`cyberpunk`, `neon`), `tron-grid` (`tron`, `mcp`) |
| **Modern / iTerm2** | `dracula`, `solarized-dark` (`solarized`), `gruvbox-dark` (`gruvbox`), `tokyo-night` (`tokyonight`), `catppuccin-mocha` (`catppuccin`), `nord`, `monokai-remastered` (`monokai`) |

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{
  "name":"bilterm_colon_command",
  "arguments":{"command":":theme 3278"}
}}
```

### Importing `.itermcolors` & Per-Window Overrides (`:window`)

```bash
# List or import external .itermcolors / .toml themes
bilterm ctl wm.command '{"command":":theme list"}'
bilterm ctl wm.command '{"command":":theme import ~/themes/ayu-dark.itermcolors"}'

# Customize only the active pane's theme, border outline, or colors
bilterm ctl wm.command '{"command":":window theme 3279"}'
bilterm ctl wm.command '{"command":":window outline rounded"}'
bilterm ctl wm.command '{"command":":window font-color #00FF66"}'
bilterm ctl wm.command '{"command":":window reset"}'

# Global border outlines, shadows, and glyph packs
bilterm ctl wm.command '{"command":":outline double"}'         # double|single|thin|rounded|thick|bold|dotted|dashed|quadruple|ascii
bilterm ctl wm.command '{"command":":shadows"}'                # toggle drop shadows
bilterm ctl wm.command '{"command":":glyphs nerd-font"}'       # unicode | cp437 | nerd-font | ascii
```

---

## 4. Post-Processing Shaders (`:crt`, `:crosshair`, `:spotlight`, `:focalzoom`)

Bilterm's 3-pass Ratatui / GUI compositor applies real-time cell and pixel shaders:

```bash
# CRT scanline shader
bilterm ctl wm.command '{"command":":crt on"}'

# IBM 3270 cross-hair cursor shader (pane-local or full-viewport)
bilterm ctl wm.command '{"command":":crosshair on"}'
bilterm ctl wm.command '{"command":":crosshair full"}'
bilterm ctl wm.command '{"command":":crosshair off"}'

# Spotlight dimming shader around active cursor
bilterm ctl wm.command '{"command":":spotlight circle fuzzy size 24 8 feather 40"}'
bilterm ctl wm.command '{"command":":spotlight off"}'

# Focal-zoom magnifier lens shader
bilterm ctl wm.command '{"command":":focalzoom 2x ellipse fuzzy size 20 6"}'
bilterm ctl wm.command '{"command":":focalzoom off"}'
```

---

## 5. Typography & Dynamic Font Scaling (`bilterm_workspace_fontsize`)

Use `bilterm_workspace_fontsize` (`workspace.fontsize`) to inspect or scale workspace and per-pane typography:

```json
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{
  "name":"bilterm_workspace_fontsize",
  "arguments":{
    "action": "in",
    "size": 16
  }
}}
```

Response payload:

```json
{
  "base_font_size_pt": 16,
  "cell_height_px": 20,
  "cell_width_px": 10,
  "font_scale_pct": 100,
  "font_size_pt": 16,
  "host_font_passthrough": false,
  "pane_font_bold": false,
  "pane_font_double_width": false,
  "pane_font_family": "Cascadia Mono",
  "pane_font_italic": false,
  "pane_font_scale_pct": 100,
  "pane_font_size_pt": 16
}
```

Per-pane font families (`cascadia`, `consolas`, `jetbrains`, `firacode`, `iosevka`, `plex`, `hack`, `ibm3270`, `vga`) can also be set via `:font`:

```bash
bilterm ctl wm.command '{"command":":font ibm3270 16pt bold"}'
bilterm ctl wm.command '{"command":":panel scale 125"}'
```

---

## 6. Layout Profiles, Slide-Out Drawer & Guake Drop-Down Terminal

### Layout Profiles (`:layout`, `:profile`)

```bash
# Built-in layout arrangements
bilterm ctl wm.command '{"command":":layout arrange main-side"}'  # grid | columns | rows | main-side | main-top | stack-grid

# Save, switch, and set default layout profiles
bilterm ctl wm.command '{"command":":layout save agent-cockpit"}'
bilterm ctl wm.command '{"command":":layout switch agent-cockpit"}'
bilterm ctl wm.command '{"command":":layout default agent-cockpit"}'
bilterm ctl wm.command '{"command":":layout autosave on"}'
```

### Guake Drop-Down Pane (`:quake`) & Slide-Out Drawer (`:drawer`)

```bash
# Toggle the top drop-down Quake/Guake scratchpad terminal
bilterm ctl wm.command '{"command":":quake"}'

# Control the Slide-Out Side Drawer (:drawer)
bilterm ctl wm.command '{"command":":drawer toggle"}'
bilterm ctl wm.command '{"command":":drawer right"}'           # left | right | top | bottom
bilterm ctl wm.command '{"command":":drawer push"}'            # float (overlay) | push (reflows workspace)
bilterm ctl wm.command '{"command":":drawer size 36"}'
```
