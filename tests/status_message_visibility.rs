//! A pane status message (feedback for `:agents list`, `:mcp pin`, ...) must stay visible on the
//! OIA row of the non-Legacy profiles even when the detail segments would push it off the edge.
//! Legacy keeps its historical layout byte-for-byte (covered by the Classic parity goldens).

use bilterm::pane::CompositePane;
use bilterm::ui::glyphs::GlyphSet;
use bilterm::ui::oia::render_oia_status_bar_themed;
use bilterm::ui::theme::ThemePalette;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn row_text(buf: &Buffer, w: u16) -> String {
    (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

fn render(profile: &str, msg: Option<&str>, w: u16) -> String {
    let mut pane = CompositePane::new_headless(1, 24, 120);
    pane.set_status_message(msg.map(str::to_string));
    let palette = ThemePalette { name: profile.to_string(), ..ThemePalette::default() };
    let rect = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(rect);
    render_oia_status_bar_themed(&pane, rect, &palette, &GlyphSet::default(), &mut buf);
    row_text(&buf, w)
}

#[test]
fn obsidian_row_shows_a_message_that_would_overflow() {
    let msg = "Agents: 0 working · 0 blocked · 1 idle · 0 done · 0 errored | #1 home";
    let row = render("obsidian", Some(msg), 100);
    assert!(row.contains("Agents: 0 working"), "got: {row}");
}

#[test]
fn obsidian_row_without_a_message_keeps_the_detail_segments() {
    let row = render("obsidian", None, 100);
    assert!(row.contains("Ln") || row.contains("OUT:"), "got: {row}");
}
