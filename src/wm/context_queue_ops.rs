//! Operation enums for `:context` and `:agent queue|approve|deny` (UNI-R01, Phase 68).

/// `:context ...` operation on the shared [`crate::wm::context_bus::ContextBus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextOp {
    Set { key: String, value: String },
    Get { key: String },
    List { prefix: Option<String> },
    /// `:context clear <key>` removes one key (tombstone).
    Clear { key: String },
}

impl ContextOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Set { key, value } => format!(":context set {key} {value}"),
            Self::Get { key } => format!(":context get {key}"),
            Self::List { prefix: None } => ":context list".to_string(),
            Self::List { prefix: Some(p) } => format!(":context list {p}"),
            Self::Clear { key } => format!(":context clear {key}"),
        }
    }
}

/// `:agent queue ...` operation on the active pane's [`crate::wm::prompt_queue::PromptQueue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentQueueOp {
    List,
    Add(String),
    Clear,
    /// 1-based index as shown by `list`.
    Drop(usize),
}

impl AgentQueueOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::List => ":agent queue list".to_string(),
            Self::Add(t) => format!(":agent queue add {t}"),
            Self::Clear => ":agent queue clear".to_string(),
            Self::Drop(n) => format!(":agent queue drop {n}"),
        }
    }
}

/// `:agent approve|deny` - answer the blocked-prompt classifier result for the active pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentGateOp {
    Approve,
    Deny,
}

impl AgentGateOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Approve => ":agent approve".to_string(),
            Self::Deny => ":agent deny".to_string(),
        }
    }
}
