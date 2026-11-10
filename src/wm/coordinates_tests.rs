use super::*;

fn c(d: Option<&str>, w: Option<&str>, s: Option<&str>, y: Option<&str>, p: Option<&str>) -> TargetCoord {
    let o = |x: Option<&str>| x.map(str::to_string);
    TargetCoord { daemon: o(d), workspace: o(w), session: o(s), window: o(y), pane: o(p) }
}

fn ok(s: &str) -> TargetCoord {
    TargetCoord::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
}

#[test]
fn parse_daemon_tuple() {
    let want = c(Some("d0"), Some("w1"), Some("s1"), Some("y2"), Some("p1"));
    assert_eq!(ok("(d0:(w1,s1,y2,p1))"), want);
    assert_eq!(ok("  ( d0 : ( w1 , s1 , y2 ) ) ").pane, None);
}

#[test]
fn parse_positional_tuple() {
    assert_eq!(ok("(w1,main,y2)"), c(None, Some("w1"), Some("main"), Some("y2"), None));
    assert_eq!(ok("(w1,main,#2,%3)").pane.as_deref(), Some("%3"));
    assert_eq!(ok("(main,2)"), c(None, None, Some("main"), Some("y2"), None));
}

#[test]
fn parse_labelled_any_order() {
    let want = c(None, Some("w1"), Some("main"), Some("y2"), Some("%3"));
    assert_eq!(ok("(Wksp: w1, Sess: main, Win: y2, Pane: %3)"), want);
    assert_eq!(ok("(pane: %3, WIN: #2, sess: main, wksp: w1)"), want);
    assert_eq!(ok("(Win: 2)").window.as_deref(), Some("y2"));
    assert_eq!(ok("(Daemon: d0, Win: y2)").daemon.as_deref(), Some("d0"));
}

#[test]
fn parse_paths() {
    let want = c(Some("d0"), Some("w1"), Some("s1"), Some("y2"), Some("p1"));
    assert_eq!(ok("d0:w1:s1:y2:p1"), want);
    assert_eq!(ok("d0/w1/s1/y2/p1"), want);
    assert_eq!(ok("w1:s1:y2"), c(None, Some("w1"), Some("s1"), Some("y2"), None));
    assert_eq!(ok("w12:p6"), c(None, Some("w12"), None, None, Some("p6")));
    assert_eq!(ok("s1:y2"), c(None, None, Some("s1"), Some("y2"), None));
    assert_eq!(ok("w1:main:y2:%3"), c(None, Some("w1"), Some("main"), Some("y2"), Some("%3")));
    assert_eq!(ok("d0:w1:main:y2"), c(Some("d0"), Some("w1"), Some("main"), Some("y2"), None));
}

#[test]
fn parse_singletons() {
    assert_eq!(ok("#2"), c(None, None, None, Some("y2"), None));
    assert_eq!(ok("Y2"), ok("y2"));
    assert_eq!(ok("2"), ok("y2"));
    assert_eq!(ok("%3"), c(None, None, None, None, Some("%3")));
    assert_eq!(ok("P6").pane.as_deref(), Some("p6"));
}

#[test]
fn parse_errors() {
    for bad in [
        "", "  ", "(", "(w1,s1,y2", "w1,s1,y2)", "((w1,s1,y2))", "a:b:c:d:e:f", "y2:w1", "w1:w2",
        "(w1)", "(a,b,c,d,e)", "(Foo: 1)", "(Win: y2, Win: y3)", "(Win: y2, w1)", "w1::y2",
        "s1:abc", "(w1,s1,y2,zz)", "(d0:w1,s1,y2)",
    ] {
        assert!(TargetCoord::parse(bad).is_err(), "should fail: {bad:?}");
    }
}

#[test]
fn style_names() {
    assert_eq!(CoordDisplayStyle::from_name("Daemon-Tuple"), Some(CoordDisplayStyle::DaemonTuple));
    assert_eq!(CoordDisplayStyle::from_name("nope"), None);
    for s in [CoordDisplayStyle::Verbose, CoordDisplayStyle::Compact, CoordDisplayStyle::DaemonTuple, CoordDisplayStyle::Off] {
        assert_eq!(CoordDisplayStyle::from_name(s.name()), Some(s));
    }
}

#[test]
fn formats() {
    let a = c(Some("d0"), Some("w1"), Some("main"), Some("y2"), None);
    let b = TargetCoord { pane: Some("%3".into()), ..a.clone() };
    assert_eq!(a.format(CoordDisplayStyle::Verbose).unwrap(), "(Wksp: w1, Sess: main, Win: y2)");
    assert_eq!(b.format(CoordDisplayStyle::Verbose).unwrap(), "(Wksp: w1, Sess: main, Win: y2, Pane: %3)");
    assert_eq!(a.format(CoordDisplayStyle::Compact).unwrap(), "(w1,main,y2)");
    assert_eq!(b.format(CoordDisplayStyle::Compact).unwrap(), "(w1,main,y2,%3)");
    assert_eq!(a.format(CoordDisplayStyle::DaemonTuple).unwrap(), "(d0:(w1,main,y2))");
    assert_eq!(b.format(CoordDisplayStyle::DaemonTuple).unwrap(), "(d0:(w1,main,y2,%3))");
    assert_eq!(a.format(CoordDisplayStyle::Off), None);
    assert_eq!(a.to_path(), "d0:w1:main:y2");
    assert_eq!(b.to_path(), "d0:w1:main:y2:%3");
    let w = c(None, None, None, Some("y2"), None);
    assert_eq!(w.format(CoordDisplayStyle::Compact).unwrap(), "(*,*,y2)");
    assert_eq!(w.to_path(), "y2");
}

#[test]
fn round_trip() {
    let a = c(Some("d0"), Some("w1"), Some("main"), Some("y2"), None);
    let b = TargetCoord { pane: Some("%3".into()), ..a.clone() };
    for x in [&a, &b] {
        for st in [CoordDisplayStyle::Compact, CoordDisplayStyle::Verbose] {
            let back = ok(&x.format(st).unwrap());
            assert_eq!(back, TargetCoord { daemon: None, ..x.clone() });
        }
        assert_eq!(ok(&x.format(CoordDisplayStyle::DaemonTuple).unwrap()), *x);
        assert_eq!(ok(&x.to_path()), *x);
    }
}

fn wm3() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(crate::wm::SplitDirection::Horizontal).unwrap();
    wm.split_focused(crate::wm::SplitDirection::Vertical).unwrap();
    wm
}

#[test]
fn coord_of_live() {
    let wm = wm3();
    let c1 = wm.coord_of(1).unwrap();
    assert_eq!(c1.daemon.as_deref(), Some("d0"));
    assert_eq!(c1.workspace.as_deref(), Some("w1"));
    assert_eq!(c1.session.as_deref(), Some(wm.config.history_session.as_str()));
    assert_eq!(c1.window.as_deref(), Some("y1"));
    assert_eq!(c1.pane, None);
    assert!(wm.coord_of(999).is_none());
}

#[test]
fn resolve_wildcards_and_mismatches() {
    let wm = wm3();
    let ids: Vec<PaneId> = wm.panes().keys().copied().collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(wm.resolve_coord(&TargetCoord::default()).panes, ids);
    assert_eq!(wm.resolve_coord(&ok("w1")).panes, ids);
    let r = wm.resolve_coord(&ok("y2"));
    assert_eq!(r.panes, vec![2]);
    assert_eq!(r.coords[0], wm.coord_of(2).unwrap());
    assert_eq!(wm.resolve_coord(&ok("#2")).panes, vec![2]);
    assert_eq!(wm.resolve_coord(&ok(&wm.coord_of(3).unwrap().to_path())).panes, vec![3]);
    assert!(wm.resolve_coord(&ok("y99")).panes.is_empty());
    assert!(wm.resolve_coord(&ok("d1:y2")).panes.is_empty());
    assert!(wm.resolve_coord(&ok("w2:s1:y2")).panes.is_empty());
    assert!(wm.resolve_coord(&ok("nosuchsession:y2")).panes.is_empty());
    assert!(wm.resolve_coord(&ok("%3")).panes.is_empty());
}

#[test]
fn resolve_herdr_pane_fallback() {
    let wm = wm3();
    // "w12:p6": workspace 12 does not exist.
    assert!(wm.resolve_coord(&ok("w12:p6")).panes.is_empty());
    assert_eq!(wm.resolve_coord(&ok("w1:p2")).panes, vec![2]);
    assert_eq!(wm.resolve_coord(&ok("p3")).panes, vec![3]);
}

#[test]
fn style_setter() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Off);
    wm.set_coord_style(CoordDisplayStyle::Compact);
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Compact);
}

#[test]
fn tmux_pane_lookup_feeds_coord_and_resolve() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.coord_of(1).unwrap().pane, None);
    let id = wm.insert_tmux_pane(7);
    wm.tmux_bridge.tmux_to_bilterm.insert(7, id);
    wm.tmux_bridge.bilterm_to_tmux.insert(id, 7);
    assert_eq!(wm.tmux_pane_of(id).as_deref(), Some("%7"));
    assert_eq!(wm.tmux_pane_of(1), None);
    assert_eq!(wm.coord_of(id).unwrap().pane.as_deref(), Some("%7"));
    assert_eq!(wm.coord_of(1).unwrap().pane, None);
    let t = TargetCoord { pane: Some("%7".into()), ..Default::default() };
    assert_eq!(wm.resolve_coord(&t).panes, vec![id]);
}

#[test]
fn config_badge_applies_on_apply_config_and_config_set() {
    let mut wm = WindowManager::new_headless(24, 80);
    let mut cfg = wm.config().clone();
    cfg.coords.badge = CoordDisplayStyle::Verbose;
    wm.apply_config(cfg);
    assert_eq!(wm.coord_style(), CoordDisplayStyle::Verbose);
}
