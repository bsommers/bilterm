use std::io;

use bilterm::api::{
    AutomationControlPlane, JsonValue, Playbook, run_headless_tcp_server, run_mcp_stdio_server,
    send_rpc_request,
};
use bilterm::app::App;
use bilterm::config::BiltermConfig;
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
             Usage:\n  \
               bilterm [OPTIONS]\n  \
               bilterm ctl <ENDPOINT> <METHOD> [JSON_PARAMS]\n\
             Options:\n  \
               --mcp                        Run Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio\n  \
               --headless --listen <ADDR>   Run headless JSON-RPC 2.0 automation daemon on TCP/Unix socket\n  \
               --playbook <FILE.toml>       Execute a declarative TOML automation playbook and exit\n  \
               --gui-dump <PATH.ppm>        Render initial workspace via GuiWorkspaceRenderer to a PPM framebuffer\n  \
               --session <NAME>             Use named persistent command-history session (overrides history_session)\n  \
               --completions <bash|zsh|fish> Print shell completion script to stdout\n  \
               --man                        Print roff manpage (bilterm.1) to stdout\n  \
               --version, -V                Print version information\n  \
               --help, -h                   Print this help message",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("ctl") {
        let endpoint = args.get(1).map_or("127.0.0.1:7370", String::as_str);
        let method = args.get(2).map_or("workspace.status", String::as_str);
        let params = args
            .get(3)
            .and_then(|raw| JsonValue::parse(raw))
            .unwrap_or_else(|| JsonValue::obj([] as [(&str, JsonValue); 0]));
        let resp = send_rpc_request(endpoint, method, params).await?;
        println!("{resp}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--mcp") {
        run_mcp_stdio_server(24, 80).await?;
        return Ok(());
    }
    if let Some(pos) = args.iter().position(|a| a == "--playbook")
        && let Some(pb_path) = args.get(pos + 1)
    {
        let raw_toml = std::fs::read_to_string(pb_path)?;
        let pb = Playbook::from_toml_str(&raw_toml)?;
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        let report = plane
            .run_playbook(&pb)
            .map_err(|e| color_eyre::eyre::eyre!(e))?;
        println!(
            "{{\"playbook\":{:?},\"passed\":{},\"steps_completed\":{}}}",
            report.name, report.passed, report.steps_completed
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--headless")
        && let Some(pos) = args.iter().position(|a| a == "--listen")
        && let Some(bind_addr) = args.get(pos + 1)
    {
        run_headless_tcp_server(bind_addr, 24, 80).await?;
        return Ok(());
    }
    if let Some(pos) = args.iter().position(|a| a == "--completions")
        && let Some(shell) = args.get(pos + 1)
    {
        match shell.as_str() {
            "bash" => println!(
                "complete -W \"ctl --mcp --headless --listen --playbook --help --version --gui-dump --completions --man --session\" bilterm"
            ),
            "zsh" => println!(
                "#compdef bilterm\n_arguments '--mcp[Run MCP server]' '--headless[Headless daemon]' '--listen[Listen address]:addr:' '--playbook[Run TOML playbook]:file:_files' '--help[Print help]' '--version[Print version]' '--gui-dump[Dump GPU PPM]:file:_files' '--completions[Shell completions]:shell:(bash zsh fish)' '--man[Print manpage]' '--session[History session name]:name:'"
            ),
            "fish" => println!(
                "complete -c bilterm -l mcp -d 'Run MCP server'\ncomplete -c bilterm -l headless -d 'Headless daemon'\ncomplete -c bilterm -l listen -r -d 'Listen address'\ncomplete -c bilterm -l playbook -r -d 'Run TOML playbook'\ncomplete -c bilterm -l help -d 'Print help'\ncomplete -c bilterm -l version -d 'Print version'\ncomplete -c bilterm -l gui-dump -r -d 'Dump GPU PPM'\ncomplete -c bilterm -l completions -x -a 'bash zsh fish' -d 'Shell completions'\ncomplete -c bilterm -l man -d 'Print manpage'\ncomplete -c bilterm -l session -r -d 'History session name'"
            ),
            _ => eprintln!("Supported shells: bash, zsh, fish"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--man") {
        print!("{}", include_str!("../docs/bilterm.1"));
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
    let mut cfg = BiltermConfig::default_path()
        .map(|p| BiltermConfig::load_or_default(&p))
        .unwrap_or_default();
    if let Some(pos) = args.iter().position(|a| a == "--session")
        && let Some(name) = args.get(pos + 1)
    {
        cfg.history_session = name.clone();
    }
    app.wm.apply_config(cfg);

    let host_guard = TerminalRestoreGuard::enter_host_tui()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let run_res = app.run(&mut terminal).await;
    host_guard.restore();
    run_res?;
    Ok(())
}
