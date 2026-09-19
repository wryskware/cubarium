#!/usr/bin/env bash
# Open the voxel world on the local screen: build this checkout, then run the strip
# through the GPU cutaway renderer into a development window. The seeded example
# habitat is on by default (--empty asks for the bare world); extra arguments are
# forwarded to `cubarium voxel`.
#
# Pass `--background` or `--float` (or set CUBARIUM_FLOAT=1) for automated runs:
# on Hyprland, this launches the window floating and unfocused in the top-right
# corner so it never re-tiles the user's workspace.
# Manual interactive runs tile normally by default.
#
# The CPU presenter's loopback viewer is the fallback where no GPU window is wanted:
#   target/release/cubarium voxel --sink web --web-port 7393
# and it shows the same world at http://127.0.0.1:7393/.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"
CARGO_TARGET_DIR="$repo_dir/target" cargo build --release --locked -p cubarium --bin cubarium

float_mode="${CUBARIUM_FLOAT:-${CUBARIUM_BACKGROUND:-0}}"
args=()
for arg in "$@"; do
  case "$arg" in
    --float|--background)
      float_mode=1
      ;;
    *)
      args+=("$arg")
      ;;
  esac
done

# Manual runs: execute directly in foreground (tiles normally)
if [ "$float_mode" != "1" ]; then
  exec "$repo_dir/target/release/cubarium" voxel \
    --sink gpu --gpu-target window \
    "${args[@]}"
fi

# Locate Hyprland instance signature if not exported in this shell
if [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
  sig=$(ls -1 "${XDG_RUNTIME_DIR:-/run/user/$UID}/hypr" 2>/dev/null | grep -E "^[0-9a-f_]+$" | head -n 1 || true)
  if [ -n "$sig" ]; then
    export HYPRLAND_INSTANCE_SIGNATURE="$sig"
  fi
fi

# If not running under Hyprland, fall back to direct execution
if [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] || ! command -v hyprctl >/dev/null 2>&1; then
  exec "$repo_dir/target/release/cubarium" voxel \
    --sink gpu --gpu-target window \
    "${args[@]}"
fi

# Register the same per-window rule used by the interactive Hyprland config,
# then run the process directly.  Keeping the child in this shell preserves its
# exit status and signal handling; the old detached wrapper could lose both.
hyprctl eval 'hl.window_rule({ name = "cubarium-agent-float", match = { initial_title = "^(cubarium)" }, float = true, no_initial_focus = true, size = { "640", "400" }, move = { "monitor_w - window_w - 24", "48" } })' >/dev/null

exec "$repo_dir/target/release/cubarium" voxel \
  --sink gpu --gpu-target window \
  "${args[@]}"
