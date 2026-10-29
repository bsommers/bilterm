use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use serde::{Deserialize, Serialize};

/// Selectable software caret styles for the floating prompt and active PTY canvas (`UI-05`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CaretStyle {
    /// Classic inverted block cursor (`█` / `Modifier::REVERSED`).
    #[default]
    SolidBlock,
    /// IBM 3270 hollow block cursor (`▯`).
    BlinkingHollowBlock,
    /// Classic DOS underscore caret (`_`).
    Underscore,
    /// Vertical bar caret (`▏`).
    VerticalBar,
}

impl CaretStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "block" | "solid" | "solid-block" => Some(Self::SolidBlock),
            "hollow" | "hollow-block" | "blinking-hollow-block" => Some(Self::BlinkingHollowBlock),
            "underscore" | "underline" => Some(Self::Underscore),
            "bar" | "beam" | "vertical-bar" => Some(Self::VerticalBar),
            _ => None,
        }
    }
}

/// Toggleable focal spotlight shape (`:spotlight`, `Alt+O` / `Ctrl+X O`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpotlightMode {
    /// Spotlight disabled.
    #[default]
    Off,
    /// Circular focal spotlight (compensates 2:1 for terminal character cell aspect ratio).
    Circle,
    /// Elliptical focal spotlight with independent horizontal (`rx`) and vertical (`ry`) radii.
    Ellipse,
}

impl SpotlightMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" | "disable" => Some(Self::Off),
            "circle" | "circular" | "round" => Some(Self::Circle),
            "ellipse" | "elliptical" | "oval" | "beam" => Some(Self::Ellipse),
            _ => None,
        }
    }

    pub fn label(self, pixelated: bool) -> &'static str {
        match (self, pixelated) {
            (Self::Off, _) => "OFF",
            (Self::Circle, false) => "CIRCLE (FUZZY)",
            (Self::Circle, true) => "CIRCLE (PIXELATED)",
            (Self::Ellipse, false) => "ELLIPSE (FUZZY)",
            (Self::Ellipse, true) => "ELLIPSE (PIXELATED)",
        }
    }
}

/// Converts any Ratatui `Color` into an `(r, g, b)` tuple for smooth TrueColor shader modulation.
pub fn color_to_rgb(color: Color, default_rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (170, 0, 0),
        Color::Green => (0, 170, 0),
        Color::Yellow => (170, 85, 0),
        Color::Blue => (0, 0, 170),
        Color::Magenta => (170, 0, 170),
        Color::Cyan => (0, 170, 170),
        Color::Gray => (170, 170, 170),
        Color::DarkGray => (85, 85, 85),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (85, 255, 85),
        Color::LightYellow => (255, 255, 85),
        Color::LightBlue => (85, 85, 255),
        Color::LightMagenta => (255, 85, 255),
        Color::LightCyan => (85, 255, 255),
        Color::White => (255, 255, 255),
        Color::Indexed(idx) => match idx {
            0 => (0, 0, 0),
            1 => (170, 0, 0),
            2 => (0, 170, 0),
            3 => (170, 85, 0),
            4 => (0, 0, 170),
            5 => (170, 0, 170),
            6 => (0, 170, 170),
            7 => (170, 170, 170),
            8 => (85, 85, 85),
            9 => (255, 85, 85),
            10 => (85, 255, 85),
            11 => (255, 255, 85),
            12 => (85, 85, 255),
            13 => (255, 85, 255),
            14 => (85, 255, 255),
            15 => (255, 255, 255),
            16..=231 => {
                let n = idx - 16;
                let r = (n / 36) % 6;
                let g = (n / 6) % 6;
                let b = n % 6;
                let to_u8 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                (to_u8(r), to_u8(g), to_u8(b))
            }
            232..=255 => {
                let gray = 8 + (idx - 232) * 10;
                (gray, gray, gray)
            }
        },
        Color::Reset => default_rgb,
    }
}

/// Renders a resizable circular or elliptical focal spotlight centered at `(cursor_x, cursor_y)`
/// with either smooth fuzzy Hermite `smoothstep` edges or pixelated character-block stepped edges.
#[allow(clippy::too_many_arguments)]
pub fn render_spotlight_cursor(
    buf: &mut Buffer,
    target_rect: Rect,
    cursor_x: u16,
    cursor_y: u16,
    mode: SpotlightMode,
    rx_cols: u16,
    ry_rows: u16,
    feather_pct: u8,
    pixelated: bool,
    bloom_bg: Color,
) {
    if mode == SpotlightMode::Off {
        return;
    }
    let clamped = target_rect.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    let rx = f32::from(rx_cols.max(4));
    let ry = match mode {
        SpotlightMode::Circle => (rx * 0.5).max(2.0),
        SpotlightMode::Ellipse => f32::from(ry_rows.max(2)),
        SpotlightMode::Off => return,
    };

    let feather = f32::from(feather_pct.clamp(5, 100)) / 100.0;
    let inner_edge = (1.0 - feather).max(0.0);
    let outer_edge = 1.0 + feather;
    let outer_dim: f32 = 0.24;
    let bloom_rgb = color_to_rgb(bloom_bg, (0, 51, 51));

    for y in clamped.y..clamped.y + clamped.height {
        for x in clamped.x..clamped.x + clamped.width {
            let (dx, dy) = if pixelated {
                // Quantize to 2x1 character-block cells aligned to the terminal grid so circular/elliptical edges step in crisp character blocks
                let block_x = i32::from(x & !1);
                let block_cx = i32::from(cursor_x & !1);
                let block_dx = block_x - block_cx;
                let block_dy = i32::from(y) - i32::from(cursor_y);
                (block_dx as f32, block_dy as f32)
            } else {
                (
                    f32::from(x) - f32::from(cursor_x),
                    f32::from(y) - f32::from(cursor_y),
                )
            };

            let norm_x = dx / rx;
            let norm_y = dy / ry;
            let dist = (norm_x * norm_x + norm_y * norm_y).sqrt();

            let (brightness, bloom_mix) = if dist <= inner_edge {
                let core_ratio = if inner_edge > 0.01 {
                    dist / inner_edge
                } else {
                    0.0
                };
                let b_mix = if pixelated {
                    if core_ratio < 0.55 { 0.42 } else { 0.22 }
                } else {
                    0.42 * (1.0 - core_ratio * 0.55)
                };
                (1.0_f32, b_mix)
            } else if dist < outer_edge {
                let t = ((dist - inner_edge) / (outer_edge - inner_edge)).clamp(0.0, 1.0);
                if pixelated {
                    // Quantize feather transition into discrete character-block luminance steps
                    if t < 0.33 {
                        (0.82, 0.16)
                    } else if t < 0.66 {
                        (0.54, 0.06)
                    } else {
                        (outer_dim, 0.0)
                    }
                } else {
                    let smooth = t * t * (3.0 - 2.0 * t);
                    let b = 1.0 - smooth * (1.0 - outer_dim);
                    let b_mix = 0.20 * (1.0 - smooth);
                    (b, b_mix)
                }
            } else {
                (outer_dim, 0.0)
            };

            if let Some(cell) = buf.cell_mut((x, y)) {
                let style = cell.style();
                let (fg_r, fg_g, fg_b) = color_to_rgb(style.fg.unwrap_or(Color::White), (220, 220, 220));
                let (bg_r, bg_g, bg_b) = color_to_rgb(style.bg.unwrap_or(Color::Black), (8, 12, 24));

                let new_fg = Color::Rgb(
                    (f32::from(fg_r) * brightness).round().clamp(0.0, 255.0) as u8,
                    (f32::from(fg_g) * brightness).round().clamp(0.0, 255.0) as u8,
                    (f32::from(fg_b) * brightness).round().clamp(0.0, 255.0) as u8,
                );

                let dimmed_bg_r = f32::from(bg_r) * brightness;
                let dimmed_bg_g = f32::from(bg_g) * brightness;
                let dimmed_bg_b = f32::from(bg_b) * brightness;

                let new_bg = Color::Rgb(
                    (dimmed_bg_r * (1.0 - bloom_mix) + f32::from(bloom_rgb.0) * bloom_mix)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                    (dimmed_bg_g * (1.0 - bloom_mix) + f32::from(bloom_rgb.1) * bloom_mix)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                    (dimmed_bg_b * (1.0 - bloom_mix) + f32::from(bloom_rgb.2) * bloom_mix)
                        .round()
                        .clamp(0.0, 255.0) as u8,
                );

                // In pixelated mode, render stepped character-block dither glyph on empty cells in the feather ring
                if pixelated && cell.symbol() == " " && dist > inner_edge && dist < outer_edge {
                    let t = ((dist - inner_edge) / (outer_edge - inner_edge)).clamp(0.0, 1.0);
                    let block_sym = if t < 0.33 {
                        "▒"
                    } else if t < 0.66 {
                        "░"
                    } else {
                        " "
                    };
                    if block_sym != " " {
                        cell.set_symbol(block_sym);
                        cell.set_style(
                            style
                                .fg(Color::Rgb(
                                    (f32::from(bloom_rgb.0) * 0.65) as u8,
                                    (f32::from(bloom_rgb.1) * 0.65) as u8,
                                    (f32::from(bloom_rgb.2) * 0.65) as u8,
                                ))
                                .bg(new_bg),
                        );
                        continue;
                    }
                }

                cell.set_style(style.fg(new_fg).bg(new_bg));
            }
        }
    }
}

/// Renders a circular or elliptical **Focal Zoom (Fisheye Magnifying Lens)** centered at
/// `(cursor_x, cursor_y)` with smooth Hermite `smoothstep` radial magnification falloff
/// (or quantized `2x1` character-block magnification when `pixelated` is `true`) across
/// multiple magnification levels (`zoom_pct`: e.g. `125` = `1.25x`, `150` = `1.50x`,
/// `200` = `2.00x`, `300` = `3.00x`).
#[allow(clippy::too_many_arguments)]
pub fn render_focal_zoom_cursor(
    buf: &mut Buffer,
    target_rect: Rect,
    cursor_x: u16,
    cursor_y: u16,
    mode: SpotlightMode,
    zoom_pct: u16,
    rx_cols: u16,
    ry_rows: u16,
    feather_pct: u8,
    pixelated: bool,
    lens_tint: Color,
) {
    if mode == SpotlightMode::Off || zoom_pct <= 100 {
        return;
    }
    let clamped = target_rect.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    let rx = f32::from(rx_cols.max(4));
    let ry = match mode {
        SpotlightMode::Circle => (rx * 0.5).max(2.0),
        SpotlightMode::Ellipse => f32::from(ry_rows.max(2)),
        SpotlightMode::Off => return,
    };

    let peak_zoom = f32::from(zoom_pct.clamp(105, 400)) / 100.0;
    let feather = f32::from(feather_pct.clamp(5, 100)) / 100.0;
    let inner_edge = (1.0 - feather).max(0.0);
    let outer_edge = 1.0 + feather;
    let tint_rgb = color_to_rgb(lens_tint, (0, 51, 51));

    // Snapshot pre-zoom cells inside `clamped` so inverse-mapped lens sampling is artifact-free
    let width_usize = usize::from(clamped.width);
    let height_usize = usize::from(clamped.height);
    let mut snapshot = Vec::with_capacity(width_usize * height_usize);
    for y in clamped.y..clamped.y + clamped.height {
        for x in clamped.x..clamped.x + clamped.width {
            snapshot.push(buf.cell((x, y)).cloned().unwrap_or_default());
        }
    }

    let local_mag_at = |x: u16, y: u16| -> (f32, f32) {
        let (dx, dy) = if pixelated {
            let block_x = i32::from(x & !1);
            let block_cx = i32::from(cursor_x & !1);
            (
                (block_x - block_cx) as f32,
                (i32::from(y) - i32::from(cursor_y)) as f32,
            )
        } else {
            (
                f32::from(x) - f32::from(cursor_x),
                f32::from(y) - f32::from(cursor_y),
            )
        };
        let norm_x = dx / rx;
        let norm_y = dy / ry;
        let dist = (norm_x * norm_x + norm_y * norm_y).sqrt();
        if dist <= inner_edge {
            if pixelated {
                (peak_zoom, dist)
            } else {
                let core_t = if inner_edge > 0.01 {
                    (dist / inner_edge).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let core_s = core_t * core_t * (3.0 - 2.0 * core_t);
                let mag = peak_zoom - (peak_zoom - 1.0) * 0.12 * core_s;
                (mag, dist)
            }
        } else if dist < outer_edge {
            let t = ((dist - inner_edge) / (outer_edge - inner_edge)).clamp(0.0, 1.0);
            if pixelated {
                let step_mag = if t < 0.33 {
                    1.0 + (peak_zoom - 1.0) * 0.75
                } else if t < 0.66 {
                    1.0 + (peak_zoom - 1.0) * 0.40
                } else {
                    1.0 + (peak_zoom - 1.0) * 0.15
                };
                (step_mag, dist)
            } else {
                let s = t * t * (3.0 - 2.0 * t);
                let edge_start = peak_zoom - (peak_zoom - 1.0) * 0.12;
                let mag = edge_start * (1.0 - s) + 1.0 * s;
                (mag, dist)
            }
        } else {
            (1.0, dist)
        }
    };

    let map_to_src = |x: u16, y: u16, mag: f32| -> (u16, u16, f32, f32) {
        let (dx, dy) = if pixelated {
            let block_x = i32::from(x & !1);
            let block_cx = i32::from(cursor_x & !1);
            (
                (block_x - block_cx) as f32,
                (i32::from(y) - i32::from(cursor_y)) as f32,
            )
        } else {
            (
                f32::from(x) - f32::from(cursor_x),
                f32::from(y) - f32::from(cursor_y),
            )
        };
        let src_x_f = (f32::from(cursor_x) + dx / mag).clamp(
            f32::from(clamped.x),
            f32::from(clamped.x + clamped.width - 1),
        );
        let src_y_f = (f32::from(cursor_y) + dy / mag).clamp(
            f32::from(clamped.y),
            f32::from(clamped.y + clamped.height - 1),
        );
        (src_x_f.round() as u16, src_y_f.round() as u16, src_x_f, src_y_f)
    };

    for y in clamped.y..clamped.y + clamped.height {
        for x in clamped.x..clamped.x + clamped.width {
            let (mag, dist) = local_mag_at(x, y);
            if dist >= outer_edge || mag <= 1.001 {
                continue;
            }
            let (src_x, src_y, _src_x_f, _src_y_f) = map_to_src(x, y, mag);
            let snap_idx = usize::from(src_y - clamped.y) * width_usize + usize::from(src_x - clamped.x);
            let Some(sampled) = snapshot.get(snap_idx) else {
                continue;
            };

            let sampled_sym = sampled.symbol();
            let style = sampled.style();
            let fg_rgb = color_to_rgb(style.fg.unwrap_or(Color::Reset), (192, 208, 224));
            let bg_rgb = color_to_rgb(style.bg.unwrap_or(Color::Reset), (0, 0, 20));

            let norm_boost = ((mag - 1.0) / (peak_zoom - 1.0).max(0.05)).clamp(0.0, 1.0);
            let fg_gain = 1.0 + 0.22 * norm_boost;
            let lens_mix = 0.28 * norm_boost;

            let new_fg = Color::Rgb(
                (f32::from(fg_rgb.0) * fg_gain).round().clamp(0.0, 255.0) as u8,
                (f32::from(fg_rgb.1) * fg_gain).round().clamp(0.0, 255.0) as u8,
                (f32::from(fg_rgb.2) * fg_gain).round().clamp(0.0, 255.0) as u8,
            );
            let new_bg = Color::Rgb(
                (f32::from(bg_rgb.0) * (1.0 - lens_mix) + f32::from(tint_rgb.0) * lens_mix)
                    .round()
                    .clamp(0.0, 255.0) as u8,
                (f32::from(bg_rgb.1) * (1.0 - lens_mix) + f32::from(tint_rgb.1) * lens_mix)
                    .round()
                    .clamp(0.0, 255.0) as u8,
                (f32::from(bg_rgb.2) * (1.0 - lens_mix) + f32::from(tint_rgb.2) * lens_mix)
                    .round()
                    .clamp(0.0, 255.0) as u8,
            );

            let is_graphical_subcell = sampled_sym.chars().next().is_some_and(|ch| {
                let u = ch as u32;
                (0x2800..=0x28ff).contains(&u)
                    || (0x1fb00..=0x1fb3b).contains(&u)
                    || (0x2580..=0x259f).contains(&u)
            });

            if let Some(cell) = buf.cell_mut((x, y)) {
                if pixelated || is_graphical_subcell {
                    let out_sym = if sampled_sym == " " && dist > inner_edge && dist < outer_edge {
                        let t = ((dist - inner_edge) / (outer_edge - inner_edge)).clamp(0.0, 1.0);
                        if t < 0.33 {
                            "▒"
                        } else if t < 0.66 {
                            "░"
                        } else {
                            " "
                        }
                    } else {
                        sampled_sym
                    };
                    cell.set_symbol(out_sym);
                    let mut new_style = style.fg(new_fg).bg(new_bg);
                    if mag >= 1.15 && out_sym != " " {
                        new_style = new_style.add_modifier(Modifier::BOLD);
                    }
                    cell.set_style(new_style);
                } else {
                    // Text mode: never double-space characters with blank gaps; preserve text alignment
                    // while applying smooth radial lens luminance boost + bold weight (paired with font scaling).
                    let orig_idx =
                        usize::from(y - clamped.y) * width_usize + usize::from(x - clamped.x);
                    let orig_cell = snapshot.get(orig_idx).unwrap_or(sampled);
                    let orig_sym = orig_cell.symbol();
                    let orig_style = orig_cell.style();
                    let ofg = color_to_rgb(orig_style.fg.unwrap_or(Color::Reset), (192, 208, 224));
                    let obg = color_to_rgb(orig_style.bg.unwrap_or(Color::Reset), (0, 0, 20));
                    let tfg = Color::Rgb(
                        (f32::from(ofg.0) * fg_gain).round().clamp(0.0, 255.0) as u8,
                        (f32::from(ofg.1) * fg_gain).round().clamp(0.0, 255.0) as u8,
                        (f32::from(ofg.2) * fg_gain).round().clamp(0.0, 255.0) as u8,
                    );
                    let tbg = Color::Rgb(
                        (f32::from(obg.0) * (1.0 - lens_mix) + f32::from(tint_rgb.0) * lens_mix)
                            .round()
                            .clamp(0.0, 255.0) as u8,
                        (f32::from(obg.1) * (1.0 - lens_mix) + f32::from(tint_rgb.1) * lens_mix)
                            .round()
                            .clamp(0.0, 255.0) as u8,
                        (f32::from(obg.2) * (1.0 - lens_mix) + f32::from(tint_rgb.2) * lens_mix)
                            .round()
                            .clamp(0.0, 255.0) as u8,
                    );
                    cell.set_symbol(orig_sym);
                    let mut new_style = orig_style.fg(tfg).bg(tbg);
                    if mag >= 1.15 && orig_sym != " " {
                        new_style = new_style.add_modifier(Modifier::BOLD);
                    }
                    cell.set_style(new_style);
                }
            }
        }
    }
}

/// Renders the non-destructive IBM 3270 full-pane cross-hair cursor ruler (`UI-05`) by tinting
/// the background of all cells in `pane_rect` sharing `cursor_x` (vertical hair) or `cursor_y`
/// (horizontal hair) with `tint_bg`.
pub fn render_crosshair_cursor(
    buf: &mut Buffer,
    pane_rect: Rect,
    cursor_x: u16,
    cursor_y: u16,
    tint_bg: Color,
) {
    let clamped = pane_rect.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    let tint_rgb = color_to_rgb(tint_bg, (0, 43, 85));

    // Horizontal hair across `cursor_y`
    if cursor_y >= clamped.y && cursor_y < clamped.y + clamped.height {
        for x in clamped.x..clamped.x + clamped.width {
            if let Some(cell) = buf.cell_mut((x, cursor_y)) {
                let mut st = cell.style().bg(tint_bg);
                if let Some(fg_col) = st.fg
                    && fg_col != Color::Reset
                {
                    let fg_rgb = color_to_rgb(fg_col, (232, 238, 245));
                    if crate::ui::contrast::rgb_contrast_ratio(fg_rgb, tint_rgb) < 4.5 {
                        let (r, g, b) = crate::ui::contrast::boost_fg_contrast(fg_rgb, tint_rgb, 4.8);
                        st = st.fg(Color::Rgb(r, g, b));
                    }
                }
                cell.set_style(st);
            }
        }
    }

    // Vertical hair down `cursor_x`
    if cursor_x >= clamped.x && cursor_x < clamped.x + clamped.width {
        for y in clamped.y..clamped.y + clamped.height {
            if let Some(cell) = buf.cell_mut((cursor_x, y)) {
                let mut st = cell.style().bg(tint_bg);
                if let Some(fg_col) = st.fg
                    && fg_col != Color::Reset
                {
                    let fg_rgb = color_to_rgb(fg_col, (232, 238, 245));
                    if crate::ui::contrast::rgb_contrast_ratio(fg_rgb, tint_rgb) < 4.5 {
                        let (r, g, b) = crate::ui::contrast::boost_fg_contrast(fg_rgb, tint_rgb, 4.8);
                        st = st.fg(Color::Rgb(r, g, b));
                    }
                }
                cell.set_style(st);
            }
        }
    }
}

/// Renders the software caret at `(cursor_x, cursor_y)` according to `caret_style` (`UI-05`).
pub fn render_caret(
    buf: &mut Buffer,
    cursor_x: u16,
    cursor_y: u16,
    caret_style: CaretStyle,
    accent_fg: Color,
) {
    let Some(cell) = buf.cell_mut((cursor_x, cursor_y)) else {
        return;
    };
    match caret_style {
        CaretStyle::SolidBlock => {
            cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
        }
        CaretStyle::BlinkingHollowBlock => {
            if cell.symbol() == " " {
                cell.set_symbol("▯");
            }
            cell.set_style(
                cell.style()
                    .fg(accent_fg)
                    .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            );
        }
        CaretStyle::Underscore => {
            if cell.symbol() == " " {
                cell.set_symbol("_");
            }
            cell.set_style(
                cell.style()
                    .fg(accent_fg)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
            );
        }
        CaretStyle::VerticalBar => {
            if cell.symbol() == " " {
                cell.set_symbol("▏");
            }
            cell.set_style(cell.style().fg(accent_fg).add_modifier(Modifier::BOLD));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosshair_cursor_tints_full_row_and_column_non_destructively() {
        let area = Rect::new(0, 0, 10, 6);
        let mut buf = Buffer::empty(area);
        buf.cell_mut((2, 3)).unwrap().set_symbol("A");

        let tint = Color::Rgb(0, 51, 0);
        render_crosshair_cursor(&mut buf, area, 5, 3, tint);
        // Cell (2, 3) keeps its "A" glyph while receiving the horizontal hair tint
        assert_eq!(buf.cell((2, 3)).unwrap().symbol(), "A");
        assert_eq!(buf.cell((2, 3)).unwrap().style().bg, Some(tint));
        // Cell (5, 1) receives the vertical hair tint
        assert_eq!(buf.cell((5, 1)).unwrap().style().bg, Some(tint));
    }

    #[test]
    fn spotlight_cursor_fuzzy_and_pixelated_shapes_modulate_cells() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        for y in 0..20 {
            for x in 0..40 {
                let c = buf.cell_mut((x, y)).unwrap();
                c.set_style(c.style().fg(Color::Rgb(0, 240, 100)).bg(Color::Rgb(0, 20, 40)));
            }
        }
        buf.cell_mut((20, 10)).unwrap().set_symbol("X");
        buf.cell_mut((0, 0)).unwrap().set_symbol("Z");

        // 1. Elliptical Fuzzy Spotlight centered at (20, 10)
        render_spotlight_cursor(
            &mut buf,
            area,
            20,
            10,
            SpotlightMode::Ellipse,
            12,
            5,
            45,
            false,
            Color::Rgb(0, 80, 40),
        );

        // Focal center (20, 10) retains full luminance while distant corner (0, 0) is dimmed
        let center_fg = buf.cell((20, 10)).unwrap().style().fg.unwrap();
        let corner_fg = buf.cell((0, 0)).unwrap().style().fg.unwrap();
        assert_eq!(center_fg, Color::Rgb(0, 240, 100));
        if let Color::Rgb(_, g, _) = corner_fg {
            assert!(g < 100, "corner green channel should be dimmed, got {g}");
        } else {
            panic!("expected Rgb color at corner");
        }

        // 2. Circular Pixelated Character-Block Spotlight
        let mut buf_px = Buffer::empty(area);
        render_spotlight_cursor(
            &mut buf_px,
            area,
            20,
            10,
            SpotlightMode::Circle,
            10,
            5,
            50,
            true,
            Color::Rgb(0, 80, 40),
        );

        // Adjacent horizontal cells in the same 2x1 character block share identical quantized background
        let bg_even = buf_px.cell((14, 10)).unwrap().style().bg;
        let bg_odd = buf_px.cell((15, 10)).unwrap().style().bg;
        assert_eq!(bg_even, bg_odd);
    }

    #[test]
    fn focal_zoom_lens_smooth_and_pixelated_magnify_across_levels() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        // Place "ABCDE" at (18..=22, 10) with 'C' at the focal center (20, 10)
        for (i, ch) in ["A", "B", "C", "D", "E"].iter().enumerate() {
            let cell = buf.cell_mut((18 + i as u16, 10)).unwrap();
            cell.set_symbol(ch);
            cell.set_style(cell.style().fg(Color::Rgb(100, 200, 100)).bg(Color::Rgb(0, 10, 20)));
        }
        buf.cell_mut((0, 0)).unwrap().set_symbol("Z");

        // 2.00x (200%) smooth elliptical focal zoom centered at (20, 10)
        render_focal_zoom_cursor(
            &mut buf,
            area,
            20,
            10,
            SpotlightMode::Ellipse,
            200,
            12,
            5,
            45,
            false,
            Color::Rgb(0, 60, 80),
        );

        // Center 'C' stays at (20, 10) with BOLD modifier and boosted phosphor luminance,
        // and neighboring text 'D' stays contiguous at (21, 10) without double-spacing!
        assert_eq!(buf.cell((20, 10)).unwrap().symbol(), "C");
        assert!(
            buf.cell((20, 10))
                .unwrap()
                .style()
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(buf.cell((21, 10)).unwrap().symbol(), "D");
        // Distant cell (0, 0) outside the lens is completely untouched
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "Z");

        // Pixelated 2.00x circular focal zoom expands center cell into 2x1 character block
        let mut buf_px = Buffer::empty(area);
        buf_px.cell_mut((20, 10)).unwrap().set_symbol("#");
        render_focal_zoom_cursor(
            &mut buf_px,
            area,
            20,
            10,
            SpotlightMode::Circle,
            200,
            10,
            5,
            50,
            true,
            Color::Rgb(0, 60, 80),
        );
        assert_eq!(buf_px.cell((20, 10)).unwrap().symbol(), "#");
        assert_eq!(buf_px.cell((21, 10)).unwrap().symbol(), "#");
    }
}
