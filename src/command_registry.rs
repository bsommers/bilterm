//! Single read-only view over every `:colon` command (Phase 49-03).
//!
//! The parser stays hand-written (Legacy parity); this module joins the three
//! per-command tables - Tab completion (`COLON_COMMANDS`), the Ctrl+X palette
//! (`PALETTE_CATALOG`) and the F1 help (`HELP_LINES`) - plus `EXTRA` below into one
//! catalog that `:commands`, MCP resources and the public API all read.
//! Tests fail if a completion command has no description, so the lists cannot drift.

use crate::pane::completion::colon_command_table;
use crate::ui::modals::{HELP_LINES, PALETTE_CATALOG};

/// One command as seen by humans, MCP and the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInfo {
    /// Leading word including the colon, e.g. `:split`.
    pub name: &'static str,
    /// Known sub-arguments (may be empty = free-form or none).
    pub subargs: &'static [&'static str],
    /// One-line description.
    pub summary: &'static str,
    /// `Some(canonical)` when this name is only another spelling of `canonical`.
    pub alias_of: Option<&'static str>,
    /// Ready-to-run or fill-in examples from the palette (`:split h`, `:ssh `).
    pub examples: Vec<&'static str>,
    /// Keyboard shortcut text from the palette ("—" entries are dropped).
    pub shortcut: Option<&'static str>,
    /// Probes of this command deliberately left out of the palette, with the reason
    /// (`PALETTE_EXCLUSIONS`); automation sees why a form has no palette entry.
    pub palette_exclusions: Vec<(&'static str, &'static str)>,
}

/// Commands whose text exists in neither the palette nor the help lines:
/// `(name, alias_of or "", summary)`.
pub(crate) const EXTRA: &[(&str, &str, &str)] = &[
    (":agent", "", "Agent-to-agent messaging and coordination (:agent name|send|broadcast|inbox|ack|list|enable|queue|approve|deny|replay)."),
    (":aero", ":glass", "Alias of :glass."),
    (":auto-arrange", ":arrange", "Alias of :arrange."),
    (":asciinema", ":record", "Alias of :record."),
    (":bcast", ":broadcast", "Alias of :broadcast."),
    (":bcanvas", ":browser", "Alias of :browser."),
    (":block-select", ":rect", "Alias of :rect."),
    (":blockselect", ":rect", "Alias of :rect."),
    (":border", ":outline", "Alias of :outline."),
    (":box-select", ":rect", "Alias of :rect."),
    (":clear", "", "Clear the focused pane's screen and scrollback and reset the page anchor."),
    (":clipboard", "", "List clipboard history, or run a clipboard subcommand."),
    (":clip", ":clipboard", "Alias of :clipboard."),
    (":clips", ":clipboard", "Alias of :clipboard."),
    (":cmd", ":commands", "Alias of :commands."),
    (":exit!", ":quit!", "Alias of :quit! (force quit)."),
    (":exit-protection", ":exit-guard", "Alias of :exit-guard."),
    (":exitguard", ":exit-guard", "Alias of :exit-guard."),
    (":explorer", ":browse", "Alias of :browse."),
    (":fields", ":hints", "Alias of :hints."),
    (":filebrowser", ":browse", "Alias of :browse."),
    (":files", ":browse", "Alias of :browse."),
    (":fkey", ":fkeys", "Alias of :fkeys."),
    (":fleet", ":remote", "Alias of :remote."),
    (":focus", "", "Focus a pane by id, direction (left/right/up/down) or next/prev; cycle|pane|<window> moves the keyboard between the pane and open side windows."),
    (":focus-light", ":spotlight", "Alias of :spotlight."),
    (":frosted", ":glass", "Alias of :glass."),
    (":function-keys", ":fkeys", "Alias of :fkeys."),
    (":fzoom", ":focalzoom", "Alias of :focalzoom."),
    (":glass", "", "Turn the glass visual style on, off or toggle it."),
    (":herdr", ":remote", "Alias of :remote."),
    (":herdr-style", ":snazzy", "Alias of :snazzy."),
    (":hint", ":hints", "Alias of :hints."),
    (":hist", ":history", "Alias of :history."),
    (":iterm2colors", ":itermcolors", "Alias of :itermcolors."),
    (":liquid-glass", ":liquid", "Alias of :liquid."),
    (":liquidglass", ":liquid", "Alias of :liquid."),
    (":magnify", ":focalzoom", "Alias of :focalzoom."),
    (":master-control", "", "Run a master-control (MCP) subcommand; same as :mcp."),
    (":menu", "", "Show, hide or toggle the menu bar (on/off)."),
    (":minimal", ":modern", "Alias of :modern."),
    (":new-session", "", "Create a new session via the session tree, with an optional name."),
    (":newsession", ":new-session", "Alias of :new-session."),
    (":non-retro", ":modern", "Alias of :modern."),
    (":nonretro", ":modern", "Alias of :modern."),
    (":pager", ":scroll", "Alias of :scroll."),
    (":pane-style", ":window", "Alias of :window (style the focused pane: theme, outline, bold and colors)."),
    (":packing", ":sessions", "Alias of :sessions map for the 2D session packing viewer."),
    (":palette", ":commands", "Alias of :commands."),
    (":panefont", "", "Show or change the focused pane's font settings."),
    (":navigator", ":drawer", "Alias of :drawer for the Navigator tree (expand all, collapse all, style)."),
    (":peek", ":drawer", "Alias of :drawer preview for the live window peek preview pane."),
    (":panels", ":drawer", "Alias of :drawer."),
    (":prot", ":lock", "Alias of :lock."),
    (":q", ":quit", "Alias of :quit."),
    (":q!", ":quit!", "Alias of :quit! (force quit)."),
    (":raise", ":focus", "Alias of :focus."),
    (":readonly", ":lock", "Alias of :lock."),
    (":rec", ":record", "Alias of :record."),
    (":rect-select", ":rect", "Alias of :rect (rectangular selection)."),
    (":rectselect", ":rect", "Alias of :rect."),
    (":rename-session", "", "Rename a session via the session tree."),
    (":renamesession", ":rename-session", "Alias of :rename-session."),
    (":scrl", ":scroll", "Alias of :scroll."),
    (":sel", ":select", "Alias of :select."),
    (":select", "", "Run a selection subcommand on the focused pane; no argument runs the default."),
    (":session-rename", ":rename", "Alias of :rename."),
    (":session-save", ":save", "Alias of :save."),
    (":shadow", ":shadows", "Alias of :shadows."),
    (":sidebar", ":drawer", "Alias of :drawer."),
    (":snazzy", "", "Turn the snazzy visual style on, off or toggle it."),
    (":spot", ":spotlight", "Alias of :spotlight."),
    (":sshtmux", ":ssh-tmux", "Alias of :ssh-tmux."),
    (":studio", ":script", "Alias of :script."),
    (":style", "", "Pick or cycle the visual style (classic, snazzy, glass, modern, liquid, ...)."),
    (":textsize", ":fontsize", "Alias of :fontsize."),
    (":url", ":urls", "Alias of :urls (same as :links)."),
    (":ver", ":version", "Alias of :version."),
    (":visual", ":style", "Alias of :style."),
    (":visual-style", ":style", "Alias of :style."),
    (":vsplit", "", "Split the focused pane vertically (also :vs)."),
    (":vcanvas", ":video", "Alias of :video."),
    (":win", ":window", "Alias of :window."),
    (":winfont", ":panefont", "Alias of :panefont."),
    (":winmove", ":move", "Alias of :move."),
    (":winresize", ":resize", "Alias of :resize."),
    (":workspace", ":sessions", "Alias of :sessions."),
    (":yank-history", ":clipboard", "Alias of :clipboard."),
    (":yank-ring", ":clipboard", "Alias of :clipboard."),
    (":pane", ":window", "Alias of :window (manage or style the active window/pane)."),
    (":kill-pane", ":close", "Alias of :close (close the active pane)."),
    (":traffic", "", "TRAFFIC MONITOR window: bytes per connection, session, window, pane (Agent Traffic is :agents traffic)."),
    (":swarm", "", "Worktree swarm DAG orchestrator: :swarm status | load <plan.toml> | clear | run | spawn | verify | diff | merge | clean."),
    (":context", "", "Shared context bus: :context set <k> <v> | get <k> | list [prefix] | clear <k>."),
    (":coords", "", "Hierarchical D:(W,S,Y,P) coordinates: show the focused window's, set the title badge style, locate a window, copy."),
    (":netmon", ":traffic", "Alias of :traffic."),
    (":tab", "", "Tab-style window commands (:tab new|next|prev|close|rename)."),
    (":tabnew", ":tab", "Alias of :tab new (create a new terminal window)."),
    (":tabnext", ":tab", "Alias of :tab next (focus next terminal window)."),
    (":tabprev", ":tab", "Alias of :tab prev (focus previous terminal window)."),
    (":tabclose", ":tab", "Alias of :tab close (close active terminal window)."),
    (":voice", "", "Speech-to-text dictation control (:voice toggle|start|stop|cancel|status|provider <kind>)."),
    (":stt", ":voice", "Alias of :voice."),
];

/// Reason for the `:finder` rows (Phase 50.1 exception E1): they drive the open Find modal.
pub(crate) const FINDER_REASON: &str =
    "modal-control verbs, reachable only while the Find modal is open (50.1 exception E1)";

/// The closed vocabulary of exclusion reasons (D-02).
#[cfg(test)]
pub(crate) const EXCLUSION_REASONS: &[&str] = &[
    "free-text",
    "host-or-path",
    "entity-id",
    "numeric",
    "combinatorial",
    "destructive",
    "reachable-via-picker",
    "short-alias",
    FINDER_REASON,
];

/// Colon forms deliberately without a palette entry: `(probe, reason)`. A probe is the bare head
/// or `head sub` of a Tab-completion row (a sub ending in a space needs an argument). Anything
/// without a palette entry (same parsed `Action`) must be listed here; the sweep in `tests` fails
/// for a new verb that is neither, and for a stale row that is real but already covered (D-02).
pub(crate) const PALETTE_EXCLUSIONS: &[(&str, &str)] = &[
    (":agent name", "free-text"),
    (":agent send", "free-text"),
    (":agent broadcast", "free-text"),
    (":agent ack", "free-text"),
    (":prompt float", "numeric"),
    (":prompt height", "numeric"),
    (":prompt width", "numeric"),
    (":history max-files", "numeric"),
    (":fontsize 14pt", "numeric"),
    (":fontsize 125%", "numeric"),
    (":fontsize 150%", "numeric"),
    (":spotlight size", "numeric"),
    (":spotlight feather", "numeric"),
    (":focalzoom size", "numeric"),
    (":focalzoom feather", "numeric"),
    (":drawer size", "numeric"),
    (":drawer preview row", "numeric"),
    (":drawer preview pane", "entity-id"),
    (":drawer preview session", "entity-id"),
    (":font size", "numeric"),
    (":font scale", "numeric"),
    (":canvas res", "numeric"),
    (":canvas line", "numeric"),
    (":canvas circle", "numeric"),
    (":canvas bezier", "numeric"),
    (":canvas sprite", "numeric"),
    (":canvas step", "numeric"),
    (":canvas plot", "numeric"),
    (":canvas pan", "numeric"),
    (":canvas zoom", "numeric"),
    (":agents rect", "numeric"),
    (":agents list rect", "numeric"),
    (":agents node-pin", "numeric"),
    (":focus 1", "entity-id"),
    (":focus 2", "entity-id"),
    (":focus 3", "entity-id"),
    (":focus 4", "entity-id"),
    // `:focus <window>`: one palette entry (`:focus agents-list`) stands for the window names; `:focus cycle` reaches each.
    (":focus agents", "entity-id"),
    (":focus sessions", "entity-id"),
    (":focus clipboard", "entity-id"),
    (":focus traffic", "entity-id"),
    (":focus commands", "entity-id"),
    (":focus script", "entity-id"),
    (":focus fonts", "entity-id"),
    (":focus daemon", "entity-id"),
    (":focus hud", "entity-id"),
    (":focus drawer", "entity-id"),
    (":focus console", "entity-id"),
    (":layout switch", "entity-id"),
    (":profile delete", "entity-id"),
    (":session switch", "entity-id"),
    (":session reorder", "entity-id"),
    (":session delete", "entity-id"),
    (":session branch", "entity-id"),
    (":session graft", "entity-id"),
    (":session detach-subtree", "entity-id"),
    (":mcp attach", "entity-id"),
    (":mcp spawn", "entity-id"),
    (":mcp graft", "entity-id"),
    (":mcp detach-subtree", "entity-id"),
    (":mcp broadcast", "entity-id"),
    (":mcp freeze", "entity-id"),
    (":mcp thaw", "entity-id"),
    (":mcp derez", "destructive"),
    (":mcp shutdown --confirm", "destructive"),
    (":trigger add", "free-text"),
    (":session shell", "free-text"),
    (":session cwd", "free-text"),
    (":font pane", "free-text"),
    (":font all", "free-text"),
    (":ssh -p", "host-or-path"),
    (":ssh user@host", "host-or-path"),
    (":ssh-tmux user@host", "host-or-path"),
    (":remote add", "host-or-path"),
    (":remote exec", "host-or-path"),
    (":remote disconnect", "host-or-path"),
    (":font custom", "host-or-path"),
    (":font google", "host-or-path"),
    (":font scan", "host-or-path"),
    (":canvas image", "host-or-path"),
    (":canvas export", "host-or-path"),
    (":canvas encode", "host-or-path"),
    (":window theme import", "host-or-path"),
    (":fkeys send", "host-or-path"),
    (":theme global", "host-or-path"),
    (":theme export", "host-or-path"),
    (":outline window", "combinatorial"),
    (":outline prompt", "combinatorial"),
    (":window fg", "combinatorial"),
    (":window bg", "combinatorial"),
    (":window prompt-bg", "combinatorial"),
    (":window prompt-color", "combinatorial"),
    (":window inactive-border", "combinatorial"),
    (":window title-color", "combinatorial"),
    (":window badge-color", "combinatorial"),
    (":window crosshair-color", "combinatorial"),
    (":window shadow-color", "combinatorial"),
    (":cursor color", "combinatorial"),
    (":cursor cursor-color", "combinatorial"),
    (":cursor fg", "combinatorial"),
    (":cursor window", "combinatorial"),
    (":cursor global", "combinatorial"),
    (":cursor green", "combinatorial"),
    (":cursor amber", "combinatorial"),
    (":cursor cyan", "combinatorial"),
    (":cursor yellow", "combinatorial"),
    (":cursor crimson", "combinatorial"),
    (":cursor magenta", "combinatorial"),
    (":cursor purple", "combinatorial"),
    (":cursor orange", "combinatorial"),
    (":cursor white", "combinatorial"),
    (":canvas backend", "combinatorial"),
    (":window theme turbo-blue", "reachable-via-picker"),
    (":window theme turbo-cyan", "reachable-via-picker"),
    (":window theme ibm-3278-green", "reachable-via-picker"),
    (":window theme ibm-3279-amber", "reachable-via-picker"),
    (":window theme ibm-mono", "reachable-via-picker"),
    (":window theme cyberpunk-neon", "reachable-via-picker"),
    (":window theme dracula", "reachable-via-picker"),
    (":window theme solarized-dark", "reachable-via-picker"),
    (":window theme gruvbox-dark", "reachable-via-picker"),
    (":window theme tokyo-night", "reachable-via-picker"),
    (":window theme catppuccin-mocha", "reachable-via-picker"),
    (":window theme nord", "reachable-via-picker"),
    (":window theme monokai-remastered", "reachable-via-picker"),
    (":window theme tron-grid", "reachable-via-picker"),
    (":border", "short-alias"),
    (":window select toggle", "entity-id"),
    (":window select range", "entity-id"),
    (":appearance select", "entity-id"),
    (":appearance duplicate", "free-text"),
    (":appearance modify", "free-text"),
    (":appearance import", "host-or-path"),
    (":appearance export", "host-or-path"),
    (":layout main-side", "short-alias"),
    (":profile single", "short-alias"),
    (":profile dual-v", "short-alias"),
    (":profile dual-h", "short-alias"),
    (":profile main-side", "short-alias"),
    (":profile quad", "short-alias"),
    (":profile cascade", "short-alias"),
    (":raise", "short-alias"),
    (":finder", FINDER_REASON),
    (":finder kind", FINDER_REASON),
    (":finder query", FINDER_REASON),
    (":finder move", FINDER_REASON),
    (":finder select", FINDER_REASON),
    (":finder enter", FINDER_REASON),
    (":finder prefill", FINDER_REASON),
    (":finder close", FINDER_REASON),
    (":commands dismiss", "reachable-via-picker"),
    (":commands blur", "reachable-via-picker"),
    (":commands pin", "reachable-via-picker"),
    (":commands unpin", "reachable-via-picker"),
    (":commands dock", "reachable-via-picker"),
    (":commands float", "reachable-via-picker"),
    (":fonts dismiss", "reachable-via-picker"),
    (":fonts blur", "reachable-via-picker"),
    (":fonts pin", "reachable-via-picker"),
    (":fonts unpin", "reachable-via-picker"),
    (":fonts dock", "reachable-via-picker"),
    (":fonts float", "reachable-via-picker"),
    (":fonts apply", "reachable-via-picker"),
    (":fonts all", "reachable-via-picker"),
    (":fonts scan", "reachable-via-picker"),
    (":fonts next", "reachable-via-picker"),
    (":fonts prev", "reachable-via-picker"),
    (":fonts first", "reachable-via-picker"),
    (":fonts last", "reachable-via-picker"),
    (":script run", "reachable-via-picker"),
    (":script step", "reachable-via-picker"),
    (":script reset", "reachable-via-picker"),
    (":script record", "reachable-via-picker"),
    (":script edit", "reachable-via-picker"),
    (":script add", "free-text"),
    (":script clear", "reachable-via-picker"),
    (":browser open", "host-or-path"),
    (":browser back", "reachable-via-picker"),
    (":browser forward", "reachable-via-picker"),
    (":browser reload", "reachable-via-picker"),
    (":browser stop", "reachable-via-picker"),
    (":browser focus", "reachable-via-picker"),
    (":browser zoom", "numeric"),
    (":browser close", "reachable-via-picker"),
    (":video open", "host-or-path"),
    (":video play", "reachable-via-picker"),
    (":video pause", "reachable-via-picker"),
    (":video toggle", "reachable-via-picker"),
    (":video seek", "numeric"),
    (":video volume", "numeric"),
    (":video mute", "reachable-via-picker"),
    (":video loop", "reachable-via-picker"),
    (":video speed", "numeric"),
    (":video close", "reachable-via-picker"),
    (":script clear-log", "reachable-via-picker"),
    (":script save", "free-text"),
    (":script load", "free-text"),
    (":script list", "reachable-via-picker"),
    (":script pin", "reachable-via-picker"),
    (":script dock", "reachable-via-picker"),
    (":script up", "reachable-via-picker"),
    (":script down", "reachable-via-picker"),
    (":script left", "reachable-via-picker"),
    (":script right", "reachable-via-picker"),
    (":script select", "entity-id"),
    (":clipboard dismiss", "reachable-via-picker"),
    (":clipboard blur", "reachable-via-picker"),
    (":clipboard pin", "reachable-via-picker"),
    (":clipboard unpin", "reachable-via-picker"),
    (":clipboard dock", "reachable-via-picker"),
    (":clipboard float", "reachable-via-picker"),
    (":clipboard filter", "free-text"),
    (":clipboard copy", "free-text"),
    (":clipboard pin-entry", "reachable-via-picker"),
    (":clipboard delete", "reachable-via-picker"),
    (":voice start", "entity-id"),
    (":voice stop", "reachable-via-picker"),
    (":voice cancel", "reachable-via-picker"),
    (":traffic pin", "reachable-via-picker"),
    (":traffic unpin", "reachable-via-picker"),
    (":traffic dock", "reachable-via-picker"),
    (":traffic float", "reachable-via-picker"),
    (":traffic move", "reachable-via-picker"),
    (":traffic resize", "reachable-via-picker"),
    (":traffic rect", "reachable-via-picker"),
    (":traffic size", "reachable-via-picker"),
    (":voice status", "reachable-via-picker"),
    (":voice provider", "reachable-via-picker"),
    (":copy mouse", "reachable-via-picker"),
    (":copy mouse on", "reachable-via-picker"),
    (":copy mouse off", "reachable-via-picker"),
    (":copy auto", "reachable-via-picker"),
    (":copy auto on", "reachable-via-picker"),
    (":copy auto off", "reachable-via-picker"),
];

fn head(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

/// Builds the full catalog in Tab-completion order.
pub fn catalog() -> Vec<CommandInfo> {
    colon_command_table()
        .iter()
        .map(|&(name, subargs)| {
            let palette: Vec<_> = PALETTE_CATALOG.iter().filter(|e| head(e.0) == name).collect();
            let extra = EXTRA.iter().find(|e| e.0 == name);
            let summary = palette
                .first()
                .map(|e| e.2)
                .or_else(|| HELP_LINES.iter().find(|l| head(l.0) == name).map(|l| l.1))
                .or_else(|| extra.map(|e| e.2))
                .unwrap_or("");
            let alias_of = extra.map(|e| e.1).filter(|a| !a.is_empty());
            let shortcut = palette.iter().map(|e| e.1).find(|k| !k.is_empty() && *k != "—");
            CommandInfo {
                name,
                subargs,
                summary,
                alias_of,
                examples: palette.iter().map(|e| e.0).collect(),
                shortcut,
                palette_exclusions: PALETTE_EXCLUSIONS
                    .iter()
                    .filter(|(probe, _)| head(probe) == name)
                    .copied()
                    .collect(),
            }
        })
        .collect()
}

/// Looks one command up by its leading word (with or without the colon).
pub fn lookup(name: &str) -> Option<CommandInfo> {
    let want = if name.starts_with(':') { name.to_string() } else { format!(":{name}") };
    catalog().into_iter().find(|c| c.name == want)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::parse_colon_command;

    #[test]
    fn every_completion_command_has_a_description() {
        let missing: Vec<_> = catalog().iter().filter(|c| c.summary.is_empty()).map(|c| c.name).collect();
        assert!(missing.is_empty(), "commands without a summary (add to command_registry::EXTRA or the palette): {missing:?}");
    }

    #[test]
    fn aliases_point_at_a_real_command() {
        let names: Vec<_> = catalog().iter().map(|c| c.name).collect();
        for c in catalog() {
            if let Some(a) = c.alias_of {
                assert!(names.contains(&a), "{} aliases unknown {a}", c.name);
            }
        }
    }

    #[test]
    fn catalog_names_are_unique_and_lookup_works() {
        let cat = catalog();
        let mut seen = std::collections::HashSet::new();
        assert!(cat.iter().all(|c| seen.insert(c.name)), "duplicate command name in COLON_COMMANDS");
        let split = lookup("split").expect(":split exists");
        assert!(split.examples.contains(&":split h"));
        assert_eq!(lookup(":split"), Some(split));
        assert!(lookup("nonexistent-cmd").is_none());
    }

    #[test]
    fn every_palette_example_head_is_in_the_catalog() {
        let names: Vec<_> = catalog().iter().map(|c| c.name).collect();
        for e in PALETTE_CATALOG {
            assert!(names.contains(&head(e.0)), "palette entry `{}` is not in the completion list", e.0);
        }
    }

    #[test]
    fn every_catalog_command_is_known_to_the_parser() {
        for c in catalog() {
            // free-form commands need an argument; a bare head must at least not be "not a command".
            let probe = c.examples.iter().find(|e| !e.ends_with(' ')).copied().unwrap_or(c.name);
            assert!(parse_colon_command(probe).is_some(), "parser rejects catalog command {} (probe `{probe}`)", c.name);
        }
    }

    // ---- Phase 51-04 (UIT-02): palette coverage sweep ----

    /// Every completion row: its bare head, plus `head sub` per sub-argument (`true` = needs an
    /// argument). Sub-arguments are probed only for heads that are not aliases of another head.
    fn completion_probes() -> Vec<(String, bool)> {
        let mut v = Vec::new();
        for &(name, subs) in colon_command_table() {
            v.push((name.to_string(), false));
            if EXTRA.iter().any(|e| e.0 == name && !e.1.is_empty()) {
                continue;
            }
            for s in subs {
                v.push((format!("{name} {}", s.trim()), s.ends_with(' ')));
            }
        }
        v
    }

    fn is_covered(probe: &str, needs_arg: bool, palette: &[&str], palette_actions: &[crate::action::Action]) -> bool {
        let parsed = parse_colon_command(probe);
        if let Some(Ok(a)) = &parsed
            && palette_actions.contains(a)
        {
            return true;
        }
        let needs_arg = needs_arg || matches!(parsed, Some(Err(_)));
        needs_arg && palette.iter().any(|e| e.starts_with(probe))
    }

    fn palette_actions(palette: &[&str]) -> Vec<crate::action::Action> {
        palette.iter().filter_map(|e| parse_colon_command(e)).filter_map(Result::ok).collect()
    }

    /// The sweep as a plain function so a synthetic table can prove it reports a new verb.
    fn uncovered_probes(probes: &[(String, bool)], palette: &[&str], exclusions: &[(&str, &str)]) -> Vec<String> {
        let actions = palette_actions(palette);
        probes
            .iter()
            .filter(|(p, _)| !exclusions.iter().any(|(x, r)| x == p && !r.is_empty()))
            .filter(|(p, n)| !is_covered(p, *n, palette, &actions))
            .map(|(p, _)| p.clone())
            .collect()
    }

    fn palette_texts() -> Vec<&'static str> {
        PALETTE_CATALOG.iter().map(|e| e.0).collect()
    }

    #[test]
    fn every_completion_probe_has_a_palette_entry_or_exclusion() {
        let missing = uncovered_probes(&completion_probes(), &palette_texts(), PALETTE_EXCLUSIONS);
        assert!(
            missing.is_empty(),
            "add a PALETTE_CATALOG entry (src/ui/help_catalog.rs) or a PALETTE_EXCLUSIONS reason for {} probes: {missing:#?}",
            missing.len()
        );
    }

    #[test]
    fn palette_sweep_detects_a_new_verb() {
        let mut probes = completion_probes();
        probes.push((":zzz-new-verb".to_string(), false));
        probes.push((":zzz-new-verb sub".to_string(), false));
        let missing = uncovered_probes(&probes, &palette_texts(), PALETTE_EXCLUSIONS);
        assert!(missing.contains(&":zzz-new-verb".to_string()), "{missing:?}");
        assert!(missing.contains(&":zzz-new-verb sub".to_string()), "{missing:?}");
        // an exclusion row with a reason silences exactly that probe
        let silenced = uncovered_probes(&probes, &palette_texts(), &[(":zzz-new-verb", "free-text")]);
        assert!(!silenced.contains(&":zzz-new-verb".to_string()));
        assert!(silenced.contains(&":zzz-new-verb sub".to_string()));
        // an empty reason does not silence
        let empty = uncovered_probes(&probes, &palette_texts(), &[(":zzz-new-verb", "")]);
        assert!(empty.contains(&":zzz-new-verb".to_string()));
    }

    #[test]
    fn palette_exclusions_are_real_and_not_redundant() {
        let probes = completion_probes();
        let palette = palette_texts();
        let actions = palette_actions(&palette);
        let mut seen = std::collections::HashSet::new();
        for &(probe, reason) in PALETTE_EXCLUSIONS {
            assert!(seen.insert(probe), "duplicate exclusion `{probe}`");
            assert!(
                !reason.is_empty() && EXCLUSION_REASONS.contains(&reason),
                "`{probe}`: reason `{reason}` is not in the vocabulary"
            );
            let needs = probes
                .iter()
                .find(|(p, _)| p == probe)
                .unwrap_or_else(|| panic!("exclusion `{probe}` is not a completion probe (typo or renamed verb)"))
                .1;
            assert!(
                !is_covered(probe, needs, &palette, &actions),
                "exclusion `{probe}` is already covered by a palette entry: delete the exclusion row"
            );
        }
        // every :finder probe carries the Find-modal reason; :find agent|session|history are
        // covered by the 50.1-09 palette entries, not excluded
        for (p, _) in probes.iter().filter(|(p, _)| p.starts_with(":finder ")) {
            assert!(
                PALETTE_EXCLUSIONS.iter().any(|(x, r)| x == p && *r == FINDER_REASON),
                "{p} needs the Find-modal exclusion"
            );
        }
        for p in [":find agent", ":find session", ":find history"] {
            assert!(!PALETTE_EXCLUSIONS.iter().any(|(x, _)| *x == p), "{p} has a palette entry, not an exclusion");
            assert!(palette.contains(&p), "{p} palette entry (50.1-09) missing");
        }
    }

    /// Parser arm heads that are not Tab-completion rows (short spellings kept for muscle memory).
    const PARSER_ONLY_ALIASES: &[&str] = &["?", "a", "cb", "d", "h", "p", "sb", "sp", "vi", "vs"];

    #[test]
    fn parser_heads_are_known_best_effort() {
        let names = crate::pane::completion::colon_command_names();
        let mut found = 0;
        let mut unknown = Vec::new();
        for src in [
            include_str!("colon_command/parse.rs"),
            include_str!("colon_command/parse_visual.rs"),
            include_str!("colon_command/parse_config.rs"),
            include_str!("colon_command/parse_find.rs"),
        ] {
            // only the arms of the top-level `match head {` (ends at the first `    }` line)
            let Some(start) = src.lines().position(|l| l == "    match head {") else { continue };
            for line in src.lines().skip(start + 1).take_while(|l| *l != "    }") {
                let Some(rest) = line.strip_prefix("        \"") else { continue };
                let Some((arms, _)) = rest.split_once(" =>") else { continue };
                let arms = format!("\"{}", arms.split(" if ").next().unwrap_or(arms));
                let ok = arms.split(" | ").all(|a| {
                    a.starts_with('"') && a.ends_with('"') && a.len() > 2 && a[1..a.len() - 1].chars().all(|c| c.is_ascii_alphanumeric() || "-!?".contains(c))
                });
                if !ok {
                    continue;
                }
                for arm in arms.split(" | ") {
                    let arm = &arm[1..arm.len() - 1];
                    found += 1;
                    if !names.contains(&format!(":{arm}").as_str()) && !PARSER_ONLY_ALIASES.contains(&arm) {
                        unknown.push(arm.to_string());
                    }
                }
            }
        }
        // parse_agents.rs guards with `if head != "agents"` instead of a head match
        assert!(include_str!("colon_command/parse_agents.rs").contains("head != \"agents\""));
        assert!(found >= 60, "head scan found only {found} heads: the parser formatting changed, fix the scan");
        assert!(unknown.is_empty(), "parser heads missing from COLON_COMMANDS (add a row or PARSER_ONLY_ALIASES): {unknown:?}");
    }
}
