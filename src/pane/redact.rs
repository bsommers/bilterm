//! Secret / exclusion classifier for captured commands (Phase 50.1, D-U2).
//!
//! 50.1-01 ships the `Empty` and `Colon` rules; plan 50.1-02 fills the `Secret` heuristics
//! (`50.1-CONTRACT.md` section 9). Pane layer: imports nothing outside `std`.

/// Why a command is not recorded or indexed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
    /// Blank command.
    Empty,
    /// A bilterm `:colon` command (never a shell command).
    Colon,
    /// Looks like it carries a credential.
    Secret,
}

/// Classifies `command`; `None` means it may be stored and indexed.
pub fn classify(command: &str) -> Option<Exclusion> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Some(Exclusion::Empty);
    }
    if trimmed.starts_with(':') {
        return Some(Exclusion::Colon);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_colon_are_excluded_plain_commands_are_not() {
        assert_eq!(classify(""), Some(Exclusion::Empty));
        assert_eq!(classify("  \t "), Some(Exclusion::Empty));
        assert_eq!(classify(":find agent"), Some(Exclusion::Colon));
        assert_eq!(classify("  :q"), Some(Exclusion::Colon));
        assert_eq!(classify("ls -la"), None);
    }
}
