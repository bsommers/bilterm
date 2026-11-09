//! Terminal traffic metrics layer (Phase 64, `TMON-01`, `TMON-02`).
//!
//! Leaf layer: this module tree imports nothing from the crate except itself (std only), so
//! `pty`, `vt`, `tmux`, `pane`, `wm`, `api`, ... may all depend on it without creating a cycle
//! (`tests/phase64_layer_dag.rs` enforces this).
//!
//! Model: each I/O boundary owns an `Arc<ChannelCounters>` (a [`TrafficTap`]) bumped with relaxed
//! atomics (one relaxed load and a branch when disabled). A cold [`TrafficMeter`] registry adopts
//! the cells, diffs them once per second into 60-bucket rings (clock injected via
//! `sample(now_ms)`), and produces immutable [`TrafficSnapshot`]s that the pure
//! [`group::group_by`] reduces by connection, origin, locality, session, window, pane or
//! direction. Only counts and sanitised labels are stored, never payload bytes.

pub mod cell;
pub mod classify;
pub mod fixture;
pub mod group;
pub mod meter;
pub mod origin;
pub mod window;

pub use cell::{
    CellReading, ChannelCounters, F_CLASSIFY, F_ENABLED, F_OIA, TrafficTap, set_default_flags,
};
pub use group::{
    AttributionMap, Filter, GroupKey, GroupResult, GroupRow, SortKey, SortSpec, group_by,
};
pub use meter::{
    ChannelMeta, ChannelRow, Totals, TrafficConfigView, TrafficMeter, TrafficSnapshot,
};
pub use origin::{ConnectionId, ConnectionKey, Locality, Origin, Role, Surface};
pub use window::{Bucket, Dir, Rates, Ring};

/// Maximum simultaneously registered channels (live + closed), including the overflow channel.
pub const MAX_CHANNELS: usize = 256;
/// Maximum retained closed channels before the oldest roll into [`Totals`].
pub const MAX_CLOSED: usize = 64;
/// Ring length in one-second buckets.
pub const BUCKETS: usize = 60;
/// Maximum label length in characters.
pub const LABEL_CAP: usize = 48;
/// Default snapshot row limit.
pub const SNAPSHOT_LIMIT_DEFAULT: usize = 100;
/// Hard snapshot row limit.
pub const SNAPSHOT_LIMIT_MAX: usize = 500;
/// Maximum filter text length in characters.
pub const FILTER_CAP: usize = 64;
