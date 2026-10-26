use std::time::{Duration, Instant};

use bilterm::pty::{PtyConfig, PtyEvent, PtySession};
use bilterm::vt::VtParser;

#[tokio::test]
async fn pty_spawns_shell_executes_command_and_deduplicates_sigwinch() {
    let mut cfg = PtyConfig::default();
    if cfg!(windows) {
        cfg.program = "cmd.exe".to_string();
    } else {
        cfg.program = "/bin/sh".to_string();
    }
    cfg.rows = 24;
    cfg.cols = 80;

    let mut session = PtySession::spawn(cfg).expect("spawn pty");
    assert_eq!(session.size(), (24, 80));
    assert_eq!(session.sigwinch_count(), 0);

    // Resizing to identical dimensions must NOT emit SIGWINCH
    let resized_same = session.resize(24, 80).expect("resize same");
    assert!(!resized_same);
    assert_eq!(session.sigwinch_count(), 0);

    // Resizing to new dimensions MUST emit exactly one SIGWINCH
    let resized_new = session.resize(30, 100).expect("resize new");
    assert!(resized_new);
    assert_eq!(session.size(), (30, 100));
    assert_eq!(session.sigwinch_count(), 1);

    // Execute a command and verify output arrives in VtParser
    let cmd_bytes: &[u8] = if cfg!(windows) {
        b"echo BILTERM_PTY_OK_3270\r"
    } else {
        b"printf 'BILTERM_PTY_OK_%d\\n' 3270\n"
    };
    session.write_all(cmd_bytes).expect("write command");

    let mut vt = VtParser::new(30, 100, 500);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found = false;

    while Instant::now() < deadline {
        if let Ok(Some(PtyEvent::Output(bytes))) =
            tokio::time::timeout(Duration::from_millis(100), session.output_rx_mut().recv()).await
        {
            vt.process(&bytes);
            for resp in vt.take_query_responses() {
                let _ = session.write_all(&resp);
            }
            if vt.contents().contains("BILTERM_PTY_OK_3270") {
                found = true;
                break;
            }
        }
    }

    assert!(
        found,
        "Expected 'BILTERM_PTY_OK_3270' in VT screen contents, got: {:?}",
        vt.contents()
    );
}
