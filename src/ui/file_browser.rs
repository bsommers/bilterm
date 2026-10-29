//! Interactive Text File & Directory Browser and Smart `:theme import` Resolver (`THM-01`).
//!
//! Supports:
//! - File paths (`/path/to/Dracula.itermcolors`, `~/my-theme.toml`)
//! - Directory paths (`/home/bill/styling/iterm2colors`, `/home/bill/styling/iterm2colors/schemes`)
//!   with automatic subdirectory discovery (`schemes/`, `themes/`, `alacritty/`, etc.)
//! - Wildcard globs (`/home/bill/styling/iterm2colors/schemes/*.itermcolors`, `*Solarized*`)
//! - Interactive keyboard & mouse File/Directory Browser (`:browse`, `:files`, `:theme browse`, `:theme import`)
//!   with live type-to-filter, parent/child directory navigation, instant `.itermcolors` application,
//!   and text file preview.

use std::path::{Path, PathBuf};

use crate::mcp_console::McpRowAction;

/// Priority subdirectory names inside theme repositories like `iterm2-color-schemes`.
const PRIORITY_THEME_SUBDIRS: &[&str] = &["schemes", "themes", "iterm2", "alacritty", "wezterm", "base16"];

/// Non-theme project configuration filenames to skip when auto-discovering `.toml`/`.yaml` themes.
const NON_THEME_FILENAMES: &[&str] = &[
    "cargo.toml", "pyproject.toml", "netlify.toml", "rust-toolchain.toml", "rustfmt.toml",
    "clippy.toml", "config.toml", "makefile", "package.json", "docker-compose.yml", "docker-compose.yaml",
];

/// Returns the user's Bilterm configuration directory (`$XDG_CONFIG_HOME/bilterm` or `~/.config/bilterm`).
pub fn user_config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("bilterm"));
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".config").join("bilterm"))
}

/// Ensures `~/.config/bilterm/` and `~/.config/bilterm/themes/` exist and seeds a documented
/// `~/.config/bilterm/config.toml` if none exists yet.
pub fn ensure_user_config_initialized() -> Option<PathBuf> {
    let cfg_dir = user_config_dir()?;
    let themes_dir = cfg_dir.join("themes");
    let _ = std::fs::create_dir_all(&themes_dir);
    let cfg_file = cfg_dir.join("config.toml");
    if !cfg_file.exists() {
        let default_toml = "\
# Bilterm Configuration (~/.config/bilterm/config.toml)
# Custom .itermcolors / .toml / .yaml themes can be placed in ~/.config/bilterm/themes/
# or imported from any directory via `:theme import <path|dir|glob>` or `:browse`.

theme = \"turbo-blue\"
glyphs = \"unicode\"
cursor_style = \"solid-block\"      # solid-block | blinking-hollow-block | underscore | vertical-bar | custom-char
cursor_custom_char = \"◆\"          # Custom glyph when cursor_style = \"custom-char\"
cursor_blink = \"auto\"             # auto | solid | blink | rapid-blink
# cursor_color = \"#00FF66\"        # Optional hex or named color (green, amber, cyan, yellow, crimson, magenta, white)
# cursor_text_color = \"#000000\"   # Optional text color inside block cursor
window_outline = \"double\"
prompt_outline = \"double\"
drop_shadows = true
shadow_style = \"translucent\"
crosshair_cursor = false
show_top_menubar = true
show_dos_fkey_bar = true
";
        let _ = std::fs::write(&cfg_file, default_toml);
    }
    Some(cfg_dir)
}

/// Persists a directory path into `~/.config/bilterm/themes/sources.txt` so discovered theme
/// repositories remain available across sessions.
pub fn register_theme_source_dir(dir: &Path) {
    let Some(cfg_dir) = ensure_user_config_initialized() else {
        return;
    };
    let canonical = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let sources_file = cfg_dir.join("themes").join("sources.txt");
    let mut existing: Vec<String> = std::fs::read_to_string(&sources_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let line = canonical.to_string_lossy().to_string();
    if !existing.iter().any(|e| e == &line) {
        existing.push(line);
        let _ = std::fs::write(&sources_file, format!("{}\n", existing.join("\n")));
    }
}

/// Reads registered theme directories from `~/.config/bilterm/themes/sources.txt`.
pub fn registered_theme_source_dirs() -> Vec<PathBuf> {
    let Some(cfg_dir) = user_config_dir() else {
        return Vec::new();
    };
    let sources_file = cfg_dir.join("themes").join("sources.txt");
    std::fs::read_to_string(sources_file)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

/// Expands `~`, `~/...`, and `$HOME/...` and strips surrounding quotes from a user path string.
pub fn expand_user_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed == "~"
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home);
    }
    if let Some(rest) = trimmed.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("$HOME/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(trimmed)
}

/// Case-insensitive wildcard matcher supporting `*` and `?`.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pat = pattern.trim();
    if pat.is_empty() || pat == "*" {
        return true;
    }
    let p: Vec<char> = if pat.contains('*') || pat.contains('?') {
        pat.to_ascii_lowercase().chars().collect()
    } else {
        format!("*{}*", pat.to_ascii_lowercase()).chars().collect()
    };
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();

    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti) = (None, 0usize);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            pi += 1;
            star_ti = ti;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Returns `true` if `path` has a supported open color scheme extension (`.itermcolors`, `.toml`, `.yaml`, `.yml`).
pub fn is_theme_file_path(path: &Path) -> bool {
    let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower_name = fname.to_ascii_lowercase();
    if NON_THEME_FILENAMES.contains(&lower_name.as_str()) || lower_name.starts_with('.') {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "itermcolors" | "toml" | "yaml" | "yml"
            )
        })
}

/// Lists theme files directly inside `dir` matching optional `filter_glob`, without parsing every file upfront.
pub fn list_theme_paths_in_dir(dir: &Path, filter_glob: Option<&str>) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_theme_file_path(p))
        .filter(|p| {
            let Some(pat) = filter_glob else {
                return true;
            };
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            wildcard_match(pat, fname)
        })
        .collect();
    out.sort();
    out
}

/// Smartly discovers theme files in `dir` or its known theme subdirectories (`schemes/`, `themes/`, etc.).
///
/// When given a repository root like `/home/bill/styling/iterm2colors`, automatically pivots to
/// `/home/bill/styling/iterm2colors/schemes` (where `.itermcolors` files reside) and collects any
/// additional theme files from sibling subdirectories.
pub fn discover_theme_paths_smart(
    dir: &Path,
    filter_glob: Option<&str>,
) -> (PathBuf, Vec<PathBuf>) {
    let direct = list_theme_paths_in_dir(dir, filter_glob);
    if !direct.is_empty() {
        return (dir.to_path_buf(), direct);
    }

    // Check priority subdirectories first (`schemes/`, `themes/`, `iterm2/`, `alacritty/`, etc.)
    let mut effective_dir = dir.to_path_buf();
    let mut collected = Vec::new();
    for &sub in PRIORITY_THEME_SUBDIRS {
        let candidate = dir.join(sub);
        if candidate.is_dir() {
            let sub_files = list_theme_paths_in_dir(&candidate, filter_glob);
            if !sub_files.is_empty() {
                if collected.is_empty() {
                    effective_dir = candidate;
                }
                collected.extend(sub_files);
            }
        }
    }
    if !collected.is_empty() {
        return (effective_dir, collected);
    }

    // Fallback: scan any immediate child directory for `.itermcolors` files
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut subdirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        for sub in subdirs {
            let sub_files = list_theme_paths_in_dir(&sub, filter_glob);
            if !sub_files.is_empty() {
                if collected.is_empty() {
                    effective_dir = sub;
                }
                collected.extend(sub_files);
            }
        }
    }
    (effective_dir, collected)
}

/// Resolved target for `:theme import [spec]` / `:browse [spec]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeImportTarget {
    /// Open interactive file/directory browser at `dir` with optional `filter`.
    OpenBrowser {
        dir: PathBuf,
        filter: Option<String>,
    },
    /// Import and immediately apply a single theme file.
    SingleFile(PathBuf),
    /// Import a directory (or wildcard glob within a directory), applying the first match and opening the browser.
    Directory {
        effective_dir: PathBuf,
        files: Vec<PathBuf>,
        filter: Option<String>,
    },
}

/// Resolves a user `:theme import <spec>` argument (empty, file path, directory path, or wildcard glob).
pub fn resolve_theme_import_target(raw_spec: &str) -> ThemeImportTarget {
    let trimmed = raw_spec.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        return ThemeImportTarget::OpenBrowser {
            dir: default_browser_start_dir(),
            filter: None,
        };
    }

    let expanded = expand_user_path(trimmed);

    // 1. Check if the spec contains wildcard characters (`*` or `?`)
    let expanded_str = expanded.to_string_lossy();
    if expanded_str.contains('*') || expanded_str.contains('?') {
        let parent = expanded
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(default_browser_start_dir);
        let pat = expanded
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("*")
            .to_string();
        if parent.is_dir() {
            let (effective_dir, files) = discover_theme_paths_smart(&parent, Some(&pat));
            return ThemeImportTarget::Directory {
                effective_dir,
                files,
                filter: Some(pat),
            };
        }
    }

    // 2. Existing directory
    if expanded.is_dir() {
        let (effective_dir, files) = discover_theme_paths_smart(&expanded, None);
        return ThemeImportTarget::Directory {
            effective_dir,
            files,
            filter: None,
        };
    }

    // 3. Existing file
    if expanded.is_file() {
        return ThemeImportTarget::SingleFile(expanded);
    }

    // 4. If parent directory exists and final segment is a partial name/filter, open directory with filter
    if let Some(parent) = expanded.parent()
        && !parent.as_os_str().is_empty()
        && parent.is_dir()
        && let Some(fname) = expanded.file_name().and_then(|s| s.to_str())
    {
        let (effective_dir, files) = discover_theme_paths_smart(parent, Some(fname));
        if files.len() == 1 {
            return ThemeImportTarget::SingleFile(files[0].clone());
        }
        if !files.is_empty() {
            return ThemeImportTarget::Directory {
                effective_dir,
                files,
                filter: Some(fname.to_string()),
            };
        }
    }

    ThemeImportTarget::SingleFile(expanded)
}

/// Chooses the best initial directory when opening the File/Theme Browser without a path argument.
pub fn default_browser_start_dir() -> PathBuf {
    for dir in registered_theme_source_dirs() {
        if dir.is_dir() {
            return dir;
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for candidate in [
            home.join("styling/iterm2colors/schemes"),
            home.join("styling/iterm2colors"),
            home.join("iterm2-color-schemes/schemes"),
            home.join(".config/bilterm/themes"),
            home.join(".config/bilterm"),
        ] {
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Interactive state for the in-TUI Text File & Directory Browser (`THM-01`).
#[derive(Clone, Debug)]
pub struct FileBrowserState {
    pub current_dir: PathBuf,
    pub filter: String,
    pub preview_file: Option<(PathBuf, Vec<String>)>,
}

impl FileBrowserState {
    /// Creates a new [`FileBrowserState`] rooted at `dir` with an optional initial `filter`.
    pub fn new(dir: PathBuf, filter: Option<String>) -> Self {
        let resolved_dir = if dir.is_dir() {
            std::fs::canonicalize(&dir).unwrap_or(dir)
        } else if let Some(parent) = dir.parent().filter(|p| p.is_dir()) {
            std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf())
        } else {
            default_browser_start_dir()
        };
        Self {
            current_dir: resolved_dir,
            filter: filter.unwrap_or_default(),
            preview_file: None,
        }
    }

    /// Navigates into `dir` and clears any active file preview.
    pub fn navigate_to(&mut self, dir: PathBuf) {
        if dir.is_dir() {
            self.current_dir = std::fs::canonicalize(&dir).unwrap_or(dir);
            self.preview_file = None;
        }
    }

    /// Navigates up to the parent directory (`..`), or closes file preview if one is open.
    pub fn navigate_up(&mut self) -> bool {
        if self.preview_file.is_some() {
            self.preview_file = None;
            return true;
        }
        if let Some(parent) = self.current_dir.parent().map(Path::to_path_buf)
            && parent != self.current_dir
        {
            self.current_dir = parent;
            return true;
        }
        false
    }

    /// Loads a text file into the preview buffer (up to 200 lines).
    pub fn open_file_preview(&mut self, path: PathBuf) -> Result<(), String> {
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<String> = text.lines().take(200).map(str::to_string).collect();
        self.preview_file = Some((path, lines));
        Ok(())
    }

    /// Builds the formatted lines and interactive [`McpRowAction`] table for `McpConsoleState`.
    pub fn build_view(&self) -> (String, Vec<String>, Vec<McpRowAction>) {
        if let Some((ref file_path, ref content_lines)) = self.preview_file {
            let title = format!("FILE VIEWER // {}", file_path.display());
            let mut lines = vec![
                format!("📄 FILE: {} ({} lines shown)", file_path.display(), content_lines.len()),
                "  ⬅ [BACK TO DIRECTORY BROWSER] (Enter / Backspace)".to_string(),
            ];
            let mut actions = vec![
                McpRowAction::None,
                McpRowAction::BrowseDir(self.current_dir.clone()),
            ];
            if is_theme_file_path(file_path) {
                let name = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("theme");
                lines.push(format!("  🎨 [IMPORT & APPLY THIS THEME] ({name})"));
                actions.push(McpRowAction::ApplyTheme(format!("import {}", file_path.display())));
            }
            lines.push("─".repeat(72));
            actions.push(McpRowAction::None);
            for (idx, line) in content_lines.iter().enumerate() {
                lines.push(format!("{:4} │ {}", idx + 1, line));
                actions.push(McpRowAction::None);
            }
            return (title, lines, actions);
        }

        let title = format!("FILE & THEME BROWSER // {}", self.current_dir.display());
        let filter_display = if self.filter.is_empty() {
            "* (type characters to filter, Backspace=up/del, Ctrl+U=clear)".to_string()
        } else {
            format!("'{}' (Backspace=edit, Ctrl+U=clear)", self.filter)
        };
        let cfg_hint = user_config_dir()
            .map(|d| d.join("config.toml").display().to_string())
            .unwrap_or_else(|| "~/.config/bilterm/config.toml".to_string());

        let mut lines = vec![
            format!("📂 DIR: {}   │   CONFIG: {cfg_hint}", self.current_dir.display()),
            format!("🔍 FILTER: {filter_display}"),
            "─".repeat(72),
        ];
        let mut actions = vec![McpRowAction::None, McpRowAction::None, McpRowAction::None];

        if let Some(parent) = self.current_dir.parent().filter(|p| *p != self.current_dir) {
            lines.push(format!("  📁 [..]  (Up to {})", parent.display()));
            actions.push(McpRowAction::BrowseDir(parent.to_path_buf()));
        }

        let (mut subdirs, mut theme_files, mut text_files) = (Vec::new(), Vec::new(), Vec::new());
        if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if (fname.starts_with('.') && fname != ".config")
                    || (!self.filter.is_empty() && !wildcard_match(&self.filter, fname))
                {
                    continue;
                }
                if p.is_dir() {
                    subdirs.push(p);
                } else if p.is_file() {
                    if is_theme_file_path(&p) {
                        theme_files.push(p);
                    } else {
                        text_files.push(p);
                    }
                }
            }
        }

        subdirs.sort_by(|a, b| {
            let a_n = a.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let b_n = b.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let a_p = PRIORITY_THEME_SUBDIRS.contains(&a_n.to_ascii_lowercase().as_str());
            let b_p = PRIORITY_THEME_SUBDIRS.contains(&b_n.to_ascii_lowercase().as_str());
            b_p.cmp(&a_p).then_with(|| a_n.cmp(b_n))
        });
        theme_files.sort();
        text_files.sort();

        if !theme_files.is_empty() {
            lines.push(format!("  ✨ [IMPORT & REGISTER ALL {} THEME(S) IN THIS DIRECTORY]", theme_files.len()));
            actions.push(McpRowAction::ImportAllDir(self.current_dir.clone()));
        }
        for sub in &subdirs {
            let name = sub.file_name().and_then(|s| s.to_str()).unwrap_or("dir");
            let badge = if PRIORITY_THEME_SUBDIRS.contains(&name.to_ascii_lowercase().as_str()) { " ★ THEME DIR" } else { "" };
            lines.push(format!("  📁 [DIR] {name}/{badge}"));
            actions.push(McpRowAction::BrowseDir(sub.clone()));
        }
        for tf in &theme_files {
            let name = tf.file_name().and_then(|s| s.to_str()).unwrap_or("theme");
            lines.push(format!("  🎨 {name} — Enter/Click to import & apply"));
            actions.push(McpRowAction::ApplyTheme(format!("import {}", tf.display())));
        }
        for f in text_files.iter().take(100) {
            let name = f.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            let sz = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            lines.push(format!("  📄 {name} ({sz} B) — Enter/Click to preview"));
            actions.push(McpRowAction::PreviewFile(f.clone()));
        }
        if subdirs.is_empty() && theme_files.is_empty() && text_files.is_empty() {
            lines.push("  (No entries matching filter in this directory)".to_string());
            actions.push(McpRowAction::None);
        }
        (title, lines, actions)
    }
}

/// Copies an imported theme file into `~/.config/bilterm/themes/` so it persists across restarts.
pub fn persist_imported_theme_file(src: &Path) {
    let Some(cfg_dir) = ensure_user_config_initialized() else {
        return;
    };
    let themes_dir = cfg_dir.join("themes");
    if let Some(fname) = src.file_name() {
        let dst = themes_dir.join(fname);
        if src != dst {
            let _ = std::fs::copy(src, dst);
        }
    }
}

/// Handles [`McpRowAction`] variants specific to the File & Theme Browser (`BrowseDir`, `ImportAllDir`, `PreviewFile`).
pub fn activate_browser_row_action(
    wm: &mut crate::wm::WindowManager,
    action: McpRowAction,
) -> bool {
    match action {
        McpRowAction::BrowseDir(dir) => {
            if let Some(ref mut fb) = wm.mcp_console_mut().file_browser {
                fb.navigate_to(dir.clone());
                wm.mcp_console_mut().refresh_file_browser();
            } else {
                wm.mcp_console_mut().open_file_browser(dir.clone(), None);
            }
            wm.active_pane_mut()
                .set_status_message(Some(format!("Browsing {}", dir.display())));
            true
        }
        McpRowAction::ImportAllDir(dir) => {
            register_theme_source_dir(&dir);
            let (effective_dir, files) = discover_theme_paths_smart(&dir, None);
            if let Some(first_path) = files.first()
                && let Ok(scheme) = crate::ui::theme_import::load_open_theme_file(first_path)
            {
                wm.apply_open_color_scheme(&scheme);
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Imported & registered {} theme(s) from {}; applied '{}'",
                    files.len(),
                    effective_dir.display(),
                    scheme.name
                )));
            } else {
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Registered theme directory {}",
                    effective_dir.display()
                )));
            }
            true
        }
        McpRowAction::PreviewFile(path) => {
            let res = if let Some(ref mut fb) = wm.mcp_console_mut().file_browser {
                fb.open_file_preview(path.clone())
            } else {
                let mut fb = FileBrowserState::new(
                    path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
                    None,
                );
                let r = fb.open_file_preview(path.clone());
                wm.mcp_console_mut().file_browser = Some(fb);
                r
            };
            match res {
                Ok(()) => {
                    wm.mcp_console_mut().refresh_file_browser();
                    wm.active_pane_mut()
                        .set_status_message(Some(format!("Viewing file {}", path.display())));
                }
                Err(e) => {
                    wm.active_pane_mut().set_status_message(Some(e));
                }
            }
            true
        }
        _ => false,
    }
}

/// Executes `:theme <subcmd>` and `:browse [path]` commands (`THM-01`, `THM-02`).
pub fn execute_theme_or_browser_command(wm: &mut crate::wm::WindowManager, subcmd: &str) {
    use crate::ui::theme::ThemePreset;
    use crate::ui::theme_import::{
        OpenColorScheme, build_theme_catalog_lines, export_open_theme_toml, load_open_theme_file,
        resolve_theme_file_or_name,
    };

    let _ = ensure_user_config_initialized();
    let parts: Vec<&str> = subcmd.split_whitespace().collect();
    match parts.first().copied() {
        Some("cursor") => {
            crate::ui::cursor_cmd::execute_cursor_command(wm, subcmd);
        }
        None | Some("list" | "ls" | "itermcolors" | "iterm2colors") => {
            let raw_extra = parts[1..].join(" ");
            let extra_trimmed = raw_extra.trim();
            if !extra_trimmed.is_empty()
                && !matches!(extra_trimmed, "list" | "ls" | "itermcolors" | "iterm2colors")
            {
                execute_theme_or_browser_command(wm, &format!("import {extra_trimmed}"));
                return;
            }
            let names: Vec<&str> = ThemePreset::all().iter().map(|p| p.label()).collect();
            let lines = build_theme_catalog_lines(
                wm.config().theme,
                wm.config().custom_palette_override.is_some(),
                None,
            );
            wm.open_mcp_popup_window("AVAILABLE THEME PRESETS (THM-01)", lines);
            wm.active_pane_mut().set_status_message(Some(format!(
                "Themes ({}): {}",
                names.len(),
                names.join(", ")
            )));
        }
        Some("browse" | "files" | "dir" | "open" | "explorer") => {
            let raw_arg = parts[1..].join(" ");
            let expanded = expand_user_path(&raw_arg);
            if !raw_arg.trim().is_empty() && expanded.is_dir() {
                wm.mcp_console_mut().open_file_browser(expanded.clone(), None);
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Browsing {} (↑/↓/Enter, type to filter, Backspace=up)",
                    expanded.display()
                )));
                return;
            }
            match resolve_theme_import_target(&raw_arg) {
                ThemeImportTarget::OpenBrowser { dir, filter }
                | ThemeImportTarget::Directory { effective_dir: dir, filter, .. } => {
                    wm.mcp_console_mut().open_file_browser(dir.clone(), filter);
                    wm.active_pane_mut().set_status_message(Some(format!(
                        "File & Theme Browser: {} (↑/↓/Enter, type to filter, Backspace=up)",
                        dir.display()
                    )));
                }
                ThemeImportTarget::SingleFile(path) => {
                    if path.is_file() && is_theme_file_path(&path) {
                        execute_theme_or_browser_command(wm, &format!("import {}", path.display()));
                    } else if path.is_file() {
                        let _ = activate_browser_row_action(wm, McpRowAction::PreviewFile(path));
                    } else {
                        wm.mcp_console_mut().open_file_browser(default_browser_start_dir(), Some(raw_arg));
                    }
                }
            }
        }
        Some("config" | "where") => {
            if let Some(cfg_dir) = ensure_user_config_initialized() {
                let cfg_file = cfg_dir.join("config.toml");
                let _ = activate_browser_row_action(wm, McpRowAction::PreviewFile(cfg_file.clone()));
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Bilterm config: {} (themes: {})",
                    cfg_file.display(),
                    cfg_dir.join("themes").display()
                )));
            }
        }
        Some("import" | "load") => {
            let raw_arg = parts[1..].join(" ");
            let path_str = raw_arg.trim().trim_matches('"').trim_matches('\'');
            if path_str.is_empty() {
                let dir = default_browser_start_dir();
                wm.mcp_console_mut().open_file_browser(dir.clone(), None);
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Opened Theme & File Browser at {} — select a file/dir or use :theme import <path|dir|glob>",
                    dir.display()
                )));
                return;
            }
            match resolve_theme_import_target(path_str) {
                ThemeImportTarget::OpenBrowser { dir, filter } => {
                    wm.mcp_console_mut().open_file_browser(dir, filter);
                }
                ThemeImportTarget::Directory { effective_dir, files, filter } => {
                    register_theme_source_dir(&effective_dir);
                    wm.mcp_console_mut().open_file_browser(effective_dir.clone(), filter);
                    if let Some((first_path, first_scheme)) = files
                        .iter()
                        .find_map(|p| load_open_theme_file(p).ok().map(|s| (p, s)))
                    {
                        persist_imported_theme_file(first_path);
                        wm.apply_open_color_scheme(&first_scheme);
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "Imported {} theme(s) from {}; applied '{}' (select any theme in browser to switch)",
                            files.len(),
                            effective_dir.display(),
                            first_scheme.name
                        )));
                    } else {
                        wm.active_pane_mut().set_status_message(Some(format!(
                            "No themes found in {} (opened File Browser)",
                            effective_dir.display()
                        )));
                    }
                }
                ThemeImportTarget::SingleFile(path) => {
                    if let Ok(scheme) = load_open_theme_file(&path) {
                        persist_imported_theme_file(&path);
                        if let Some(parent) = path.parent() {
                            register_theme_source_dir(parent);
                        }
                        wm.apply_open_color_scheme(&scheme);
                    } else if let Some((resolved_path, scheme)) = resolve_theme_file_or_name(path_str) {
                        persist_imported_theme_file(&resolved_path);
                        wm.apply_open_color_scheme(&scheme);
                    } else {
                        let err = load_open_theme_file(&path)
                            .err()
                            .unwrap_or_else(|| format!("Theme not found: {path_str}"));
                        wm.record_automation_error_msg(err.clone());
                        wm.active_pane_mut().set_status_message(Some(format!("Theme import error: {err}")));
                    }
                }
            }
        }
        Some("export" | "save") => {
            let scheme = OpenColorScheme::from_preset(wm.config().theme);
            let toml_out = export_open_theme_toml(&scheme);
            if let Some(dest) = parts.get(1).copied() {
                if let Err(e) = crate::secure_fs::validate_plain_file_name(dest) {
                    wm.record_automation_error_msg(e.clone());
                    wm.active_pane_mut().set_status_message(Some(e));
                    return;
                }
                let target = wm.export_confine_base_dir().join(dest);
                let msg = match crate::secure_fs::write_private_file(&target, toml_out.as_bytes()) {
                    Ok(()) => format!("Exported open theme TOML to {}", target.display()),
                    Err(e) => format!("Theme export error: {e}"),
                };
                wm.active_pane_mut().set_status_message(Some(msg));
            } else {
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Exported '{}' ({} bytes open TOML)",
                    scheme.name,
                    toml_out.len()
                )));
            }
        }
        Some(name) => {
            if let Some(preset) = ThemePreset::from_name(name).or_else(|| {
                ThemePreset::all().iter().copied().find(|p| p.label().eq_ignore_ascii_case(name))
            }) {
                wm.apply_theme_preset(preset);
            } else if let Some((_, scheme)) = resolve_theme_file_or_name(subcmd.trim()) {
                wm.apply_open_color_scheme(&scheme);
            } else if expand_user_path(subcmd.trim()).is_dir() || subcmd.contains('*') || subcmd.contains('/') {
                execute_theme_or_browser_command(wm, &format!("import {}", subcmd.trim()));
            } else {
                wm.active_pane_mut().set_status_message(Some(format!(
                    "Unknown theme '{name}'. Try :theme list, :browse, or :theme import <path>"
                )));
            }
        }
    }
}
