# Cookbook

| Script | What it does |
|---|---|
| `agent-traffic-demo.sh [secs] [--keep]` | Starts a private headless daemon, splits it into 3 panes, starts `agy` in each (`AGY=0` for plain shells), and generates skewed pane-to-pane traffic for a few seconds. Prints `:agents list` / `:agents topology`. `--keep` leaves the daemon running. |

Notes:
- Traffic is `pane.exec`, `:mcp broadcast` and similar calls. Until Phase 51 every edge goes through the `api` actor (star-shaped graph); pane-to-pane agent messaging comes later.
- `:agents topology` needs the build that includes the agents-graph PR (#8).
- Uses an isolated `XDG_*` dir, so your saved sessions are untouched.
