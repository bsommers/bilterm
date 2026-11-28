//! Per-pane image paste policy (`[clipboard.images]`), pushed by the window manager.

use super::CompositePane;
use crate::clipboard_image::ImagePasteSettings;

impl CompositePane {
    /// Current image paste policy (delivery mode and budgets).
    pub fn image_paste(&self) -> ImagePasteSettings {
        self.image_paste
    }

    pub fn set_image_paste(&mut self, settings: ImagePasteSettings) {
        self.image_paste = settings;
    }
}
