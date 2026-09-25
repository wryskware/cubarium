#!/usr/bin/env bash
# Run the live world on this host, as its profile config/hosts/<host>.env says: the
# CPUs the process keeps to (RUN_CPUS, through taskset; the machine's placement lives
# here, not in the program), the arguments (RUN_ARGS) and the state directory (STATE).
# Extra arguments are forwarded to cubarium. A service's ExecStart calls this.
#
#     scripts/run-live.sh voidrunner
#     scripts/run-live.sh voidrunner --empty
#
# The window it opens is the host's own screen: never run this under an agent on the
# development machine (use scripts/run-voxel.sh, which hides it).
set -euo pipefail
host=${1:?usage: run-live.sh <host> [cubarium args...]}
shift
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"
# shellcheck source=/dev/null
. "config/hosts/$host.env"
mkdir -p "$STATE"
read -r -a args <<<"$RUN_ARGS"
cmd=("$repo_dir/target/release/cubarium" "${args[@]}" --state "$STATE" "$@")
if [ -n "${RUN_CPUS:-}" ]; then
  exec taskset -c "$RUN_CPUS" "${cmd[@]}"
fi
exec "${cmd[@]}"
