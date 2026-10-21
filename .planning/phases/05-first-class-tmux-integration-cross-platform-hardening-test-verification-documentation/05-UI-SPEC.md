# Phase 5: First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation - UI Design Specification

**Phase:** 5
**UI Hint:** yes

## 1. Outer `$TMUX` & `tmux -CC` OIA Status Bar Telemetry (`TMUX-01`, `TMUX-02`)
- When outer `$TMUX` is detected or a `CompositePane` is attached to a `tmux -CC` `%pane_id`, the OIA status bar displays a `[TMUX]` or `[%0]` badge so the operator knows session persistence and `DCS tmux;` passthrough are active.
- Outbound `OSC 52` sequences are automatically wrapped via `wrap_tmux_dcs_passthrough(seq)` when `TmuxEnv::detect().is_inside_tmux` is `true`.
