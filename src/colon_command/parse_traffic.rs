//! Parser for `:traffic <verb>` and `:netmon <verb>` (`TMON-03`, `TMON-04`, Phase 64-03).

use super::ColonCommand;
use crate::wm::traffic_monitor::TrafficMonitorOp as Op;
use crate::wm::traffic_monitor::RateCol;
use crate::wm::traffic_monitor::data::{parse_group, parse_locality, parse_sort};

pub const TRAFFIC_USAGE: &str = "Usage: :traffic [open|close|toggle|dismiss|blur|pin [toggle]|unpin|dock [toggle]|float|group <connection|session|window|pane|origin|locality|direction|next|prev>|sort <rate|rate10|rate1m|total|peak|name|next|prev|reverse> [asc|desc]|rate <1s|10s|1m>|filter <text>|clear-filter|locality <all|local|loopback|network|next>|pause [toggle]|resume|drill|up|select <n>|step <+n|-n>|page <+n|-n>|first|last|scroll <+n|-n>|show [n]|reset|enable|disable|classify <on|off|toggle>|oia <on|off|toggle>|scale <75..200>|move <dx> <dy>|resize <dw> <dh>|rect <x> <y> <w> <h>|size auto]";

fn usage<T>() -> Result<T, String> {
    Err(TRAFFIC_USAGE.to_string())
}

fn flag(args: &[&str]) -> Result<Option<bool>, String> {
    match args {
        [] | ["on" | "true" | "1"] => Ok(Some(true)),
        ["off" | "false" | "0"] => Ok(Some(false)),
        ["toggle"] => Ok(None),
        _ => usage(),
    }
}

fn num<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    s.trim_start_matches('+').parse::<T>().map_err(|_| TRAFFIC_USAGE.to_string())
}

/// Parses the arguments after `:traffic` / `:netmon`.
pub(super) fn parse(args: &[&str]) -> Result<ColonCommand, String> {
    let op = match args {
        [] | ["toggle"] => Op::Toggle,
        ["open" | "show-window"] => Op::Open,
        ["close" | "hide"] => Op::Close,
        ["dismiss"] => Op::Dismiss,
        ["blur"] => Op::Blur,
        ["pin", rest @ ..] => Op::Pin(flag(rest)?),
        ["unpin"] => Op::Pin(Some(false)),
        ["dock", rest @ ..] => Op::Dock(flag(rest)?),
        ["float" | "undock"] => Op::Dock(Some(false)),
        ["group", "next"] => Op::CycleGroup(1),
        ["group", "prev"] => Op::CycleGroup(-1),
        ["group", g] => Op::GroupBy(parse_group(g)?),
        ["sort", "next"] => Op::CycleSort(1),
        ["sort", "prev"] => Op::CycleSort(-1),
        ["sort", "reverse" | "flip"] => Op::ToggleSortDir,
        ["sort", k] => Op::Sort(parse_sort(k)?, None),
        ["sort", k, "asc"] => Op::Sort(parse_sort(k)?, Some(false)),
        ["sort", k, "desc"] => Op::Sort(parse_sort(k)?, Some(true)),
        ["rate", "1s" | "1"] => Op::RateCol(RateCol::R1),
        ["rate", "10s" | "10"] => Op::RateCol(RateCol::R10),
        ["rate", "1m" | "60"] => Op::RateCol(RateCol::R60),
        ["filter-focus", rest @ ..] => Op::FilterFocus(flag(rest)?.unwrap_or(true)),
        ["filter-char", "space"] => Op::FilterChar(' '),
        ["filter-char", c] if c.chars().count() == 1 => Op::FilterChar(c.chars().next().unwrap_or(' ')),
        ["filter-backspace"] => Op::FilterBackspace,
        ["filter", rest @ ..] if !rest.is_empty() => Op::Filter(rest.join(" ")),
        ["filter"] | ["clear-filter" | "unfilter"] => Op::FilterClear,
        ["locality", "next"] => Op::CycleLocality,
        ["locality", l] => Op::Locality(parse_locality(l)?),
        ["pause"] => Op::Pause(Some(true)),
        ["pause", "toggle"] => Op::Pause(None),
        ["resume"] => Op::Pause(Some(false)),
        ["drill"] => Op::Drill,
        ["up" | "drill-up"] => Op::DrillUp,
        ["select", n] => Op::SelectRow(num::<usize>(n)?.max(1) - 1),
        ["step", n] => Op::Step(num(n)?),
        ["page", n] => Op::Page(num(n)?),
        ["first"] => Op::First,
        ["last"] => Op::Last,
        ["scroll", n] => Op::Scroll(num(n)?),
        ["show"] => Op::Show(None),
        ["show", n] => Op::Show(Some(num(n)?)),
        ["reset"] => Op::Reset,
        ["enable", rest @ ..] => Op::Enable(flag(rest)?),
        ["disable"] => Op::Enable(Some(false)),
        ["classify", rest @ ..] => Op::Classify(flag(rest)?),
        ["oia", rest @ ..] => Op::Oia(flag(rest)?),
        ["scale", p] => {
            let p: u16 = num(p)?;
            if !(75..=200).contains(&p) {
                return usage();
            }
            Op::Scale(p)
        }
        ["move", dx, dy] => Op::Move { dx: num(dx)?, dy: num(dy)? },
        ["resize", dw, dh] => Op::Resize { dw: num(dw)?, dh: num(dh)? },
        ["rect", x, y, w, h] => Op::SetRect { x: num(x)?, y: num(y)?, w: num(w)?, h: num(h)? },
        ["size", "auto"] => Op::SizeAuto,
        _ => return usage(),
    };
    Ok(ColonCommand::TrafficMonitor(op))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::parse_colon_command;
    use crate::traffic::{GroupKey, Locality, SortKey};

    fn all_ops() -> Vec<Op> {
        let mut v = vec![
            Op::Open, Op::Close, Op::Toggle, Op::Dismiss, Op::Blur,
            Op::Pin(Some(true)), Op::Pin(Some(false)), Op::Pin(None),
            Op::Dock(Some(true)), Op::Dock(Some(false)), Op::Dock(None),
            Op::CycleGroup(1), Op::CycleGroup(-1), Op::CycleSort(1), Op::CycleSort(-1), Op::ToggleSortDir,
            Op::Sort(SortKey::Total, None), Op::Sort(SortKey::Name, Some(false)), Op::Sort(SortKey::Peak, Some(true)),
            Op::RateCol(RateCol::R1), Op::RateCol(RateCol::R10), Op::RateCol(RateCol::R60),
            Op::Filter("prod db".into()), Op::FilterClear, Op::FilterFocus(true), Op::FilterFocus(false),
            Op::FilterChar('a'), Op::FilterChar(' '), Op::FilterBackspace, Op::CycleLocality,
            Op::Pause(Some(true)), Op::Pause(Some(false)), Op::Pause(None), Op::Drill, Op::DrillUp,
            Op::SelectRow(0), Op::SelectRow(4), Op::Step(-1), Op::Step(2), Op::Page(1), Op::Page(-1),
            Op::First, Op::Last, Op::Scroll(-3), Op::Scroll(3), Op::Show(None), Op::Show(Some(5)), Op::Reset,
            Op::Enable(Some(true)), Op::Enable(Some(false)), Op::Enable(None),
            Op::Classify(Some(true)), Op::Classify(None), Op::Oia(Some(false)), Op::Oia(None),
            Op::Scale(150), Op::Move { dx: -2, dy: 1 }, Op::Resize { dw: 4, dh: -1 },
            Op::SetRect { x: 1, y: 2, w: 60, h: 20 }, Op::SizeAuto,
        ];
        for (_, g) in crate::wm::traffic_monitor::data::GROUP_NAMES {
            v.push(Op::GroupBy(*g));
        }
        for (_, l) in crate::wm::traffic_monitor::data::LOCALITY_NAMES {
            v.push(Op::Locality(*l));
        }
        v.push(Op::Locality(Some(Locality::Network)));
        v.push(Op::GroupBy(GroupKey::Pane));
        v
    }

    #[test]
    fn colon_form_round_trips_for_every_variant() {
        for op in all_ops() {
            let form = op.colon_form();
            let parsed = parse_colon_command(&form).unwrap_or_else(|| panic!("no parse: {form}"));
            assert_eq!(parsed, Ok(ColonCommand::TrafficMonitor(op.clone())), "{form}");
        }
    }

    #[test]
    fn alias_and_bad_forms() {
        assert_eq!(parse_colon_command(":traffic").unwrap(), Ok(ColonCommand::TrafficMonitor(Op::Toggle)));
        assert_eq!(parse_colon_command(":netmon").unwrap(), Ok(ColonCommand::TrafficMonitor(Op::Toggle)));
        for bad in [":traffic bogus", ":traffic scale 5", ":traffic group zzz", ":traffic move 1", ":traffic sort name up"] {
            assert!(parse_colon_command(bad).unwrap().is_err(), "{bad}");
        }
    }
}
