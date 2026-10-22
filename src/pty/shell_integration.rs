use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Supported shells for zero-config `OSC 133` + `OSC 7` shell integration (`VAL-01`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

impl ShellKind {
    /// Detects the shell kind from the executable path or name.
    pub fn from_program(program: &str) -> Option<Self> {
        let name = Path::new(program)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(program);
        match name {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            _ => None,
        }
    }
}

/// Bash integration script sourced via `--rcfile` for interactive Bash sessions (`VAL-01`).
pub const BASH_INTEGRATION_SCRIPT: &str = r#"# Bilterm zero-config Bash integration (OSC 133 + OSC 7)
if [ -f /etc/bash.bashrc ]; then
    source /etc/bash.bashrc
fi
if [ -f "$HOME/.bashrc" ]; then
    source "$HOME/.bashrc"
fi

__bilterm_prompt_shown=""

__bilterm_precmd() {
    local ret="$?"
    if [ -n "$__bilterm_prompt_shown" ]; then
        printf '\e]133;D;%s\a' "$ret"
    fi
    __bilterm_prompt_shown="1"
    printf '\e]7;file://%s%s\a' "${HOSTNAME:-localhost}" "$PWD"
    printf '\e]133;A\a\e]133;B\a'
}

if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND+=("__bilterm_precmd")
elif [[ "$PROMPT_COMMAND" != *"__bilterm_precmd"* ]]; then
    PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND;}__bilterm_precmd"
fi
PS0='\e]133;C\a'
"#;

/// Zsh `.zshrc` wrapper script injected via `ZDOTDIR` (`VAL-01`).
pub const ZSH_INTEGRATION_SCRIPT: &str = r#"# Bilterm zero-config Zsh integration (OSC 133 + OSC 7)
if [ -n "$BILTERM_ORIG_ZDOTDIR" ]; then
    export ZDOTDIR="$BILTERM_ORIG_ZDOTDIR"
    unset BILTERM_ORIG_ZDOTDIR
else
    unset ZDOTDIR
fi

if [ -f "${ZDOTDIR:-$HOME}/.zshrc" ]; then
    source "${ZDOTDIR:-$HOME}/.zshrc"
fi

autoload -Uz add-zsh-hook

__bilterm_executing=""

__bilterm_precmd() {
    local ret="$?"
    if [ -n "$__bilterm_executing" ]; then
        printf '\e]133;D;%s\a' "$ret"
        __bilterm_executing=""
    fi
    printf '\e]7;file://%s%s\a' "${HOST:-localhost}" "$PWD"
    printf '\e]133;A\a\e]133;B\a'
}

__bilterm_preexec() {
    __bilterm_executing="1"
    printf '\e]133;C\a'
}

add-zsh-hook precmd __bilterm_precmd
add-zsh-hook preexec __bilterm_preexec
"#;

/// Fish integration script injected via `XDG_DATA_DIRS` (`VAL-01`).
pub const FISH_INTEGRATION_SCRIPT: &str = r#"# Bilterm zero-config Fish integration (OSC 133 + OSC 7)
function __bilterm_mark_prompt --on-event fish_prompt
    set -l ret $status
    printf '\e]133;D;%s\a' "$ret"
    printf '\e]7;file://%s%s\a' (hostname) "$PWD"
    printf '\e]133;A\a\e]133;B\a'
end

function __bilterm_mark_preexec --on-event fish_preexec
    printf '\e]133;C\a'
end
"#;

/// Prepares zero-config shell integration environment variables and arguments for an interactive
/// shell spawn (`args.is_empty()`). Does not modify any user dotfiles (`VAL-01`).
pub fn prepare_shell_integration(
    program: &str,
    args: &mut Vec<String>,
    env: &mut BTreeMap<String, String>,
) -> Option<ShellKind> {
    if !args.is_empty() {
        return None;
    }
    if env.get("BILTERM_SHELL_INTEGRATION").map(String::as_str) == Some("0") {
        return None;
    }
    let kind = ShellKind::from_program(program)?;
    let base_dir = shell_integration_root_dir();

    match kind {
        ShellKind::Bash => {
            let bash_dir = base_dir.join("bash");
            fs::create_dir_all(&bash_dir).ok()?;
            let rcfile_path = bash_dir.join("bilterm.bash");
            fs::write(&rcfile_path, BASH_INTEGRATION_SCRIPT).ok()?;
            args.push("--rcfile".to_string());
            args.push(rcfile_path.to_string_lossy().into_owned());
            env.insert("BILTERM_SHELL_INTEGRATION".to_string(), "1".to_string());
        }
        ShellKind::Zsh => {
            let zsh_dir = base_dir.join("zsh");
            fs::create_dir_all(&zsh_dir).ok()?;
            let zshrc_path = zsh_dir.join(".zshrc");
            fs::write(&zshrc_path, ZSH_INTEGRATION_SCRIPT).ok()?;
            if let Some(orig) = env
                .get("ZDOTDIR")
                .cloned()
                .or_else(|| std::env::var("ZDOTDIR").ok())
            {
                env.insert("BILTERM_ORIG_ZDOTDIR".to_string(), orig);
            }
            env.insert(
                "ZDOTDIR".to_string(),
                zsh_dir.to_string_lossy().into_owned(),
            );
            env.insert("BILTERM_SHELL_INTEGRATION".to_string(), "1".to_string());
        }
        ShellKind::Fish => {
            let fish_data_dir = base_dir.join("fish-data");
            let conf_dir = fish_data_dir.join("fish").join("vendor_conf.d");
            fs::create_dir_all(&conf_dir).ok()?;
            let script_path = conf_dir.join("bilterm-shell-integration.fish");
            fs::write(&script_path, FISH_INTEGRATION_SCRIPT).ok()?;
            let existing = env
                .get("XDG_DATA_DIRS")
                .cloned()
                .or_else(|| std::env::var("XDG_DATA_DIRS").ok())
                .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
            env.insert(
                "XDG_DATA_DIRS".to_string(),
                format!("{}:{existing}", fish_data_dir.to_string_lossy()),
            );
            env.insert("BILTERM_SHELL_INTEGRATION".to_string(), "1".to_string());
        }
    }
    Some(kind)
}

fn shell_integration_root_dir() -> PathBuf {
    std::env::temp_dir().join(format!("bilterm-shell-integration-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_bash_zsh_and_fish_and_injects_hooks() {
        let mut args = Vec::new();
        let mut env = BTreeMap::new();
        let kind = prepare_shell_integration("/bin/bash", &mut args, &mut env);
        assert_eq!(kind, Some(ShellKind::Bash));
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--rcfile");
        assert!(Path::new(&args[1]).exists());
        assert_eq!(env.get("BILTERM_SHELL_INTEGRATION").map(String::as_str), Some("1"));

        let mut zsh_args = Vec::new();
        let mut zsh_env = BTreeMap::new();
        let zkind = prepare_shell_integration("/usr/bin/zsh", &mut zsh_args, &mut zsh_env);
        assert_eq!(zkind, Some(ShellKind::Zsh));
        assert!(zsh_env.contains_key("ZDOTDIR"));

        let mut fish_args = Vec::new();
        let mut fish_env = BTreeMap::new();
        let fkind = prepare_shell_integration("fish", &mut fish_args, &mut fish_env);
        assert_eq!(fkind, Some(ShellKind::Fish));
        assert!(fish_env.contains_key("XDG_DATA_DIRS"));
    }

    #[test]
    fn skips_non_interactive_commands_with_explicit_args() {
        let mut args = vec!["-c".to_string(), "echo hi".to_string()];
        let mut env = BTreeMap::new();
        let kind = prepare_shell_integration("/bin/bash", &mut args, &mut env);
        assert_eq!(kind, None);
        assert_eq!(args.len(), 2);
    }
}
