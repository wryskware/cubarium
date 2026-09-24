#!/usr/bin/env bash
# Open the voxel world on the local screen: build this checkout, then run the strip
# through the GPU cutaway renderer into a development window. The seeded example
# habitat is on by default (--empty asks for the bare world); extra arguments are
# forwarded to `cubarium voxel`.
#
# Agent runs are hidden: under an agent (CLAUDECODE=1 or CODEX_*), or with `--hidden`,
# `--background` or `--float` (or CUBARIUM_FLOAT=1), the window goes through
# scripts/hidden.sh onto Hyprland's hidden `special:agents` workspace and never reaches
# the screen. A person's own run tiles normally.
#
# The CPU presenter's loopback viewer is the fallback where no GPU window is wanted:
#   target/release/cubarium voxel --sink web --web-port 7393
# and it shows the same world at http://127.0.0.1:7393/.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"
CARGO_TARGET_DIR="$repo_dir/target" cargo build --release --locked -p cubarium --bin cubarium

hidden=0
[ "${CUBARIUM_FLOAT:-${CUBARIUM_BACKGROUND:-0}}" = 1 ] && hidden=1
[ "${CLAUDECODE:-}" = 1 ] && hidden=1
env | grep -q '^CODEX_' && hidden=1
[ "${CUBARIUM_HIDDEN:-}" = 0 ] && hidden=0
args=()
for arg in "$@"; do
  case "$arg" in
    --hidden|--float|--background)
      hidden=1
      ;;
    *)
      args+=("$arg")
      ;;
  esac
done

cmd=("$repo_dir/target/release/cubarium" voxel --sink gpu --gpu-target window "${args[@]}")
if [ "$hidden" = 1 ]; then
  exec "$repo_dir/scripts/hidden.sh" "${cmd[@]}"
fi
exec "${cmd[@]}"
