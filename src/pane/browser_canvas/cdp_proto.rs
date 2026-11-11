//! Chrome DevTools Protocol wire format for `--remote-debugging-pipe`: NUL-terminated JSON
//! messages, command builders and event parsing (`WEB-01..02`).
//!
//! Pure data code: no I/O, no threads. Nothing here trusts Chrome: every field is optional and a
//! malformed message is an error value, never a panic.

use std::fmt;

use serde_json::{Map, Value, json};

/// Largest single CDP message accepted from Chrome. A bigger one is a fatal backend error.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Fatal protocol error raised by the framer or the parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtoError {
    Oversize(usize),
    BadJson(String),
    NotAnObject,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize(n) => write!(f, "CDP message exceeds {n} bytes"),
            Self::BadJson(e) => write!(f, "CDP message is not JSON: {e}"),
            Self::NotAnObject => write!(f, "CDP message is not a JSON object"),
        }
    }
}

/// Splits a byte stream into NUL-terminated messages with a size cap.
pub struct Framer {
    buf: Vec<u8>,
    max: usize,
}

impl Default for Framer {
    fn default() -> Self {
        Self::new(MAX_MESSAGE_BYTES)
    }
}

impl Framer {
    pub fn new(max: usize) -> Self {
        Self { buf: Vec::new(), max }
    }

    /// Feeds `chunk`; complete messages (without the NUL) are appended to `out`.
    pub fn push(&mut self, chunk: &[u8], out: &mut Vec<Vec<u8>>) -> Result<(), ProtoError> {
        let mut rest = chunk;
        while let Some(pos) = rest.iter().position(|&b| b == 0) {
            if self.buf.len() + pos > self.max {
                return Err(ProtoError::Oversize(self.max));
            }
            self.buf.extend_from_slice(&rest[..pos]);
            if !self.buf.is_empty() {
                out.push(std::mem::take(&mut self.buf));
            }
            rest = &rest[pos + 1..];
        }
        if self.buf.len() + rest.len() > self.max {
            return Err(ProtoError::Oversize(self.max));
        }
        self.buf.extend_from_slice(rest);
        Ok(())
    }

    /// Bytes buffered for an unfinished message.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

/// Which CDP session a command is sent on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Browser,
    Page,
}

/// What to do when the response to a command arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    None,
    GetTargets,
    CreateTarget,
    Attach,
    Navigate,
    /// `Page.getNavigationHistory`; the offset is `-1` back, `+1` forward, `0` refresh only.
    History(i8),
}

/// Queue class: decides coalescing and what may be dropped under pressure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Normal,
    MouseMove,
    Viewport,
    /// Protocol bookkeeping (acks, handshake). Never dropped.
    Internal,
}

/// One outgoing CDP command, before an id is assigned.
#[derive(Clone, Debug, PartialEq)]
pub struct Cmd {
    pub method: &'static str,
    pub params: Value,
    pub scope: Scope,
    pub tag: Tag,
    pub class: Class,
}

impl Cmd {
    fn page(method: &'static str, params: Value) -> Self {
        Self { method, params, scope: Scope::Page, tag: Tag::None, class: Class::Normal }
    }

    fn browser(method: &'static str, params: Value) -> Self {
        Self { method, params, scope: Scope::Browser, tag: Tag::None, class: Class::Internal }
    }

    fn tagged(mut self, tag: Tag) -> Self {
        self.tag = tag;
        self
    }

    pub fn with_class(mut self, class: Class) -> Self {
        self.class = class;
        self
    }

    /// Serializes to the wire form: one JSON object followed by a NUL byte.
    pub fn encode(&self, id: u64, session: Option<&str>) -> Vec<u8> {
        let mut obj = Map::new();
        obj.insert("id".into(), json!(id));
        obj.insert("method".into(), json!(self.method));
        obj.insert("params".into(), self.params.clone());
        if self.scope == Scope::Page
            && let Some(s) = session
        {
            obj.insert("sessionId".into(), json!(s));
        }
        let mut bytes = serde_json::to_vec(&Value::Object(obj)).unwrap_or_default();
        bytes.push(0);
        bytes
    }
}

pub fn get_targets() -> Cmd {
    Cmd::browser("Target.getTargets", json!({})).tagged(Tag::GetTargets)
}

pub fn create_target() -> Cmd {
    Cmd::browser("Target.createTarget", json!({ "url": "about:blank" })).tagged(Tag::CreateTarget)
}

pub fn attach_to_target(target_id: &str) -> Cmd {
    Cmd::browser("Target.attachToTarget", json!({ "targetId": target_id, "flatten": true })).tagged(Tag::Attach)
}

pub fn discover_targets() -> Cmd {
    Cmd::browser("Target.setDiscoverTargets", json!({ "discover": true }))
}

pub fn close_target(target_id: &str) -> Cmd {
    Cmd::browser("Target.closeTarget", json!({ "targetId": target_id }))
}

pub fn deny_downloads() -> Cmd {
    Cmd::browser("Browser.setDownloadBehavior", json!({ "behavior": "deny" }))
}

pub fn page_enable() -> Cmd {
    Cmd::page("Page.enable", json!({})).with_class(Class::Internal)
}

pub fn navigate(url: &str) -> Cmd {
    Cmd::page("Page.navigate", json!({ "url": url })).tagged(Tag::Navigate)
}

pub fn reload() -> Cmd {
    Cmd::page("Page.reload", json!({}))
}

pub fn stop_loading() -> Cmd {
    Cmd::page("Page.stopLoading", json!({}))
}

pub fn get_history(offset: i8) -> Cmd {
    let class = if offset == 0 { Class::Internal } else { Class::Normal };
    Cmd::page("Page.getNavigationHistory", json!({})).tagged(Tag::History(offset)).with_class(class)
}

pub fn navigate_to_history_entry(entry_id: i64) -> Cmd {
    Cmd::page("Page.navigateToHistoryEntry", json!({ "entryId": entry_id })).with_class(Class::Internal)
}

pub fn handle_dialog(accept: bool) -> Cmd {
    Cmd::page("Page.handleJavaScriptDialog", json!({ "accept": accept })).with_class(Class::Internal)
}

/// `width`/`height` are CSS pixels; the screencast frame is `scale` times larger.
pub fn set_device_metrics(css_width: u32, css_height: u32, scale: f64) -> Cmd {
    Cmd::page(
        "Emulation.setDeviceMetricsOverride",
        json!({ "width": css_width, "height": css_height, "deviceScaleFactor": scale, "mobile": false }),
    )
    .with_class(Class::Viewport)
}

pub fn start_screencast(max_width: u32, max_height: u32, quality: u8) -> Cmd {
    Cmd::page(
        "Page.startScreencast",
        json!({ "format": "jpeg", "quality": quality, "maxWidth": max_width, "maxHeight": max_height, "everyNthFrame": 1 }),
    )
    .with_class(Class::Viewport)
}

/// Chrome refuses `Page.startScreencast` while one is active, so a resize stops it first.
pub fn stop_screencast() -> Cmd {
    Cmd::page("Page.stopScreencast", json!({})).with_class(Class::Viewport)
}

pub fn screencast_ack(ack_id: i64) -> Cmd {
    Cmd::page("Page.screencastFrameAck", json!({ "sessionId": ack_id })).with_class(Class::Internal)
}

pub fn insert_text(text: &str) -> Cmd {
    Cmd::page("Input.insertText", json!({ "text": text }))
}

/// One parsed message from Chrome.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    Response { id: u64, result: Value, error: Option<String> },
    Event(Event),
    /// A well-formed message this client does not use.
    Ignored,
}

/// One entry of `Page.getNavigationHistory`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
}

/// CDP events this client reacts to.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    FrameNavigated { frame_id: String, is_main: bool, url: String },
    NavigatedWithinDocument { frame_id: String, url: String },
    LoadingStarted { frame_id: String },
    LoadingStopped { frame_id: String },
    LoadEventFired,
    TargetCreated { target_id: String, kind: String, opener: Option<String>, url: String },
    TargetInfoChanged { target_id: String, title: String, url: String },
    ScreencastFrame { data: String, ack_id: i64, device_width: f64, device_height: f64 },
    DialogOpening { kind: String },
    TargetCrashed,
    Detached,
}

fn take_str(map: &mut Map<String, Value>, key: &str) -> String {
    match map.remove(key) {
        Some(Value::String(s)) => s,
        _ => String::new(),
    }
}

fn take_obj(map: &mut Map<String, Value>, key: &str) -> Map<String, Value> {
    match map.remove(key) {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// Parses one framed message. Unknown methods are [`Incoming::Ignored`]; non-JSON is an error.
pub fn parse_incoming(bytes: &[u8]) -> Result<Incoming, ProtoError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| ProtoError::BadJson(e.to_string()))?;
    let Value::Object(mut msg) = value else {
        return Err(ProtoError::NotAnObject);
    };
    if let Some(id) = msg.get("id").and_then(Value::as_u64) {
        let error = msg.get("error").map(|e| {
            e.get("message").and_then(Value::as_str).map_or_else(|| e.to_string(), str::to_string)
        });
        let result = msg.remove("result").unwrap_or(Value::Null);
        return Ok(Incoming::Response { id, result, error });
    }
    let method = take_str(&mut msg, "method");
    let mut p = take_obj(&mut msg, "params");
    let event = match method.as_str() {
        "Page.frameNavigated" => {
            let mut frame = take_obj(&mut p, "frame");
            let is_main = !frame.contains_key("parentId");
            Event::FrameNavigated { frame_id: take_str(&mut frame, "id"), is_main, url: take_str(&mut frame, "url") }
        }
        "Page.navigatedWithinDocument" => {
            Event::NavigatedWithinDocument { frame_id: take_str(&mut p, "frameId"), url: take_str(&mut p, "url") }
        }
        "Page.frameStartedLoading" => Event::LoadingStarted { frame_id: take_str(&mut p, "frameId") },
        "Page.frameStoppedLoading" => Event::LoadingStopped { frame_id: take_str(&mut p, "frameId") },
        "Page.loadEventFired" => Event::LoadEventFired,
        "Target.targetCreated" | "Target.targetInfoChanged" => {
            let mut info = take_obj(&mut p, "targetInfo");
            let target_id = take_str(&mut info, "targetId");
            let url = take_str(&mut info, "url");
            if method == "Target.targetCreated" {
                let opener = match info.remove("openerId") {
                    Some(Value::String(s)) => Some(s),
                    _ => None,
                };
                Event::TargetCreated { target_id, kind: take_str(&mut info, "type"), opener, url }
            } else {
                Event::TargetInfoChanged { target_id, title: take_str(&mut info, "title"), url }
            }
        }
        "Page.screencastFrame" => {
            let meta = take_obj(&mut p, "metadata");
            Event::ScreencastFrame {
                ack_id: p.get("sessionId").and_then(Value::as_i64).unwrap_or(0),
                data: take_str(&mut p, "data"),
                device_width: meta.get("deviceWidth").and_then(Value::as_f64).unwrap_or(0.0),
                device_height: meta.get("deviceHeight").and_then(Value::as_f64).unwrap_or(0.0),
            }
        }
        "Page.javascriptDialogOpening" => Event::DialogOpening { kind: take_str(&mut p, "type") },
        "Inspector.targetCrashed" | "Target.targetCrashed" => Event::TargetCrashed,
        "Inspector.detached" | "Target.detachedFromTarget" => Event::Detached,
        _ => return Ok(Incoming::Ignored),
    };
    Ok(Incoming::Event(event))
}

/// Picks the first `page` target id out of a `Target.getTargets` result.
pub fn first_page_target(result: &Value) -> Option<String> {
    result
        .get("targetInfos")?
        .as_array()?
        .iter()
        .find(|t| t.get("type").and_then(Value::as_str) == Some("page"))
        .and_then(|t| t.get("targetId").and_then(Value::as_str))
        .map(str::to_string)
}

/// Parses a `Page.getNavigationHistory` result into `(current index, entries)`.
pub fn parse_history(result: &Value) -> Option<(usize, Vec<HistoryEntry>)> {
    let idx = usize::try_from(result.get("currentIndex")?.as_i64()?).ok()?;
    let entries: Vec<HistoryEntry> = result
        .get("entries")?
        .as_array()?
        .iter()
        .filter_map(|e| {
            Some(HistoryEntry {
                id: e.get("id")?.as_i64()?,
                url: e.get("url").and_then(Value::as_str).unwrap_or("").to_string(),
                title: e.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        })
        .collect();
    (idx < entries.len()).then_some((idx, entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framer_splits_nul_terminated_messages_across_chunks() {
        let mut f = Framer::default();
        let mut out = Vec::new();
        f.push(b"{\"a\":1}\0{\"b\"", &mut out).unwrap();
        assert_eq!(out, vec![b"{\"a\":1}".to_vec()]);
        assert_eq!(f.pending(), 4);
        f.push(b":2}\0\0{\"c\":3}\0", &mut out).unwrap();
        assert_eq!(out.len(), 3, "empty message between two NULs is skipped");
        assert_eq!(out[1], b"{\"b\":2}");
        assert_eq!(out[2], b"{\"c\":3}");
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn framer_rejects_oversize_messages() {
        let mut f = Framer::new(8);
        let mut out = Vec::new();
        f.push(b"12345", &mut out).unwrap();
        assert_eq!(f.push(b"6789", &mut out), Err(ProtoError::Oversize(8)));
        let mut g = Framer::new(8);
        assert_eq!(g.push(b"123456789\0", &mut out), Err(ProtoError::Oversize(8)));
        assert!(out.is_empty());
    }

    #[test]
    fn commands_encode_with_id_session_and_nul() {
        let bytes = navigate("https://example.com/").encode(7, Some("S1"));
        assert_eq!(bytes.last(), Some(&0));
        let v: Value = serde_json::from_slice(&bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["method"], "Page.navigate");
        assert_eq!(v["params"]["url"], "https://example.com/");
        assert_eq!(v["sessionId"], "S1");

        let b = get_targets().encode(1, Some("S1"));
        let v: Value = serde_json::from_slice(&b[..b.len() - 1]).unwrap();
        assert!(v.get("sessionId").is_none(), "browser-scope commands carry no session");
        assert!(!b[..b.len() - 1].contains(&0), "JSON never embeds a raw NUL");
    }

    #[test]
    fn command_builders_carry_expected_params() {
        let m = set_device_metrics(800, 460, 1.5);
        assert_eq!(m.params, json!({ "width": 800, "height": 460, "deviceScaleFactor": 1.5, "mobile": false }));
        assert_eq!(m.class, Class::Viewport);
        let s = start_screencast(1200, 690, 70);
        assert_eq!(s.params["format"], "jpeg");
        assert_eq!(s.params["maxWidth"], 1200);
        assert_eq!(s.params["everyNthFrame"], 1);
        assert_eq!(screencast_ack(42).params, json!({ "sessionId": 42 }));
        assert_eq!(deny_downloads().params, json!({ "behavior": "deny" }));
        assert_eq!(attach_to_target("T").params, json!({ "targetId": "T", "flatten": true }));
        assert_eq!(insert_text("a\0b").encode(1, None).iter().filter(|&&b| b == 0).count(), 1);
        assert_eq!(get_history(-1).tag, Tag::History(-1));
    }

    #[test]
    fn parses_responses_and_errors() {
        let ok = parse_incoming(br#"{"id":3,"result":{"frameId":"F"}}"#).unwrap();
        assert_eq!(ok, Incoming::Response { id: 3, result: json!({ "frameId": "F" }), error: None });
        let err = parse_incoming(br#"{"id":4,"error":{"code":-32000,"message":"boom"}}"#).unwrap();
        assert_eq!(err, Incoming::Response { id: 4, result: Value::Null, error: Some("boom".into()) });
    }

    #[test]
    fn parses_page_and_target_events() {
        let ev = |s: &str| match parse_incoming(s.as_bytes()).unwrap() {
            Incoming::Event(e) => e,
            other => panic!("expected event, got {other:?}"),
        };
        assert_eq!(
            ev(r#"{"method":"Page.frameNavigated","params":{"frame":{"id":"F","url":"https://a.b/"}}}"#),
            Event::FrameNavigated { frame_id: "F".into(), is_main: true, url: "https://a.b/".into() }
        );
        assert_eq!(
            ev(r#"{"method":"Page.frameNavigated","params":{"frame":{"id":"C","parentId":"F","url":"https://ad/"}}}"#),
            Event::FrameNavigated { frame_id: "C".into(), is_main: false, url: "https://ad/".into() }
        );
        assert_eq!(
            ev(r#"{"method":"Page.frameStartedLoading","params":{"frameId":"F"}}"#),
            Event::LoadingStarted { frame_id: "F".into() }
        );
        assert_eq!(ev(r#"{"method":"Page.loadEventFired","params":{"timestamp":1}}"#), Event::LoadEventFired);
        assert_eq!(
            ev(r#"{"method":"Target.targetInfoChanged","params":{"targetInfo":{"targetId":"T","title":"Hi","url":"https://a.b/"}}}"#),
            Event::TargetInfoChanged { target_id: "T".into(), title: "Hi".into(), url: "https://a.b/".into() }
        );
        assert_eq!(
            ev(r#"{"method":"Target.targetCreated","params":{"targetInfo":{"targetId":"P","type":"page","openerId":"T","url":""}}}"#),
            Event::TargetCreated { target_id: "P".into(), kind: "page".into(), opener: Some("T".into()), url: String::new() }
        );
        assert_eq!(
            ev(r#"{"method":"Page.screencastFrame","params":{"data":"QUJD","sessionId":9,"metadata":{"deviceWidth":800,"deviceHeight":460.5}}}"#),
            Event::ScreencastFrame { data: "QUJD".into(), ack_id: 9, device_width: 800.0, device_height: 460.5 }
        );
        assert_eq!(ev(r#"{"method":"Inspector.targetCrashed","params":{}}"#), Event::TargetCrashed);
        assert_eq!(ev(r#"{"method":"Page.javascriptDialogOpening","params":{"type":"alert"}}"#), Event::DialogOpening { kind: "alert".into() });
    }

    #[test]
    fn garbage_is_an_error_and_unknown_events_are_ignored() {
        assert!(matches!(parse_incoming(b"not json"), Err(ProtoError::BadJson(_))));
        assert_eq!(parse_incoming(b"[1,2]"), Err(ProtoError::NotAnObject));
        assert_eq!(parse_incoming(br#"{"method":"Network.requestWillBeSent","params":{}}"#).unwrap(), Incoming::Ignored);
        // Wrong field types never panic: they degrade to empty values.
        let e = parse_incoming(br#"{"method":"Page.frameNavigated","params":{"frame":7}}"#).unwrap();
        assert_eq!(e, Incoming::Event(Event::FrameNavigated { frame_id: String::new(), is_main: true, url: String::new() }));
        assert_eq!(parse_incoming(br#"{"method":5}"#).unwrap(), Incoming::Ignored);
    }

    #[test]
    fn history_and_target_helpers() {
        let h = json!({ "currentIndex": 1, "entries": [
            { "id": 10, "url": "https://a/", "title": "A" },
            { "id": 11, "url": "https://b/", "title": "B" }
        ]});
        let (idx, entries) = parse_history(&h).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(entries[0], HistoryEntry { id: 10, url: "https://a/".into(), title: "A".into() });
        assert!(parse_history(&json!({ "currentIndex": 5, "entries": [] })).is_none());
        assert!(parse_history(&json!({ "currentIndex": -1, "entries": [] })).is_none());
        let t = json!({ "targetInfos": [ { "targetId": "B", "type": "browser" }, { "targetId": "P", "type": "page" } ] });
        assert_eq!(first_page_target(&t).as_deref(), Some("P"));
        assert_eq!(first_page_target(&json!({})), None);
    }
}
