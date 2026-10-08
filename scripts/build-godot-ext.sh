#!/usr/bin/env bash
# Build the Godot extension (crates/cubarium-godot) and put it where game/cubarium.gdextension
# points: game/bin/libcubarium_godot.{so,dylib}. Then register the extension with the project
# (game/.godot/extension_list.cfg), which the editor would otherwise write on first import,
# so `godot --headless --path game` loads it without ever opening the editor.
#
#   scripts/build-godot-ext.sh            # debug build
#   scripts/build-godot-ext.sh --release  # release build (what the ambient piece runs)
set -euo pipefail
cd "$(dirname "$0")/.."
profile=debug
args=()
if [[ "${1:-}" == "--release" ]]; then
    profile=release
    args+=(--release)
fi
cargo build -p cubarium-godot "${args[@]}"
target="${CARGO_TARGET_DIR:-target}"
case "$(uname -s)" in
    Darwin) lib=libcubarium_godot.dylib ;;
    *) lib=libcubarium_godot.so ;;
esac
mkdir -p game/bin game/.godot
cp -f "$target/$profile/$lib" "game/bin/$lib"
grep -qx 'res://cubarium.gdextension' game/.godot/extension_list.cfg 2>/dev/null \
    || echo 'res://cubarium.gdextension' >> game/.godot/extension_list.cfg
echo "built game/bin/$lib ($profile)"
