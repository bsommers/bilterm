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
}

/// Commands whose text exists in neither the palette nor the help lines:
/// `(name, alias_of or "", summary)`.
pub(crate) const EXTRA: &[(&str, &str, &str)] = &[
    (":aero", ":glass", "Alias of :glass."),
    (":asciinema", ":record", "Alias of :record."),
    (":bcast", ":broadcast", "Alias of :broadcast."),
    (":block-select", ":rect-select", "Alias of :rect-select."),
    (":blockselect", ":rect-select", "Alias of :rect-select."),
    (":border", "", "Set the window/prompt outline style, or toggle global bold; same as :outline."),
    (":box-select", ":rect-select", "Alias of :rect-select."),
    (":clear", "", "Clear the focused pane's screen and scrollback and reset the page anchor."),
    (":clipboard", "", "List clipboard history, or run a clipboard subcommand."),
    (":clip", ":clipboard", "Alias of :clipboard."),
    (":cmd", ":commands", "Alias of :commands."),
    (":exit!", "", "Quit immediately, skipping the exit confirmation."),
    (":exit-protection", ":exit-guard", "Alias of :exit-guard."),
    (":exitguard", ":exit-guard", "Alias of :exit-guard."),
    (":explorer", ":browse", "Alias of :browse."),
    (":fields", ":hints", "Alias of :hints."),
    (":filebrowser", ":browse", "Alias of :browse."),
    (":files", ":browse", "Alias of :browse."),
    (":fkey", ":fkeys", "Alias of :fkeys."),
    (":fleet", ":remote", "Alias of :remote."),
    (":focus", "", "Focus a pane by id, direction (left/right/up/down) or next/prev."),
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
    (":pane-style", "", "Style the focused pane: theme, outline, bold and colors; same as :window."),
    (":panefont", "", "Show or change the focused pane's font settings."),
    (":navigator", ":drawer", "Alias of :drawer for the Navigator tree (expand all, collapse all, style)."),
    (":panels", ":drawer", "Alias of :drawer."),
    (":prot", ":lock", "Alias of :lock."),
    (":q", ":quit", "Alias of :quit."),
    (":q!", ":exit!", "Alias of :exit! (force quit)."),
    (":raise", ":focus", "Alias of :focus."),
    (":readonly", ":lock", "Alias of :lock."),
    (":rec", ":record", "Alias of :record."),
    (":rect-select", "", "Run a select subcommand (rectangular selection); shares :select's handler."),
    (":rectselect", ":rect-select", "Alias of :rect-select."),
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
    (":style", "", "Pick or cycle the visual style (classic, snazzy, glass, modern, liquid, ...)."),
    (":textsize", ":fontsize", "Alias of :fontsize."),
    (":url", ":links", "Alias of :links."),
    (":ver", ":version", "Alias of :version."),
    (":visual", ":style", "Alias of :style."),
    (":visual-style", ":style", "Alias of :style."),
    (":vsplit", "", "Split the focused pane vertically (also :vs)."),
    (":win", ":pane-style", "Alias of :pane-style."),
    (":winfont", ":panefont", "Alias of :panefont."),
    (":winmove", ":move", "Alias of :move."),
    (":winresize", ":resize", "Alias of :resize."),
    (":workspace", ":sessions", "Alias of :sessions."),
    (":yank-history", ":clipboard", "Alias of :clipboard."),
    (":yank-ring", ":clipboard", "Alias of :clipboard."),
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
}
