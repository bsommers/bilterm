//! Parser for `:context set <k> <v> | get <k> | list [prefix] | clear <k>` (UNI-R01, Phase 68).

use super::ColonCommand;
use crate::action::Action;
use crate::wm::context_queue_ops::ContextOp;

pub const CONTEXT_USAGE: &str = "Usage: :context set <key> <value> | get <key> | list [prefix] | clear <key>";

/// Parses the arguments after `:context`. The value keeps interior spacing of `rest` tokens joined by one space.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        [] | ["list" | "ls"] => ContextOp::List { prefix: None },
        ["list" | "ls", p] => ContextOp::List { prefix: Some((*p).to_string()) },
        // Bare `get` / `set` / `clear` (palette prompt entries) parse; the executor reports the usage.
        ["get"] => ContextOp::Get { key: String::new() },
        ["get", k] => ContextOp::Get { key: (*k).to_string() },
        ["clear" | "delete" | "rm"] => ContextOp::Clear { key: String::new() },
        ["clear" | "delete" | "rm", k] => ContextOp::Clear { key: (*k).to_string() },
        ["set"] => ContextOp::Set { key: String::new(), value: String::new() },
        ["set", k, v @ ..] => ContextOp::Set { key: (*k).to_string(), value: v.join(" ") },
        _ => return Err(CONTEXT_USAGE.to_string()),
    };
    Ok(Action::Context(op))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::parse_colon_command;

    fn p(s: &str) -> Result<Action, String> {
        parse_colon_command(s).expect("recognized")
    }

    #[test]
    fn parses_all_verbs() {
        assert_eq!(p(":context"), Ok(Action::Context(ContextOp::List { prefix: None })));
        assert_eq!(p(":context list a."), Ok(Action::Context(ContextOp::List { prefix: Some("a.".into()) })));
        assert_eq!(p(":context get k"), Ok(Action::Context(ContextOp::Get { key: "k".into() })));
        assert_eq!(p(":context clear k"), Ok(Action::Context(ContextOp::Clear { key: "k".into() })));
        assert_eq!(
            p(":context set k hello big world"),
            Ok(Action::Context(ContextOp::Set { key: "k".into(), value: "hello big world".into() }))
        );
    }

    #[test]
    fn rejects_bad_usage() {
        assert!(p(":context bogus").is_err());
        assert!(p(":context get a b").is_err());
    }

    #[test]
    fn colon_form_round_trips() {
        let a = Action::Context(ContextOp::Set { key: "k".into(), value: "a b".into() });
        assert_eq!(p(&a.colon_form_or_summary()), Ok(a));
    }
}
