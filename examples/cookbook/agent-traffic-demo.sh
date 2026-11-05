#!/usr/bin/env bash
# Generate a short burst of traffic between 3 panes so the Navigator and the
# `:agents zoom` window have something to show.
#
# - Starts a private headless bilterm daemon (own socket, own state dir), so it
#   never touches your real sessions.
# - Splits it into 3 panes and starts an `agy` agent in each (set AGY=0 to run
#   plain shells instead, no model calls).
# - For DURATION seconds, panes "talk": pane.exec / pane.send_keys / :mcp
#   broadcast calls go from one pane to another, with pair frequencies skewed
#   (A<->B busy, B<->C occasional, A<->C rare) so edge brightness differs.
# - Prints `:agents list` and `:agents topology` at the end (topology needs
#   the agents-graph PR #8 build).
#
# Usage: examples/cookbook/agent-traffic-demo.sh [DURATION_SECS] [--keep]
#   --keep  leave the daemon running so you can attach a UI to the socket
#   BILTERM=/path/to/bilterm  AGY=0|1  AGY_PROMPT="..."
set -euo pipefail

DURATION="${1:-20}"; KEEP=0; [[ "${2:-}" == "--keep" || "${1:-}" == "--keep" ]] && KEEP=1
[[ "$DURATION" == "--keep" ]] && DURATION=20
BILTERM="${BILTERM:-bilterm}"
AGY="${AGY:-1}"
AGY_PROMPT="${AGY_PROMPT:-Reply with the single word: ready}"

DIR="$(mktemp -d)"; SOCK="$DIR/rpc.sock"
export XDG_STATE_HOME="$DIR/state" XDG_RUNTIME_DIR="$DIR/run" XDG_CONFIG_HOME="$DIR/cfg"
mkdir -p "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"; chmod 700 "$DIR" "$XDG_RUNTIME_DIR"

ctl() { "$BILTERM" ctl "unix:$SOCK" "$@"; }
cmd() { ctl wm.command "{\"command\": \"$1\"}" >/dev/null; }
exec_in() { ctl pane.exec "{\"command\": \"$2\", \"pane_id\": $1, \"allow_echo_lock\": true}" >/dev/null; }

cleanup() { [[ $KEEP == 1 ]] && { echo "daemon left running; socket: unix:$SOCK  (pid $DPID)"; return; }
            kill "$DPID" 2>/dev/null || true; rm -rf "$DIR"; }
trap cleanup EXIT

"$BILTERM" --headless -l "unix:$SOCK" >"$DIR/daemon.log" 2>&1 & DPID=$!
for _ in $(seq 1 50); do [[ -S "$SOCK" ]] && break; sleep 0.1; done
[[ -S "$SOCK" ]] || { echo "daemon did not start; see $DIR/daemon.log" >&2; KEEP=1; exit 1; }

cmd ":split v"; cmd ":split h"
IDS=($(ctl workspace.status | python3 -c 'import sys,json; print(*[p["id"] for p in json.load(sys.stdin)["result"]["panes"]][:3])'))
(( ${#IDS[@]} >= 3 )) || { echo "expected 3 panes, got ${#IDS[@]}" >&2; exit 1; }
A=${IDS[0]}; B=${IDS[1]}; C=${IDS[2]}
echo "panes: A=$A B=$B C=$C"

if [[ "$AGY" == "1" ]]; then
  for p in $A $B $C; do exec_in "$p" "agy --print --print-timeout 60s '$AGY_PROMPT'"; done
else
  for p in $A $B $C; do exec_in "$p" "echo agent-$p ready"; done
fi

say() { exec_in "$2" "echo '[$1 -> $2] $3'"; }   # from-pane label, target pane
end=$((SECONDS + DURATION)); n=0
while (( SECONDS < end )); do
  n=$((n+1))
  say $A $B "ping $n"; say $B $A "pong $n"                 # busy pair
  (( n % 3 == 0 )) && say $B $C "relay $n"                 # occasional pair
  (( n % 8 == 0 )) && say $A $C "rare $n"                  # rare pair
  (( n % 5 == 0 )) && cmd ":mcp broadcast sync $n"
  sleep 0.5
done

echo "--- :agents list"; ctl wm.command '{"command": ":agents list"}'
echo "--- :agents topology (needs a build with PR #8; skipped otherwise)"
ctl wm.command '{"command": ":agents topology"}' || true
