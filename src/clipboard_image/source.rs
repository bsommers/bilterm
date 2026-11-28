//! Reading an image off the host clipboard by shelling out to platform tools.
//!
//! The process spawn is behind [`CommandRunner`] so tests drive the whole selection and
//! error-mapping logic with a fake and never touch a real clipboard. The real runner never
//! buffers more than `max_bytes + 1` bytes and kills the child on overflow or timeout.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{ImageError, ImageLimits};

/// Host platform family (selected at runtime so the logic is testable on any host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// One tool invocation that may yield PNG bytes on stdout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub prog: &'static str,
    pub args: Vec<String>,
    /// Output is AppleScript's `«data PNGf<hex>»` text rather than raw bytes.
    pub applescript_hex: bool,
}

const WINDOWS_SCRIPT: &str = "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
$i=[Windows.Forms.Clipboard]::GetImage(); if($null -eq $i){exit 3}; \
$m=New-Object IO.MemoryStream; $i.Save($m,[Drawing.Imaging.ImageFormat]::Png); \
$b=$m.ToArray(); $o=[Console]::OpenStandardOutput(); $o.Write($b,0,$b.Length); $o.Flush()";

fn cand(prog: &'static str, args: &[&str], applescript_hex: bool) -> Candidate {
    Candidate { prog, args: args.iter().map(|s| (*s).to_string()).collect(), applescript_hex }
}

/// Ordered tools to try for `platform`; pure so it is unit-tested without spawning anything.
pub fn candidates(platform: Platform, wayland: bool, x11: bool) -> Vec<Candidate> {
    match platform {
        Platform::Linux => {
            let mut v = Vec::new();
            if wayland {
                v.push(cand("wl-paste", &["--no-newline", "--type", "image/png"], false));
            }
            if x11 {
                v.push(cand("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"], false));
            }
            v
        }
        Platform::MacOs => vec![
            cand("pngpaste", &["-"], false),
            cand("osascript", &["-e", "the clipboard as \u{ab}class PNGf\u{bb}"], true),
        ],
        Platform::Windows => vec![cand(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-STA", "-Command", WINDOWS_SCRIPT],
            false,
        )],
    }
}

/// One bounded run request.
pub struct RunSpec<'a> {
    pub prog: &'a str,
    pub args: &'a [String],
    /// Reading stops (and the child dies) once more than this many stdout bytes arrive.
    pub max_bytes: usize,
    pub timeout: Duration,
}

/// What a bounded run produced.
#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Output(Vec<u8>),
    Exceeded,
    TimedOut,
    NotFound,
    Failed,
}

/// Spawns a program and collects its stdout under the caps in [`RunSpec`].
pub trait CommandRunner: Send + Sync {
    fn run(&self, spec: &RunSpec<'_>) -> RunOutcome;
}

/// Real runner: `std::process` with a reader thread so a full pipe can never deadlock the child.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, spec: &RunSpec<'_>) -> RunOutcome {
        let mut child = match Command::new(spec.prog)
            .args(spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return RunOutcome::NotFound,
            Err(_) => return RunOutcome::Failed,
        };
        let Some(out) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return RunOutcome::Failed;
        };
        let cap = spec.max_bytes as u64 + 1;
        let (tx, rx) = mpsc::channel();
        // Detached on purpose: a grandchild holding the pipe open must never block the UI thread.
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = out.take(cap).read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
        let deadline = Instant::now() + spec.timeout;
        let outcome = match rx.recv_timeout(spec.timeout) {
            Ok(buf) if buf.len() as u64 >= cap => {
                let _ = child.kill();
                RunOutcome::Exceeded
            }
            Ok(buf) => wait_status(&mut child, deadline, buf),
            Err(_) => {
                let _ = child.kill();
                RunOutcome::TimedOut
            }
        };
        let _ = child.wait();
        outcome
    }
}

fn wait_status(child: &mut std::process::Child, deadline: Instant, buf: Vec<u8>) -> RunOutcome {
    loop {
        match child.try_wait() {
            Ok(Some(s)) if s.success() && !buf.is_empty() => return RunOutcome::Output(buf),
            Ok(Some(_)) | Err(_) => return RunOutcome::Failed,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                return RunOutcome::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Decodes AppleScript's `«data PNGf89504E47...»` rendering into bytes.
pub fn decode_applescript_hex(text: &[u8]) -> Option<Vec<u8>> {
    let s = std::str::from_utf8(text).ok()?;
    let rest = s.trim().strip_prefix("\u{ab}data ")?.strip_suffix('\u{bb}')?;
    let hex = rest.get(4..)?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

/// Where raw (unvalidated) clipboard image bytes come from. Mocked in tests.
pub trait ImageSource: Send + Sync {
    fn read(&self, limits: &ImageLimits) -> Result<Vec<u8>, ImageError>;
}

/// Platform shell-out source.
pub struct ShellImageSource {
    pub runner: Box<dyn CommandRunner>,
    pub platform: Platform,
    pub wayland: bool,
    pub x11: bool,
}

impl ShellImageSource {
    pub fn system() -> Self {
        Self {
            runner: Box::new(SystemRunner),
            platform: Platform::current(),
            wayland: std::env::var_os("WAYLAND_DISPLAY").is_some(),
            x11: std::env::var_os("DISPLAY").is_some(),
        }
    }
}

impl ImageSource for ShellImageSource {
    fn read(&self, limits: &ImageLimits) -> Result<Vec<u8>, ImageError> {
        let list = candidates(self.platform, self.wayland, self.x11);
        if list.is_empty() {
            return Err(ImageError::ToolMissing("no display server (WAYLAND_DISPLAY/DISPLAY unset)".into()));
        }
        let (mut found_tool, mut timed_out) = (false, false);
        for c in &list {
            let max_bytes = if c.applescript_hex { limits.max_image_bytes * 2 + 64 } else { limits.max_image_bytes };
            let spec = RunSpec { prog: c.prog, args: &c.args, max_bytes, timeout: limits.read_timeout };
            match self.runner.run(&spec) {
                RunOutcome::Output(raw) => {
                    let bytes = if c.applescript_hex { decode_applescript_hex(&raw) } else { Some(raw) };
                    match bytes {
                        Some(b) if b.len() > limits.max_image_bytes => return Err(ImageError::TooLarge),
                        Some(b) => return Ok(b),
                        None => found_tool = true,
                    }
                }
                RunOutcome::Exceeded => return Err(ImageError::TooLarge),
                RunOutcome::TimedOut => {
                    found_tool = true;
                    timed_out = true;
                }
                RunOutcome::Failed => found_tool = true,
                RunOutcome::NotFound => {}
            }
        }
        if timed_out {
            Err(ImageError::Timeout)
        } else if found_tool {
            Err(ImageError::NoImage)
        } else {
            let names: Vec<&str> = list.iter().map(|c| c.prog).collect();
            Err(ImageError::ToolMissing(format!("install one of: {}", names.join(", "))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Fake {
        script: Mutex<Vec<RunOutcome>>,
        seen: Mutex<Vec<(String, usize)>>,
    }

    impl Fake {
        fn new(mut script: Vec<RunOutcome>) -> Self {
            script.reverse();
            Self { script: Mutex::new(script), seen: Mutex::new(Vec::new()) }
        }
    }

    impl CommandRunner for std::sync::Arc<Fake> {
        fn run(&self, spec: &RunSpec<'_>) -> RunOutcome {
            self.seen.lock().unwrap().push((spec.prog.to_string(), spec.max_bytes));
            self.script.lock().unwrap().pop().unwrap_or(RunOutcome::NotFound)
        }
    }

    fn src(platform: Platform, wl: bool, x: bool, f: &std::sync::Arc<Fake>) -> ShellImageSource {
        ShellImageSource { runner: Box::new(f.clone()), platform, wayland: wl, x11: x }
    }

    fn lim() -> ImageLimits {
        ImageLimits { max_image_bytes: 100, ..ImageLimits::default() }
    }

    #[test]
    fn candidate_tables_per_platform() {
        let l = candidates(Platform::Linux, true, true);
        assert_eq!(l.iter().map(|c| c.prog).collect::<Vec<_>>(), ["wl-paste", "xclip"]);
        assert_eq!(candidates(Platform::Linux, false, false), vec![]);
        assert_eq!(candidates(Platform::Linux, false, true)[0].prog, "xclip");
        let m = candidates(Platform::MacOs, false, false);
        assert_eq!((m[0].prog, m[1].prog, m[1].applescript_hex), ("pngpaste", "osascript", true));
        let w = candidates(Platform::Windows, false, false);
        assert_eq!(w[0].prog, "powershell.exe");
        assert!(w[0].args.iter().any(|a| a == "-STA"));
    }

    #[test]
    fn first_tool_that_returns_bytes_wins_and_later_tools_are_skipped() {
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::Output(vec![1, 2, 3])]));
        assert_eq!(src(Platform::Linux, true, true, &f).read(&lim()).unwrap(), vec![1, 2, 3]);
        assert_eq!(f.seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn falls_through_missing_and_failed_tools() {
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::NotFound, RunOutcome::Output(vec![9])]));
        assert_eq!(src(Platform::Linux, true, true, &f).read(&lim()).unwrap(), vec![9]);
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::Failed, RunOutcome::Failed]));
        assert_eq!(src(Platform::Linux, true, true, &f).read(&lim()), Err(ImageError::NoImage));
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::NotFound, RunOutcome::NotFound]));
        assert!(matches!(src(Platform::Linux, true, true, &f).read(&lim()), Err(ImageError::ToolMissing(_))));
    }

    #[test]
    fn exceeded_and_timeout_map_to_typed_errors() {
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::Exceeded]));
        assert_eq!(src(Platform::Linux, true, true, &f).read(&lim()), Err(ImageError::TooLarge));
        assert_eq!(f.seen.lock().unwrap().len(), 1, "no further tool after an oversize image");
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::TimedOut, RunOutcome::Failed]));
        assert_eq!(src(Platform::Linux, true, true, &f).read(&lim()), Err(ImageError::Timeout));
    }

    #[test]
    fn no_display_server_reports_tool_missing() {
        let f = std::sync::Arc::new(Fake::new(vec![]));
        assert!(matches!(src(Platform::Linux, false, false, &f).read(&lim()), Err(ImageError::ToolMissing(_))));
        assert!(f.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn mac_falls_back_to_applescript_hex_with_doubled_read_cap() {
        let hex = "\u{ab}data PNGf89504E47\u{bb}\n".as_bytes().to_vec();
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::NotFound, RunOutcome::Output(hex)]));
        let out = src(Platform::MacOs, false, false, &f).read(&lim()).unwrap();
        assert_eq!(out, vec![0x89, 0x50, 0x4e, 0x47]);
        let seen = f.seen.lock().unwrap();
        assert_eq!(seen[0], ("pngpaste".to_string(), 100));
        assert_eq!(seen[1], ("osascript".to_string(), 264));
    }

    #[test]
    fn decoded_hex_over_the_cap_is_rejected() {
        let mut s = "\u{ab}data PNGf".to_string();
        s.push_str(&"AB".repeat(101));
        s.push('\u{bb}');
        let f = std::sync::Arc::new(Fake::new(vec![RunOutcome::Output(s.into_bytes())]));
        assert_eq!(src(Platform::MacOs, false, false, &f).read(&lim()), Err(ImageError::TooLarge));
    }

    #[test]
    fn hex_decoder_rejects_malformed_text() {
        assert!(decode_applescript_hex(b"nope").is_none());
        assert!(decode_applescript_hex("\u{ab}data PNGfZZ\u{bb}".as_bytes()).is_none());
        assert!(decode_applescript_hex("\u{ab}data PNGfABC\u{bb}".as_bytes()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn system_runner_caps_output_and_kills_the_child() {
        let args: Vec<String> = ["-c", "head -c 10000000 /dev/zero; sleep 30"].iter().map(|s| s.to_string()).collect();
        let t = Instant::now();
        let r = SystemRunner.run(&RunSpec { prog: "sh", args: &args, max_bytes: 1000, timeout: Duration::from_secs(10) });
        assert_eq!(r, RunOutcome::Exceeded);
        assert!(t.elapsed() < Duration::from_secs(5), "child must be killed, not waited out");
    }

    #[cfg(unix)]
    #[test]
    fn system_runner_times_out_and_reports_missing_and_failed() {
        let sleep: Vec<String> = ["-c", "sleep 30"].iter().map(|s| s.to_string()).collect();
        let r = SystemRunner.run(&RunSpec { prog: "sh", args: &sleep, max_bytes: 1000, timeout: Duration::from_millis(150) });
        assert_eq!(r, RunOutcome::TimedOut);
        let r = SystemRunner.run(&RunSpec { prog: "bilterm-no-such-tool", args: &[], max_bytes: 10, timeout: Duration::from_secs(1) });
        assert_eq!(r, RunOutcome::NotFound);
        let fail: Vec<String> = ["-c", "echo x; exit 1"].iter().map(|s| s.to_string()).collect();
        let r = SystemRunner.run(&RunSpec { prog: "sh", args: &fail, max_bytes: 10, timeout: Duration::from_secs(2) });
        assert_eq!(r, RunOutcome::Failed);
        let ok: Vec<String> = ["-c", "printf abc"].iter().map(|s| s.to_string()).collect();
        let r = SystemRunner.run(&RunSpec { prog: "sh", args: &ok, max_bytes: 10, timeout: Duration::from_secs(2) });
        assert_eq!(r, RunOutcome::Output(b"abc".to_vec()));
    }

    /// Manual platform check: put an image on the host clipboard first, then run
    /// `cargo test -p bilterm --lib real_host_clipboard_image -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs a real clipboard image (set BILTERM_EXPECT_CLIPBOARD_IMAGE=1)"]
    fn real_host_clipboard_image() {
        let src = ShellImageSource::system();
        let r = src.read(&ImageLimits::default());
        if std::env::var_os("BILTERM_EXPECT_CLIPBOARD_IMAGE").is_none() {
            eprintln!("no expectation set, result: {:?}", r.as_ref().map(|b| b.len()));
            return;
        }
        let bytes = r.expect("clipboard image read");
        let info = crate::clipboard_image::detect::sniff(&bytes).expect("sniff");
        eprintln!("host clipboard image: {:?} {}x{} {} bytes", info.kind, info.width, info.height, bytes.len());
    }
}
