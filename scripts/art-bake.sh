#!/usr/bin/env bash
# Import artist edits, then bake the actual AnimationPlayer poses without a GPU.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
"$repo_dir/scripts/godot.sh" --headless --editor --import
exec "$repo_dir/scripts/godot.sh" --headless --script bake.gd -- "$@"
