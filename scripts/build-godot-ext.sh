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
# Install by rename, never by copying over the old file. A running Godot maps
# game/bin/$lib; `cp` truncates that same inode and rewrites it, and the next page the
# process faults in from it is past the file's end for a moment, which is SIGBUS
# (2026-10-08: the app died founding candidates while an agent rebuilt the extension).
# A rename leaves the running process on the old inode and the next launch on the new one.
cp "$target/$profile/$lib" "game/bin/.$lib.new"
mv -f "game/bin/.$lib.new" "game/bin/$lib"
grep -qx 'res://cubarium.gdextension' game/.godot/extension_list.cfg 2>/dev/null \
    || echo 'res://cubarium.gdextension' >> game/.godot/extension_list.cfg
echo "built game/bin/$lib ($profile)"
# Import once, headless (no window), so Godot's global class cache knows `class_name` types
# such as WorldView on a project the editor has never opened.
godot_bin="${GODOT_BIN:-$(command -v godot || command -v godot4 || true)}"
if [[ -n "$godot_bin" ]]; then
    "$godot_bin" --headless --path game --import >/dev/null 2>&1 \
        && echo "imported game/ (class cache)" \
        || echo "warning: headless import of game/ failed; run: $godot_bin --headless --path game --import" >&2
fi
