#!/usr/bin/env bash
# Run a command whose windows must never reach Wrysk's screen (AGENTS.md, "Windows").
#
# Hyprland launches the command with a per-process rule, so every window it maps lands
# floating and unfocused on the hidden `special:agents` workspace from its first frame.
# Title and class rules can't do this for our windows: minifb maps its window before it
# sets a title, and sets no app id. The command keeps this shell's environment and working
# directory, its output streams here, its exit status is this script's, and an interrupt
# or timeout here kills it.
#
#   scripts/hidden.sh target/release/cubarium voxel --sink gpu --gpu-target window
#
# To look at what's there: `hyprctl dispatch togglespecialworkspace agents`.
set -euo pipefail

[ $# -gt 0 ] || { echo "usage: scripts/hidden.sh COMMAND [ARGS...]" >&2; exit 2; }

# The live Hyprland: the exported signature if its socket answers, otherwise the first
# instance under the runtime dir that answers. Stale instance directories are common
# (every Hyprland restart leaves one), so never take the first one listed.
runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/hypr"
live=""
for sig in "${HYPRLAND_INSTANCE_SIGNATURE:-}" $(ls -1t "$runtime" 2>/dev/null); do
  [ -n "$sig" ] || continue
  if HYPRLAND_INSTANCE_SIGNATURE="$sig" hyprctl version >/dev/null 2>&1; then
    live="$sig"
    break
  fi
done
if [ -z "$live" ]; then
  echo "scripts/hidden.sh: no live Hyprland; refusing to open a window it can't hide." >&2
  echo "Use a headless target instead (--gpu-target headless, --gpu-capture DIR)." >&2
  exit 1
fi
export HYPRLAND_INSTANCE_SIGNATURE="$live"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/cubarium-hidden.XXXXXX")
export -p > "$tmp/env"
{
  echo 'source "$1/env"'
  echo 'export CUBARIUM_HIDDEN_LAUNCH=1'
  printf 'cd %q\n' "$PWD"
  printf '%q ' "$@"
  echo '> "$1/log" 2>&1 &'
  echo 'echo $! > "$1/pid"'
  echo 'wait $!; echo $? > "$1/status"'
} > "$tmp/run.sh"
: > "$tmp/log"

child=""
cleanup() {
  if [ -n "$child" ] && kill -0 "$child" 2>/dev/null; then
    kill -TERM "$child" 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

hyprctl eval "hl.exec_cmd(\"bash $tmp/run.sh $tmp\", { workspace = \"special:agents silent\", float = true, no_initial_focus = true })" >/dev/null

for _ in $(seq 100); do
  [ -s "$tmp/pid" ] && break
  sleep 0.1
done
[ -s "$tmp/pid" ] || { echo "scripts/hidden.sh: Hyprland did not start the command" >&2; exit 1; }
child=$(cat "$tmp/pid")

tail -n +1 -f --pid="$child" "$tmp/log"
for _ in $(seq 50); do
  [ -s "$tmp/status" ] && break
  sleep 0.1
done
exit "$(cat "$tmp/status" 2>/dev/null || echo 1)"
