//! Reserved words of `:ssh-tmux` / `:sshtmux` (Phase 51.1, P-13): `reconnect`, `disconnect`,
//! `info`, `prefill`. Everything else (a host, `list`, `ls`, `status`) falls through to the
//! existing `ssh-tmux` arm in `parse.rs`. A host that is literally named like a reserved word is
//! reached with `--`: `:ssh-tmux -- info [session]`.

use super::ColonCommand;
use crate::wm::link_cmd::RemoteLinkOp;

const USAGE: &str =
    "Usage: :ssh-tmux reconnect | disconnect | info | prefill (a host with such a name: :ssh-tmux -- <host>)";

/// Answers only for head `ssh-tmux`/`sshtmux` whose first argument is a reserved word.
pub(super) fn parse_link_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    if !matches!(head, "ssh-tmux" | "sshtmux") {
        return None;
    }
    // `--` ends the reserved words: `:ssh-tmux -- info [session]` connects to a host named `info`.
    if parts.get(1) == Some(&"--") {
        return Some(match parts.get(2) {
            Some(target) if !target.starts_with('-') && parts.len() <= 4 => {
                Ok(ColonCommand::SshTmuxConnect {
                    target: (*target).to_string(),
                    session: parts.get(3).copied().unwrap_or("bilterm").to_string(),
                })
            }
            _ => Err("Usage: :ssh-tmux -- <user@host|profile> [session]".to_string()),
        });
    }
    let op = RemoteLinkOp::from_word(parts.get(1)?)?;
    if parts.len() > 2 {
        return Some(Err(USAGE.to_string()));
    }
    Some(Ok(ColonCommand::RemoteLink(op)))
}
