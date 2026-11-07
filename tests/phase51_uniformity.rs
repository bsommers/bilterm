//! Phase 51-12: UNI-R01 parity matrix. Every Phase 51 capability is driven from identical headless
//! fixtures through every surface it has (colon, `wm.command`, key, mouse, palette entry by key and
//! by click, menu entry by key and by click, named RPC `agents.list` / `agents.demo`, MCP tools) and
//! must leave the same observable state. A surface that is missing must be one of the six recorded
//! exceptions of `51-CONTRACT.md` section 9, asserted here by capability, surface and reason.
//!
//! State is compared through `sig`, never through raw bytes or status text.

mod phase51_uniformity_support;

use std::collections::BTreeSet;

use bilterm::action::Action;
use bilterm::api::JsonValue;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::agents_list_keys::AGENTS_LIST_BINDINGS;
use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG, palette_header_rows};
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::agents_list_geometry::{ListBadge, ListChip};
use bilterm::wm::inset_window_frame;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use phase51_uniformity_support::*;

// ------------------------------------------------------------------------------------------------
// tests
// ------------------------------------------------------------------------------------------------

#[test]
fn uniformity_matrix() {
    let mut failures: Vec<String> = Vec::new();
    for cap in capabilities() {
        let before = sig(&cap.fixture());
        // The reference: the colon command, run through parse + apply.
        let mut reference = cap.fixture();
        let cmd = cap.cmd(&reference);
        colon(&mut reference, &cmd);
        let want = sig(&reference);
        if want == before {
            failures.push(format!("[{}] `{cmd}` changes nothing from the fixture: the row proves nothing", cap.name));
            continue;
        }
        let mut check = |surface: &str, run: &dyn Fn(&mut Fx, &str)| {
            let mut fx = cap.fixture();
            run(&mut fx, &cmd);
            let got = sig(&fx);
            if got != want {
                failures.push(format!("[{}] surface `{surface}` for `{cmd}` diverges from the colon path\n  colon: {want}\n  {surface}: {got}", cap.name));
            }
        };
        check("wm.command", &|fx, c| wm_command(fx, c));
        check("mcp bilterm_colon_command", &|fx, c| mcp_colon(fx, c));
        if let Some(k) = cap.key {
            check("key", &|fx, _| k(fx));
        }
        if let Some(m) = cap.mouse {
            check("mouse", &|fx, _| m(fx));
        }
        if let Some(entry) = cap.palette {
            check("palette key", &|fx, _| palette_key(fx, entry));
            check("palette click", &|fx, _| palette_click(fx, entry));
        }
        if let Some(menu) = cap.menu {
            check("menu key", &|fx, _| menu_key(fx, menu));
            check("menu click", &|fx, _| menu_click(fx, menu));
        }
        let named = match cap.named {
            Named::Derive => named_of(&cmd),
            Named::Fixed(m, p, t) => Some((m, p.to_string(), t)),
            Named::No => None,
        };
        match named {
            Some((method, params, tool)) => {
                check(method, &|fx, _| {
                    rpc(fx, method, &params);
                });
                check(tool, &|fx, _| {
                    mcp_tool(fx, tool, &params);
                });
            }
            None => assert!(
                matches!(cap.named, Named::No),
                "[{}] `{cmd}` has no named agents.* method but the row does not say so",
                cap.name
            ),
        }
    }
    assert!(failures.is_empty(), "{} surface(s) drifted:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn exceptions_are_exactly_the_recorded_six() {
    // The six exceptions of 51-CONTRACT.md section 9 by reason; expanded into (capability, surface) cells.
    let reasons: BTreeSet<&str> = EXCEPTIONS.iter().map(|e| e.2).collect();
    assert_eq!(
        reasons,
        BTreeSet::from(["entity-id", "numeric", "explicit-value", "not-const-constructible", "twin-op", "data-only"]),
        "exactly six exception reasons"
    );
    let allowed: BTreeSet<(&str, &str)> = EXCEPTIONS.iter().map(|e| (e.0, e.1)).collect();
    assert_eq!(allowed.len(), EXCEPTIONS.len(), "no duplicate exception cell");
    let mut empty: BTreeSet<(&str, &str)> = BTreeSet::new();
    let names: BTreeSet<&str> = capabilities().iter().map(|c| c.name).collect();
    for cap in capabilities() {
        if cap.key.is_none() {
            empty.insert((cap.name, "key"));
        }
        if cap.mouse.is_none() {
            empty.insert((cap.name, "mouse"));
        }
    }
    let unexplained: Vec<_> = empty.difference(&allowed).collect();
    assert!(unexplained.is_empty(), "empty cells that are not recorded exceptions: {unexplained:?}");
    let stale: Vec<_> = allowed.difference(&empty).collect();
    assert!(stale.is_empty(), "recorded exceptions whose cell is no longer empty: {stale:?}");
    for (cap, _, _) in EXCEPTIONS {
        assert!(names.contains(cap), "exception names an unknown capability {cap}");
    }
    // The claims behind the twin exceptions: the Minimize badge is Dismiss and the wheel is Scroll.
    let mut fx = Fx::std();
    s_open(&mut fx);
    let want = {
        let mut r = Fx::std();
        s_open(&mut r);
        colon(&mut r, ":agents list dismiss");
        sig(&r)
    };
    click_badge(&mut fx, ListBadge::Minimize);
    assert_eq!(sig(&fx), want, "the Minimize badge is the mouse twin of dismiss");
    // Exception 4: the pattern lowers to a runtime string action, not a const row.
    let arrange = parse_colon_command(":arrange grid").unwrap().unwrap();
    assert!(matches!(arrange, Action::LayoutProfileCommand(_)), "{arrange:?}");
}

#[test]
fn every_list_op_is_in_the_matrix() {
    let caps = capabilities();
    let names: BTreeSet<&str> = caps.iter().map(|c| c.name).collect();
    assert_eq!(names.len(), caps.len(), "capability names are unique");
    let mut covered = BTreeSet::new();
    for op in sample_list_ops() {
        let cap = capability_of(&op);
        assert!(names.contains(cap), "{op:?} belongs to `{cap}` which has no matrix row");
        covered.insert(cap);
        // Colon round trip.
        let form = op.colon_form();
        let parsed = parse_colon_command(&form).unwrap().unwrap_or_else(|e| panic!("{form}: {e}"));
        assert_eq!(parsed, Action::AgentsList(op.clone()), "`{form}` round-trips");
    }
    // Every list row is the home of some operation (a row with no operation is dead weight).
    let list_rows: BTreeSet<&str> = names.iter().copied().filter(|n| !matches!(*n, "status" | "demo.start" | "demo.stop" | "arrange")).collect();
    assert_eq!(covered, list_rows, "rows and operations match one to one");
    // And each row's colon command parses to an operation of that very row.
    for cap in &caps {
        if !list_rows.contains(cap.name) {
            continue;
        }
        let fx = cap.fixture();
        let cmd = cap.cmd(&fx);
        match parse_colon_command(&cmd).unwrap().unwrap() {
            Action::AgentsList(o) => assert_eq!(capability_of(&o), cap.name, "`{cmd}`"),
            other => panic!("`{cmd}` parsed to {other:?}"),
        }
    }
    // The in-window key table is covered by `key` cells of the right rows.
    for (_, _, action) in AGENTS_LIST_BINDINGS {
        let Action::AgentsList(o) = action else { panic!("{action:?} is not an AgentsList action") };
        let cap = caps.iter().find(|c| c.name == capability_of(o)).unwrap();
        assert!(cap.key.is_some() || EXCEPTIONS.iter().any(|e| e.0 == cap.name && e.1 == "key"), "{o:?} key row");
    }
}

#[test]
fn list_key_rows_match_colon_twins() {
    for (mods, code, action) in AGENTS_LIST_BINDINGS {
        let Action::AgentsList(o) = action else { panic!("{action:?}") };
        let form = o.colon_form();
        let setup = |fx: &mut Fx| {
            s_open(fx);
            op(fx, AgentsListOp::SelectPane(2));
        };
        let mut by_key = Fx::std();
        setup(&mut by_key);
        press(&mut by_key, *code, *mods);
        let mut by_colon = Fx::std();
        setup(&mut by_colon);
        colon(&mut by_colon, &form);
        assert_eq!(sig(&by_key), sig(&by_colon), "key {code:?}+{mods:?} vs `{form}`");
    }
}

#[test]
fn list_completion_help_palette_per_verb() {
    let row = bilterm::pane::completion::colon_command_table()
        .iter()
        .find(|(c, _)| *c == ":agents")
        .expect(":agents completion row")
        .1;
    let exclusions = bilterm::command_registry::lookup(":agents").expect(":agents in the registry").palette_exclusions;
    for op in sample_list_ops() {
        let form = op.colon_form();
        let verb = form.split_whitespace().nth(2).unwrap();
        // Completion: a `list <verb>` entry.
        assert!(
            row.iter().any(|s| s.trim_end().starts_with(&format!("list {verb}"))),
            "`{form}`: no `list {verb}` entry in the :agents completion row"
        );
        // Help: a `:agents list ...` row naming the verb.
        let documented = HELP_LINES.iter().any(|(l, _)| {
            l.starts_with(":agents list")
                && l.split(|c: char| !c.is_ascii_alphanumeric() && c != '-').any(|w| w == verb)
        });
        assert!(documented, "`{form}`: no HELP_LINES row names `{verb}`");
        // Palette: an entry parsing to the same Action, a runnable entry of the verb family, or an exclusion row.
        let parsed = parse_colon_command(&form).unwrap().unwrap();
        let exact = PALETTE_CATALOG
            .iter()
            .any(|e| parse_colon_command(e.0).and_then(Result::ok).as_ref() == Some(&parsed));
        let family = PALETTE_CATALOG.iter().any(|e| e.0.starts_with(&format!(":agents list {verb}")));
        let excluded = exclusions.iter().any(|(probe, _)| probe.starts_with(&format!(":agents list {verb}")));
        assert!(exact || family || excluded, "`{form}`: no palette entry and no exclusion row");
    }
    // Demo verbs: all three surfaces too.
    for form in [":agents demo", ":agents demo stop"] {
        let parsed = parse_colon_command(form).unwrap().unwrap();
        assert!(
            PALETTE_CATALOG.iter().any(|e| parse_colon_command(e.0).and_then(Result::ok).as_ref() == Some(&parsed)),
            "`{form}` has no palette entry"
        );
        assert!(HELP_LINES.iter().any(|(l, _)| l.starts_with(form)), "`{form}` has no help row");
    }
    assert!(row.contains(&"demo stop"), "completion row has `demo stop`");
}

#[test]
fn drawn_controls_are_clickable() {
    // Every badge and chip drawn at the default size does what its twin does, by clicking its center.
    let cases: [(&str, Drv, &str); 7] = [
        ("badge Pin", |fx| click_badge(fx, ListBadge::Pin), ":agents list pin toggle"),
        ("badge Minimize", |fx| click_badge(fx, ListBadge::Minimize), ":agents list dismiss"),
        ("badge Dock", |fx| click_badge(fx, ListBadge::Dock), ":agents list dock toggle"),
        ("badge Close", |fx| click_badge(fx, ListBadge::Close), ":agents list close"),
        ("chip [filter]", |fx| click_chip(fx, ListChip::Filter), ":agents list filter toggle"),
        ("chip [dock]", |fx| click_chip(fx, ListChip::Dock), ":agents list dock toggle"),
        ("chip [zoom]", |fx| click_chip(fx, ListChip::Zoom), ":agents list zoom"),
    ];
    let mut probe = Fx::std();
    s_open(&mut probe);
    let l = probe.layout();
    let badges: BTreeSet<String> = l.badges.iter().map(|(b, _)| format!("{b:?}")).collect();
    assert_eq!(badges, BTreeSet::from(["Pin", "Minimize", "Dock", "Close"].map(String::from)), "drawn badges");
    let chips: BTreeSet<String> = l.chips.iter().map(|(c, _)| format!("{c:?}")).collect();
    assert_eq!(chips, BTreeSet::from(["Filter", "Dock", "Zoom"].map(String::from)), "drawn chips");
    for (name, click_it, twin) in cases {
        let mut by_click = Fx::std();
        s_open(&mut by_click);
        let before = sig(&by_click);
        click_it(&mut by_click);
        let mut by_colon = Fx::std();
        s_open(&mut by_colon);
        colon(&mut by_colon, twin);
        assert_ne!(sig(&by_colon), before, "{name}: the twin changes something");
        assert_eq!(sig(&by_click), sig(&by_colon), "{name} vs `{twin}`");
    }
    // The documented effect, spelled out for the three most visible controls.
    let mut fx = Fx::std();
    s_open(&mut fx);
    click_badge(&mut fx, ListBadge::Pin);
    assert!(fx.p.wm.agents_list().pinned, "Pin badge pins");
    click_badge(&mut fx, ListBadge::Dock);
    assert!(fx.p.wm.agents_list().docked, "Dock badge docks");
    click_chip(&mut fx, ListChip::Filter);
    assert_eq!(fx.p.wm.agents_list().filter, ListFilter::Agents, "[filter] toggles the filter");
    click_badge(&mut fx, ListBadge::Minimize);
    assert!(fx.p.wm.agents_list().active && !fx.p.wm.agents_list().focused, "a pinned list is only blurred");
    click_chip(&mut fx, ListChip::Zoom); // retakes the keyboard (Open) and opens the traffic window
    assert!(fx.p.wm.agents_window().active, "[zoom] opens the traffic window");
}

#[test]
fn palette_entry_reachable_by_key_and_mouse() {
    for (entry, setup) in [
        (":agents list pin", s_open as Drv),
        (":agents list filter agents", s_open),
        (":agents list select last", s_open),
        (":agents list dock", s_open),
        (":agents list scale +10", s_open),
        (":agents list open", s_closed),
        (":agents demo", s_closed),
        (":arrange grid", s_panels_open),
    ] {
        let mut by_colon = Fx::new(2, 120, 40);
        setup(&mut by_colon);
        colon(&mut by_colon, entry);
        let want = sig(&by_colon);
        let mut by_key = Fx::new(2, 120, 40);
        setup(&mut by_key);
        palette_key(&mut by_key, entry);
        assert_eq!(sig(&by_key), want, "palette Enter on `{entry}`");
        let mut by_click = Fx::new(2, 120, 40);
        setup(&mut by_click);
        palette_click(&mut by_click, entry);
        assert_eq!(sig(&by_click), want, "palette click on `{entry}`");
    }
    // The palette is reachable from the documented chords: Ctrl+P and leader Space.
    let mut fx = Fx::std();
    press(&mut fx, KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert!(fx.p.wm.is_palette_visible(), "Ctrl+P opens the palette");
    press(&mut fx, KeyCode::Esc, N);
    leader(&mut fx, KeyCode::Char(' '), N);
    assert!(fx.p.wm.is_palette_visible(), "leader Space opens the palette");
    // Automation sees the exclusions: `bilterm://commands` lists a reason for the numeric rect form.
    let mut fx = Fx::std();
    let resp = fx
        .p
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"bilterm://commands"}}"#)
        .expect("resource response");
    let body = JsonValue::parse(&resp).expect("json");
    let text = body
        .get("result")
        .and_then(|r| r.get("contents"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .expect("resource text")
        .to_string();
    let commands = JsonValue::parse(&text).expect("commands json");
    let agents = commands
        .get("commands")
        .and_then(JsonValue::as_array)
        .and_then(|c| c.iter().find(|x| x.get("name").and_then(JsonValue::as_str) == Some(":agents")))
        .expect(":agents command")
        .clone();
    let ex = agents.get("palette_exclusions").and_then(JsonValue::as_array).expect("palette_exclusions");
    assert!(
        ex.iter().any(|e| e.get("probe").and_then(JsonValue::as_str) == Some(":agents list rect")
            && e.get("reason").and_then(JsonValue::as_str) == Some("numeric")),
        "{}",
        agents.to_json_string()
    );
}

/// Gate: every Tab-completion entry of the Phase 51 verb families has a runnable palette entry of its
/// family or an exclusion row (the full sweep lives in `src/command_registry.rs`; this re-runs it on
/// the families this phase touched, through the public registry).
#[test]
fn phase51_verb_families_are_palette_covered() {
    for head in [":agents", ":arrange", ":drawer"] {
        let row = bilterm::pane::completion::colon_command_table()
            .iter()
            .find(|(c, _)| *c == head)
            .unwrap_or_else(|| panic!("{head} completion row"))
            .1;
        let info = bilterm::command_registry::lookup(head).unwrap_or_else(|| panic!("{head} in the registry"));
        for sub in row {
            let probe = format!("{head} {}", sub.trim_end());
            let entry = PALETTE_CATALOG.iter().any(|e| e.0.starts_with(&probe));
            let excluded = info.palette_exclusions.iter().any(|(p, _)| probe.starts_with(p) || p.starts_with(&probe));
            assert!(entry || excluded, "`{probe}` has neither a palette entry nor an exclusion row");
        }
    }
}

#[test]
fn arrange_surfaces_agree() {
    // With the list and the traffic window open, every surface of `:arrange grid` tiles the same way.
    let drivers: [(&str, BoxDrv); 6] = [
        ("colon", Box::new(|fx| colon(fx, ":arrange grid"))),
        ("menu key", Box::new(|fx| menu_key(fx, ":arrange grid"))),
        ("menu click", Box::new(|fx| menu_click(fx, ":arrange grid"))),
        ("palette", Box::new(|fx| palette_key(fx, ":arrange grid"))),
        ("wm.command", Box::new(|fx| wm_command(fx, ":arrange grid"))),
        ("mcp", Box::new(|fx| mcp_colon(fx, ":arrange grid"))),
    ];
    let mut results = Vec::new();
    for (name, drive) in &drivers {
        let mut fx = Fx::std();
        s_panels_open(&mut fx);
        drive(&mut fx);
        assert!(fx.p.wm.agents_list().docked, "{name}: the list is docked");
        assert!(fx.p.wm.agents_window().docked, "{name}: the traffic window is docked");
        let (list, win) = (fx.list_rect(), fx.p.wm.agents_window_rect(fx.ws()).expect("traffic rect"));
        assert_eq!(list.intersection(win).area(), 0, "{name}: the two docked panels do not overlap");
        for (id, r) in fx.p.wm.compute_workspace_rects(fx.ws()) {
            assert_eq!(r.intersection(list).area(), 0, "{name}: pane {id} {r:?} overlaps the docked list {list:?}");
            assert_eq!(r.intersection(win).area(), 0, "{name}: pane {id} {r:?} overlaps the docked traffic window {win:?}");
        }
        results.push((*name, sig(&fx)));
    }
    for (name, s) in &results[1..] {
        assert_eq!(*s, results[0].1, "surface `{name}` diverges from the colon path");
    }
    // No panel open: every surface equals a fixture arranged by colon alone, and nothing is docked.
    let mut baseline = Fx::std();
    colon(&mut baseline, ":arrange grid");
    for (name, drive) in &drivers {
        let mut fx = Fx::std();
        drive(&mut fx);
        assert!(!fx.p.wm.agents_list().docked && !fx.p.wm.agents_window().docked, "{name}: nothing to dock");
        assert_eq!(sig(&fx), sig(&baseline), "{name}: with no panel open `:arrange grid` is the plain arrangement");
    }
    // Dock / float of the list agree by colon, key, badge, chip and RPC.
    let dock: [(&str, Drv); 5] = [
        ("colon", |fx| colon(fx, ":agents list dock toggle")),
        ("key", |fx| press(fx, KeyCode::Char('d'), N)),
        ("badge", |fx| click_badge(fx, ListBadge::Dock)),
        ("chip", |fx| click_chip(fx, ListChip::Dock)),
        ("rpc", |fx| {
            rpc(fx, "agents.list", r#"{"op":"dock","args":"toggle"}"#);
        }),
    ];
    let mut docked = Vec::new();
    for (name, drive) in dock {
        let mut fx = Fx::std();
        s_open(&mut fx);
        drive(&mut fx);
        assert!(fx.p.wm.agents_list().docked, "{name} docks");
        let s = sig(&fx);
        drive(&mut fx);
        assert!(!fx.p.wm.agents_list().docked, "{name} floats again");
        docked.push((name, s));
    }
    for (name, s) in &docked[1..] {
        assert_eq!(*s, docked[0].1, "dock by `{name}` diverges from the colon path");
    }
}

#[test]
fn demo_surfaces_agree() {
    let start: [(&str, BoxDrv); 8] = [
        ("colon", Box::new(|fx| colon(fx, ":agents demo"))),
        ("leader y", Box::new(|fx| leader(fx, KeyCode::Char('y'), N))),
        ("menu key", Box::new(|fx| menu_key(fx, ":agents demo"))),
        ("menu click", Box::new(|fx| menu_click(fx, ":agents demo"))),
        ("palette", Box::new(|fx| palette_key(fx, ":agents demo"))),
        ("agents.demo", Box::new(|fx| {
            rpc(fx, "agents.demo", "{}");
        })),
        ("wm.command", Box::new(|fx| wm_command(fx, ":agents demo"))),
        ("mcp tool", Box::new(|fx| {
            mcp_tool(fx, "bilterm_agents_demo", "{}");
        })),
    ];
    let stop: [(&str, BoxDrv); 8] = [
        ("colon", Box::new(|fx| colon(fx, ":agents demo stop"))),
        ("leader Y", Box::new(|fx| leader(fx, KeyCode::Char('Y'), KeyModifiers::SHIFT))),
        ("menu key", Box::new(|fx| menu_key(fx, ":agents demo stop"))),
        ("menu click", Box::new(|fx| menu_click(fx, ":agents demo stop"))),
        ("palette", Box::new(|fx| palette_key(fx, ":agents demo stop"))),
        ("agents.demo", Box::new(|fx| {
            rpc(fx, "agents.demo", r#"{"stop":true}"#);
        })),
        ("wm.command", Box::new(|fx| wm_command(fx, ":agents demo stop"))),
        ("mcp tool", Box::new(|fx| {
            mcp_tool(fx, "bilterm_agents_demo", r#"{"stop":true}"#);
        })),
    ];
    let origin = Fx::new(2, 120, 40);
    let (panes0, ring0, topo0, order0) = (
        origin.p.wm.pane_count(),
        origin.p.wm.traffic().len(),
        origin.p.wm.traffic().topology().len(),
        origin.pane_ids(),
    );
    let demo_events = |fx: &Fx| fx.p.wm.traffic().iter().filter(|e| e.preview.starts_with("[demo] ")).count();
    let mut started = Vec::new();
    for (name, drive) in &start {
        let mut fx = Fx::new(2, 120, 40);
        drive(&mut fx);
        assert_eq!(fx.p.wm.pane_count(), 3, "{name}: the demo splits to three panes");
        assert_eq!(demo_events(&fx), 13, "{name}: 13 [demo] events");
        assert!(fx.p.wm.agents_demo_summary().running, "{name}: running");
        started.push((*name, sig(&fx)));
    }
    for (name, s) in &started[1..] {
        assert_eq!(*s, started[0].1, "start by `{name}` diverges from the colon path");
    }
    let mut stopped = Vec::new();
    for (name, drive) in &stop {
        let mut fx = Fx::new(2, 120, 40);
        colon(&mut fx, ":agents demo");
        drive(&mut fx);
        assert_eq!(fx.p.wm.pane_count(), panes0, "{name}: stop restores the pane count");
        assert_eq!(fx.pane_ids(), order0, "{name}: and the original panes");
        assert_eq!(demo_events(&fx), 0, "{name}: no [demo] events left");
        assert_eq!(fx.p.wm.traffic().len(), ring0, "{name}: the ring is back to the start");
        assert_eq!(fx.p.wm.traffic().topology().len(), topo0, "{name}: no demo topology pairs left");
        assert!(!fx.p.wm.agents_demo_summary().running, "{name}: not running");
        stopped.push((*name, sig(&fx)));
    }
    for (name, s) in &stopped[1..] {
        assert_eq!(*s, stopped[0].1, "stop by `{name}` diverges from the colon path");
    }
}

#[test]
fn mcp_and_rpc_agree() {
    // Named RPC, the MCP tool and the colon path give the same list state for these operations.
    for (op_name, args, colon_cmd) in [
        ("pin", "", ":agents list pin"),
        ("select", "down", ":agents list select down"),
        ("filter", "agents", ":agents list filter agents"),
        ("dock", "", ":agents list dock"),
        ("float", "", ":agents list float"),
        ("scale", "120", ":agents list scale 120"),
        ("size", "auto", ":agents list size auto"),
    ] {
        let params = if args.is_empty() {
            format!(r#"{{"op":"{op_name}"}}"#)
        } else {
            format!(r#"{{"op":"{op_name}","args":"{args}"}}"#)
        };
        let setup = |fx: &mut Fx| {
            s_open(fx);
            if op_name == "float" {
                op(fx, AgentsListOp::Dock(Some(true)));
            }
        };
        let mut by_colon = Fx::std();
        setup(&mut by_colon);
        colon(&mut by_colon, colon_cmd);
        let want = sig(&by_colon);
        let mut by_rpc = Fx::std();
        setup(&mut by_rpc);
        let r = rpc(&mut by_rpc, "agents.list", &params);
        let mut by_mcp = Fx::std();
        setup(&mut by_mcp);
        mcp_tool(&mut by_mcp, "bilterm_agents_list", &params);
        assert_eq!(sig(&by_rpc), want, "agents.list {params}");
        assert_eq!(sig(&by_mcp), want, "bilterm_agents_list {params}");
        // The result object reports the same summary the colon path leaves behind.
        let al = r.get("agents_list").unwrap_or_else(|| panic!("agents_list object for {params}"));
        let s = by_colon.p.wm.agents_list_summary();
        assert_eq!(al.get("active").and_then(JsonValue::as_bool), Some(s.active), "{params}");
        assert_eq!(al.get("pinned").and_then(JsonValue::as_bool), Some(s.pinned), "{params}");
        assert_eq!(al.get("docked").and_then(JsonValue::as_bool), Some(s.docked), "{params}");
        assert_eq!(al.get("row_count").and_then(JsonValue::as_i64), Some(s.row_count as i64), "{params}");
        assert_eq!(al.get("scale_pct").and_then(JsonValue::as_i64), Some(i64::from(s.scale_pct)), "{params}");
    }
}

#[test]
fn modal_click_beats_floating_windows_under_it() {
    // A palette entry drawn over the list or the traffic window takes the click (regression of the
    // press being swallowed, or blurring the focused list, by the window beneath the modal).
    for setup in [s_open as Drv, s_panels_open, s_open_pinned] {
        let mut fx = Fx::std();
        setup(&mut fx);
        // Park the open list under the palette (the palette is centered on the screen).
        fx.p.wm.open_palette_with_query("agents toggle");
        let probe = fx.p.wm.active_modal_rect(fx.area).expect("palette rect");
        fx.p.wm.close_modal();
        if fx.p.wm.agents_list().active {
            op(&mut fx, AgentsListOp::SetRect { x: probe.x + 4, y: probe.y, w: 36, h: 8 });
        }
        if fx.p.wm.agents_window().active {
            fx.p.wm.set_agents_rect(Rect::new(probe.x, probe.y, 60, 20));
        }
        let focused = fx.p.wm.agents_list().focused;
        let entry = ":agents list select last";
        let idx = palette_open(&mut fx, entry);
        let outer = fx.p.wm.active_modal_rect(fx.area).expect("palette rect");
        let inner = inset_window_frame(outer);
        let hdr = palette_header_rows(fx.p.wm.config(), inner.height);
        click(&mut fx, inner.x + 6, inner.y + hdr + idx as u16);
        assert!(!fx.p.wm.is_palette_visible(), "the palette took the click");
        assert_eq!(fx.p.wm.agents_list().selected, Some(3), "the palette entry ran");
        assert_eq!(fx.p.wm.agents_list().focused, focused, "the click did not blur the list");
    }
}
