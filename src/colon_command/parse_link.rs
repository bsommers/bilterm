//! Reserved words of `:ssh-tmux` / `:sshtmux` (Phase 51.1, P-13): `reconnect`, `disconnect`,
//! `info`, `prefill`. Everything else (a host, `list`, `ls`, `status`) falls through to the
//! existing `ssh-tmux` arm in `parse.rs`.

use super::ColonCommand;
use crate::wm::link_cmd::RemoteLinkOp;

const USAGE: &str = "Usage: :ssh-tmux reconnect | disconnect | info | prefill";

/// Answers only for head `ssh-tmux`/`sshtmux` whose first argument is a reserved word.
pub(super) fn parse_link_colon_command(
    head: &str,
    parts: &[&str],
) -> Option<Result<ColonCommand, String>> {
    if !matches!(head, "ssh-tmux" | "sshtmux") {
        return None;
    }
    let op = RemoteLinkOp::from_word(parts.get(1)?)?;
    if parts.len() > 2 {
        return Some(Err(USAGE.to_string()));
    }
    Some(Ok(ColonCommand::RemoteLink(op)))
}
