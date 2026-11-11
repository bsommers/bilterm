//! Shared headless render fixture: a realistic multi-pane workspace with coloured output.
//! Used by the render-cache equivalence tests and by the `#[ignore]`d frame-time harness.
#![allow(dead_code)]

use ratatui::layout::Rect;

use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::WindowManager;

/// Deterministic coloured terminal output for pane `seed`: SGR 16/256/TrueColor foregrounds,
/// highlighted backgrounds, inverse runs, bold/underline, a URL per few lines and wide glyphs.
pub fn coloured_output(seed: usize, lines: usize) -> Vec<u8> {
    let mut out = String::new();
    for i in 0..lines {
        let n = seed * 131 + i;
        match n % 8 {
            0 => out.push_str(&format!(
                "\x1b[1;32m   Compiling\x1b[0m crate-{n} v0.{}.{} (\x1b[4;34mhttps://example.org/crates/{n}\x1b[0m)\r\n",
                n % 9,
                n % 13
            )),
            1 => out.push_str(&format!(
                "\x1b[38;5;{}mwarning\x1b[0m: unused variable `\x1b[1mvalue_{n}\x1b[0m` \x1b[38;5;244m--> src/lib.rs:{}:{}\x1b[0m\r\n",
                (n % 200) + 20,
                n % 900,
                n % 80
            )),
            2 => out.push_str(&format!(
                "\x1b[48;5;22m\x1b[38;5;120m+ let total_{n} = items.iter().map(|i| i.cost).sum::<u64>();\x1b[0m\r\n"
            )),
            3 => out.push_str(&format!(
                "\x1b[48;5;52m\x1b[38;5;217m- let total_{n} = 0; // TODO ERROR remove\x1b[0m\r\n"
            )),
            4 => {
                for k in 0..40usize {
                    let r = (k * 6 + n) % 256;
                    let g = (k * 3 + n * 7) % 256;
                    let b = (255 + n - k * 5) % 256;
                    out.push_str(&format!("\x1b[38;2;{r};{g};{b}m#\x1b[48;2;{b};{r};{g}m \x1b[0m"));
                }
                out.push_str("\r\n");
            }
            5 => out.push_str(&format!(
                "\x1b[7m STATUS \x1b[0m \x1b[33mjob {n}\x1b[0m \x1b[2mdim text\x1b[0m \x1b[3;35mitalic\x1b[0m see http://localhost:{}/job/{n}\r\n",
                3000 + n % 100
            )),
            6 => out.push_str(&format!(
                "\x1b[36m{:>6}\x1b[0m │ \x1b[31mERROR\x1b[0m 日本語 wide ✓ plain text row number {n} with some trailing words\r\n",
                n
            )),
            _ => out.push_str(&format!(
                "plain default-colour output line {n} lorem ipsum dolor sit amet consectetur\r\n"
            )),
        }
    }
    out.into_bytes()
}

/// Builds a tiled headless workspace of `panes` panes at `cols`x`rows`, every pane filled with
/// [`coloured_output`].
pub fn build_workspace(cols: u16, rows: u16, panes: usize) -> (WindowManager, Rect) {
    let area = Rect::new(0, 0, cols, rows);
    let mut wm = WindowManager::new_headless(rows, cols);
    for i in 1..panes {
        let dir = if i % 2 == 1 { SplitDirection::Vertical } else { SplitDirection::Horizontal };
        wm.split_focused(dir).expect("split");
    }
    wm.resize_all(area).expect("resize");
    let ids: Vec<_> = wm.compute_workspace_rects(area).into_iter().map(|(id, _)| id).collect();
    for (k, id) in ids.into_iter().enumerate() {
        if let Some(p) = wm.pane_mut(id) {
            p.process_pty_output(&coloured_output(k + 1, usize::from(rows) + 20));
        }
    }
    (wm, area)
}
