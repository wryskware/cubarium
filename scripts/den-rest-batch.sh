#!/usr/bin/env bash
# Launch once: setsid nohup bash scripts/den-rest-batch.sh ABS_OUTPUT SLOT >ABS_OUTPUT/batch.log 2>&1 </dev/null &
# All four seeds run consecutively under one long timeout. Finalize even on errors.
set -u
batch_output=$1
batch_slot=${2:-1}
finish() {
    if python3 scripts/den-rest-batch.py summarize "$batch_output"; then
        printf '%s\n' "$batch_exit" > "$batch_output/exit"
        touch "$batch_output/DONE"
    else
        printf '%s\n' "Final table failed; DONE was not written." >&2
    fi
}
batch_exit=1
trap finish EXIT
timeout --kill-after=60 14400 python3 scripts/den-rest-batch.py run "$batch_output" --slot "$batch_slot"
batch_exit=$?
exit "$batch_exit"
