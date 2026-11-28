//! Image paste into a pane: read the host clipboard image, store it privately, paste its path.
//!
//! This is the one implementation behind every surface: smart `Ctrl+V`, `:paste image`, the
//! leader chord, the palette, and the `clipboard.paste` RPC / MCP call (all via `Action`).

use crate::clipboard::ClipboardSource;
use crate::clipboard_image::{self, ImageDelivery, ImageError, StoredImage};
use crate::pane::CompositePane;

/// Quotes `path` for a shell prompt only when it contains characters a shell would split or expand.
pub fn quote_path_for_paste(path: &str, windows: bool) -> String {
    let plain = !path.is_empty()
        && path.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@' | '%' | ':' | '=' | ',') || (windows && c == '\\'));
    if plain {
        return path.to_string();
    }
    if windows {
        format!("\"{}\"", path.replace('"', ""))
    } else {
        format!("'{}'", path.replace('\'', "'\\''"))
    }
}

/// Reads the clipboard image and pastes its stored path into `pane` (bracketed when the child
/// asked for it, via `handle_paste_event`). Sets the pane status line either way.
pub fn paste_image_into_pane(pane: &mut CompositePane) -> Result<StoredImage, ImageError> {
    let settings = pane.image_paste();
    let result = if settings.enabled { clipboard_image::acquire(&settings.limits) } else { Err(ImageError::Disabled) };
    match &result {
        Ok(img) => {
            let shown = img.path.display().to_string();
            let text = quote_path_for_paste(&shown, cfg!(windows));
            pane.record_clipboard_event(
                ClipboardSource::SystemPaste,
                format!("[image {} {}x{} {} B] {shown}", img.kind.mime(), img.width, img.height, img.bytes),
            );
            let _ = super::handle_paste_event(pane, &text);
            pane.set_status_message(Some(format!(
                "Pasted image path ({} {}x{}, {} KiB{})",
                img.kind.mime(),
                img.width,
                img.height,
                img.bytes.div_ceil(1024),
                if img.reused { ", reused" } else { "" }
            )));
        }
        Err(e) => pane.set_status_message(Some(e.to_string())),
    }
    result
}

/// Smart `Ctrl+V` with an image on the clipboard. Returns `true` when the key was consumed (the
/// path was pasted, or a limit error was shown); `false` means fall back to raw `Ctrl+V` forwarding
/// (delivery `passthrough`, feature disabled, non-image media, or no usable clipboard tool).
pub fn smart_ctrl_v_image(pane: &mut CompositePane, mime: &str) -> bool {
    let s = pane.image_paste();
    if !s.enabled || s.delivery != ImageDelivery::Path || !mime.starts_with("image/") {
        return false;
    }
    match paste_image_into_pane(pane) {
        Ok(_) => true,
        Err(ImageError::NoImage | ImageError::ToolMissing(_) | ImageError::Timeout | ImageError::Disabled) => false,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote_path_for_paste("/run/user/1000/bilterm/img-1-ab.png", false), "/run/user/1000/bilterm/img-1-ab.png");
        assert_eq!(quote_path_for_paste("/tmp/a b/it's.png", false), "'/tmp/a b/it'\\''s.png'");
        assert_eq!(quote_path_for_paste("C:\\Users\\Some One\\x.png", true), "\"C:\\Users\\Some One\\x.png\"");
        assert_eq!(quote_path_for_paste("C:\\Users\\x\\x.png", true), "C:\\Users\\x\\x.png");
        assert_eq!(quote_path_for_paste("/a;rm -rf $HOME", false), "'/a;rm -rf $HOME'");
    }
}
