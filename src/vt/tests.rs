use ratatui::style::{Color, Modifier};

use super::*;
use crate::ui::cell_to_ratatui_style;

#[test]
fn parses_truecolor_and_256_color_cells() {
    let mut vt = VtParser::new(24, 80, 1000);
    vt.process(b"\x1b[38;2;12;34;56;48;5;208;1;3;4mA\x1b[0m");
    let cell = vt.screen().cell(0, 0).expect("cell (0,0)");
    assert_eq!(cell.contents(), "A");
    let style = cell_to_ratatui_style(cell);
    assert_eq!(style.fg, Some(Color::Rgb(12, 34, 56)));
    assert_eq!(style.bg, Some(Color::Indexed(208)));
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn tracks_decset_1049_1047_and_47_alternate_screen() {
    let mut vt = VtParser::new(24, 80, 1000);
    assert!(!vt.is_alternate_screen());

    vt.process(b"\x1b[?1049h");
    assert!(vt.is_alternate_screen());
    vt.process(b"\x1b[?1049l");
    assert!(!vt.is_alternate_screen());

    vt.process(b"\x1b[?1047h");
    assert!(vt.is_alternate_screen());
    vt.process(b"\x1b[?1047l");
    assert!(!vt.is_alternate_screen());

    vt.process(b"\x1b[?47h");
    assert!(vt.is_alternate_screen());
    vt.process(b"\x1b[?47l");
    assert!(!vt.is_alternate_screen());
}

#[test]
fn parses_osc_133_prompt_and_command_markers() {
    let mut vt = VtParser::new(24, 80, 1000);
    vt.process(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandStart));

    vt.record_command_submission("ls -la");
    vt.process(b"\x1b]133;C\x07file1.txt\r\n");
    assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandExecuted));

    vt.process(b"\x1b]133;D;0\x07");
    assert_eq!(vt.osc133_phase(), Some(Osc133Phase::CommandFinished(Some(0))));
    assert_eq!(vt.command_blocks().len(), 1);
    assert_eq!(vt.command_blocks()[0].exit_code, Some(0));
    assert!(vt.command_blocks()[0].duration_ms.is_some());
    assert_eq!(vt.last_exit_code(), Some(0));
}

#[test]
fn supports_scrollback_navigation() {
    let mut vt = VtParser::new(5, 40, 100);
    for i in 0..15 {
        vt.process(format!("line {i}\r\n").as_bytes());
    }
    assert_eq!(vt.scrollback_offset(), 0);
    vt.scroll_up(4);
    assert!(vt.scrollback_offset() > 0);
    vt.reset_scrollback();
    assert_eq!(vt.scrollback_offset(), 0);
}

#[test]
fn tracks_decset_2004_bracketed_paste_and_osc52_clipboard() {
    let mut vt = VtParser::new(10, 40, 100);
    assert!(!vt.is_bracketed_paste());
    vt.process(b"\x1b[?2004h");
    assert!(vt.is_bracketed_paste());
    vt.process(b"\x1b[?2004l");
    assert!(!vt.is_bracketed_paste());

    let seq = encode_osc52_clipboard("bilterm-3270");
    assert_eq!(seq, "\x1b]52;c;YmlsdGVybS0zMjcw\x07");

    vt.process(seq.as_bytes());
    let captured = vt.take_outbound_osc52();
    assert_eq!(captured, vec![seq]);
}

#[test]
fn jumps_between_osc133_command_blocks_in_scrollback() {
    let mut vt = VtParser::new(4, 40, 200);
    vt.record_command_submission("cmd-one");
    for i in 0..8 {
        vt.process(format!("out1-{i}\r\n").as_bytes());
    }
    vt.record_command_submission("cmd-two");
    for i in 0..8 {
        vt.process(format!("out2-{i}\r\n").as_bytes());
    }

    let b2 = vt.jump_to_prev_command_block().expect("jump to cmd-two");
    assert_eq!(b2.command, "cmd-two");
    let b1 = vt.jump_to_prev_command_block().expect("jump to cmd-one");
    assert_eq!(b1.command, "cmd-one");
    let b2_again = vt.jump_to_next_command_block().expect("jump forward to cmd-two");
    assert_eq!(b2_again.command, "cmd-two");
    assert!(vt.jump_to_next_command_block().is_none());
    assert_eq!(vt.scrollback_offset(), 0);
}

#[test]
fn sanitizes_paste_control_bytes_and_detects_unsafe_paste() {
    let malicious = "echo safe\x1b[2J\x03\x7f\x00\nrm -rf /";
    let clean = sanitize_paste_bytes(malicious);
    assert!(!clean.contains('\x1b'));
    assert!(!clean.contains('\x03'));
    assert!(!clean.contains('\x7f'));
    assert!(!clean.contains('\x00'));
    assert!(clean.contains("echo safe"));
    assert!(clean.contains("\nrm -rf /"));

    // Multi-line paste is unsafe when bracketed paste (?2004) is off
    assert!(is_unsafe_paste("line1\nline2", false));
    assert!(!is_unsafe_paste("line1\nline2", true));
    // Embedded bracketed-paste end marker is always unsafe
    assert!(is_unsafe_paste("evil\x1b[201~payload", true));
}

#[test]
fn replies_to_decrpm_kitty_keyboard_osc_colors_and_xtgettcap() {
    let mut vt = VtParser::new(24, 80, 200);
    // Default ?1007 is set (1), ?2004 is reset (2)
    vt.process(b"\x1b[?1007$p\x1b[?2004$p\x1b[4$p");
    let replies = vt.take_query_responses();
    assert_eq!(
        replies,
        vec![
            b"\x1b[?1007;1$y".to_vec(),
            b"\x1b[?2004;2$y".to_vec(),
            b"\x1b[4;2$y".to_vec(),
        ]
    );

    // Kitty keyboard push/query/pop
    vt.process(b"\x1b[>5u\x1b[?u");
    assert_eq!(vt.kitty_keyboard_flags(), 5);
    assert_eq!(vt.take_query_responses(), vec![b"\x1b[?5u".to_vec()]);
    vt.process(b"\x1b[<1u");
    assert_eq!(vt.kitty_keyboard_flags(), 0);

    // OSC 10 / 11 color queries
    vt.set_theme_colors((0x55, 0xFF, 0x55), (0x00, 0x00, 0xA8), (0xFF, 0xFF, 0x55));
    vt.process(b"\x1b]10;?\x07\x1b]11;?\x1b\\");
    let osc_replies = vt.take_query_responses();
    assert_eq!(
        osc_replies,
        vec![
            b"\x1b]10;rgb:5555/ffff/5555\x1b\\".to_vec(),
            b"\x1b]11;rgb:0000/0000/a8a8\x1b\\".to_vec(),
        ]
    );

    // XTGETTCAP query for "TN" (544E) and "RGB" (524742)
    vt.process(b"\x1bP+q544E;524742\x1b\\");
    let cap_replies = vt.take_query_responses();
    assert_eq!(cap_replies.len(), 2);
    assert!(String::from_utf8_lossy(&cap_replies[0]).starts_with("\x1bP1+r544E="));
    assert!(String::from_utf8_lossy(&cap_replies[1]).starts_with("\x1bP1+r524742="));
}

#[test]
fn detects_urls_and_searches_canvas_grid() {
    let mut vt = VtParser::new(6, 60, 100);
    vt.process(b"Visit https://github.com/ghostty-org/ghostty for details.\r\n");
    vt.process(b"\x1b]8;id=doc;https://ratatui.rs\x07Ratatui Docs\x1b]8;;\x07\r\n");

    let urls = vt.detect_visible_urls();
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://github.com/ghostty-org/ghostty");
    assert_eq!(
        vt.url_at(0, 10).as_deref(),
        Some("https://github.com/ghostty-org/ghostty")
    );
    assert_eq!(vt.osc8_links().len(), 1);
    assert_eq!(vt.osc8_links()[0].uri, "https://ratatui.rs");

    let matches = vt.search_canvas("GHOSTTY");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].row, 0);
}

fn history_text(vt: &mut VtParser) -> String {
    let used = vt.scrollback_used_rows();
    let (rows, _) = vt.size();
    let mut out = String::new();
    let mut j = 0;
    while j < used {
        let off = used - j;
        vt.parser.screen_mut().set_scrollback(off);
        let take = usize::from(rows).min(off);
        for r in 0..take {
            let (_, cols) = vt.parser.screen().size();
            for c in 0..cols {
                if let Some(cell) = vt.parser.screen().cell(r as u16, c) {
                    out.push_str(cell.contents());
                }
            }
            out.push('\n');
        }
        j += take;
    }
    vt.parser.screen_mut().set_scrollback(0);
    out
}

#[test]
fn column_resize_preserves_scrollback_beyond_primary_log_cap() {
    let mut vt = VtParser::new(24, 80, 50_000);
    let mut bytes = Vec::new();
    for i in 0..20_000 {
        bytes.extend_from_slice(format!("line-{i:06}\r\n").as_bytes());
    }
    assert!(bytes.len() > 200_000);
    vt.process(&bytes);
    vt.set_size(24, 60);
    let text = history_text(&mut vt);
    assert!(
        text.contains("line-000000"),
        "oldest line lost after column resize"
    );
    assert!(text.contains("line-010000"));
    assert!(vt.scrollback_used_rows() >= 19_000);
    assert_eq!(vt.size(), (24, 60));
}

#[test]
fn rebuild_reflows_wrapped_lines_and_honours_new_capacity() {
    let mut vt = VtParser::new(5, 20, 1000);
    let long: String = "abcdefghij".repeat(4); // 40 cols -> 2 rows at width 20
    vt.process(format!("{long}\r\n").as_bytes());
    for i in 0..100 {
        vt.process(format!("n{i}\r\n").as_bytes());
    }
    vt.process(b"$ ");
    vt.rebuild(5, 40, 1000);
    let text = history_text(&mut vt);
    assert!(
        text.contains(&long),
        "wrapped line should be rejoined at width 40"
    );
    assert_eq!(vt.screen().cursor_position().1, 2);
    // Capacity shrink keeps only the newest lines.
    vt.rebuild(5, 40, 10);
    assert_eq!(vt.scrollback_capacity(), 10);
    assert!(vt.scrollback_used_rows() <= 10);
    let text = history_text(&mut vt);
    assert!(text.contains("n95"));
    assert!(!text.contains("n10\n"));
}

#[test]
fn same_size_rebuild_restores_screen_and_cursor_exactly() {
    let mut vt = VtParser::new(4, 20, 100);
    for i in 0..10 {
        vt.process(format!("r{i}\r\n").as_bytes());
    }
    vt.process(b"\x1b[31mred\x1b[0m $ ");
    let before = vt.screen().contents();
    let cursor = vt.screen().cursor_position();
    vt.set_scrollback_capacity(500);
    assert_eq!(vt.scrollback_capacity(), 500);
    assert_eq!(vt.screen().contents(), before);
    assert_eq!(vt.screen().cursor_position(), cursor);
    assert_eq!(
        vt.screen().cell(3, 0).unwrap().fgcolor(),
        vt100_ctt::Color::Idx(1)
    );
    assert!(history_text(&mut vt).contains("r0"));
}

#[test]
fn clear_scrollback_frees_history_and_skips_alt_screen() {
    let mut vt = VtParser::new(4, 20, 100);
    for i in 0..30 {
        vt.process(format!("r{i}\r\n").as_bytes());
    }
    vt.record_command_submission("old");
    assert!(vt.scrollback_used_rows() > 0);
    vt.clear_scrollback();
    assert_eq!(vt.scrollback_used_rows(), 0);
    assert!(vt.screen().contents().contains("r29"));
    vt.process(b"\x1b[?1049h");
    assert!(!vt.set_scrollback_capacity(50));
    assert_eq!(vt.scrollback_capacity(), 100);
}

#[test]
#[cfg(unix)]
fn private_recording_is_0600_and_exclusive() {
    use std::os::unix::fs::MetadataExt;
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.cast");
    let mut vt = VtParser::new(24, 80, 100);
    vt.start_recording_private(a.clone());
    vt.process(b"hello\r\n");
    let stopped = vt.stop_recording().unwrap();
    assert_eq!(stopped, Some(a.clone()));
    assert_eq!(
        std::fs::symlink_metadata(&a).unwrap().mode() & 0o777,
        0o600
    );

    let b = tmp.path().join("b.cast");
    std::fs::write(&b, b"existing content").unwrap();
    vt.start_recording_private(b.clone());
    vt.process(b"new content\r\n");
    let err = vt.stop_recording().unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&b).unwrap(), b"existing content");
}

#[test]
fn explicit_recording_path_overwrites_as_before() {
    let tmp = tempfile::tempdir().unwrap();
    let c = tmp.path().join("c.cast");
    std::fs::write(&c, b"initial").unwrap();
    let mut vt = VtParser::new(24, 80, 100);
    vt.start_recording(c.clone());
    vt.process(b"overwritten\r\n");
    let stopped = vt.stop_recording().unwrap();
    assert_eq!(stopped, Some(c.clone()));
    let contents = std::fs::read_to_string(&c).unwrap();
    assert!(contents.contains("overwritten"));
    assert!(!contents.contains("initial"));
}
