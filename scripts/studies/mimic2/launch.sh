#!/bin/bash
# The caller launches this script with setsid nohup; all sim invocations are long.
set -eu
SOURCE=$1
OUT=$2
DATA=$3
RUNNER=$4
WAIT_FOR=$5
mkdir -p "$OUT/runs" "$DATA"
finish() {
    rc=$?
    trap - EXIT
    set +e
    timeout 270 python3 "$SOURCE/scripts/studies/mimic2/table.py" "$OUT/runs" > "$OUT/table.txt" 2> "$OUT/table.err"
    table_rc=$?
    if (( table_rc != 0 )); then
        printf 'table exit %s\n' "$table_rc" >> "$OUT/FAILED"
        if (( rc == 0 )); then rc=$table_rc; fi
    fi
    if (( rc != 0 )); then printf 'batch exit %s; see batch/run.log and runs/errors\n' "$rc" >> "$OUT/FAILED"; fi
    # Delete only this job's disposable binary, foundings and checkpoints after all
    # foreground children return. Readouts and staged status/FAILED remain in OUT.
    timeout 270 python3 - "$DATA" "$OUT" <<'PY_CLEAN'
import shutil
import sys
from pathlib import Path
root = Path(sys.argv[1])
for p in root.iterdir():
    if p.is_dir():
        shutil.rmtree(p)
    else:
        p.unlink()
out = Path(sys.argv[2])
for p in [out / "found", *[d / "ckpt" for d in (out / "runs").iterdir() if d.is_dir()]]:
    if p.is_symlink():
        p.unlink()
PY_CLEAN
    printf '%s\n' "$rc" > "$OUT/DONE"
    exit "$rc"
}
trap finish EXIT
trap 'exit 143' TERM HUP
trap 'exit 130' INT
# Stage status lives in OUT; only snapshots and the frozen executable live on disk.
mkdir -p "$DATA/found"
ln -s "$DATA/found" "$OUT/found"
for seed in 1 2 3 4; do
    for arm in main mimic both; do
        mkdir -p "$OUT/runs/$arm-s$seed" "$DATA/$arm-s$seed/ckpt"
        ln -s "$DATA/$arm-s$seed/ckpt" "$OUT/runs/$arm-s$seed/ckpt"
    done
done
timeout 270 cp "$SOURCE/target/release/examples/hunter_study" "$DATA/hunter_study"
timeout 270 python3 "$SOURCE/scripts/studies/mimic2/table.py" "$OUT/runs" > "$OUT/table.txt"
# Long timeout covers each founding and each six-hour sim inside this detached job.
python3 "$SOURCE/scripts/studies/mimic2/run_batches.py" --root "$OUT" --source "$SOURCE" --binary "$DATA/hunter_study" --runner "$RUNNER" --wait-for "$WAIT_FOR" --timeout-s 30000
