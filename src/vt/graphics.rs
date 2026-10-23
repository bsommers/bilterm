use super::VtParser;

/// Terminal graphics protocol family (`GFX-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicProtocol {
    /// Kitty Graphics Protocol (`APC G`, `\x1b_G...\x1b\\`).
    Kitty,
    /// DEC Sixel Graphics (`DCS q`, `\x1bP...q...\x1b\\`).
    Sixel,
}

/// Decoded RGBA image placement anchored to a terminal cell region (`GFX-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalGraphicPlacement {
    pub id: u32,
    pub protocol: GraphicProtocol,
    pub row: u16,
    pub col: u16,
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
    pub rgba_bytes: Vec<u8>,
}

impl VtParser {
    /// Returns all active Kitty Graphics (`APC G`) and DEC Sixel (`DCS q`) placements on the canvas (`GFX-01`).
    pub fn graphic_placements(&self) -> &[TerminalGraphicPlacement] {
        &self.graphic_placements
    }

    /// Clears all active graphics placements (`GFX-01`).
    pub fn clear_graphics(&mut self) {
        self.graphic_placements.clear();
    }
}

pub(super) fn strip_apc_and_dcs_envelopes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        let next_apc = rest.find("\x1b_");
        let next_dcs = rest.find("\x1bP");
        let next_env = match (next_apc, next_dcs) {
            (Some(a), Some(d)) => Some(a.min(d)),
            (Some(a), None) => Some(a),
            (None, Some(d)) => Some(d),
            (None, None) => None,
        };
        if let Some(pos) = next_env {
            out.push_str(&rest[..pos]);
            let after_start = &rest[pos + 2..];
            if let Some(end_st) = after_start.find("\x1b\\") {
                rest = &after_start[end_st + 2..];
            } else if let Some(end_bel) = after_start.find('\x07') {
                rest = &after_start[end_bel + 1..];
            } else {
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

pub(super) fn expand_kitty_payload_to_rgba(raw: &[u8], format_code: u32, w: u16, h: u16) -> Vec<u8> {
    let total_px = usize::from(w) * usize::from(h);
    let mut rgba = Vec::with_capacity(total_px * 4);
    match format_code {
        24 => {
            for px in raw.chunks(3).take(total_px) {
                let r = *px.first().unwrap_or(&0);
                let g = *px.get(1).unwrap_or(&0);
                let b = *px.get(2).unwrap_or(&0);
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        _ => {
            for px in raw.chunks(4).take(total_px) {
                let r = *px.first().unwrap_or(&0);
                let g = *px.get(1).unwrap_or(&0);
                let b = *px.get(2).unwrap_or(&0);
                let a = *px.get(3).unwrap_or(&255);
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
    }
    while rgba.len() < total_px * 4 {
        rgba.extend_from_slice(&[0, 0xFF, 0x66, 255]);
    }
    rgba
}

pub(super) fn decode_sixel_placement(
    id: u32,
    row: u16,
    col: u16,
    payload: &str,
) -> TerminalGraphicPlacement {
    let mut palette: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (51, 102, 255),
        (255, 51, 51),
        (0, 255, 102),
        (255, 51, 255),
        (51, 255, 255),
        (255, 255, 85),
        (255, 255, 255),
        (128, 128, 128),
        (0, 170, 0),
        (255, 176, 0),
        (0, 170, 170),
        (170, 0, 170),
        (170, 170, 0),
        (85, 85, 255),
        (200, 200, 200),
    ];
    let mut active_reg: usize = 3;
    let mut width_px: u16 = 0;
    let mut height_px: u16 = 6;
    let mut bands: Vec<Vec<(u8, u8, u8, u8)>> = vec![Vec::new()];
    let mut cur_x: usize = 0;

    let bytes = payload.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                // Raster attributes "Pan;Pad;Ph;Pv
                i += 1;
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                    i += 1;
                }
                let attr = &payload[start..i];
                let nums: Vec<u16> = attr
                    .split(';')
                    .filter_map(|s| s.parse::<u16>().ok())
                    .collect();
                if nums.len() >= 4 {
                    width_px = width_px.max(nums[2]);
                    height_px = height_px.max(nums[3]);
                }
            }
            b'#' => {
                // Color specifier #<reg> or #<reg>;2;<r>;<g>;<b>
                i += 1;
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                    i += 1;
                }
                let spec = &payload[start..i];
                let nums: Vec<u16> = spec
                    .split(';')
                    .filter_map(|s| s.parse::<u16>().ok())
                    .collect();
                if let Some(&reg) = nums.first() {
                    active_reg = usize::from(reg) % palette.len();
                    if nums.len() >= 5 && nums[1] == 2 {
                        let r = ((u32::from(nums[2].min(100)) * 255) / 100) as u8;
                        let g = ((u32::from(nums[3].min(100)) * 255) / 100) as u8;
                        let b = ((u32::from(nums[4].min(100)) * 255) / 100) as u8;
                        palette[active_reg] = (r, g, b);
                    }
                }
            }
            b'!' => {
                // RLE repeat !<count><sixel_char>
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let count = payload[start..i]
                    .parse::<usize>()
                    .unwrap_or(1)
                    .clamp(1, 512);
                if i < bytes.len() && (0x3F..=0x7E).contains(&bytes[i]) {
                    let bits = bytes[i] - 0x3F;
                    let (r, g, b) = palette[active_reg];
                    let band = bands.last_mut().expect("band");
                    for _ in 0..count {
                        if cur_x >= band.len() {
                            band.push((r, g, b, bits));
                        } else if bits != 0 {
                            band[cur_x] = (r, g, b, bits);
                        }
                        cur_x += 1;
                    }
                    width_px = width_px.max(cur_x as u16);
                    i += 1;
                }
            }
            b'$' => {
                cur_x = 0;
                i += 1;
            }
            b'-' => {
                cur_x = 0;
                bands.push(Vec::new());
                height_px = height_px.max((bands.len() * 6) as u16);
                i += 1;
            }
            0x3F..=0x7E => {
                let bits = bytes[i] - 0x3F;
                let (r, g, b) = palette[active_reg];
                let band = bands.last_mut().expect("band");
                if cur_x >= band.len() {
                    band.push((r, g, b, bits));
                } else if bits != 0 {
                    band[cur_x] = (r, g, b, bits);
                }
                cur_x += 1;
                width_px = width_px.max(cur_x as u16);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let w = width_px.clamp(1, 512);
    let h = height_px.clamp(6, 512);
    let mut rgba = vec![0u8; usize::from(w) * usize::from(h) * 4];
    for (band_idx, band) in bands.iter().enumerate() {
        for (x, &(r, g, b, bits)) in band.iter().enumerate() {
            if x >= usize::from(w) {
                break;
            }
            for bit in 0..6usize {
                let y = band_idx * 6 + bit;
                if y >= usize::from(h) {
                    break;
                }
                if (bits & (1 << bit)) != 0 {
                    let off = (y * usize::from(w) + x) * 4;
                    rgba[off] = r;
                    rgba[off + 1] = g;
                    rgba[off + 2] = b;
                    rgba[off + 3] = 255;
                }
            }
        }
    }

    let cols = w.div_ceil(8).max(1);
    let rows = h.div_ceil(16).max(1);
    TerminalGraphicPlacement {
        id,
        protocol: GraphicProtocol::Sixel,
        row,
        col,
        cols,
        rows,
        pixel_width: w,
        pixel_height: h,
        rgba_bytes: rgba,
    }
}
