//! Highest-Resolution Sub-Cell TrueColor Supersampling & Hardware Pixel Probing (`CANV-02`, `CANV-03`).
//!
//! Provides:
//! 1. **Hardware Terminal Pixel Probing ([`probe_terminal_pixel_size`]) & [`CanvasResolution`] Presets**:
//!    Queries OS `ioctl(TIOCGWINSZ)` (`ws_xpixel × ws_ypixel`) when available and supports
//!    `Auto` (`2x` HiDPI `16×32` px/cell fallback), `1x` (`8×16`), `2x` (`16×32` Retina),
//!    `4x` (`32×64` Ultra-HiDPI), `720p` (`1280×720`), `1080p` (`1920×1080` Full HD),
//!    `4K` (`3840×2160` UHD), and arbitrary `<W>x<H>` pixel dimensions.
//! 2. **2-Color TrueColor Partitioned Sub-Cell Supersampling ([`PixelCanvasBuffer::flush_truecolor_subcells_to_ratatui`])**:
//!    Partitions each cell's `1×2` (`HalfBlock`), `2×2` (`Quadrant`), `2×3` (`Sextant`), or `2×4` (`Octant`)
//!    sub-pixel grid into two optimal 24-bit RGB colors (`fg_rgb` + `bg_rgb`), delivering up to **4× the
//!    spatial resolution of `HalfBlock`** with full 24-bit TrueColor anti-aliased shading.

use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::config::BiltermConfig;
pub use crate::pane::typography::{
    CanvasResolution, probe_terminal_pixel_size, refresh_terminal_pixel_size,
};
use crate::pane::{CanvasRenderBackend, CanvasSubcellMode, CompositePane, PaneCanvasScene};
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::wm::WindowManager;

pub const QUADRANT_CHARS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// Maps a 6-bit (`0..64`) `2×3` sextant mask (`bit 0`=TL, `1`=TR, `2`=ML, `3`=MR, `4`=BL, `5`=BR)
/// to its Unicode Legacy Computing Sextant glyph (`U+1FB00..=U+1FB3B`, `' '` for `0`, `'▌'` for `0b010101`, `'▐'` for `0b101010`, `'█'` for `0x3F`).
pub fn sextant_char_from_mask(mask: u8) -> char {
    match mask & 0x3f {
        0x00 => ' ',
        0x15 => '▌',
        0x2a => '▐',
        0x3f => '█',
        m => {
            let mut idx = u32::from(m) - 1;
            if m > 0x15 {
                idx -= 1;
            }
            if m > 0x2a {
                idx -= 1;
            }
            char::from_u32(0x1fb00 + idx).unwrap_or('█')
        }
    }
}

/// Maps an 8-bit (`0..=255`) `2×4` sub-pixel mask to a Unicode Braille glyph (`U+2800..=U+28FF`).
#[inline]
pub fn braille_char_from_mask(mask: u8) -> char {
    char::from_u32(0x2800 + u32::from(mask)).unwrap_or('⠀')
}

/// Maps an 8-bit (`0..=255`) `2×4` sub-pixel mask to a Unicode 16.0 Legacy Computing Octant
/// (`U+1CD00..U+1CDE5`) or full/half block (`CANV-03`).
#[inline]
pub fn subcell_mask_to_octant(mask: u8) -> char {
    match mask {
        0x00 => ' ',
        0x03 => '▔',
        0x0f => '▀',
        0xf0 => '▄',
        0xc0 => '▁',
        0x55 => '▌',
        0xaa => '▐',
        0xff => '█',
        m => braille_char_from_mask(m),
    }
}

impl PixelCanvasBuffer {
    /// Area-averages the RGBA pixels inside rectangle `[x0..x1, y0..y1]` for anti-aliased supersampling.
    fn sample_box(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> [u8; 4] {
        let xe = x1.max(x0 + 1).min(self.width);
        let ye = y1.max(y0 + 1).min(self.height);
        let mut r_sum = 0u32;
        let mut g_sum = 0u32;
        let mut b_sum = 0u32;
        let mut a_sum = 0u32;
        let mut opaque_cnt = 0u32;
        let mut total_cnt = 0u32;
        for y in y0.min(self.height.saturating_sub(1))..ye {
            for x in x0.min(self.width.saturating_sub(1))..xe {
                let px = self.pixel_at(x, y);
                total_cnt += 1;
                a_sum += u32::from(px[3]);
                if px[3] > 16 {
                    r_sum += u32::from(px[0]);
                    g_sum += u32::from(px[1]);
                    b_sum += u32::from(px[2]);
                    opaque_cnt += 1;
                }
            }
        }
        if opaque_cnt == 0 || total_cnt == 0 {
            return [0, 0, 0, 0];
        }
        [
            (r_sum / opaque_cnt) as u8,
            (g_sum / opaque_cnt) as u8,
            (b_sum / opaque_cnt) as u8,
            (a_sum / total_cnt) as u8,
        ]
    }

    /// Blits this high-DPI `PixelCanvasBuffer` into Ratatui `buf` using 2-Color TrueColor Partitioned
    /// Supersampling across `HalfBlock` (`1×2`), `Quadrant` (`2×2`), `Sextant` (`2×3`), or `Octant` (`2×4`) (`CANV-03`).
    pub fn flush_truecolor_subcells_to_ratatui(
        &self,
        mode: CanvasSubcellMode,
        canvas: Rect,
        buf: &mut Buffer,
    ) {
        if mode == CanvasSubcellMode::HalfBlock {
            self.flush_halfblocks_to_ratatui(canvas, buf);
            return;
        }
        let (sub_w, sub_h) = mode.subcell_grid();
        let sw = u32::from(sub_w.max(1));
        let sh = u32::from(sub_h.max(1));
        let total_sub_w = u32::from(canvas.width.max(1)) * sw;
        let total_sub_h = u32::from(canvas.height.max(1)) * sh;

        for r in 0..canvas.height {
            for c in 0..canvas.width {
                let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r)) else {
                    continue;
                };
                if cell.symbol() != " " {
                    continue;
                }
                let mut samples = [[0u8; 4]; 8];
                let mut n = 0usize;
                let mut opaque_mask = 0u8;
                for sy in 0..sh {
                    let py0 = ((u32::from(r) * sh + sy) * self.height) / total_sub_h;
                    let py1 = ((u32::from(r) * sh + sy + 1) * self.height) / total_sub_h;
                    for sx in 0..sw {
                        let px0 = ((u32::from(c) * sw + sx) * self.width) / total_sub_w;
                        let px1 = ((u32::from(c) * sw + sx + 1) * self.width) / total_sub_w;
                        let s = self.sample_box(px0, py0, px1, py1);
                        samples[n] = s;
                        if s[3] > 24 {
                            opaque_mask |= 1 << n;
                        }
                        n += 1;
                    }
                }
                if opaque_mask == 0 {
                    continue;
                }
                let all_bits = ((1u16 << n) - 1) as u8;
                if opaque_mask == all_bits {
                    // All sub-pixels in this cell are opaque: partition into 2 optimal TrueColor clusters (fg + bg)
                    let mut min_lum = u32::MAX;
                    let mut max_lum = 0u32;
                    let mut c_dark = [0u8; 3];
                    let mut c_bright = [0u8; 3];
                    for s in &samples[..n] {
                        let lum =
                            u32::from(s[0]) * 3 + u32::from(s[1]) * 6 + u32::from(s[2]);
                        if lum < min_lum {
                            min_lum = lum;
                            c_dark = [s[0], s[1], s[2]];
                        }
                        if lum >= max_lum {
                            max_lum = lum;
                            c_bright = [s[0], s[1], s[2]];
                        }
                    }
                    let mut fg_mask = 0u8;
                    for (idx, s) in samples[..n].iter().enumerate() {
                        let d_dark = (i32::from(s[0]) - i32::from(c_dark[0])).pow(2)
                            + (i32::from(s[1]) - i32::from(c_dark[1])).pow(2)
                            + (i32::from(s[2]) - i32::from(c_dark[2])).pow(2);
                        let d_bright = (i32::from(s[0]) - i32::from(c_bright[0])).pow(2)
                            + (i32::from(s[1]) - i32::from(c_bright[1])).pow(2)
                            + (i32::from(s[2]) - i32::from(c_bright[2])).pow(2);
                        if d_bright <= d_dark {
                            fg_mask |= 1 << idx;
                        }
                    }
                    let ch = mask_to_mode_char(mode, fg_mask);
                    let mut sym = [0u8; 4];
                    cell.set_symbol(ch.encode_utf8(&mut sym));
                    cell.set_fg(Color::Rgb(c_bright[0], c_bright[1], c_bright[2]));
                    cell.set_bg(Color::Rgb(c_dark[0], c_dark[1], c_dark[2]));
                } else {
                    // Partial coverage: average opaque sub-pixels as foreground
                    let mut r_acc = 0u32;
                    let mut g_acc = 0u32;
                    let mut b_acc = 0u32;
                    let mut cnt = 0u32;
                    for (idx, s) in samples[..n].iter().enumerate() {
                        if (opaque_mask & (1 << idx)) != 0 {
                            r_acc += u32::from(s[0]);
                            g_acc += u32::from(s[1]);
                            b_acc += u32::from(s[2]);
                            cnt += 1;
                        }
                    }
                    let cnt = cnt.max(1);
                    let ch = mask_to_mode_char(mode, opaque_mask);
                    let mut sym = [0u8; 4];
                    cell.set_symbol(ch.encode_utf8(&mut sym));
                    cell.set_fg(Color::Rgb(
                        (r_acc / cnt) as u8,
                        (g_acc / cnt) as u8,
                        (b_acc / cnt) as u8,
                    ));
                }
            }
        }
    }
}

fn mask_to_mode_char(mode: CanvasSubcellMode, mask: u8) -> char {
    match mode {
        CanvasSubcellMode::Quadrant => QUADRANT_CHARS[usize::from(mask & 0x0f)],
        CanvasSubcellMode::Sextant => sextant_char_from_mask(mask & 0x3f),
        CanvasSubcellMode::Octant => subcell_mask_to_octant(mask),
        CanvasSubcellMode::Braille => braille_char_from_mask(mask),
        CanvasSubcellMode::HalfBlock => '▀',
    }
}

impl CompositePane {
    pub fn canvas_scene(&self) -> &PaneCanvasScene {
        &self.canvas_scene
    }

    pub fn canvas_scene_mut(&mut self) -> &mut PaneCanvasScene {
        &mut self.canvas_scene
    }

    fn write_canvas_export_bytes(&self, dst: &Path, display_path: &str) -> std::result::Result<String, String> {
        if let Ok(meta) = std::fs::symlink_metadata(dst)
            && meta.file_type().is_symlink()
        {
            return Err("Invalid export path (refusing to overwrite symlink)".to_string());
        }
        let cfg = BiltermConfig::default();
        let (canvas_rows, canvas_cols) = self.vt.size();
        let cols = canvas_cols.max(1);
        let rows = canvas_rows.max(1);
        // Export intentionally uses the full resolved resolution (not the on-screen render cap).
        let (px_c, px_r) = self.canvas_scene.resolution.resolve_px_per_cell(cols, rows);
        let buf = PixelCanvasBuffer::render_pane_scene(self, &cfg, cols, rows, px_c, px_r);
        let payload = if display_path.ends_with(".svg") {
            buf.export_svg("Bilterm High-Res Canvas").into_bytes()
        } else {
            buf.export_ppm()
        };
        #[cfg(unix)]
        let write_res = crate::secure_fs::write_private_file(dst, &payload);
        #[cfg(not(unix))]
        let write_res = std::fs::write(dst, &payload);
        match write_res {
            Ok(()) => Ok(format!(
                "CANVAS EXPORT: wrote {}x{} high-res image to {display_path}",
                buf.width, buf.height
            )),
            Err(e) => Err(format!("CANVAS EXPORT ERROR: {e}")),
        }
    }

    /// Executes a `:canvas <subcommand>` on this pane (`TEK-01`, `CANV-01`, `CANV-02`, `CANV-03`).
    pub fn execute_canvas_command(&mut self, cmd: &str) -> std::result::Result<String, String> {
        self.execute_canvas_command_in(cmd, None)
    }

    /// Like [`Self::execute_canvas_command`], but when `confine` is set `export` accepts only a plain
    /// file name and writes it inside that directory (`SEC-R01`, `P0.1`).
    pub fn execute_canvas_command_in(
        &mut self,
        cmd: &str,
        confine: Option<&Path>,
    ) -> std::result::Result<String, String> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let res = match parts.first().copied() {
            Some("export") => {
                let Some(path) = parts.get(1).copied() else {
                    return Err("Usage: :canvas export <file.ppm|file.svg>".to_string());
                };
                if let Some(dir) = confine {
                    crate::secure_fs::validate_plain_file_name(path)?;
                    let dst = dir.join(path);
                    self.write_canvas_export_bytes(&dst, &dst.display().to_string())
                } else if path.contains("..") || path.contains('\0') {
                    return Err("Invalid export path (path traversal rejected)".to_string());
                } else {
                    self.write_canvas_export_bytes(Path::new(path), path)
                }
            }
            Some("encode") => {
                let req = parts
                    .get(1)
                    .and_then(|s| CanvasRenderBackend::from_name(s))
                    .unwrap_or(self.canvas_scene.render_backend);
                let eff = req.resolve_auto();
                let in_tmux = std::env::var("TMUX").is_ok_and(|v| !v.is_empty());
                let cfg = BiltermConfig::default();
                let (canvas_rows, canvas_cols) = self.vt.size();
                let cols = (canvas_cols / 2).max(40);
                let rows = (canvas_rows / 2).max(12);
                let (px_c, px_r) = self.canvas_scene.resolution.resolve_px_per_cell(cols, rows);
                let buf = PixelCanvasBuffer::render_pane_scene(self, &cfg, cols, rows, px_c, px_r);
                let stream = match eff {
                    CanvasRenderBackend::Sixel => buf.encode_sixel_dcs(in_tmux),
                    CanvasRenderBackend::ITerm2 => buf.encode_iterm2_osc1337(cols, rows, in_tmux),
                    _ => buf.encode_kitty_apc(cols, rows, in_tmux),
                };
                Ok(format!(
                    "CANVAS ENCODE ({eff:?}, {}x{}px, tmux_passthrough={in_tmux}): {} bytes ready",
                    buf.width,
                    buf.height,
                    stream.len()
                ))
            }
            _ => {
                let (scene, vt) = (&mut self.canvas_scene, self.vt.tektronix_mut());
                scene.execute_command(vt, cmd)
            }
        };
        match res {
            Ok(msg) => {
                self.set_status_message(Some(msg.clone()));
                Ok(msg)
            }
            Err(err) => {
                self.set_status_message(Some(err.clone()));
                Err(err)
            }
        }
    }
}

impl WindowManager {
    /// Executes a `:canvas <subcommand>` on the active pane (`TEK-01`, `CANV-01`).
    pub fn execute_canvas_action(&mut self, subcommand: &str) -> String {
        let confine = self.automation_confine_dir().map(std::path::Path::to_path_buf);
        match self
            .active_pane_mut()
            .execute_canvas_command_in(subcommand, confine.as_deref())
        {
            Ok(msg) => msg,
            Err(msg) => {
                self.record_automation_error(msg.clone());
                msg
            }
        }
    }
}
