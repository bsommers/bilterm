//! RESP-01: the event loop is woken by PTY output, paces its redraws, and keeps servicing input
//! while a pane floods. Drives the real `App::run_loop` headlessly (no host terminal).

#![cfg(unix)]

use std::cell::Cell;
use std::io;
use std::time::{Duration, Instant};

use bilterm::app::{App, LoopStats, FRAME_FLOOR};
use bilterm::pane::CompositePane;
use bilterm::pty::PtyConfig;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

/// Generous bound for "this must eventually happen"; passing never depends on it being short.
const LONG: Duration = Duration::from_secs(60);

fn sh(script: &str) -> PtyConfig {
    PtyConfig { program: "sh".into(), args: vec!["-c".into(), script.into()], rows: 24, cols: 80, ..PtyConfig::default() }
}

fn key(c: char) -> io::Result<Event> {
    Ok(Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))
}

/// Yields to the loop until `cond` holds; panics after `LONG`.
async fn until(what: &str, cond: impl Fn() -> bool) {
    let end = Instant::now() + LONG;
    while !cond() {
        assert!(Instant::now() < end, "timed out waiting for: {what}");
        tokio::task::yield_now().await;
    }
}

type Events = UnboundedSender<io::Result<Event>>;

/// Runs `App::run_loop` over `pane` next to `driver`; the loop ends when the driver drops the
/// event sender. `on_draw` is called for every frame the loop decides to draw.
async fn run_with<F, Fut>(pane: CompositePane, stats: &LoopStats, on_draw: impl Fn(&App), driver: F)
where
    F: FnOnce(Events) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut app = App::new(pane);
    let (tx, mut rx) = unbounded_channel();
    let events = futures::stream::poll_fn(move |cx| rx.poll_recv(cx));
    let draw = |app: &mut App| {
        on_draw(app);
        Ok(())
    };
    let looped = app.run_loop(events, draw, None, std::future::pending(), stats);
    let (res, ()) = tokio::join!(looped, driver(tx));
    res.unwrap();
}

/// `yes`-style flood in the pane while input events are delivered: every event is consumed
/// within two loop turns of being sent, and redraws are coalesced by the frame floor.
#[tokio::test]
async fn input_is_serviced_within_two_turns_during_a_flood_and_draws_are_coalesced() {
    const EVENTS: u64 = 100;
    let pane = CompositePane::spawn(1, sh("exec yes bilterm-flood-line-0123456789")).unwrap();
    let stats = LoopStats::default();
    let started = Instant::now();
    let worst = Cell::new(0u64);
    let wakes_during_input = Cell::new(0u64);
    run_with(pane, &stats, |_| {}, |tx| async {
        until("the flood to wake the loop", || stats.pty_wakes.get() >= 50).await;
        let wakes_at_start = stats.pty_wakes.get();
        for i in 0..EVENTS {
            // Only send while the flood is demonstrably keeping the loop busy.
            let wakes = stats.pty_wakes.get();
            until("more flood output", || stats.pty_wakes.get() > wakes).await;
            let sent_at_turn = stats.turns.get();
            tx.send(key('a')).unwrap();
            until("the input event to be consumed", || stats.inputs.get() > i).await;
            worst.set(worst.get().max(stats.last_input_turn.get() - sent_at_turn));
        }
        wakes_during_input.set(stats.pty_wakes.get() - wakes_at_start);
        drop(tx);
    })
    .await;
    let elapsed = started.elapsed();

    assert_eq!(stats.inputs.get(), EVENTS + 1, "every event plus the end of the stream");
    assert!(wakes_during_input.get() >= EVENTS, "the pane flooded throughout: {wakes_during_input:?}");
    // A sent event is taken on the next turn, or the one after if the 16 ms tick was due.
    assert!(worst.get() <= 2, "input waited {} loop turns behind PTY output", worst.get());
    // The pacer never starts two draws closer than FRAME_FLOOR, so N draws need (N-1) floors.
    // Slowness can only reduce the draw count, so this cannot flake.
    let max_draws = (elapsed.as_micros() / FRAME_FLOOR.as_micros()) as u64 + 1;
    assert!(stats.draws.get() >= 2, "the flood was drawn");
    assert!(stats.draws.get() <= max_draws, "{} draws in {elapsed:?}", stats.draws.get());
    // Not asserted: `pty_wakes > draws`. On a loaded machine the 16 ms tick (first in the biased
    // select) drains most of the output and input events cause redraws too, so draws can exceed
    // wake-driven drains (CI saw 618 draws against 450 wakes). The floor bound above is the
    // coalescing guarantee.
}

const RAW_ECHO_CHILD: &str = "stty raw -echo; printf '\\033[?1049hREADY'; exec cat";

fn marks(app: &App) -> usize {
    app.wm.active_pane().vt().contents().matches('#').count()
}

/// A key forwarded to the PTY is echoed and drawn without waiting for the 16 ms tick. The old
/// loop drained PTY output only inside the tick, so every echo needed at least one tick between
/// the keypress and the frame showing it.
#[tokio::test]
async fn echo_is_drawn_by_the_output_wake_not_the_tick() {
    const KEYS: usize = 60;
    // Alternate screen: the pane routes keys straight to the PTY (like vim/htop). Raw and no
    // tty echo: `cat` writes each byte back as soon as it reads it.
    let pane = CompositePane::spawn(1, sh(RAW_ECHO_CHILD)).unwrap();
    let stats = LoopStats::default();
    let drawn_marks = Cell::new(0usize);
    let ready = Cell::new(false);
    let zero_tick_echoes = Cell::new(0usize);
    let on_draw = |app: &App| {
        drawn_marks.set(marks(app));
        ready.set(ready.get() || app.wm.active_pane().vt().contents().contains("READY"));
    };
    run_with(pane, &stats, on_draw, |tx| async {
        until("the child to configure its tty", || ready.get()).await;
        for i in 0..KEYS {
            // Start each sample just after a tick so the next one is a full period away.
            let t = stats.ticks.get();
            until("a tick boundary", || stats.ticks.get() > t).await;
            let ticks_before = stats.ticks.get();
            tx.send(key('#')).unwrap();
            until("the echo to be drawn", || drawn_marks.get() > i).await;
            if stats.ticks.get() == ticks_before {
                zero_tick_echoes.set(zero_tick_echoes.get() + 1);
            }
        }
        drop(tx);
    })
    .await;
    assert_eq!(drawn_marks.get(), KEYS, "every key was echoed and drawn exactly once");
    assert!(stats.pty_wakes.get() >= 1);
    assert!(
        zero_tick_echoes.get() >= 1,
        "no echo was drawn between ticks: output is still waiting for the 16 ms tick"
    );
    println!("echoes drawn before the next tick: {}/{KEYS}", zero_tick_echoes.get());
}

/// A pane whose child exits is reaped/held by the wake path just as by the tick, and the loop
/// ends when the last pane is gone (no hang waiting for input).
#[tokio::test]
async fn loop_ends_when_the_only_pane_exits() {
    let pane = CompositePane::spawn(1, sh("printf bye")).unwrap();
    let stats = LoopStats::default();
    let mut app = App::new(pane);
    let events = futures::stream::pending::<io::Result<Event>>();
    let looped = app.run_loop(events, |_| Ok(()), None, std::future::pending(), &stats);
    tokio::time::timeout(LONG, looped).await.expect("loop did not end").unwrap();
    assert!(app.all_panes_exited());
}
