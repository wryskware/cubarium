#!/usr/bin/env bash
# labd launches this once. All suites, captures and matrices finish in one chain.
set -uo pipefail
PERF_OUTPUT=$1
shift
mkdir -p "$PERF_OUTPUT"
PERF_SCRIPT=$(cd -- "$(dirname -- "$0")" && pwd)/run.py
timeout --kill-after=30s 21600 python3 "$PERF_SCRIPT" --out "$PERF_OUTPUT" "$@"
PERF_RC=$?
if [ "$PERF_RC" -ne 0 ]; then
    echo "outer timeout/chain exit $PERF_RC; see stages.json and logs" > "$PERF_OUTPUT/FAIL"
fi
exit "$PERF_RC"
