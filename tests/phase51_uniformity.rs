//! Phase 51-12: UNI-R01 parity matrix. Every Phase 51 capability is driven from identical headless
//! fixtures through every surface it has (colon, `wm.command`, key, mouse, palette entry by key and
//! by click, menu entry by key and by click, named RPC `agents.list` / `agents.demo`, MCP tools) and
//! must leave the same observable state. A surface that is missing must be one of the six recorded
//! exceptions of `51-CONTRACT.md` section 9, asserted here by capability, surface and reason.
//!
//! State is compared through `sig`, never through raw bytes or status text.

use std::collections::BTreeSet;
use std::sync::Once;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::agents_list_keys::AGENTS_LIST_BINDINGS;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::modals::{
    HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect, menubar_item_command,
    palette_header_rows,
};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::agents_list_geometry::{AgentsListRects, ListBadge, ListChip, ListHit};
use bilterm::wm::{SplitDirection, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

// ------------------------------------------------------------------------------------------------
// fixture and drivers
// ------------------------------------------------------------------------------------------------

static ENV: Once = Once::new();

fn init_env() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-p51-uniformity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY-equivalent: runs once, before any plane or key dispatch in this binary.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

const N: KeyModifiers = KeyModifiers::NONE;
const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

struct Fx {
    p: AutomationControlPlane,
    area: Rect,
}

impl Fx {
    fn new(panes: usize, cols: u16, rows: u16) -> Fx {
        init_env();
        let mut p = AutomationControlPlane::new_headless(rows, cols);
        let area = Rect::new(0, 0, cols, rows);
        for _ in 1..panes {
            p.wm.split_focused(SplitDirection::Vertical).unwrap();
        }
        p.wm.resize_all(area).unwrap();
        p.wm.update_agent_states(Instant::now());
        let first = p.wm.ordered_pane_ids()[0];
        p.wm.focus_and_raise(first);
        Fx { p, area }
    }

    /// The standard 120x40 fixture with three panes (ids 1, 2, 3; pane 1 active).
    fn std() -> Fx {
        let fx = Fx::new(3, 120, 40);
        assert_eq!(fx.pane_ids(), vec![1, 2, 3], "fixture pane ids");
        fx
    }

    fn pane_ids(&self) -> Vec<usize> {
        self.p.wm.ordered_pane_ids()
    }

    fn ws(&self) -> Rect {
        ShellLayout::compute(self.area, self.p.wm.config(), self.p.wm.pane_count()).workspace
    }

    fn layout(&self) -> AgentsListRects {
        self.p.wm.agents_list_layout(self.ws()).expect("list layout")
    }

    fn list_rect(&self) -> Rect {
        self.p.wm.agents_list_rect(self.ws()).expect("list rect")
    }
}

/// Everything a capability can change that a user (or an automation client) can observe.
fn sig(fx: &Fx) -> String {
    let wm = &fx.p.wm;
    let ws = fx.ws();
    let mut rects = wm.compute_workspace_rects(ws);
    rects.sort_by_key(|(id, _)| *id);
    let sizes: Vec<_> = wm.ordered_pane_ids().iter().map(|&id| wm.pane(id).unwrap().vt().size()).collect();
    let aw = wm.agents_window();
    let demo_events = wm.traffic().iter().filter(|e| e.preview.starts_with("[demo] ")).count();
    format!(
        "list={:?} scroll={} list_rect={:?} win=({},{},{},{},{:?}) win_rect={:?} demo={:?} panes={} active={} order={:?} \
         rects={:?} sizes={:?} ring={} demo_events={} topo={} menu={:?} palette={}",
        wm.agents_list_summary(),
        wm.agents_list().scroll,
        wm.agents_list_rect(ws),
        aw.active,
        aw.pinned,
        aw.focused,
        aw.docked,
        aw.selection.node,
        wm.agents_window_rect(ws),
        wm.agents_demo_summary(),
        wm.pane_count(),
        wm.active_pane_id(),
        wm.ordered_pane_ids(),
        rects,
        sizes,
        wm.traffic().len(),
        demo_events,
        wm.traffic().topology().len(),
        wm.menubar_state(),
        wm.is_palette_visible(),
    )
}

fn op(fx: &mut Fx, op: AgentsListOp) {
    fx.p.wm.apply(Action::AgentsList(op)).unwrap();
}

fn colon(fx: &mut Fx, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    fx.p.wm.apply(parsed).unwrap();
}

fn press(fx: &mut Fx, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(&mut fx.p.wm, KeyEvent::new(code, mods)).unwrap();
}

fn leader(fx: &mut Fx, code: KeyCode, mods: KeyModifiers) {
    press(fx, KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert!(fx.p.wm.is_leader_pending(), "leader pending");
    press(fx, code, mods);
}

fn mouse_ev(fx: &mut Fx, kind: MouseEventKind, col: u16, row: u16, modifiers: KeyModifiers) {
    handle_wm_mouse_event(&mut fx.p.wm, MouseEvent { kind, column: col, row, modifiers }, fx.area).unwrap();
}

fn click(fx: &mut Fx, col: u16, row: u16) {
    mouse_ev(fx, MouseEventKind::Down(MouseButton::Left), col, row, N);
    mouse_ev(fx, MouseEventKind::Up(MouseButton::Left), col, row, N);
}

fn drag(fx: &mut Fx, from: (u16, u16), to: (u16, u16)) {
    mouse_ev(fx, MouseEventKind::Down(MouseButton::Left), from.0, from.1, N);
    mouse_ev(fx, MouseEventKind::Drag(MouseButton::Left), to.0, to.1, N);
    mouse_ev(fx, MouseEventKind::Up(MouseButton::Left), to.0, to.1, N);
}

fn center(r: Rect) -> (u16, u16) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

fn click_badge(fx: &mut Fx, badge: ListBadge) {
    let r = fx.layout().badges.iter().find(|(b, _)| *b == badge).unwrap_or_else(|| panic!("no {badge:?} badge")).1;
    let (c, y) = center(r);
    click(fx, c, y);
}

fn click_chip(fx: &mut Fx, chip: ListChip) {
    let r = fx.layout().chips.iter().find(|(c, _)| *c == chip).unwrap_or_else(|| panic!("no {chip:?} chip")).1;
    let (c, y) = center(r);
    click(fx, c, y);
}

fn row_cell(fx: &Fx, idx: u16) -> (u16, u16) {
    let l = fx.layout();
    (l.rows_area.x + 2, l.rows_area.y + idx)
}

fn click_row(fx: &mut Fx, idx: u16) {
    let (c, r) = row_cell(fx, idx);
    click(fx, c, r);
}

/// A title cell that is neither a badge nor a border (what a drag grabs).
fn title_cell(fx: &Fx) -> (u16, u16) {
    let l = fx.layout();
    let ws = fx.ws();
    let x = (l.title_row.x..l.title_row.right())
        .find(|&x| fx.p.wm.agents_list_hit(ws, x, l.title_row.y) == ListHit::Title)
        .expect("a draggable title cell");
    (x, l.title_row.y)
}

fn wheel(fx: &mut Fx, up: bool, mods: KeyModifiers) {
    let (c, r) = row_cell(fx, 0);
    let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
    mouse_ev(fx, kind, c, r, mods);
}

fn find_entry(cmd: &str) -> (usize, usize) {
    (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(cmd))
        .unwrap_or_else(|| panic!("no menu entry for {cmd}"))
}

fn menu_key(fx: &mut Fx, cmd: &str) {
    let (cat, item) = find_entry(cmd);
    fx.p.wm.set_menubar_open_category(Some(cat));
    for _ in 0..item {
        press(fx, KeyCode::Down, N);
    }
    press(fx, KeyCode::Enter, N);
    assert!(fx.p.wm.menubar_state().is_none(), "the menu closes after the pick of {cmd}");
}

fn menu_click(fx: &mut Fx, cmd: &str) {
    let (cat, item) = find_entry(cmd);
    fx.p.wm.set_menubar_open_category(Some(cat));
    let inner = inset_window_frame(menubar_dropdown_rect(cat, fx.ws()));
    click(fx, inner.x + 2, inner.y + item as u16);
    assert!(fx.p.wm.menubar_state().is_none(), "the click on {cmd} closes the menu");
}

/// Opens the palette on the plain words of `entry` and returns the row index of the entry.
fn palette_open(fx: &mut Fx, entry: &str) -> usize {
    fx.p.wm.open_palette_with_query(entry.trim_start_matches(':'));
    fx.p.wm
        .filtered_palette_items()
        .iter()
        .position(|e| e.0 == entry)
        .unwrap_or_else(|| panic!("palette does not list `{entry}` for its own words"))
}

fn palette_key(fx: &mut Fx, entry: &str) {
    let idx = palette_open(fx, entry);
    for _ in 0..idx {
        press(fx, KeyCode::Down, N);
    }
    press(fx, KeyCode::Enter, N);
    assert!(!fx.p.wm.is_palette_visible(), "Enter on `{entry}` closes the palette");
}

fn palette_click(fx: &mut Fx, entry: &str) {
    let idx = palette_open(fx, entry);
    let outer = fx.p.wm.active_modal_rect(fx.area).expect("palette rect");
    let inner = inset_window_frame(outer);
    let hdr = palette_header_rows(fx.p.wm.config(), inner.height);
    let list_rows = usize::from(inner.height.saturating_sub(hdr)).max(1);
    // Past the first screenful the palette scrolls so the selected row is the last visible one.
    let visible_row = if idx < list_rows {
        idx
    } else {
        for _ in 0..idx {
            press(fx, KeyCode::Down, N);
        }
        list_rows - 1
    };
    click(fx, inner.x + 4, inner.y + hdr + visible_row as u16);
    assert!(!fx.p.wm.is_palette_visible(), "a click on `{entry}` closes the palette");
}

fn rpc(fx: &mut Fx, method: &str, params: &str) -> JsonValue {
    let params = JsonValue::parse(params).unwrap_or_else(|| panic!("bad json {params}"));
    fx.p.dispatch_method(method, &params).unwrap_or_else(|e| panic!("{method}: {e}"))
}

fn wm_command(fx: &mut Fx, cmd: &str) {
    let params = JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]).to_json_string();
    rpc(fx, "wm.command", &params);
}

fn mcp_tool(fx: &mut Fx, tool: &str, args: &str) -> JsonValue {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#
    );
    let raw = fx.p.handle_mcp_line(&line).expect("mcp response");
    let r = JsonValue::parse(&raw).expect("mcp json");
    let err = r.get("result").and_then(|x| x.get("isError")).and_then(JsonValue::as_bool);
    assert_eq!(err, Some(false), "{tool} {args}: {raw}");
    r
}

fn mcp_colon(fx: &mut Fx, cmd: &str) {
    let args = JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]).to_json_string();
    mcp_tool(fx, "bilterm_colon_command", &args);
}

/// `(rpc method, params json, mcp tool)` of the named method that carries `cmd`, when there is one.
fn named_of(cmd: &str) -> Option<(&'static str, String, &'static str)> {
    if let Some(rest) = cmd.strip_prefix(":agents list") {
        let rest = rest.trim();
        let json = match rest.split_once(' ') {
            None if rest.is_empty() => "{}".to_string(),
            None => format!(r#"{{"op":"{rest}"}}"#),
            Some((op, args)) => format!(r#"{{"op":"{op}","args":"{}"}}"#, args.trim()),
        };
        return Some(("agents.list", json, "bilterm_agents_list"));
    }
    match cmd {
        ":agents demo" => Some(("agents.demo", "{}".into(), "bilterm_agents_demo")),
        ":agents demo stop" => Some(("agents.demo", r#"{"stop":true}"#.into(), "bilterm_agents_demo")),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------
// the capability table
// ------------------------------------------------------------------------------------------------

type Drv = fn(&mut Fx);
type BoxDrv = Box<dyn Fn(&mut Fx)>;

#[derive(Clone, Copy)]
enum Cmd {
    Lit(&'static str),
    Dyn(fn(&Fx) -> String),
}

#[derive(Clone, Copy)]
enum Named {
    /// Derived from the colon form (`agents.list` / `agents.demo`).
    Derive,
    /// A named method whose request differs from the colon form (data read).
    Fixed(&'static str, &'static str, &'static str),
    /// No named method: `wm.command` and `bilterm_colon_command` are the API path.
    No,
}

struct Capability {
    name: &'static str,
    panes: usize,
    cols: u16,
    rows: u16,
    setup: Drv,
    colon: Cmd,
    key: Option<Drv>,
    mouse: Option<Drv>,
    palette: Option<&'static str>,
    menu: Option<&'static str>,
    named: Named,
}

impl Capability {
    fn fixture(&self) -> Fx {
        let mut fx = Fx::new(self.panes, self.cols, self.rows);
        (self.setup)(&mut fx);
        fx
    }

    fn cmd(&self, fx: &Fx) -> String {
        match self.colon {
            Cmd::Lit(s) => s.to_string(),
            Cmd::Dyn(f) => f(fx),
        }
    }
}

fn row(name: &'static str, setup: Drv, colon: &'static str) -> Capability {
    Capability {
        name,
        panes: 3,
        cols: 120,
        rows: 40,
        setup,
        colon: Cmd::Lit(colon),
        key: None,
        mouse: None,
        palette: None,
        menu: None,
        named: Named::Derive,
    }
}

fn s_closed(_: &mut Fx) {}
fn s_open(fx: &mut Fx) {
    fx.p.wm.open_agents_list();
}
fn s_open_pinned(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Pin(Some(true)));
}
fn s_open_second(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::SelectPane(2));
}
fn s_open_last(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Last);
}
fn s_open_docked(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Dock(Some(true)));
}
fn s_open_agents_filter(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Filter(Some(ListFilter::Agents)));
}
fn s_open_scrolled(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Scroll(6));
}
fn s_demo_started(fx: &mut Fx) {
    colon(fx, ":agents demo");
}
fn s_panels_open(fx: &mut Fx) {
    s_open(fx);
    fx.p.wm.open_agents_window(None);
}

fn blur_by_outside_click(fx: &mut Fx) {
    let ws = fx.ws();
    let list = fx.list_rect();
    let active = fx.p.wm.active_pane_id();
    let (_, r) = fx.p.wm.compute_workspace_rects(ws).into_iter().find(|(id, _)| *id == active).expect("active pane rect");
    let (c, y) = (r.x + 2, r.y + r.height / 2);
    assert!(!list.contains((c, y).into()), "the click lands outside the list");
    click(fx, c, y);
}

fn drag_title(fx: &mut Fx, dx: i32, dy: i32) {
    let from = title_cell(fx);
    let to = ((i32::from(from.0) + dx) as u16, (i32::from(from.1) + dy) as u16);
    drag(fx, from, to);
}

fn drag_east_edge(fx: &mut Fx, dx: i32) {
    let o = fx.list_rect();
    let from = (o.right() - 1, o.y + o.height / 2);
    assert!(matches!(fx.p.wm.agents_list_hit(fx.ws(), from.0, from.1), ListHit::Border(_)), "east border cell");
    drag(fx, from, ((i32::from(from.0) + dx) as u16, from.1));
}

fn drag_corner(fx: &mut Fx, dx: i32, dy: i32) {
    let o = fx.list_rect();
    let from = (o.right() - 1, o.bottom() - 1);
    assert!(matches!(fx.p.wm.agents_list_hit(fx.ws(), from.0, from.1), ListHit::Border(_)), "corner cell");
    drag(fx, from, ((i32::from(from.0) + dx) as u16, (i32::from(from.1) + dy) as u16));
}

fn capabilities() -> Vec<Capability> {
    let ctrl_alt_n: Drv = |fx| press(fx, KeyCode::Char('n'), CTRL_ALT);
    let mut v = Vec::new();

    // UIT-03: open / close / toggle.
    v.push(Capability {
        key: Some(ctrl_alt_n),
        mouse: Some(|fx| menu_click(fx, ":agents list toggle")),
        palette: Some(":agents list open"),
        ..row("list.open", s_closed, ":agents list open")
    });
    v.push(Capability {
        key: Some(ctrl_alt_n),
        mouse: Some(|fx| click_badge(fx, ListBadge::Close)),
        palette: Some(":agents list close"),
        ..row("list.close", s_open, ":agents list close")
    });
    v.push(Capability {
        key: Some(|fx| leader(fx, KeyCode::Char('A'), KeyModifiers::SHIFT)),
        mouse: Some(|fx| menu_click(fx, ":agents list toggle")),
        palette: Some(":agents list toggle"),
        menu: Some(":agents list toggle"),
        ..row("list.toggle", s_closed, ":agents list toggle")
    });
    // dismiss / blur (mouse of dismiss is the exception-5 twin; blur has the click outside).
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Esc, N)),
        palette: Some(":agents list dismiss"),
        ..row("list.dismiss", s_open, ":agents list dismiss")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Esc, N)),
        mouse: Some(blur_by_outside_click),
        palette: Some(":agents list blur"),
        ..row("list.blur", s_open_pinned, ":agents list blur")
    });
    // select / step / first / last / page / id.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Down, N)),
        mouse: Some(|fx| click_row(fx, 1)),
        palette: Some(":agents list select down"),
        ..row("select.down", s_open, ":agents list select down")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('k'), N)),
        mouse: Some(|fx| click_row(fx, 1)),
        palette: Some(":agents list select up"),
        ..row("select.up", s_open_last, ":agents list select up")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Home, N)),
        mouse: Some(|fx| click_row(fx, 0)),
        palette: Some(":agents list select first"),
        ..row("select.first", s_open_last, ":agents list select first")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::End, N)),
        mouse: Some(|fx| click_row(fx, 2)),
        palette: Some(":agents list select last"),
        ..row("select.last", s_open, ":agents list select last")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::PageDown, N)),
        ..row("select.page", s_open, ":agents list select page-down")
    });
    v.push(Capability {
        mouse: Some(|fx| click_row(fx, 1)),
        ..row("select.id", s_open, ":agents list select #2")
    });
    // jump / zoom.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Enter, N)),
        mouse: Some(|fx| click_row(fx, 1)),
        palette: Some(":agents list jump"),
        ..row("jump", s_open_second, ":agents list jump")
    });
    v.push(Capability { mouse: Some(|fx| click_row(fx, 2)), ..row("jump.id", s_open_last, ":agents list jump 3") });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('z'), N)),
        mouse: Some(|fx| click_chip(fx, ListChip::Zoom)),
        palette: Some(":agents list zoom"),
        ..row("zoom", s_open_second, ":agents list zoom")
    });
    v.push(Capability {
        mouse: Some(|fx| {
            let (c, r) = row_cell(fx, 1);
            mouse_ev(fx, MouseEventKind::Down(MouseButton::Right), c, r, N);
        }),
        ..row("zoom.id", s_open, ":agents list zoom 2")
    });
    // pin / unpin.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('*'), N)),
        mouse: Some(|fx| click_badge(fx, ListBadge::Pin)),
        ..row("pin.toggle", s_open, ":agents list pin toggle")
    });
    v.push(Capability { palette: Some(":agents list pin"), ..row("pin.set", s_open, ":agents list pin") });
    v.push(Capability { palette: Some(":agents list unpin"), ..row("unpin", s_open_pinned, ":agents list unpin") });
    // scale.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char(']'), N)),
        mouse: Some(|fx| wheel(fx, true, KeyModifiers::CONTROL)),
        palette: Some(":agents list scale +10"),
        ..row("scale.up", s_open, ":agents list scale +10")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('['), N)),
        mouse: Some(|fx| wheel(fx, false, KeyModifiers::CONTROL)),
        palette: Some(":agents list scale -10"),
        ..row("scale.down", s_open, ":agents list scale -10")
    });
    // filter.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('f'), N)),
        mouse: Some(|fx| click_chip(fx, ListChip::Filter)),
        ..row("filter.toggle", s_open, ":agents list filter toggle")
    });
    v.push(Capability {
        palette: Some(":agents list filter agents"),
        ..row("filter.set", s_open, ":agents list filter agents")
    });
    v.push(Capability {
        palette: Some(":agents list filter all"),
        ..row("filter.all", s_open_agents_filter, ":agents list filter all")
    });
    // dock / float (UIT-04).
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('d'), N)),
        mouse: Some(|fx| click_badge(fx, ListBadge::Dock)),
        ..row("dock.toggle", s_open, ":agents list dock toggle")
    });
    v.push(Capability { palette: Some(":agents list dock"), ..row("dock.set", s_open, ":agents list dock") });
    v.push(Capability { palette: Some(":agents list float"), ..row("float", s_open_docked, ":agents list float") });
    // geometry.
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Left, KeyModifiers::SHIFT)),
        mouse: Some(|fx| drag_title(fx, -1, 0)),
        palette: Some(":agents list move -1 0"),
        ..row("move", s_open, ":agents list move -1 0")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Right, KeyModifiers::ALT)),
        mouse: Some(|fx| drag_east_edge(fx, 1)),
        palette: Some(":agents list resize 1 0"),
        ..row("resize", s_open, ":agents list resize 1 0")
    });
    v.push(Capability {
        mouse: Some(|fx| drag_title(fx, -4, 2)),
        colon: Cmd::Dyn(|fx| {
            let r = fx.list_rect();
            format!(":agents list rect {} {} {} {}", r.x - 4, r.y + 2, r.width, r.height)
        }),
        ..row("rect", s_open, "")
    });
    v.push(Capability {
        mouse: Some(|fx| drag_corner(fx, -4, -1)),
        colon: Cmd::Dyn(|fx| {
            let r = fx.list_rect();
            format!(":agents list size {}x{}", r.width - 4, r.height - 1)
        }),
        ..row("size", s_open, "")
    });
    v.push(Capability {
        mouse: Some(|fx| wheel(fx, true, N)),
        palette: Some(":agents list scroll -3"),
        panes: 12,
        rows: 20,
        ..row("scroll", s_open_scrolled, ":agents list scroll -3")
    });
    // data-only status.
    v.push(Capability {
        palette: Some(":agents list"),
        named: Named::Fixed("agents.list", "{}", "bilterm_agents_list"),
        ..row("status", s_closed, ":agents status")
    });
    // UIT-01: demo.
    v.push(Capability {
        panes: 2,
        key: Some(|fx| leader(fx, KeyCode::Char('y'), N)),
        mouse: Some(|fx| menu_click(fx, ":agents demo")),
        palette: Some(":agents demo"),
        menu: Some(":agents demo"),
        ..row("demo.start", s_closed, ":agents demo")
    });
    v.push(Capability {
        panes: 2,
        key: Some(|fx| leader(fx, KeyCode::Char('Y'), KeyModifiers::SHIFT)),
        mouse: Some(|fx| menu_click(fx, ":agents demo stop")),
        palette: Some(":agents demo stop"),
        menu: Some(":agents demo stop"),
        ..row("demo.stop", s_demo_started, ":agents demo stop")
    });
    // UIT-04: tile everything with the panels open (no key: exception 4).
    v.push(Capability {
        mouse: Some(|fx| menu_click(fx, ":arrange grid")),
        palette: Some(":arrange grid"),
        menu: Some(":arrange grid"),
        named: Named::No,
        ..row("arrange", s_panels_open, ":arrange grid")
    });
    v
}

/// The capability a list operation belongs to. Exhaustive on purpose: a new `AgentsListOp` variant
/// fails to compile here until it gets a row (and a row name) in the matrix.
fn capability_of(op: &AgentsListOp) -> &'static str {
    use AgentsListOp as O;
    match op {
        O::Open => "list.open",
        O::Close => "list.close",
        O::Toggle => "list.toggle",
        O::Dismiss => "list.dismiss",
        O::Blur => "list.blur",
        O::Step(n) if *n < 0 => "select.up",
        O::Step(_) => "select.down",
        O::First => "select.first",
        O::Last => "select.last",
        O::Page(_) => "select.page",
        O::SelectPane(_) => "select.id",
        O::Jump(None) => "jump",
        O::Jump(Some(_)) => "jump.id",
        O::Zoom(None) => "zoom",
        O::Zoom(Some(_)) => "zoom.id",
        O::Pin(None) => "pin.toggle",
        O::Pin(Some(true)) => "pin.set",
        O::Pin(Some(false)) => "unpin",
        O::ScaleStep(n) if *n < 0 => "scale.down",
        O::ScaleStep(_) => "scale.up",
        O::Filter(None) => "filter.toggle",
        O::Filter(Some(ListFilter::Agents)) => "filter.set",
        O::Filter(Some(ListFilter::All)) => "filter.all",
        O::Dock(None) => "dock.toggle",
        O::Dock(Some(true)) => "dock.set",
        O::Dock(Some(false)) => "float",
        O::Move { .. } => "move",
        O::Resize { .. } => "resize",
        O::SetRect { .. } => "rect",
        O::Size(_) => "size",
        O::Scroll(_) => "scroll",
    }
}

/// One value of every `AgentsListOp` variant and sub-form (the table of 51-01's `sample_list_ops`).
fn sample_list_ops() -> Vec<AgentsListOp> {
    use AgentsListOp as O;
    vec![
        O::Open,
        O::Close,
        O::Toggle,
        O::Dismiss,
        O::Blur,
        O::Step(-1),
        O::Step(1),
        O::First,
        O::Last,
        O::Page(-1),
        O::Page(1),
        O::SelectPane(2),
        O::Jump(None),
        O::Jump(Some(3)),
        O::Zoom(None),
        O::Zoom(Some(2)),
        O::Pin(None),
        O::Pin(Some(true)),
        O::Pin(Some(false)),
        O::ScaleStep(-10),
        O::ScaleStep(10),
        O::Filter(None),
        O::Filter(Some(ListFilter::All)),
        O::Filter(Some(ListFilter::Agents)),
        O::Dock(None),
        O::Dock(Some(true)),
        O::Dock(Some(false)),
        O::Move { dx: -1, dy: 0 },
        O::Resize { dw: 1, dh: 0 },
        O::SetRect { x: 2, y: 2, w: 40, h: 10 },
        O::Size(Some((30, 8))),
        O::Size(None),
        O::Scroll(-3),
        O::Scroll(3),
    ]
}

// ------------------------------------------------------------------------------------------------
// the six recorded exceptions (51-CONTRACT.md section 9), literal copy
// ------------------------------------------------------------------------------------------------

/// `(capability, surface, reason)`: the matrix cells allowed to be empty.
const EXCEPTIONS: &[(&str, &str, &str)] = &[
    // 1. SelectPane(id), Jump(Some(id)), Zoom(Some(id)): no dedicated key (arrows + Enter act on the selection).
    ("select.id", "key", "entity-id"),
    ("jump.id", "key", "entity-id"),
    ("zoom.id", "key", "entity-id"),
    // 2. Size, SetRect: no key (numeric); the mouse path is the edge / title drag.
    ("size", "key", "numeric"),
    ("rect", "key", "numeric"),
    // 3. Explicit-value toggle forms: key and mouse send the toggle form; explicit forms are colon/RPC/palette.
    ("pin.set", "key", "explicit-value"),
    ("pin.set", "mouse", "explicit-value"),
    ("unpin", "key", "explicit-value"),
    ("unpin", "mouse", "explicit-value"),
    ("filter.set", "key", "explicit-value"),
    ("filter.set", "mouse", "explicit-value"),
    ("filter.all", "key", "explicit-value"),
    ("filter.all", "mouse", "explicit-value"),
    ("dock.set", "key", "explicit-value"),
    ("dock.set", "mouse", "explicit-value"),
    ("float", "key", "explicit-value"),
    ("float", "mouse", "explicit-value"),
    // 4. `:arrange <pattern>` lowers to Action::LayoutProfileCommand(String): not const-constructible, so no key row.
    ("arrange", "key", "not-const-constructible"),
    // 5. Dismiss mouse and Page mouse: the twins are other ops (Minimize badge / wheel Scroll); Scroll's key
    //    twin is Page (PgUp/PgDn), the mirror image of the same pairing.
    ("list.dismiss", "mouse", "twin-op"),
    ("select.page", "mouse", "twin-op"),
    ("scroll", "key", "twin-op"),
    // 6. `:agents status` (the data-only List): no key or mouse.
    ("status", "key", "data-only"),
    ("status", "mouse", "data-only"),
];

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
