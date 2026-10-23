use std::path::PathBuf;
use std::time::Instant;

use super::VtParser;
use super::codec::escape_json_string;

impl VtParser {
    /// Starts recording terminal output to an asciinema v2 `.cast` file (`v0.7.0`).
    pub fn start_recording(&mut self, path: PathBuf) {
        self.recording_started_at = Some(Instant::now());
        self.recording_path = Some(path);
        self.recording_private = false;
        self.recording_events.clear();
    }

    /// Starts recording terminal output to an asciinema v2 `.cast` file with private permissions (`v1.7.0`, `SEC-R03`).
    pub fn start_recording_private(&mut self, path: PathBuf) {
        self.recording_started_at = Some(Instant::now());
        self.recording_path = Some(path);
        self.recording_private = true;
        self.recording_events.clear();
    }

    /// Returns `true` when asciinema v2 `.cast` recording is active (`v0.7.0`).
    pub fn is_recording(&self) -> bool {
        self.recording_started_at.is_some()
    }

    /// Stops recording and writes the asciinema v2 `.cast` file to disk (`v0.7.0`).
    pub fn stop_recording(&mut self) -> std::io::Result<Option<PathBuf>> {
        self.recording_started_at = None;
        let is_private = std::mem::replace(&mut self.recording_private, false);
        let Some(path) = self.recording_path.take() else {
            return Ok(None);
        };
        if !is_private
            && let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = format!(
            "{{\"version\":2,\"width\":{},\"height\":{},\"timestamp\":1792550000}}\n",
            self.cols, self.rows
        );
        for (elapsed, chunk) in self.recording_events.drain(..) {
            let escaped = escape_json_string(&chunk);
            out.push_str(&format!("[{elapsed:.6},\"o\",\"{escaped}\"]\n"));
        }
        if is_private {
            use std::io::Write;
            let mut file = crate::secure_fs::create_private_file(&path)?;
            file.write_all(out.as_bytes())?;
            file.sync_all()?;
        } else {
            std::fs::write(&path, out)?;
        }
        Ok(Some(path))
    }
}
