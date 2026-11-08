//! Executor for `Action::Web` / `:web` (`WEB-FE-01..04`, `UNI-R01`, Phase 60).
//!
//! Every surface (keys, colon, palette, menu, `web.*` RPC, `bilterm_web_surface` MCP) lowers to
//! this one function, so they all report the same `WebSurfaceStatus` JSON via `command_data`.

use crate::action::WebOp;
use crate::wm::WindowManager;

/// Applies `op` to the workspace's shared web surface; never quits.
pub fn execute_web_action(wm: &mut WindowManager, op: &WebOp) -> bool {
    let surface = wm.web_surface().clone();
    let msg = match op {
        WebOp::Status => surface.status().summary(),
        WebOp::OpenBrowser => {
            let status = surface.status();
            if !status.enabled {
                status.summary()
            } else if wm.is_headless() || wm.is_api_origin() {
                format!("Web UI: {}", status.url)
            } else {
                match open_in_browser(&status.url) {
                    Ok(()) => format!("Web UI: opened {}", status.url),
                    Err(e) => format!("Web UI: {} (could not launch browser: {e})", status.url),
                }
            }
        }
        WebOp::SetPort(port) => match surface.set_port(*port) {
            Ok(()) => format!("Web UI: port set to {port} (used on next start)"),
            Err(e) => {
                wm.record_automation_error(e.clone());
                format!("Web UI: {e}")
            }
        },
        WebOp::Stop => {
            if surface.is_enabled() {
                surface.request_stop();
                "Web UI: stopping (no new connections accepted)".to_string()
            } else {
                "Web UI: not running".to_string()
            }
        }
    };
    wm.set_command_data(surface.status().to_json().to_json_string());
    wm.active_pane_mut().set_status_message(Some(msg));
    false
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    if std::env::var_os("BILTERM_NO_BROWSER").is_some() {
        return Err(std::io::Error::other("BILTERM_NO_BROWSER is set"));
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}
