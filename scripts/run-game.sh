#!/usr/bin/env bash
# Run the Cubarium app (game/) on the first GPU the Vulkan driver lists, the one the
# renderer and the display use, rather than Godot's own pick (it takes the most powerful
# GPU, which on a two-GPU desktop is not the one the screen is on, so every frame would
# cross between GPUs). The world's renderer follows whatever GPU Godot is on.
# CUBARIUM_GODOT_GPU overrides the index. Extra arguments go to Godot.
# Agents' hidden captures on Xvfb (scripts/hidden.sh without --hyprland): the 5080 cannot
# present there, so use CUBARIUM_GODOT_GPU=1 (the 5090); --hyprland works on the default.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
godot_bin=${GODOT_BIN:-$(command -v godot || command -v godot4)}
args=(--path "$repo_dir/game" --gpu-index "${CUBARIUM_GODOT_GPU:-0}")
# Under Wayland ask for it directly: Godot tries X11 first, and through XWayland the
# display's GPU reports no present queue, so it logs an error before falling back.
[[ -n "${WAYLAND_DISPLAY:-}" ]] && args+=(--display-driver wayland)
exec "$godot_bin" "${args[@]}" "$@"
