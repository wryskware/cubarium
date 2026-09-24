#!/usr/bin/env bash
# Run a command whose windows must never reach Wrysk's screen (AGENTS.md, "Windows").
#
#   scripts/hidden.sh COMMAND [ARGS...]             # default: a private virtual display
#   scripts/hidden.sh --hyprland COMMAND [ARGS...]  # the real compositor, hidden workspace
#
# Default: the command runs on its own Xvfb display with Wayland unset, so no number of
# windows it opens can appear anywhere. Its output, exit status and environment are this
# shell's. Use this for everything that doesn't need the real compositor.
#
# --hyprland: for the rare test that needs Hyprland itself (Wayland presentation, a
# hidden window's frame pacing). Hyprland launches the command with a rule that puts its
# first window on the hidden `special:agents` workspace, and a watchdog moves any later
# window of the command's process tree there the moment it maps. That's needed because
# a launch rule only reaches the first window, and title or class rules can't catch
# minifb (it maps before it sets a title and has no app id). Look with
# `hyprctl dispatch togglespecialworkspace agents`.
set -euo pipefail

mode=xvfb
if [ "${1:-}" = --hyprland ]; then
  mode=hyprland
  shift
fi
[ $# -gt 0 ] || { echo "usage: scripts/hidden.sh [--hyprland] COMMAND [ARGS...]" >&2; exit 2; }
export CUBARIUM_HIDDEN_LAUNCH=1

if [ "$mode" = xvfb ]; then
  exec env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u HYPRLAND_INSTANCE_SIGNATURE \
    XDG_SESSION_TYPE=x11 \
    xvfb-run -a -s "-screen 0 3840x2160x24" "$@"
fi

# The live Hyprland: the exported signature if its socket answers, otherwise the newest
# instance that answers. Every Hyprland restart leaves a stale directory behind.
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
  echo "scripts/hidden.sh: no live Hyprland for --hyprland; use the default mode." >&2
  exit 1
fi
export HYPRLAND_INSTANCE_SIGNATURE="$live"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/cubarium-hidden.XXXXXX")
token=$(basename "$tmp")
export CUBARIUM_HIDDEN_TOKEN="$token"
export -p > "$tmp/env"
{
  echo 'source "$1/env"'
  printf 'cd %q\n' "$PWD"
  printf '%q ' "$@"
  echo '> "$1/log" 2>&1 &'
  echo 'echo $! > "$1/pid"'
  echo 'wait $!; echo $? > "$1/status"'
} > "$tmp/run.sh"
: > "$tmp/log"

# The watchdog: every window that maps anywhere but special:agents and belongs to a
# process carrying this run's token in its environment is moved there at once.
python3 - "$runtime/$live/.socket2.sock" "$token" <<'PY' &
import json, os, socket, subprocess, sys

sock_path, token = sys.argv[1], sys.argv[2].encode()

def ours(pid):
    try:
        with open(f"/proc/{pid}/environ", "rb") as f:
            return b"CUBARIUM_HIDDEN_TOKEN=" + token in f.read().split(b"\0")
    except OSError:
        return False

s = socket.socket(socket.AF_UNIX)
s.connect(sock_path)
buf = b""
while True:
    data = s.recv(65536)
    if not data:
        break
    buf += data
    while b"\n" in buf:
        line, buf = buf.split(b"\n", 1)
        if not line.startswith(b"openwindow>>"):
            continue
        fields = line[len(b"openwindow>>"):].decode(errors="replace").split(",")
        addr, ws = "0x" + fields[0], fields[1] if len(fields) > 1 else ""
        if ws == "special:agents":
            continue
        clients = json.loads(subprocess.run(["hyprctl", "clients", "-j"],
                                            capture_output=True).stdout or b"[]")
        pid = next((c["pid"] for c in clients if c["address"] == addr), None)
        if pid and ours(pid):
            subprocess.run(["hyprctl", "dispatch",
                            f'hl.dsp.window.move({{ workspace = "special:agents", '
                            f'window = "address:{addr}", follow = false }})'],
                           capture_output=True)
PY
watchdog=$!

child=""
cleanup() {
  if [ -n "$child" ] && kill -0 "$child" 2>/dev/null; then
    kill -TERM "$child" 2>/dev/null || true
  fi
  kill "$watchdog" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

sleep 0.2
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
