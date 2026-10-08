#!/usr/bin/env bash
# Run the Cubarium app (game/) on the display's GPU and this host's run CPUs.
#
# GPU: Godot on the first GPU the Vulkan driver lists (`--gpu-index 0`, the display's on the
# desktop; CUBARIUM_GODOT_GPU overrides) through its Wayland driver where there is one (the
# project asks for it too; under X11 through XWayland Godot took the 5090). The world's
# renderer follows whatever GPU Godot is on. Agents' hidden captures on Xvfb
# (scripts/hidden.sh without --hyprland): the 5080 cannot present there, so use
# CUBARIUM_GODOT_GPU=1 (the 5090); --hyprland works on the default.
#
# CPUs: a person's run keeps to the CPUs this host's profile names, as scripts/run-voxel.sh
# does (RUN_CPUS in config/hosts/$(hostname).env: the V-cache chiplet on the desktop;
# CUBARIUM_CPUS overrides, CUBARIUM_CPUS= runs unpinned). Every thread Godot and the
# extension start inherits it. An agent's run (CLAUDECODE=1 or CODEX_*) inherits the
# agent's own CPUs.
#
# Extra arguments go to Godot.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
godot_bin=${GODOT_BIN:-$(command -v godot || command -v godot4)}
args=(--path "$repo_dir/game" --gpu-index "${CUBARIUM_GODOT_GPU:-0}")
[[ -n "${WAYLAND_DISPLAY:-}" ]] && args+=(--display-driver wayland)

agent=0
[ "${CLAUDECODE:-}" = 1 ] && agent=1
env | grep -q '^CODEX_' && agent=1
cpus=""
profile="$repo_dir/config/hosts/$(hostname).env"
if [ "$agent" = 0 ] && [ -f "$profile" ]; then
  cpus=$(. "$profile"; echo "${RUN_CPUS:-}")
fi
cpus=${CUBARIUM_CPUS-$cpus}

cmd=("$godot_bin" "${args[@]}" "$@")
[ -n "$cpus" ] && cmd=(taskset -c "$cpus" "${cmd[@]}")
exec "${cmd[@]}"
