#!/usr/bin/env bash
# Always build this checkout and use its normal assets. No frozen release copies.
# Additional arguments are forwarded to `cubarium run`; --fresh requires empty state.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_dir"
CARGO_TARGET_DIR="$repo_dir/target" cargo build --release --locked -p cubarium --bin cubarium
exec "$repo_dir/target/release/cubarium" run \
  --art "$repo_dir/assets/atelier" --state "$repo_dir/state" \
  --sink shim --mirror-web --web-port 7393 --fps 60 --speed 1 --care "$@"
