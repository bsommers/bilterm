//! Phase 51.1-09: Palette, completion, help and top menu bar entries for persistent SSH-tmux links (UNI-R01).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::command_registry;
use bilterm::input::AppAction;
use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS};
use bilterm::pane::completion::colon_command_table;
use bilterm::ui::modals::{
    HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect,
    menubar_item_command,
};
use bilterm::wm::link_cmd::RemoteLinkOp;
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

// ---------------------------------------------------------------------------
// Task 1: Completion, Palette, Keymap Hints, Help Lines, Command Registry
// ---------------------------------------------------------------------------

#[test]
fn completion_row_lists_the_new_verbs_and_each_parses() {
    let table = colon_command_table();
    let (_, subs) = table
        .iter()
        .find(|&&(cmd, _)| cmd == ":ssh-tmux")
        .expect(":ssh-tmux completion row exists");

    for required in &["list", "reconnect", "disconnect", "info", "prefill", "user@host"] {
        assert!(
            subs.contains(required),
            ":ssh-tmux completion candidates missing `{required}`, found: {subs:?}"
        );
    }

    for verb in &["reconnect", "disconnect", "info", "prefill"] {
        let cmd = format!(":ssh-tmux {verb}");
        let parsed = parse_colon_command(&cmd)
            .expect("parse returns Some")
            .expect("parses cleanly");
        let expected_op = RemoteLinkOp::from_word(verb).expect("valid op word");
        assert_eq!(
            parsed,
            Action::RemoteLink(expected_op),
            "command `{cmd}` did not parse to expected RemoteLinkOp"
        );
    }

    // :ssh-tmux list parses to RemoteList
    let list_parsed = parse_colon_command(":ssh-tmux list")
        .expect("parse returns Some")
        .expect("parses cleanly");
    assert_eq!(list_parsed, Action::RemoteList);
}

#[test]
fn palette_entries_parse_to_remote_link_actions() {
    // Existing entry stays
    let old_entry = PALETTE_CATALOG.iter().find(|e| e.0 == ":ssh-tmux ");
    assert!(old_entry.is_some(), "existing `:ssh-tmux ` entry must remain in palette");

    let cases = [
        (
            ":ssh-tmux reconnect",
            RemoteLinkOp::Reconnect,
            "Reconnect the persistent remote tmux link now (skips the retry timer)",
        ),
        (
            ":ssh-tmux disconnect",
            RemoteLinkOp::Disconnect,
            "Stop the remote tmux link; the remote tmux session keeps running",
        ),
        (
            ":ssh-tmux info",
            RemoteLinkOp::Info,
            "Show tmux link state, retry timer, last exit reason and mapped panes",
        ),
        (
            ":ssh-tmux prefill",
            RemoteLinkOp::Prefill,
            "Open the prompt ready to type :ssh-tmux <user@host> [session]",
        ),
    ];

    for (cmd, expected_op, expected_desc) in cases {
        let entry = PALETTE_CATALOG
            .iter()
            .find(|e| e.0 == cmd)
            .unwrap_or_else(|| panic!("missing palette entry for `{cmd}`"));
        assert_eq!(entry.2, expected_desc, "description mismatch for `{cmd}`");

        let parsed = parse_colon_command(entry.0)
            .expect("parse returns Some")
            .expect("parses cleanly");
        assert_eq!(
            parsed,
            Action::RemoteLink(expected_op),
            "palette entry `{cmd}` must parse to Action::RemoteLink"
        );
    }
}

#[test]
fn shortcut_hints_match_keymap() {
    // Verify leader bindings exist
    assert!(
        DEFAULT_LEADER_BINDINGS.iter().any(|(code, action)| {
            *code == KeyCode::Char('u') && *action == Action::RemoteLink(RemoteLinkOp::Prefill)
        }),
        "leader 'u' -> RemoteLink(Prefill) missing from DEFAULT_LEADER_BINDINGS"
    );
    assert!(
        DEFAULT_LEADER_BINDINGS.iter().any(|(code, action)| {
            *code == KeyCode::Char('U') && *action == Action::RemoteLink(RemoteLinkOp::Reconnect)
        }),
        "leader 'U' -> RemoteLink(Reconnect) missing from DEFAULT_LEADER_BINDINGS"
    );
    assert!(
        DEFAULT_LEADER_BINDINGS.iter().any(|(code, action)| {
            *code == KeyCode::Char('Q') && *action == Action::RemoteLink(RemoteLinkOp::Disconnect)
        }),
        "leader 'Q' -> RemoteLink(Disconnect) missing from DEFAULT_LEADER_BINDINGS"
    );

    // Verify global binding Ctrl+Alt+R exists
    let ctrl_alt = KeyModifiers::CONTROL.union(KeyModifiers::ALT);
    assert!(
        DEFAULT_GLOBAL_BINDINGS.iter().any(|(mods, code, action)| {
            *mods == ctrl_alt
                && (*code == KeyCode::Char('r') || *code == KeyCode::Char('R'))
                && *action == Action::RemoteLink(RemoteLinkOp::Reconnect)
        }),
        "Ctrl+Alt+R -> RemoteLink(Reconnect) missing from DEFAULT_GLOBAL_BINDINGS"
    );

    // Verify palette shortcut hints match
    let reconnect = PALETTE_CATALOG
        .iter()
        .find(|e| e.0 == ":ssh-tmux reconnect")
        .expect(":ssh-tmux reconnect in palette");
    assert_eq!(reconnect.1, "Ctrl+Alt+R / Ctrl+X U");

    let disconnect = PALETTE_CATALOG
        .iter()
        .find(|e| e.0 == ":ssh-tmux disconnect")
        .expect(":ssh-tmux disconnect in palette");
    assert_eq!(disconnect.1, "Ctrl+X Q");

    let prefill = PALETTE_CATALOG
        .iter()
        .find(|e| e.0 == ":ssh-tmux prefill")
        .expect(":ssh-tmux prefill in palette");
    assert_eq!(prefill.1, "Ctrl+X u");

    let info = PALETTE_CATALOG
        .iter()
        .find(|e| e.0 == ":ssh-tmux info")
        .expect(":ssh-tmux info in palette");
    assert_eq!(info.1, "—");
}

#[test]
fn help_rows_exist_for_each_verb() {
    let verbs = [
        ":ssh-tmux reconnect",
        ":ssh-tmux disconnect",
        ":ssh-tmux info",
        ":ssh-tmux prefill",
    ];

    for verb in verbs {
        let row = HELP_LINES.iter().find(|(cmd, _)| cmd.starts_with(verb));
        assert!(
            row.is_some(),
            "missing HELP_LINES row starting with `{verb}`"
        );
        let (_, desc) = row.unwrap();
        assert!(
            !desc.is_empty(),
            "description for `{verb}` in HELP_LINES must not be empty"
        );
        assert!(
            desc.chars().count() < 100,
            "description for `{verb}` in HELP_LINES must be under 100 characters (was {})",
            desc.chars().count()
        );
    }

    // Mention banner keys for reconnect (Enter/r on the banner) and disconnect (x stops link)
    let reconnect_desc = HELP_LINES
        .iter()
        .find(|(cmd, _)| cmd.starts_with(":ssh-tmux reconnect"))
        .unwrap()
        .1;
    assert!(
        reconnect_desc.contains("Enter/r on the banner"),
        "reconnect help must mention 'Enter/r on the banner': {reconnect_desc}"
    );

    let disconnect_desc = HELP_LINES
        .iter()
        .find(|(cmd, _)| cmd.starts_with(":ssh-tmux disconnect"))
        .unwrap()
        .1;
    assert!(
        disconnect_desc.contains("x on banner") || disconnect_desc.contains("x stops"),
        "disconnect help must mention banner key 'x': {disconnect_desc}"
    );
}

#[test]
fn catalog_lists_the_examples() {
    let cat = command_registry::catalog();
    let info = cat
        .iter()
        .find(|c| c.name == ":ssh-tmux")
        .expect(":ssh-tmux exists in command_registry::catalog");

    for ex in &[
        ":ssh-tmux reconnect",
        ":ssh-tmux disconnect",
        ":ssh-tmux info",
        ":ssh-tmux prefill",
    ] {
        assert!(
            info.examples.contains(ex),
            ":ssh-tmux examples missing `{ex}`, found: {:?}",
            info.examples
        );
    }

    let mut seen = std::collections::HashSet::new();
    assert!(
        cat.iter().all(|c| seen.insert(c.name)),
        "duplicate command name in command_registry::catalog"
    );
}

// ---------------------------------------------------------------------------
// Task 2: Tmux Menu Bar Items (Mouse Path) and Catalog Sweep
// ---------------------------------------------------------------------------

#[test]
fn menu_items_run_the_verbs() {
    let tmux_cat = 5; // Tmux is category 5
    let count = menubar_category_item_count(tmux_cat);
    assert_eq!(count, 8, "Tmux category must have 8 items (4 existing + 4 new)");

    // Existing 4 items preserved in order
    assert_eq!(menubar_item_command(tmux_cat, 0), Some(":broadcast"));
    assert_eq!(menubar_item_command(tmux_cat, 1), Some(":lock"));
    assert_eq!(menubar_item_command(tmux_cat, 2), Some(":quake"));
    assert_eq!(menubar_item_command(tmux_cat, 3), Some(":raw"));

    // New 4 items at the end
    let expected = [
        (4, ":ssh-tmux prefill", "SSH tmux link", RemoteLinkOp::Prefill),
        (5, ":ssh-tmux reconnect", "Reconnect link", RemoteLinkOp::Reconnect),
        (6, ":ssh-tmux disconnect", "Disconnect link", RemoteLinkOp::Disconnect),
        (7, ":ssh-tmux info", "Link info", RemoteLinkOp::Info),
    ];

    for (item_idx, expected_cmd, _label_substr, expected_op) in expected {
        let cmd = menubar_item_command(tmux_cat, item_idx)
            .unwrap_or_else(|| panic!("item {item_idx} missing in Tmux category"));
        assert_eq!(cmd, expected_cmd);

        let parsed = parse_colon_command(cmd)
            .expect("parse returns Some")
            .expect("parses cleanly");
        assert_eq!(parsed, Action::RemoteLink(expected_op));
    }
}

#[test]
fn menu_activation_reaches_apply() {
    let tmux_cat = 5;

    // Test Link info: headless WM without link leaves status message
    {
        let mut wm = WindowManager::new_headless(24, 80);
        let cmd_str = menubar_item_command(tmux_cat, 7).expect("Link info command");
        let cmd = parse_colon_command(cmd_str).unwrap().unwrap();
        let result = wm.apply(cmd).expect("apply succeeds");
        assert_eq!(result, AppAction::Handled);
        let status = wm.active_pane().status_message().unwrap_or("");
        assert!(
            status.contains("No tmux link"),
            "status should indicate no tmux link, got: `{status}`"
        );
    }

    // Test SSH tmux link... (prefill): sets prompt text to ":ssh-tmux "
    {
        let mut wm = WindowManager::new_headless(24, 80);
        let cmd_str = menubar_item_command(tmux_cat, 4).expect("prefill command");
        let cmd = parse_colon_command(cmd_str).unwrap().unwrap();
        let result = wm.apply(cmd).expect("apply succeeds");
        assert_eq!(result, AppAction::Handled);
        assert_eq!(
            wm.active_pane().prompt().text(),
            ":ssh-tmux ",
            "prompt text must be set to ':ssh-tmux '"
        );
    }
}

#[test]
fn category_titles_and_indices_unchanged() {
    // Tmux category index is 5; Help category index is 6
    assert_eq!(menubar_category_item_count(5), 8, "Tmux category has 8 items");

    // Help category is still at index 6
    assert_eq!(
        menubar_item_command(6, 0),
        Some(":help"),
        "Help category must remain at index 6"
    );

    // Dropdown rect for Tmux category (index 5)
    let area_24 = Rect::new(0, 0, 80, 24);
    let rect_24 = menubar_dropdown_rect(5, area_24);
    assert!(rect_24.height <= 24, "dropdown height must be clamped to workspace height");
    assert_eq!(rect_24.height, 10, "8 items + 2 borders = 10 rows");
    assert!(rect_24.width <= 30, "dropdown width must be <= 30");

    let area_5 = Rect::new(0, 0, 80, 5);
    let rect_5 = menubar_dropdown_rect(5, area_5);
    assert_eq!(rect_5.height, 5, "dropdown height clamped in 5-row workspace");
}

#[test]
fn headless_examples_run_without_error() {
    let examples = [
        ":ssh-tmux reconnect",
        ":ssh-tmux disconnect",
        ":ssh-tmux info",
        ":ssh-tmux prefill",
    ];

    for ex in examples {
        let mut wm = WindowManager::new_headless(24, 80);
        let parsed = parse_colon_command(ex)
            .unwrap_or_else(|| panic!("failed to parse `{ex}`"))
            .unwrap_or_else(|e| panic!("parse error for `{ex}`: {e}"));
        let result = wm.apply(parsed).expect("apply does not error");
        assert_eq!(result, AppAction::Handled);

        if ex == ":ssh-tmux prefill" {
            assert_eq!(wm.active_pane().prompt().text(), ":ssh-tmux ");
        } else {
            assert!(
                wm.active_pane().status_message().is_some(),
                "command `{ex}` should set status message"
            );
        }
    }
}
