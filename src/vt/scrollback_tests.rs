use super::*;

fn policy(budget: u64, pressure: bool) -> ResolvedPolicy {
    ResolvedPolicy {
        budget_bytes: budget,
        max_lines: None,
        min_lines: 100,
        initial_lines: 1_000,
        memory_pressure: pressure,
    }
}

#[test]
fn parses_budget_strings_and_ints() {
    assert_eq!(parse_memory_budget("auto"), Ok(MemoryBudget::Auto));
    assert_eq!(parse_memory_budget(" AUTO "), Ok(MemoryBudget::Auto));
    assert_eq!(parse_memory_budget("25%"), Ok(MemoryBudget::Percent(25)));
    assert_eq!(
        parse_memory_budget("8GiB"),
        Ok(MemoryBudget::Bytes(8 * GIB))
    );
    assert_eq!(
        parse_memory_budget("512M"),
        Ok(MemoryBudget::Bytes(512 * MIB))
    );
    assert_eq!(
        parse_memory_budget("64 KiB"),
        Ok(MemoryBudget::Bytes(64 * KIB))
    );
    assert_eq!(parse_memory_budget("4096"), Ok(MemoryBudget::Bytes(4096)));
    assert!(parse_memory_budget("0%").is_err());
    assert!(parse_memory_budget("150%").is_err());
    assert!(parse_memory_budget("8 parsecs").is_err());

    let cfg: ScrollbackConfig = toml::from_str(
        "memory_budget = 1073741824\nmax_lines = \"250_000\"\nmin_lines = \"500\"\ninitial_lines = 4000\n",
    )
    .unwrap();
    assert_eq!(cfg.memory_budget, MemoryBudget::Bytes(GIB));
    assert_eq!(cfg.max_lines, LineLimit::Fixed(250_000));
    assert_eq!(cfg.min_lines, 500);
    assert_eq!(cfg.initial_lines, 4000);
    let cfg: ScrollbackConfig =
        toml::from_str("max_lines = \"auto\"\nmemory_budget = \"10%\"").unwrap();
    assert_eq!(cfg.max_lines, LineLimit::Auto);
    assert_eq!(cfg.memory_budget, MemoryBudget::Percent(10));
    assert!(toml::from_str::<ScrollbackConfig>("memory_budget = \"lots\"").is_err());
    let text = toml::to_string(&cfg).unwrap();
    assert_eq!(toml::from_str::<ScrollbackConfig>(&text).unwrap(), cfg);
}

#[test]
fn auto_budget_scales_with_host_ram() {
    let big = resolve_budget_bytes(MemoryBudget::Auto, Some(192 * GIB));
    let laptop = resolve_budget_bytes(MemoryBudget::Auto, Some(16 * GIB));
    assert_eq!(big, 192 * GIB / 10);
    assert_eq!(laptop, 16 * GIB / 10);
    assert_eq!(big / laptop, 12);
    assert_eq!(
        resolve_budget_bytes(MemoryBudget::Auto, Some(256 * MIB)),
        MIN_AUTO_BUDGET_BYTES
    );
    assert_eq!(
        resolve_budget_bytes(MemoryBudget::Auto, None),
        FALLBACK_BUDGET_BYTES
    );
    assert_eq!(
        resolve_budget_bytes(MemoryBudget::Percent(25), Some(16 * GIB)),
        4 * GIB
    );
    assert_eq!(
        resolve_budget_bytes(MemoryBudget::Bytes(8 * GIB), None),
        8 * GIB
    );

    let cfg = ScrollbackConfig::default();
    let snap = |t| MemorySnapshot {
        total_bytes: Some(t),
        available_bytes: Some(t / 2),
    };
    let big = ResolvedPolicy::resolve(&cfg, snap(192 * GIB));
    let small = ResolvedPolicy::resolve(&cfg, snap(16 * GIB));
    assert!(big.pane_cap_lines(80) > 7_000_000);
    assert!((600_000..700_000).contains(&small.pane_cap_lines(80)));
    assert_eq!(small.initial_capacity(80), DEFAULT_INITIAL_LINES);
}

#[test]
fn per_pane_cap_respects_fixed_max_and_min() {
    let mut p = policy(10 * MIB, false);
    assert_eq!(row_bytes(80), 80 * 32 + 64);
    assert_eq!(p.pane_cap_lines(80), (10 * MIB / row_bytes(80)) as usize);
    p.max_lines = Some(1_500);
    assert_eq!(p.pane_cap_lines(80), 1_500);
    p.max_lines = Some(50);
    assert_eq!(p.pane_cap_lines(80), 100, "min_lines floor wins");
    let tiny = policy(1, false);
    assert_eq!(tiny.pane_cap_lines(80), 100);
    let cfg = ScrollbackConfig {
        max_lines: LineLimit::Fixed(1_000),
        ..Default::default()
    };
    let r = ResolvedPolicy::resolve(&cfg, MemorySnapshot::default());
    assert_eq!(r.min_lines, 1_000);
    assert_eq!(r.initial_capacity(80), 1_000);
}

#[test]
fn nearly_full_pane_doubles_and_active_pane_borrows_idle_budget() {
    let p = policy(100 * MIB, false);
    let full = PaneUsage {
        used_rows: 950,
        capacity: 1_000,
        cols: 80,
    };
    let idle = PaneUsage {
        used_rows: 10,
        capacity: 1_000,
        cols: 80,
    };
    assert_eq!(plan_capacities(&[full, idle], &p), vec![2_000, 1_000]);
    let half = PaneUsage {
        used_rows: 500,
        capacity: 1_000,
        cols: 80,
    };
    assert_eq!(plan_capacities(&[half], &p), vec![1_000]);
    let big = PaneUsage {
        used_rows: 15_000,
        capacity: 15_000,
        cols: 80,
    };
    let fair = (50 * MIB / row_bytes(80)) as usize;
    let planned = plan_capacities(&[big, idle], &p);
    assert_eq!(planned[0], 30_000);
    assert!(planned[0] > fair);
    let near = PaneUsage {
        used_rows: 39_000,
        capacity: 39_000,
        cols: 80,
    };
    let cap = (100 * MIB / row_bytes(80)) as usize;
    assert_eq!(plan_capacities(&[near], &p), vec![cap]);
}

#[test]
fn contracts_largest_panes_when_over_budget_or_low_memory() {
    let p = policy(10 * MIB, false);
    let fair = (5 * MIB / row_bytes(80)) as usize;
    let huge = PaneUsage {
        used_rows: 6_000,
        capacity: 8_000,
        cols: 80,
    };
    let small = PaneUsage {
        used_rows: 200,
        capacity: 1_000,
        cols: 80,
    };
    let planned = plan_capacities(&[small, huge], &p);
    assert_eq!(planned, vec![1_000, fair]);

    let cfg = ScrollbackConfig {
        memory_budget: MemoryBudget::Bytes(10 * MIB),
        min_lines: 100,
        ..Default::default()
    };
    let mem = MemorySnapshot {
        total_bytes: Some(16 * GIB),
        available_bytes: Some(GIB),
    };
    let pressured = ResolvedPolicy::resolve(&cfg, mem);
    assert!(pressured.memory_pressure);
    let mid = PaneUsage {
        used_rows: 3_000,
        capacity: 4_000,
        cols: 80,
    };
    let tiny = PaneUsage {
        used_rows: 900,
        capacity: 1_000,
        cols: 80,
    };
    let planned = plan_capacities(&[mid, mid, mid, tiny], &pressured);
    let quarter = (10 * MIB / 4 / row_bytes(80)) as usize;
    assert_eq!(
        planned,
        vec![quarter, quarter, quarter, 1_000],
        "no growth under pressure"
    );
    let p = policy(1, true);
    assert_eq!(plan_capacities(&[mid], &p), vec![100]);
}

#[test]
fn parses_proc_meminfo() {
    let snap = parse_meminfo(
        "MemTotal:       16384000 kB\nMemFree: 1 kB\nMemAvailable:    8192000 kB\n",
    );
    assert_eq!(snap.total_bytes, Some(16_384_000 * 1024));
    assert_eq!(snap.available_bytes, Some(8_192_000 * 1024));
    assert_eq!(parse_meminfo("garbage"), MemorySnapshot::default());
}
