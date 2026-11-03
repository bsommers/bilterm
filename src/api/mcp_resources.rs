//! MCP read-only resources (`resources/list`, `resources/read`): state, config, commands, panes.

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;

type McpResult = std::result::Result<JsonValue, (i64, String)>;

fn s(v: impl Into<String>) -> JsonValue {
    JsonValue::String(v.into())
}

fn resource(uri: &str, name: &str, desc: &str) -> JsonValue {
    JsonValue::obj([
        ("uri", s(uri)),
        ("name", s(name)),
        ("description", s(desc)),
        ("mimeType", s("application/json")),
    ])
}

impl AutomationControlPlane {
    pub fn mcp_resources_list(&self) -> JsonValue {
        let mut list = vec![
            resource(
                "bilterm://state",
                "Workspace state",
                "Workspace status plus appearance profile, display mode and drawer state.",
            ),
            resource(
                "bilterm://config",
                "Effective config",
                "Effective non-secret configuration values (read-only).",
            ),
            resource(
                "bilterm://commands",
                "Colon command catalog",
                "All colon commands accepted by bilterm_colon_command.",
            ),
        ];
        for &id in self.wm.panes().keys() {
            list.push(resource(
                &format!("bilterm://pane/{id}"),
                &format!("Pane #{id}"),
                "Visible lines, cursor, command blocks, hints and notifications for the pane.",
            ));
        }
        JsonValue::obj([("resources", JsonValue::Array(list))])
    }

    pub(crate) fn mcp_resources_read(&mut self, params: &JsonValue) -> McpResult {
        let uri = params
            .get("uri")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| (-32602, "Invalid params: missing 'uri'".to_string()))?;
        let body = match uri {
            "bilterm://state" => self.mcp_state_resource(),
            "bilterm://config" => self.mcp_config_resource(),
            "bilterm://commands" => Self::mcp_commands_resource(),
            _ => {
                let id = uri
                    .strip_prefix("bilterm://pane/")
                    .and_then(|r| r.parse::<usize>().ok())
                    .ok_or_else(|| {
                        (-32602, format!("Invalid params: unknown resource URI: {uri}"))
                    })?;
                if self.wm.pane(id).is_none() {
                    return Err((-32602, format!("Invalid params: pane #{id} not found")));
                }
                let args = JsonValue::obj([("pane_id", JsonValue::Number(id as i64))]);
                self.dispatch_method("pane.read", &args)
                    .map_err(|e| (-32603, format!("Internal error: {e}")))?
            }
        };
        Ok(JsonValue::obj([(
            "contents",
            JsonValue::Array(vec![JsonValue::obj([
                ("uri", s(uri)),
                ("mimeType", s("application/json")),
                ("text", s(body.to_json_string())),
            ])]),
        )]))
    }

    fn mcp_state_resource(&self) -> JsonValue {
        let cfg = self.wm.config();
        let dc = self.wm.drawer_config();
        let drawer = JsonValue::obj([
            ("open", JsonValue::Bool(self.wm.drawer_is_open())),
            (
                "visible",
                JsonValue::Bool(self.wm.is_drawer_effectively_visible()),
            ),
            ("edge", s(format!("{:?}", dc.edge).to_lowercase())),
            ("width_cols", JsonValue::Number(i64::from(dc.width_cols))),
            ("height_rows", JsonValue::Number(i64::from(dc.height_rows))),
        ]);
        JsonValue::obj([
            ("workspace", self.workspace_status_json()),
            (
                "appearance_profile",
                s(format!("{:?}", crate::ui::design::AppearanceProfile::resolve(cfg)).to_lowercase()),
            ),
            (
                "display_mode",
                s(crate::ui::shell::effective_display_mode(cfg).label()),
            ),
            ("drawer", drawer),
        ])
    }

    fn mcp_config_resource(&self) -> JsonValue {
        let c = self.wm.config();
        let b = JsonValue::Bool;
        JsonValue::obj([
            ("theme", s(format!("{:?}", c.theme))),
            ("snazzy_mode", b(c.snazzy_mode)),
            ("glass_mode", b(c.glass_mode)),
            ("modern_mode", b(c.modern_mode)),
            ("liquid_glass", b(c.liquid_glass)),
            ("crt_scanlines", b(c.crt_scanlines)),
            ("show_top_menubar", b(c.show_top_menubar)),
            ("show_dos_fkey_bar", b(c.show_dos_fkey_bar)),
            ("copy_on_select", b(c.copy_on_select)),
            ("history_enabled", b(c.history_enabled)),
            ("layout_autosave", b(c.layout_autosave)),
            ("leader_key", s(c.leader_key.clone())),
            ("default_prompt_anchor", s(c.default_prompt_anchor.clone())),
            (
                "default_prompt_height",
                JsonValue::Number(i64::from(c.default_prompt_height)),
            ),
            ("active_layout_profile", s(c.active_layout_profile.clone())),
            (
                "appearance_profile",
                s(format!("{:?}", crate::ui::design::AppearanceProfile::resolve(c)).to_lowercase()),
            ),
            (
                "display_mode",
                s(crate::ui::shell::effective_display_mode(c).label()),
            ),
        ])
    }

    fn mcp_commands_resource() -> JsonValue {
        let cmds: Vec<JsonValue> = crate::pane::completion::colon_command_names()
            .into_iter()
            .map(|n| JsonValue::obj([("name", s(n))]))
            .collect();
        JsonValue::obj([
            ("count", JsonValue::Number(cmds.len() as i64)),
            ("commands", JsonValue::Array(cmds)),
        ])
    }
}
