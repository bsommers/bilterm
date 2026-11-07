//! Phase 53 Wave 1 seam test: grammar parsing, serialization, name validation,
//! tab completion, and Action dispatch (`SES-01..05`, `UNI-R01`).

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::pane::completion::compute_completions;
use bilterm::wm::window_manage::{
    disambiguate_name, validate_session_path, validate_window_name, BulkWindowOp,
    SessionManageOp, WindowBulkOp, WindowManageOp, WindowSelectTarget,
};
use bilterm::wm::WindowManager;

#[test]
fn test_window_command_parsing() {
    // 1. Move variants
    let cmd = parse_colon_command(":window move dev").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Move {
            window_id: None,
            target_session: "dev".to_string(),
            switch: false,
        })
    );

    let cmd = parse_colon_command(":window move 2 dev").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Move {
            window_id: Some(2),
            target_session: "dev".to_string(),
            switch: false,
        })
    );

    let cmd = parse_colon_command(":window move --switch dev")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Move {
            window_id: None,
            target_session: "dev".to_string(),
            switch: true,
        })
    );

    let cmd = parse_colon_command(":window move 2 --switch dev")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Move {
            window_id: Some(2),
            target_session: "dev".to_string(),
            switch: true,
        })
    );

    // 2. Rename variants
    let cmd = parse_colon_command(":window rename editor").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Rename {
            window_id: None,
            new_name: "editor".to_string(),
        })
    );

    let cmd = parse_colon_command(":window rename 3 editor")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Rename {
            window_id: Some(3),
            new_name: "editor".to_string(),
        })
    );

    // 3. Selection variants
    let cmd = parse_colon_command(":window select toggle 3")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::Toggle(3)))
    );

    let cmd = parse_colon_command(":window select all").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::All))
    );

    let cmd = parse_colon_command(":window select none").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::None))
    );

    let cmd = parse_colon_command(":window select clear").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::None))
    );

    let cmd = parse_colon_command(":window select invert")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::Invert))
    );

    let cmd = parse_colon_command(":window select range 1 4")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::Range {
            from: 1,
            to: 4,
        }))
    );

    let cmd = parse_colon_command(":window select explicit 1 3 5")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Select(WindowSelectTarget::Explicit(vec![
            1, 3, 5
        ])))
    );

    // 4. Bulk variants
    let cmd = parse_colon_command(":window bulk move staging")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Move {
            target_session: "staging".to_string(),
        }))
    );

    let cmd = parse_colon_command(":window bulk close").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Close))
    );

    let cmd = parse_colon_command(":window bulk close --force")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::ForceClose))
    );

    let cmd = parse_colon_command(":window bulk rename-prefix test-")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::RenamePrefix {
            prefix: "test-".to_string(),
        }))
    );

    let cmd = parse_colon_command(":window bulk tile").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Tile))
    );

    let cmd = parse_colon_command(":window bulk pin on").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Pin(Some(true))))
    );

    let cmd = parse_colon_command(":window bulk pin off").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Pin(Some(false))))
    );

    let cmd = parse_colon_command(":window bulk pin").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::WindowManage(WindowManageOp::Bulk(BulkWindowOp::Pin(None)))
    );

    // 5. Undo
    let cmd = parse_colon_command(":window undo").unwrap().unwrap();
    assert_eq!(cmd, Action::WindowManage(WindowManageOp::Undo));
}

#[test]
fn test_session_command_parsing() {
    let cmd = parse_colon_command(":session rename dev").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::SessionManage(SessionManageOp::Rename {
            old_name: None,
            new_name: "dev".to_string(),
        })
    );

    let cmd = parse_colon_command(":session rename prod staging")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::SessionManage(SessionManageOp::Rename {
            old_name: Some("prod".to_string()),
            new_name: "staging".to_string(),
        })
    );

    let cmd = parse_colon_command(":session move top").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::SessionManage(SessionManageOp::MoveOrder {
            name: None,
            target_pos: "top".to_string(),
        })
    );

    let cmd = parse_colon_command(":session move dev bottom")
        .unwrap()
        .unwrap();
    assert_eq!(
        cmd,
        Action::SessionManage(SessionManageOp::MoveOrder {
            name: Some("dev".to_string()),
            target_pos: "bottom".to_string(),
        })
    );

    let cmd = parse_colon_command(":session undo").unwrap().unwrap();
    assert_eq!(cmd, Action::SessionManage(SessionManageOp::Undo));

    let cmd = parse_colon_command(":rename alpha beta").unwrap().unwrap();
    assert_eq!(
        cmd,
        Action::SessionManage(SessionManageOp::Rename {
            old_name: Some("alpha".to_string()),
            new_name: "beta".to_string(),
        })
    );
}

#[test]
fn test_serialization_round_trip() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct WindowWrapper {
        op: WindowManageOp,
    }

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct SessionWrapper {
        op: SessionManageOp,
    }

    let ops = vec![
        WindowManageOp::Move {
            window_id: Some(1),
            target_session: "infra/nodes".to_string(),
            switch: true,
        },
        WindowManageOp::Rename {
            window_id: None,
            new_name: "worker-1".to_string(),
        },
        WindowManageOp::Select(WindowSelectTarget::Range { from: 2, to: 5 }),
        WindowManageOp::Bulk(WindowBulkOp::Move {
            target_session: "backup".to_string(),
        }),
        WindowManageOp::Bulk(WindowBulkOp::Close),
        WindowManageOp::Bulk(WindowBulkOp::ForceClose),
        WindowManageOp::Bulk(WindowBulkOp::RenamePrefix {
            prefix: "grp-".to_string(),
        }),
        WindowManageOp::Bulk(WindowBulkOp::Tile),
        WindowManageOp::Bulk(WindowBulkOp::Pin(Some(true))),
        WindowManageOp::Undo,
    ];

    for op in &ops {
        let wrapper = WindowWrapper { op: op.clone() };
        let serialized = toml::to_string(&wrapper).expect("serialize toml");
        let deserialized: WindowWrapper =
            toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(&wrapper, &deserialized);
    }

    let session_ops = vec![
        SessionManageOp::Rename {
            old_name: Some("work".to_string()),
            new_name: "personal".to_string(),
        },
        SessionManageOp::MoveOrder {
            name: Some("test".to_string()),
            target_pos: "1".to_string(),
        },
        SessionManageOp::Undo,
    ];

    for op in &session_ops {
        let wrapper = SessionWrapper { op: op.clone() };
        let serialized = toml::to_string(&wrapper).expect("serialize toml");
        let deserialized: SessionWrapper =
            toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(&wrapper, &deserialized);
    }
}

#[test]
fn test_name_and_path_validation() {
    // Valid window names
    assert!(validate_window_name("editor").is_ok());
    assert!(validate_window_name("test_pane-2").is_ok());

    // Invalid window names
    assert!(validate_window_name("").is_err());
    assert!(validate_window_name("   ").is_err());
    assert!(validate_window_name("path/traversal").is_err());
    assert!(validate_window_name("../escaped").is_err());
    assert!(validate_window_name(&"x".repeat(65)).is_err());

    // Valid session paths
    assert!(validate_session_path("dev").is_ok());
    assert!(validate_session_path("infra/cluster/node1").is_ok());

    // Invalid session paths
    assert!(validate_session_path("").is_err());
    assert!(validate_session_path("infra//node").is_err());
    assert!(validate_session_path("infra/../node").is_err());
    assert!(validate_session_path(&format!("{}/toolong", "a".repeat(65))).is_err());

    // Disambiguation
    let mut set = std::collections::HashSet::new();
    set.insert("worker".to_string());
    assert_eq!(disambiguate_name("worker", &set), "worker-2");
    set.insert("worker-2".to_string());
    assert_eq!(disambiguate_name("worker", &set), "worker-3");
    assert_eq!(disambiguate_name("unique", &set), "unique");
}

#[test]
fn test_tab_completion_contains_stems() {
    let dummy_path = std::path::Path::new(".");

    let (_, _, cands) = compute_completions(":window ", 8, dummy_path);
    let completions: Vec<&str> = cands.iter().map(|c| c.display.as_str()).collect();

    assert!(completions.contains(&"move"));
    assert!(completions.contains(&"rename"));
    assert!(completions.contains(&"select"));
    assert!(completions.contains(&"bulk"));
    assert!(completions.contains(&"undo"));

    let (_, _, sess_cands) = compute_completions(":session ", 9, dummy_path);
    let sess_completions: Vec<&str> = sess_cands.iter().map(|c| c.display.as_str()).collect();
    assert!(sess_completions.contains(&"undo"));
    assert!(sess_completions.contains(&"rename"));
}

#[test]
fn test_initial_stub_execution() {
    let mut wm = WindowManager::new_headless(24, 80);

    // Apply window select
    let res = wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::All,
    )));
    assert!(res.is_ok());
    assert_eq!(wm.selected_windows().len(), 1);

    // Apply clear selection
    let res = wm.apply(Action::WindowManage(WindowManageOp::Select(
        WindowSelectTarget::None,
    )));
    assert!(res.is_ok());
    assert_eq!(wm.selected_windows().len(), 0);

    // Apply window undo
    let res = wm.apply(Action::WindowManage(WindowManageOp::Undo));
    assert!(res.is_ok());

    // Apply session undo
    let res = wm.apply(Action::SessionManage(SessionManageOp::Undo));
    assert!(res.is_ok());
}
