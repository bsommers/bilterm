use std::sync::Once;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::modals::{
    menubar_category_item_count, menubar_dropdown_rect, menubar_item_command, palette_header_rows,
};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::agents_list_geometry::{AgentsListRects, ListBadge, ListChip, ListHit};
use bilterm::wm::{SplitDirection, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

static ENV: Once = Once::new();

pub fn init_env() {
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

pub const N: KeyModifiers = KeyModifiers::NONE;
pub const CTRL_ALT: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

pub struct Fx {
    pub p: AutomationControlPlane,
    pub area: Rect,
}

impl Fx {
    pub fn new(panes: usize, cols: u16, rows: u16) -> Fx {
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
    pub fn std() -> Fx {
        let fx = Fx::new(3, 120, 40);
        assert_eq!(fx.pane_ids(), vec![1, 2, 3], "fixture pane ids");
        fx
    }

    pub fn pane_ids(&self) -> Vec<usize> {
        self.p.wm.ordered_pane_ids()
    }

    pub fn ws(&self) -> Rect {
        ShellLayout::compute(self.area, self.p.wm.config(), self.p.wm.pane_count()).workspace
    }

    pub fn layout(&self) -> AgentsListRects {
        self.p.wm.agents_list_layout(self.ws()).expect("list layout")
    }

    pub fn list_rect(&self) -> Rect {
        self.p.wm.agents_list_rect(self.ws()).expect("list rect")
    }
}

/// Everything a capability can change that a user (or an automation client) can observe.
pub fn sig(fx: &Fx) -> String {
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

pub fn op(fx: &mut Fx, op: AgentsListOp) {
    fx.p.wm.apply(Action::AgentsList(op)).unwrap();
}

pub fn colon(fx: &mut Fx, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    fx.p.wm.apply(parsed).unwrap();
}

pub fn press(fx: &mut Fx, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(&mut fx.p.wm, KeyEvent::new(code, mods)).unwrap();
}

pub fn leader(fx: &mut Fx, code: KeyCode, mods: KeyModifiers) {
    press(fx, KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert!(fx.p.wm.is_leader_pending(), "leader pending");
    press(fx, code, mods);
}

pub fn mouse_ev(fx: &mut Fx, kind: MouseEventKind, col: u16, row: u16, modifiers: KeyModifiers) {
    handle_wm_mouse_event(&mut fx.p.wm, MouseEvent { kind, column: col, row, modifiers }, fx.area).unwrap();
}

pub fn click(fx: &mut Fx, col: u16, row: u16) {
    mouse_ev(fx, MouseEventKind::Down(MouseButton::Left), col, row, N);
    mouse_ev(fx, MouseEventKind::Up(MouseButton::Left), col, row, N);
}

pub fn drag(fx: &mut Fx, from: (u16, u16), to: (u16, u16)) {
    mouse_ev(fx, MouseEventKind::Down(MouseButton::Left), from.0, from.1, N);
    mouse_ev(fx, MouseEventKind::Drag(MouseButton::Left), to.0, to.1, N);
    mouse_ev(fx, MouseEventKind::Up(MouseButton::Left), to.0, to.1, N);
}

pub fn center(r: Rect) -> (u16, u16) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

pub fn click_badge(fx: &mut Fx, badge: ListBadge) {
    let r = fx.layout().badges.iter().find(|(b, _)| *b == badge).unwrap_or_else(|| panic!("no {badge:?} badge")).1;
    let (c, y) = center(r);
    click(fx, c, y);
}

pub fn click_chip(fx: &mut Fx, chip: ListChip) {
    let r = fx.layout().chips.iter().find(|(c, _)| *c == chip).unwrap_or_else(|| panic!("no {chip:?} chip")).1;
    let (c, y) = center(r);
    click(fx, c, y);
}

pub fn row_cell(fx: &Fx, idx: u16) -> (u16, u16) {
    let l = fx.layout();
    (l.rows_area.x + 2, l.rows_area.y + idx)
}

pub fn click_row(fx: &mut Fx, idx: u16) {
    let (c, r) = row_cell(fx, idx);
    click(fx, c, r);
}

/// Ctrl+click: selects the row without jumping (FOCUS-01: a plain click selects and jumps).
pub fn ctrl_click_row(fx: &mut Fx, idx: u16) {
    let (c, r) = row_cell(fx, idx);
    mouse_ev(fx, MouseEventKind::Down(MouseButton::Left), c, r, KeyModifiers::CONTROL);
    mouse_ev(fx, MouseEventKind::Up(MouseButton::Left), c, r, KeyModifiers::CONTROL);
}

/// A title cell that is neither a badge nor a border (what a drag grabs).
pub fn title_cell(fx: &Fx) -> (u16, u16) {
    let l = fx.layout();
    let ws = fx.ws();
    let x = (l.title_row.x..l.title_row.right())
        .find(|&x| fx.p.wm.agents_list_hit(ws, x, l.title_row.y) == ListHit::Title)
        .expect("a draggable title cell");
    (x, l.title_row.y)
}

pub fn wheel(fx: &mut Fx, up: bool, mods: KeyModifiers) {
    let (c, r) = row_cell(fx, 0);
    let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
    mouse_ev(fx, kind, c, r, mods);
}

pub fn find_entry(cmd: &str) -> (usize, usize) {
    (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(cmd))
        .unwrap_or_else(|| panic!("no menu entry for {cmd}"))
}

pub fn menu_key(fx: &mut Fx, cmd: &str) {
    let (cat, item) = find_entry(cmd);
    fx.p.wm.set_menubar_open_category(Some(cat));
    for _ in 0..item {
        press(fx, KeyCode::Down, N);
    }
    press(fx, KeyCode::Enter, N);
    assert!(fx.p.wm.menubar_state().is_none(), "the menu closes after the pick of {cmd}");
}

pub fn menu_click(fx: &mut Fx, cmd: &str) {
    let (cat, item) = find_entry(cmd);
    fx.p.wm.set_menubar_open_category(Some(cat));
    let inner = inset_window_frame(menubar_dropdown_rect(cat, fx.ws()));
    click(fx, inner.x + 2, inner.y + item as u16);
    assert!(fx.p.wm.menubar_state().is_none(), "the click on {cmd} closes the menu");
}

/// Opens the palette on the plain words of `entry` and returns the row index of the entry.
pub fn palette_open(fx: &mut Fx, entry: &str) -> usize {
    fx.p.wm.open_palette_with_query(entry.trim_start_matches(':'));
    fx.p.wm
        .filtered_palette_items()
        .iter()
        .position(|e| e.0 == entry)
        .unwrap_or_else(|| panic!("palette does not list `{entry}` for its own words"))
}

pub fn palette_key(fx: &mut Fx, entry: &str) {
    let idx = palette_open(fx, entry);
    for _ in 0..idx {
        press(fx, KeyCode::Down, N);
    }
    press(fx, KeyCode::Enter, N);
    assert!(!fx.p.wm.is_palette_visible(), "Enter on `{entry}` closes the palette");
}

pub fn palette_click(fx: &mut Fx, entry: &str) {
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

pub fn rpc(fx: &mut Fx, method: &str, params: &str) -> JsonValue {
    let params = JsonValue::parse(params).unwrap_or_else(|| panic!("bad json {params}"));
    fx.p.dispatch_method(method, &params).unwrap_or_else(|e| panic!("{method}: {e}"))
}

pub fn wm_command(fx: &mut Fx, cmd: &str) {
    let params = JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]).to_json_string();
    rpc(fx, "wm.command", &params);
}

pub fn mcp_tool(fx: &mut Fx, tool: &str, args: &str) -> JsonValue {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#
    );
    let raw = fx.p.handle_mcp_line(&line).expect("mcp response");
    let r = JsonValue::parse(&raw).expect("mcp json");
    let err = r.get("result").and_then(|x| x.get("isError")).and_then(JsonValue::as_bool);
    assert_eq!(err, Some(false), "{tool} {args}: {raw}");
    r
}

pub fn mcp_colon(fx: &mut Fx, cmd: &str) {
    let args = JsonValue::obj([("command", JsonValue::String(cmd.to_string()))]).to_json_string();
    mcp_tool(fx, "bilterm_colon_command", &args);
}

/// `(rpc method, params json, mcp tool)` of the named method that carries `cmd`, when there is one.
pub fn named_of(cmd: &str) -> Option<(&'static str, String, &'static str)> {
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

pub type Drv = fn(&mut Fx);
pub type BoxDrv = Box<dyn Fn(&mut Fx)>;

#[derive(Clone, Copy)]
pub enum Cmd {
    Lit(&'static str),
    Dyn(fn(&Fx) -> String),
}

#[derive(Clone, Copy)]
pub enum Named {
    /// Derived from the colon form (`agents.list` / `agents.demo`).
    Derive,
    /// A named method whose request differs from the colon form (data read).
    Fixed(&'static str, &'static str, &'static str),
    /// No named method: `wm.command` and `bilterm_colon_command` are the API path.
    No,
}

pub struct Capability {
    pub name: &'static str,
    pub panes: usize,
    pub cols: u16,
    pub rows: u16,
    pub setup: Drv,
    pub colon: Cmd,
    pub key: Option<Drv>,
    pub mouse: Option<Drv>,
    pub palette: Option<&'static str>,
    pub menu: Option<&'static str>,
    pub named: Named,
}

impl Capability {
    pub fn fixture(&self) -> Fx {
        let mut fx = Fx::new(self.panes, self.cols, self.rows);
        (self.setup)(&mut fx);
        fx
    }

    pub fn cmd(&self, fx: &Fx) -> String {
        match self.colon {
            Cmd::Lit(s) => s.to_string(),
            Cmd::Dyn(f) => f(fx),
        }
    }
}

pub fn row(name: &'static str, setup: Drv, colon: &'static str) -> Capability {
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

pub fn s_closed(_: &mut Fx) {}
pub fn s_open(fx: &mut Fx) {
    fx.p.wm.open_agents_list();
}
pub fn s_open_pinned(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Pin(Some(true)));
}
pub fn s_open_second(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::SelectPane(2));
}
pub fn s_open_last(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Last);
}
pub fn s_open_docked(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Dock(Some(true)));
}
pub fn s_open_agents_filter(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Filter(Some(ListFilter::Agents)));
}
pub fn s_open_scrolled(fx: &mut Fx) {
    s_open(fx);
    op(fx, AgentsListOp::Scroll(6));
}
pub fn s_demo_started(fx: &mut Fx) {
    colon(fx, ":agents demo");
}
pub fn s_panels_open(fx: &mut Fx) {
    s_open(fx);
    fx.p.wm.open_agents_window(None);
}

pub fn blur_by_outside_click(fx: &mut Fx) {
    let ws = fx.ws();
    let list = fx.list_rect();
    let active = fx.p.wm.active_pane_id();
    let (_, r) = fx.p.wm.compute_workspace_rects(ws).into_iter().find(|(id, _)| *id == active).expect("active pane rect");
    let (c, y) = (r.x + 2, r.y + r.height / 2);
    assert!(!list.contains((c, y).into()), "the click lands outside the list");
    click(fx, c, y);
}

pub fn drag_title(fx: &mut Fx, dx: i32, dy: i32) {
    let from = title_cell(fx);
    let to = ((i32::from(from.0) + dx) as u16, (i32::from(from.1) + dy) as u16);
    drag(fx, from, to);
}

pub fn drag_east_edge(fx: &mut Fx, dx: i32) {
    let o = fx.list_rect();
    let from = (o.right() - 1, o.y + o.height / 2);
    assert!(matches!(fx.p.wm.agents_list_hit(fx.ws(), from.0, from.1), ListHit::Border(_)), "east border cell");
    drag(fx, from, ((i32::from(from.0) + dx) as u16, from.1));
}

pub fn drag_corner(fx: &mut Fx, dx: i32, dy: i32) {
    let o = fx.list_rect();
    let from = (o.right() - 1, o.bottom() - 1);
    assert!(matches!(fx.p.wm.agents_list_hit(fx.ws(), from.0, from.1), ListHit::Border(_)), "corner cell");
    drag(fx, from, ((i32::from(from.0) + dx) as u16, (i32::from(from.1) + dy) as u16));
}

pub fn capabilities() -> Vec<Capability> {
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
    // dismiss / blur. FOCUS-01: Esc is blur (it never closes), so dismiss has no key of its own (the
    // exception-5 twin; `q` closes) and its mouse path is the Minimize badge; blur has the click outside.
    v.push(Capability {
        mouse: Some(|fx| click_badge(fx, ListBadge::Minimize)),
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
        mouse: Some(|fx| ctrl_click_row(fx, 1)),
        palette: Some(":agents list select down"),
        ..row("select.down", s_open, ":agents list select down")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Char('k'), N)),
        mouse: Some(|fx| ctrl_click_row(fx, 1)),
        palette: Some(":agents list select up"),
        ..row("select.up", s_open_last, ":agents list select up")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::Home, N)),
        mouse: Some(|fx| ctrl_click_row(fx, 0)),
        palette: Some(":agents list select first"),
        ..row("select.first", s_open_last, ":agents list select first")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::End, N)),
        mouse: Some(|fx| ctrl_click_row(fx, 2)),
        palette: Some(":agents list select last"),
        ..row("select.last", s_open, ":agents list select last")
    });
    v.push(Capability {
        key: Some(|fx| press(fx, KeyCode::PageDown, N)),
        ..row("select.page", s_open, ":agents list select page-down")
    });
    v.push(Capability {
        mouse: Some(|fx| ctrl_click_row(fx, 1)),
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
pub fn capability_of(op: &AgentsListOp) -> &'static str {
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
pub fn sample_list_ops() -> Vec<AgentsListOp> {
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
pub const EXCEPTIONS: &[(&str, &str, &str)] = &[
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
    // 5. Dismiss key and Page mouse: the twins are other ops (Esc is Blur, `q` is Close / wheel Scroll); Scroll's
    //    key twin is Page (PgUp/PgDn), the mirror image of the same pairing. (Until FOCUS-01 Esc was Dismiss and
    //    the empty cell was Dismiss mouse.)
    ("list.dismiss", "key", "twin-op"),
    ("select.page", "mouse", "twin-op"),
    ("scroll", "key", "twin-op"),
    // 6. `:agents status` (the data-only List): no key or mouse.
    ("status", "key", "data-only"),
    ("status", "mouse", "data-only"),
];
