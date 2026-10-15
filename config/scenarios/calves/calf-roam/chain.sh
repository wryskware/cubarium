#!/usr/bin/env bash
# CALF-ROAM chain (design/handoffs/calf-roam-2026-10-15.md): for each saves job (a seed pair),
# wait until it ends, then write the four arms' specs from arm-*.toml.in against its m240
# checkpoints and submit them. labd has no job dependency, so this waits for it; it runs no
# simulation itself. Log, submitted job ids and DONE land in OUT.
#
# First it waits for the two preflight quick jobs (base and arm c) and submits nothing unless
# preflight-check.py shows the knobs and the calves' census differ.
#
#   systemd-run --user --unit calf-roam-chain --collect --working-directory=WORKTREE \
#     config/scenarios/calves/calf-roam/chain.sh OUT PRE_BASE_JOB PRE_C_JOB SEEDS=SAVES_JOB ...
set -u
out=$1 pre_base=$2 pre_c=$3
shift 3
lab=/home/wrysk/wryskware/cubarium/scripts/lab/lab
here=config/scenarios/calves/calf-roam
mkdir -p "$out/specs"
runs=/home/wrysk/wryskware/cubarium/runs/lab
"$lab" wait --job "$pre_base"; "$lab" wait --job "$pre_c"
if ! python3 -I "$here/preflight-check.py" "$runs/$pre_base" "$runs/$pre_c" >>"$out/chain.log" 2>&1; then
  echo "preflight failed: arms not submitted $(date -Is)" | tee -a "$out/chain.log" >"$out/DONE"
  exit 1
fi
pair() {
  local seeds=${1%%=*} saves=${1#*=}
  "$lab" wait --job "$saves"
  local rc=$?
  echo "$(date -Is) saves $saves (seeds $seeds) ended rc=$rc" >>"$out/chain.log"
  if ! ls "$runs/$saves"/batch-task/batch/settle/s*/run/ckpt/m000240 >/dev/null 2>&1; then
    echo "$(date -Is) seeds $seeds: no m240 checkpoints under $saves; arms not submitted" >>"$out/chain.log"
    return
  fi
  for arm in base a b c; do
    spec="$out/specs/arm-$arm-s$seeds.toml"
    sed -e "s/@SAVES@/$saves/g" -e "s/@SEEDS@/$seeds/g" "$here/arm-$arm.toml.in" >"$spec"
    id=$("$lab" submit "$spec" 2>>"$out/chain.log")
    echo "$(date -Is) arm $arm seeds $seeds: ${id:-SUBMIT FAILED} ($spec)" >>"$out/chain.log"
    [ -n "$id" ] && echo "$id" >>"$out/arm-jobs.txt"
  done
}
for p in "$@"; do pair "$p" & done
wait
echo "chain done $(date -Is)" >"$out/DONE"
