use std::io;

use bilterm::app::App;
use bilterm::error::TerminalRestoreGuard;
use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
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
