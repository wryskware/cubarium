#!/usr/bin/env bash
# CALF-ROAM chain (design/handoffs/calf-roam-2026-10-15.md): for each saves job (a seed pair),
# wait until it ends, then write the four arms' specs from arm-*.toml.in against its m240
# checkpoints and submit them. labd has no job dependency, so this waits for it; it runs no
# simulation itself. Log, submitted job ids and DONE land in OUT.
#
#   systemd-run --user --unit calf-roam-chain --collect --working-directory=WORKTREE \
#     config/scenarios/calves/calf-roam/chain.sh OUT SEEDS=SAVES_JOB ...
# e.g. chain.sh runs/calf-roam/chain 1-2=1015-080850-calf-roam-saves-s1-2-6478 ...
set -u
out=$1
shift
lab=/home/wrysk/wryskware/cubarium/scripts/lab/lab
here=config/scenarios/calves/calf-roam
mkdir -p "$out/specs"
pair() {
  local seeds=${1%%=*} saves=${1#*=}
  "$lab" wait --job "$saves"
  local rc=$?
  echo "$(date -Is) saves $saves (seeds $seeds) ended rc=$rc" >>"$out/chain.log"
  if ! ls /home/wrysk/wryskware/cubarium/runs/lab/"$saves"/batch-task/batch/settle/s*/run/ckpt/m000240 >/dev/null 2>&1; then
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
