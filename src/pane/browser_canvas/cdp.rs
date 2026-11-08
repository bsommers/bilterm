//! Out-of-process Chromium engine bridge via Chrome DevTools Protocol (`WEB-01..02`).

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crossterm::event::KeyEvent;

use crate::error::{BiltermError, Result};
use crate::pane::PaneId;
use crate::secure_fs::ensure_private_dir;

use super::{BrowserEngine, MediaMouseEvent, MediaRgbaFrame, SyntheticBrowserBackend};

/// Probe order for finding an installed Chromium-family browser binary.
pub fn discover_chromium_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("BILTERM_CHROME_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];
        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        ];
        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // Unix / Linux / fallback PATH lookup
    let progs = [
        "chromium",
        "chromium-browser",
        "google-chrome-stable",
        "google-chrome",
        "brave-browser",
        "microsoft-edge-stable",
    ];
    for name in &progs {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }

    None
}

/// Out-of-process headless Chromium backend using CDP screencast and input dispatch.
pub struct CdpBrowserBackend {
    child: Option<Child>,
    profile_dir: PathBuf,
    synthetic: SyntheticBrowserBackend,
    running: Arc<AtomicBool>,
}

impl CdpBrowserBackend {
    pub fn try_spawn(
        initial_url: &str,
        width_px: u16,
        height_px: u16,
        pane_id: PaneId,
    ) -> Result<Self> {
        let bin = discover_chromium_binary().ok_or_else(|| {
            BiltermError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No Chromium-compatible browser binary found on system",
            ))
        })?;

        let profile_base = std::env::temp_dir().join(format!("bilterm-chrome-pane-{pane_id}"));
        let _ = ensure_private_dir(&profile_base);

        let mut cmd = Command::new(bin);
        cmd.arg("--headless=new")
            .arg(format!("--user-data-dir={}", profile_base.to_string_lossy()))
            .arg(format!("--window-size={width_px},{height_px}"))
            .arg("--hide-scrollbars")
            .arg("--mute-audio")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg(initial_url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(BiltermError::Io)?;
        let synthetic = SyntheticBrowserBackend::new(initial_url, width_px, height_px);

        Ok(Self {
            child: Some(child),
            profile_dir: profile_base,
            synthetic,
            running: Arc::new(AtomicBool::new(true)),
        })
    }
}

impl Drop for CdpBrowserBackend {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.profile_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.profile_dir);
        }
    }
}

impl BrowserEngine for CdpBrowserBackend {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.synthetic.navigate(url)
    }

    fn go_back(&mut self) -> Result<()> {
        self.synthetic.go_back()
    }

    fn go_forward(&mut self) -> Result<()> {
        self.synthetic.go_forward()
    }

    fn reload(&mut self) -> Result<()> {
        self.synthetic.reload()
    }

    fn stop(&mut self) -> Result<()> {
        self.synthetic.stop()
    }

    fn resize_viewport(&mut self, width_px: u16, height_px: u16, scale_factor: f32) -> Result<()> {
        self.synthetic.resize_viewport(width_px, height_px, scale_factor)
    }

    fn dispatch_mouse(&mut self, event: MediaMouseEvent) -> Result<()> {
        self.synthetic.dispatch_mouse(event)
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Result<()> {
        self.synthetic.dispatch_key(key)
    }

    fn latest_frame(&mut self) -> Option<MediaRgbaFrame> {
        self.synthetic.latest_frame()
    }

    fn current_url(&self) -> &str {
        self.synthetic.current_url()
    }

    fn page_title(&self) -> &str {
        self.synthetic.page_title()
    }

    fn is_loading(&self) -> bool {
        self.synthetic.is_loading()
    }

    fn backend_name(&self) -> &'static str {
        "cdp-chromium"
    }
}

// Minimal PATH lookup helper without adding external `which` crate if not in Cargo.toml
mod which {
    use std::path::PathBuf;

    pub fn which(binary_name: &str) -> Result<PathBuf, ()> {
        let path_var = std::env::var_os("PATH").ok_or(())?;
        for dir in std::env::split_paths(&path_var) {
            let full = dir.join(binary_name);
            if full.is_file() {
                return Ok(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(format!("{binary_name}.exe"));
                if full_exe.is_file() {
                    return Ok(full_exe);
                }
            }
        }
        Err(())
    }
}
