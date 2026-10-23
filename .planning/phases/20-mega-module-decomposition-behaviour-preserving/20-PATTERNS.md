# Phase 20: Mega-Module Decomposition - Pattern Map

**Mapped:** 2026-10-23
**Files analyzed:** 32 (7 decomposed parent modules, 22 new child modules, 3 updated test files)

## Decomposition Pattern Rules

1. **Child Module Visibility & Split `impl` Pattern:**
   - When splitting methods of a struct (`VtParser`, `AutomationControlPlane`, `WindowManager`) across child modules (`vt/scan.rs`, `api/rpc.rs`, `wm/session.rs`), keep the struct definition in the parent `mod.rs` (or `rpc.rs` with `pub(super)` fields where sibling modules access them) and place `impl StructName { ... }` blocks in the child modules.
   - Because child modules are descendants of `mod.rs`, private fields on structs defined in `mod.rs` remain accessible to all child modules without widening public visibility.
   - Where sibling submodules need access to fields of a struct defined in a sibling submodule (e.g. `AutomationControlPlane` in `src/api/rpc.rs` accessed by `src/api/mcp.rs` and `src/api/playbook.rs`), mark internal fields `pub(super)` (or keep existing `pub` fields unchanged until Phase 21 encapsulates invariants).

2. **Re-Export Preservation at Module Root (`mod.rs`):**
   - `src/vt/mod.rs`: `pub use codec::*; pub use detect::*; pub use graphics::*;` (Note: `cell_to_ratatui_style` moves to `crate::ui::cell_to_ratatui_style`).
   - `src/pane/mod.rs`: `pub use modes::*; pub use prompt_editor::*;` (Note: `colon_command` moves to `crate::colon_command`).
   - `src/input/mod.rs`: `pub use encode::*; pub use keymap::*; pub use mouse::*;`
   - `src/api/mod.rs`: `pub use json::*; pub use mcp::*; pub use playbook::*; pub use rpc::*; pub use server::*;`
   - `src/ui/mod.rs`: `pub use status_bar::{render_oia_status_bar, render_oia_status_bar_themed};`

3. **Test Module Extraction Pattern (`#[cfg(test)] #[path = "..."] mod tests;`):**
   - For files whose code + inline `mod tests` slightly exceed 800 lines (`src/vt/scrollback.rs` at 834 lines, `src/pane/history_store.rs` at 850 lines), use `#[cfg(test)] #[path = "scrollback_tests.rs"] mod tests;` (or `#[cfg(test)] mod tests;` in `mod.rs` pointing to `tests.rs`). This keeps the exact unit test path (`vt::scrollback::tests::*`, `pane::history_store::tests::*`) identical for `cargo test --lib` filters while keeping every `.rs` file well under 800 lines.
