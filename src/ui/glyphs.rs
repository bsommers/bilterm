use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

/// Configurable glyph pack preset (`UI-04`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GlyphPack {
    /// Standard Unicode box-drawing + geometric dingbats (`╔═╗`, `[▲][▼]`, `[▪]`, `[↕]`, `◢`).
    #[default]
    Unicode,
    /// Classic IBM PC Code Page 437 symbols (`♦`, `▲`, `▼`, `■`, `↕`, `╬`).
    Cp437,
    /// Powerline + Nerd Font glyphs (``, ``, `│`, `[▲][▼]`).
    NerdFont,
    /// Pure 7-bit ASCII fallback (`[^][v]`, `[x]`, `[+]`, `/`).
    Ascii,
}

impl GlyphPack {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "unicode" | "default" => Some(Self::Unicode),
            "cp437" | "dos" | "ibm" => Some(Self::Cp437),
            "nerd" | "nerd-font" | "nerdfont" | "powerline" => Some(Self::NerdFont),
            "ascii" => Some(Self::Ascii),
            _ => None,
        }
    }

    pub fn glyphs(self) -> GlyphSet {
        match self {
            Self::Unicode => GlyphSet {
                pin_top_bottom: "[▲][▼]",
                close_badge: "[▪]",
                zoom_badge: "[↕]",
                resize_corner: "◢",
                oia_separator: "│",
                prompt_prefix: "==> ",
                prompt_continuation: "... ",
                status_ready_icon: "●",
                status_run_icon: "⟳",
                status_lock_icon: "♦",
                status_alt_icon: "▣",
                status_scroll_icon: "⇳",
                output_len_icon: "≣",
                cursor_pos_icon: "⌖",
            },
            Self::Cp437 => GlyphSet {
                pin_top_bottom: "[▲][▼]",
                close_badge: "[■]",
                zoom_badge: "[↕]",
                resize_corner: "╬",
                oia_separator: "║",
                prompt_prefix: "♦=> ",
                prompt_continuation: "··· ",
                status_ready_icon: "♦",
                status_run_icon: "►",
                status_lock_icon: "‼",
                status_alt_icon: "■",
                status_scroll_icon: "↕",
                output_len_icon: "≡",
                cursor_pos_icon: "┼",
            },
            Self::NerdFont => GlyphSet {
                pin_top_bottom: "[▲][▼]",
                close_badge: "[▪]",
                zoom_badge: "[↕]",
                resize_corner: "◢",
                oia_separator: "",
                prompt_prefix: "❯=> ",
                prompt_continuation: "... ",
                status_ready_icon: "✔",
                status_run_icon: "⚙",
                status_lock_icon: "",
                status_alt_icon: "",
                status_scroll_icon: "⇳",
                output_len_icon: "☰",
                cursor_pos_icon: "⌖",
            },
            Self::Ascii => GlyphSet {
                pin_top_bottom: "[^][v]",
                close_badge: "[x]",
                zoom_badge: "[+]",
                resize_corner: "/",
                oia_separator: "|",
                prompt_prefix: "==> ",
                prompt_continuation: "... ",
                status_ready_icon: "*",
                status_run_icon: "~",
                status_lock_icon: "!",
                status_alt_icon: "#",
                status_scroll_icon: "^",
                output_len_icon: "=",
                cursor_pos_icon: "@",
            },
        }
    }
}

/// Resolved glyph tokens guaranteed to maintain deterministic terminal cell widths (`UI-04`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphSet {
    pub pin_top_bottom: &'static str,
    pub close_badge: &'static str,
    pub zoom_badge: &'static str,
    pub resize_corner: &'static str,
    pub oia_separator: &'static str,
    pub prompt_prefix: &'static str,
    pub prompt_continuation: &'static str,
    pub status_ready_icon: &'static str,
    pub status_run_icon: &'static str,
    pub status_lock_icon: &'static str,
    pub status_alt_icon: &'static str,
    pub status_scroll_icon: &'static str,
    pub output_len_icon: &'static str,
    pub cursor_pos_icon: &'static str,
}

impl Default for GlyphSet {
    fn default() -> Self {
        GlyphPack::default().glyphs()
    }
}

impl GlyphSet {
    /// Verifies that every badge in the `GlyphSet` has the exact expected column width.
    pub fn validate_cell_widths(&self) -> bool {
        UnicodeWidthStr::width(self.pin_top_bottom) == 6
            && UnicodeWidthStr::width(self.close_badge) == 3
            && UnicodeWidthStr::width(self.zoom_badge) == 3
            && UnicodeWidthStr::width(self.resize_corner) == 1
            && UnicodeWidthStr::width(self.oia_separator) == 1
            && UnicodeWidthStr::width(self.prompt_prefix) == 4
            && UnicodeWidthStr::width(self.prompt_continuation) == 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_glyph_packs_preserve_strict_cell_width_alignment() {
        for pack in [
            GlyphPack::Unicode,
            GlyphPack::Cp437,
            GlyphPack::NerdFont,
            GlyphPack::Ascii,
        ] {
            assert!(
                pack.glyphs().validate_cell_widths(),
                "GlyphPack {pack:?} failed cell-width validation"
            );
        }
    }
}
