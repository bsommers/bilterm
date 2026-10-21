use std::io;

use bilterm::app::App;
use bilterm::error::TerminalRestoreGuard;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::wm::WindowManager;
use crossterm::terminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("bilterm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Bilterm v{} — IBM 3270 x Borland Turbo Vision Composite Terminal Multiplexer\n\
             Usage: bilterm [OPTIONS]\n\
             Options:\n  \
               --gui-dump <PATH.ppm>  Render initial workspace via GuiWorkspaceRenderer to a PPM framebuffer\n  \
               --version, -V          Print version information\n  \
               --help, -h             Print this help message",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if let Some(pos) = args.iter().position(|a| a == "--gui-dump")
        && let Some(out_path) = args.get(pos + 1)
    {
        let wm = WindowManager::new_headless(24, 80);
        let mut renderer = GuiWorkspaceRenderer::default();
        let mut surface = GuiFrameSurface::new(8, 16);
        renderer.render_workspace(&wm, Rect::new(0, 0, 80, 24), &mut surface);
        std::fs::write(out_path, surface.export_ppm())?;
        return Ok(());
    }

    let guard = TerminalRestoreGuard::new();
    guard.install_panic_hook()?;

    let file_appender = tracing_appender::rolling::never(".", "bilterm.log");
    let (non_blocking, _log_guard) = tracing_appender::non_blocking(file_appender);
    let _ = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .try_init();

    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let mut app = App::spawn_default(rows, cols)?;

    let host_guard = TerminalRestoreGuard::enter_host_tui()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let run_res = app.run(&mut terminal).await;
    host_guard.restore();
    run_res?;
    Ok(())
}
