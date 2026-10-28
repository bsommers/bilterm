//! Milestone `v2.0` (`Phases 30–34`) `WindowManager` runtime methods:
//! - Phase 30 (`SAFE-01..02`): Explicit Exit Guard modal, final/root pane protection & `:quit!`/`:detach`
//! - Phase 31 (`THM-01..02`): `.itermcolors` & Open Theme/Typography import, export & directory discovery
//! - Phase 32 (`DAEM-01..02`): Persistent Background Multiplexer Daemon (`DaemonServerState`) & multi-client attach/detach
//! - Phase 33 (`TREE-01..02`, `PERS-01..02`): Hierarchical `SessionTree` (`branch`/`graft`/`detach-subtree`) & atomic `WorkspaceJournal`
//! - Phase 34 (`MCP-01..02`): TRON-inspired Master Control Program (`MCP`) root terminal

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::daemon::DaemonServerState;
use crate::error::Result;
use crate::mcp_console::McpConsoleState;
use crate::pane::exit_guard::{
    ExitGuardChoice, ExitGuardState, ExitProtectionPolicy, should_intercept_ctrl_d,
    should_intercept_prompt_exit,
};
use crate::pane::{CompositePane, PaneId};
use crate::ui::theme::ThemePreset;
use crate::ui::theme_import::{
    discover_themes_in_dir, export_open_theme_toml, load_open_theme_file, OpenColorScheme,
};
use crate::wm::journal::{PaneJournalRecord, SessionJournalRecord, WorkspaceJournal};
use crate::wm::session_tree::{SessionNodeKind, SessionTree};
use crate::wm::split_tree::SplitDirection;
use crate::wm::{Modal, WindowManager, WorkspaceLayoutMode};

impl WindowManager {
    /// Drains any outbound OSC 52 clipboard payloads queued across workspace panes and the quick terminal.
    pub fn drain_outbound_osc52(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for pane in self
            .panes
            .values_mut()
            .chain(std::iter::once(&mut self.quick_terminal.pane))
        {
            out.extend(pane.take_all_outbound_osc52());
        }
        out
    }

    /// Submits `cmd` through [`CompositePane::submit_command`] across all writable workspace panes.
    pub fn broadcast_command(&mut self, cmd: &str) -> Result<usize> {
        let mut count = 0;
        for pane in self.panes.values_mut() {
            if !pane.is_read_only() && !pane.is_password_prompt() {
                pane.submit_command(cmd)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Allocates and registers a new headless pane mapped to a `tmux -CC` `%pane_id`.
    pub fn insert_tmux_pane(&mut self, tmux_pane_id: usize) -> PaneId {
        let new_id = self
            .panes
            .keys()
            .max()
            .copied()
            .unwrap_or(1)
            .max(self.next_pane_id.saturating_sub(1))
            + 1;
        self.next_pane_id = new_id + 1;
        let (rows, cols) = self.active_pane().vt().size();
        let mut pane = CompositePane::new_headless(new_id, rows + 1, cols);
        pane.set_title(format!("TMUX %{tmux_pane_id}"));
        pane.set_scroll_mode_from_config(self.config.scroll_mode);
        Self::attach_history(&mut pane, &self.config.history_settings());
        let top = self.z_stack.top().unwrap_or(1);
        self.panes.insert(new_id, pane);
        self.split_tree
            .split_leaf(top, new_id, SplitDirection::Vertical);
        self.z_stack.push(new_id);
        self.rebalance_scrollback(Instant::now());
        new_id
    }

    /// Returns the active Borland/3270 Exit Guard modal state if open (`SAFE-01`).
    pub fn exit_guard_state(&self) -> Option<&ExitGuardState> {
        match &self.modal {
            Modal::ExitGuard(s) => Some(s),
            _ => None,
        }
    }

    /// Mutable access to the active Borland/3270 Exit Guard modal state (`SAFE-01`).
    pub fn exit_guard_state_mut(&mut self) -> Option<&mut ExitGuardState> {
        match &mut self.modal {
            Modal::ExitGuard(s) => Some(s),
            _ => None,
        }
    }

    /// Opens the interactive Borland/3270 Exit Guard modal (`SAFE-01`).
    pub fn open_exit_guard(&mut self, trigger_reason: impl Into<String>) {
        let pane_id = self.active_pane_id();
        let session_name = self.config.history_session.clone();
        let is_root = self.config.is_root_session;
        self.modal = Modal::ExitGuard(ExitGuardState::new(
            pane_id,
            session_name,
            is_root,
            trigger_reason,
        ));
    }

    /// Closes the Exit Guard modal without exiting (`SAFE-01`).
    pub fn dismiss_exit_guard(&mut self) {
        if matches!(self.modal, Modal::ExitGuard(_)) {
            self.modal = Modal::None;
        }
    }

    /// Checks if submitting `cmd` in the focused pane's FloatingCommandPrompt should be intercepted
    /// by the Exit Guard (`SAFE-01`). If intercepted, opens the Exit Guard modal (or executes policy)
    /// and returns `true`.
    pub fn intercept_prompt_exit_if_needed(&mut self, cmd: &str) -> bool {
        let pane_count = self.panes.len();
        let is_root = self.config.is_root_session;
        let policy = self.config.exit_protection;
        let in_alt = self.active_pane().vt().is_alternate_screen();
        let in_echo_lock = self.active_pane().is_password_prompt();

        if !should_intercept_prompt_exit(cmd, pane_count, is_root, policy, in_alt, in_echo_lock) {
            return false;
        }

        match policy {
            ExitProtectionPolicy::Guard => {
                self.open_exit_guard(format!("Command '{}' on final/root pane", cmd.trim()));
                true
            }
            ExitProtectionPolicy::Respawn => {
                let _ = self.active_pane_mut().respawn_shell_in_place();
                true
            }
            ExitProtectionPolicy::Detach => {
                self.detach_client();
                true
            }
            ExitProtectionPolicy::Off => false,
        }
    }

    /// Checks if pressing `Ctrl+D` on an empty prompt should be intercepted by the Exit Guard (`SAFE-01`).
    pub fn intercept_ctrl_d_if_needed(&mut self) -> bool {
        let prompt_is_empty = self.active_pane().prompt().text().is_empty();
        let pane_count = self.panes.len();
        let is_root = self.config.is_root_session;
        let policy = self.config.exit_protection;
        let in_alt = self.active_pane().vt().is_alternate_screen();
        let in_echo_lock = self.active_pane().is_password_prompt();

        if !should_intercept_ctrl_d(
            prompt_is_empty,
            pane_count,
            is_root,
            policy,
            in_alt,
            in_echo_lock,
        ) {
            return false;
        }

        match policy {
            ExitProtectionPolicy::Guard => {
                self.open_exit_guard("Ctrl+D (EOF) on empty prompt in final/root pane");
                true
            }
            ExitProtectionPolicy::Respawn => {
                let _ = self.active_pane_mut().respawn_shell_in_place();
                true
            }
            ExitProtectionPolicy::Detach => {
                self.detach_client();
                true
            }
            ExitProtectionPolicy::Off => false,
        }
    }

    /// Executes an [`ExitGuardChoice`] inside the Exit Guard modal. Returns `true` if the client should exit.
    pub fn apply_exit_guard_choice(&mut self, choice: ExitGuardChoice) -> bool {
        self.dismiss_exit_guard();
        match choice {
            ExitGuardChoice::RespawnShell => {
                let _ = self.active_pane_mut().respawn_shell_in_place();
                false
            }
            ExitGuardChoice::DetachClient => {
                self.detach_client();
                true
            }
            ExitGuardChoice::QuitClient | ExitGuardChoice::KillSession => true,
            ExitGuardChoice::Cancel => {
                self.active_pane_mut()
                    .set_status_message(Some("Exit cancelled — session preserved".to_string()));
                false
            }
        }
    }

    /// Detaches the current UI client while preserving the background daemon / session (`SAFE-02`, `DAEM-02`).
    pub fn detach_client(&mut self) -> bool {
        self.client_detached = true;
        self.active_pane_mut().set_status_message(Some(
            "Detached client from session (session preserved in background)".to_string(),
        ));
        true
    }

    /// Returns `true` if the client detached cleanly via `:detach` or Exit Guard `(d)etach` (`SAFE-02`).
    pub fn is_client_detached(&self) -> bool {
        self.client_detached
    }

    /// Access to the embedded background multiplexer daemon coordinator (`DAEM-01`).
    pub fn daemon_state(&self) -> &DaemonServerState {
        &self.daemon_state
    }

    /// Mutable access to the embedded background multiplexer daemon coordinator (`DAEM-01`).
    pub fn daemon_state_mut(&mut self) -> &mut DaemonServerState {
        &mut self.daemon_state
    }

    /// Access to the hierarchical `SessionTree` (`TREE-01`).
    pub fn session_tree(&self) -> &SessionTree {
        &self.daemon_state.session_tree
    }

    /// Mutable access to the hierarchical `SessionTree` (`TREE-01`).
    pub fn session_tree_mut(&mut self) -> &mut SessionTree {
        &mut self.daemon_state.session_tree
    }

    /// Access to the TRON Master Control Program (`MCP`) root terminal state (`MCP-01`).
    pub fn mcp_console(&self) -> &McpConsoleState {
        &self.mcp_console
    }

    /// Mutable access to the TRON Master Control Program (`MCP`) root terminal state (`MCP-01`).
    pub fn mcp_console_mut(&mut self) -> &mut McpConsoleState {
        &mut self.mcp_console
    }

    /// Builds a crash-safe [`WorkspaceJournal`] snapshot of the current workspace (`PERS-01`, `PERS-02`).
    ///
    /// Strictly separates `SplitTree` ratios (`Tiled` mode) from `ZStack` coordinates (`Stacked` mode).
    pub fn build_workspace_journal(&self) -> WorkspaceJournal {
        let layout_str = match self.layout_mode {
            WorkspaceLayoutMode::Tiled => "tiled",
            WorkspaceLayoutMode::Stacked => "stacked",
        };
        let z_rects = if self.layout_mode == WorkspaceLayoutMode::Stacked {
            let area = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            self.z_stack
                .compute_cascade_rects(area)
                .into_iter()
                .map(|(id, r)| (id, r.x, r.y, r.width, r.height))
                .collect()
        } else {
            Vec::new()
        };

        let panes = self
            .panes
            .values()
            .map(|p| PaneJournalRecord {
                pane_id: p.id(),
                title: p.title().to_string(),
                cwd: p
                    .vt()
                    .reported_cwd()
                    .and_then(|c| c.to_str())
                    .map(str::to_string),
                font_scale_pct: self.pane_effective_scale_pct(p.id()),
                scroll_mode: p.scroll_mode().label().to_ascii_lowercase(),
                prompt_anchor: format!("{:?}", p.prompt().anchor),
                prompt_offset_y: 0,
                prompt_height: p.prompt().height,
                command_history: p.prompt().history.iter().rev().take(50).cloned().collect(),
                scrollback_tail: (0..p.vt().size().0.min(10))
                    .map(|r| p.vt().viewport_line_text(r))
                    .filter(|s| !s.trim().is_empty())
                    .collect(),
            })
            .collect();

        let session_rec = SessionJournalRecord {
            session_name: self.config.history_session.clone(),
            layout_mode: layout_str.to_string(),
            active_pane_id: self.active_pane_id(),
            split_ratios: vec![50],
            z_stack_rects: z_rects,
            theme_name: self.config.theme.label().to_string(),
            panes,
        };

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        WorkspaceJournal {
            version: 1,
            timestamp_epoch_secs: now_secs,
            active_session: self.config.history_session.clone(),
            session_tree: self.daemon_state.session_tree.clone(),
            sessions: vec![session_rec],
        }
    }

    /// Saves an atomic `0600` crash-recovery journal checkpoint to `journal_dir` (or default private dir) (`PERS-02`).
    pub fn save_journal_checkpoint(
        &mut self,
        journal_dir: Option<&Path>,
        slot_name: &str,
    ) -> std::io::Result<PathBuf> {
        let dir = match journal_dir.or(self.daemon_state.journal_dir.as_deref()) {
            Some(d) => d.to_path_buf(),
            None => crate::secure_fs::private_runtime_dir()
                .and_then(|base| crate::secure_fs::private_dir_in(&base, "journal"))?,
        };
        let journal = self.build_workspace_journal();
        journal.save_atomic(&dir, slot_name)
    }

    /// Applies an [`OpenColorScheme`] to the workspace and active pane, including any per-theme typography profile (`THM-01`, `THM-02`).
    pub fn apply_open_color_scheme(&mut self, scheme: &OpenColorScheme) {
        self.config.custom_palette_override = Some(scheme.to_theme_palette());
        if let Some(ref fam) = scheme.typography.font_family {
            self.config.font_family = fam.clone();
        }
        if let Some(pt) = scheme.typography.font_size_pt {
            self.config.font_size_pt = pt.clamp(6, 72);
        }
        if let Some(scale) = scheme.typography.font_scale_pct {
            let clamped = scale.clamp(50, 300);
            self.active_pane_mut().font_options_mut().scale_pct = Some(clamped);
            self.config.font_scale_pct = clamped;
            let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            let _ = self.resize_all(vp);
        }
        self.active_pane_mut().set_status_message(Some(format!(
            "Applied open color scheme '{}'",
            scheme.name
        )));
    }

    /// Synchronizes the live workspace session & pane topology into `self.daemon_state`
    /// so `:mcp status`, `:mcp tree`, and `:session tree` always reflect live panes (`DAEM-01`, `TREE-01`).
    pub fn sync_daemon_state_from_workspace(&mut self) {
        let sess = self.config.history_session.clone();
        let pane_count = self.panes.len().max(1);
        if self.daemon_state.session_tree.node(&sess).is_none() {
            self.daemon_state
                .session_tree
                .upsert_session(&sess, SessionNodeKind::Rooted, None);
        }
        if let Some(node) = self.daemon_state.session_tree.node_mut(&sess) {
            node.pane_count = pane_count;
        }
        self.daemon_state.session_panes.insert(sess, pane_count);
    }

    /// Opens the floating (`Overlay`) or docked (`Push`) MCP pop-up window with `title` and `body_lines`
    /// without polluting the child shell's PTY output canvas (`MCP-01`, `MCP-02`).
    pub fn open_mcp_popup_window(&mut self, title: &str, body_lines: Vec<String>) {
        self.mcp_console.open_with_view(title, body_lines);
        if self.mcp_console.layout_mode == crate::wm::DrawerLayoutMode::Push {
            let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            let _ = self.resize_all(vp);
        }
    }

    /// Builds the multi-line session + live-pane hierarchy lines for `:mcp tree` and `:session tree`.
    pub fn build_full_tree_lines(&mut self) -> Vec<String> {
        self.sync_daemon_state_from_workspace();
        let active_sess = self.config.history_session.clone();
        let active_pane_id = self.active_pane_id();
        let base_lines = self
            .daemon_state
            .session_tree
            .render_herdr_tree_lines(&active_sess);
        let mut pane_ids: Vec<PaneId> = self.panes.keys().copied().collect();
        pane_ids.sort_unstable();

        let mut out = Vec::new();
        for line in base_lines {
            let is_active_sess_line = line.contains(&format!("● {active_sess} "));
            out.push(line);
            if is_active_sess_line {
                let total = pane_ids.len();
                for (idx, pid) in pane_ids.iter().copied().enumerate() {
                    if let Some(p) = self.panes.get(&pid) {
                        let branch = if idx + 1 == total { "╰─" } else { "├─" };
                        let focus_mark = if pid == active_pane_id { "►" } else { " " };
                        let (rows, cols) = p.vt().size();
                        let scale = self.pane_effective_scale_pct(pid);
                        let mode_lbl = if p.vt().is_alternate_screen() {
                            "ALT_SCREEN"
                        } else {
                            match p.prompt_mode() {
                                crate::pane::PromptMode::Floating => "FLOAT_CMD",
                                crate::pane::PromptMode::Fixed => "FIXED_CMD",
                                crate::pane::PromptMode::RegularTerminal => "RAW_TERM",
                            }
                        };
                        out.push(format!(
                            "   {branch} {focus_mark} [#{pid}] {} ({cols}x{rows}, {mode_lbl}, SCRL:{}, {scale}%)",
                            p.title(),
                            p.scroll_mode().label()
                        ));
                    }
                }
            }
        }
        out
    }

    /// Builds the multi-line TRON `MCP` system & session status lines for `:mcp status`.
    pub fn build_mcp_status_lines(&mut self) -> Vec<String> {
        self.sync_daemon_state_from_workspace();
        let uptime = self.daemon_state.started_at.elapsed().as_secs();
        let pid = std::process::id();
        let sock = crate::daemon::DaemonServerState::default_socket_path();
        let layout_lbl = match self.layout_mode {
            WorkspaceLayoutMode::Tiled => "TILED (SplitTree)",
            WorkspaceLayoutMode::Stacked => "STACKED (ZStack)",
        };
        let active_id = self.active_pane_id();
        let active_scale = self.pane_effective_scale_pct(active_id);
        let active_scroll = self.active_pane().scroll_mode().label();
        let mut lines = vec![
            format!(
                "DAEMON     : {} | PID={pid} | UPTIME={uptime}s | CLIENTS={}",
                if self.daemon_state.running { "ONLINE" } else { "OFFLINE" },
                self.daemon_state.clients.len().max(1)
            ),
            format!("SOCKET     : {}", sock.display()),
            format!(
                "WORKSPACE  : Session='{}' | Layout={layout_lbl} | Panes={} | Active=#{active_id}",
                self.config.history_session,
                self.panes.len()
            ),
            format!(
                "PROTECTION : ExitGuard={:?} | RootSession={}",
                self.config.exit_protection, self.config.is_root_session
            ),
            format!(
                "AESTHETICS : Theme='{}' | Font='{}' {}pt ({active_scale}%) | Scroll={active_scroll}",
                self.config.theme.label(),
                self.config.font_family,
                self.config.font_size_pt
            ),
            "─".repeat(66),
        ];
        lines.extend(self.build_full_tree_lines());
        lines
    }

    /// Executes `:theme import|load|export|list|<preset>` commands (`THM-01`, `THM-02`).
    pub fn execute_theme_command(&mut self, subcmd: &str) {
        let parts: Vec<&str> = subcmd.split_whitespace().collect();
        match parts.first().copied() {
            None | Some("list" | "ls") => {
                let names: Vec<&str> = ThemePreset::all().iter().map(|p| p.label()).collect();
                let mut lines = Vec::new();
                for (idx, preset) in ThemePreset::all().iter().enumerate() {
                    let marker = if *preset == self.config.theme { "►" } else { " " };
                    lines.push(format!("{marker} {:2}. {}", idx + 1, preset.label()));
                }
                lines.push("Import open schemes: :theme import <file.itermcolors|.toml|.yaml>".to_string());
                self.open_mcp_popup_window("AVAILABLE THEME PRESETS (THM-01)", lines);
                self.active_pane_mut().set_status_message(Some(format!(
                    "Themes ({}): {}",
                    names.len(),
                    names.join(", ")
                )));
            }
            Some("import" | "load") => {
                let Some(path_str) = parts.get(1).copied() else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :theme import <file.itermcolors|file.toml|file.yaml|dir>".to_string(),
                    ));
                    return;
                };
                let path = Path::new(path_str);
                if path.is_dir() {
                    let schemes = discover_themes_in_dir(path);
                    if let Some(first) = schemes.first() {
                        self.apply_open_color_scheme(first);
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Imported {} theme(s) from {}; applied '{}'",
                            schemes.len(),
                            path.display(),
                            first.name
                        )));
                    } else {
                        self.active_pane_mut().set_status_message(Some(format!(
                            "No themes found in {}",
                            path.display()
                        )));
                    }
                } else {
                    match load_open_theme_file(path) {
                        Ok(scheme) => self.apply_open_color_scheme(&scheme),
                        Err(err) => {
                            self.record_automation_error(err.clone());
                            self.active_pane_mut()
                                .set_status_message(Some(format!("Theme import error: {err}")));
                        }
                    }
                }
            }
            Some("export" | "save") => {
                let scheme = OpenColorScheme::from_preset(self.config.theme);
                let toml_out = export_open_theme_toml(&scheme);
                if let Some(dest) = parts.get(1).copied() {
                    if let Err(e) = crate::secure_fs::validate_plain_file_name(dest) {
                        self.record_automation_error(e.clone());
                        self.active_pane_mut().set_status_message(Some(e));
                        return;
                    }
                    let base = self
                        .automation_confine_dir
                        .clone()
                        .or_else(|| crate::secure_fs::private_runtime_dir().ok())
                        .unwrap_or_else(std::env::temp_dir);
                    let target = base.join(dest);
                    match crate::secure_fs::write_private_file(&target, toml_out.as_bytes()) {
                        Ok(()) => self.active_pane_mut().set_status_message(Some(format!(
                            "Exported open theme TOML to {}",
                            target.display()
                        ))),
                        Err(e) => self.active_pane_mut().set_status_message(Some(format!(
                            "Theme export error: {e}"
                        ))),
                    }
                } else {
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Exported '{}' ({} bytes open TOML)",
                        scheme.name,
                        toml_out.len()
                    )));
                }
            }
            Some(name) => {
                if let Some(preset) = ThemePreset::from_name(name) {
                    self.config.theme = preset;
                    self.config.custom_palette_override = None;
                    self.notify_theme_changed();
                } else {
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Unknown theme '{name}'. Try :theme list or :theme import <path>"
                    )));
                }
            }
        }
    }

    /// Executes `:session branch|graft|detach-subtree|tree|checkpoint` commands (`TREE-01..02`, `PERS-01..02`).
    pub fn execute_session_tree_command(&mut self, subcmd: &str) {
        let parts: Vec<&str> = subcmd.split_whitespace().collect();
        match parts.first().copied() {
            None | Some("tree") => {
                let lines = self.build_full_tree_lines();
                let joined = lines.join(" | ");
                self.open_mcp_popup_window("SESSION & PANE HIERARCHY (TREE-01)", lines);
                self.active_pane_mut().set_status_message(Some(joined));
            }
            Some("branch" | "spawn") => {
                let Some(arg) = parts.get(1).copied() else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :session branch <parent>/<child>".to_string(),
                    ));
                    return;
                };
                let res = if let Some((parent, child)) = arg.split_once('/') {
                    self.daemon_state.session_tree.branch(parent, child)
                } else if let Some(child) = parts.get(2).copied() {
                    self.daemon_state.session_tree.branch(arg, child)
                } else {
                    self.daemon_state
                        .session_tree
                        .upsert_session(arg, SessionNodeKind::Independent, None);
                    Ok(())
                };
                match res {
                    Ok(()) => self.active_pane_mut().set_status_message(Some(format!(
                        "Session tree updated: branched '{arg}'"
                    ))),
                    Err(e) => {
                        self.record_automation_error(e.clone());
                        self.active_pane_mut().set_status_message(Some(e));
                    }
                }
            }
            Some("graft") => {
                let (Some(src), Some(dst)) = (parts.get(1).copied(), parts.get(2).copied()) else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :session graft <src> <dst_parent>".to_string(),
                    ));
                    return;
                };
                match self.daemon_state.session_tree.graft(src, dst) {
                    Ok(()) => self.active_pane_mut().set_status_message(Some(format!(
                        "Grafted subtree '{src}' under '{dst}'"
                    ))),
                    Err(e) => {
                        self.record_automation_error(e.clone());
                        self.active_pane_mut().set_status_message(Some(e));
                    }
                }
            }
            Some("detach-subtree" | "promote") => {
                let Some(node) = parts.get(1).copied() else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :session detach-subtree <node>".to_string(),
                    ));
                    return;
                };
                match self.daemon_state.session_tree.detach_subtree(node) {
                    Ok(()) => self.active_pane_mut().set_status_message(Some(format!(
                        "Detached '{node}' as an independent root session"
                    ))),
                    Err(e) => {
                        self.record_automation_error(e.clone());
                        self.active_pane_mut().set_status_message(Some(e));
                    }
                }
            }
            Some("checkpoint" | "journal") => {
                let slot = parts.get(1).copied().unwrap_or("latest");
                match self.save_journal_checkpoint(None, slot) {
                    Ok(path) => self.active_pane_mut().set_status_message(Some(format!(
                        "Saved crash-safe journal checkpoint to {}",
                        path.display()
                    ))),
                    Err(e) => self.active_pane_mut().set_status_message(Some(format!(
                        "Journal checkpoint error: {e}"
                    ))),
                }
            }
            _ => {}
        }
    }

    /// Executes `:mcp [status|tree|float|push|attach|spawn|graft|broadcast|freeze|thaw|derez|checkpoint|shutdown ...]` (`MCP-01..02`).
    /// Returns `true` only if `mcp shutdown --confirm` was executed and should terminate.
    pub fn execute_mcp_command(&mut self, subcmd: &str) -> bool {
        self.sync_daemon_state_from_workspace();
        let trimmed = subcmd.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let first_verb = parts.first().copied().unwrap_or("");

        if trimmed.is_empty()
            || first_verb.eq_ignore_ascii_case("open")
            || first_verb.eq_ignore_ascii_case("console")
            || first_verb.eq_ignore_ascii_case("status")
        {
            let _ = self.mcp_console.execute("status", &mut self.daemon_state);
            let lines = self.build_mcp_status_lines();
            self.open_mcp_popup_window("MASTER CONTROL PROGRAM // SYSTEM & DAEMON STATUS", lines);
            let sess_len = self.daemon_state.session_tree.nodes.len();
            let pane_len = self.panes.len();
            let cli_len = self.daemon_state.clients.len().max(1);
            self.active_pane_mut().set_status_message(Some(format!(
                "MCP STATUS: ONLINE | SESSIONS={sess_len} | PANES={pane_len} | CLIENTS={cli_len}. END OF LINE."
            )));
            return false;
        }
        if first_verb.eq_ignore_ascii_case("close") || first_verb.eq_ignore_ascii_case("exit") {
            self.mcp_console.close_root_terminal();
            let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            let _ = self.resize_all(vp);
            self.active_pane_mut().set_status_message(Some(
                "MCP ROOT CONSOLE CLOSED (DAEMON & SESSIONS PRESERVED). END OF LINE.".to_string(),
            ));
            return false;
        }
        if first_verb.eq_ignore_ascii_case("float") || first_verb.eq_ignore_ascii_case("overlay") {
            self.mcp_console.layout_mode = crate::wm::DrawerLayoutMode::Overlay;
            if !self.mcp_console.active {
                let lines = self.build_mcp_status_lines();
                self.mcp_console.open_with_view("MASTER CONTROL PROGRAM // SYSTEM & DAEMON STATUS", lines);
            }
            let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            let _ = self.resize_all(vp);
            self.active_pane_mut().set_status_message(Some(
                "MCP WINDOW MODE: FLOATING POP-UP (OVERLAY). END OF LINE.".to_string(),
            ));
            return false;
        }
        if first_verb.eq_ignore_ascii_case("push") {
            self.mcp_console.layout_mode = crate::wm::DrawerLayoutMode::Push;
            if let Some(edge_arg) = parts.get(1).copied() {
                self.mcp_console.edge = match edge_arg.to_ascii_lowercase().as_str() {
                    "bottom" | "bot" => crate::wm::DrawerEdge::Bottom,
                    "left" => crate::wm::DrawerEdge::Left,
                    "right" => crate::wm::DrawerEdge::Right,
                    _ => crate::wm::DrawerEdge::Top,
                };
            }
            if !self.mcp_console.active {
                let lines = self.build_mcp_status_lines();
                self.mcp_console.open_with_view("MASTER CONTROL PROGRAM // SYSTEM & DAEMON STATUS", lines);
            }
            let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
            let _ = self.resize_all(vp);
            let edge_lbl = self.mcp_console.edge.label();
            self.active_pane_mut().set_status_message(Some(format!(
                "MCP WINDOW MODE: PUSH-OUT ({edge_lbl}). END OF LINE."
            )));
            return false;
        }
        if first_verb.eq_ignore_ascii_case("tree") {
            let _ = self.mcp_console.execute("tree", &mut self.daemon_state);
            let lines = self.build_full_tree_lines();
            self.open_mcp_popup_window("MASTER CONTROL PROGRAM // SESSION & PANE TREE", lines);
            let sess_len = self.daemon_state.session_tree.nodes.len();
            let pane_len = self.panes.len();
            let sess_name = self.config.history_session.clone();
            self.active_pane_mut().set_status_message(Some(format!(
                "MCP TREE: {sess_len} session(s), {pane_len} pane(s) in '{sess_name}'. END OF LINE."
            )));
            return false;
        }
        match self.mcp_console.execute(trimmed, &mut self.daemon_state) {
            Ok(msg) => {
                let should_quit = !self.daemon_state.running;
                let lines: Vec<String> = msg.lines().map(str::to_string).collect();
                if !should_quit {
                    self.open_mcp_popup_window("MASTER CONTROL PROGRAM // DIRECTIVE", lines);
                }
                self.active_pane_mut().set_status_message(Some(msg));
                should_quit
            }
            Err(err) => {
                self.record_automation_error(err.clone());
                let lines = vec![err.clone()];
                self.open_mcp_popup_window("MASTER CONTROL PROGRAM // ERROR", lines);
                self.active_pane_mut().set_status_message(Some(err));
                false
            }
        }
    }
}
