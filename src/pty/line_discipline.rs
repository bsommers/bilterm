use crate::vt::Osc133Phase;

/// Routing target for keyboard events inside a `CompositePane`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputRoutingMode {
    /// Keyboard events edit the pane's `FloatingCommandPrompt` buffer and submit lines on Enter.
    CommandPrompt,
    /// Keyboard events are encoded as raw VT100/xterm bytes and written directly to the PTY master
    /// without echoing into the floating command prompt (used during `DECSET 47/1047/1049` alt-screen,
    /// `!ECHO` password prompts like `sudo`/`ssh`, foreground sub-REPLs, or manual raw override).
    RawPtyPassthrough,
}

/// Three-signal line-discipline telemetry for a `CompositePane` (`PTY-03`):
/// 1. `OSC 133` semantic shell prompt phase (`CommandExecuted` vs `PromptStart`/`CommandStart`)
/// 2. Foreground process group ID (`tcgetpgrp`) vs top-level shell process group ID
/// 3. Termios `ECHO` and `ICANON` flags (`!ECHO` indicates password prompt; `!ICANON` indicates raw/cbreak REPL)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineDisciplineState {
    pub echo_enabled: bool,
    pub canonical_mode: bool,
    pub foreground_pgid: Option<i32>,
    pub shell_pgid: Option<i32>,
    pub osc133_executing: bool,
}

impl Default for LineDisciplineState {
    fn default() -> Self {
        Self {
            echo_enabled: true,
            canonical_mode: true,
            foreground_pgid: None,
            shell_pgid: None,
            osc133_executing: false,
        }
    }
}

impl LineDisciplineState {
    /// Returns `true` when canonical password prompt protection (`ICANON && !ECHO`) is active.
    pub fn is_password_prompt(&self) -> bool {
        !self.echo_enabled && self.canonical_mode
    }

    /// Returns `true` when a foreground child process group different from the shell owns the TTY.
    pub fn has_foreground_subcommand(&self) -> bool {
        match (self.foreground_pgid, self.shell_pgid) {
            (Some(fg), Some(sh)) if fg > 0 && sh > 0 => fg != sh,
            _ => false,
        }
    }

    /// Evaluates the 3-signal heuristic (`PTY-03`):
    /// - Signal 1: `!echo_enabled` (`sudo`/`ssh`/`read -s` password prompt) -> always `RawPtyPassthrough`
    /// - Signal 2: `foreground_pgid != shell_pgid && !canonical_mode` (interactive raw/cbreak sub-REPL like `python`, `node`, `less`) -> `RawPtyPassthrough`
    /// - Signal 3: `osc133_executing && !canonical_mode` (raw interactive command under OSC 133) -> `RawPtyPassthrough`
    ///
    /// Crucially, when a foreground subcommand runs in normal cooked mode (`echo_enabled && canonical_mode`,
    /// such as `tail -f /dev/random`, `ping`, `watch`, `yes`, `cargo build`), `requires_raw_passthrough()`
    /// returns `false` so the `FloatingCommandPrompt` remains visible and floats smoothly over the
    /// scrolling `OutputCanvas`.
    pub fn requires_raw_passthrough(&self) -> bool {
        if !self.echo_enabled {
            return true;
        }
        if !self.canonical_mode && (self.has_foreground_subcommand() || self.osc133_executing) {
            return true;
        }
        false
    }
}

/// Inspects a live PTY file descriptor and combines OS termios/pgrp state with `OSC 133` phase.
#[cfg(unix)]
pub fn inspect_pty_fd(
    fd: std::os::fd::RawFd,
    shell_pid: Option<u32>,
    osc133_phase: Option<Osc133Phase>,
) -> LineDisciplineState {
    let osc133_executing = matches!(osc133_phase, Some(Osc133Phase::CommandExecuted));
    let shell_pgid = shell_pid.map(|pid| pid as i32);

    if fd < 0 {
        return LineDisciplineState {
            echo_enabled: true,
            canonical_mode: true,
            foreground_pgid: None,
            shell_pgid,
            osc133_executing,
        };
    }

    let mut termios_buf: libc::termios = unsafe { std::mem::zeroed() };
    let tc_ok = unsafe { libc::tcgetattr(fd, &mut termios_buf) } == 0;

    let (echo_enabled, canonical_mode) = if tc_ok {
        let echo = (termios_buf.c_lflag & libc::ECHO) != 0;
        let icanon = (termios_buf.c_lflag & libc::ICANON) != 0;
        (echo, icanon)
    } else {
        (true, true)
    };

    let fg_pgrp = unsafe { libc::tcgetpgrp(fd) };
    let foreground_pgid = if fg_pgrp > 0 { Some(fg_pgrp) } else { None };

    // If the foreground process group is the shell itself (e.g. bash readline or zsh ZLE) and
    // OSC 133 is not in CommandExecuted, treat shell line-editing raw/no-echo as normal prompt state
    // unless a foreground child process or explicit password prompt is active.
    let shell_owns_tty = match (foreground_pgid, shell_pgid) {
        (Some(fg), Some(sh)) => fg == sh,
        _ => false,
    };

    let effective_echo = if !canonical_mode {
        // Interactive shell line editors (Bash Readline, Zsh ZLE, Fish) and raw/cbreak modes
        // disable ICANON and ECHO simultaneously; only canonical mode with !ECHO (ICANON && !ECHO)
        // is a genuine password prompt (sudo, ssh, passwd, read -s).
        true
    } else if shell_owns_tty && !osc133_executing {
        // While the top-level shell itself owns the foreground process group and no command is
        // executing (e.g. during .bashrc / .zshrc initialization), ignore transient stty !ECHO.
        true
    } else {
        echo_enabled
    };

    LineDisciplineState {
        echo_enabled: effective_echo,
        canonical_mode,
        foreground_pgid,
        shell_pgid,
        osc133_executing,
    }
}

#[cfg(not(unix))]
pub fn inspect_pty_fd(
    _fd: i32,
    shell_pid: Option<u32>,
    osc133_phase: Option<Osc133Phase>,
) -> LineDisciplineState {
    LineDisciplineState {
        echo_enabled: true,
        canonical_mode: true,
        foreground_pgid: None,
        shell_pgid: shell_pid.map(|p| p as i32),
        osc133_executing: matches!(osc133_phase, Some(Osc133Phase::CommandExecuted)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_signal_detection_switches_to_raw_passthrough_on_no_echo_or_subcommand() {
        let normal = LineDisciplineState::default();
        assert!(!normal.requires_raw_passthrough());
        assert!(!normal.is_password_prompt());

        // Cooked-mode streaming command (e.g. `tail -f /dev/random`, `ping`) keeps floating prompt visible!
        let cooked_streaming = LineDisciplineState {
            echo_enabled: true,
            canonical_mode: true,
            foreground_pgid: Some(4200),
            shell_pgid: Some(4100),
            osc133_executing: true,
        };
        assert!(cooked_streaming.has_foreground_subcommand());
        assert!(!cooked_streaming.requires_raw_passthrough());

        // Signal 1: OSC 133;C command executing in non-canonical/cbreak mode
        let osc_running = LineDisciplineState {
            osc133_executing: true,
            canonical_mode: false,
            ..Default::default()
        };
        assert!(osc_running.requires_raw_passthrough());

        // Signal 2: Foreground process group differs from shell PGID in non-canonical mode (e.g. python REPL)
        let sub_repl = LineDisciplineState {
            foreground_pgid: Some(4200),
            shell_pgid: Some(4100),
            canonical_mode: false,
            ..Default::default()
        };
        assert!(sub_repl.has_foreground_subcommand());
        assert!(sub_repl.requires_raw_passthrough());

        // Signal 3: Termios !ECHO (e.g. sudo password prompt)
        let sudo_prompt = LineDisciplineState {
            echo_enabled: false,
            canonical_mode: true,
            ..Default::default()
        };
        assert!(sudo_prompt.is_password_prompt());
        assert!(sudo_prompt.requires_raw_passthrough());
    }

    #[cfg(unix)]
    #[test]
    fn inspect_pty_fd_detects_disabled_echo_on_unix_pty() {
        use portable_pty::{native_pty_system, PtySize};

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let fd = pair.master.as_raw_fd().expect("master raw fd");
        let initial = inspect_pty_fd(fd, None, None);
        assert!(initial.echo_enabled);

        // Disable ECHO on the PTY descriptor as `sudo` or `ssh` does
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(fd, &mut t), 0);
            t.c_lflag &= !libc::ECHO;
            assert_eq!(libc::tcsetattr(fd, libc::TCSANOW, &t), 0);
        }

        let after_no_echo = inspect_pty_fd(fd, None, None);
        assert!(!after_no_echo.echo_enabled);
        assert!(after_no_echo.is_password_prompt());
        assert!(after_no_echo.requires_raw_passthrough());
    }
}
