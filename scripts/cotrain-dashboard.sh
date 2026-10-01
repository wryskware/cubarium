#!/bin/bash
# Co-training dashboard loop: probes hosts every 30 s and syncs runs every 120 s, each time
# rebuilding runs/dashboard/index.html. Intervals: COTRAIN_HOSTS_INTERVAL / COTRAIN_RUNS_INTERVAL.
# First time: cp scripts/cotrain-dashboard.sources.example runs/dashboard/sources.txt
#             cp scripts/cotrain-dashboard.hosts.example   runs/dashboard/hosts.txt
# Read-only on every host (ssh cat/stat/ps/tail/grep/date, aws describe-*). Stop with Ctrl-C / kill.
cd "$(dirname "$0")/.." || exit 1
mkdir -p runs/dashboard
[ -f runs/dashboard/sources.txt ] || cp scripts/cotrain-dashboard.sources.example runs/dashboard/sources.txt
[ -f runs/dashboard/hosts.txt ] || cp scripts/cotrain-dashboard.hosts.example runs/dashboard/hosts.txt
trap 'kill $(jobs -p) 2>/dev/null; exit 0' INT TERM

loop() { # mode interval
  while true; do
    python3 scripts/cotrain_dashboard.py "$1" || echo "dashboard $1 cycle failed" >&2
    sleep "$2" &
    wait $!
  done
}
loop --hosts-only "${COTRAIN_HOSTS_INTERVAL:-30}" &
loop --runs-only "${COTRAIN_RUNS_INTERVAL:-120}" &
wait
