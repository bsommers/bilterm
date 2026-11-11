//! Phase 64-02 (`TMON-01`, `TMON-02`, `TMON-05`): per-hook byte-boundary tests with real PTYs,
//! in-memory streams and loopback sockets; conservation on the real `WindowManager`.

#[path = "support/phase60.rs"]
mod support;

use std::io::Write;
use std::time::{Duration, Instant};

use bilterm::api::metered::Metered;
use bilterm::api::{AutomationControlPlane, WebServeOptions};
use bilterm::pane::CompositePane;
#[cfg(target_os = "linux")]
use bilterm::pty::{PtyConfig, PtySession};
use bilterm::tmux::TmuxControlSubprocess;
use bilterm::tmux::link::{
    DriverPoll, LinkConnector, LinkDriver, LinkTarget, LinkTransport,
};
use bilterm::traffic::{
    ChannelCounters, F_CLASSIFY, F_ENABLED, F_OIA, GroupKey, Origin, Role, TrafficSnapshot,
    TrafficTap, group_by,
};
use bilterm::wm::WindowManager;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ON: u8 = F_ENABLED;

#[cfg(target_os = "linux")]
fn sh(script: &str) -> PtyConfig {
    PtyConfig {
        program: "sh".into(),
        args: vec!["-c".into(), script.into()],
        rows: 24,
        cols: 80,
        ..PtyConfig::default()
    }
}

fn wait_for(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn chan(
    snap: &TrafficSnapshot,
    pred: impl Fn(&bilterm::traffic::ChannelMeta) -> bool,
) -> Option<&bilterm::traffic::ChannelRow> {
    snap.channels.iter().find(|c| pred(&c.meta))
}

// ---------------------------------------------------------------- H01/H02 PTY

#[cfg(target_os = "linux")]
#[test]
fn pty_read_and_write_are_counted_once() {
    let mut s = PtySession::spawn(sh("printf hello; sleep 2")).unwrap();
    let cell = s.traffic_cell().clone();
    cell.set_flags(ON);
    assert!(wait_for(|| {
        let _ = s.drain_events();
        cell.read().rx_bytes >= 5
    }));
    assert!(cell.read().rx_msgs >= 1);
    s.write_all(b"x").unwrap();
    // Counted by the PTY writer thread once the byte is really written (RESP-01).
    assert!(wait_for(|| cell.read().tx_bytes == 1));
    assert_eq!(cell.read().tx_bytes, 1);
    assert_eq!(cell.read().tx_msgs, 1);
}

#[cfg(target_os = "linux")]
#[test]
fn pty_flags_zero_records_nothing() {
    let mut s = PtySession::spawn(sh("sleep 0.3; printf hello")).unwrap();
    let cell = s.traffic_cell().clone();
    cell.set_flags(0);
    let _ = s.write_all(b"y");
    assert!(wait_for(|| s.drain_events().iter().any(|e| matches!(e, bilterm::pty::PtyEvent::Exited(_)))));
    let r = cell.read();
    assert_eq!((r.rx_bytes, r.tx_bytes, r.rx_msgs, r.tx_msgs), (0, 0, 0, 0));
}

// ---------------------------------------------------------------- H03-H06 worker

#[test]
fn vt_worker_counts_write_pty_but_not_feed_output() {
    let h = bilterm::vt::worker::VtWorkerHandle::spawn_headless(24, 80, 100).unwrap();
    let cell = h.traffic_cell().clone();
    cell.set_flags(ON);
    h.feed_output(b"abc".to_vec());
    h.write_pty(b"abc".to_vec());
    assert!(wait_for(|| cell.read().tx_bytes >= 3));
    assert_eq!(cell.read().rx_bytes, 0, "FeedOutput is a synthetic feed (TRAFFIC-INV-04)");
}

// ---------------------------------------------------------------- H07-H12 pane

#[test]
fn classification_counts_escape_control_printable() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.payload_cell().set_flags(F_ENABLED | F_CLASSIFY);
    pane.process_pty_output(b"\x1b[31mhi\x1b[0m\r\n");
    let r = pane.payload_cell().read();
    assert_eq!((r.esc_bytes, r.esc_seqs, r.ctrl_bytes, r.print_bytes), (9, 2, 2, 2));
}

#[test]
fn classification_off_records_zero() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.payload_cell().set_flags(F_ENABLED);
    pane.process_pty_output(b"\x1b[31mhi\x1b[0m\r\n");
    let r = pane.payload_cell().read();
    assert_eq!((r.esc_bytes, r.ctrl_bytes, r.print_bytes), (0, 0, 0));
}

#[test]
fn classification_is_correct_across_chunk_splits() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.payload_cell().set_flags(F_ENABLED | F_CLASSIFY);
    for chunk in [&b"\x1b"[..], b"[3", b"1mh", b"i\x1b[0", b"m\r\n"] {
        pane.process_pty_output(chunk);
    }
    let r = pane.payload_cell().read();
    assert_eq!((r.esc_bytes, r.esc_seqs, r.ctrl_bytes, r.print_bytes), (9, 2, 2, 2));
}

#[test]
fn pty_less_pane_counts_tx_once_and_events() {
    let mut pane = CompositePane::new_headless(1, 24, 80);
    pane.write_to_pty(b"ls\n").unwrap();
    assert_eq!(pane.payload_cell().read().tx_bytes, 3);
    pane.record_media_passthrough("image/png", 10);
    assert_eq!(pane.payload_cell().read().events, 1);
}

#[cfg(target_os = "linux")]
#[test]
fn live_pty_pane_input_is_counted_at_the_pty_only() {
    let mut pane = CompositePane::spawn(2, sh("sleep 2")).unwrap();
    pane.write_to_pty(b"abc").unwrap();
    assert!(wait_for(|| pane.traffic_cell().unwrap().read().tx_bytes == 3));
    assert_eq!(pane.traffic_cell().unwrap().read().tx_bytes, 3);
    assert!(pane.existing_payload_cell().is_none(), "no double count on a live PTY pane");
}

// ---------------------------------------------------------------- adoption

#[cfg(target_os = "linux")]
#[test]
fn tick_adopts_local_and_ssh_panes_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let ssh = dir.path().join("ssh");
    std::os::unix::fs::symlink("/bin/sleep", &ssh).unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.traffic_tick(1_000);
    let local = CompositePane::spawn(2, sh("sleep 2")).unwrap();
    let ssh_cfg = PtyConfig {
        program: ssh.to_string_lossy().into_owned(),
        args: vec!["-p".into(), "2222".into(), "--".into(), "bill@prod-db".into()],
        ..PtyConfig::default()
    };
    let remote = CompositePane::spawn(3, ssh_cfg).unwrap();
    wm.panes_mut().insert(2, local);
    wm.panes_mut().insert(3, remote);
    wm.traffic_tick(2_000);
    wm.traffic_tick(3_000);
    let snap = wm.traffic_snapshot(100);
    let l = chan(&snap, |m| m.pane == Some(2) && m.role == Role::Both).expect("local pane");
    assert_eq!(l.meta.origin, Origin::LocalPty);
    let s = chan(&snap, |m| m.pane == Some(3) && m.role == Role::Both).expect("ssh pane");
    assert_eq!(s.meta.origin, Origin::Ssh { host: "prod-db".into(), port: 2222 });
    assert_eq!(snap.channels.iter().filter(|c| c.meta.pane == Some(2)).count(), 1);
    // Dropping the pane closes the channel on the next sample.
    wm.panes_mut().remove(&2);
    // The reader thread releases its clone once the child is reaped, so poll.
    let mut t = 4_000;
    assert!(wait_for(|| {
        t += 1_000;
        wm.traffic_tick(t);
        let snap = wm.traffic_snapshot(100);
        chan(&snap, |m| m.pane == Some(2)).is_some_and(|c| c.meta.closed_ms.is_some())
    }));
}

struct CellDriver(TrafficTap);
impl LinkDriver for CellDriver {
    fn pump(&mut self) -> DriverPoll {
        DriverPoll::default()
    }
    fn write_raw(&mut self, _: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
    fn resize(&mut self, _: u16, _: u16) {}
    fn kill(&mut self) {}
    fn traffic_cell(&self) -> Option<TrafficTap> {
        Some(self.0.clone())
    }
}
struct CellConnector(TrafficTap);
impl LinkConnector for CellConnector {
    fn connect(&mut self, t: &LinkTarget, _: (u16, u16)) -> std::io::Result<LinkTransport> {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (line_tx, line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        std::mem::forget((cmd_rx, line_tx)); // keep the control channel open
        Ok(LinkTransport {
            control: TmuxControlSubprocess::from_channels(&t.session, cmd_tx, line_rx),
            driver: Some(Box::new(CellDriver(self.0.clone()))),
        })
    }
}

#[test]
fn ssh_tmux_link_is_a_wire_channel_and_panes_are_payload() {
    let cell = ChannelCounters::new();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_link_connector(Box::new(CellConnector(cell.clone())));
    let id = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    cell.rx(100);
    cell.tx(7);
    wm.pane_mut(id).unwrap().count_payload_rx(40);
    wm.traffic_tick(1_000);
    let snap = wm.traffic_snapshot(100);
    let w = chan(&snap, |m| m.role == Role::Wire && matches!(m.origin, Origin::TmuxLink { .. }))
        .expect("link wire channel");
    assert_eq!(w.meta.pane, None);
    assert_eq!(w.meta.connection.as_str(), "tmux-link:box/main");
    assert_eq!(w.reading.rx_bytes, 100);
    let p = chan(&snap, |m| m.role == Role::Payload && m.pane == Some(id)).expect("payload channel");
    assert_eq!(p.meta.connection.as_str(), "tmux-link:box/main");
    assert_eq!(p.reading.rx_bytes, 40);
    // Connection totals never include Payload-only channels.
    let g = group_by(
        &snap,
        GroupKey::Connection,
        &Default::default(),
        &Default::default(),
        Default::default(),
        50,
    );
    let rx: u64 = g.rows.iter().map(|r| r.rx_total).sum();
    assert_eq!(rx, 100);
}

// ---------------------------------------------------------------- H19 Metered

#[tokio::test]
async fn metered_counts_exact_bytes_including_partial_writes() {
    let tap = ChannelCounters::with_flags(ON);
    let (a, mut b) = tokio::io::duplex(8); // tiny buffer forces partial writes
    let mut m = Metered::new(a, tap.clone());
    let writer = tokio::spawn(async move {
        m.write_all(b"0123456789abcdef").await.unwrap();
        let mut got = [0u8; 5];
        m.read_exact(&mut got).await.unwrap();
        m.shutdown().await.unwrap();
        got
    });
    let mut sink = Vec::new();
    let mut buf = [0u8; 64];
    while sink.len() < 16 {
        let n = b.read(&mut buf).await.unwrap();
        assert!(n > 0);
        sink.extend_from_slice(&buf[..n]);
    }
    b.write_all(b"hello").await.unwrap();
    assert_eq!(&writer.await.unwrap(), b"hello");
    let r = tap.read();
    assert_eq!((r.tx_bytes, r.rx_bytes), (16, 5));
}

#[cfg(unix)]
#[tokio::test]
async fn rpc_unix_connection_is_metered() {
    let plane = AutomationControlPlane::new_headless(24, 80);
    let meter = plane.wm.traffic_meter();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rpc.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    tokio::spawn(bilterm::api::serve_jsonrpc_unix(listener, plane, None));
    let mut s = tokio::net::UnixStream::connect(&path).await.unwrap();
    let req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace.status\",\"params\":{}}\n";
    s.write_all(req.as_bytes()).await.unwrap();
    let mut resp = Vec::new();
    let mut b = [0u8; 4096];
    while !resp.ends_with(b"\n") {
        let n = s.read(&mut b).await.unwrap();
        resp.extend_from_slice(&b[..n]);
    }
    let snap = meter.snapshot(100);
    let c = chan(&snap, |m| m.origin == Origin::RpcUnix).expect("rpc-unix channel");
    assert_eq!(c.reading.rx_bytes, req.len() as u64);
    assert_eq!(c.reading.tx_bytes, resp.len() as u64);
}

// ---------------------------------------------------------------- H20-H25 HTTP/SSE/WS

#[tokio::test]
async fn http_request_and_response_bytes_are_counted() {
    let plane = AutomationControlPlane::new_headless(24, 80);
    let meter = plane.wm.traffic_meter();
    let addr = support::start(plane, WebServeOptions::default()).await;
    let req = support::get(addr, "/health", true);
    let resp = support::raw(addr, req.as_bytes()).await;
    assert_eq!(support::status(&resp), 200);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let snap = meter.snapshot(100);
    let c = chan(&snap, |m| m.origin == Origin::Http).expect("http channel");
    assert_eq!(c.reading.rx_bytes, req.len() as u64);
    assert_eq!(c.reading.tx_bytes, resp.len() as u64);
}

#[tokio::test]
async fn websocket_frames_are_counted_on_a_web_n_channel() {
    let plane = AutomationControlPlane::new_headless(24, 80);
    let meter = plane.wm.traffic_meter();
    let addr = support::start(plane, WebServeOptions::default()).await;
    let mut ws = support::WsClient::connect(addr, "/ws", false).await;
    ws.send_json(&support::hello(&format!(r#","token":"{}""#, support::TOKEN))).await;
    let _ = ws.wait_for("hello_ack").await;
    ws.send_json(r#"{"type":"ping","t":1}"#).await;
    let _ = ws.wait_for("pong").await;
    let snap = meter.snapshot(100);
    let c = chan(&snap, |m| matches!(&m.origin, Origin::Ws { client } if client == "web-1"))
        .expect("ws channel");
    assert!(c.reading.rx_bytes > 0 && c.reading.tx_bytes > 0, "{:?}", c.reading);
    assert_eq!(c.meta.pane, None);
}

// ---------------------------------------------------------------- H28/H29/H31

#[test]
fn counting_stdout_counts_bytes_written() {
    let meter = bilterm::traffic::TrafficMeter::new();
    let mut out = bilterm::app::CountingStdout::new(&meter);
    out.write_all(b"\x1b[0m").unwrap();
    out.flush().unwrap();
    let snap = meter.snapshot(10);
    let c = chan(&snap, |m| m.origin == Origin::HostTty).unwrap();
    assert_eq!(c.reading.tx_bytes, 4);
}

#[test]
fn clipboard_tap_counts_writes_and_reads() {
    let tap = ChannelCounters::with_flags(ON);
    bilterm::clipboard::set_traffic_tap(Some(tap.clone()));
    bilterm::clipboard::set_system_clipboard("abc");
    let _ = bilterm::clipboard::get_system_clipboard();
    bilterm::clipboard::set_traffic_tap(None);
    assert!(tap.read().tx_bytes >= 3);
}

// ---------------------------------------------------------------- H33 STT

#[tokio::test]
async fn stt_provider_counts_request_and_response() {
    use bilterm::stt::provider::{OpenAiCompatSttProvider, SttProvider, set_traffic_meter};
    use bilterm::stt::TranscriptionRequest;
    let meter = bilterm::traffic::TrafficMeter::new();
    set_traffic_meter(Some(meter.clone()));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut b = vec![0u8; 16384];
        let _ = s.read(&mut b).await;
        s.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{\"text\": \"hi\"}").await.unwrap();
    });
    let p = OpenAiCompatSttProvider::new(format!("http://127.0.0.1:{port}/v1/x"), "m".into(), None);
    let req = TranscriptionRequest { wav_bytes: vec![0; 64], model: "m".into(), language: None, vocabulary_prompt: None };
    p.transcribe(&req).await.unwrap();
    set_traffic_meter(None);
    let snap = meter.snapshot(10);
    let c = chan(&snap, |m| matches!(m.origin, Origin::Stt { .. })).expect("stt channel");
    assert!(c.reading.tx_bytes > 64 && c.reading.rx_bytes > 0);
    assert_eq!(c.meta.origin.locality(), bilterm::traffic::Locality::Loopback);
}

// ---------------------------------------------------------------- OIA

fn oia_row(profile: &str, oia: bool) -> String {
    use bilterm::ui::glyphs::GlyphSet;
    use bilterm::ui::oia::render_oia_status_bar_themed;
    use bilterm::ui::theme::ThemePalette;
    use ratatui::{buffer::Buffer, layout::Rect};
    let pane = CompositePane::new_headless(1, 24, 120);
    let cell = pane.payload_cell();
    cell.set_flags(if oia { F_ENABLED | F_OIA } else { F_ENABLED });
    cell.publish_rates(1536, 12);
    let palette = ThemePalette { name: profile.to_string(), ..ThemePalette::default() };
    let rect = Rect::new(0, 0, 200, 1);
    let mut buf = Buffer::empty(rect);
    render_oia_status_bar_themed(&pane, rect, &palette, &GlyphSet::default(), &mut buf);
    (0..200).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

#[test]
fn oia_rate_indicator_is_opt_in() {
    assert!(oia_row("obsidian", true).contains("↓1.5K/s ↑12B/s"));
    assert!(!oia_row("obsidian", false).contains("↓"));
    let legacy = oia_row("CLASSIC", true);
    assert!(legacy.contains("RX:1.5K TX:12B"), "{legacy}");
    assert!(!oia_row("CLASSIC", false).contains("RX:"));
}

// ---------------------------------------------------------------- conservation (INV-03)

#[cfg(target_os = "linux")]
#[tokio::test]
async fn mixed_scenario_conserves_totals_on_the_real_wm() {
    let cell = ChannelCounters::new();
    let mut wm = WindowManager::new_headless(24, 80);
    wm.set_link_connector(Box::new(CellConnector(cell.clone())));
    let link_pane = wm.connect_ssh_tmux("me@box", "main", vec![]).unwrap();
    let mut local = CompositePane::spawn(5, sh("printf hello; sleep 2")).unwrap();
    assert!(wait_for(|| {
        local.poll_pty_events();
        local.traffic_cell().unwrap().read().rx_bytes >= 5
    }));
    local.write_to_pty(b"zz").unwrap();
    wm.panes_mut().insert(5, local);
    cell.rx(300);
    cell.tx(30);
    wm.pane_mut(link_pane).unwrap().count_payload_rx(250);
    let m = wm.traffic_meter();
    for (i, n) in [(1usize, 111usize), (2, 222)] {
        let t = m.open_channel(Origin::RpcUnix, &format!("rpc#{i}"), None, Role::Wire);
        t.rx(n);
        t.tx(n / 2);
    }
    let ws = m.open_channel(Origin::Ws { client: "web-1".into() }, "web-1", None, Role::Wire);
    ws.rx(10);
    ws.tx(500);
    wm.traffic_tick(1_000);
    wm.traffic_tick(2_000);
    let snap = wm.traffic_snapshot(100);
    let grand = |k: GroupKey| -> u64 {
        group_by(&snap, k, &Default::default(), &Default::default(), Default::default(), 100)
            .rows
            .iter()
            .map(|r| r.rx_total + r.tx_total)
            .sum()
    };
    let wire: u64 = snap
        .channels
        .iter()
        .filter(|c| c.meta.role.is_wire())
        .map(|c| c.reading.rx_bytes + c.reading.tx_bytes)
        .sum();
    let g = group_by(&snap, GroupKey::Connection, &Default::default(), &Default::default(), Default::default(), 100);
    assert_eq!(grand(GroupKey::Connection), wire);
    assert_eq!(grand(GroupKey::Origin), wire);
    assert_eq!(g.totals.rx + g.totals.tx, wire);
    assert!(wire >= 300 + 30 + 333 + 111 + 510 + 5 + 2);
    let payload: u64 = snap
        .channels
        .iter()
        .filter(|c| c.meta.role.is_payload() && c.meta.pane.is_some())
        .map(|c| c.reading.rx_bytes + c.reading.tx_bytes)
        .sum();
    let by_pane = group_by(&snap, GroupKey::Pane, &Default::default(), &Default::default(), Default::default(), 100);
    let pane_sum: u64 = by_pane.rows.iter().map(|r| r.rx_total + r.tx_total).sum();
    assert_eq!(pane_sum, payload);
}

