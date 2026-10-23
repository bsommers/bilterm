use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A single tab-completion candidate for the `FloatingCommandPrompt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionCandidate {
    /// Short label displayed in the completion candidate ribbon (e.g. `Cargo.toml` or `src/`).
    pub display: String,
    /// Full replacement string for the token span `[token_start..token_end]` (e.g. `/dev/random ` or `src/`).
    pub replacement: String,
    /// Whether this candidate is a directory (ends with `/`, no trailing space).
    pub is_dir: bool,
}

/// Interactive Tab completion state attached to `FloatingPromptState`.
#[derive(Clone, Debug, Default)]
pub struct TabCompletionState {
    pub active: bool,
    pub token_start: usize,
    pub token_end: usize,
    pub original_prefix: String,
    pub candidates: Vec<CompletionCandidate>,
    pub selected: Option<usize>,
}

impl TabCompletionState {
    pub fn is_active(&self) -> bool {
        self.active && !self.candidates.is_empty()
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.candidates.clear();
        self.selected = None;
    }
}

const SHELL_BUILTINS: &[&str] = &[
    "alias", "bg", "cd", "clear", "echo", "eval", "exec", "exit", "export", "false", "fg",
    "history", "jobs", "kill", "pwd", "read", "set", "source", "test", "true", "type", "unset",
    "which",
];

const COLON_COMMANDS: &[(&str, &[&str])] = &[
    (
        ":prompt",
        &[
            "top",
            "bottom",
            "fixed",
            "floating",
            "terminal",
            "mode",
            "split-top",
            "split-bottom",
            "split",
            "float ",
            "height ",
            "width ",
        ],
    ),
    (":mode", &["float", "fixed", "terminal", "cycle"]),
    (":split", &["h", "v", "horizontal", "vertical"]),
    (":deck", &["next", "prev"]),
    (":focus", &["1", "2", "3", "4"]),
    (":zoom", &[]),
    (":close", &[]),
    (":layout", &["tiled", "stacked"]),
    (":quake", &[]),
    (":guake", &[]),
    (":help", &[]),
    (":history", &["popup", "save", "on", "off", "runs", "max-files "]),
    (":hist", &["popup", "save", "on", "off", "runs", "max-files "]),
    (
        ":outline",
        &[
            "solid",
            "double",
            "rounded",
            "thick",
            "dotted",
            "dashed",
            "quadruple",
            "ascii",
            "window ",
            "prompt ",
        ],
    ),
    (
        ":border",
        &[
            "solid",
            "double",
            "rounded",
            "thick",
            "dotted",
            "dashed",
            "quadruple",
            "ascii",
        ],
    ),
    (":shadows", &["on", "off", "translucent", "tinted", "solid", "none"]),
    (
        ":theme",
        &["turbo-blue", "turbo-cyan", "ibm-3278-green", "ibm-3279-amber", "ibm-mono"],
    ),
    (":glyphs", &["unicode", "cp437", "nerd-font", "ascii"]),
    (":cursor", &["block", "hollow", "underscore", "bar"]),
    (":crosshair", &["on", "off", "full", "pane"]),
    (
        ":spotlight",
        &[
            "circle",
            "ellipse",
            "pixel",
            "fuzzy",
            "full",
            "pane",
            "+",
            "-",
            "size ",
            "feather ",
            "off",
        ],
    ),
    (
        ":focalzoom",
        &[
            "1.25x",
            "1.5x",
            "2x",
            "3x",
            "circle",
            "ellipse",
            "fuzzy",
            "pixel",
            "+",
            "-",
            "grow",
            "shrink",
            "size ",
            "feather ",
            "off",
        ],
    ),
    (
        ":lens",
        &[
            "1.25x",
            "1.5x",
            "2x",
            "3x",
            "circle",
            "ellipse",
            "fuzzy",
            "pixel",
            "off",
        ],
    ),
    (":fkeybar", &["on", "off"]),
    (":search", &["next", "prev", "off"]),
    (":find", &["next", "prev", "off"]),
    (":copy", &[]),
    (":links", &[]),
    (":url", &[]),
    (":inspector", &["on", "off", "toggle"]),
    (":hints", &[]),
    (":lock", &["on", "off", "toggle"]),
    (":broadcast", &["on", "off", "toggle"]),
    (":menubar", &["on", "off", "toggle"]),
    (":session", &["save ", "restore ", "list"]),
    (":block", &["prev", "next"]),
    (":yank", &[]),
    (":clear", &[]),
    (":raw", &[]),
    (":quit", &[]),
    (":q", &[]),
];

/// Computes sorted, deduplicated completion candidates for `line` at character offset `cursor_col`.
///
/// Returns `(token_start_char_idx, token_end_char_idx, candidates)`.
pub fn compute_completions(
    line: &str,
    cursor_col: usize,
    cwd: &Path,
) -> (usize, usize, Vec<CompletionCandidate>) {
    let chars: Vec<char> = line.chars().collect();
    let col = cursor_col.min(chars.len());
    let prefix_slice: String = chars[..col].iter().collect();

    // 1. Check for in-prompt :colon command completion
    let trimmed_start = prefix_slice.trim_start();
    if trimmed_start.starts_with(':') {
        let leading_ws = prefix_slice.chars().count() - trimmed_start.chars().count();
        if let Some(space_char_idx) = trimmed_start.chars().position(char::is_whitespace) {
            let cmd_name: String = trimmed_start.chars().take(space_char_idx).collect();
            let after_cmd: String = trimmed_start.chars().skip(space_char_idx).collect();
            let arg_trimmed = after_cmd.trim_start();
            let arg_ws = after_cmd.chars().count() - arg_trimmed.chars().count();
            let arg_start = leading_ws + space_char_idx + arg_ws;
            let mut out = Vec::new();
            for &(cmd, subargs) in COLON_COMMANDS {
                if cmd == cmd_name {
                    for &sub in subargs {
                        if sub.starts_with(arg_trimmed) {
                            out.push(CompletionCandidate {
                                display: sub.trim().to_string(),
                                replacement: sub.to_string(),
                                is_dir: sub.ends_with(' '),
                            });
                        }
                    }
                }
            }
            return (arg_start, col, out);
        } else {
            let mut out = Vec::new();
            for &(cmd, subargs) in COLON_COMMANDS {
                if cmd.starts_with(trimmed_start) {
                    let suffix = if subargs.is_empty() { "" } else { " " };
                    out.push(CompletionCandidate {
                        display: cmd.to_string(),
                        replacement: format!("{cmd}{suffix}"),
                        is_dir: !subargs.is_empty(),
                    });
                }
            }
            return (leading_ws, col, out);
        }
    }

    // Find start of current shell token before `col`
    let mut token_start = col;
    while token_start > 0 {
        let prev = chars[token_start - 1];
        if prev.is_whitespace() || matches!(prev, '|' | ';' | '&' | '(' | ')') {
            break;
        }
        token_start -= 1;
    }

    let token: String = chars[token_start..col].iter().collect();

    // Determine whether this token is in command position (first token on line or after |, ;, &)
    let before_token: String = chars[..token_start].iter().collect();
    let before_trimmed = before_token.trim_end();
    let is_command_pos = before_trimmed.is_empty()
        || before_trimmed.ends_with('|')
        || before_trimmed.ends_with(';')
        || before_trimmed.ends_with('&');

    // 2. Environment variable completion ($VAR or ${VAR})
    if let Some(var_prefix) = token.strip_prefix("${") {
        let var_clean = var_prefix.strip_suffix('}').unwrap_or(var_prefix);
        let mut out = Vec::new();
        for (k, _) in std::env::vars() {
            if k.starts_with(var_clean) {
                out.push(CompletionCandidate {
                    display: format!("${{{k}}}"),
                    replacement: format!("${{{k}}}"),
                    is_dir: false,
                });
            }
        }
        out.sort_by(|a, b| a.display.cmp(&b.display));
        return (token_start, col, out);
    } else if let Some(var_prefix) = token.strip_prefix('$') {
        let mut out = Vec::new();
        for (k, _) in std::env::vars() {
            if k.starts_with(var_prefix) {
                out.push(CompletionCandidate {
                    display: format!("${k}"),
                    replacement: format!("${k}"),
                    is_dir: false,
                });
            }
        }
        out.sort_by(|a, b| a.display.cmp(&b.display));
        return (token_start, col, out);
    }

    // 3. Command position without path separators: complete builtins + $PATH executables + local dirs/files
    let looks_like_path =
        token.contains('/') || token.starts_with('.') || token.starts_with('~');
    if is_command_pos && !looks_like_path && !token.is_empty() {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();

        for &b in SHELL_BUILTINS {
            if b.starts_with(&token) && seen.insert(b.to_string()) {
                out.push(CompletionCandidate {
                    display: b.to_string(),
                    replacement: format!("{b} "),
                    is_dir: false,
                });
            }
        }

        if let Some(path_os) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_os) {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str()
                            && name.starts_with(&token) && seen.insert(name.to_string()) {
                                out.push(CompletionCandidate {
                                    display: name.to_string(),
                                    replacement: format!("{name} "),
                                    is_dir: false,
                                });
                            }
                    }
                }
            }
        }

        // Also include matching subdirectories/files in cwd
        for cand in complete_filesystem_path(&token, cwd) {
            if seen.insert(cand.display.clone()) {
                out.push(cand);
            }
        }

        out.sort_by(|a, b| a.display.cmp(&b.display));
        out.truncate(64);
        return (token_start, col, out);
    }

    // 4. Filesystem path completion
    let out = complete_filesystem_path(&token, cwd);
    (token_start, col, out)
}

fn complete_filesystem_path(token: &str, cwd: &Path) -> Vec<CompletionCandidate> {
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);

    let (dir_prefix_for_replacement, search_dir, file_prefix) = if token == "~" {
        if home_dir.is_some() {
            return vec![CompletionCandidate {
                display: "~/".to_string(),
                replacement: "~/".to_string(),
                is_dir: true,
            }];
        }
        (String::new(), cwd.to_path_buf(), token.to_string())
    } else if let Some(rest) = token.strip_prefix("~/") {
        let base = home_dir.unwrap_or_else(|| cwd.to_path_buf());
        split_dir_and_file_prefix("~/", &base, rest)
    } else if let Some(rest) = token.strip_prefix('/') {
        split_dir_and_file_prefix("/", Path::new("/"), rest)
    } else {
        split_dir_and_file_prefix("", cwd, token)
    };

    let show_hidden = file_prefix.starts_with('.');
    let Ok(read_dir) = std::fs::read_dir(&search_dir) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for entry in read_dir.flatten() {
        let file_name_os = entry.file_name();
        let Some(name) = file_name_os.to_str() else {
            continue;
        };
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if !name.starts_with(&file_prefix) {
            continue;
        }

        let is_dir = entry
            .file_type()
            .map(|ft| ft.is_dir())
            .unwrap_or_else(|_| entry.path().is_dir());

        let display = if is_dir {
            format!("{name}/")
        } else {
            name.to_string()
        };
        let replacement = if is_dir {
            format!("{dir_prefix_for_replacement}{name}/")
        } else {
            format!("{dir_prefix_for_replacement}{name}")
        };

        candidates.push(CompletionCandidate {
            display,
            replacement,
            is_dir,
        });
    }

    candidates.sort_by(|a, b| a.display.cmp(&b.display));
    candidates.truncate(64);
    candidates
}

fn split_dir_and_file_prefix(
    root_prefix: &str,
    base_dir: &Path,
    rel_token: &str,
) -> (String, PathBuf, String) {
    if let Some((dir_part, file_part)) = rel_token.rsplit_once('/') {
        let search_dir = if dir_part.is_empty() {
            base_dir.to_path_buf()
        } else {
            base_dir.join(dir_part)
        };
        let prefix = if dir_part.is_empty() {
            root_prefix.to_string()
        } else {
            format!("{root_prefix}{dir_part}/")
        };
        (prefix, search_dir, file_part.to_string())
    } else {
        (
            root_prefix.to_string(),
            base_dir.to_path_buf(),
            rel_token.to_string(),
        )
    }
}

/// Computes the longest common character prefix across candidate replacements.
pub fn longest_common_prefix(candidates: &[CompletionCandidate]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut prefix_chars: Vec<char> = first.replacement.chars().collect();
    for cand in &candidates[1..] {
        let cand_chars: Vec<char> = cand.replacement.chars().collect();
        let common_len = prefix_chars
            .iter()
            .zip(cand_chars.iter())
            .take_while(|(a, b)| a == b)
            .count();
        prefix_chars.truncate(common_len);
        if prefix_chars.is_empty() {
            break;
        }
    }
    prefix_chars.into_iter().collect()
}
