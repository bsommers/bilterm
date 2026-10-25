pub mod execute;
pub mod execute_wm;
pub mod parse;

pub use crate::action::Action;
pub use execute::execute_colon_command;
pub use execute_wm::execute_wm_colon_command;
pub use parse::parse_colon_command;

/// Type alias preserving `ColonCommand` compatibility while unifying on [`crate::action::Action`] (`ARCH-R01`).
pub type ColonCommand = crate::action::Action;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{CompositePane, PromptAnchor};
    use crate::ui::cursor::SpotlightMode;
    use crate::wm::WindowManager;

    #[test]
    fn parses_and_executes_colon_commands_without_dispatching_to_pty() {
        let mut pane = CompositePane::new_headless(1, 25, 80);

        let cmd = parse_colon_command(":prompt bottom").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt().anchor, PromptAnchor::Bottom);

        let cmd = parse_colon_command(":prompt float 7").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt().anchor, PromptAnchor::Floating { y_offset: 7 });

        let cmd = parse_colon_command(":prompt height 6").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert_eq!(pane.prompt().height, 6);

        pane.vt_mut().record_command_submission("cargo test");
        let cmd = parse_colon_command(":yank").unwrap().unwrap();
        assert!(!execute_colon_command(&mut pane, cmd));
        assert!(pane.last_osc52_sequence().is_some());
    }

    #[test]
    fn history_runs_parses_and_reports_segments() {
        assert_eq!(
            parse_colon_command(":history runs").unwrap().unwrap(),
            ColonCommand::HistoryRuns
        );
        assert_eq!(
            parse_colon_command(":hist segments").unwrap().unwrap(),
            ColonCommand::HistoryRuns
        );

        let tmp = tempfile::tempdir().unwrap();
        let mut pane = CompositePane::new_headless(1, 25, 80);
        pane.history_store_mut().base_dir = tmp.path().to_path_buf();
        pane.history_store_mut().enabled = true;
        pane.history_store_mut().session_file_path =
            tmp.path().join("sessions/default/history.log");
        assert!(!execute_colon_command(&mut pane, ColonCommand::HistoryRuns));
        assert!(pane.status_message().unwrap().contains("no recorded runs"));

        pane.history_store_mut().run_id = "run-1".to_string();
        pane.history_store_mut().record_command("ls").unwrap();
        pane.history_store_mut().run_id = "run-2".to_string();
        pane.history_store_mut().record_command("pwd").unwrap();
        pane.history_store_mut().record_command("id").unwrap();
        execute_colon_command(&mut pane, ColonCommand::HistoryRuns);
        let msg = pane.status_message().unwrap().to_string();
        assert!(msg.contains("2 runs"), "{msg}");
        assert!(msg.find("run-2").unwrap() < msg.find("run-1").unwrap(), "{msg}");
        assert!(msg.contains("(2 cmds)"), "{msg}");
    }

    #[test]
    fn spotlight_colon_commands_configure_shape_pixelation_and_size() {
        let mut wm = WindowManager::new_headless(25, 80);
        assert_eq!(wm.config().spotlight_mode, SpotlightMode::Off);

        // Cycle via bare :spotlight
        let cmd = parse_colon_command(":spotlight").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().spotlight_mode, SpotlightMode::Circle);
        assert!(!wm.config().spotlight_pixelated);

        // Configure elliptical pixelated spotlight with custom size and feather
        let cmd = parse_colon_command(":spotlight ellipse pixel size 30 8 feather 60")
            .unwrap()
            .unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().spotlight_mode, SpotlightMode::Ellipse);
        assert!(wm.config().spotlight_pixelated);
        assert_eq!(wm.config().spotlight_rx, 30);
        assert_eq!(wm.config().spotlight_ry, 8);
        assert_eq!(wm.config().spotlight_feather_pct, 60);

        // Grow and shrink via :spotlight + and :spotlight -
        let cmd = parse_colon_command(":spotlight +").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().spotlight_rx, 34);
        assert_eq!(wm.config().spotlight_ry, 9);

        let cmd = parse_colon_command(":spotlight off").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().spotlight_mode, SpotlightMode::Off);

        // Cycle Focal Zoom levels: Off -> 1.25x -> 1.50x -> 2.00x -> 3.00x -> Off
        for expected_pct in [125u16, 150, 200, 300] {
            let cmd = parse_colon_command(":focalzoom").unwrap().unwrap();
            execute_wm_colon_command(&mut wm, cmd);
            assert_eq!(wm.config().focal_zoom_mode, SpotlightMode::Ellipse);
            assert_eq!(wm.config().focal_zoom_pct, expected_pct);
        }
        let cmd = parse_colon_command(":focalzoom").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().focal_zoom_mode, SpotlightMode::Off);

        // Configure 2.5x circular pixelated focal zoom with custom size and step
        let cmd = parse_colon_command(":focalzoom 2.5x circle pixel size 24 8 feather 55")
            .unwrap()
            .unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert_eq!(wm.config().focal_zoom_mode, SpotlightMode::Circle);
        assert_eq!(wm.config().focal_zoom_pct, 250);
        assert!(wm.config().focal_zoom_pixelated);
        assert_eq!(wm.config().focal_zoom_rx, 24);
        assert_eq!(wm.config().focal_zoom_ry, 8);
        assert_eq!(wm.config().focal_zoom_feather_pct, 55);
    }

    #[test]
    fn record_start_default_path_is_private() {
        let mut pane = CompositePane::new_headless(1, 24, 80);
        execute_colon_command(&mut pane, ColonCommand::RecordStart(None));
        let status = pane.status_message().unwrap();
        let private_root = crate::secure_fs::private_runtime_dir().unwrap();
        assert!(status.contains(&private_root.display().to_string()));
        assert!(status.contains("recordings"));
        assert!(status.contains(".cast"));
    }

    #[test]
    fn remote_add_rejects_option_destination() {
        let mut wm = WindowManager::new_headless(24, 80);
        let cmd = parse_colon_command(":remote add evil -oProxyCommand=x").unwrap().unwrap();
        execute_wm_colon_command(&mut wm, cmd);
        assert!(!wm.fleet().nodes.contains_key("evil"));
        let status = wm.active_pane().status_message().unwrap_or("");
        assert!(
            status.contains("Remote not added:") && status.contains("invalid SSH destination"),
            "status: {status}"
        );
    }
}
