//! Unit tests for the CDP session and engine over a fake transport (in-memory pipes).

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton};
use serde_json::{Value, json};

use super::*;
use crate::pane::browser_canvas::cdp::CdpBrowserBackend;
use crate::pane::browser_canvas::{BrowserEngine, EngineHealth, MediaMouseEvent, MediaMouseEventKind};

/// 16x8 solid-colour JPEG, quality 70.
const TINY_JPEG_B64: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAoHBwgHBgoICAgLCgoLDhgQDg0NDh0VFhEYIx8lJCIfIiEmKzcvJik0KSEiMEExNDk7Pj4+JS5ESUM8SDc9Pjv/2wBDAQoLCw4NDhwQEBw7KCIoOzs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozs7Ozv/wAARCAAIABADASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAb/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFQEBAQAAAAAAAAAAAAAAAAAABQf/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIRAxEAPwCaAGKG/9k=";

struct MemReader {
    rx: Receiver<Vec<u8>>,
    left: Vec<u8>,
}

struct MemWriter(Sender<Vec<u8>>);

/// One direction of an in-memory pipe. Dropping the writer is EOF for the reader.
fn mem_pipe() -> (MemWriter, MemReader) {
    let (tx, rx) = channel();
    (MemWriter(tx), MemReader { rx, left: Vec::new() })
}

impl Read for MemReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.left.is_empty() {
            match self.rx.recv() {
                Ok(chunk) => self.left = chunk,
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.left.len());
        buf[..n].copy_from_slice(&self.left[..n]);
        self.left.drain(..n);
        Ok(n)
    }
}

impl Write for MemWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.send(buf.to_vec()).map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn boot(url: &str) -> Boot {
    Boot { url: url.to_string(), viewport: Viewport::from_frame_px(800, 460, 1.0), allow_file: false }
}

fn reply(protocol: &mut Protocol, id: u64, result: Value) {
    protocol.handle(json!({ "id": id, "result": result }).to_string().as_bytes()).unwrap();
}

fn event(protocol: &mut Protocol, method: &str, params: Value) {
    protocol.handle(json!({ "method": method, "params": params }).to_string().as_bytes()).unwrap();
}

/// Runs the handshake by hand and returns the protocol attached to target `T`.
fn attached(url: &str) -> (std::sync::Arc<Shared>, Protocol) {
    let shared = Shared::new(boot(url));
    let mut p = Protocol::new(std::sync::Arc::clone(&shared));
    p.begin();
    let q = shared.queue.drain_for_test();
    assert_eq!(q[0].1.method, "Target.getTargets");
    reply(&mut p, q[0].0, json!({ "targetInfos": [{ "targetId": "T", "type": "page" }] }));
    let q = shared.queue.drain_for_test();
    assert_eq!(q[0].1.method, "Target.attachToTarget");
    assert_eq!(shared.phase(), Phase::Starting, "not cdp until the attach answer arrives");
    reply(&mut p, q[0].0, json!({ "sessionId": "S" }));
    (shared, p)
}

fn methods(shared: &Shared) -> Vec<&'static str> {
    shared.queue.drain_for_test().into_iter().map(|(_, c)| c.method).collect()
}

#[test]
fn handshake_attaches_configures_and_navigates() {
    let (shared, _p) = attached("https://example.com/");
    assert_eq!(shared.phase(), Phase::Ready);
    assert_eq!(
        methods(&shared),
        [
            "Page.enable",
            "Browser.setDownloadBehavior",
            "Target.setDiscoverTargets",
            "Emulation.setDeviceMetricsOverride",
            "Page.startScreencast",
            "Page.navigate",
        ]
    );
    assert!(shared.page().loading);
}

#[test]
fn handshake_creates_a_page_target_when_none_exists_and_fails_cleanly() {
    let shared = Shared::new(boot("about:blank"));
    let mut p = Protocol::new(std::sync::Arc::clone(&shared));
    p.begin();
    let q = shared.queue.drain_for_test();
    reply(&mut p, q[0].0, json!({ "targetInfos": [{ "targetId": "B", "type": "browser" }] }));
    let q = shared.queue.drain_for_test();
    assert_eq!(q[0].1.method, "Target.createTarget");
    p.handle(json!({ "id": q[0].0, "error": { "message": "nope" } }).to_string().as_bytes()).unwrap();
    assert_eq!(shared.phase(), Phase::Failed("Chrome has no page target".into()));
    assert!(shared.queue.is_closed());
}

#[test]
fn page_events_update_url_title_loading_and_history() {
    let (shared, mut p) = attached("https://a.example/");
    methods(&shared);
    event(&mut p, "Page.frameStartedLoading", json!({ "frameId": "T" }));
    assert!(shared.page().loading);
    // A page-initiated navigation: the address bar must follow it.
    event(&mut p, "Page.frameNavigated", json!({ "frame": { "id": "T", "url": "https://b.example/next" } }));
    event(&mut p, "Page.frameNavigated", json!({ "frame": { "id": "AD", "parentId": "T", "url": "https://ads/" } }));
    assert_eq!(shared.page().url, "https://b.example/next");
    let q = shared.queue.drain_for_test();
    assert_eq!(q.len(), 1, "sub-frame navigations are ignored");
    assert_eq!(q[0].1.method, "Page.getNavigationHistory");
    reply(&mut p, q[0].0, json!({ "currentIndex": 1, "entries": [
        { "id": 1, "url": "https://a.example/", "title": "A" },
        { "id": 2, "url": "https://b.example/next", "title": "B" },
    ]}));
    let page = shared.page();
    assert_eq!(page.history, ["https://a.example/", "https://b.example/next"]);
    assert_eq!(page.history_idx, 1);
    assert_eq!(page.title, "B", "the committed history entry supplies the title");
    event(&mut p, "Target.targetInfoChanged", json!({ "targetInfo": { "targetId": "T", "title": "Real title", "url": "https://b.example/next" } }));
    event(&mut p, "Target.targetInfoChanged", json!({ "targetInfo": { "targetId": "OTHER", "title": "x", "url": "https://x/" } }));
    assert_eq!(shared.page().title, "Real title");
    event(&mut p, "Page.frameStoppedLoading", json!({ "frameId": "OTHER" }));
    assert!(shared.page().loading, "only the main frame controls the loading state");
    event(&mut p, "Page.loadEventFired", json!({}));
    assert!(!shared.page().loading);
    event(&mut p, "Page.navigatedWithinDocument", json!({ "frameId": "T", "url": "https://b.example/next#x" }));
    assert_eq!(shared.page().url, "https://b.example/next#x");
}

#[test]
fn back_and_forward_use_real_history_entries() {
    let (shared, mut p) = attached("https://a.example/");
    methods(&shared);
    let entries = json!({ "currentIndex": 1, "entries": [
        { "id": 11, "url": "https://a/", "title": "" },
        { "id": 12, "url": "https://b/", "title": "" },
        { "id": 13, "url": "https://c/", "title": "" },
    ]});
    for (offset, entry) in [(-1, 11), (1, 13)] {
        shared.queue.push(proto::get_history(offset)).unwrap();
        let q = shared.queue.drain_for_test();
        reply(&mut p, q[0].0, entries.clone());
        let q = shared.queue.drain_for_test();
        assert_eq!(q[0].1.method, "Page.navigateToHistoryEntry");
        assert_eq!(q[0].1.params, json!({ "entryId": entry }));
    }
    // The about:blank entry Chrome was launched on is hidden: no going back to it.
    let launch = json!({ "currentIndex": 1, "entries": [
        { "id": 1, "url": "about:blank", "title": "" },
        { "id": 2, "url": "https://a/", "title": "A" },
        { "id": 3, "url": "https://b/", "title": "B" },
    ]});
    shared.queue.push(proto::get_history(-1)).unwrap();
    let q = shared.queue.drain_for_test();
    reply(&mut p, q[0].0, launch.clone());
    assert!(shared.queue.is_empty(), "back from the first real page goes nowhere");
    shared.queue.push(proto::get_history(0)).unwrap();
    let q = shared.queue.drain_for_test();
    let generation = shared.page().generation;
    reply(&mut p, q[0].0, launch.clone());
    let page = shared.page();
    assert_eq!((page.history.clone(), page.history_idx), (vec!["https://a/".to_string(), "https://b/".to_string()], 0));
    assert_eq!(page.title, "A");
    assert_ne!(page.generation, generation);
    // An identical refresh changes nothing, so it must not request a redraw.
    shared.queue.push(proto::get_history(0)).unwrap();
    let q = shared.queue.drain_for_test();
    reply(&mut p, q[0].0, launch);
    assert_eq!(shared.page().generation, page.generation);
    // Back at the first entry does nothing.
    shared.queue.push(proto::get_history(-1)).unwrap();
    let q = shared.queue.drain_for_test();
    reply(&mut p, q[0].0, json!({ "currentIndex": 0, "entries": [{ "id": 11, "url": "https://a/", "title": "" }] }));
    assert!(shared.queue.is_empty());
}

#[test]
fn screencast_frames_are_acked_decoded_and_only_the_newest_is_kept() {
    let (shared, mut p) = attached("https://a.example/");
    methods(&shared);
    for ack in [5, 6] {
        event(&mut p, "Page.screencastFrame", json!({
            "data": TINY_JPEG_B64, "sessionId": ack, "metadata": { "deviceWidth": 8.0, "deviceHeight": 4.0 }
        }));
        let q = shared.queue.drain_for_test();
        assert_eq!(q[0].1.method, "Page.screencastFrameAck");
        assert_eq!(q[0].1.params, json!({ "sessionId": ack }));
    }
    let frame = shared.latest_frame().expect("decoded frame");
    assert_eq!((frame.seq, frame.width_px, frame.height_px), (2, 16, 8));
    assert_eq!(frame.rgba.len(), 16 * 8 * 4);
    assert!(frame.rgba[0] > 150 && frame.rgba[1] < 90 && frame.rgba[3] == 255, "red-ish opaque pixel");
    assert_eq!(shared.page().frame_px, (16, 8));
    assert_eq!(shared.page().device_px, (8.0, 4.0));
    // A corrupt frame is acked and skipped; the last good frame stays.
    event(&mut p, "Page.screencastFrame", json!({ "data": "AAAA", "sessionId": 7, "metadata": {} }));
    assert_eq!(methods(&shared), ["Page.screencastFrameAck"]);
    assert_eq!(shared.latest_frame().unwrap().seq, 2);
    assert!(decode_jpeg_frame("", 1, 0).is_none());
}

#[test]
fn crash_dialog_and_navigation_errors() {
    let (shared, mut p) = attached("https://a.example/");
    methods(&shared);
    event(&mut p, "Page.javascriptDialogOpening", json!({ "type": "confirm" }));
    let q = shared.queue.drain_for_test();
    assert_eq!(q[0].1.params, json!({ "accept": false }));
    shared.queue.push(proto::navigate("https://nope.invalid/")).unwrap();
    let q = shared.queue.drain_for_test();
    reply(&mut p, q[0].0, json!({ "frameId": "T", "errorText": "net::ERR_NAME_NOT_RESOLVED" }));
    assert_eq!(shared.page().nav_error.as_deref(), Some("net::ERR_NAME_NOT_RESOLVED"));
    event(&mut p, "Page.screencastFrame", json!({ "data": TINY_JPEG_B64, "sessionId": 1, "metadata": {} }));
    event(&mut p, "Inspector.targetCrashed", json!({}));
    assert_eq!(shared.phase(), Phase::Crashed("page crashed".into()));
    assert!(shared.latest_frame().is_none(), "no frozen frame after a crash");
    assert_eq!(shared.queue.push(proto::reload()), Err(PushError::Closed));
}

#[test]
fn popups_load_in_the_visible_page_and_are_closed() {
    let (shared, mut p) = attached("https://a.example/");
    methods(&shared);
    event(&mut p, "Target.targetCreated", json!({ "targetInfo": { "targetId": "POP", "type": "page", "openerId": "T", "url": "" } }));
    assert!(shared.queue.is_empty(), "blank popup waits for its URL");
    event(&mut p, "Target.targetInfoChanged", json!({ "targetInfo": { "targetId": "POP", "title": "", "url": "https://c.example/" } }));
    let q = shared.queue.drain_for_test();
    assert_eq!(q[0].1.method, "Page.navigate");
    assert_eq!(q[0].1.params, json!({ "url": "https://c.example/" }));
    assert_eq!(q[1].1.method, "Target.closeTarget");
    assert_eq!(q[1].1.params, json!({ "targetId": "POP" }));
    // Unrelated targets and disallowed schemes are left alone.
    event(&mut p, "Target.targetCreated", json!({ "targetInfo": { "targetId": "X", "type": "page", "url": "https://x/" } }));
    event(&mut p, "Target.targetCreated", json!({ "targetInfo": { "targetId": "J", "type": "page", "openerId": "T", "url": "javascript:alert(1)" } }));
    assert!(shared.queue.is_empty());
}

#[test]
fn queue_coalesces_mouse_moves_and_viewport_and_is_bounded() {
    let q = CmdQueue::default();
    let mv = || proto::insert_text("m").with_class(Class::MouseMove);
    for _ in 0..50 {
        q.push(mv()).unwrap();
    }
    assert_eq!(q.len(), 1, "consecutive mouse moves coalesce");
    for size in [100, 200, 300] {
        q.push(proto::set_device_metrics(size, size, 1.0)).unwrap();
        q.push(proto::start_screencast(size, size, 70)).unwrap();
    }
    let drained = q.drain_for_test();
    assert_eq!(drained.len(), 3, "only the newest viewport pair is kept");
    assert_eq!(drained[1].1.params["width"], 300);
    assert_eq!(drained[2].1.params["maxWidth"], 300);

    for _ in 0..QUEUE_CAP {
        q.push(proto::insert_text("x")).unwrap();
    }
    assert_eq!(q.push(proto::insert_text("x")), Err(PushError::Busy), "user input is refused when full");
    q.push(mv()).unwrap();
    assert_eq!(q.len(), QUEUE_CAP, "mouse moves are dropped under pressure");
    q.push(proto::screencast_ack(1)).unwrap();
    assert_eq!(q.len(), QUEUE_CAP + 1, "protocol bookkeeping is never dropped");
    q.close();
    assert_eq!(q.push(proto::reload()), Err(PushError::Closed));
    assert!(q.is_empty());
}

#[test]
fn viewport_zoom_shrinks_css_size_and_keeps_frame_size() {
    let v = Viewport::from_frame_px(800, 460, 1.0);
    assert_eq!((v.css_width, v.css_height, v.frame_px()), (800, 460, (800, 460)));
    let z = Viewport::from_frame_px(800, 460, 2.0);
    assert_eq!((z.css_width, z.css_height, z.scale, z.frame_px()), (400, 230, 2.0, (800, 460)));
    let out = Viewport::from_frame_px(800, 460, 0.5);
    assert_eq!((out.css_width, out.frame_px()), (1600, (800, 460)));
    assert_eq!(Viewport::from_frame_px(800, 460, f32::NAN).scale, 1.0);
}

// ---- threaded tests: the real reader/writer threads against a scripted fake Chrome ----

struct FakeChrome {
    to_client: MemWriter,
    from_client: MemReader,
    framer: Framer,
    inbox: std::collections::VecDeque<Value>,
}

impl FakeChrome {
    fn send(&mut self, v: Value) {
        let mut bytes = v.to_string().into_bytes();
        bytes.push(0);
        self.to_client.write_all(&bytes).unwrap();
    }

    /// Next command written by the client (blocks; the client threads are live).
    fn next(&mut self) -> Value {
        let mut buf = [0u8; 4096];
        while self.inbox.is_empty() {
            let n = self.from_client.read(&mut buf).unwrap();
            assert!(n > 0, "client closed the pipe");
            let mut msgs = Vec::new();
            self.framer.push(&buf[..n], &mut msgs).unwrap();
            self.inbox.extend(msgs.iter().map(|m| serde_json::from_slice::<Value>(m).unwrap()));
        }
        self.inbox.pop_front().unwrap()
    }

    /// Reads commands until `method` arrives, answering nothing in between.
    fn expect(&mut self, method: &str) -> Value {
        loop {
            let v = self.next();
            if v["method"] == method {
                return v;
            }
        }
    }

    fn handshake(&mut self) {
        let get = self.expect("Target.getTargets");
        self.send(json!({ "id": get["id"], "result": { "targetInfos": [{ "targetId": "T", "type": "page" }] } }));
        let attach = self.expect("Target.attachToTarget");
        assert_eq!(attach["params"]["flatten"], true);
        self.send(json!({ "id": attach["id"], "result": { "sessionId": "S" } }));
    }
}

fn engine_with_fake_chrome(url: &str) -> (CdpBrowserBackend, FakeChrome) {
    let (to_client, client_reader) = mem_pipe();
    let (client_writer, from_client) = mem_pipe();
    let engine =
        CdpBrowserBackend::with_transport(client_reader, client_writer, url, Viewport::from_frame_px(800, 460, 1.0), false)
            .unwrap();
    (engine, FakeChrome { to_client, from_client, framer: Framer::default(), inbox: Default::default() })
}

fn wait_until(engine: &mut CdpBrowserBackend, what: &str, cond: impl Fn(&CdpBrowserBackend) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.poll();
        if cond(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn engine_reports_cdp_only_after_the_handshake_and_drives_the_page() {
    let (mut engine, mut chrome) = engine_with_fake_chrome("https://example.com/");
    assert_eq!(engine.health(), EngineHealth::Starting);
    // Input before the handshake is dropped, never queued for a page that is not attached.
    engine.dispatch_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)).unwrap();
    chrome.handshake();
    wait_until(&mut engine, "ready", |e| e.health() == EngineHealth::Ready);
    assert_eq!(engine.backend_name(), "cdp");

    let nav = chrome.expect("Page.navigate");
    assert_eq!(nav["params"]["url"], "https://example.com/");
    assert_eq!(nav["sessionId"], "S");

    chrome.send(json!({ "method": "Page.screencastFrame", "sessionId": "S", "params": {
        "data": TINY_JPEG_B64, "sessionId": 3, "metadata": { "deviceWidth": 8.0, "deviceHeight": 4.0 } } }));
    let ack = chrome.expect("Page.screencastFrameAck");
    assert_eq!(ack["params"]["sessionId"], 3);
    wait_until(&mut engine, "frame", |e| e.frame_size_px() == (16, 8));
    assert_eq!(engine.latest_frame().unwrap().width_px, 16);

    // Frame is 16x8 px for an 8x4 CSS viewport: frame pixel (8, 4) is CSS (4, 2).
    engine.dispatch_mouse(MediaMouseEvent {
        kind: MediaMouseEventKind::Down(MouseButton::Left), x_px: 8, y_px: 4, modifiers: KeyModifiers::NONE,
    }).unwrap();
    let click = chrome.expect("Input.dispatchMouseEvent");
    assert_eq!((click["params"]["x"].as_f64(), click["params"]["y"].as_f64()), (Some(4.0), Some(2.0)));
    assert_eq!(click["params"]["clickCount"], 1);

    engine.dispatch_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)).unwrap();
    let key = chrome.expect("Input.dispatchKeyEvent");
    assert_eq!((key["params"]["type"].as_str(), key["params"]["text"].as_str()), (Some("keyDown"), Some("h")));
    engine.insert_text("pasted").unwrap();
    assert_eq!(chrome.expect("Input.insertText")["params"]["text"], "pasted");

    engine.resize_viewport(1000, 600, 2.0).unwrap();
    let metrics = chrome.expect("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics["params"], json!({ "width": 500, "height": 300, "deviceScaleFactor": 2.0, "mobile": false }));
    chrome.expect("Page.stopScreencast");
    assert_eq!(chrome.expect("Page.startScreencast")["params"]["maxWidth"], 1000);

    assert!(engine.navigate("javascript:alert(1)").is_err(), "policy is enforced at the engine");
    engine.navigate("example.org/x").unwrap();
    assert_eq!(engine.current_url(), "https://example.org/x");
    assert_eq!(chrome.expect("Page.navigate")["params"]["url"], "https://example.org/x");

    chrome.send(json!({ "method": "Target.targetInfoChanged", "params": {
        "targetInfo": { "targetId": "T", "title": "Example", "url": "https://example.org/x" } } }));
    wait_until(&mut engine, "title", |e| e.page_title() == "Example");

    // Chrome goes away: crashed state, not a frozen frame.
    drop(chrome);
    wait_until(&mut engine, "crash", |e| matches!(e.health(), EngineHealth::Crashed(_)));
    assert_eq!(engine.health(), EngineHealth::Crashed("Chrome exited".into()));
    assert!(engine.latest_frame().is_none());
    assert!(engine.reload().is_err());
}

#[test]
fn exit_or_garbage_before_the_handshake_is_a_launch_failure_not_a_panic() {
    let (mut engine, chrome) = engine_with_fake_chrome("https://example.com/");
    drop(chrome);
    wait_until(&mut engine, "failed", |e| matches!(e.health(), EngineHealth::Failed(_)));
    assert_eq!(engine.health(), EngineHealth::Failed("Chrome exited".into()));

    let (mut engine, mut chrome) = engine_with_fake_chrome("https://example.com/");
    chrome.to_client.write_all(b"<html>not devtools</html>\0").unwrap();
    wait_until(&mut engine, "failed", |e| matches!(e.health(), EngineHealth::Failed(_)));
    let EngineHealth::Failed(why) = engine.health() else { unreachable!() };
    assert!(why.contains("not JSON"), "{why}");
}

#[test]
fn navigation_before_the_handshake_replaces_the_boot_url() {
    let (mut engine, mut chrome) = engine_with_fake_chrome("https://first.example/");
    engine.navigate("https://second.example/").unwrap();
    assert_eq!(engine.current_url(), "https://second.example/");
    chrome.handshake();
    assert_eq!(chrome.expect("Page.navigate")["params"]["url"], "https://second.example/");
}
