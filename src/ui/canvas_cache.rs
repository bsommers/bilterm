//! Per-pane cache of the canvas base layer ([`crate::ui::canvas_layer`]) and the switch that turns
//! it on, off, or into a self-checking mode.
//!
//! A pane whose grid and layer inputs are unchanged since the last frame is not recomposed: the
//! stored cells are replayed into the frame buffer. The stored layer lives in the pane's
//! [`VtParser`] ([`crate::vt::derived::DerivedSlot`]), so it is dropped with the pane and is never
//! shared between two parsers.
//!
//! # What invalidates a stored layer
//! [`LayerKey`] lists every input. From the grid: `VtParser::generation` (bumped by every
//! `process`, resize and rebuild), the scrollback offset (scrolling does not bump the generation)
//! and the grid size. From the caller: every field of [`CanvasLayerInputs`]. State drawn *after*
//! the layer (graphics, search matches, selection, copy mode, hints, cursor and its blink, prompt,
//! status bar, focus chrome) is not cached at all and so cannot go stale.
//!
//! # Modes
//! `BILTERM_RENDER_CACHE=on|off|verify` picks the mode for the process; the default is `verify`
//! in debug builds (so every test that renders a pane checks the cache against the direct path)
//! and `on` in release builds. [`with_render_cache_mode`] overrides it for the current thread.

use std::cell::Cell as StdCell;
use std::sync::{Arc, OnceLock};

use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;

use crate::ui::canvas_layer::{CanvasLayerInputs, paint_canvas_layer};
use crate::ui::theme::{ThemePalette, ThemePreset};
use crate::vt::VtParser;

/// How the canvas base layer is produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderCacheMode {
    /// Replay the stored layer when its key matches; otherwise paint and (on the second sighting
    /// of a key) store.
    On,
    /// Always paint directly. The reference behaviour.
    Off,
    /// Paint directly and also through the cache; panic if the two differ, or if a stored layer
    /// differs from a fresh one with the same key (an under-invalidated key).
    Verify,
}

thread_local! {
    static MODE_OVERRIDE: StdCell<Option<RenderCacheMode>> = const { StdCell::new(None) };
}

fn process_default_mode() -> RenderCacheMode {
    static DEFAULT: OnceLock<RenderCacheMode> = OnceLock::new();
    *DEFAULT.get_or_init(|| {
        match std::env::var("BILTERM_RENDER_CACHE").ok().as_deref().map(str::trim) {
            Some("on" | "1") => RenderCacheMode::On,
            Some("off" | "0") => RenderCacheMode::Off,
            Some("verify") => RenderCacheMode::Verify,
            _ if cfg!(debug_assertions) => RenderCacheMode::Verify,
            _ => RenderCacheMode::On,
        }
    })
}

/// The mode in effect on this thread.
pub fn render_cache_mode() -> RenderCacheMode {
    MODE_OVERRIDE.with(StdCell::get).unwrap_or_else(process_default_mode)
}

/// Runs `f` with the render cache in `mode` on the current thread, restoring the previous mode
/// afterwards (also when `f` panics).
pub fn with_render_cache_mode<R>(mode: RenderCacheMode, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<RenderCacheMode>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MODE_OVERRIDE.with(|m| m.set(self.0));
        }
    }
    let _restore = Restore(MODE_OVERRIDE.with(|m| m.replace(Some(mode))));
    f()
}

/// Stored layers kept per pane (the workspace view plus one other view of the same pane).
const MAX_LAYERS_PER_PANE: usize = 2;
/// Canvases larger than this many cells are painted directly and never stored.
const MAX_CACHED_CELLS: usize = 200_000;

/// Every input of a stored layer. See the module documentation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LayerKey {
    vt_generation: u64,
    scrollback_offset: usize,
    vt_size: (u16, u16),
    canvas: Rect,
    theme: ThemePreset,
    palette: ThemePalette,
    font_bold: bool,
    font_italic: bool,
    font_double_width: bool,
    triggers: Vec<String>,
    crt_scanlines: bool,
}

impl LayerKey {
    fn capture(vt: &VtParser, inputs: &CanvasLayerInputs<'_>) -> Self {
        // Exhaustive: adding a field to `CanvasLayerInputs` fails to compile here and in `matches`.
        let CanvasLayerInputs {
            canvas,
            theme,
            palette,
            font_bold,
            font_italic,
            font_double_width,
            triggers,
            crt_scanlines,
        } = *inputs;
        Self {
            vt_generation: vt.generation(),
            scrollback_offset: vt.scrollback_offset(),
            vt_size: vt.size(),
            canvas,
            theme,
            palette: palette.clone(),
            font_bold,
            font_italic,
            font_double_width,
            triggers: triggers.to_vec(),
            crt_scanlines,
        }
    }

    /// Allocation-free equivalent of `*self == LayerKey::capture(vt, inputs)`.
    fn matches(&self, vt: &VtParser, inputs: &CanvasLayerInputs<'_>) -> bool {
        let CanvasLayerInputs {
            canvas,
            theme,
            palette,
            font_bold,
            font_italic,
            font_double_width,
            triggers,
            crt_scanlines,
        } = *inputs;
        self.vt_generation == vt.generation()
            && self.scrollback_offset == vt.scrollback_offset()
            && self.vt_size == vt.size()
            && self.canvas == canvas
            && self.theme == theme
            && self.font_bold == font_bold
            && self.font_italic == font_italic
            && self.font_double_width == font_double_width
            && self.crt_scanlines == crt_scanlines
            && self.triggers.as_slice() == triggers
            && self.palette == *palette
    }
}

/// The value parked in the pane's `DerivedSlot`: at most [`MAX_LAYERS_PER_PANE`] stored layers and
/// as many keys that were painted once but not stored yet.
#[derive(Default)]
struct PaneLayerCache {
    layers: Vec<(LayerKey, Arc<Vec<Cell>>)>,
    seen_once: Vec<LayerKey>,
}

enum Lookup {
    Hit(Arc<Vec<Cell>>),
    /// Second consecutive sighting of this key: worth storing.
    Store,
    /// First sighting: paint directly. A pane that changes every frame stays on this path and
    /// pays nothing for the cache.
    PaintDirect,
}

impl PaneLayerCache {
    fn find(&self, vt: &VtParser, inputs: &CanvasLayerInputs<'_>) -> Option<Arc<Vec<Cell>>> {
        self.layers.iter().find(|(key, _)| key.matches(vt, inputs)).map(|(_, cells)| Arc::clone(cells))
    }

    fn lookup(&mut self, vt: &VtParser, inputs: &CanvasLayerInputs<'_>) -> Lookup {
        if let Some(cells) = self.find(vt, inputs) {
            return Lookup::Hit(cells);
        }
        if self.seen_once.iter().any(|key| key.matches(vt, inputs)) {
            return Lookup::Store;
        }
        self.seen_once.insert(0, LayerKey::capture(vt, inputs));
        self.seen_once.truncate(MAX_LAYERS_PER_PANE);
        Lookup::PaintDirect
    }

    fn store(&mut self, key: LayerKey, cells: Arc<Vec<Cell>>) {
        self.seen_once.retain(|seen| *seen != key);
        self.layers.retain(|(stored, _)| *stored != key);
        self.layers.insert(0, (key, cells));
        self.layers.truncate(MAX_LAYERS_PER_PANE);
    }
}

fn cacheable(canvas: Rect) -> bool {
    let cells = usize::from(canvas.width) * usize::from(canvas.height);
    cells > 0 && cells <= MAX_CACHED_CELLS
}

/// Paints the layer into a private buffer covering exactly `inputs.canvas`.
fn paint_to_cells(vt: &VtParser, inputs: &CanvasLayerInputs<'_>) -> Vec<Cell> {
    let mut scratch = Buffer::empty(inputs.canvas);
    paint_canvas_layer(vt, inputs, &mut scratch);
    scratch.content
}

/// Replays stored layer `cells` (row-major over `canvas`) into `buf`, with the same effect on each
/// destination cell as [`paint_canvas_layer`]: symbol, foreground and background are replaced and
/// modifiers are added; `skip` and the underline colour are left alone.
fn replay(cells: &[Cell], canvas: Rect, buf: &mut Buffer) {
    let clip = canvas.intersection(buf.area);
    if clip.width == 0 || clip.height == 0 {
        return;
    }
    let stride = usize::from(canvas.width);
    let width = usize::from(clip.width);
    for y in clip.y..clip.y + clip.height {
        let src_start = usize::from(y - canvas.y) * stride + usize::from(clip.x - canvas.x);
        let dst_start = buf.index_of(clip.x, y);
        let src_row = &cells[src_start..src_start + width];
        let dst_row = &mut buf.content[dst_start..dst_start + width];
        for (dst, src) in dst_row.iter_mut().zip(src_row) {
            dst.set_symbol(src.symbol());
            dst.fg = src.fg;
            dst.bg = src.bg;
            dst.modifier.insert(src.modifier);
        }
    }
}

/// Paints the canvas base layer of `vt` into `buf`, through the per-pane cache when the current
/// [`RenderCacheMode`] allows it. The result is identical to [`paint_canvas_layer`] in every mode.
pub fn paint_canvas_layer_cached(vt: &VtParser, inputs: &CanvasLayerInputs<'_>, buf: &mut Buffer) {
    let mode = render_cache_mode();
    if mode == RenderCacheMode::Off || !cacheable(inputs.canvas) {
        paint_canvas_layer(vt, inputs, buf);
        return;
    }
    if mode == RenderCacheMode::Verify {
        paint_verified(vt, inputs, buf);
        return;
    }
    match vt.derived().with(|cache: &mut PaneLayerCache| cache.lookup(vt, inputs)) {
        Lookup::Hit(cells) => replay(&cells, inputs.canvas, buf),
        Lookup::PaintDirect => paint_canvas_layer(vt, inputs, buf),
        Lookup::Store => {
            let cells = Arc::new(paint_to_cells(vt, inputs));
            replay(&cells, inputs.canvas, buf);
            let key = LayerKey::capture(vt, inputs);
            vt.derived().with(|cache: &mut PaneLayerCache| cache.store(key, cells));
        }
    }
}

fn region_cells(buf: &Buffer, region: Rect) -> Vec<Cell> {
    let mut out = Vec::with_capacity(usize::from(region.width) * usize::from(region.height));
    for y in region.y..region.y + region.height {
        let start = buf.index_of(region.x, y);
        out.extend_from_slice(&buf.content[start..start + usize::from(region.width)]);
    }
    out
}

fn restore_region(buf: &mut Buffer, region: Rect, cells: &[Cell]) {
    let width = usize::from(region.width);
    for (row, y) in (region.y..region.y + region.height).enumerate() {
        let start = buf.index_of(region.x, y);
        buf.content[start..start + width].clone_from_slice(&cells[row * width..(row + 1) * width]);
    }
}

fn first_difference(a: &[Cell], b: &[Cell], region: Rect) -> Option<String> {
    let width = usize::from(region.width.max(1));
    a.iter().zip(b).position(|(x, y)| x != y).map(|i| {
        format!(
            "first difference at column {} row {} of the canvas: cached {:?}, direct {:?}",
            i % width,
            i / width,
            a[i],
            b[i]
        )
    })
}

/// `Verify` mode: leaves `buf` exactly as the direct path paints it, after checking that
/// 1. a stored layer with a matching key equals a freshly painted one (the key covers every input);
/// 2. replaying a layer over the current buffer content equals painting directly over it.
fn paint_verified(vt: &VtParser, inputs: &CanvasLayerInputs<'_>, buf: &mut Buffer) {
    let fresh = Arc::new(paint_to_cells(vt, inputs));
    let stored = vt.derived().with(|cache: &mut PaneLayerCache| cache.find(vt, inputs));
    if let Some(stored) = stored
        && let Some(diff) = first_difference(&stored, &fresh, inputs.canvas)
    {
        panic!(
            "render cache: a stored canvas layer is stale although its key matches (an input is \
             missing from the key). {diff}. Run with BILTERM_RENDER_CACHE=off to work around."
        );
    }
    let key = LayerKey::capture(vt, inputs);
    vt.derived().with(|cache: &mut PaneLayerCache| cache.store(key, Arc::clone(&fresh)));

    let clip = inputs.canvas.intersection(buf.area);
    if clip.width == 0 || clip.height == 0 {
        return;
    }
    let before = region_cells(buf, clip);
    replay(&fresh, inputs.canvas, buf);
    let replayed = region_cells(buf, clip);
    restore_region(buf, clip, &before);
    paint_canvas_layer(vt, inputs, buf);
    let direct = region_cells(buf, clip);
    if let Some(diff) = first_difference(&replayed, &direct, clip) {
        panic!(
            "render cache: replaying the canvas layer differs from painting it directly. {diff}. \
             Run with BILTERM_RENDER_CACHE=off to work around."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BiltermConfig;
    use crate::pane::CompositePane;
    use ratatui::style::{Color, Modifier, Style};

    fn pane_with_output() -> CompositePane {
        let mut pane = CompositePane::new_headless(1, 12, 50);
        pane.process_pty_output(
            b"\x1b[1;32mgreen\x1b[0m plain \x1b[7minverse\x1b[0m https://example.org/x\r\n\
              \x1b[48;5;22m\x1b[38;5;120m+ added line\x1b[0m \xe6\x97\xa5\xe6\x9c\xac ERROR\r\n",
        );
        pane
    }

    fn paint(mode: RenderCacheMode, pane: &CompositePane, cfg: &BiltermConfig, canvas: Rect, buf: &mut Buffer) {
        let palette = cfg.palette();
        let inputs = CanvasLayerInputs::gather(pane, canvas, cfg, &palette);
        with_render_cache_mode(mode, || paint_canvas_layer_cached(pane.vt(), &inputs, buf));
    }

    /// A frame buffer whose cells already carry modifiers, colours and `skip`, as left behind by
    /// whatever was drawn underneath the pane.
    fn dirty_buffer(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        for (i, cell) in buf.content.iter_mut().enumerate() {
            cell.set_symbol("x");
            cell.set_style(Style::default().fg(Color::Red).bg(Color::Blue).add_modifier(Modifier::ITALIC | Modifier::DIM));
            cell.skip = i % 7 == 0;
        }
        buf
    }

    #[test]
    fn third_identical_render_is_a_cache_hit_and_equals_the_direct_path() {
        let pane = pane_with_output();
        let cfg = BiltermConfig::default();
        let area = Rect::new(0, 0, 60, 14);
        let canvas = Rect::new(3, 1, 50, 11);

        let mut reference = dirty_buffer(area);
        paint(RenderCacheMode::Off, &pane, &cfg, canvas, &mut reference);
        for round in 0..4 {
            let mut buf = dirty_buffer(area);
            paint(RenderCacheMode::On, &pane, &cfg, canvas, &mut buf);
            assert_eq!(buf, reference, "round {round}");
        }
        let stored = pane.vt().derived().with(|c: &mut PaneLayerCache| c.layers.len());
        assert_eq!(stored, 1, "first sighting paints directly, second stores, later ones hit");
    }

    #[test]
    fn canvas_partly_outside_the_buffer_is_clipped_like_the_direct_path() {
        let pane = pane_with_output();
        let cfg = BiltermConfig::default();
        let area = Rect::new(2, 2, 30, 6);
        let canvas = Rect::new(0, 0, 50, 11);
        let mut reference = dirty_buffer(area);
        paint(RenderCacheMode::Off, &pane, &cfg, canvas, &mut reference);
        for _ in 0..3 {
            let mut buf = dirty_buffer(area);
            paint(RenderCacheMode::On, &pane, &cfg, canvas, &mut buf);
            assert_eq!(buf, reference);
        }
        let mut buf = dirty_buffer(area);
        paint(RenderCacheMode::Verify, &pane, &cfg, canvas, &mut buf);
        assert_eq!(buf, reference);
    }

    #[test]
    fn every_key_input_invalidates_the_stored_layer() {
        let mut pane = pane_with_output();
        let mut cfg = BiltermConfig::default();
        let area = Rect::new(0, 0, 60, 14);
        let mut canvas = Rect::new(0, 0, 50, 11);

        // After each change the cached render must equal a direct render of the new state and
        // differ from the frame before the change.
        let mut previous: Option<Buffer> = None;
        let mut check = |label: &str, pane: &CompositePane, cfg: &BiltermConfig, canvas: Rect| {
            let mut reference = Buffer::empty(area);
            paint(RenderCacheMode::Off, pane, cfg, canvas, &mut reference);
            for _ in 0..3 {
                let mut buf = Buffer::empty(area);
                paint(RenderCacheMode::On, pane, cfg, canvas, &mut buf);
                assert_eq!(buf, reference, "{label}: cached frame differs from direct frame");
            }
            if let Some(prev) = &previous {
                assert_ne!(*prev, reference, "{label}: the change must be visible");
            }
            previous = Some(reference);
        };

        check("initial", &pane, &cfg, canvas);
        pane.process_pty_output(b"new output line\r\n");
        check("vt generation", &pane, &cfg, canvas);
        for i in 0..40 {
            pane.process_pty_output(format!("scroll filler {i}\r\n").as_bytes());
        }
        check("more output", &pane, &cfg, canvas);
        pane.vt_mut().scroll_up(5);
        check("scrollback offset", &pane, &cfg, canvas);
        pane.vt_mut().reset_scrollback();
        check("scrollback reset", &pane, &cfg, canvas);
        cfg.theme = ThemePreset::Ibm3278Green;
        check("theme", &pane, &cfg, canvas);
        cfg.triggers = vec!["filler".to_string()];
        check("triggers", &pane, &cfg, canvas);
        cfg.crt_scanlines = true;
        check("crt scanlines", &pane, &cfg, canvas);
        canvas = Rect::new(4, 2, 40, 9);
        check("canvas rect", &pane, &cfg, canvas);
        pane.vt_mut().set_size(9, 40);
        check("vt size", &pane, &cfg, canvas);
    }

    #[test]
    fn a_pane_that_changes_every_frame_never_stores_a_layer() {
        let mut pane = pane_with_output();
        let cfg = BiltermConfig::default();
        let area = Rect::new(0, 0, 50, 11);
        for i in 0..6 {
            pane.process_pty_output(format!("tick {i}\r\n").as_bytes());
            let mut buf = Buffer::empty(area);
            paint(RenderCacheMode::On, &pane, &cfg, area, &mut buf);
        }
        let (layers, seen) = pane.vt().derived().with(|c: &mut PaneLayerCache| (c.layers.len(), c.seen_once.len()));
        assert_eq!(layers, 0);
        assert!(seen <= MAX_LAYERS_PER_PANE);
    }

    #[test]
    fn stored_layers_are_bounded_per_pane() {
        let pane = pane_with_output();
        let cfg = BiltermConfig::default();
        let area = Rect::new(0, 0, 60, 14);
        for w in 20..30 {
            for _ in 0..3 {
                let mut buf = Buffer::empty(area);
                paint(RenderCacheMode::On, &pane, &cfg, Rect::new(0, 0, w, 10), &mut buf);
            }
        }
        let (layers, seen) = pane.vt().derived().with(|c: &mut PaneLayerCache| (c.layers.len(), c.seen_once.len()));
        assert!(layers <= MAX_LAYERS_PER_PANE && seen <= MAX_LAYERS_PER_PANE);
        assert!(!cacheable(Rect::new(0, 0, 1000, 1000)), "oversized canvases are never stored");
    }

    #[test]
    #[should_panic(expected = "stale although its key matches")]
    fn verify_mode_catches_a_stale_stored_layer() {
        let pane = pane_with_output();
        let cfg = BiltermConfig::default();
        let area = Rect::new(0, 0, 50, 11);
        let mut buf = Buffer::empty(area);
        paint(RenderCacheMode::Verify, &pane, &cfg, area, &mut buf);
        // Corrupt the stored layer behind the key's back, as a missed invalidation would.
        pane.vt().derived().with(|c: &mut PaneLayerCache| {
            let mut cells = (*c.layers[0].1).clone();
            cells[0].set_symbol("!");
            c.layers[0].1 = Arc::new(cells);
        });
        paint(RenderCacheMode::Verify, &pane, &cfg, area, &mut buf);
    }
}
