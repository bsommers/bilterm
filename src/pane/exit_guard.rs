use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// Default banner message displayed when a final/root pane shell exits under `Guard` policy.
pub const DEFAULT_HELD_EXITED_BANNER: &str =
    "🔒 [SHELL EXITED — Enter/r: Respawn Shell | d: Detach | :quit!: Exit]";

/// Policy governing how Bilterm handles shell exit, `logout`, and `Ctrl+D` EOF on final or root panes (`SAFE-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExitProtectionPolicy {
    /// Intercept `exit`/`logout`/`Ctrl+D` on final/root panes and open the Borland/3270 Exit Guard modal;
    /// if a raw PTY shell exits on the final/root pane, hold the pane open showing `[SHELL EXITED]`.
    #[default]
    Guard,
    /// Automatically respawn a fresh shell when the final/root pane shell exits.
    Respawn,
    /// Automatically detach the client when the final/root pane shell exits.
    Detach,
    /// Allow immediate exit without interception or hold-on-exit protection.
    Off,
}

impl ExitProtectionPolicy {
    /// Parses a case-insensitive policy name from CLI, `:set`, or `config.toml`.
    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "guard" | "modal" | "prompt" | "confirm" | "on" | "true" => Some(Self::Guard),
            "respawn" | "restart" | "reopen" => Some(Self::Respawn),
            "detach" | "background" => Some(Self::Detach),
            "off" | "none" | "false" | "allow" | "exit" => Some(Self::Off),
            _ => None,
        }
    }

    /// Returns a concise uppercase badge label for status bars and inspector views.
    pub fn label(self) -> &'static str {
        match self {
            Self::Guard => "GUARD",
            Self::Respawn => "RESPAWN",
            Self::Detach => "DETACH",
            Self::Off => "OFF",
        }
    }
}

/// Interactive choice inside the Borland / IBM 3270 Exit Guard modal (`SAFE-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitGuardChoice {
    /// Respawn a fresh login shell inside the current pane (`'r'` / `'R'` / `Enter`).
    RespawnShell,
    /// Detach the current client while preserving the workspace session (`'d'` / `'D'`).
    DetachClient,
    /// Quit the client immediately (`'q'` / `'Q'`).
    QuitClient,
    /// Terminate the entire session (`'K'`).
    KillSession,
    /// Dismiss the Exit Guard modal and return to the pane (`Esc` / `'c'` / `'C'`).
    Cancel,
}

impl ExitGuardChoice {
    /// Ordered list of choices corresponding to `ExitGuardState::selected_index` (`0..5`).
    pub const ALL: [Self; 5] = [
        Self::RespawnShell,
        Self::DetachClient,
        Self::QuitClient,
        Self::KillSession,
        Self::Cancel,
    ];

    /// Maps a keyboard event to an explicit `ExitGuardChoice` hotkey.
    pub fn from_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Self> {
        if modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::ALT) {
            return None;
        }
        match code {
            KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => Some(Self::RespawnShell),
            KeyCode::Char('d') | KeyCode::Char('D') => Some(Self::DetachClient),
            KeyCode::Char('q') | KeyCode::Char('Q') => Some(Self::QuitClient),
            KeyCode::Char('K') => Some(Self::KillSession),
            KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => Some(Self::Cancel),
            _ => None,
        }
    }

    /// Returns the interactive button text rendered inside the Exit Guard modal.
    pub fn button_label(self) -> &'static str {
        match self {
            Self::RespawnShell => "[ (r)espawn shell ]",
            Self::DetachClient => "[ (d)etach client ]",
            Self::QuitClient => "[ (q)uit client ]",
            Self::KillSession => "[ (K)ill session ]",
            Self::Cancel => "[ (Esc) cancel ]",
        }
    }
}

/// State of the active Borland / IBM 3270 Exit Guard confirmation modal (`SAFE-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitGuardState {
    pub pane_id: usize,
    pub session_name: String,
    pub is_root_session: bool,
    pub trigger_reason: String,
    pub selected_index: usize,
}

impl ExitGuardState {
    pub fn new(
        pane_id: usize,
        session_name: impl Into<String>,
        is_root_session: bool,
        trigger_reason: impl Into<String>,
    ) -> Self {
        Self {
            pane_id,
            session_name: session_name.into(),
            is_root_session,
            trigger_reason: trigger_reason.into(),
            selected_index: 0,
        }
    }

    /// Advances selection to the next button (`0..5`, wrapping).
    pub fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % ExitGuardChoice::ALL.len();
    }

    /// Moves selection to the previous button (`0..5`, wrapping).
    pub fn select_prev(&mut self) {
        let len = ExitGuardChoice::ALL.len();
        self.selected_index = (self.selected_index + len - 1) % len;
    }

    /// Returns the currently highlighted `ExitGuardChoice`.
    pub fn selected_choice(&self) -> ExitGuardChoice {
        ExitGuardChoice::ALL
            .get(self.selected_index)
            .copied()
            .unwrap_or(ExitGuardChoice::RespawnShell)
    }
}

/// Hold-on-exit state retained on a pane when its underlying PTY shell exits under `Guard` policy (`SAFE-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldExitedShellState {
    pub exit_code: Option<i32>,
    pub banner_message: String,
}

impl HeldExitedShellState {
    pub fn new(exit_code: Option<i32>) -> Self {
        let banner_message = match exit_code {
            Some(code) => format!(
                "🔒 [SHELL EXITED (code {code}) — Enter/r: Respawn Shell | d: Detach | :quit!: Exit]"
            ),
            None => DEFAULT_HELD_EXITED_BANNER.to_string(),
        };
        Self {
            exit_code,
            banner_message,
        }
    }
}

impl Default for HeldExitedShellState {
    fn default() -> Self {
        Self {
            exit_code: None,
            banner_message: DEFAULT_HELD_EXITED_BANNER.to_string(),
        }
    }
}

/// Returns `true` if `cmd` is a standalone shell exit/logout command (`exit`, `logout`, `exit <code/arg>`,
/// `logout <arg>`, or `:q`) without compound pipeline/chaining operators (`|`, `&`, `;`).
pub fn is_exit_or_logout_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "exit" || trimmed == "logout" || trimmed == ":q" {
        return true;
    }
    if let Some(rest) = trimmed
        .strip_prefix("exit ")
        .or_else(|| trimmed.strip_prefix("logout "))
    {
        let rest = rest.trim();
        !rest.is_empty() && !rest.contains(['|', '&', ';'])
    } else {
        false
    }
}

/// Determines whether submitting `cmd` in the Floating Command Prompt should be intercepted by the Exit Guard.
///
/// Never intercepts when:
/// - `in_alt_screen` (`DECSET 1049` full-screen TUI like `vim`/`htop`/`less`),
/// - `in_echo_lock` (`!ECHO LOCK` password prompt like `sudo`/`ssh`), or
/// - `policy == ExitProtectionPolicy::Off`.
pub fn should_intercept_prompt_exit(
    cmd: &str,
    pane_count: usize,
    is_root_session: bool,
    policy: ExitProtectionPolicy,
    in_alt_screen: bool,
    in_echo_lock: bool,
) -> bool {
    if in_alt_screen || in_echo_lock || policy == ExitProtectionPolicy::Off {
        return false;
    }
    (pane_count <= 1 || is_root_session) && is_exit_or_logout_command(cmd)
}

/// Determines whether pressing `Ctrl+D` (`EOF`) should be intercepted by the Exit Guard.
///
/// Never intercepts when the prompt buffer is non-empty (`Ctrl+D` deletes the character under the cursor),
/// when `in_alt_screen` (`DECSET 1049`), when `in_echo_lock` (`!ECHO LOCK`), or when `policy == Off`.
pub fn should_intercept_ctrl_d(
    prompt_is_empty: bool,
    pane_count: usize,
    is_root_session: bool,
    policy: ExitProtectionPolicy,
    in_alt_screen: bool,
    in_echo_lock: bool,
) -> bool {
    if !prompt_is_empty || in_alt_screen || in_echo_lock || policy == ExitProtectionPolicy::Off {
        return false;
    }
    pane_count <= 1 || is_root_session
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_protection_policy_parses_and_labels() {
        assert_eq!(ExitProtectionPolicy::default(), ExitProtectionPolicy::Guard);
        assert_eq!(ExitProtectionPolicy::from_name("guard"), Some(ExitProtectionPolicy::Guard));
        assert_eq!(ExitProtectionPolicy::from_name("RESPAWN"), Some(ExitProtectionPolicy::Respawn));
        assert_eq!(ExitProtectionPolicy::from_name("detach"), Some(ExitProtectionPolicy::Detach));
        assert_eq!(ExitProtectionPolicy::from_name("off"), Some(ExitProtectionPolicy::Off));
        assert_eq!(ExitProtectionPolicy::from_name("invalid"), None);
        assert_eq!(ExitProtectionPolicy::Guard.label(), "GUARD");
    }

    #[test]
    fn is_exit_or_logout_command_matches_standalone_commands_only() {
        for cmd in ["exit", "  exit  ", "logout", "exit 0", "exit 1", "logout 0", ":q"] {
            assert!(is_exit_or_logout_command(cmd), "expected match for {cmd}");
        }
        for cmd in ["", "echo exit", "exit_status", "exit 0 | cat", "exit 0 && echo hi", "exit 0; ls"] {
            assert!(!is_exit_or_logout_command(cmd), "expected non-match for {cmd}");
        }
    }

    #[test]
    fn should_intercept_prompt_exit_respects_alt_screen_echo_lock_and_policy() {
        let g = ExitProtectionPolicy::Guard;
        assert!(should_intercept_prompt_exit("exit", 1, false, g, false, false));
        assert!(should_intercept_prompt_exit("logout", 3, true, g, false, false));
        assert!(!should_intercept_prompt_exit("exit", 2, false, g, false, false));
        assert!(!should_intercept_prompt_exit("exit", 1, true, g, true, false));
        assert!(!should_intercept_prompt_exit("exit", 1, true, g, false, true));
        assert!(!should_intercept_prompt_exit("exit", 1, true, ExitProtectionPolicy::Off, false, false));
    }

    #[test]
    fn should_intercept_ctrl_d_respects_non_empty_prompt_and_modes() {
        let g = ExitProtectionPolicy::Guard;
        assert!(should_intercept_ctrl_d(true, 1, false, g, false, false));
        assert!(should_intercept_ctrl_d(true, 4, true, g, false, false));
        assert!(!should_intercept_ctrl_d(false, 1, true, g, false, false));
        assert!(!should_intercept_ctrl_d(true, 1, true, g, true, false));
        assert!(!should_intercept_ctrl_d(true, 1, true, g, false, true));
        assert!(!should_intercept_ctrl_d(true, 1, true, ExitProtectionPolicy::Off, false, false));
    }

    #[test]
    fn exit_guard_choice_from_key_and_state_navigation() {
        let none = KeyModifiers::NONE;
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Enter, none), Some(ExitGuardChoice::RespawnShell));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Char('r'), none), Some(ExitGuardChoice::RespawnShell));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Char('d'), none), Some(ExitGuardChoice::DetachClient));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Char('q'), none), Some(ExitGuardChoice::QuitClient));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Char('K'), KeyModifiers::SHIFT), Some(ExitGuardChoice::KillSession));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Esc, none), Some(ExitGuardChoice::Cancel));
        assert_eq!(ExitGuardChoice::from_key(KeyCode::Char('r'), KeyModifiers::CONTROL), None);

        let mut state = ExitGuardState::new(1, "main", true, "Command 'exit' on final pane");
        assert_eq!(state.selected_choice(), ExitGuardChoice::RespawnShell);
        state.select_next();
        assert_eq!(state.selected_choice(), ExitGuardChoice::DetachClient);
        state.select_prev();
        assert_eq!(state.selected_choice(), ExitGuardChoice::RespawnShell);
        state.select_prev();
        assert_eq!(state.selected_choice(), ExitGuardChoice::Cancel);
    }
}
