#!/usr/bin/env bash
# Launch the artist's editor or its headless baker from the repository root.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
godot_bin=${GODOT_BIN:-}
if [[ -z "$godot_bin" ]]; then
  for candidate in godot godot4; do
    if command -v "$candidate" >/dev/null 2>&1; then
      godot_bin=$(command -v "$candidate")
      break
    fi
  done
fi
if [[ -z "$godot_bin" && -x "$repo_dir/.tools/godot/godot" ]]; then
  godot_bin="$repo_dir/.tools/godot/godot"
fi
if [[ -z "$godot_bin" ]]; then
  echo 'Godot 4.6 is required. Install it or set GODOT_BIN to the editor executable.' >&2
  echo 'https://godotengine.org/download/archive/4.6.3-stable/' >&2
  exit 1
fi
cd -- "$repo_dir"
exec "$godot_bin" --path "$repo_dir/art" "$@"
