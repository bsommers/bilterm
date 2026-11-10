//! CDP session: bounded command queue, protocol state machine and the reader/writer threads
//! that keep the UI thread off Chrome's pipe (`WEB-01..02`).
//!
//! The UI thread only ever takes short mutexes: it enqueues commands ([`CmdQueue::push`]) and reads
//! the shared page state and the newest frame. The reader thread parses events, decodes screencast
//! JPEGs and drives the handshake; the writer thread drains the queue into the pipe. Generic over
//! `Read`/`Write` so tests run it against in-memory pipes.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::Instant;

use super::MediaRgbaFrame;
use super::cdp_proto::{self as proto, Class, Cmd, Event, Framer, Incoming, ProtoError, Scope, Tag};
use super::url_policy::validate_url;

/// Commands queued for Chrome before user input is refused (`Busy`).
pub const QUEUE_CAP: usize = 512;
/// JPEG quality requested for screencast frames.
pub const SCREENCAST_QUALITY: u8 = 70;
/// Largest decoded frame edge accepted from Chrome.
pub const MAX_FRAME_EDGE: usize = 4096;
const MAX_PENDING_TAGS: usize = 1024;
const MAX_HISTORY: usize = 256;
const MAX_POPUPS: usize = 16;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Lifecycle of the CDP backend.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// Chrome was launched; the attach handshake has not finished.
    #[default]
    Starting,
    /// Attached to the page target: the engine is really CDP.
    Ready,
    /// Was `Ready`, then the page or Chrome died.
    Crashed(String),
    /// Never became `Ready`.
    Failed(String),
}

/// Viewport request: CSS pixel size and device scale factor (zoom). Frames are `css * scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub css_width: u32,
    pub css_height: u32,
    pub scale: f64,
}

impl Viewport {
    /// Frame size `width_px x height_px` shown at `scale` (CSS size shrinks as zoom grows).
    pub fn from_frame_px(width_px: u16, height_px: u16, scale: f32) -> Self {
        let scale = if scale.is_finite() { f64::from(scale).clamp(0.25, 5.0) } else { 1.0 };
        let css = |px: u16| ((f64::from(px) / scale).round() as u32).max(16);
        Self { css_width: css(width_px), css_height: css(height_px), scale }
    }

    /// Largest frame asked of the screencast. Chrome captures at CSS-pixel resolution, so a
    /// zoomed-in page (scale > 1) comes back smaller than this and is stretched by the renderer.
    pub fn frame_px(&self) -> (u32, u32) {
        let px = |css: u32| ((f64::from(css) * self.scale).round() as u32).clamp(16, MAX_FRAME_EDGE as u32);
        (px(self.css_width), px(self.css_height))
    }

    fn commands(&self) -> [Cmd; 2] {
        let (w, h) = self.frame_px();
        [
            proto::set_device_metrics(self.css_width, self.css_height, self.scale),
            proto::start_screencast(w, h, SCREENCAST_QUALITY),
        ]
    }
}

/// Page state published by the reader thread.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageState {
    pub phase: Phase,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub nav_error: Option<String>,
    pub history: Vec<String>,
    pub history_idx: usize,
    /// Size of the newest frame in pixels and the CSS size it covers.
    pub frame_px: (u16, u16),
    pub device_px: (f64, f64),
    /// Bumped on every change so readers can poll cheaply.
    pub generation: u64,
}

/// Why a command was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushError {
    Busy,
    Closed,
}

struct QueueInner {
    q: VecDeque<(u64, Cmd)>,
    next_id: u64,
    pending: HashMap<u64, Tag>,
    closed: bool,
}

/// Bounded, coalescing command queue between the UI thread and the writer thread.
pub struct CmdQueue {
    inner: Mutex<QueueInner>,
    cv: Condvar,
}

impl Default for CmdQueue {
    fn default() -> Self {
        Self {
            inner: Mutex::new(QueueInner { q: VecDeque::new(), next_id: 1, pending: HashMap::new(), closed: false }),
            cv: Condvar::new(),
        }
    }
}

impl CmdQueue {
    /// Queues `cmd` without blocking. Mouse moves coalesce and are dropped when the queue is full;
    /// a viewport command replaces a queued one of the same method; other user input gets `Busy`.
    pub fn push(&self, cmd: Cmd) -> Result<(), PushError> {
        let mut g = lock(&self.inner);
        if g.closed {
            return Err(PushError::Closed);
        }
        match cmd.class {
            Class::MouseMove => {
                if let Some(back) = g.q.back_mut()
                    && back.1.class == Class::MouseMove
                {
                    back.1 = cmd;
                    return Ok(());
                }
                if g.q.len() >= QUEUE_CAP {
                    return Ok(());
                }
            }
            Class::Viewport => g.q.retain(|(_, c)| !(c.class == Class::Viewport && c.method == cmd.method)),
            Class::Normal if g.q.len() >= QUEUE_CAP => return Err(PushError::Busy),
            Class::Normal | Class::Internal => {}
        }
        let id = g.next_id;
        g.next_id += 1;
        if cmd.tag != Tag::None {
            if g.pending.len() >= MAX_PENDING_TAGS {
                g.pending.clear();
            }
            g.pending.insert(id, cmd.tag);
        }
        g.q.push_back((id, cmd));
        self.cv.notify_one();
        Ok(())
    }

    fn pop_blocking(&self) -> Option<(u64, Cmd)> {
        let mut g = lock(&self.inner);
        loop {
            if g.closed {
                return None;
            }
            if let Some(item) = g.q.pop_front() {
                return Some(item);
            }
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn take_tag(&self, id: u64) -> Tag {
        lock(&self.inner).pending.remove(&id).unwrap_or(Tag::None)
    }

    pub fn close(&self) {
        let mut g = lock(&self.inner);
        g.closed = true;
        g.q.clear();
        g.pending.clear();
        self.cv.notify_all();
    }

    pub fn len(&self) -> usize {
        lock(&self.inner).q.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_closed(&self) -> bool {
        lock(&self.inner).closed
    }

    /// Removes and returns everything queued, with ids (tests drive the protocol without threads).
    #[cfg(test)]
    pub(crate) fn drain_for_test(&self) -> Vec<(u64, Cmd)> {
        lock(&self.inner).q.drain(..).collect()
    }
}

/// What to load and how big, read by the handshake when the page target is attached.
#[derive(Clone, Debug)]
pub struct Boot {
    pub url: String,
    pub viewport: Viewport,
    pub allow_file: bool,
}

/// State shared between the engine (UI thread) and the session threads.
pub struct Shared {
    page: Mutex<PageState>,
    frame: Mutex<Option<MediaRgbaFrame>>,
    boot: Mutex<Boot>,
    session: OnceLock<String>,
    pub queue: CmdQueue,
}

impl Shared {
    pub fn new(boot: Boot) -> Arc<Self> {
        let page = PageState { url: boot.url.clone(), loading: true, ..PageState::default() };
        Arc::new(Self {
            page: Mutex::new(page),
            frame: Mutex::new(None),
            boot: Mutex::new(boot),
            session: OnceLock::new(),
            queue: CmdQueue::default(),
        })
    }

    pub fn page(&self) -> PageState {
        lock(&self.page).clone()
    }

    /// Returns the page state only when it changed since `seen_generation`.
    pub fn page_if_changed(&self, seen_generation: u64) -> Option<PageState> {
        let g = lock(&self.page);
        (g.generation != seen_generation).then(|| g.clone())
    }

    pub fn phase(&self) -> Phase {
        lock(&self.page).phase.clone()
    }

    pub fn latest_frame(&self) -> Option<MediaRgbaFrame> {
        lock(&self.frame).clone()
    }

    pub fn update_page(&self, f: impl FnOnce(&mut PageState)) {
        let mut g = lock(&self.page);
        f(&mut g);
        g.generation += 1;
    }

    /// Like [`Self::update_page`], but leaves the generation alone when nothing changed, so
    /// periodic refreshes do not cause redraws.
    pub fn update_page_if_changed(&self, f: impl FnOnce(&mut PageState)) {
        let mut g = lock(&self.page);
        let before = g.clone();
        f(&mut g);
        if *g != before {
            g.generation += 1;
        }
    }

    /// Updates what the handshake will load (used while the backend is still starting).
    pub fn update_boot(&self, f: impl FnOnce(&mut Boot)) {
        f(&mut lock(&self.boot));
    }

    fn boot(&self) -> Boot {
        lock(&self.boot).clone()
    }

    /// Marks the backend dead: `Crashed` when it had been ready, `Failed` otherwise. Idempotent.
    pub fn mark_dead(&self, reason: &str) {
        {
            let mut g = lock(&self.page);
            match g.phase {
                Phase::Starting => g.phase = Phase::Failed(reason.to_string()),
                Phase::Ready => g.phase = Phase::Crashed(reason.to_string()),
                Phase::Crashed(_) | Phase::Failed(_) => return,
            }
            g.loading = false;
            g.generation += 1;
        }
        *lock(&self.frame) = None;
        self.queue.close();
    }
}

/// Decodes a base64 screencast JPEG to RGBA. `None` for anything undecodable or oversize.
pub fn decode_jpeg_frame(b64: &str, seq: u64, pts_ms: u64) -> Option<MediaRgbaFrame> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let jpeg = crate::vt::codec::base64_decode(b64);
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_max_width(MAX_FRAME_EDGE)
        .set_max_height(MAX_FRAME_EDGE);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&jpeg[..]), options);
    let rgba = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (w, h) = (usize::from(info.width), usize::from(info.height));
    if w == 0 || h == 0 || w > MAX_FRAME_EDGE || h > MAX_FRAME_EDGE || rgba.len() != w * h * 4 {
        return None;
    }
    Some(MediaRgbaFrame::new(seq, info.width, info.height, pts_ms, rgba))
}

/// Protocol state machine owned by the reader thread.
pub struct Protocol {
    shared: Arc<Shared>,
    target_id: Option<String>,
    main_frame: Option<String>,
    popups: Vec<String>,
    frame_seq: u64,
    started: Instant,
}

impl Protocol {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared, target_id: None, main_frame: None, popups: Vec::new(), frame_seq: 0, started: Instant::now() }
    }

    fn send(&self, cmd: Cmd) {
        let _ = self.shared.queue.push(cmd);
    }

    /// Starts the handshake: find the page target.
    pub fn begin(&self) {
        self.send(proto::get_targets());
    }

    /// Handles one framed message from Chrome.
    pub fn handle(&mut self, bytes: &[u8]) -> Result<(), ProtoError> {
        match proto::parse_incoming(bytes)? {
            Incoming::Response { id, result, error } => {
                let tag = self.shared.queue.take_tag(id);
                self.on_response(tag, &result, error);
            }
            Incoming::Event(event) => self.on_event(event),
            Incoming::Ignored => {}
        }
        Ok(())
    }

    fn on_response(&mut self, tag: Tag, result: &serde_json::Value, error: Option<String>) {
        let str_field = |key: &str| result.get(key).and_then(serde_json::Value::as_str).map(str::to_string);
        match tag {
            Tag::None => {}
            Tag::GetTargets => match (error, proto::first_page_target(result)) {
                (Some(e), _) => self.shared.mark_dead(&format!("Target.getTargets failed: {e}")),
                (None, Some(target)) => self.attach(target),
                (None, None) => self.send(proto::create_target()),
            },
            Tag::CreateTarget => match str_field("targetId") {
                Some(target) if error.is_none() => self.attach(target),
                _ => self.shared.mark_dead("Chrome has no page target"),
            },
            Tag::Attach => match str_field("sessionId") {
                Some(session) if error.is_none() => self.on_attached(session),
                _ => self.shared.mark_dead(&format!("attach failed: {}", error.unwrap_or_default())),
            },
            Tag::Navigate => {
                let failure = error.or_else(|| str_field("errorText").filter(|t| !t.is_empty()));
                if let Some(text) = failure {
                    self.shared.update_page(|p| {
                        p.nav_error = Some(text);
                        p.loading = false;
                    });
                }
            }
            Tag::History(offset) => {
                let Some((idx, entries)) = proto::parse_history(result) else { return };
                // Chrome is launched on about:blank before the first navigation; that launch entry
                // is not part of the user's history.
                let base = usize::from(entries.len() > 1 && entries[0].url == "about:blank");
                if offset != 0 {
                    let target = usize::try_from(idx as i64 + i64::from(offset)).ok().filter(|t| *t >= base);
                    if let Some(entry) = target.and_then(|t| entries.get(t)) {
                        self.send(proto::navigate_to_history_entry(entry.id));
                    }
                    return;
                }
                let skip = entries.len().saturating_sub(MAX_HISTORY).max(base).min(idx);
                self.shared.update_page_if_changed(|p| {
                    // `Target.targetInfoChanged` does not fire for `document.title` changes, so the
                    // committed history entry is the source of truth for the title.
                    if !entries[idx].title.is_empty() {
                        p.title = entries[idx].title.clone();
                    }
                    p.history = entries[skip..].iter().take(MAX_HISTORY).map(|e| e.url.clone()).collect();
                    p.history_idx = idx - skip;
                });
            }
        }
    }

    fn attach(&mut self, target: String) {
        self.send(proto::attach_to_target(&target));
        self.target_id = Some(target);
    }

    fn on_attached(&mut self, session: String) {
        let _ = self.shared.session.set(session);
        let boot = self.shared.boot();
        self.send(proto::page_enable());
        self.send(proto::deny_downloads());
        self.send(proto::discover_targets());
        for cmd in boot.viewport.commands() {
            self.send(cmd);
        }
        self.send(proto::navigate(&boot.url));
        self.shared.update_page(|p| {
            p.phase = Phase::Ready;
            p.url = boot.url;
            p.loading = true;
        });
    }

    fn is_main_frame(&self, frame_id: &str) -> bool {
        self.main_frame.as_deref() == Some(frame_id) || self.target_id.as_deref() == Some(frame_id)
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::FrameNavigated { frame_id, is_main, url } => {
                if !is_main {
                    return;
                }
                self.main_frame = Some(frame_id);
                self.shared.update_page(|p| {
                    p.url = url;
                    p.nav_error = None;
                });
                self.send(proto::get_history(0));
            }
            Event::NavigatedWithinDocument { frame_id, url } => {
                if self.is_main_frame(&frame_id) {
                    self.shared.update_page(|p| p.url = url);
                    self.send(proto::get_history(0));
                }
            }
            Event::LoadingStarted { frame_id } => {
                if self.is_main_frame(&frame_id) {
                    self.shared.update_page(|p| p.loading = true);
                }
            }
            Event::LoadingStopped { frame_id } => {
                if self.is_main_frame(&frame_id) {
                    self.shared.update_page(|p| p.loading = false);
                }
            }
            Event::LoadEventFired => {
                self.shared.update_page(|p| p.loading = false);
                self.send(proto::get_history(0));
            }
            Event::TargetInfoChanged { target_id, title, url } => {
                if self.target_id.as_deref() == Some(target_id.as_str()) {
                    self.shared.update_page(|p| {
                        p.title = title;
                        if !url.is_empty() {
                            p.url = url;
                        }
                    });
                } else if self.popups.contains(&target_id) {
                    self.adopt_popup(target_id, &url);
                }
            }
            Event::TargetCreated { target_id, kind, opener, url } => {
                if kind == "page" && opener.is_some() && opener == self.target_id {
                    self.adopt_popup(target_id, &url);
                }
            }
            Event::ScreencastFrame { data, ack_id, device_width, device_height } => {
                self.send(proto::screencast_ack(ack_id));
                self.frame_seq += 1;
                let pts_ms = self.started.elapsed().as_millis() as u64;
                if let Some(frame) = decode_jpeg_frame(&data, self.frame_seq, pts_ms) {
                    let size = (frame.width_px, frame.height_px);
                    *lock(&self.shared.frame) = Some(frame);
                    self.shared.update_page(|p| {
                        p.frame_px = size;
                        p.device_px = (device_width, device_height);
                    });
                }
            }
            Event::DialogOpening { kind } => {
                self.send(proto::handle_dialog(matches!(kind.as_str(), "alert" | "beforeunload")));
            }
            Event::TargetCrashed => self.shared.mark_dead("page crashed"),
            Event::Detached => self.shared.mark_dead("page target detached"),
        }
    }

    /// A link opened a new window: load it in the one visible page instead and close the popup.
    fn adopt_popup(&mut self, popup_id: String, url: &str) {
        let allow_file = self.shared.boot().allow_file;
        match validate_url(url, allow_file) {
            Ok(u) if u != "about:blank" && !url.is_empty() => {
                self.popups.retain(|p| p != &popup_id);
                self.send(proto::navigate(&u).with_class(Class::Internal));
                self.send(proto::close_target(&popup_id));
            }
            _ => {
                if !self.popups.contains(&popup_id) {
                    if self.popups.len() >= MAX_POPUPS {
                        let dropped = self.popups.remove(0);
                        self.send(proto::close_target(&dropped));
                    }
                    self.popups.push(popup_id);
                }
            }
        }
    }
}

/// Reader and writer threads of one browser.
pub struct SessionThreads {
    pub reader: JoinHandle<()>,
    pub writer: JoinHandle<()>,
}

fn reader_loop<R: Read>(mut reader: R, shared: Arc<Shared>) {
    let mut protocol = Protocol::new(Arc::clone(&shared));
    protocol.begin();
    let mut framer = Framer::default();
    let mut buf = vec![0u8; 64 * 1024];
    let mut messages = Vec::new();
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => return shared.mark_dead("Chrome exited"),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return shared.mark_dead(&format!("read from Chrome failed: {e}")),
        };
        if let Err(e) = framer.push(&buf[..n], &mut messages) {
            return shared.mark_dead(&e.to_string());
        }
        for msg in messages.drain(..) {
            tracing::trace!(target: "bilterm::browser", bytes = msg.len(), "cdp <- {}", String::from_utf8_lossy(&msg[..msg.len().min(160)]));
            if let Err(e) = protocol.handle(&msg) {
                return shared.mark_dead(&e.to_string());
            }
        }
        if shared.queue.is_closed() {
            return;
        }
    }
}

fn writer_loop<W: Write>(mut writer: W, shared: Arc<Shared>) {
    while let Some((id, cmd)) = shared.queue.pop_blocking() {
        let session = shared.session.get().map(String::as_str);
        if cmd.scope == Scope::Page && session.is_none() {
            continue;
        }
        let bytes = cmd.encode(id, session);
        tracing::trace!(target: "bilterm::browser", "cdp -> {}", String::from_utf8_lossy(&bytes[..bytes.len().min(160)]));
        if let Err(e) = writer.write_all(&bytes).and_then(|()| writer.flush()) {
            return shared.mark_dead(&format!("write to Chrome failed: {e}"));
        }
    }
}

/// Spawns the reader and writer threads over an already connected transport.
pub fn spawn_session<R, W>(reader: R, writer: W, shared: Arc<Shared>) -> std::io::Result<SessionThreads>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    let (rs, ws) = (Arc::clone(&shared), Arc::clone(&shared));
    let writer = std::thread::Builder::new().name("bilterm-cdp-writer".into()).spawn(move || writer_loop(writer, ws))?;
    let reader = std::thread::Builder::new()
        .name("bilterm-cdp-reader".into())
        .spawn(move || reader_loop(reader, rs))
        .inspect_err(|_| shared.queue.close())?;
    Ok(SessionThreads { reader, writer })
}

#[cfg(test)]
#[path = "cdp_session_tests.rs"]
mod tests;
