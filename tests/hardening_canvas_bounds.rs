//! Regression tests: in-band `OSC 1337;Canvas=<cmd>` (untrusted pane output) must not be able to
//! exhaust memory or hang the UI thread through canvas sizes, counts or coordinates.

use std::time::{Duration, Instant};

use bilterm::config::BiltermConfig;
use bilterm::pane::canvas_scene::{
    MAX_CANVAS_DIM, MAX_CANVAS_PIXELS, MAX_SCENE_PLOT_POINTS, MAX_SCENE_PLOTS,
    MAX_SCENE_PRIMITIVES, MAX_SCENE_SPRITES,
};
use bilterm::pane::{
    CandlestickBar, CompositePane, DeclarativePlot, InteractiveImageView, PlotKind, Sprite2D,
    VectorPrimitive,
};
use bilterm::ui::cursor::SpotlightMode;
use bilterm::ui::pixel_canvas::PixelCanvasBuffer;
use bilterm::ui::render_graphical_canvas;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Generous wall-clock bound: the pre-fix code needed minutes (or panicked) on these inputs.
const BOUNDED: Duration = Duration::from_secs(10);

fn pane() -> CompositePane {
    CompositePane::new_headless(1, 24, 80)
}

fn osc(cmd: &str) -> Vec<u8> {
    format!("\x1b]1337;Canvas={cmd}\x07").into_bytes()
}

/// Renders the pane's canvas into a `40x12` cell area and returns the cell symbols row by row.
fn render_cells(pane: &CompositePane, cfg: &BiltermConfig) -> String {
    let area = Rect::new(0, 0, 40, 12);
    let mut buf = Buffer::empty(area);
    render_graphical_canvas(pane, area, cfg, &mut buf);
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn oversize_image_and_sprite_commands_are_rejected_quickly_without_allocating() {
    let started = Instant::now();
    let mut p = pane();
    for cmd in [
        "image 65535 65535",
        "image 4097 8",
        "image 8 4097",
        "image 4096 4096", // within the per-dimension limit, above the total-pixel limit
    ] {
        let err = p.execute_canvas_command(cmd).unwrap_err();
        assert!(err.contains("exceeds the limit"), "{cmd}: {err}");
        assert!(p.canvas_scene().image_view.is_none(), "{cmd} must not allocate an image");
    }
    for cmd in ["sprite 1 0 0 0 0 65535 65535", "sprite 1 0 0 0 0 4097 1"] {
        let err = p.execute_canvas_command(cmd).unwrap_err();
        assert!(err.contains("exceeds the limit"), "{cmd}: {err}");
        assert!(p.canvas_scene().sprites.is_empty(), "{cmd} must not allocate a sprite");
    }
    // The same commands arriving in-band from pane output are dropped the same way.
    p.process_pty_output(&osc("image 65535 65535"));
    p.process_pty_output(&osc("sprite 7 0 0 0 0 65535 65535"));
    assert!(p.canvas_scene().image_view.is_none());
    assert!(p.canvas_scene().sprites.is_empty());

    // Normal sizes still work, and the constructors clamp for any other caller.
    p.execute_canvas_command("image 64 48").unwrap();
    let img = p.canvas_scene().image_view.as_ref().unwrap();
    assert_eq!((img.width, img.height, img.rgba.len()), (64, 48, 64 * 48 * 4));
    let sp = Sprite2D::new_solid(1, "big", 0.0, 0.0, 65535, 65535, (1, 2, 3));
    assert!(sp.width <= MAX_CANVAS_DIM && sp.height <= MAX_CANVAS_DIM);
    assert!(sp.frames[0].len() <= MAX_CANVAS_PIXELS);
    assert!(started.elapsed() < BOUNDED, "took {:?}", started.elapsed());
}

#[test]
fn sprites_primitives_and_plot_points_are_capped_per_scene() {
    let mut p = pane();
    for id in 0..MAX_SCENE_SPRITES {
        p.execute_canvas_command(&format!("sprite {id} 1 1 0 0 2 2")).unwrap();
    }
    let err = p.execute_canvas_command("sprite 999999 1 1 0 0 2 2").unwrap_err();
    assert!(err.contains("sprite limit"), "{err}");
    p.execute_canvas_command("sprite 5 9 9 0 0 3 3").expect("replacing an existing id is allowed");
    assert_eq!(p.canvas_scene().sprites.len(), MAX_SCENE_SPRITES);

    // Total sprite pixels are budgeted too (one 4000x4000 sprite fits, a second does not).
    p.execute_canvas_command("clear").unwrap();
    p.execute_canvas_command("sprite 1 0 0 0 0 4000 4000").unwrap();
    let err = p.execute_canvas_command("sprite 2 0 0 0 0 4000 4000").unwrap_err();
    assert!(err.contains("pixels in total"), "{err}");
    assert_eq!(p.canvas_scene().sprites.len(), 1);

    p.execute_canvas_command("clear").unwrap();
    for i in 0..MAX_SCENE_PRIMITIVES {
        let cmd = if i % 2 == 0 { "line 0 0 5 5" } else { "circle 4 4 2" };
        p.execute_canvas_command(cmd).unwrap();
    }
    for cmd in ["line 0 0 5 5", "circle 4 4 2", "bezier 0 0 1 1 2 2 3 3"] {
        let err = p.execute_canvas_command(cmd).unwrap_err();
        assert!(err.contains("primitive limit"), "{cmd}: {err}");
    }
    p.execute_canvas_command("demo").unwrap();
    assert_eq!(p.canvas_scene().primitives.len(), MAX_SCENE_PRIMITIVES);

    p.execute_canvas_command("clear").unwrap();
    let too_many = vec!["1"; MAX_SCENE_PLOT_POINTS + 1].join(",");
    let err = p.execute_canvas_command(&format!("plot line {too_many}")).unwrap_err();
    assert!(err.contains("plot limit"), "{err}");
    assert!(p.canvas_scene().plots.is_empty());
    for _ in 0..MAX_SCENE_PLOTS {
        p.execute_canvas_command("plot bar 1,2,3").unwrap();
    }
    let err = p.execute_canvas_command("plot bar 1,2,3").unwrap_err();
    assert!(err.contains("plot limit"), "{err}");
    assert_eq!(p.canvas_scene().plots.len(), MAX_SCENE_PLOTS);
}

#[test]
fn non_finite_and_negative_arguments_are_rejected_not_cast() {
    let mut p = pane();
    for cmd in [
        "line nan 0 5 5",
        "line 0 0 inf 5",
        "line 0 0 5 1e39",
        "circle nan 4 2",
        "circle 4 4 inf",
        "circle 4 4 -3",
        "bezier 0 0 1 1 2 2 3 -inf",
        "sprite 1 nan 0",
        "sprite 1 0 0 inf 0",
        "plot line 1,nan,3",
        "plot bar inf",
        "step nan",
    ] {
        assert!(p.execute_canvas_command(cmd).is_err(), "`{cmd}` must be rejected");
    }
    let scene = p.canvas_scene();
    assert!(scene.primitives.is_empty() && scene.sprites.is_empty() && scene.plots.is_empty());

    p.execute_canvas_command("image 8 8").unwrap();
    assert!(p.execute_canvas_command("zoom nan").is_err());
    assert!(p.canvas_scene().image_view.as_ref().unwrap().zoom.is_finite());
    // Repeated pans saturate instead of overflowing.
    p.execute_canvas_command("pan 2147483647 -2147483648").unwrap();
    p.execute_canvas_command("pan 2147483647 -2147483648").unwrap();
    let img = p.canvas_scene().image_view.as_ref().unwrap();
    assert_eq!((img.pan_x, img.pan_y), (i32::MAX, i32::MIN));
}

/// Hostile primitives placed straight into the scene (as a restored session or API caller could).
fn hostile_scene(p: &mut CompositePane) {
    let rgb = (255, 255, 255);
    let scene = p.canvas_scene_mut();
    scene.active = true;
    scene.primitives.clear();
    scene.primitives.extend([
        VectorPrimitive::Line { x0: 0.0, y0: 0.0, x1: 2e9, y1: 2e9, rgb },
        VectorPrimitive::Line { x0: -3e38, y0: -3e38, x1: 3e38, y1: 3e38, rgb },
        VectorPrimitive::Line { x0: f32::NAN, y0: 0.0, x1: f32::INFINITY, y1: 9.0, rgb },
        VectorPrimitive::Circle { cx: 0.0, cy: 0.0, radius: 1e9, rgb, filled: false },
        VectorPrimitive::Circle { cx: 3e9, cy: -3e9, radius: 3e38, rgb, filled: true },
        VectorPrimitive::Circle { cx: f32::NAN, cy: 1.0, radius: f32::INFINITY, rgb, filled: true },
        VectorPrimitive::RectShape { x: -2e9, y: -2e9, width: 4e9, height: 4e9, rgb, filled: true },
        VectorPrimitive::RectShape { x: 2e9, y: 2e9, width: 3e38, height: 3e38, rgb, filled: false },
        VectorPrimitive::Polygon { vertices: vec![(-1e30, 0.0), (1e30, 5.0), (0.0, 1e30)], rgb },
        VectorPrimitive::CubicBezier {
            p0: (-3e38, -3e38),
            p1: (3e38, 3e38),
            p2: (-3e38, 3e38),
            p3: (3e38, -3e38),
            rgb,
        },
    ]);
    scene.plots = vec![
        DeclarativePlot {
            title: "c".into(),
            kind: PlotKind::Candlestick(vec![CandlestickBar {
                open: -1e30,
                high: 1.0,
                low: 0.0,
                close: 1e30,
            }]),
            rgb,
        },
        DeclarativePlot {
            title: "s".into(),
            kind: PlotKind::Scatter(vec![(3e38, 3e38), (-3e38, f32::NAN)]),
            rgb,
        },
        DeclarativePlot {
            title: "h".into(),
            kind: PlotKind::Heatmap { rows: usize::MAX / 4, cols: 3, values: vec![1.0; 9] },
            rgb,
        },
    ];
    let mut far = Sprite2D::new_solid(1, "far", f32::INFINITY, f32::NAN, 4000, 4000, rgb);
    far.vx = 3e38;
    let near = Sprite2D::new_solid(2, "near", 2_147_483_000.0, -5.0, 4000, 4000, rgb);
    scene.sprites = vec![far, near];
    let mut img = InteractiveImageView::new_test_pattern(1500, 1500);
    img.zoom = 8.0;
    img.pan_x = i32::MIN;
    scene.image_view = Some(img);
}

#[test]
fn huge_circles_lines_and_blits_render_in_bounded_time_on_both_rasterizers() {
    let cfg = BiltermConfig::default();
    let lens = BiltermConfig {
        focal_zoom_mode: SpotlightMode::Circle,
        focal_zoom_pct: 200,
        ..BiltermConfig::default()
    };

    // The exact in-band commands from the audit, on the default Braille sub-cell rasterizer.
    let mut p = pane();
    p.process_pty_output(&osc("circle 0 0 1e9"));
    p.process_pty_output(&osc("line 0 0 2e9 2e9"));
    assert_eq!(p.canvas_scene().primitives.len(), 2);
    let started = Instant::now();
    let cells = render_cells(&p, &cfg);
    assert!(started.elapsed() < BOUNDED, "sub-cell render took {:?}", started.elapsed());
    // The clipped diagonal is still drawn where it crosses the canvas (2x4 sub-pixels per cell).
    let rows: Vec<Vec<char>> = cells.lines().map(|l| l.chars().collect()).collect();
    for (col, row) in [(0usize, 0usize), (10, 5), (22, 11)] {
        assert_ne!(rows[row][col], ' ', "diagonal missing at cell ({col},{row}):\n{cells}");
    }
    assert_eq!(rows[0][39], ' ', "nothing may be drawn off the diagonal:\n{cells}");

    // Every primitive kind with hostile coordinates, on each sub-cell mode and with the focal lens.
    hostile_scene(&mut p);
    for mode in ["braille", "sextant", "quadrant", "halfblock", "octant"] {
        p.execute_canvas_command(&format!("mode {mode}")).unwrap();
        for cfg in [&cfg, &lens] {
            let started = Instant::now();
            let _ = render_cells(&p, cfg);
            assert!(started.elapsed() < BOUNDED, "{mode} render took {:?}", started.elapsed());
        }
    }

    // The pixel rasterizer's primitives directly.
    let started = Instant::now();
    let mut buf = PixelCanvasBuffer::new(64, 48, [0, 0, 0, 0]);
    buf.draw_line_aa(0.0, 0.0, 2e9, 2e9, (255, 0, 0), 1.0);
    assert_eq!(buf.pixel_at(20, 20), [255, 0, 0, 255], "clipped diagonal must still be drawn");
    assert_eq!(buf.pixel_at(40, 5)[3], 0);
    let before = buf.clone();
    buf.draw_line_aa(f32::NAN, 0.0, 9.0, 9.0, (9, 9, 9), 1.0);
    buf.draw_line_aa(0.0, f32::NEG_INFINITY, 9.0, f32::INFINITY, (9, 9, 9), 1.0);
    buf.draw_line_aa(1e9, 1e9, 2e9, 3e38, (9, 9, 9), 1.0);
    buf.draw_circle_aa(f32::NAN, 5.0, 3.0, true, (9, 9, 9));
    buf.draw_circle_aa(5.0, 5.0, f32::INFINITY, true, (9, 9, 9));
    buf.draw_circle_aa(3e9, 3e9, 10.0, true, (9, 9, 9));
    buf.draw_rect_aa(f32::NAN, 0.0, 5.0, 5.0, true, (9, 9, 9), 1.0);
    assert_eq!(buf, before, "non-finite and off-canvas shapes must draw nothing");
    buf.draw_line_aa(-3e38, 10.0, 3e38, 10.0, (0, 255, 0), 1.0);
    assert_eq!(buf.pixel_at(0, 10), [0, 255, 0, 255]);
    assert_eq!(buf.pixel_at(63, 10), [0, 255, 0, 255]);
    buf.draw_circle_aa(0.0, 0.0, 1e9, false, (9, 9, 9));
    assert!(started.elapsed() < BOUNDED, "pixel raster took {:?}", started.elapsed());
}

/// The Xiaolin Wu loop exactly as it was before clipping was added.
fn reference_line_aa(
    buf: &mut PixelCanvasBuffer,
    (mut x0, mut y0): (f32, f32),
    (mut x1, mut y1): (f32, f32),
    rgb: (u8, u8, u8),
) {
    let steep = (y1 - y0).abs() > (x1 - x0).abs();
    if steep {
        std::mem::swap(&mut x0, &mut y0);
        std::mem::swap(&mut x1, &mut y1);
    }
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let gradient = if dx.abs() < 1e-5 { 1.0 } else { dy / dx };
    let (x_start, x_end) = (x0.round() as i32, x1.round() as i32);
    let mut intery = y0 + gradient * ((x_start as f32) - x0);
    for x in x_start..=x_end {
        let y_int = intery.floor() as i32;
        let fpart = intery - (y_int as f32);
        if steep {
            buf.blend_pixel(y_int, x, rgb, 1.0 - fpart);
            buf.blend_pixel(y_int + 1, x, rgb, fpart);
        } else {
            buf.blend_pixel(x, y_int, rgb, 1.0 - fpart);
            buf.blend_pixel(x, y_int + 1, rgb, fpart);
        }
        intery += gradient;
    }
}

/// The SDF circle scan exactly as it was before (bounding box clamped to the canvas).
fn reference_circle_aa(
    buf: &mut PixelCanvasBuffer,
    (cx, cy): (f32, f32),
    radius: f32,
    filled: bool,
    rgb: (u8, u8, u8),
) {
    let r = radius.max(1.0);
    for y in 0..buf.height as i32 {
        for x in 0..buf.width as i32 {
            if (x as f32) < (cx - r - 2.0).floor()
                || (x as f32) > (cx + r + 2.0).ceil()
                || (y as f32) < (cy - r - 2.0).floor()
                || (y as f32) > (cy + r + 2.0).ceil()
            {
                continue;
            }
            let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let cov = if filled {
                (r + 0.5 - dist).clamp(0.0, 1.0)
            } else {
                (1.2 - (dist - r).abs()).clamp(0.0, 1.0)
            };
            if cov > 0.0 {
                buf.blend_pixel(x, y, rgb, cov);
            }
        }
    }
}

#[test]
fn normal_pixel_lines_and_circles_are_pixel_identical_to_the_unclipped_algorithms() {
    let lines = [
        ((3.0, 4.5), (60.2, 40.0)),
        ((50.0, 2.0), (8.0, 45.0)),
        ((10.0, 10.0), (10.0, 40.0)),
        ((-20.5, -10.0), (90.0, 70.0)), // partly off-canvas, inside the one-extent margin
        ((63.0, -40.0), (-50.0, 90.0)),
    ];
    let circles = [
        ((30.0, 20.0), 9.0, false),
        ((45.5, 30.25), 7.0, true),
        ((2.0, 46.0), 12.0, false), // partly off-canvas
        ((70.0, 10.0), 15.0, true),
    ];
    let mut got = PixelCanvasBuffer::new(64, 48, [0, 0, 0, 0]);
    let mut want = got.clone();
    for (i, (a, b)) in lines.into_iter().enumerate() {
        let rgb = (200, 40 * i as u8, 90);
        got.draw_line_aa(a.0, a.1, b.0, b.1, rgb, 1.0);
        reference_line_aa(&mut want, a, b, rgb);
    }
    for (i, (c, r, filled)) in circles.into_iter().enumerate() {
        let rgb = (30, 220, 50 * i as u8);
        got.draw_circle_aa(c.0, c.1, r, filled, rgb);
        reference_circle_aa(&mut want, c, r, filled, rgb);
    }
    assert!(got.rgba.iter().any(|&b| b != 0), "the scene must draw something");
    assert_eq!(got, want);
}

/// Sub-cell output captured from the rasterizer before clipping was added.
const GOLDEN_IN_BOUNDS: &str = "\
\x20\u{2840}                           \u{2880}\u{2804}         \n\
\x20\u{2808}\u{2811}\u{2822}\u{2884}\u{2840}                     \u{2860}\u{2814}\u{2801}          \n\
\x20    \u{2808}\u{2811}\u{2824}\u{28c0}    \u{2880}\u{28c0}\u{28c0}\u{28c0}        \u{2860}\u{280a}             \n\
\x20        \u{2809}\u{2812}\u{28e4}\u{285e}\u{2809}  \u{2808}\u{2819}\u{28a6}\u{2840}  \u{2880}\u{2814}\u{280a}               \n\
\x20         \u{28b0}\u{2803}\u{2808}\u{2811}\u{2822}\u{2884}\u{2840}  \u{28b3}\u{2880}\u{2814}\u{2801}  \u{2880}\u{28e4}\u{28f4}\u{28e4}\u{28c4}          \n\
\x20         \u{28b8}\u{2840}    \u{2808}\u{2811}\u{2864}\u{28fa}\u{2801}   \u{28b0}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28f7}         \n\
\x20          \u{2833}\u{28c4}  \u{2880}\u{2860}\u{288a}\u{2874}\u{2803}\u{2809}\u{2812}\u{2824}\u{28c0}\u{28b9}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{2801}        \n\
\x20           \u{2808}\u{2899}\u{2816}\u{2813}\u{281a}\u{2809}      \u{2811}\u{283b}\u{28bf}\u{28ff}\u{283f}\u{281f}\u{2801}         \n\
\x20          \u{2860}\u{2814}\u{2801}             \u{2808}\u{2811}\u{2822}\u{28c0}         \n\
\x20        \u{2860}\u{280a}                    \u{2809}\u{2812}\u{2824}\u{28c0}     \n\
\x20     \u{2880}\u{2814}\u{280a}                          \u{2801}    \n\
\x20     \u{2801}                                 \n\
";
const GOLDEN_PARTLY_OFF_CANVAS: &str = "\
\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{2867}                                 \n\
\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{28ff}\u{2807}                                 \n\
\u{283f}\u{283f}\u{287f}\u{283f}\u{281f}\u{2809}\u{2811}\u{2822}\u{28c0}                               \n\
\x20  \u{2880}\u{28c0}\u{2840}   \u{2809}\u{2812}\u{2884}\u{2840}                           \n\
\u{2860}\u{2812}\u{2809}\u{2801} \u{2808}\u{2809}\u{2811}\u{2812}\u{2822}\u{2824}\u{28c0}\u{2808}\u{2811}\u{2822}\u{28c0}                        \n\
\x20           \u{2809}\u{2812}\u{2824}\u{2840}\u{2809}\u{2812}\u{2884}\u{2840}                    \n\
\x20              \u{2808}\u{2811}\u{2884}\u{2840}\u{2808}\u{2811}\u{2822}\u{28c0}                 \n\
\x20                 \u{2808}\u{2812}\u{2824}\u{28c0} \u{2809}\u{2812}\u{2884}\u{2840}          \u{2880}\u{2860}\u{2814}\n\
\x20                     \u{2809}\u{2811}\u{2822}\u{2824}\u{28c8}\u{28d1}\u{2862}\u{28c0}   \u{2880}\u{28e4}\u{2836}\u{281b}\u{2809}\u{2809}\u{2811}\n\
\x20                           \u{2808}\u{2809}\u{2809}\u{281b}\u{28fd}\u{2841}      \n\
\x20                              \u{28b0}\u{2801}\u{2808}\u{2811}\u{2822}\u{28c0}   \n\
\x20                              \u{28b8}     \u{2809}\u{2812}\u{2884}\n\
";
const GOLDEN_FOCAL_LENS: &str = "\
\u{282a}\u{28e8}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2805}\u{2805}\u{2804}                           \n\
\u{2828}\u{2828}\u{2829}\u{283a}\u{28ac}\u{2868}\u{2828}\u{2828}\u{2828}\u{2828}\u{2805}\u{2805}                            \n\
\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2839}\u{282a}\u{28ac}\u{2868}\u{2805}\u{2805}                            \n\
\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{281c}\u{2823}\u{2884}\u{2840}                           \n\
\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2828}\u{2808}   \u{2808}\u{2811}\u{2822}\u{28c0}                        \n\
\x20  \u{2808}            \u{2809}\u{2812}\u{2824}\u{28c0}                    \n\
\x20      \u{28c0}\u{28c0}\u{28c0}         \u{2808}\u{2811}\u{2822}\u{28c0}                 \n\
\x20 \u{28c0}\u{2824}\u{2812}\u{280a}\u{2809}   \u{2809}\u{2809}\u{2811}\u{2812}\u{2824}\u{2824}\u{2884}\u{28c0}\u{2840}    \u{2809}\u{2832}\u{28a4}\u{28c0}          \u{2880}\u{28e0}\u{2814}\n\
\u{280a}\u{2809}                \u{2808}\u{2809}\u{2813}\u{2822}\u{2824}\u{28f0}\u{2872}\u{2824}\u{28cc}\u{28d1}\u{2862}\u{28c4}\u{2840}  \u{2880}\u{28e4}\u{2876}\u{281f}\u{2809}\u{2809}\u{2811}\n\
\x20                      \u{2808}\u{2801}   \u{2808}\u{2809}\u{2809}\u{281b}\u{28fd}\u{28c9}      \n\
\x20                              \u{28b0}\u{2801}\u{2808}\u{2811}\u{2822}\u{28c4}\u{2840}  \n\
\x20                              \u{28b8}     \u{2809}\u{2812}\u{28a4}\n\
";

fn subcell_scenes() -> [String; 3] {
    let lens = BiltermConfig {
        focal_zoom_mode: SpotlightMode::Circle,
        focal_zoom_pct: 200,
        ..BiltermConfig::default()
    };
    let mut inside = pane();
    for cmd in ["line 2 3 70 40", "line 60 2 12 44", "circle 30 20 9", "circle 55 24 7 fill"] {
        inside.execute_canvas_command(cmd).unwrap();
    }
    let mut partly = pane();
    for cmd in [
        "line -20 -10 120 70",
        "circle 75 44 12",
        "circle 4 2 9 fill",
        "bezier -10 30 20 -20 50 80 95 10",
    ] {
        partly.execute_canvas_command(cmd).unwrap();
    }
    [
        render_cells(&inside, &BiltermConfig::default()),
        render_cells(&partly, &BiltermConfig::default()),
        render_cells(&partly, &lens),
    ]
}

#[test]
fn normal_subcell_lines_and_circles_are_pixel_identical_to_the_pre_fix_golden() {
    let [inside, partly, lens] = subcell_scenes();
    assert_eq!(inside, GOLDEN_IN_BOUNDS, "in-bounds scene changed:\n{inside}");
    assert_eq!(partly, GOLDEN_PARTLY_OFF_CANVAS, "partly off-canvas scene changed:\n{partly}");
    assert_eq!(lens, GOLDEN_FOCAL_LENS, "focal-lens scene changed:\n{lens}");
}
