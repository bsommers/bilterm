//! Frame-time measurement harness for the workspace renderer (responsiveness 3b).
//!
//! Not part of the normal suite: every test here is `#[ignore]` and makes no wall-clock
//! assertion. Run (release, numbers in debug are dominated by unoptimised float maths):
//!
//! ```text
//! cargo test --release --test perf_render_frame -- --ignored --nocapture
//! BILTERM_PERF_ITERS=400 cargo test --release --test perf_render_frame -- --ignored --nocapture
//! ```
mod render_fixture;

use std::hint::black_box;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::config::BiltermConfig;
use bilterm::ui::contrast::readable_canvas_cell_style;
use bilterm::ui::{CompositePaneWidget, WorkspaceWidget};
use bilterm::wm::WindowManager;

fn iters() -> usize {
    std::env::var("BILTERM_PERF_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(200)
}

/// Median wall time of `f` over `n` runs (after 5 warm-up runs).
fn median(n: usize, mut f: impl FnMut()) -> Duration {
    for _ in 0..5 {
        f();
    }
    let mut samples: Vec<Duration> = (0..n)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    samples.sort();
    samples[samples.len() / 2]
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

fn frame(wm: &WindowManager, area: Rect, buf: &mut Buffer) {
    buf.reset();
    WorkspaceWidget::new(wm).render(area, buf);
}

#[test]
#[ignore = "measurement harness; run explicitly with --ignored --nocapture"]
fn frame_time_and_breakdown_6_panes_200x60() {
    let n = iters();
    let (mut wm, area) = render_fixture::build_workspace(200, 60, 6);
    let mut buf = Buffer::empty(area);
    let ids: Vec<_> = wm.compute_workspace_rects(area).into_iter().map(|(id, _)| id).collect();

    // --- whole frame -------------------------------------------------------------------------
    let idle = median(n, || frame(&wm, area, &mut buf));
    wm.toggle_help();
    assert!(wm.is_help_visible());
    let idle_overlay = median(n, || frame(&wm, area, &mut buf));
    wm.toggle_help();

    // One pane receives output before every frame (the common "one busy pane" case).
    let busy_id = ids[0];
    let mut tick = 0usize;
    let busy = median(n, || {
        tick += 1;
        let line = format!("\x1b[32mtick\x1b[0m {tick} https://example.org/t/{tick}\r\n");
        wm.pane_mut(busy_id).unwrap().process_pty_output(line.as_bytes());
        frame(&wm, area, &mut buf);
    });
    let busy_feed_only = median(n, || {
        tick += 1;
        let line = format!("\x1b[32mtick\x1b[0m {tick} https://example.org/t/{tick}\r\n");
        wm.pane_mut(busy_id).unwrap().process_pty_output(line.as_bytes());
    });

    // --- breakdown (same data, component cost in isolation) ---------------------------------
    // Claim 2: per-cell contrast maths over every visible cell of every pane.
    let cfg = wm.config().clone();
    let palette = cfg.palette();
    let mut cells = 0usize;
    let contrast = median(n, || {
        cells = 0;
        for id in &ids {
            let vt = wm.pane(*id).unwrap().vt();
            let (rows, cols) = vt.size();
            for r in 0..rows {
                for c in 0..cols {
                    if let Some(cell) = vt.cell_view(r, c) {
                        black_box(readable_canvas_cell_style(&cell, cfg.theme, &palette));
                        cells += 1;
                    }
                }
            }
        }
    });
    // Cell iteration alone, to subtract from the contrast figure.
    let cell_walk = median(n, || {
        for id in &ids {
            let vt = wm.pane(*id).unwrap().vt();
            let (rows, cols) = vt.size();
            for r in 0..rows {
                for c in 0..cols {
                    black_box(vt.cell_view(r, c));
                }
            }
        }
    });

    // Claim 3: URL detection and trigger scans, every pane.
    let urls = median(n, || {
        for id in &ids {
            black_box(wm.pane(*id).unwrap().vt().detect_visible_urls());
        }
    });
    let triggers = ["ERROR", "warning", "TODO"];
    let trig = median(n, || {
        for id in &ids {
            for t in triggers {
                black_box(wm.pane(*id).unwrap().vt().search_canvas(t));
            }
        }
    });

    // Claim 4: per-pane config clone + palette resolve, and BiltermConfig::default().
    let cfg_clone = median(n, || {
        for id in &ids {
            black_box(wm.pane_chrome(*id).resolve_config_and_palette(wm.config()));
        }
    });
    let cfg_default = median(n, || {
        for _ in &ids {
            black_box(BiltermConfig::default());
        }
    });

    // Claim 1: pane composition (all passes) vs. the rest of the frame (chrome + overlays).
    let rects = wm.compute_workspace_rects(area);
    let panes_only = median(n, || {
        for (id, rect) in &rects {
            let pane = wm.pane(*id).unwrap();
            CompositePaneWidget::with_config(pane, false, wm.config()).render(*rect, &mut buf);
        }
    });

    // Same frame with three output triggers configured.
    wm.config_mut().triggers = triggers.iter().map(|s| s.to_string()).collect();
    let idle_triggers = median(n, || frame(&wm, area, &mut buf));

    eprintln!("== perf_render_frame: 6 panes, 200x60, {n} iterations, median, {} ==",
        if cfg!(debug_assertions) { "DEBUG build" } else { "release build" });
    eprintln!("frame idle (nothing changed)          {:>9.1} us", us(idle));
    eprintln!("frame idle + help overlay             {:>9.1} us", us(idle_overlay));
    eprintln!("frame idle + 3 triggers               {:>9.1} us", us(idle_triggers));
    eprintln!("frame, one busy pane (feed + render)  {:>9.1} us  (feed alone {:.1} us)", us(busy), us(busy_feed_only));
    eprintln!("-- breakdown, all 6 panes --");
    eprintln!("pane composition (6 pane widgets)     {:>9.1} us", us(panes_only));
    eprintln!("contrast per cell ({cells} cells)      {:>9.1} us  (cell walk alone {:.1} us)", us(contrast), us(cell_walk));
    eprintln!("URL detection                         {:>9.1} us", us(urls));
    eprintln!("trigger scan (3 triggers)             {:>9.1} us", us(trig));
    eprintln!("config clone + palette resolve        {:>9.1} us", us(cfg_clone));
    eprintln!("BiltermConfig::default() x6           {:>9.1} us", us(cfg_default));
}
