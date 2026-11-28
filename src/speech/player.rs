//! Audio playback by shell-out to the platform player, behind a mockable trait.
//!
//! No audio library is linked: Linux uses the first of `paplay`, `pw-play`, `aplay`, `play`,
//! `ffplay` found on `PATH`; macOS uses `afplay`; Windows uses PowerShell's `Media.SoundPlayer`
//! (the WAV path is passed through an environment variable, never interpolated into a script).

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// A way to play a finished WAV file. Implementations must poll `cancelled` and stop promptly.
pub trait Player: Send + Sync {
    /// Short human name of the backend (for status output).
    fn name(&self) -> String;
    /// `Ok` when playback can work on this machine, else an actionable message.
    fn available(&self) -> Result<(), String>;
    /// Plays `wav` to completion (blocking the calling worker thread) or until `cancelled()` is true.
    fn play(&self, wav: &Path, cancelled: &dyn Fn() -> bool) -> Result<(), String>;
}

/// One player command: program plus leading arguments (the WAV path is appended).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerCmd {
    pub program: &'static str,
    pub args: &'static [&'static str],
    /// When true the WAV path is exported as `BILTERM_SAY_WAV` instead of appended.
    pub path_in_env: bool,
}

/// Candidate players for `os` (a `std::env::consts::OS` value), in preference order.
pub fn candidates(os: &str) -> Vec<PlayerCmd> {
    let c = |program, args| PlayerCmd { program, args, path_in_env: false };
    match os {
        "macos" => vec![c("afplay", &[])],
        "windows" => vec![PlayerCmd {
            program: "powershell.exe",
            args: &["-NoProfile", "-NonInteractive", "-Command", "(New-Object System.Media.SoundPlayer $env:BILTERM_SAY_WAV).PlaySync()"],
            path_in_env: true,
        }],
        _ => vec![
            c("paplay", &[]),
            c("pw-play", &[]),
            c("aplay", &["-q"]),
            c("play", &["-q"]),
            c("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
        ],
    }
}

/// True when `exe` is an executable file on `PATH` (also tries `.exe` on Windows).
pub fn on_path(exe: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|d| {
        let p = d.join(exe);
        let hit = |p: &Path| {
            p.is_file() && {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    p.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    true
                }
            }
        };
        hit(&p) || (cfg!(windows) && hit(&d.join(format!("{exe}.exe"))))
    })
}

/// Hint shown when no player exists.
pub const NO_PLAYER_HINT: &str = "no audio player found; install pulseaudio-utils (paplay), pipewire (pw-play), alsa-utils (aplay), sox or ffmpeg";

/// Plays through the first available candidate for the host OS.
pub struct ShellPlayer {
    cands: Vec<PlayerCmd>,
}

impl ShellPlayer {
    pub fn new() -> Self {
        Self { cands: candidates(std::env::consts::OS) }
    }

    fn pick(&self) -> Option<&PlayerCmd> {
        self.cands.iter().find(|c| on_path(c.program))
    }
}

impl Default for ShellPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Player for ShellPlayer {
    fn name(&self) -> String {
        self.pick().map_or_else(|| "none".to_string(), |c| c.program.to_string())
    }

    fn available(&self) -> Result<(), String> {
        self.pick().map(|_| ()).ok_or_else(|| NO_PLAYER_HINT.to_string())
    }

    fn play(&self, wav: &Path, cancelled: &dyn Fn() -> bool) -> Result<(), String> {
        let cmd = self.pick().ok_or_else(|| NO_PLAYER_HINT.to_string())?;
        let mut c = Command::new(cmd.program);
        c.args(cmd.args);
        if cmd.path_in_env {
            c.env("BILTERM_SAY_WAV", wav);
        } else {
            c.arg(wav);
        }
        let mut child = c
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", cmd.program))?;
        loop {
            match child.try_wait() {
                Ok(Some(st)) if st.success() => return Ok(()),
                Ok(Some(st)) => return Err(format!("{} exited with {st}", cmd.program)),
                Ok(None) => {}
                Err(e) => return Err(format!("{} wait failed: {e}", cmd.program)),
            }
            if cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Test double: records what would have been played and can simulate a long playback.
#[derive(Default)]
pub struct RecordingPlayer {
    /// Size in bytes of each WAV handed to `play`.
    pub played: Mutex<Vec<u64>>,
    /// Simulated playback length; polled against `cancelled`.
    pub hold: Duration,
    /// When set, `available` fails with this message.
    pub unavailable: Option<String>,
}

impl Player for RecordingPlayer {
    fn name(&self) -> String {
        "recording".to_string()
    }

    fn available(&self) -> Result<(), String> {
        self.unavailable.clone().map_or(Ok(()), Err)
    }

    fn play(&self, wav: &Path, cancelled: &dyn Fn() -> bool) -> Result<(), String> {
        let len = std::fs::metadata(wav).map_err(|e| e.to_string())?.len();
        let start = std::time::Instant::now();
        while start.elapsed() < self.hold {
            if cancelled() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.played.lock().expect("played lock").push(len);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_lists_per_os() {
        assert_eq!(candidates("macos")[0].program, "afplay");
        let w = &candidates("windows")[0];
        assert!(w.program.starts_with("powershell") && w.path_in_env);
        assert!(w.args.iter().all(|a| !a.contains('\'') || a.contains("$env:")), "path never interpolated");
        let l = candidates("linux");
        assert_eq!(l[0].program, "paplay");
        assert!(l.iter().any(|c| c.program == "aplay" && c.args == ["-q"]));
        assert_eq!(candidates("freebsd")[0].program, "paplay");
    }

    #[test]
    fn on_path_finds_nothing_for_garbage() {
        assert!(!on_path("definitely-not-a-real-player-binary"));
    }

    #[test]
    fn recording_player_honours_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.wav");
        std::fs::write(&f, b"1234").unwrap();
        let p = RecordingPlayer { hold: Duration::from_secs(5), ..Default::default() };
        let t = std::time::Instant::now();
        p.play(&f, &|| true).unwrap();
        assert!(t.elapsed() < Duration::from_secs(1));
        assert!(p.played.lock().unwrap().is_empty());
        let q = RecordingPlayer::default();
        q.play(&f, &|| false).unwrap();
        assert_eq!(*q.played.lock().unwrap(), vec![4]);
    }
}
