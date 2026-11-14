//! Desktop Launcher, Non-TTY Terminal Self-Relaunch & Working Directory Normalization (`LAUNCH-01`).
//!
//! Enables `bilterm` to be launched directly from OS application launchers without first opening a
//! terminal window:
//! - **Windows Run (`Win+R` -> `bilterm` or `bilterm --gui`) & Start Menu `.lnk` shortcuts**
//! - **Linux Ubuntu GNOME Application Launcher / Rofi / dmenu (`bilterm.desktop`)**
//! - **macOS Spotlight / Launchpad / Finder (`Bilterm.app`)**

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Environment variable guarding against infinite relaunch recursion when no terminal emulator succeeds.
pub const RELAUNCH_GUARD_ENV: &str = "BILTERM_RELAUNCHED";

/// Returns `true` when both `stdin` and `stdout` are attached to an interactive controlling TTY/console.
pub fn has_controlling_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Returns `true` if `dir` looks like an OS system/root directory commonly inherited from desktop
/// launchers (`Win+R` -> `C:\Windows\System32`, GNOME/macOS launcher -> `/`).
pub fn is_system_launch_dir(dir: &Path) -> bool {
    let s = dir.to_string_lossy();
    if s == "/" || s == "\\" {
        return true;
    }
    let lower = s.replace('/', "\\").to_ascii_lowercase();
    let trimmed = lower.trim_end_matches('\\');
    trimmed.ends_with("\\windows\\system32")
        || trimmed.ends_with("\\windows\\syswow64")
        || trimmed.ends_with("\\windows")
}

/// Normalizes the process working directory to the user's home directory (`$HOME` / `%USERPROFILE%`)
/// when `bilterm` is started from an OS Run dialog or desktop launcher whose initial working
/// directory is `/` or `C:\Windows\System32`.
pub fn normalize_desktop_working_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok();
    let needs_home = match &cwd {
        Some(dir) => is_system_launch_dir(dir),
        None => true,
    };
    if !needs_home {
        return cwd;
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    if home.is_dir() && std::env::set_current_dir(&home).is_ok() {
        return Some(home);
    }
    cwd
}

/// Resolves a writable directory for `bilterm.log` so launching from `Win+R` (`C:\Windows\System32`),
/// GNOME Launcher (`/`), or macOS Spotlight never fails or pollutes system directories.
pub fn resolve_log_directory() -> PathBuf {
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        let dir = PathBuf::from(state_home).join("bilterm");
        if crate::secure_fs::create_private_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let dir = PathBuf::from(local_app_data).join("bilterm").join("logs");
        if crate::secure_fs::create_private_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("bilterm");
        if crate::secure_fs::create_private_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    if let Ok(runtime) = crate::secure_fs::private_runtime_dir() {
        return runtime;
    }
    std::env::temp_dir()
}

/// Builds the prioritized list of `(program, args)` candidate terminal emulator commands used to
/// host `exe_path` when `bilterm` is launched without a controlling TTY (`LAUNCH-01`).
pub fn candidate_terminal_commands(
    exe_path: &Path,
    passthrough_args: &[String],
) -> Vec<(String, Vec<String>)> {
    let exe = exe_path.to_string_lossy().to_string();
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();

    for env_key in ["BILTERM_TERMINAL", "TERMINAL"] {
        if let Ok(custom) = std::env::var(env_key) {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                let mut cmd_args = vec!["-e".to_string(), exe.clone()];
                cmd_args.extend(passthrough_args.iter().cloned());
                candidates.push((trimmed.to_string(), cmd_args));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let mut wt_args = vec![
            "new-tab".to_string(),
            "--title".to_string(),
            "Bilterm".to_string(),
            "--".to_string(),
            exe.clone(),
        ];
        wt_args.extend(passthrough_args.iter().cloned());
        candidates.push(("wt.exe".to_string(), wt_args));

        let mut conhost_args = vec![exe.clone()];
        conhost_args.extend(passthrough_args.iter().cloned());
        candidates.push(("conhost.exe".to_string(), conhost_args));
    }

    #[cfg(target_os = "macos")]
    {
        for (app_bin, flag) in [
            ("/Applications/Ghostty.app/Contents/MacOS/ghostty", "-e"),
            ("/Applications/kitty.app/Contents/MacOS/kitty", "-e"),
            ("/Applications/Alacritty.app/Contents/MacOS/alacritty", "-e"),
        ] {
            let mut a = vec![flag.to_string(), exe.clone()];
            a.extend(passthrough_args.iter().cloned());
            candidates.push((app_bin.to_string(), a));
        }
        let mut wez_args = vec!["start".to_string(), "--".to_string(), exe.clone()];
        wez_args.extend(passthrough_args.iter().cloned());
        candidates.push((
            "/Applications/WezTerm.app/Contents/MacOS/wezterm".to_string(),
            wez_args,
        ));

        let quoted_cmd = shell_quote_command(&exe, passthrough_args);
        let script = format!(
            "tell application \"Terminal\"\nactivate\ndo script \"exec {}\"\nend tell",
            quoted_cmd.replace('\\', "\\\\").replace('"', "\\\"")
        );
        candidates.push(("osascript".to_string(), vec!["-e".to_string(), script]));
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut wez_args = vec!["start".to_string(), "--".to_string(), exe.clone()];
        wez_args.extend(passthrough_args.iter().cloned());

        for (prog, prefix) in [
            ("x-terminal-emulator", vec!["-e"]),
            ("ghostty", vec!["-e"]),
            ("kitty", vec!["--"]),
            ("alacritty", vec!["-e"]),
            ("ptyxis", vec!["--"]),
            ("gnome-terminal", vec!["--"]),
            ("konsole", vec!["-e"]),
            ("xfce4-terminal", vec!["-x"]),
            ("tilix", vec!["-e"]),
            ("mate-terminal", vec!["-x"]),
            ("foot", vec!["--"]),
            ("urxvt", vec!["-e"]),
            ("xterm", vec!["-e"]),
        ] {
            let mut a: Vec<String> = prefix.into_iter().map(String::from).collect();
            a.push(exe.clone());
            a.extend(passthrough_args.iter().cloned());
            candidates.push((prog.to_string(), a));
        }
        candidates.insert(2, ("wezterm".to_string(), wez_args));
    }

    candidates
}

#[cfg(target_os = "macos")]
fn shell_quote_command(exe: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(1 + args.len());
    for token in std::iter::once(exe).chain(args.iter().map(String::as_str)) {
        parts.push(format!("'{}'", token.replace('\'', "'\\''")));
    }
    parts.join(" ")
}

/// Facts that decide whether a bare `bilterm` should open the native GUI window instead of the TUI.
#[derive(Clone, Copy, Debug, Default)]
pub struct LaunchContext {
    /// stdin and stdout are both attached to a terminal.
    pub has_tty: bool,
    /// A graphical session is reachable (Linux: `DISPLAY` set; macOS/Windows: always).
    pub display: bool,
    /// Windows only: this process is the sole owner of its console (started from Run/Explorer, not a shell).
    pub private_console: bool,
    /// `--tui` or `BILTERM_GUI=0`.
    pub force_tui: bool,
    /// Already relaunched inside a terminal emulator by [`ensure_terminal_or_relaunch`].
    pub relaunched: bool,
}

/// `true` when a bare `bilterm` was started by the desktop (GNOME/KDE launcher, Windows Run, Finder)
/// rather than by a shell, so the native GUI window is the right default. Pure; see [`LaunchContext::detect`].
pub fn should_default_to_gui(ctx: LaunchContext) -> bool {
    if ctx.force_tui || ctx.relaunched {
        return false;
    }
    (!ctx.has_tty && ctx.display) || ctx.private_console
}

impl LaunchContext {
    /// Reads the live process environment. `args` are the CLI arguments after the program name.
    pub fn detect(args: &[String]) -> Self {
        let force_tui = args.iter().any(|a| a == "--tui")
            || std::env::var("BILTERM_GUI").as_deref() == Ok("0");
        Self {
            has_tty: has_controlling_tty(),
            display: display_available(),
            private_console: owns_private_console(),
            force_tui,
            relaunched: std::env::var(RELAUNCH_GUARD_ENV).as_deref() == Ok("1"),
        }
    }
}

fn display_available() -> bool {
    #[cfg(any(windows, target_os = "macos"))]
    {
        true
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())
    }
}

/// Windows: a console exe started from `Win+R`/Explorer gets a fresh console that only it uses; one started
/// from `cmd`/PowerShell/Windows Terminal shares the console with the shell (process list >= 2).
fn owns_private_console() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        }
        let mut pids = [0u32; 4];
        // SAFETY: `pids` is a valid writable buffer of `count` u32s.
        let n = unsafe { GetConsoleProcessList(pids.as_mut_ptr(), pids.len() as u32) };
        n == 1
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Checks whether the current process has a controlling TTY; if not, attempts to relaunch `bilterm`
/// inside an available host terminal emulator.
///
/// Returns `Ok(true)` if a child terminal emulator was spawned (caller should exit `0`), or
/// `Ok(false)` if a TTY is already attached and normal TUI startup should proceed.
pub fn ensure_terminal_or_relaunch(passthrough_args: &[String]) -> io::Result<bool> {
    if has_controlling_tty() {
        return Ok(false);
    }
    if std::env::var(RELAUNCH_GUARD_ENV).as_deref() == Ok("1") {
        return Ok(false);
    }
    let exe = std::env::current_exe()?;
    for (prog, cmd_args) in candidate_terminal_commands(&exe, passthrough_args) {
        let spawn_res = Command::new(&prog)
            .args(&cmd_args)
            .env(RELAUNCH_GUARD_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawn_res.is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Generates a freedesktop.org `.desktop` launcher entry for Linux application launchers
/// (Ubuntu GNOME Activities, KDE KRunner, Rofi, Wofi, dmenu) with both default TUI and
/// Native GUI (`--gui`) actions (`LAUNCH-01`).
pub fn desktop_entry_contents(exec_path: &str, icon_path: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Version=1.0\n\
         Type=Application\n\
         Name=Bilterm\n\
         GenericName=Composite Terminal Multiplexer\n\
         Comment=IBM 3270 Mainframe & Borland Turbo Vision Composite Terminal Multiplexer\n\
         Exec={exec_path} --gui\n\
         TryExec={exec_path}\n\
         Icon={icon_path}\n\
         Terminal=false\n\
         Categories=System;TerminalEmulator;Utility;\n\
         Keywords=terminal;tmux;3270;borland;multiplexer;shell;ssh;\n\
         StartupNotify=true\n\
         StartupWMClass=bilterm\n\
         Actions=Terminal;QuickSession;\n\
         \n\
         [Desktop Action Terminal]\n\
         Name=Open in a Terminal (TUI)\n\
         Exec={exec_path} --tui\n\
         Terminal=true\n\
         \n\
         [Desktop Action QuickSession]\n\
         Name=New Persistent Session\n\
         Exec={exec_path} --tui -s main\n\
         Terminal=true\n"
    )
}

/// Generates the `Contents/Info.plist` for a macOS `Bilterm.app` bundle (`LAUNCH-01`).
pub fn macos_info_plist_contents(version: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
           <key>CFBundleName</key>\n\
           <string>Bilterm</string>\n\
           <key>CFBundleDisplayName</key>\n\
           <string>Bilterm</string>\n\
           <key>CFBundleIdentifier</key>\n\
           <string>dev.bilterm.app</string>\n\
           <key>CFBundleVersion</key>\n\
           <string>{version}</string>\n\
           <key>CFBundleShortVersionString</key>\n\
           <string>{version}</string>\n\
           <key>CFBundlePackageType</key>\n\
           <string>APPL</string>\n\
           <key>CFBundleExecutable</key>\n\
           <string>bilterm-launcher</string>\n\
           <key>LSMinimumSystemVersion</key>\n\
           <string>11.0</string>\n\
           <key>NSHighResolutionCapable</key>\n\
           <true/>\n\
         </dict>\n\
         </plist>\n"
    )
}

/// Generates the `Contents/MacOS/bilterm-launcher` script for a macOS `Bilterm.app` bundle,
/// defaulting to standalone native GUI mode (`bilterm --gui`).
pub fn macos_launcher_script_contents(exec_path: &str) -> String {
    format!(
        "#!/usr/bin/env bash\n\
         export PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"\n\
         exec \"{exec_path}\" --gui \"$@\"\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_system_launch_directories() {
        assert!(is_system_launch_dir(Path::new("/")));
        assert!(is_system_launch_dir(Path::new(r"C:\Windows\System32")));
        assert!(is_system_launch_dir(Path::new(r"c:\windows\syswow64")));
        assert!(is_system_launch_dir(Path::new(r"C:\Windows")));
        assert!(!is_system_launch_dir(Path::new(
            r"C:\Users\bills\src\terminals\bilterm"
        )));
        assert!(!is_system_launch_dir(Path::new("/home/bills/src/bilterm")));
    }

    #[test]
    fn builds_candidate_terminal_commands_with_env_override() {
        let exe = Path::new("/usr/local/bin/bilterm");
        let args = vec!["-s".to_string(), "ops".to_string()];
        let list = candidate_terminal_commands(exe, &args);
        assert!(!list.is_empty());
        assert!(list.iter().any(|(_, a)| a.contains(&"-s".to_string())));
    }

    #[test]
    fn desktop_launch_defaults_to_gui_but_shell_and_opt_out_do_not() {
        let base = LaunchContext { display: true, ..Default::default() };
        assert!(should_default_to_gui(base), "no tty + display = desktop launcher");
        assert!(!should_default_to_gui(LaunchContext { has_tty: true, ..base }), "shell keeps the TUI");
        assert!(!should_default_to_gui(LaunchContext { display: false, ..base }), "no display: terminal relaunch");
        assert!(!should_default_to_gui(LaunchContext { force_tui: true, ..base }), "--tui / BILTERM_GUI=0");
        assert!(!should_default_to_gui(LaunchContext { relaunched: true, ..base }));
        assert!(should_default_to_gui(LaunchContext { has_tty: true, private_console: true, display: true, ..Default::default() }));
        assert!(!should_default_to_gui(LaunchContext { private_console: true, force_tui: true, ..base }));
    }

    #[test]
    fn generates_valid_xdg_desktop_entry_and_macos_plist() {
        let desktop = desktop_entry_contents("/home/user/.local/bin/bilterm", "bilterm");
        assert!(desktop.contains("[Desktop Entry]"));
        assert!(desktop.contains("Exec=/home/user/.local/bin/bilterm --gui\n"));
        assert!(desktop.contains("Terminal=false"));
        assert!(desktop.contains("Exec=/home/user/.local/bin/bilterm --tui\n"));

        let plist = macos_info_plist_contents("1.10.2");
        assert!(plist.contains("<string>dev.bilterm.app</string>"));
        assert!(plist.contains("<string>bilterm-launcher</string>"));

        let mac_launcher = macos_launcher_script_contents("/Users/bill/.local/bin/bilterm");
        assert!(mac_launcher.contains("exec \"/Users/bill/.local/bin/bilterm\" --gui \"$@\""));
    }
}
