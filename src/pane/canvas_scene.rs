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

/// Sub-cell character rasterization mode for TUI rendering (`CANV-01`, `CANV-02`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasSubcellMode {
    /// Unicode `2×4` Braille patterns (`U+2800..U+28FF`, 8 sub-pixels per cell).
    #[default]
    Braille,
    /// Unicode `2×3` Legacy Computing Sextants (`U+1FB00..U+1FB3B`, 6 sub-pixels per cell).
    Sextant,
    /// Unicode `2×2` Block Quadrants (`▘▝▖▗▚▞▛▜▙▟█`, 4 sub-pixels per cell).
    Quadrant,
    /// Unicode `1×2` Half-Blocks (`▀` / `▄` with independent 24-bit TrueColor fg=top, bg=bottom pixel).
    HalfBlock,
    /// Unicode `2×4` 2-Color TrueColor Partitioned Octants (`8` sub-pixels per cell with dual `fg`+`bg` 24-bit RGB).
    Octant,
}

impl CanvasSubcellMode {
    /// Sub-pixel multiplier `(sub_cols_per_cell, sub_rows_per_cell)`.
    #[inline]
    pub const fn subcell_grid(self) -> (u16, u16) {
        match self {
            Self::Braille | Self::Octant => (2, 4),
            Self::Sextant => (2, 3),
            Self::Quadrant => (2, 2),
            Self::HalfBlock => (1, 2),
        }
    }

    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "braille" | "2x4" => Some(Self::Braille),
            "octant" | "octants" | "2x4-tc" | "ultra" => Some(Self::Octant),
            "sextant" | "sextants" | "2x3" => Some(Self::Sextant),
            "quadrant" | "quadrants" | "2x2" | "block" => Some(Self::Quadrant),
            "halfblock" | "half-block" | "half" | "1x2" | "truecolor" => Some(Self::HalfBlock),
            _ => None,
        }
    }
}

/// Adaptive multi-transport graphics rendering backend across Native GUI, Pure SSH, `tmux`, and TUI (`SSH-GFX-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasRenderBackend {
    /// Automatically negotiate optimal transport from host terminal & SSH environment.
    #[default]
    Auto,
    /// Kitty Graphics Protocol (`APC G`, `\x1b_Ga=T,f=32...\x1b\\`) — Ghostty, Kitty, WezTerm, Konsole (works over SSH!).
    Kitty,
    /// DEC Sixel Pixel Graphics (`DCS q`, `\x1bP0;1;q...\x1b\\`) — foot, xterm, Windows Terminal 1.22+, mlterm (works over SSH!).
    Sixel,
    /// iTerm2 Inline Images (`OSC 1337;File=inline=1:...`) — iTerm2, WezTerm, Warp (works over SSH!).
    ITerm2,
    /// 24-bit TrueColor `1×2` Half-Block (`▀`) bitmap cells — universal over any SSH / `tmux` terminal.
    HalfBlock,
    /// 24-bit TrueColor `2×4` Octant (`8` sub-pixels/cell with dual `fg`+`bg` TrueColor).
    Octant,
    /// Unicode `2×4` Braille (`U+2800..U+28FF`) vector sub-cells.
    Braille,
    /// Unicode `2×3` Legacy Computing Sextants (`U+1FB00..U+1FB3B`).
    Sextant,
    /// Unicode `2×2` Quadrant blocks.
    Quadrant,
}

impl CanvasRenderBackend {
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "kitty" | "ghostty" => Some(Self::Kitty),
            "sixel" => Some(Self::Sixel),
            "iterm" | "iterm2" | "osc1337" => Some(Self::ITerm2),
            "halfblock" | "half-block" | "half" | "1x2" | "truecolor" => Some(Self::HalfBlock),
            "octant" | "octants" | "ultra" => Some(Self::Octant),
            "braille" | "2x4" => Some(Self::Braille),
            "sextant" | "2x3" => Some(Self::Sextant),
            "quadrant" | "2x2" => Some(Self::Quadrant),
            _ => None,
        }
    }

    /// Resolves `Self::Auto` by inspecting terminal & SSH environment variables (`SSH-GFX-01`).
    pub fn resolve_auto(self) -> Self {
        if self != Self::Auto {
            return self;
        }
        Self::detect_from_vars(
            std::env::var("TERM").ok().as_deref(),
            std::env::var("TERM_PROGRAM").ok().as_deref(),
            std::env::var("LC_TERMINAL").ok().as_deref(),
            std::env::var("KITTY_WINDOW_ID").ok().as_deref(),
            std::env::var("GHOSTTY_RESOURCES_DIR").ok().as_deref(),
            std::env::var("WEZTERM_EXECUTABLE").ok().as_deref(),
            std::env::var("WT_SESSION").ok().as_deref(),
        )
    }

    /// Pure, deterministic capability resolver for local and SSH sessions (`SSH-GFX-01`).
    pub fn detect_from_vars(
        term: Option<&str>,
        term_program: Option<&str>,
        lc_terminal: Option<&str>,
        kitty_id: Option<&str>,
        ghostty_dir: Option<&str>,
        wezterm_exe: Option<&str>,
        wt_session: Option<&str>,
    ) -> Self {
        let term_l = term.unwrap_or("").to_ascii_lowercase();
        let prog_l = term_program.unwrap_or("").to_ascii_lowercase();
        let lc_l = lc_terminal.unwrap_or("").to_ascii_lowercase();

        if kitty_id.is_some_and(|s| !s.is_empty())
            || ghostty_dir.is_some_and(|s| !s.is_empty())
            || term_l.contains("kitty")
            || term_l.contains("ghostty")
            || prog_l.contains("ghostty")
            || prog_l.contains("kitty")
        {
            return Self::Kitty;
        }
        if wezterm_exe.is_some_and(|s| !s.is_empty()) || prog_l.contains("wezterm") {
            return Self::Kitty;
        }
        if prog_l.contains("iterm") || lc_l.contains("iterm") || prog_l.contains("warp") {
            return Self::ITerm2;
        }
        if wt_session.is_some_and(|s| !s.is_empty())
            || term_l.contains("foot")
            || term_l.contains("sixel")
            || term_l.contains("mlterm")
            || prog_l.contains("foot")
        {
            return Self::Sixel;
        }
        if term_l.contains("256color")
            || term_l.contains("direct")
            || term_l.contains("truecolor")
            || term_l.contains("alacritty")
        {
            return Self::HalfBlock;
        }
        Self::Braille
    }

    /// Returns whether this backend emits raw pixel escape sequences (`Kitty`, `Sixel`, `ITerm2`).
    pub const fn is_pixel_protocol(self) -> bool {
        matches!(self, Self::Kitty | Self::Sixel | Self::ITerm2)
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
        (idx + 3 < self.rgba.len()).then(|| {
            [self.rgba[idx], self.rgba[idx + 1], self.rgba[idx + 2], self.rgba[idx + 3]]
        })
    }
}

/// Complete retained scene-graph for a `CompositePane` (`CANV-01`, `CANV-02`, `CANV-03`, `SSH-GFX-01`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PaneCanvasScene {
    pub active: bool,
    pub subcell_mode: CanvasSubcellMode,
    #[serde(default)]
    pub render_backend: CanvasRenderBackend,
    #[serde(default)]
    pub resolution: crate::pane::typography::CanvasResolution,
    #[serde(skip)]
    pub generation: u64,
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
        self.generation = self.generation.wrapping_add(1);
        self.primitives.clear();
        self.sprites.clear();
        self.plots.clear();
        self.image_view = None;
    }

    /// Steps all sprite physics by `dt` and returns pairs of colliding sprite `(id_a, id_b)` (`CANV-01`).
    pub fn step_sprites(&mut self, dt: f32) -> Vec<(u32, u32)> {
        self.generation = self.generation.wrapping_add(1);
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
        self.generation = self.generation.wrapping_add(1);
        let parts: Vec<&str> = raw.split_whitespace().collect();
        match parts.first().copied().unwrap_or("status") {
            "status" => Ok(format!(
                "CANVAS: backend={:?}(eff={:?}) subcell={:?} primitives={} sprites={} plots={} tek_vecs={}",
                self.render_backend, self.render_backend.resolve_auto(), self.subcell_mode,
                self.primitives.len(), self.sprites.len(), self.plots.len(), tektronix.segments.len()
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
                    return Err("Usage: :canvas mode <octant|sextant|quadrant|halfblock|braille>".to_string());
                };
                let mode = CanvasSubcellMode::from_name(mode_str).ok_or_else(|| {
                    "Usage: :canvas mode <octant|sextant|quadrant|halfblock|braille>".to_string()
                })?;
                self.subcell_mode = mode;
                Ok(format!("CANVAS SUBCELL MODE: {mode:?} ({}x{} sub-pixels/cell)", mode.subcell_grid().0, mode.subcell_grid().1))
            }
            "res" | "resolution" | "dpi" => {
                let Some(res_str) = parts.get(1) else {
                    let (px_c, px_r) = self.resolution.resolve_px_per_cell(80, 24);
                    return Ok(format!(
                        "CANVAS RESOLUTION: {} ({}x{} px at 80x24)",
                        self.resolution.label(),
                        80 * px_c,
                        24 * px_r
                    ));
                };
                let res = crate::pane::typography::CanvasResolution::from_name(res_str).ok_or_else(|| {
                    "Usage: :canvas res <auto|1x|2x|4x|hd|720p|fhd|1080p|4k|2160p|<WxH>>".to_string()
                })?;
                self.resolution = res;
                let (px_c, px_r) = self.resolution.resolve_px_per_cell(80, 24);
                Ok(format!(
                    "CANVAS RESOLUTION: {} ({}x{} px at 80x24)",
                    self.resolution.label(),
                    80 * px_c,
                    24 * px_r
                ))
            }
            "backend" | "transport" => {
                let Some(b_str) = parts.get(1) else {
                    return Ok(format!(
                        "CANVAS BACKEND: {:?} (effective: {:?})",
                        self.render_backend, self.render_backend.resolve_auto()
                    ));
                };
                let backend = CanvasRenderBackend::from_name(b_str).ok_or_else(|| {
                    "Usage: :canvas backend <auto|kitty|sixel|iterm2|octant|halfblock|braille|sextant|quadrant>".to_string()
                })?;
                self.render_backend = backend;
                match backend {
                    CanvasRenderBackend::HalfBlock => self.subcell_mode = CanvasSubcellMode::HalfBlock,
                    CanvasRenderBackend::Octant => self.subcell_mode = CanvasSubcellMode::Octant,
                    CanvasRenderBackend::Braille => self.subcell_mode = CanvasSubcellMode::Braille,
                    CanvasRenderBackend::Sextant => self.subcell_mode = CanvasSubcellMode::Sextant,
                    CanvasRenderBackend::Quadrant => self.subcell_mode = CanvasSubcellMode::Quadrant,
                    _ => {}
                }
                Ok(format!("CANVAS BACKEND: {backend:?} (effective: {:?})", backend.resolve_auto()))
            }
            "hires" | "demo-hires" => {
                self.clear();
                self.active = true;
                self.subcell_mode = CanvasSubcellMode::HalfBlock;
                let wave: Vec<f32> = (0..32).map(|i| 50.0 + 38.0 * ((i as f32) * 0.32).sin()).collect();
                self.plots.push(DeclarativePlot {
                    title: "HiRes Sine Telemetry".to_string(),
                    kind: PlotKind::Line(wave),
                    rgb: (0, 230, 255),
                });
                self.primitives.push(VectorPrimitive::CubicBezier {
                    p0: (4.0, 28.0), p1: (22.0, 4.0), p2: (46.0, 32.0), p3: (68.0, 8.0), rgb: (255, 180, 40),
                });
                self.primitives.push(VectorPrimitive::Circle {
                    cx: 20.0, cy: 16.0, radius: 8.0, rgb: (120, 255, 140), filled: true,
                });
                self.image_view = Some(InteractiveImageView::new_test_pattern(24, 14));
                Ok("CANVAS HIRES: loaded anti-aliased TrueColor Half-Block/Pixel scene (:canvas export <file.ppm> | :canvas clear)".to_string())
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
                    cx: 24.0, cy: 20.0, radius: 10.0, rgb: (255, 255, 85), filled: false,
                });
                Ok("CANVAS DEMO: loaded Tektronix 4014 wireframe + Braille vector/plot scene (:canvas clear to reset)".to_string())
            }
            "line" => {
                if parts.len() < 5 {
                    return Err("Usage: :canvas line <x0> <y0> <x1> <y1>".to_string());
                }
                let (x0, y0) = (parts[1].parse().unwrap_or(0.0), parts[2].parse().unwrap_or(0.0));
                let (x1, y1) = (parts[3].parse().unwrap_or(20.0), parts[4].parse().unwrap_or(10.0));
                self.active = true;
                self.primitives.push(VectorPrimitive::Line { x0, y0, x1, y1, rgb: (85, 255, 255) });
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
                self.primitives.push(VectorPrimitive::Circle { cx, cy, radius, rgb: (255, 255, 85), filled });
                Ok(format!("CANVAS: added circle r={radius} at ({cx},{cy})"))
            }
            "bezier" => {
                if parts.len() < 9 {
                    return Err("Usage: :canvas bezier <x0> <y0> <x1> <y1> <x2> <y2> <x3> <y3>".to_string());
                }
                let p0 = (parts[1].parse().unwrap_or(0.0), parts[2].parse().unwrap_or(0.0));
                let p1 = (parts[3].parse().unwrap_or(10.0), parts[4].parse().unwrap_or(0.0));
                let p2 = (parts[5].parse().unwrap_or(20.0), parts[6].parse().unwrap_or(20.0));
                let p3 = (parts[7].parse().unwrap_or(30.0), parts[8].parse().unwrap_or(20.0));
                self.active = true;
                self.primitives.push(VectorPrimitive::CubicBezier { p0, p1, p2, p3, rgb: (85, 255, 85) });
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
                "Unknown :canvas subcommand '{other}'. Usage: :canvas <hires|backend|tek|mode|line|circle|bezier|sprite|step|plot|image|pan|zoom|export|encode|clear>"
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
