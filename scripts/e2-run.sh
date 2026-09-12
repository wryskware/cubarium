#!/usr/bin/env bash
#
# scripts/e2-run.sh <batch-name> <matrix.toml> [-j N]
#
# Runs the E2 matrix defined by <matrix.toml> as one headless world per row, per
# design/experiments-e2-harness.md. Each row gets its own directory under
# runs/<batch-name>/ holding the full config that produced it, the world state,
# the telemetry, the run log, and a manifest:
#
#   runs/<batch>/<row>/config.toml      the base config merged with this row's axis values
#   runs/<batch>/<row>/state/           snapshots and journal (--state)
#   runs/<batch>/<row>/telemetry.jsonl  telemetry samples (--telemetry)
#   runs/<batch>/<row>/run.log          stdout+stderr of the run
#   runs/<batch>/<row>/manifest.json    row name, axis values, seed, build id,
#                                       wall seconds, exit code
#
# The release binary is built once up front and then invoked directly, so the
# per-row cost is the world and nothing else. `runs/` is git-ignored.
#
# Options:
#   -j N   run N rows in parallel (default: the number of cores)
#
# Exits nonzero if any row's run exited nonzero; a failing row never stops the
# rest of the batch.

set -euo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
REPO="$(cd "$(dirname "$SELF")/.." && pwd)"
MATRIX_PY="$(dirname "$SELF")/e2-matrix.py"

# ---------------------------------------------------------------------------
# Worker: run one row. Re-entry point for xargs; not part of the public CLI.
# ---------------------------------------------------------------------------
if [[ "${1:-}" == "--run-row" ]]; then
    row="$2"
    row_dir="$E2_BATCH_DIR/$row"
    mkdir -p "$row_dir/state"

    started=$(date +%s.%N)
    set +e
    "$E2_BIN" run \
        --sink none \
        --speed 0 \
        --fresh \
        --seconds "$E2_SIM_SECONDS" \
        --config "$row_dir/config.toml" \
        --state "$row_dir/state" \
        --telemetry "$row_dir/telemetry.jsonl" \
        >"$row_dir/run.log" 2>&1
    code=$?
    set -e
    finished=$(date +%s.%N)

    python3 - "$row_dir/manifest.json" "$E2_BUILD_ID" "$started" "$finished" "$code" <<'PY'
import json, sys
path, build_id, started, finished, code = sys.argv[1:6]
with open(path) as fh:
    manifest = json.load(fh)
manifest["build_id"] = build_id
manifest["wall_seconds"] = round(float(finished) - float(started), 3)
manifest["exit_code"] = int(code)
with open(path, "w") as fh:
    json.dump(manifest, fh, indent=2)
    fh.write("\n")
PY

    if [[ "$code" -ne 0 ]]; then
        echo "e2-run: row $row FAILED (exit $code); see $row_dir/run.log" >&2
    else
        echo "e2-run: row $row done"
    fi
    # Always succeed: a bad row is recorded in its manifest, not fatal to the batch.
    exit 0
fi

# ---------------------------------------------------------------------------
# Driver.
# ---------------------------------------------------------------------------
usage() {
    echo "usage: $(basename "$SELF") <batch-name> <matrix.toml> [-j N]" >&2
    exit 2
}

jobs=""
positional=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        -j)
            [[ $# -ge 2 ]] || usage
            jobs="$2"
            shift 2
            ;;
        -j*)
            jobs="${1#-j}"
            shift
            ;;
        -h|--help)
            usage
            ;;
        --)
            shift
            positional+=("$@")
            break
            ;;
        -*)
            echo "e2-run: unknown option $1" >&2
            usage
            ;;
        *)
            positional+=("$1")
            shift
            ;;
    esac
done

[[ ${#positional[@]} -eq 2 ]] || usage
batch="${positional[0]}"
matrix="${positional[1]}"

[[ -f "$matrix" ]] || { echo "e2-run: no such matrix file: $matrix" >&2; exit 1; }
matrix="$(cd "$(dirname "$matrix")" && pwd)/$(basename "$matrix")"

if [[ -z "$jobs" ]]; then
    jobs="$( (nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1) )"
fi
[[ "$jobs" =~ ^[0-9]+$ && "$jobs" -ge 1 ]] || { echo "e2-run: -j needs a positive integer" >&2; exit 2; }

cd "$REPO"

# One release build for the whole batch; the runs then invoke the binary directly.
# The default target directory is used so a user ends up with one release binary,
# but an existing CARGO_TARGET_DIR in the environment is honoured.
echo "e2-run: building cubarium (release)" >&2
cargo build --release -p cubarium
bin="${CARGO_TARGET_DIR:-$REPO/target}/release/cubarium"
[[ -x "$bin" ]] || { echo "e2-run: no release binary at $bin" >&2; exit 1; }

# Build id: the binary's own version if it reports one, else the git short hash.
if build_id="$("$bin" --version 2>/dev/null)" && [[ -n "$build_id" ]]; then
    build_id="$(echo "$build_id" | head -n 1)"
else
    build_id="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)"
fi

batch_dir="$REPO/runs/$batch"
mkdir -p "$batch_dir"

sim_seconds="$(python3 "$MATRIX_PY" seconds "$matrix")"
mapfile -t rows < <(python3 "$MATRIX_PY" expand "$matrix" "$batch_dir" --batch "$batch")
[[ ${#rows[@]} -gt 0 ]] || { echo "e2-run: the matrix expanded to no rows" >&2; exit 1; }

echo "e2-run: batch $batch — ${#rows[@]} rows × ${sim_seconds}s simulated, -j $jobs, build $build_id" >&2

export E2_BATCH_DIR="$batch_dir"
export E2_BIN="$bin"
export E2_SIM_SECONDS="$sim_seconds"
export E2_BUILD_ID="$build_id"

started=$(date +%s.%N)
printf '%s\n' "${rows[@]}" | xargs -r -P "$jobs" -I{} "$SELF" --run-row {}
finished=$(date +%s.%N)

python3 - "$batch_dir" "$started" "$finished" <<'PY'
import json, pathlib, sys
batch_dir = pathlib.Path(sys.argv[1])
elapsed = float(sys.argv[3]) - float(sys.argv[2])
meta = json.loads((batch_dir / "batch.json").read_text())
failed = []
for row in meta["rows"]:
    manifest = json.loads((batch_dir / row / "manifest.json").read_text())
    if manifest.get("exit_code") != 0:
        failed.append((row, manifest.get("exit_code")))
meta["wall_seconds"] = round(elapsed, 3)
meta["failed_rows"] = [r for r, _ in failed]
(batch_dir / "batch.json").write_text(json.dumps(meta, indent=2) + "\n")
print(f"e2-run: {len(meta['rows']) - len(failed)}/{len(meta['rows'])} rows succeeded "
      f"in {elapsed:.1f} s wall", file=sys.stderr)
for row, code in failed:
    print(f"e2-run:   FAILED {row} (exit {code})", file=sys.stderr)
sys.exit(1 if failed else 0)
PY

echo "e2-run: analyze with scripts/e2-analyze.py runs/$batch" >&2
