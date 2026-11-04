//! Operations of the `:agents` command family (Phase 50-03, FLT-01).
//!
//! `:agents` (plural) is the Phase 50 view command; `:agent` (singular) is reserved for the
//! Phase 51 messaging family. Later Phase 50 plans extend this enum.

/// One `:agents` operation, lowered to [`crate::action::Action::Agents`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentsOp {
    /// `:agents [list]` - per-pane agent state plus a count summary (data over RPC).
    List,
}
