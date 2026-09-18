#!/usr/bin/env bash
# Open the voxel world on the local screen: build this checkout, then run the strip
# through the GPU cutaway renderer into a development window. The seeded example
# habitat is on by default (--empty asks for the bare world); extra arguments are
# forwarded to `cubarium voxel`.
#
# The CPU presenter's loopback viewer is the fallback where no GPU window is wanted:
#   target/release/cubarium voxel --sink web --web-port 7393
# and it shows the same world at http://127.0.0.1:7393/.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"
CARGO_TARGET_DIR="$repo_dir/target" cargo build --release --locked -p cubarium --bin cubarium
exec "$repo_dir/target/release/cubarium" voxel \
  --sink gpu --gpu-target window \
  "$@"