use std::io;
use std::path::{Path, PathBuf};

use bilterm::api::{
    default_socket_path, load_token_file, run_headless_server, run_mcp_stdio_server_with_plane,
    send_rpc_request_with_token, validate_listen_config, AutomationControlPlane, JsonValue,
    ListenEndpoint, Playbook, DEFAULT_TCP_LISTEN,
};
use bilterm::app::App;
use bilterm::config::BiltermConfig;
use bilterm::error::TerminalRestoreGuard;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::wm::WindowManager;
use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn has_switch(args: &[String], switches: &[&str]) -> bool {
    args.iter().any(|a| switches.contains(&a.as_str()))
}

fn switch_value<'a>(args: &'a [String], switches: &[&str]) -> Option<&'a str> {
    for (i, arg) in args.iter().enumerate() {
        if switches.contains(&arg.as_str()) {
            return args.get(i + 1).map(String::as_str);
        }
        for sw in switches {
            if let Some(rest) = arg.strip_prefix(sw)
                && let Some(val) = rest.strip_prefix('=')
            {
                return Some(val);
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if has_switch(&args, &["-v", "-V", "--version"]) {
        println!("bilterm {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if has_switch(&args, &["-h", "--help"]) {
        println!(
            "Bilterm v{} — IBM 3270 x Borland Turbo Vision Composite Terminal Multiplexer\n\
             Usage:\n  \
               bilterm [OPTIONS]\n  \
               bilterm ctl [-t|--token-file <PATH>] [ENDPOINT] <METHOD> [JSON_PARAMS]\n\
             Options:\n  \
               -m, --mcp                        Run Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio\n  \
               -d, --headless, --daemon         Run headless JSON-RPC 2.0 daemon (default: private Unix socket; TCP loopback via -l 127.0.0.1:PORT)\n  \
               -l, --listen <ADDR>              Listen address (IP:PORT, localhost:PORT, or unix:PATH)\n  \
               -r, --allow-remote               Allow listening on non-loopback TCP addresses (requires -t/--token-file)\n  \
               -t, --token-file <PATH>          Path to 0600 file containing bearer token (or BILTERM_TOKEN_FILE)\n  \
               -S, --snapshot-dir <PATH>        Directory for confined workspace snapshots (default: XDG state dir)\n  \
               -p, --playbook <FILE.toml>       Execute a declarative TOML automation playbook and exit\n  \
               -g, --gui-dump <PATH.ppm>        Render initial workspace via GuiWorkspaceRenderer to a PPM framebuffer\n  \
               -s, --session <NAME>             Use named persistent command-history session (overrides history_session)\n  \
               -c, --config <FILE.toml>         Load configuration from custom TOML file path\n  \
               -C, --completions <bash|zsh|fish> Print shell completion script to stdout\n  \
               -M, --man                        Print roff manpage (bilterm.1) to stdout\n  \
               -v, -V, --version                Print version information\n  \
               -h, --help                       Print this help message",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }

    let token = if let Some(path_str) = switch_value(&args, &["-t", "--token-file"]) {
        Some(load_token_file(Path::new(path_str))?)
    } else if let Ok(env_path) = std::env::var("BILTERM_TOKEN_FILE") {
        Some(load_token_file(Path::new(&env_path))?)
    } else {
        None
    };

    if args.first().map(String::as_str) == Some("ctl") {
        let mut rest = args[1..].to_vec();
        let mut ctl_token_path = None;
        if let Some(pos) = rest
            .iter()
            .position(|a| a == "--token-file" || a == "-t")
            && pos + 1 < rest.len()
        {
            ctl_token_path = Some(rest[pos + 1].clone());
            rest.remove(pos + 1);
            rest.remove(pos);
        }
        let ctl_token = if let Some(p) = ctl_token_path {
            Some(load_token_file(Path::new(&p))?)
        } else if let Ok(env_path) = std::env::var("BILTERM_TOKEN_FILE") {
            Some(load_token_file(Path::new(&env_path))?)
        } else {
            None
        };

        let default_ep = default_socket_path()
            .map(|p| format!("unix:{}", p.display()))
            .unwrap_or_else(|_| DEFAULT_TCP_LISTEN.to_string());

        let endpoint = rest.first().map(String::as_str).unwrap_or(&default_ep);
        let method = rest.get(1).map_or("workspace.status", String::as_str);
        let params = rest
            .get(2)
            .and_then(|raw| JsonValue::parse(raw))
            .unwrap_or_else(|| JsonValue::obj([] as [(&str, JsonValue); 0]));
        let resp =
            send_rpc_request_with_token(endpoint, method, params, ctl_token.as_deref()).await?;
        println!("{resp}");
        return Ok(());
    }
    if has_switch(&args, &["-m", "--mcp"]) {
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        if let Some(snap_dir) = switch_value(&args, &["-S", "--snapshot-dir"]) {
            plane = plane.with_snapshot_dir(PathBuf::from(snap_dir));
        }
        run_mcp_stdio_server_with_plane(plane).await?;
        return Ok(());
    }
    if let Some(pb_path) = switch_value(&args, &["-p", "--playbook"]) {
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
    if has_switch(&args, &["-d", "--headless", "--daemon"]) {
        let listen = switch_value(&args, &["-l", "--listen"]);
        let allow_remote = has_switch(&args, &["-r", "--allow-remote"]);
        let default_unix = default_socket_path().ok();
        let cfg = validate_listen_config(listen, allow_remote, token, default_unix)?;
        let mut plane = AutomationControlPlane::new_headless(24, 80);
        if let Some(snap_dir) = switch_value(&args, &["-S", "--snapshot-dir"]) {
            plane = plane.with_snapshot_dir(PathBuf::from(snap_dir));
        }
        let endpoint_display = match &cfg.endpoint {
            ListenEndpoint::Unix(p) => format!("unix:{}", p.display()),
            ListenEndpoint::Tcp(addr) => addr.to_string(),
        };
        eprintln!("bilterm: JSON-RPC listening on {endpoint_display}");
        run_headless_server(cfg, plane).await?;
        return Ok(());
    }
    if let Some(shell) = switch_value(&args, &["-C", "--completions"]) {
        match shell {
            "bash" => println!(
                "complete -W \"ctl -m --mcp -d --headless --daemon -l --listen -r --allow-remote -t --token-file -S --snapshot-dir -p --playbook -h --help -v -V --version -g --gui-dump -C --completions -M --man -s --session -c --config\" bilterm"
            ),
            "zsh" => println!(
                "#compdef bilterm\n_arguments '(-m --mcp)'{{-m,--mcp}}'[Run MCP server]' '(-d --headless --daemon)'{{-d,--headless,--daemon}}'[Headless daemon]' '(-l --listen)'{{-l,--listen}}'[Listen address]:addr:' '(-r --allow-remote)'{{-r,--allow-remote}}'[Allow remote TCP]' '(-t --token-file)'{{-t,--token-file}}'[Bearer token file]:file:_files' '(-S --snapshot-dir)'{{-S,--snapshot-dir}}'[Snapshot directory]:dir:_files -/' '(-p --playbook)'{{-p,--playbook}}'[Run TOML playbook]:file:_files' '(-h --help)'{{-h,--help}}'[Print help]' '(-v -V --version)'{{-v,-V,--version}}'[Print version]' '(-g --gui-dump)'{{-g,--gui-dump}}'[Dump GPU PPM]:file:_files' '(-C --completions)'{{-C,--completions}}'[Shell completions]:shell:(bash zsh fish)' '(-M --man)'{{-M,--man}}'[Print manpage]' '(-s --session)'{{-s,--session}}'[History session name]:name:' '(-c --config)'{{-c,--config}}'[Config TOML file]:file:_files'"
            ),
            "fish" => println!(
                "complete -c bilterm -s m -l mcp -d 'Run MCP server'\ncomplete -c bilterm -s d -l headless -d 'Headless daemon'\ncomplete -c bilterm -s l -l listen -r -d 'Listen address'\ncomplete -c bilterm -s r -l allow-remote -d 'Allow remote TCP'\ncomplete -c bilterm -s t -l token-file -r -d 'Bearer token file'\ncomplete -c bilterm -s S -l snapshot-dir -r -d 'Snapshot directory'\ncomplete -c bilterm -s p -l playbook -r -d 'Run TOML playbook'\ncomplete -c bilterm -s h -l help -d 'Print help'\ncomplete -c bilterm -s v -s V -l version -d 'Print version'\ncomplete -c bilterm -s g -l gui-dump -r -d 'Dump GPU PPM'\ncomplete -c bilterm -s C -l completions -x -a 'bash zsh fish' -d 'Shell completions'\ncomplete -c bilterm -s M -l man -d 'Print manpage'\ncomplete -c bilterm -s s -l session -r -d 'History session name'\ncomplete -c bilterm -s c -l config -r -d 'Config TOML file'"
            ),
            _ => eprintln!("Supported shells: bash, zsh, fish"),
        }
        return Ok(());
    }
    if has_switch(&args, &["-M", "--man"]) {
        print!("{}", include_str!("../docs/bilterm.1"));
        return Ok(());
    }
    if let Some(out_path) = switch_value(&args, &["-g", "--gui-dump"]) {
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
    let mut cfg = BiltermConfig::default();
    let mut cfg_error = None;
    let custom_cfg_path = switch_value(&args, &["-c", "--config"]).map(PathBuf::from);
    if let Some(cfg_path) = custom_cfg_path.or_else(BiltermConfig::default_path) {
        match BiltermConfig::load(&cfg_path) {
            Ok(Some(loaded)) => cfg = loaded,
            Ok(None) => {}
            Err(msg) => {
                tracing::warn!("{msg}; using built-in defaults");
                cfg_error = Some(format!("config error: {msg} (using defaults)"));
            }
        }
    }
    if let Some(name) = switch_value(&args, &["-s", "--session"]) {
        cfg.history_session = name.to_string();
    }
    app.wm.apply_config(cfg);
    if cfg_error.is_some() {
        app.wm.active_pane_mut().set_status_message(cfg_error);
    }

    let host_guard = TerminalRestoreGuard::enter_host_tui()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let run_res = app.run(&mut terminal).await;
    host_guard.restore();
    run_res?;
    Ok(())
}
