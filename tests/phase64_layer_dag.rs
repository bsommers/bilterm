//! Phase 64 layer-DAG and byte-boundary audit fixtures (TMON-05, TRAFFIC-INV-01).

use std::path::{Path, PathBuf};

fn visit_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn code_only(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn traffic_layer_is_a_leaf() {
    let mut files = Vec::new();
    visit_rs_files(&src_dir().join("traffic"), &mut files);
    assert!(files.len() >= 7, "expected the traffic layer files, got {files:?}");
    for f in files {
        for (n, line) in code_only(&f).lines().enumerate() {
            let mut rest = line;
            while let Some(i) = rest.find("crate::") {
                let tail = &rest[i + "crate::".len()..];
                assert!(
                    tail.starts_with("traffic"),
                    "{}:{} imports outside the leaf layer: {}",
                    f.display(),
                    n + 1,
                    line.trim()
                );
                rest = tail;
            }
        }
    }
}

#[test]
fn traffic_is_not_imported_by_config_root_and_existing_dag_holds() {
    // config/mod.rs stays free of crate::traffic; conversion lives in config/traffic.rs.
    assert!(!code_only(&src_dir().join("config/mod.rs")).contains("crate::traffic"));
    // pty, vt, pane may import crate::traffic; their other forbidden imports still hold.
    let forbid: &[(&str, &[&str])] = &[
        ("pty", &["crate::vt", "crate::pane", "crate::wm", "crate::ui", "crate::api"]),
        ("vt", &["crate::pane", "crate::wm", "crate::ui", "crate::api"]),
        ("pane", &["crate::wm", "crate::ui", "crate::api", "crate::config"]),
    ];
    for (dir, bad) in forbid {
        let mut files = Vec::new();
        visit_rs_files(&src_dir().join(dir), &mut files);
        for f in files {
            let code = code_only(&f);
            for b in *bad {
                assert!(!code.contains(b), "{} imports {b}", f.display());
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HookStatus {
    Pending,
    Done,
}

use HookStatus::Done;

/// `(file, verbatim anchor, tap call that must appear in the file once Done, status)`.
/// Plan 64-02 flipped entries to `Done`; MCP stdio (H26/H27) is `Deferred` to 64-03.
pub const HOOK_MANIFEST: &[(&str, &str, &str, HookStatus)] = &[
    ("src/pty/mod.rs", "match reader.read(&mut buf) {", ".rx(", Done),
    ("src/pty/writer.rs", "let written = sink.write_all(&chunk).and_then(|()| sink.flush());", ".tx(", Done),
    ("src/vt/worker.rs", "match pty_reader.read(&mut buf) {", ".rx(", Done),
    ("src/vt/worker.rs", "let _ = writer.write_all(&data);", ".tx(", Done),
    ("src/vt/worker.rs", "let _ = writer.write_all(&resp);", ".tx(", Done),
    ("src/vt/worker.rs", "let _ = writer.write_all(&resp);", ".tx(", Done),
    ("src/pane/mod.rs", "pub fn process_pty_output(&mut self, bytes: &[u8]) {", "observe", Done),
    ("src/pane/mod.rs", "pub fn ingest_pty_events(&mut self, events: Vec<PtyEvent>) -> bool {", "observe", Done),
    ("src/pane/mod.rs", "pub fn write_to_pty(&mut self, bytes: &[u8]) -> Result<()> {", "count_pty_less_tx", Done),
    ("src/pane/mod.rs", "fn drain_osc52_decoded(&mut self) {", "count_event", Done),
    ("src/pane/mod.rs", "pub fn record_clipboard_event(", "count_event", Done),
    ("src/pane/mod.rs", "pub fn record_media_passthrough(&mut self, mime: impl Into<String>, bytes_len: usize) {", "count_event", Done),
    ("src/tmux/mod.rs", "pane.process_pty_output(data);", "count_payload_rx", Done),
    ("src/wm/tmux_io.rs", "pane.process_pty_output(&rendered);", "count_payload_rx", Done),
    ("src/wm/tmux_link.rs", "pane.process_pty_output(&poll.raw_output);", "count_payload_rx", Done),
    ("src/tmux/mod.rs", "if stdin.write_all(cmd.as_bytes()).await.is_err() {", ".tx(", Done),
    ("src/tmux/mod.rs", "while let Ok(Some(line)) = lines.next_line().await {", ".rx(", Done),
    ("src/tmux/transport.rs", "let pty = PtySession::spawn(cfg)?;", "traffic_cell", Done),
    ("src/api/server.rs", "serve_jsonrpc_connection(stream, req_tx, token)", "Metered", Done),
    ("src/api/http.rs", ".read(&mut chunk)", "Metered", Done),
    ("src/api/http.rs", "w.write_all(head.as_bytes()).await?;", "Metered", Done),
    ("src/api/http.rs", "stream.write_all(out.as_bytes()).await?;", "Metered", Done),
    ("src/api/http.rs", "r = stream.read(&mut probe) => {", "Metered", Done),
    ("src/api/ws.rs", "r = stream.read(&mut chunk) => {", ".rx(", Done),
    ("src/api/ws.rs", "stream.write_all(&wire).await?;", ".tx(", Done),
    ("src/api/mcp.rs", "while let Some(line) = lines.next_line().await? {", ".rx(", Done),
    ("src/api/mcp.rs", "stdout.write_all(format!(\"{resp}\\n\").as_bytes()).await?;", ".tx(", Done),
    ("src/app.rs", "out.write_all(wire.as_bytes())?;", ".tx(", Done),
    ("src/main.rs", "let backend = CrosstermBackend::new(bilterm::app::CountingStdout::new(", "Counting", Done),
    ("src/app.rs", "let reader = EventStream::new();", "input_tap", Done),
    ("src/clipboard.rs", "let _ = stdin.write_all(text.as_bytes());", "traffic_tap()", Done),
    ("src/clipboard.rs", "let _ = out.read_to_end(&mut buf);", "traffic_tap()", Done),
    ("src/stt/provider.rs", ".write_all(&request)", ".tx(", Done),
];

/// Files containing an I/O primitive that are deliberately not terminal traffic.
pub const BOUNDARY_EXEMPT: &[(&str, &str)] = &[
    ("src/api/server.rs", "client fns belong to the separate `bilterm ctl` process (server side is in HOOK_MANIFEST)"),
    ("src/api/tests.rs", "test code"),
    ("src/secure_fs.rs", "file I/O, not terminal traffic"),
    ("src/pane/history_store.rs", "file I/O, not terminal traffic"),
    ("src/vt/recording.rs", "file I/O, not terminal traffic"),
    ("src/swarm/heal.rs", "verify_cmd child-process pipes, not terminal traffic"),
    ("src/daemon/supervisor.rs", "process supervision, not terminal traffic"),
    ("src/stt/wav.rs", "file I/O, not terminal traffic"),
    ("src/stt/audio.rs", "microphone capture child process, not terminal traffic"),
    ("src/stt/mod.rs", "connect-only reachability probe of the STT endpoint in VoicePreflight; no bytes are exchanged and the stream is dropped"),
    ("src/pane/video_canvas/ffmpeg.rs", "video decode child process pipes, not terminal traffic"),
    ("src/swarm/heal.rs", "self-heal verify-command child process pipes, not terminal traffic"),
    ("src/daemon/mod.rs", "connect-only liveness probe of the daemon socket; no bytes are exchanged and the stream is dropped (64-02 Task 0 audit)"),
    ("src/swarm/heal.rs", "verify_cmd child-process stdout/stderr pipes for the Phase 69 self-heal loop, not terminal traffic"),
    ("src/daemon/process.rs", "connect-only liveness probe of the daemon socket; no bytes are exchanged and the stream is dropped (64-02 Task 0 audit)"),
    ("src/swarm/heal.rs", "self-heal verify_cmd child process in a worktree, not terminal traffic"),
];

/// Taps implemented by a wrapper or accessor type rather than an adjacent call.
const FILE_WIDE_TAPS: &[&str] = &["Metered", "CountingStdout", "traffic_cell", "input_tap", "traffic_tap()"];
const HOOK_WINDOW: usize = 40;

const IO_PRIMITIVES: &[&str] = &[
    "try_clone_reader",
    "take_writer",
    "TcpStream::connect",
    "UnixStream::connect",
    "tokio::io::stdin",
    "Stdio::piped",
    ".accept()",
];

fn uses_io_primitive(code: &str) -> bool {
    IO_PRIMITIVES.iter().any(|p| code.contains(p))
}

fn is_known_boundary_file(rel: &str) -> bool {
    HOOK_MANIFEST.iter().any(|(file, ..)| *file == rel)
        || BOUNDARY_EXEMPT.iter().any(|(file, _)| *file == rel)
}

#[test]
fn audit_rejects_unlisted_io_file() {
    let synthetic = "fn x() { let _s = std::net::TcpStream::connect(\"h:1\"); }";
    assert!(uses_io_primitive(synthetic));
    assert!(!is_known_boundary_file("src/synthetic_new_io.rs"));
    assert!(is_known_boundary_file("src/pty/mod.rs"));
}

#[test]
fn no_hook_is_pending() {
    assert!(HOOK_MANIFEST.iter().all(|(.., st)| *st != HookStatus::Pending));
}

#[test]
fn hook_manifest_anchors_are_valid() {
    assert_eq!(HOOK_MANIFEST.len(), 33, "research inventory lists 33 hook sites");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (file, anchor, tap, status) in HOOK_MANIFEST {
        let text = std::fs::read_to_string(root.join(file))
            .unwrap_or_else(|e| panic!("manifest file {file} unreadable: {e}"));
        assert!(text.contains(anchor), "{file}: anchor not found: {anchor}");
        if *status == HookStatus::Done {
            assert!(text.contains(tap), "{file}: Done hook lost its tap call `{tap}` ({anchor})");
            if !FILE_WIDE_TAPS.contains(tap) {
                let lines: Vec<&str> = text.lines().collect();
                let at = lines.iter().position(|l| l.contains(anchor)).unwrap();
                let (lo, hi) = (at.saturating_sub(HOOK_WINDOW), (at + HOOK_WINDOW).min(lines.len()));
                assert!(
                    lines[lo..hi].iter().any(|l| l.contains(tap)),
                    "{file}: tap `{tap}` is not within {HOOK_WINDOW} lines of anchor `{anchor}`"
                );
            }
        }
    }
}

#[test]
fn every_io_primitive_file_is_in_manifest_or_exempt() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    visit_rs_files(&src_dir(), &mut files);
    for f in files {
        let rel = f.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        let code = code_only(&f);
        let known = is_known_boundary_file(&rel);
        assert!(known || !uses_io_primitive(&code), "{rel} uses a byte-boundary I/O primitive but is in neither HOOK_MANIFEST nor BOUNDARY_EXEMPT");
    }
}

#[test]
fn exemptions_carry_reasons() {
    for (f, why) in BOUNDARY_EXEMPT {
        assert!(!why.is_empty(), "{f}");
    }
}
