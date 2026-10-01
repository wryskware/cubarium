#!/bin/bash
# Sync the co-training runs listed in runs/dashboard/sources.txt and rebuild
# runs/dashboard/index.html every 120 s (COTRAIN_DASHBOARD_INTERVAL overrides).
# First time: cp scripts/cotrain-dashboard.sources.example runs/dashboard/sources.txt
# Read-only on remote hosts (ssh stat/cat). Stop with Ctrl-C.
cd "$(dirname "$0")/.." || exit 1
trap 'exit 0' INT TERM
mkdir -p runs/dashboard
[ -f runs/dashboard/sources.txt ] || cp scripts/cotrain-dashboard.sources.example runs/dashboard/sources.txt
while true; do
  python3 scripts/cotrain_dashboard.py || echo "dashboard cycle failed" >&2
  sleep "${COTRAIN_DASHBOARD_INTERVAL:-120}" &
  wait $!
done
