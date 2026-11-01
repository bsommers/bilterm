//! Phase 44 preview harness (additive, read-only against the existing renderer).
//!
//! Renders the CURRENT visual styles x themes x scenes to PPM files through the real
//! `GuiWorkspaceRenderer` so a redesign can be compared against a true baseline.
//!
//! Usage: `cargo run -q --example design_preview -- <out_dir>`

use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::ui::theme::ThemePreset;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

const COLS: u16 = 110;
const ROWS: u16 = 34;

const SAMPLE: &str = "\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~/src/bilterm\x1b[0m$ cargo build\r\n\
   \x1b[1;32mCompiling\x1b[0m bilterm v2.5.4\r\n\
   \x1b[1;33mwarning\x1b[0m: unused variable `x`\r\n\
    \x1b[1;32mFinished\x1b[0m dev profile in 4.21s\r\n\
\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~/src/bilterm\x1b[0m$ ls --color\r\n\
\x1b[34mdocs\x1b[0m  \x1b[34msrc\x1b[0m  \x1b[34mtests\x1b[0m  Cargo.toml  README.md\r\n";

fn style(wm: &mut WindowManager, name: &str) {
    let c = wm.config_mut();
    c.snazzy_mode = false;
    c.glass_mode = false;
    c.modern_mode = false;
    c.liquid_glass = false;
    match name {
        "classic" => {}
        "snazzy" => c.snazzy_mode = true,
        "glass" => {
            c.snazzy_mode = true;
            c.glass_mode = true;
        }
        "modern" => c.modern_mode = true,
        "liquid" => {
            c.modern_mode = true;
            c.glass_mode = true;
            c.liquid_glass = true;
        }
        _ => unreachable!(),
    }
}

fn scene(name: &str, theme: &str, style_name: &str) -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    if let Some(t) = ThemePreset::from_name(theme) {
        wm.config_mut().theme = t;
    }
    style(&mut wm, style_name);
    let vp = Rect::new(0, 0, COLS, ROWS);
    if name != "single" {
        let _ = wm.split_focused(SplitDirection::Vertical);
        let _ = wm.resize_all(vp);
    }
    let ids: Vec<_> = (1..=4).collect();
    for id in ids {
        if let Some(p) = wm.pane_mut(id) {
            p.process_pty_output(SAMPLE.as_bytes());
        }
    }
    if name == "drawer" {
        wm.toggle_drawer();
    }
    wm
}

fn main() -> std::io::Result<()> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "preview_out".into());
    std::fs::create_dir_all(&out)?;
    let styles = ["classic", "snazzy", "glass", "modern", "liquid"];
    let themes = ["nord", "tron-grid"];
    let scenes = ["single", "split", "drawer"];
    for style_name in styles {
        for theme in themes {
            for sc in scenes {
                let wm = scene(sc, theme, style_name);
                let mut r = GuiWorkspaceRenderer::default();
                let mut s = GuiFrameSurface::new(8, 16);
                r.render_workspace(&wm, Rect::new(0, 0, COLS, ROWS), &mut s);
                std::fs::write(format!("{out}/{style_name}__{theme}__{sc}.ppm"), s.export_ppm())?;
            }
        }
    }
    println!("wrote {} previews to {out}", styles.len() * themes.len() * scenes.len());
    Ok(())
}
