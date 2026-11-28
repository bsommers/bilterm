//! `:paste image` on the window manager: gate, paste into the active pane, report structured data.

use crate::api::JsonValue;
use crate::wm::WindowManager;

impl WindowManager {
    /// Applies `ClipboardWindowOp::PasteImage` to the active pane.
    ///
    /// API-originated calls are refused unless `[api] allow_clipboard_image = true`. Success stores
    /// `{path, mime, bytes, width, height, reused}` as command data; failures are recorded as
    /// automation errors so RPC/MCP callers see them.
    pub(crate) fn paste_clipboard_image(&mut self) {
        let settings = self.config.clipboard.images.settings();
        if self.is_api_origin()
            && let Some(why) = self.config_write_denied(&crate::action::Action::ClipboardWindow(
                crate::wm::clipboard_window::ClipboardWindowOp::PasteImage,
            ))
        {
            self.record_automation_error(why.clone());
            self.active_pane_mut().set_status_message(Some(why));
            return;
        }
        self.active_pane_mut().set_image_paste(settings);
        match crate::input::paste_image_into_pane(self.active_pane_mut()) {
            Ok(img) => {
                let data = JsonValue::obj([
                    ("path", JsonValue::String(img.path.display().to_string())),
                    ("mime", JsonValue::String(img.kind.mime().to_string())),
                    ("bytes", JsonValue::Number(img.bytes as i64)),
                    ("width", JsonValue::Number(i64::from(img.width))),
                    ("height", JsonValue::Number(i64::from(img.height))),
                    ("reused", JsonValue::Bool(img.reused)),
                ]);
                self.set_command_data(data.to_json_string());
                self.sync_clipboard_records();
            }
            Err(e) => self.record_automation_error(e.to_string()),
        }
    }
}
