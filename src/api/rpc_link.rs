//! `remote.link` RPC method (Phase 51.1, plan 11 fills the body and the dispatcher arm).
//!
//! Seam state (plan 01): the `impl` block exists so plan 11 edits this file only; the method
//! builds `:ssh-tmux info|reconnect|disconnect`, runs it through `parse_colon_command` and
//! `apply_with_policy`, and returns the result plus `"link"` = `link_summary_json`.

use super::rpc::AutomationControlPlane;

impl AutomationControlPlane {}
