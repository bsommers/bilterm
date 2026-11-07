//! Multi-mode `BiltermLiveDriver` harness for live interactive TUI, GUI framebuffer,
//! subprocess MCP/CLI/JSON-RPC, and `.bt`/`.toml` batch scripting tests.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::sync::{Mutex, MutexGuard};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tempfile::TempDir;

use bilterm::api::{
    AutomationControlPlane, AutomationEvent, JsonValue, Playbook, PlaybookReport, SnapshotPolicy,
};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::WindowManager;
use bilterm::wm::script_studio::ScriptStudioOp;

/// Live subprocess client speaking JSON-RPC 2.0 over `bilterm --mcp` stdio.
pub struct McpLiveClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpLiveClient {
    /// Sends a raw JSON-RPC line and reads back one response line.
    pub fn send_raw_line(&mut self, line: &str) -> String {
        writeln!(self.stdin, "{line}").expect("write JSON-RPC line to MCP stdin");
        self.stdin.flush().expect("flush MCP stdin");
        let mut resp = String::new();
        self.stdout
            .read_line(&mut resp)
            .expect("read JSON-RPC response from MCP stdout");
        resp.trim().to_string()
    }

    /// Sends a structured JSON-RPC request (`method`, `params` JSON string) and returns the response JSON string.
    pub fn request(&mut self, method: &str, params_json: &str) -> String {
        let id = self.next_id;
        self.next_id += 1;
        let params = if params_json.trim().is_empty() {
            "{}"
        } else {
            params_json
        };
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
        );
        self.send_raw_line(&line)
    }

    /// Calls an MCP tool via `tools/call` and returns the raw JSON-RPC response line.
    pub fn call_tool(&mut self, name: &str, arguments_json: &str) -> String {
        let args = if arguments_json.trim().is_empty() {
            "{}"
        } else {
            arguments_json
        };
        let params = format!(r#"{{"name":"{name}","arguments":{args}}}"#);
        self.request("tools/call", &params)
    }
}

impl Drop for McpLiveClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static HERMETIC_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Multi-mode live & batch test driver for Bilterm.
pub struct BiltermLiveDriver {
    _env_guard: MutexGuard<'static, ()>,
    pub temp_dir: TempDir,
    pub home_dir: PathBuf,
    pub config_home: PathBuf,
    pub state_home: PathBuf,
    pub data_home: PathBuf,
    pub session_dir: PathBuf,
    pub snapshot_dir: PathBuf,
    pub wm: WindowManager,
    pub viewport: Rect,
    pub rpc_events: VecDeque<AutomationEvent>,
}

impl BiltermLiveDriver {
    /// Creates a hermetic TUI driver with isolated XDG/session directories and a `cols x rows` viewport.
    pub fn new_tui(cols: u16, rows: u16) -> Self {
        let env_guard = HERMETIC_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = tempfile::tempdir().expect("create hermetic tempdir");
        let root = temp_dir.path();
        let home_dir = root.join("home");
        let config_home = root.join("config");
        let state_home = root.join("state");
        let data_home = root.join("data");
        let session_dir = root.join("sessions");
        let snapshot_dir = root.join("snapshots");

        for dir in [
            &home_dir,
            &config_home,
            &state_home,
            &data_home,
            &session_dir,
            &snapshot_dir,
        ] {
            std::fs::create_dir_all(dir).expect("create isolated XDG subdir");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }

        // Point process environment at the hermetic directories across Linux, macOS, and Windows.
        // SAFETY: Test binary coordinates env initialization through BiltermLiveDriver.
        unsafe {
            std::env::set_var("HOME", &home_dir);
            std::env::set_var("USERPROFILE", &home_dir);
            std::env::set_var("APPDATA", &config_home);
            std::env::set_var("LOCALAPPDATA", &state_home);
            std::env::set_var("XDG_CONFIG_HOME", &config_home);
            std::env::set_var("XDG_STATE_HOME", &state_home);
            std::env::set_var("XDG_DATA_HOME", &data_home);
            std::env::set_var("BILTERM_SESSION_DIR", &session_dir);
        }

        let viewport = Rect::new(0, 0, cols, rows);
        let mut wm = WindowManager::new_headless(rows.saturating_sub(1).max(1), cols);
        wm.config_mut().session_dir = Some(session_dir.to_string_lossy().into_owned());
        wm.update_agent_states(std::time::Instant::now());
        let shell = ShellLayout::compute(viewport, wm.config(), wm.pane_count());
        let _ = wm.resize_all(shell.workspace);

        Self {
            _env_guard: env_guard,
            temp_dir,
            home_dir,
            config_home,
            state_home,
            data_home,
            session_dir,
            snapshot_dir,
            wm,
            viewport,
            rpc_events: VecDeque::new(),
        }
    }

    /// Synchronizes pane geometries and agent state tracker after layout or appearance changes.
    pub fn sync_layout(&mut self) {
        self.wm.update_agent_states(std::time::Instant::now());
        let shell = ShellLayout::compute(self.viewport, self.wm.config(), self.wm.pane_count());
        let _ = self.wm.resize_all(shell.workspace);
    }

    // =========================================================================
    // Mode 1: Live Interactive TUI Driver (`Ratatui TestBackend`)
    // =========================================================================

    /// Sends a single key event through `handle_wm_key_event`.
    pub fn press_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let ev = KeyEvent::new(code, modifiers);
        let _ = handle_wm_key_event(&mut self.wm, ev);
        self.sync_layout();
    }

    /// Types a string of characters one key event at a time.
    pub fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => self.press_key(KeyCode::Enter, KeyModifiers::NONE),
                '\t' => self.press_key(KeyCode::Tab, KeyModifiers::NONE),
                c => self.press_key(KeyCode::Char(c), KeyModifiers::NONE),
            }
        }
    }

    /// Clears the current input line (`Ctrl+U`) and types `text`.
    pub fn clear_and_type(&mut self, text: &str) {
        self.press_key(KeyCode::Char('u'), KeyModifiers::CONTROL);
        self.type_text(text);
    }

    /// Synthesizes a left-click (Down + Up) at `(col, row)`.
    pub fn click(&mut self, col: u16, row: u16) {
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_wm_mouse_event(&mut self.wm, down, self.viewport);
        let _ = handle_wm_mouse_event(&mut self.wm, up, self.viewport);
        self.sync_layout();
    }

    /// Synthesizes a right-click (Down + Up) at `(col, row)`.
    pub fn right_click(&mut self, col: u16, row: u16) {
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Right),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_wm_mouse_event(&mut self.wm, down, self.viewport);
        let _ = handle_wm_mouse_event(&mut self.wm, up, self.viewport);
        self.sync_layout();
    }

    /// Synthesizes a mouse left-drag from `from` to `to`.
    pub fn drag(&mut self, from: (u16, u16), to: (u16, u16)) {
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: from.0,
            row: from.1,
            modifiers: KeyModifiers::NONE,
        };
        let move_ev = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: to.0,
            row: to.1,
            modifiers: KeyModifiers::NONE,
        };
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: to.0,
            row: to.1,
            modifiers: KeyModifiers::NONE,
        };
        let _ = handle_wm_mouse_event(&mut self.wm, down, self.viewport);
        let _ = handle_wm_mouse_event(&mut self.wm, move_ev, self.viewport);
        let _ = handle_wm_mouse_event(&mut self.wm, up, self.viewport);
        self.sync_layout();
    }

    /// Parses and executes a `:colon` command against `WindowManager`.
    pub fn run_colon(&mut self, cmd: &str) -> Result<(), String> {
        let trimmed = cmd.trim();
        let normalized = if trimmed.starts_with(':') {
            trimmed.to_string()
        } else {
            format!(":{trimmed}")
        };
        let Some(res) = parse_colon_command(&normalized) else {
            return Ok(());
        };
        let parsed = res?;
        let _ = self.wm.apply_automation(parsed, None)?;
        self.sync_layout();
        Ok(())
    }

    /// Renders `WorkspaceWidget` into a `ratatui::backend::TestBackend` and returns the `Buffer`.
    pub fn render_buffer(&self) -> Buffer {
        let backend = TestBackend::new(self.viewport.width, self.viewport.height);
        let mut terminal = Terminal::new(backend).expect("create TestBackend terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(WorkspaceWidget::new(&self.wm), area);
            })
            .expect("draw WorkspaceWidget onto TestBackend");
        terminal.backend().buffer().clone()
    }

    /// Extracts the full rendered screen as newline-separated rows.
    pub fn screen_text(&self) -> String {
        let buf = self.render_buffer();
        let mut lines = Vec::with_capacity(self.viewport.height as usize);
        for y in 0..self.viewport.height {
            let mut row = String::with_capacity(self.viewport.width as usize);
            for x in 0..self.viewport.width {
                if let Some(cell) = buf.cell((x, y)) {
                    row.push_str(cell.symbol());
                }
            }
            lines.push(row);
        }
        lines.join("\n")
    }

    /// Panics with a formatted screen dump if `needle` is not present in `screen_text()`.
    pub fn assert_screen_contains(&self, needle: &str) {
        let screen = self.screen_text();
        assert!(
            screen.contains(needle),
            "Expected screen to contain {needle:?}, but screen was:\n{screen}"
        );
    }

    // =========================================================================
    // Mode 2: Live GUI Framebuffer & Virtual Framebuffer (`GuiWorkspaceRenderer` + `xvfb-run`)
    // =========================================================================

    /// Renders the workspace through `GuiWorkspaceRenderer` and returns the `GuiFrameSurface`.
    pub fn render_gui_surface(&self, cell_w: u16, cell_h: u16) -> GuiFrameSurface {
        let mut renderer = GuiWorkspaceRenderer::new(cell_w, cell_h);
        let mut surface = GuiFrameSurface::new(cell_w, cell_h);
        renderer.render_workspace(&self.wm, self.viewport, &mut surface);
        surface
    }

    /// Rasterizes the current workspace into a packed RGBA8 pixel buffer.
    pub fn rasterize_rgba(&self, cell_w: u16, cell_h: u16) -> Vec<u8> {
        let surface = self.render_gui_surface(cell_w, cell_h);
        let (_w_px, _h_px, fb) = surface.rasterize_rgba();
        fb
    }

    /// Runs `bilterm --gui-dump` with `args` across Linux (`xvfb-run -a` fallback), macOS (Cocoa/CoreText), and Windows (Win32/GDI).
    pub fn run_xvfb_gui_dump(&self, args: &[&str]) -> Output {
        self.run_platform_gui_dump(args)
    }

    /// Cross-platform GUI dump runner supporting Linux (`xvfb-run -a` if available), macOS, and Windows.
    pub fn run_platform_gui_dump(&self, args: &[&str]) -> Output {
        let bin = env!("CARGO_BIN_EXE_bilterm");

        #[cfg(target_os = "linux")]
        let mut cmd = {
            let has_xvfb = Command::new("xvfb-run")
                .arg("--help")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if has_xvfb {
                let mut c = Command::new("xvfb-run");
                c.arg("-a").arg(bin);
                c
            } else {
                Command::new(bin)
            }
        };

        #[cfg(target_os = "macos")]
        let mut cmd = Command::new(bin);

        #[cfg(target_os = "windows")]
        let mut cmd = Command::new(bin);

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let mut cmd = Command::new(bin);

        cmd.args(args);
        self.apply_hermetic_env(&mut cmd);
        cmd.output().expect("execute bilterm --gui-dump")
    }

    // =========================================================================
    // Mode 3: Live Subprocess Daemon / JSON-RPC / MCP Stdio / CLI Driver
    // =========================================================================

    fn apply_hermetic_env(&self, cmd: &mut Command) {
        cmd.env("HOME", &self.home_dir)
            .env("USERPROFILE", &self.home_dir)
            .env("APPDATA", &self.config_home)
            .env("LOCALAPPDATA", &self.state_home)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_STATE_HOME", &self.state_home)
            .env("XDG_DATA_HOME", &self.data_home)
            .env("BILTERM_SESSION_DIR", &self.session_dir);
    }

    /// Spawns `bilterm --mcp` with piped stdin/stdout inside the hermetic environment.
    pub fn spawn_mcp_stdio(&self) -> McpLiveClient {
        let bin = env!("CARGO_BIN_EXE_bilterm");
        let mut cmd = Command::new(bin);
        cmd.arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        self.apply_hermetic_env(&mut cmd);
        let mut child = cmd.spawn().expect("spawn bilterm --mcp subprocess");
        let stdin = child.stdin.take().expect("take MCP child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("take MCP child stdout"));
        McpLiveClient {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    /// Runs `bilterm` CLI with `args` inside the hermetic environment.
    pub fn run_cli(&self, args: &[&str]) -> Output {
        let bin = env!("CARGO_BIN_EXE_bilterm");
        let mut cmd = Command::new(bin);
        cmd.args(args);
        self.apply_hermetic_env(&mut cmd);
        cmd.output().expect("run bilterm CLI")
    }

    /// Dispatches a JSON-RPC 2.0 request against the live in-memory `WindowManager` via `AutomationControlPlane`.
    pub fn drive_rpc_in_memory(&mut self, method: &str, params: &str) -> String {
        let mapped_method = match method {
            "find.query" => {
                if let Some(parsed) = JsonValue::parse(params) {
                    match parsed.get("kind").and_then(JsonValue::as_str) {
                        Some("sessions" | "session") => "find.sessions",
                        Some("history") => "find.history",
                        _ => "find.agents",
                    }
                } else {
                    "find.agents"
                }
            }
            other => other,
        };
        let params_raw = if params.trim().is_empty() {
            "{}"
        } else {
            params
        };
        let req_line = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"{mapped_method}","params":{params_raw}}}"#
        );

        let wm = std::mem::replace(&mut self.wm, WindowManager::new_headless(1, 1));
        let mut plane =
            AutomationControlPlane::new(wm, self.viewport).with_snapshot_dir(self.snapshot_dir.clone());
        plane.events = std::mem::take(&mut self.rpc_events);
        let resp = plane.handle_jsonrpc_line(&req_line);
        self.wm = plane.wm;
        self.rpc_events = plane.events;
        self.sync_layout();
        resp
    }

    // =========================================================================
    // Mode 4: Live & Batch Scripting Driver (`.bt` Script Studio & `.toml` Playbook)
    // =========================================================================

    /// Writes a `.bt` script into the hermetic script directory and loads it into Script Studio.
    pub fn load_bt_script(&mut self, name: &str, script_text: &str) -> PathBuf {
        let clean = name.trim_end_matches(".bt");
        let file_name = format!("{clean}.bt");
        let scripts_dir = self.config_home.join("bilterm").join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts dir");
        let path = scripts_dir.join(&file_name);
        std::fs::write(&path, script_text).expect("write .bt script");
        let _ = self
            .wm
            .script_studio_apply(&ScriptStudioOp::Load(clean.to_string()));
        self.sync_layout();
        path
    }

    /// Steps the next executable line in Script Studio (`F10` / `StepNext`).
    pub fn step_bt_script(&mut self) {
        let _ = self.wm.script_studio_apply(&ScriptStudioOp::StepNext);
        self.sync_layout();
    }

    /// Runs all lines in Script Studio (`F5` / `RunAll`).
    pub fn run_bt_script(&mut self) {
        let _ = self.wm.script_studio_apply(&ScriptStudioOp::RunAll);
        self.sync_layout();
    }

    /// Parses and executes a `.toml` `Playbook` against the live `WindowManager`.
    pub fn run_playbook_toml(&mut self, toml_content: &str) -> Result<PlaybookReport, String> {
        let playbook = Playbook::from_toml_str(toml_content).map_err(|e| e.to_string())?;
        let wm = std::mem::replace(&mut self.wm, WindowManager::new_headless(1, 1));
        let mut plane =
            AutomationControlPlane::new(wm, self.viewport).with_snapshot_dir(self.snapshot_dir.clone());
        let res = plane.run_playbook_with_policy(&playbook, SnapshotPolicy::Confined);
        self.wm = plane.wm;
        self.rpc_events = plane.events;
        self.sync_layout();
        res
    }
}
