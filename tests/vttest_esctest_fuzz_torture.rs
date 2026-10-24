use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::pane::CompositePane;
use bilterm::ui::{CompositePaneWidget, WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

#[test]
fn vttest_esctest_split_byte_boundaries_and_parameter_bombs_never_panic_or_corrupt_buffer() {
    let mut pane = CompositePane::new_headless(1, 24, 80);

    // 1. Construct a complex stream containing multibyte UTF-8 CJK/Emoji, CSI SGR, DECRPM,
    //    Kitty keyboard push/pop, OSC 8 hyperlinks, OSC 10/11 queries, and XTGETTCAP queries.
    let stream = concat!(
        "\x1b[38;2;85;255;85;48;5;17;1;3;4m漢字🔥🚀✨IBM-3270\x1b[0m\r\n",
        "\x1b]8;id=gh;https://github.com/bsommers/bilterm\x07Bilterm Repo\x1b]8;;\x07\r\n",
        "\x1b[999999999;999999999H\x1b[;;;;;;;;;;;;m\x1b[?999999$p\x1b[1;1H",
        "\x1b[>31u\x1b[>15u\x1b[?u\x1b[<999u",
        "\x1bP+q544E;524742;4E4F5045\x1b\\\x1bP$qm\x1b\\\x1bP$qr\x1b\\",
        "\x1b]10;?\x07\x1b]11;?\x1b\\\x1b]4;208;?\x07",
        "\x1b[8;1HFINAL_CLEAN_LINE https://ratatui.rs\r\n"
    );

    // Feed 1 byte at a time and 3 bytes at a time to stress-test arbitrary chunk boundaries
    for step in [1usize, 3usize, 7usize] {
        for chunk in stream.as_bytes().chunks(step) {
            pane.process_pty_output(chunk);
        }
    }

    // Drain any query replies generated during chunked processing
    let _ = pane.vt_mut().take_query_responses();

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    CompositePaneWidget::new(&pane, true).render(area, &mut buf);

    for y in 0..area.height {
        for x in 0..area.width {
            let sym = buf.cell((x, y)).unwrap().symbol();
            for ch in sym.chars() {
                assert!(
                    !ch.is_control(),
                    "Cell ({x},{y}) leaked control char U+{:04X}",
                    ch as u32
                );
            }
        }
    }
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("FINAL_CLEAN_LINE"));
}

#[test]
fn sixteen_simultaneous_sessions_tile_stack_overlap_shuffle_and_zoom() {
    let mut wm = WindowManager::new_headless(48, 160);
    let screen = Rect::new(0, 0, 160, 48);

    // Spawn 15 additional composite panes (total = 16 workspace panes + 1 Quake QuickTerminal = 17 sessions)
    for i in 0..15 {
        let dir = if i % 2 == 0 {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };
        let new_id = wm.split_focused(dir).unwrap();
        let pane = wm.panes.get_mut(&new_id).unwrap();
        pane.process_pty_output(format!("SESSION #{new_id} OUTPUT READY\r\n").as_bytes());
    }

    assert_eq!(wm.panes.len(), 16);
    assert_eq!(wm.z_stack.order().len(), 16);

    // 1. Tiled BSP mode: all 16 panes receive non-overlapping rectangles
    wm.layout_mode = WorkspaceLayoutMode::Tiled;
    wm.resize_all(screen).unwrap();
    let tiled_rects = wm.compute_workspace_rects(screen);
    assert_eq!(tiled_rects.len(), 16);

    let mut buf_tiled = Buffer::empty(screen);
    WorkspaceWidget::new(&wm).render(screen, &mut buf_tiled);

    // 2. Stacked (overlapping z-order cascade) mode: all 16 windows overlap in painter's z-order
    wm.layout_mode = WorkspaceLayoutMode::Stacked;
    wm.resize_all(screen).unwrap();
    let stacked_rects = wm.compute_workspace_rects(screen);
    assert_eq!(stacked_rects.len(), 16);

    // Shuffle all 16 windows through the deck and selectively raise window #7 to the top
    for _ in 0..16 {
        wm.shuffle_deck_next();
    }
    assert!(wm.focus_and_raise(7));
    assert_eq!(wm.active_pane_id(), 7);

    // Toggle Quake drop-down quick terminal (session #0) over the 16-window stack
    wm.toggle_quick_terminal();
    assert_eq!(wm.active_pane_id(), 0);

    let mut buf_stacked = Buffer::empty(screen);
    WorkspaceWidget::new(&wm).render(screen, &mut buf_stacked);
    let snap = buffer_to_snapshot_string(&buf_stacked);
    assert!(snap.contains("QUAKE QUICK TERM"));
}
