//! Parser for the config/state query commands (`:get`, `:set`, `:config`, `:state`, `:appearance save`; Phase 49-04).

use super::ColonCommand;

/// Returns `Some` only for the commands owned by this module; everything else falls through unchanged.
pub(super) fn parse_config_colon_command(
    head: &str,
    parts: &[&str],
    rest: &str,
) -> Option<Result<ColonCommand, String>> {
    match head {
        "get" => Some(match parts.get(1) {
            Some(key) if parts.len() == 2 => Ok(ColonCommand::ConfigGet((*key).to_string())),
            _ => Err("Usage: :get <key>  (dotted path, e.g. appearance.profile)".to_string()),
        }),
        "set" => {
            let after_head = rest.trim_start().strip_prefix("set").unwrap_or("").trim();
            let (key, value) = match after_head.split_once(char::is_whitespace) {
                Some((k, v)) if !v.trim().is_empty() => (k, v.trim()),
                _ => return Some(Err("Usage: :set <key> <value>".to_string())),
            };
            Some(Ok(ColonCommand::ConfigSet(key.to_string(), value.to_string())))
        }
        "config" => Some(match parts.get(1).copied() {
            Some("list") if parts.len() == 2 => Ok(ColonCommand::ConfigList),
            Some("save") if parts.len() == 2 => Ok(ColonCommand::ConfigSave),
            _ => Err("Usage: :config list | save".to_string()),
        }),
        "state" if parts.len() == 1 => Some(Ok(ColonCommand::StateQuery)),
        "appearance" if parts.get(1) == Some(&"save") && parts.len() == 2 => {
            Some(Ok(ColonCommand::AppearanceSave))
        }
        _ => None,
    }
}
