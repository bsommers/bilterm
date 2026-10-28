//! JSON-RPC 2.0 domain handler extensions for `AutomationControlPlane` (`FONT-01`, `FONT-02`, `CANV-01..03`, `SEC-R01`).
//!
//! Keeps JSON-RPC serialization and snapshot-directory confinement inside `crate::api` so
//! `crate::ui` never depends on `crate::api::JsonValue` (`P1.1`).

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;

impl AutomationControlPlane {
    /// Applies `action` through `WindowManager::apply` (via `apply_automation`), confining file writes to
    /// the snapshot dir under `SnapshotPolicy::Confined` (`P0.1`, `SEC-R01`).
    pub(super) fn apply_with_policy(
        &mut self,
        action: crate::action::Action,
        policy: super::rpc::SnapshotPolicy,
    ) -> std::result::Result<crate::input::AppAction, String> {
        let confine = match policy {
            super::rpc::SnapshotPolicy::Confined => Some(self.resolved_snapshot_dir()?),
            super::rpc::SnapshotPolicy::Trusted => None,
        };
        self.wm.apply_automation(action, confine.as_deref())
    }

    /// Handles the JSON-RPC `workspace.fontsize` method (`FONT-01`, `FONT-02`).
    pub fn handle_rpc_fontsize(
        &mut self,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        if let Some(action) = params.get("action").and_then(JsonValue::as_str) {
            match action.trim().to_ascii_lowercase().as_str() {
                "in" | "up" | "+" | "zoom_in" => {
                    self.wm.font_zoom_in();
                }
                "out" | "down" | "-" | "zoom_out" => {
                    self.wm.font_zoom_out();
                }
                "reset" | "0" | "default" => {
                    self.wm.font_zoom_reset();
                }
                other => {
                    if let Ok(val) = other
                        .trim_end_matches("pt")
                        .trim_end_matches('%')
                        .parse::<u16>()
                    {
                        self.wm.font_set_size(val);
                    } else {
                        return Err(format!("invalid workspace.fontsize action: {other}"));
                    }
                }
            }
        }
        if let Some(size) = params.get("size").and_then(JsonValue::as_usize) {
            let size = u16::try_from(size).map_err(|_| format!("size out of range: {size}"))?;
            self.wm.font_set_size(size);
        }
        if let Some(scale) = params.get("scale_pct").and_then(JsonValue::as_usize) {
            let scale = u16::try_from(scale).map_err(|_| format!("scale_pct out of range: {scale}"))?;
            self.wm.font_set_size(scale.clamp(50, 300));
        }
        if let Some(cmd) = params.get("command").and_then(JsonValue::as_str) {
            self.wm.execute_pane_font_command(cmd)?;
        }
        let (cw, ch) = self.wm.config().scaled_cell_dimensions_px();
        let pane_font = self.wm.active_pane().font_options();
        Ok(JsonValue::obj([
            (
                "font_size_pt",
                JsonValue::Number(i64::from(self.wm.config().scaled_font_size_pt())),
            ),
            (
                "base_font_size_pt",
                JsonValue::Number(i64::from(self.wm.config().font_size_pt)),
            ),
            (
                "font_scale_pct",
                JsonValue::Number(i64::from(self.wm.config().font_scale_pct)),
            ),
            ("cell_width_px", JsonValue::Number(i64::from(cw))),
            ("cell_height_px", JsonValue::Number(i64::from(ch))),
            (
                "host_font_passthrough",
                JsonValue::Bool(self.wm.config().host_font_passthrough),
            ),
            (
                "pane_font_family",
                JsonValue::String(pane_font.family.display_name().to_string()),
            ),
            (
                "pane_font_size_pt",
                JsonValue::Number(i64::from(pane_font.effective_size_pt(self.wm.config()))),
            ),
            (
                "pane_font_scale_pct",
                JsonValue::Number(i64::from(pane_font.effective_scale_pct(self.wm.config()))),
            ),
            ("pane_font_bold", JsonValue::Bool(pane_font.bold)),
            ("pane_font_italic", JsonValue::Bool(pane_font.italic)),
            (
                "pane_font_double_width",
                JsonValue::Bool(pane_font.double_width),
            ),
        ]))
    }

    /// Handles JSON-RPC `canvas.command` and `canvas.status` requests (`TEK-01`, `CANV-01..03`).
    pub fn handle_rpc_canvas(
        &mut self,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        if let Some(cmd) = params.get("command").and_then(JsonValue::as_str) {
            self.apply_with_policy(
                crate::action::Action::CanvasCommand(cmd.to_string()),
                super::rpc::SnapshotPolicy::Confined,
            )?;
        }
        let pane = self.wm.active_pane();
        let tek = pane.vt().tektronix();
        let scene = pane.canvas_scene();
        let collisions = scene.detect_sprite_collisions();
        Ok(JsonValue::obj([
            ("tektronix_active", JsonValue::Bool(tek.active)),
            (
                "tektronix_segments",
                JsonValue::Number(tek.segments.len() as i64),
            ),
            (
                "tektronix_decaying_segments",
                JsonValue::Number(tek.decaying_segments.len() as i64),
            ),
            ("canvas_active", JsonValue::Bool(scene.active)),
            (
                "render_backend",
                JsonValue::String(format!("{:?}", scene.render_backend)),
            ),
            (
                "effective_backend",
                JsonValue::String(format!("{:?}", scene.render_backend.resolve_auto())),
            ),
            (
                "subcell_mode",
                JsonValue::String(format!("{:?}", scene.subcell_mode)),
            ),
            (
                "primitives",
                JsonValue::Number(scene.primitives.len() as i64),
            ),
            ("sprites", JsonValue::Number(scene.sprites.len() as i64)),
            ("plots", JsonValue::Number(scene.plots.len() as i64)),
            ("has_image", JsonValue::Bool(scene.image_view.is_some())),
            ("sprite_collisions", JsonValue::Number(collisions.len() as i64)),
        ]))
    }

    /// Handles JSON-RPC `session.save` requests (`DAEM-04`, `ECO-02`).
    pub fn handle_rpc_session_save(
        &mut self,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        let name = params
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("default");
        let path = self.wm.save_session(name).map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([
            ("saved", JsonValue::Bool(true)),
            ("session", JsonValue::String(name.to_string())),
            ("path", JsonValue::String(path.display().to_string())),
            ("panes", JsonValue::Number(self.wm.pane_count() as i64)),
        ]))
    }

    /// Handles JSON-RPC `session.restore` requests (`DAEM-04`, `ECO-02`).
    pub fn handle_rpc_session_restore(
        &mut self,
        params: &JsonValue,
    ) -> std::result::Result<JsonValue, String> {
        let name = params
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("default");
        let count = self.wm.restore_session(name).map_err(|e| e.to_string())?;
        Ok(JsonValue::obj([
            ("restored", JsonValue::Bool(true)),
            ("session", JsonValue::String(name.to_string())),
            ("panes", JsonValue::Number(count as i64)),
        ]))
    }
}
