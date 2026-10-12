#!/usr/bin/env bash
# BALANCE: run every arm of an engagement config with one probe, N arms at a time, then
# reduce. Usage: balance-run.sh CONFIG PROBE OUT THREADS
# Writes OUT/<arm>.jsonl, OUT/summary.tsv and OUT/DONE.
set -euo pipefail
config=$1 probe=$2 out=$3 threads=$4
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
cp "$config" "$out/config.toml"
python3 -c 'import sys,tomllib; [print(a["name"]) for a in tomllib.load(open(sys.argv[1],"rb"))["arms"]]' "$config" \
  | xargs -P "$threads" -I{} sh -c '"$1" config="$2" arm="$3" seed=1 > "$4/$3.jsonl.tmp" && mv "$4/$3.jsonl.tmp" "$4/$3.jsonl"' _ "$probe" "$config" {} "$out"
python3 "$here/balance-reduce.py" "$out" > "$out/summary.tsv"
date > "$out/DONE"
