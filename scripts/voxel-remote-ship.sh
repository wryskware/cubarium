#!/bin/sh
# Build `cubarium-search` for Zen 5 and ship it to the remote episode workers
# (`voxel-train --remote host:threads`, crates/cubarium-search/src/es/voxel/remote.rs).
#
#   scripts/voxel-remote-ship.sh [host ...]      default host: eidolon.local
#
# One `-C target-cpu=znver5` build runs on both machines (both are Zen 5), so a pair
# evaluated there executes the same instructions as one evaluated here. The worker checks
# the build and its flags at connect, so the coordinator must run this same binary:
#
#   target/znver5/release/cubarium-search voxel-train ... --remote eidolon.local:12
#
# Writes only ~/cubarium-train/cubarium-search on each host.
set -eu
cd "$(dirname "$0")/.."
target="$PWD/target/znver5"
CARGO_TARGET_DIR="$target" RUSTFLAGS="-C target-cpu=znver5" \
	cargo build --release -p cubarium-search -j "${JOBS:-16}"
bin="$target/release/cubarium-search"
[ $# -gt 0 ] || set -- eidolon.local
for host in "$@"; do
	ssh -o BatchMode=yes "$host" 'mkdir -p ~/cubarium-train'
	scp -q -o BatchMode=yes "$bin" "$host:cubarium-train/cubarium-search.new"
	ssh -o BatchMode=yes "$host" \
		'mv ~/cubarium-train/cubarium-search.new ~/cubarium-train/cubarium-search'
	echo "shipped $bin to $host:~/cubarium-train/cubarium-search"
done
echo "coordinator binary: $bin"
