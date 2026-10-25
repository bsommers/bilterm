//! Modern 2026/2027 Retained Scene-Graph, 2D Sprite Engine, Declarative Plotting & Image Viewer (`CANV-01`).
//!
//! Provides a retained, inspectable graphical scene-graph ([`PaneCanvasScene`]) per composite pane
//! alongside the classic Tektronix 4010/4014 storage-tube state machine ([`TektronixState`](crate::vt::tektronix::TektronixState)):
//! - **Vector Primitives ([`VectorPrimitive`])**: `Line`, `Polyline`, `CubicBezier`, `Circle`, `RectShape`, `Polygon`, `TextLabel`.
//! - **2D Sprite Engine ([`Sprite2D`])**: Position `(x, y)`, velocity `(vx, vy)`, dimensions `(width, height)`,
//!   `z_index`, frame animation, deterministic Euler physics (`step_physics`), and AABB collision detection (`collides_with`).
//! - **Declarative Live Data Plots ([`DeclarativePlot`], [`PlotKind`])**: `Line`, `Scatter`, `Bar`, `Candlestick` (OHLC), and `Heatmap`.
//! - **Interactive Image Viewer ([`InteractiveImageView`])**: Pan `(pan_x, pan_y)`, `zoom`, and `inspect_pixel(x, y)`.

use serde::{Deserialize, Serialize};

use crate::vt::tektronix::TektronixState;

/// Sub-cell character rasterization mode for TUI rendering (`CANV-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasSubcellMode {
    /// Unicode `2×4` Braille patterns (`U+2800..U+28FF`, 8 sub-pixels per cell).
    #[default]
    Braille,
    /// Unicode `2×3` Legacy Computing Sextants (`U+1FB00..U+1FB3B`, 6 sub-pixels per cell).
    Sextant,
    /// Unicode `2×2` Block Quadrants (`▘▝▖▗▚▞▛▜▙▟█`, 4 sub-pixels per cell).
    Quadrant,
}

impl CanvasSubcellMode {
    /// Sub-pixel multiplier `(sub_cols_per_cell, sub_rows_per_cell)`.
    #[inline]
    pub const fn subcell_grid(self) -> (u16, u16) {
        match self {
            Self::Braille => (2, 4),
            Self::Sextant => (2, 3),
            Self::Quadrant => (2, 2),
        }
    }

    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "braille" | "2x4" => Some(Self::Braille),
            "sextant" | "sextants" | "2x3" => Some(Self::Sextant),
            "quadrant" | "quadrants" | "2x2" | "block" => Some(Self::Quadrant),
            _ => None,
        }
    }
}

/// Retained 2D vector graphic primitive in normalized sub-cell canvas coordinates (`CANV-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VectorPrimitive {
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        rgb: (u8, u8, u8),
    },
    Polyline {
        points: Vec<(f32, f32)>,
        rgb: (u8, u8, u8),
        closed: bool,
    },
    CubicBezier {
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
        rgb: (u8, u8, u8),
    },
    Circle {
        cx: f32,
        cy: f32,
        radius: f32,
        rgb: (u8, u8, u8),
        filled: bool,
    },
    RectShape {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rgb: (u8, u8, u8),
        filled: bool,
    },
    Polygon {
        vertices: Vec<(f32, f32)>,
        rgb: (u8, u8, u8),
    },
    TextLabel {
        x: f32,
        y: f32,
        text: String,
        rgb: (u8, u8, u8),
    },
}

/// Retained 2D sprite with velocity, frame animation, z-index, and AABB collision detection (`CANV-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprite2D {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub width: u16,
    pub height: u16,
    pub z_index: i16,
    pub visible: bool,
    pub rgb: (u8, u8, u8),
    pub current_frame: usize,
    /// Per-frame bitmask (`width * height` bytes, non-zero = opaque pixel).
    pub frames: Vec<Vec<u8>>,
}

impl Sprite2D {
    pub fn new_solid(
        id: u32,
        name: impl Into<String>,
        x: f32,
        y: f32,
        width: u16,
        height: u16,
        rgb: (u8, u8, u8),
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            id,
            name: name.into(),
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            width: w,
            height: h,
            z_index: 0,
            visible: true,
            rgb,
            current_frame: 0,
            frames: vec![vec![1u8; usize::from(w) * usize::from(h)]],
        }
    }

    /// Advances sprite position by `(vx * dt, vy * dt)` and increments `current_frame` (`CANV-01`).
    pub fn step_physics(&mut self, dt: f32) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if !self.frames.is_empty() {
            self.current_frame = (self.current_frame + 1) % self.frames.len();
        }
    }

    /// Axis-Aligned Bounding Box (AABB) collision check against `other` (`CANV-01`).
    pub fn collides_with(&self, other: &Self) -> bool {
        if !self.visible || !other.visible {
            return false;
        }
        let a_x1 = self.x + f32::from(self.width);
        let a_y1 = self.y + f32::from(self.height);
        let b_x1 = other.x + f32::from(other.width);
        let b_y1 = other.y + f32::from(other.height);
        self.x < b_x1 && a_x1 > other.x && self.y < b_y1 && a_y1 > other.y
    }
}

/// Open-High-Low-Close (OHLC) bar for `PlotKind::Candlestick` (`CANV-01`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandlestickBar {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
}

/// Declarative plot chart type (`CANV-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlotKind {
    Line(Vec<f32>),
    Scatter(Vec<(f32, f32)>),
    Bar(Vec<f32>),
    Candlestick(Vec<CandlestickBar>),
    Heatmap {
        rows: usize,
        cols: usize,
        values: Vec<f32>,
    },
}

/// Declarative live data plot rendered on the pane canvas (`CANV-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeclarativePlot {
    pub title: String,
    pub kind: PlotKind,
    pub rgb: (u8, u8, u8),
}

/// Interactive RGBA image viewer supporting pan, zoom, and pixel inspection (`CANV-01`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InteractiveImageView {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
    pub pan_x: i32,
    pub pan_y: i32,
    pub zoom: f32,
}

impl InteractiveImageView {
    pub fn new_rgba(width: u16, height: u16, rgba: Vec<u8>) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            rgba,
            pan_x: 0,
            pan_y: 0,
            zoom: 1.0,
        }
    }

    /// Generates a deterministic procedural RGBA test pattern image (`width × height`).
    pub fn new_test_pattern(width: u16, height: u16) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let mut rgba = Vec::with_capacity(usize::from(w) * usize::from(h) * 4);
        for y in 0..h {
            for x in 0..w {
                let r = ((u32::from(x) * 255) / u32::from(w.max(1))) as u8;
                let g = ((u32::from(y) * 255) / u32::from(h.max(1))) as u8;
                let b = if (x / 4 + y / 4) % 2 == 0 { 220 } else { 40 };
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        Self::new_rgba(w, h, rgba)
    }

    /// Inspects the exact `[R, G, B, A]` pixel value at image coordinate `(x, y)` (`CANV-01`).
    pub fn inspect_pixel(&self, x: u16, y: u16) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (usize::from(y) * usize::from(self.width) + usize::from(x)) * 4;
        if idx + 3 < self.rgba.len() {
            Some([
                self.rgba[idx],
                self.rgba[idx + 1],
                self.rgba[idx + 2],
                self.rgba[idx + 3],
            ])
        } else {
            None
        }
    }
}

/// Complete retained scene-graph for a `CompositePane` (`CANV-01`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaneCanvasScene {
    pub active: bool,
    pub subcell_mode: CanvasSubcellMode,
    pub primitives: Vec<VectorPrimitive>,
    pub sprites: Vec<Sprite2D>,
    pub plots: Vec<DeclarativePlot>,
    pub image_view: Option<InteractiveImageView>,
}

impl PaneCanvasScene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_visible_content(&self) -> bool {
        self.active
            && (!self.primitives.is_empty()
                || !self.sprites.is_empty()
                || !self.plots.is_empty()
                || self.image_view.is_some())
    }

    pub fn clear(&mut self) {
        self.primitives.clear();
        self.sprites.clear();
        self.plots.clear();
        self.image_view = None;
    }

    /// Steps all sprite physics by `dt` and returns pairs of colliding sprite `(id_a, id_b)` (`CANV-01`).
    pub fn step_sprites(&mut self, dt: f32) -> Vec<(u32, u32)> {
        for sp in &mut self.sprites {
            sp.step_physics(dt);
        }
        self.detect_sprite_collisions()
    }

    /// Returns all pairs `(id_a, id_b)` of visible sprites whose AABBs currently intersect (`CANV-01`).
    pub fn detect_sprite_collisions(&self) -> Vec<(u32, u32)> {
        let mut hits = Vec::new();
        for i in 0..self.sprites.len() {
            for j in (i + 1)..self.sprites.len() {
                if self.sprites[i].collides_with(&self.sprites[j]) {
                    hits.push((self.sprites[i].id, self.sprites[j].id));
                }
            }
        }
        hits
    }

    /// Parses and executes a `:canvas <subcommand>` or `OSC 1337;Canvas=<subcommand>` instruction (`TEK-01`, `CANV-01`).
    pub fn execute_command(
        &mut self,
        tektronix: &mut TektronixState,
        raw: &str,
    ) -> std::result::Result<String, String> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        match parts.first().copied().unwrap_or("status") {
            "status" => Ok(format!(
                "CANVAS: subcell={:?} primitives={} sprites={} plots={} tek_vecs={}",
                self.subcell_mode,
                self.primitives.len(),
                self.sprites.len(),
                self.plots.len(),
                tektronix.segments.len()
            )),
            "clear" | "reset" => {
                self.clear();
                tektronix.clear_all();
                self.active = false;
                tektronix.active = false;
                Ok("CANVAS: cleared all retained primitives, sprites, plots, and Tektronix vectors".to_string())
            }
            "mode" | "subcell" => {
                let Some(mode_str) = parts.get(1) else {
                    return Err("Usage: :canvas mode <braille|sextant|quadrant>".to_string());
                };
                let mode = CanvasSubcellMode::from_name(mode_str)
                    .ok_or_else(|| "Usage: :canvas mode <braille|sextant|quadrant>".to_string())?;
                self.subcell_mode = mode;
                Ok(format!("CANVAS SUBCELL MODE: {mode:?}"))
            }
            "tek" | "tektronix" => match parts.get(1).copied() {
                None => {
                    if tektronix.active {
                        tektronix.clear_all();
                        Ok("TEKTRONIX 4010/4014 MODE: OFF".to_string())
                    } else {
                        if tektronix.segments.is_empty() {
                            tektronix.load_demo_scene();
                        } else {
                            tektronix.active = true;
                        }
                        Ok("TEKTRONIX 4010/4014 MODE: ON (:tek erase | :tek off)".to_string())
                    }
                }
                Some("on" | "true") => {
                    if tektronix.segments.is_empty() {
                        tektronix.load_demo_scene();
                    } else {
                        tektronix.active = true;
                    }
                    Ok("TEKTRONIX 4010/4014 MODE: ON".to_string())
                }
                Some("off" | "false") => {
                    tektronix.clear_all();
                    Ok("TEKTRONIX 4010/4014 MODE: OFF".to_string())
                }
                Some("erase" | "ff") => {
                    tektronix.erase_screen_with_persistence();
                    Ok("TEKTRONIX 4014: screen erased with phosphor afterglow decay".to_string())
                }
                Some("clear") => {
                    tektronix.clear_all();
                    Ok("TEKTRONIX 4014: cleared all storage-tube vectors".to_string())
                }
                Some("demo") => {
                    tektronix.load_demo_scene();
                    Ok("TEKTRONIX 4014: loaded wireframe storage-tube demo scene".to_string())
                }
                _ => Err("Usage: :canvas tek [on|off|erase|clear|demo]".to_string()),
            },
            "demo" => {
                tektronix.load_demo_scene();
                self.active = true;
                self.plots.clear();
                self.plots.push(DeclarativePlot {
                    title: "Plot (line)".to_string(),
                    kind: PlotKind::Line(vec![12.0, 28.0, 19.0, 45.0, 38.0, 64.0, 52.0, 84.0]),
                    rgb: (85, 255, 255),
                });
                self.primitives.push(VectorPrimitive::Circle {
                    cx: 24.0,
                    cy: 20.0,
                    radius: 10.0,
                    rgb: (255, 255, 85),
                    filled: false,
                });
                Ok("CANVAS DEMO: loaded Tektronix 4014 wireframe + Braille vector/plot scene (:canvas clear to reset)".to_string())
            }
            "line" => {
                if parts.len() < 5 {
                    return Err("Usage: :canvas line <x0> <y0> <x1> <y1>".to_string());
                }
                let x0 = parts[1].parse::<f32>().unwrap_or(0.0);
                let y0 = parts[2].parse::<f32>().unwrap_or(0.0);
                let x1 = parts[3].parse::<f32>().unwrap_or(20.0);
                let y1 = parts[4].parse::<f32>().unwrap_or(10.0);
                self.active = true;
                self.primitives.push(VectorPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    rgb: (85, 255, 255),
                });
                Ok(format!("CANVAS: added line ({x0},{y0})->({x1},{y1})"))
            }
            "circle" => {
                if parts.len() < 4 {
                    return Err("Usage: :canvas circle <cx> <cy> <radius>".to_string());
                }
                let cx = parts[1].parse::<f32>().unwrap_or(20.0);
                let cy = parts[2].parse::<f32>().unwrap_or(20.0);
                let radius = parts[3].parse::<f32>().unwrap_or(8.0);
                let filled = parts.get(4).is_some_and(|&s| s == "fill" || s == "filled");
                self.active = true;
                self.primitives.push(VectorPrimitive::Circle {
                    cx,
                    cy,
                    radius,
                    rgb: (255, 255, 85),
                    filled,
                });
                Ok(format!("CANVAS: added circle r={radius} at ({cx},{cy})"))
            }
            "bezier" => {
                if parts.len() < 9 {
                    return Err(
                        "Usage: :canvas bezier <x0> <y0> <x1> <y1> <x2> <y2> <x3> <y3>"
                            .to_string(),
                    );
                }
                let p0 = (
                    parts[1].parse().unwrap_or(0.0),
                    parts[2].parse().unwrap_or(0.0),
                );
                let p1 = (
                    parts[3].parse().unwrap_or(10.0),
                    parts[4].parse().unwrap_or(0.0),
                );
                let p2 = (
                    parts[5].parse().unwrap_or(20.0),
                    parts[6].parse().unwrap_or(20.0),
                );
                let p3 = (
                    parts[7].parse().unwrap_or(30.0),
                    parts[8].parse().unwrap_or(20.0),
                );
                self.active = true;
                self.primitives.push(VectorPrimitive::CubicBezier {
                    p0,
                    p1,
                    p2,
                    p3,
                    rgb: (85, 255, 85),
                });
                Ok("CANVAS: added cubic Bezier curve".to_string())
            }
            "sprite" => {
                let id = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                let x = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(4.0);
                let y = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(4.0);
                let vx = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
                let vy = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let w = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(4);
                let h = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(4);
                self.active = true;
                let mut sp = Sprite2D::new_solid(id, format!("sprite-{id}"), x, y, w, h, (255, 176, 0));
                sp.vx = vx;
                sp.vy = vy;
                if let Some(existing) = self.sprites.iter_mut().find(|s| s.id == id) {
                    *existing = sp;
                } else {
                    self.sprites.push(sp);
                }
                Ok(format!("CANVAS: spawned sprite #{id} at ({x},{y}) v=({vx},{vy})"))
            }
            "step" => {
                let dt = parts.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                let collisions = self.step_sprites(dt);
                Ok(format!(
                    "CANVAS: stepped {} sprite(s) by dt={dt} (collisions={})",
                    self.sprites.len(),
                    collisions.len()
                ))
            }
            "plot" => {
                let kind_name = parts.get(1).copied().unwrap_or("line");
                let vals: Vec<f32> = parts
                    .iter()
                    .skip(2)
                    .flat_map(|s| s.split(','))
                    .filter_map(|v| v.parse::<f32>().ok())
                    .collect();
                let data = if vals.is_empty() {
                    vec![10.0, 25.0, 18.0, 42.0, 35.0, 60.0, 52.0, 80.0]
                } else {
                    vals
                };
                self.active = true;
                let kind = match kind_name {
                    "bar" => PlotKind::Bar(data),
                    "scatter" => PlotKind::Scatter(
                        data.iter()
                            .enumerate()
                            .map(|(i, &y)| (i as f32, y))
                            .collect(),
                    ),
                    "candlestick" | "ohlc" => {
                        let bars = data
                            .chunks(4)
                            .map(|c| CandlestickBar {
                                open: *c.first().unwrap_or(&10.0),
                                high: *c.get(1).unwrap_or(&20.0),
                                low: *c.get(2).unwrap_or(&5.0),
                                close: *c.get(3).unwrap_or(&15.0),
                            })
                            .collect();
                        PlotKind::Candlestick(bars)
                    }
                    "heatmap" => {
                        let cols = (data.len() as f32).sqrt().ceil().max(1.0) as usize;
                        let rows = data.len().div_ceil(cols).max(1);
                        PlotKind::Heatmap {
                            rows,
                            cols,
                            values: data,
                        }
                    }
                    _ => PlotKind::Line(data),
                };
                self.plots.push(DeclarativePlot {
                    title: format!("Plot ({kind_name})"),
                    kind,
                    rgb: (85, 255, 180),
                });
                Ok(format!("CANVAS: added declarative {kind_name} plot"))
            }
            "image" => {
                let w = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(24);
                let h = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(16);
                self.active = true;
                self.image_view = Some(InteractiveImageView::new_test_pattern(w, h));
                Ok(format!("CANVAS: loaded {w}x{h} interactive image viewer"))
            }
            "pan" => {
                let dx = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let dy = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                if let Some(img) = &mut self.image_view {
                    img.pan_x += dx;
                    img.pan_y += dy;
                    Ok(format!("CANVAS IMAGE PAN: ({}, {})", img.pan_x, img.pan_y))
                } else {
                    Err("No active canvas image to pan".to_string())
                }
            }
            "zoom" => {
                let z = parts.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(1.0);
                if let Some(img) = &mut self.image_view {
                    img.zoom = z.clamp(0.25, 8.0);
                    Ok(format!("CANVAS IMAGE ZOOM: {:.2}x", img.zoom))
                } else {
                    Err("No active canvas image to zoom".to_string())
                }
            }
            other => Err(format!(
                "Unknown :canvas subcommand '{other}'. Usage: :canvas <tek|mode|line|circle|bezier|sprite|step|plot|image|pan|zoom|clear>"
            )),
        }
    }

    /// Scans raw PTY output bytes for in-band `OSC 1337;Canvas=<command>\x07` (or `ST` `\x1b\\`)
    /// sequences and applies them to `self` and `tektronix` (`CANV-01`).
    pub fn scan_osc_canvas_sequences(&mut self, tektronix: &mut TektronixState, bytes: &[u8]) {
        const PREFIX: &[u8] = b"\x1b]1337;Canvas=";
        let mut i = 0;
        while i + PREFIX.len() <= bytes.len() {
            if &bytes[i..i + PREFIX.len()] == PREFIX {
                let start = i + PREFIX.len();
                let mut end = start;
                while end < bytes.len() {
                    if bytes[end] == 0x07
                        || (bytes[end] == 0x1b && end + 1 < bytes.len() && bytes[end + 1] == b'\\')
                    {
                        break;
                    }
                    end += 1;
                }
                if end < bytes.len()
                    && let Ok(cmd) = std::str::from_utf8(&bytes[start..end])
                {
                    let _ = self.execute_command(tektronix, cmd);
                }
                i = end + 1;
            } else {
                i += 1;
            }
        }
    }
}

impl crate::pane::CompositePane {
    pub fn canvas_scene(&self) -> &PaneCanvasScene {
        &self.canvas_scene
    }

    pub fn canvas_scene_mut(&mut self) -> &mut PaneCanvasScene {
        &mut self.canvas_scene
    }

    /// Executes a `:canvas <subcommand>` on this pane, updating `canvas_scene`, `vt.tektronix`, and `status_message` (`TEK-01`, `CANV-01`).
    pub fn execute_canvas_command(&mut self, cmd: &str) -> std::result::Result<String, String> {
        let (scene, vt) = (&mut self.canvas_scene, self.vt.tektronix_mut());
        match scene.execute_command(vt, cmd) {
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
