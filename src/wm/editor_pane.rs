//! Editor windows (`EDIT-01..07`): `Action::Editor` execution, window creation, scratchpad/artifact
//! autosave, ContextBus sync, and the close guard. The editor is an ordinary window: it joins the
//! split tree and z-stack, is saved in sessions, and closes through `remove_pane`.

use std::path::PathBuf;

use super::WindowManager;
use crate::api::JsonValue;
use crate::editor::{DEFAULT_ARTIFACT, EditorKind, EditorOp, EditorState, artifact, artifact_bus_key, io};
use crate::error::{BiltermError, Result};
use crate::pane::PaneId;
use crate::wm::agent_mail::AgentMailOp;
use crate::wm::context_bus::{ContextBusError, MAX_VALUE_BYTES};
use crate::wm::split_tree::SplitDirection;

pub const DEFAULT_SCRATCH: &str = "scratch";
/// Longest broadcast note (characters) sent to agents.
const BROADCAST_NOTE_MAX: usize = 512;

impl WindowManager {
    /// Private directory of scratchpad buffers, next to the sessions directory.
    pub(super) fn scratch_dir(&self) -> Result<PathBuf> {
        let sessions = self.resolve_session_dir()?;
        Ok(sessions.parent().map_or_else(|| sessions.join("scratch"), |p| p.join("scratch")))
    }

    /// Private directory of persisted shared artifact buffers, next to the sessions directory.
    pub(super) fn artifact_dir(&self) -> Result<PathBuf> {
        let sessions = self.resolve_session_dir()?;
        Ok(sessions.parent().map_or_else(|| sessions.join("artifacts"), |p| p.join("artifacts")))
    }

    /// Editor windows in id order.
    pub fn editor_pane_ids(&self) -> Vec<PaneId> {
        self.panes.iter().filter(|(_, p)| p.editor().is_some()).map(|(id, _)| *id).collect()
    }

    /// The active window when it is an editor, else the lowest-numbered editor window.
    pub(super) fn editor_target(&self) -> Option<PaneId> {
        let a = self.active_pane_id();
        if self.panes.get(&a).is_some_and(|p| p.editor().is_some()) { Some(a) } else { self.editor_pane_ids().first().copied() }
    }

    /// Opens a new editor window next to the focused one.
    pub fn spawn_editor_pane(&mut self, mut ed: EditorState) -> Result<PaneId> {
        ed.cwd = self.active_pane().resolve_cwd();
        let id = self.split_focused_with(SplitDirection::Vertical, None, true)?;
        let pane = self.panes.get_mut(&id).ok_or_else(|| BiltermError::Config("editor window vanished".into()))?;
        pane.set_editor(ed);
        Ok(id)
    }

    /// Rebuilds the editor of a restored session window (`None` when its backing file is gone).
    pub(super) fn build_saved_editor(&self, saved: &super::SavedEditor) -> Option<EditorState> {
        match saved {
            super::SavedEditor::Scratch { name } => {
                let file = io::scratch_file(&self.scratch_dir().ok()?, name).ok()?;
                Some(EditorState::open_scratch(name, file))
            }
            super::SavedEditor::Artifact { name, render_mode } => {
                // Names are canonicalised to lowercase so `Plan` and `plan` never diverge on
                // case-insensitive filesystems (older sessions may hold mixed case).
                let name = &name.to_ascii_lowercase();
                let file = io::scratch_file(&self.artifact_dir().ok()?, name).ok()?;
                let key = artifact_bus_key(name);
                let bus = self.context_bus.get(&key).map(|e| (e.value, e.revision));
                let mut ed = EditorState::open_artifact(name, file, bus);
                if ed.bus_rev == 0 {
                    let rev = ed.buf.revision();
                    ed.bus_attempts += 1;
                    match self.context_bus.set_if_revision(&key, &ed.buf.text(), None, 0) {
                        Ok(r) => ed.bus_push_ok(r, rev),
                        Err(e) => {
                            ed.bus_push_failed(&e, rev);
                        }
                    }
                }
                ed.render_mode = *render_mode;
                Some(ed)
            }
            super::SavedEditor::File { path, wrap } => EditorState::open_file(std::path::Path::new(path)).ok().map(|mut e| {
                e.wrap = *wrap;
                e
            }),
        }
    }

    /// Session description of an editor window (`None` for terminal windows).
    pub(super) fn saved_editor(p: &crate::pane::CompositePane) -> Option<super::SavedEditor> {
        let ed = p.editor()?;
        match (&ed.kind, &ed.path) {
            (EditorKind::Scratch(n), _) => Some(super::SavedEditor::Scratch { name: n.clone() }),
            (EditorKind::Artifact(n), _) => Some(super::SavedEditor::Artifact { name: n.clone(), render_mode: ed.render_mode }),
            (_, Some(p)) => Some(super::SavedEditor::File { path: p.to_string_lossy().into_owned(), wrap: ed.wrap }),
            _ => None,
        }
    }

    pub(super) fn editor_reply(&mut self, ok: bool, message: String, extra: Vec<(&str, JsonValue)>) {
        let mut entries = vec![
            ("status", JsonValue::String(if ok { "ok" } else { "error" }.into())),
            ("message", JsonValue::String(message.clone())),
        ];
        entries.extend(extra);
        self.set_command_data(JsonValue::obj(entries).to_json_string());
        self.active_pane_mut().set_status_message(Some(message));
    }

    /// Executes `Action::Editor`. Outcomes go to the status bar and the structured command data.
    pub fn apply_editor_op(&mut self, op: EditorOp) {
        if self.is_api_origin() {
            let denied = self.editor_write_denied(&op).or_else(|| self.editor_path_denied(self.editor_target(), &op));
            if let Some(msg) = denied {
                return self.editor_reply(false, msg, vec![]);
            }
        }
        match op {
            EditorOp::Open { path } => self.editor_open(path),
            EditorOp::Scratch { name } => self.editor_scratch(name.as_deref().unwrap_or(DEFAULT_SCRATCH)),
            EditorOp::Artifact { name } => self.editor_artifact(name.as_deref().unwrap_or(DEFAULT_ARTIFACT)),
            EditorOp::Board { name } => self.editor_board(name.as_deref().unwrap_or(DEFAULT_ARTIFACT)),
            EditorOp::Broadcast { note } => self.editor_broadcast(note),
            EditorOp::Preview => self.editor_preview(),
            EditorOp::Close { force } => self.editor_close(force),
            EditorOp::Menu(m) => self.editor_menu_op(m),
            op => {
                let Some(id) = self.editor_target() else {
                    return self.editor_reply(false, "no editor window (:edit [path], :scratch, or :artifact)".into(), vec![]);
                };
                self.editor_run(id, op);
            }
        }
    }

    fn editor_open(&mut self, path: Option<String>) {
        let mut ed = match path {
            None => EditorState::new_file(),
            Some(raw) => {
                let cwd = self.active_pane().resolve_cwd();
                let resolved = match io::resolve_path(&raw, &cwd).and_then(|p| io::check_regular(&p).map(|_| p)) {
                    Ok(p) => p,
                    Err(e) => return self.editor_reply(false, e, vec![]),
                };
                if let Some(id) = self.editor_pane_ids().into_iter().find(|id| self.panes[id].editor().and_then(|e| e.path.as_ref()) == Some(&resolved)) {
                    self.focus_and_raise(id);
                    return self.editor_reply(true, format!("editing {}", resolved.display()), vec![("pane", JsonValue::Number(id as i64))]);
                }
                match EditorState::open_file(&resolved) {
                    Ok(e) => e,
                    Err(e) => return self.editor_reply(false, e, vec![]),
                }
            }
        };
        ed.wrap = true;
        self.finish_open(ed);
    }

    fn finish_open(&mut self, ed: EditorState) {
        let name = ed.display_name();
        match self.spawn_editor_pane(ed) {
            Ok(id) => self.editor_reply(true, format!("editing {name}"), vec![("pane", JsonValue::Number(id as i64))]),
            Err(e) => self.editor_reply(false, format!("editor: {e}"), vec![]),
        }
    }

    fn editor_scratch(&mut self, name: &str) {
        if let Err(e) = crate::secure_fs::validate_safe_name(name) {
            return self.editor_reply(false, format!("scratch name: {e}"), vec![]);
        }
        let existing = self.editor_pane_ids().into_iter().find(|id| matches!(&self.panes[id].editor().map(|e| &e.kind), Some(EditorKind::Scratch(n)) if n == name));
        if let Some(id) = existing {
            self.focus_and_raise(id);
            return self.editor_reply(true, format!("scratchpad {name}"), vec![("pane", JsonValue::Number(id as i64))]);
        }
        let file = match self.scratch_dir().map_err(|e| e.to_string()).and_then(|d| io::scratch_file(&d, name)) {
            Ok(f) => f,
            Err(e) => return self.editor_reply(false, e, vec![]),
        };
        self.finish_open(EditorState::open_scratch(name, file));
    }

    fn editor_artifact(&mut self, name: &str) {
        if let Err(e) = crate::secure_fs::validate_safe_name(name) {
            return self.editor_reply(false, format!("artifact name: {e}"), vec![]);
        }
        // One canonical (lowercase) name per artifact: key and autosave file agree on
        // case-insensitive filesystems.
        let name = name.to_ascii_lowercase();
        let name = name.as_str();
        let existing = self.editor_pane_ids().into_iter().find(|id| {
            matches!(&self.panes[id].editor().map(|e| &e.kind), Some(EditorKind::Artifact(n)) if n.eq_ignore_ascii_case(name))
        });
        if let Some(id) = existing {
            self.focus_and_raise(id);
            let key = artifact_bus_key(name);
            let status = self.panes[&id].editor().map(EditorState::status_json).and_then(|s| JsonValue::parse(&s)).unwrap_or(JsonValue::Null);
            return self.editor_reply(
                true,
                format!("artifact {name}"),
                vec![("pane", JsonValue::Number(id as i64)), ("bus_key", JsonValue::String(key)), ("editor", status)],
            );
        }
        let file = match self.artifact_dir().map_err(|e| e.to_string()).and_then(|d| io::scratch_file(&d, name)) {
            Ok(f) => f,
            Err(e) => return self.editor_reply(false, e, vec![]),
        };
        let key = artifact_bus_key(name);
        let bus_entry = self.context_bus.get(&key).map(|e| (e.value, e.revision));
        let mut ed = EditorState::open_artifact(name, file, bus_entry);
        if ed.bus_rev == 0 {
            let text = ed.buf.text();
            if text.len() > MAX_VALUE_BYTES {
                return self.editor_reply(
                    false,
                    format!("artifact '{name}' is {} bytes; shared artifacts are limited to {MAX_VALUE_BYTES} bytes", text.len()),
                    vec![],
                );
            }
            match self.context_bus.set_if_revision(&key, &text, None, 0) {
                Ok(rev) => ed.bus_push_ok(rev, ed.buf.revision()),
                Err(e) => return self.editor_reply(false, format!("artifact '{name}' not shared: {e}"), vec![]),
            }
        }
        let status = JsonValue::parse(&ed.status_json()).unwrap_or(JsonValue::Null);
        match self.spawn_editor_pane(ed) {
            Ok(id) => self.editor_reply(
                true,
                format!("shared artifact '{name}' (bilterm://context/{key})"),
                vec![("pane", JsonValue::Number(id as i64)), ("bus_key", JsonValue::String(key)), ("editor", status)],
            ),
            Err(e) => self.editor_reply(false, format!("artifact: {e}"), vec![]),
        }
    }

    fn editor_broadcast(&mut self, note: Option<String>) {
        let Some(id) = self.editor_target() else {
            return self.editor_reply(false, "no editor window (:edit [path], :scratch, or :artifact)".into(), vec![]);
        };
        // Only shared artifacts are published: broadcasting a host file or scratchpad would leak
        // its text (and path) to every pane and agent, and file names make colliding bus keys.
        let Some((name, bus_key)) = self.panes.get(&id).and_then(|p| p.editor()).and_then(|ed| Some((ed.display_name(), ed.artifact_bus_key()?))) else {
            return self.editor_reply(false, "broadcast needs a shared artifact window (:artifact [name]); file and scratch buffers are not published".into(), vec![]);
        };
        // An explicit broadcast retries a previously blocked push (still compare-and-set).
        if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
            ed.bus_blocked_rev = None;
        }
        if let Err(e) = self.sync_editor_to_bus_and_preview(id) {
            return self.editor_reply(false, format!("artifact sync failed: {e}"), vec![]);
        }
        let rev = match self.panes.get(&id).and_then(|p| p.editor()) {
            Some(ed) if ed.bus_conflict || ed.bus_error.is_some() => {
                return self.editor_reply(false, ed.bus_error.clone().unwrap_or_else(|| "artifact conflict: :editor revert first".into()), vec![]);
            }
            Some(ed) => ed.bus_rev,
            None => return,
        };
        let loc = format!("bilterm://context/{bus_key}");
        let note = note.map(|n| artifact::sanitize_inline(&n, BROADCAST_NOTE_MAX)).filter(|n| !n.is_empty());
        let body = match note {
            Some(n) => format!("[artifact] {name} rev {rev} updated: {n} — read {loc}"),
            None => format!("[artifact] {name} rev {rev} updated — read {loc}"),
        };
        let mail_result = self.agent_mail_apply(&AgentMailOp::Broadcast {
            from: Some("operator".into()),
            subtree: None,
            body,
            kind: "artifact".into(),
        });
        let (notified, msg) = match mail_result {
            Ok(m) => (true, format!("synced {bus_key} (rev {rev}); {m}")),
            Err(e) => (false, format!("synced {bus_key} (rev {rev}); agent mail: {e}")),
        };
        if let Some(p) = self.panes.get_mut(&id) {
            if let Some(ed) = p.editor_mut() {
                ed.message = Some(msg.clone());
            }
            p.paint_editor();
        }
        self.editor_reply(
            true,
            msg,
            vec![
                ("pane", JsonValue::Number(id as i64)),
                ("bus_key", JsonValue::String(bus_key)),
                ("revision", JsonValue::Number(rev as i64)),
                ("notified_agents", JsonValue::Bool(notified)),
            ],
        );
    }

    fn editor_preview(&mut self) {
        let Some(id) = self.editor_target() else {
            return self.editor_reply(false, "no editor window (:edit [path], :scratch, or :artifact)".into(), vec![]);
        };
        let bus = self.context_bus.clone();
        let persist_dir = self.artifact_dir().ok();
        let (open_url, redacted, existing_preview) = match self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
            Some(ed) => match ed.ensure_preview_server(bus, persist_dir) {
                Ok((full, red)) => (full, red, ed.preview_pane),
                Err(e) => return self.editor_reply(false, e, vec![]),
            },
            None => return,
        };
        if let Some(pid) = existing_preview
            && let Some(bp) = self.panes.get_mut(&pid)
            && let Some(bc) = bp.browser_canvas_mut()
        {
            let _ = bc.reload();
            return self.editor_reply(
                true,
                format!("preview refreshed ({redacted})"),
                vec![("pane", JsonValue::Number(id as i64)), ("preview_pane", JsonValue::Number(pid as i64)), ("url", JsonValue::String(redacted))],
            );
        }
        self.focus_and_raise(id);
        let preview_id = match self.split_focused_with(SplitDirection::Vertical, None, true) {
            Ok(pid) => pid,
            Err(e) => return self.editor_reply(false, format!("preview split: {e}"), vec![]),
        };
        if let Some(bp) = self.panes.get_mut(&preview_id) {
            // Hide the floating command prompt over the preview webpage so the full app is unobstructed.
            bp.set_prompt_mode(crate::pane::PromptMode::RegularTerminal);
            bp.open_browser(&open_url);
        }
        if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
            ed.preview_pane = Some(preview_id);
            ed.message = Some(format!("Artifact Studio live in pane #{preview_id} ({redacted})"));
        }
        if let Some(p) = self.panes.get_mut(&id) {
            p.paint_editor();
        }
        self.focus_and_raise(preview_id);
        self.editor_reply(
            true,
            format!("Artifact Studio live in pane #{preview_id} ({redacted}) — :web external to pop out"),
            vec![("pane", JsonValue::Number(id as i64)), ("preview_pane", JsonValue::Number(preview_id as i64)), ("url", JsonValue::String(redacted))],
        );
    }

    /// Opens or focuses the full-width Shared Artifact Studio webpage in a single clean side pane (`:board [name]`).
    fn editor_board(&mut self, name: &str) {
        let lower = name.to_ascii_lowercase();
        let name = lower.as_str();
        if let Err(e) = crate::secure_fs::validate_safe_name(name) {
            return self.editor_reply(false, format!("board name: {e}"), vec![]);
        }
        let key = artifact_bus_key(name);
        if self.context_bus.get(&key).is_none() {
            let loaded = self
                .artifact_dir()
                .ok()
                .and_then(|d| io::scratch_file(&d, name).ok())
                .and_then(|f| io::load(&f).ok().flatten())
                .map(|l| crate::editor::artifact::sanitize_multiline(&l.text));
            let initial = loaded.unwrap_or_else(|| crate::editor::artifact::artifact_template(name));
            if let Err(e) = self.context_bus.set(&key, &initial, None) {
                return self.editor_reply(false, format!("board sync: {e}"), vec![]);
            }
        }
        let (open_url, redacted) = if let Some(srv) = &self.artifact_studio_server {
            srv.update(name.to_string(), None);
            (srv.url().to_string(), srv.redacted_url().to_string())
        } else {
            let persist_dir = self.artifact_dir().ok();
            match crate::editor::preview::PreviewServer::start(
                self.context_bus.clone(),
                name.to_string(),
                None,
                persist_dir,
            ) {
                Ok(srv) => {
                    let urls = (srv.url().to_string(), srv.redacted_url().to_string());
                    self.artifact_studio_server = Some(srv);
                    urls
                }
                Err(e) => return self.editor_reply(false, format!("board server: {e}"), vec![]),
            }
        };
        if let Some(pid) = self.artifact_studio_pane
            && let Some(bp) = self.panes.get_mut(&pid)
            && bp.browser_canvas().is_some()
        {
            self.focus_and_raise(pid);
            return self.editor_reply(
                true,
                format!("Artifact Studio '{name}' ({redacted})"),
                vec![("pane", JsonValue::Number(pid as i64)), ("bus_key", JsonValue::String(key)), ("url", JsonValue::String(redacted))],
            );
        }
        let board_id = match self.split_focused_with(SplitDirection::Vertical, None, true) {
            Ok(pid) => pid,
            Err(e) => return self.editor_reply(false, format!("board split: {e}"), vec![]),
        };
        if let Some(bp) = self.panes.get_mut(&board_id) {
            bp.set_prompt_mode(crate::pane::PromptMode::RegularTerminal);
            bp.open_browser(&open_url);
        }
        self.artifact_studio_pane = Some(board_id);
        self.focus_and_raise(board_id);
        self.editor_reply(
            true,
            format!("Artifact Studio '{name}' live in pane #{board_id} ({redacted}) — :web external to pop out"),
            vec![("pane", JsonValue::Number(board_id as i64)), ("bus_key", JsonValue::String(key)), ("url", JsonValue::String(redacted))],
        );
    }

    fn editor_close(&mut self, force: bool) {
        let Some(id) = self.editor_target() else {
            return self.editor_reply(false, "no editor window".into(), vec![]);
        };
        if force && let Some(e) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
            e.close_armed = true;
        }
        self.focus_and_raise(id);
        if self.close_focused_pane() {
            self.editor_reply(true, "editor closed".into(), vec![]);
        } else {
            let msg = self.active_pane().status_message().unwrap_or("not closed").to_string();
            self.editor_reply(false, msg, vec![]);
        }
    }

    fn editor_run(&mut self, id: PaneId, op: EditorOp) {
        let Some(pane) = self.panes.get_mut(&id) else { return };
        let Some(ed) = pane.editor_mut() else { return };
        let mut extra: Vec<(&str, JsonValue)> = Vec::new();
        let result: std::result::Result<String, String> = match op {
            EditorOp::Save { force } => {
                if ed.path.is_none() {
                    Err("no file name: use :editor saveas <path>".into())
                } else {
                    ed.save(force).map_err(|e| e.to_string())
                }
            }
            EditorOp::SaveAs { path, force } => io::resolve_path(&path, &ed.cwd).and_then(|p| ed.save_as(&p, force).map_err(|e| e.to_string())),
            EditorOp::Wrap(w) => Ok(format!("soft wrap {}", if ed.set_wrap(w) { "on" } else { "off" })),
            EditorOp::Render(r) => Ok(format!("rendered view {}", if ed.set_render_mode(r) { "on" } else { "off" })),
            EditorOp::Check => ed.toggle_check().map(|c| if c { "checked".into() } else { "unchecked".into() }),
            EditorOp::Find { query, backward } => {
                if ed.find(&query, !backward) { Ok(format!("found: {query}")) } else { Err(format!("not found: {query}")) }
            }
            EditorOp::Undo => if ed.buf.undo() { Ok("undo".into()) } else { Err("nothing to undo".into()) },
            EditorOp::Redo => if ed.buf.redo() { Ok("redo".into()) } else { Err("nothing to redo".into()) },
            EditorOp::Goto(n) => {
                ed.goto_line(n);
                Ok(format!("line {n}"))
            }
            EditorOp::Revert if ed.is_artifact() => match ed.artifact_bus_key().and_then(|k| self.context_bus.get(&k)) {
                Some(e) => {
                    ed.accept_bus(&e.value, e.revision);
                    ed.message = Some("reverted to the shared artifact".into());
                    Ok("reverted to the shared artifact".into())
                }
                None => Err("shared artifact is no longer on the bus".into()),
            },
            EditorOp::Revert => ed.revert(),
            EditorOp::Insert(t) => {
                ed.paste(&t);
                Ok(format!("inserted {} chars", t.chars().count()))
            }
            EditorOp::Text => {
                extra.push(("text", JsonValue::String(ed.buf.text())));
                Ok("text".into())
            }
            EditorOp::Status => Ok("status".into()),
            EditorOp::Open { .. }
            | EditorOp::Scratch { .. }
            | EditorOp::Artifact { .. }
            | EditorOp::Board { .. }
            | EditorOp::Broadcast { .. }
            | EditorOp::Preview
            | EditorOp::Close { .. }
            | EditorOp::Menu(_) => return,
        };
        let sync = self.sync_editor_to_bus_and_preview(id);
        let Some(pane) = self.panes.get_mut(&id) else { return };
        let Some(ed) = pane.editor_mut() else { return };
        extra.push(("editor", JsonValue::parse(&ed.status_json()).unwrap_or(JsonValue::Null)));
        extra.push(("pane", JsonValue::Number(id as i64)));
        pane.paint_editor();
        match (result, sync) {
            (Ok(m), Ok(_)) => self.editor_reply(true, m, extra),
            (Ok(m), Err(e)) => self.editor_reply(true, format!("{m}; {e}"), extra),
            (Err(m), _) => self.editor_reply(false, m, extra),
        }
    }

    /// Routes a key to the active editor window. `None` when the window is not an editor or the
    /// key is not the editor's (window-manager shortcuts keep working).
    pub fn editor_key(&mut self, key: crossterm::event::KeyEvent) -> Option<bool> {
        let id = self.z_stack.top()?;
        self.panes.get(&id)?.editor().filter(|e| e.focused)?;
        if let Some(r) = self.editor_menu_key(id, key) {
            return Some(r);
        }
        let ed = self.panes.get_mut(&id)?.editor_mut()?;
        let consumed = ed.handle_key(key);
        if consumed {
            self.after_editor_change();
        }
        consumed.then_some(true)
    }

    /// Bracketed paste into the focused editor window.
    pub fn editor_paste(&mut self, text: &str) -> bool {
        let Some(pane) = self.z_stack.top().and_then(|id| self.panes.get_mut(&id)) else { return false };
        let Some(ed) = pane.editor_mut().filter(|e| e.focused) else { return false };
        ed.paste(text);
        self.after_editor_change();
        true
    }

    /// Repaints the active editor window and syncs shared artifacts / linked preview panes.
    pub(crate) fn after_editor_change(&mut self) {
        if let Some(id) = self.z_stack.top() {
            let _ = self.sync_editor_to_bus_and_preview(id);
            if let Some(pane) = self.panes.get_mut(&id) {
                pane.paint_editor();
            }
        }
    }

    /// Pushes local edits of an `EditorKind::Artifact` to `ContextBus` with compare-and-set on the
    /// last revision this buffer saw, so a concurrent agent write is never overwritten, and
    /// refreshes any linked preview pane.
    /// `Ok(Some(rev))` pushed, `Ok(None)` nothing to do (clean, not an artifact, or this buffer
    /// revision already failed and is not retried), `Err(message)` the push failed (the message
    /// is also stored on the editor and status).
    pub(crate) fn sync_editor_to_bus_and_preview(&mut self, id: PaneId) -> std::result::Result<Option<u64>, String> {
        let mut reload_preview: Option<PaneId> = None;
        let out = {
            let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) else { return Ok(None) };
            let cur_rev = ed.buf.revision();
            if let Some(key) = ed.artifact_bus_key() {
                if cur_rev == ed.bus_pushed_rev || ed.bus_blocked_rev == Some(cur_rev) {
                    Ok(None)
                } else {
                    let text = ed.buf.text();
                    ed.bus_attempts += 1;
                    let mut res = self.context_bus.set_if_revision(&key, &text, Some(id), ed.bus_rev);
                    if matches!(res, Err(ContextBusError::Conflict { actual: 0, .. })) && ed.bus_rev != 0 {
                        // The entry expired or was deleted: re-seed it from the local text.
                        ed.bus_attempts += 1;
                        res = self.context_bus.set_if_revision(&key, &text, Some(id), 0);
                    }
                    match res {
                        Ok(rev) => {
                            ed.bus_push_ok(rev, cur_rev);
                            if ed.raw_preview_body().is_some() {
                                ed.refresh_preview_body();
                                reload_preview = ed.preview_pane;
                            }
                            Ok(Some(rev))
                        }
                        Err(e) => Err(ed.bus_push_failed(&e, cur_rev)),
                    }
                }
            } else if ed.preview_server.is_some() && cur_rev != ed.preview_pushed_rev {
                if ed.raw_preview_body().is_some() {
                    ed.refresh_preview_body();
                    reload_preview = ed.preview_pane;
                } else {
                    let key = artifact_bus_key(&ed.studio_artifact_name());
                    if let Err(e) = self.context_bus.set(&key, &ed.buf.text(), Some(id)) {
                        ed.message = Some(format!("preview sync: {e}"));
                    }
                    ed.preview_pushed_rev = cur_rev;
                }
                Ok(None)
            } else {
                Ok(None)
            }
        };
        if let Some(pid) = reload_preview
            && let Some(bp) = self.panes.get_mut(&pid)
            && let Some(bc) = bp.browser_canvas_mut()
        {
            let _ = bc.reload();
        }
        out
    }

    /// Toggles keyboard ownership between the editor and the pane's colon prompt (`Ctrl+]`).
    pub fn editor_toggle_focus(&mut self) -> bool {
        let Some(pane) = self.z_stack.top().and_then(|id| self.panes.get_mut(&id)) else { return false };
        let Some(ed) = pane.editor_mut() else { return false };
        ed.focused = !ed.focused;
        let msg = if ed.focused { "Editor focus: keys go to the buffer. Ctrl+] for the prompt" } else { "Editor focus: prompt (Ctrl+] returns to the buffer)" };
        pane.set_status_message(Some(msg.into()));
        pane.paint_editor();
        true
    }

    /// Periodic work: writes changed scratchpads/artifacts (0600, atomic), syncs shared artifacts
    /// bidirectionally with `ContextBus`, and auto-reloads clean file buffers modified on disk.
    pub fn editor_tick(&mut self) -> bool {
        let ids: Vec<PaneId> = self.editor_pane_ids();
        let mut changed = false;
        let mut reload_previews: Vec<PaneId> = Vec::new();
        let mut web_broadcasts: Vec<(String, String)> = self
            .artifact_studio_server
            .as_ref()
            .map_or_else(Vec::new, crate::editor::preview::PreviewServer::take_pending_broadcasts);
        for id in ids {
            let mut needs_paint = false;
            let mut pull_key: Option<String> = None;
            let mut preview_pid: Option<PaneId> = None;
            let mut file_preview_push: Option<(String, String)> = None;
            if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut()) {
                web_broadcasts.extend(ed.take_preview_broadcasts());
                if ed.autosave_due()
                    && let Err(e) = ed.autosave()
                {
                    ed.message = Some(format!("autosave failed: {e}"));
                    needs_paint = true;
                }
                if let Some(key) = ed.artifact_bus_key() {
                    if ed.buf.revision() == ed.bus_pushed_rev {
                        pull_key = Some(key);
                    }
                } else if ed.poll_external_file_change(false) {
                    needs_paint = true;
                    if !ed.external_change && ed.preview_server.is_some() {
                        if ed.raw_preview_body().is_some() {
                            preview_pid = ed.preview_pane;
                        } else {
                            file_preview_push = Some((artifact_bus_key(&ed.studio_artifact_name()), ed.buf.text()));
                        }
                    }
                }
            }
            if let Some((key, text)) = file_preview_push
                && self.context_bus.set(&key, &text, Some(id)).is_ok()
                && let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut())
            {
                ed.preview_pushed_rev = ed.buf.revision();
            }
            // A failed push (conflict, oversize) is reported once through the editor message and
            // not retried until the buffer changes.
            let had_attempts = self.panes.get(&id).and_then(|p| p.editor()).map_or(0, |e| e.bus_attempts);
            let _ = self.sync_editor_to_bus_and_preview(id);
            if self.panes.get(&id).and_then(|p| p.editor()).map_or(0, |e| e.bus_attempts) != had_attempts {
                needs_paint = true;
            }
            if let Some(key) = pull_key
                && let Some(entry) = self.context_bus.get(&key)
                && let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut())
                && ed.sync_from_bus(&entry.value, entry.revision)
            {
                needs_paint = true;
                if ed.raw_preview_body().is_some() {
                    preview_pid = ed.preview_pane;
                }
            }
            if needs_paint {
                if let Some(p) = self.panes.get_mut(&id) {
                    p.paint_editor();
                }
                changed = true;
            }
            if let Some(pid) = preview_pid {
                reload_previews.push(pid);
            }
        }
        for (name, note) in web_broadcasts {
            let key = artifact_bus_key(&name);
            let clean_note = crate::editor::artifact::sanitize_inline(&note, 512);
            let body = if clean_note.is_empty() {
                format!("[artifact] {name} updated — read bilterm://context/{key}")
            } else {
                format!("[artifact] {name}: {clean_note} — read bilterm://context/{key}")
            };
            let _ = self.agent_mail_apply(&AgentMailOp::Broadcast {
                from: Some("operator".into()),
                subtree: None,
                body,
                kind: "artifact".into(),
            });
            changed = true;
        }
        for pid in reload_previews {
            if let Some(bp) = self.panes.get_mut(&pid)
                && let Some(bc) = bp.browser_canvas_mut()
            {
                let _ = bc.reload();
                changed = true;
            }
        }
        changed
    }

    /// Flushes a scratchpad before its window goes away.
    pub(super) fn editor_flush(&mut self, id: PaneId) {
        if let Some(ed) = self.panes.get_mut(&id).and_then(|p| p.editor_mut())
            && ed.autosave_pending()
        {
            let _ = ed.autosave();
        }
    }

    /// Close guard: an unsaved file buffer needs a second close (or `:editor close --force`).
    pub(super) fn editor_allow_close(&mut self, id: PaneId) -> bool {
        let Some(p) = self.panes.get_mut(&id) else { return true };
        let Some(ed) = p.editor_mut() else { return true };
        if !ed.is_dirty() || ed.close_armed {
            return true;
        }
        ed.close_armed = true;
        let msg = format!("{} has unsaved changes: save (Ctrl+S) or close again to discard", ed.display_name());
        p.set_status_message(Some(msg));
        false
    }
}
