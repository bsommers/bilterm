//! Echo-latency harness (`#[ignore]`: wall-clock measurements, never part of the CI gate).
//!
//! Run: `cargo test --release --test echo_latency -- --ignored --nocapture --test-threads=1`
//!
//! Each sample writes one byte to a `cat` PTY (the tty echoes it) and times how long it takes
//! until the pane has ingested the echo, i.e. until it is visible to the pane's VT state.

#![cfg(unix)]

use std::time::{Duration, Instant};

use bilterm::pane::CompositePane;
use bilterm::pty::PtyConfig;

const SAMPLES: usize = 300;

fn cat() -> PtyConfig {
    PtyConfig { program: "cat".into(), args: Vec::new(), rows: 24, cols: 80, ..PtyConfig::default() }
}

/// Idle gap before sample `i`: spreads writes over every phase of a 16 ms tick.
fn phase(i: usize) -> Duration {
    Duration::from_micros(20_000 + ((i * 3_700) % 16_000) as u64)
}

fn report(label: &str, mut v: Vec<Duration>) {
    v.sort();
    let us = |d: Duration| d.as_secs_f64() * 1e3;
    let pct = |p: f64| us(v[((v.len() as f64 * p) as usize).min(v.len() - 1)]);
    println!(
        "ECHO {label}: n={} median={:.3}ms p95={:.3}ms min={:.3}ms max={:.3}ms",
        v.len(),
        pct(0.50),
        pct(0.95),
        us(v[0]),
        us(v[v.len() - 1])
    );
}

/// The pre-change loop policy: PTY output is drained only on the 16 ms tick.
#[tokio::test]
#[ignore = "wall-clock measurement"]
async fn echo_latency_tick_polled_16ms() {
    let mut pane = CompositePane::spawn(1, cat()).unwrap();
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut out = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        tokio::time::sleep(phase(i)).await;
        while pane.poll_pty_events() {}
        let t0 = Instant::now();
        pane.write_to_pty(b"x").unwrap();
        loop {
            tick.tick().await;
            if pane.poll_pty_events() {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "no echo");
        }
        out.push(t0.elapsed());
    }
    report("before (16 ms tick poll)", out);
}

/// The new policy, same measurement: the reader thread's wake drains the PTY at once; the 16 ms
/// tick stays as the fallback. Same `select!` shape as `App::run_loop`.
#[tokio::test]
#[ignore = "wall-clock measurement"]
async fn echo_latency_wake_driven() {
    let mut pane = CompositePane::spawn(1, cat()).unwrap();
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut seen = bilterm::pty::wake::output_seq();
    let mut out = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        tokio::time::sleep(phase(i)).await;
        while pane.poll_pty_events() {}
        let t0 = Instant::now();
        pane.write_to_pty(b"x").unwrap();
        loop {
            tokio::select! {
                biased;
                _ = tick.tick() => {}
                _ = bilterm::pty::wake::output_ready(&mut seen) => {}
            }
            if pane.poll_pty_events() {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "no echo");
        }
        out.push(t0.elapsed());
    }
    report("after (output wake)", out);
}

/// End to end through the real `App::run_loop`: a key event enters the loop, is forwarded to the
/// PTY, `cat` writes it back, and the time is taken when the loop draws the frame that shows it
/// (so it includes the writer thread, the wake, the echo hold and the frame floor).
#[tokio::test]
#[ignore = "wall-clock measurement"]
async fn echo_latency_run_loop_key_to_frame() {
    use bilterm::app::{App, LoopStats};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::cell::{Cell, RefCell};

    let cfg = PtyConfig {
        program: "sh".into(),
        args: vec!["-c".into(), "stty raw -echo; printf '\\033[?1049hREADY'; exec cat".into()],
        rows: 24,
        cols: 80,
        ..PtyConfig::default()
    };
    let mut app = App::new(CompositePane::spawn(1, cfg).unwrap());
    let stats = LoopStats::default();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<std::io::Result<Event>>();
    let events = futures::stream::poll_fn(move |cx| rx.poll_recv(cx));
    let ready = Cell::new(false);
    let drawn = Cell::new(0usize);
    let sent_at: Cell<Option<Instant>> = Cell::new(None);
    let samples = RefCell::new(Vec::with_capacity(SAMPLES));
    let draw = |app: &mut App| {
        let text = app.wm.active_pane().vt().contents();
        ready.set(ready.get() || text.contains("READY"));
        let marks = text.matches('#').count();
        if marks > drawn.get() {
            drawn.set(marks);
            if let Some(t0) = sent_at.take() {
                samples.borrow_mut().push(t0.elapsed());
            }
        }
        Ok(())
    };
    let driver = async {
        while !ready.get() {
            tokio::task::yield_now().await;
        }
        for i in 0..SAMPLES {
            tokio::time::sleep(phase(i)).await;
            sent_at.set(Some(Instant::now()));
            tx.send(Ok(Event::Key(KeyEvent::new(KeyCode::Char('#'), KeyModifiers::NONE)))).unwrap();
            while drawn.get() <= i {
                tokio::task::yield_now().await;
            }
        }
        drop(tx);
    };
    let (res, ()) = tokio::join!(app.run_loop(events, draw, None, std::future::pending(), &stats), driver);
    res.unwrap();
    println!(
        "run_loop: draws={} pty_wakes={} ticks={} inputs={}",
        stats.draws.get(),
        stats.pty_wakes.get(),
        stats.ticks.get(),
        stats.inputs.get()
    );
    report("after (run_loop, key event -> frame drawn)", samples.into_inner());
}
