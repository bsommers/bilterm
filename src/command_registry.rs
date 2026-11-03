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
pub(crate) const EXTRA: &[(&str, &str, &str)] = &[];

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
