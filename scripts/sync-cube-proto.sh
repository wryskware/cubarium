#!/usr/bin/env bash
# Refresh vendor/cube-proto from the local display-shim checkout.
# Usage: scripts/sync-cube-proto.sh [path-to-led-cube-shim]
set -euo pipefail
shim="${1:-$HOME/vuzic/led-cube-shim}"
root="$(cd "$(dirname "$0")/.." && pwd)"
src="$shim/crates/cube-proto"
dst="$root/vendor/cube-proto"
[ -d "$src" ] || { echo "no cube-proto at $src" >&2; exit 1; }
if [ -n "$(git -C "$shim" status --porcelain -- crates/cube-proto)" ]; then
  echo "refusing to vendor a dirty cube-proto checkout" >&2; exit 1
fi
rev="$(git -C "$shim" rev-parse HEAD)"
rm -rf "$dst/src" "$dst/tests"
cp -r "$src/src" "$dst/src"
cp -r "$src/tests" "$dst/tests"
rm -f "$dst/tests/python_interop.rs"
echo "$rev" > "$root/vendor/cube-proto.rev"
echo "vendored cube-proto at $rev; review $dst/Cargo.toml against $src/Cargo.toml by hand"
