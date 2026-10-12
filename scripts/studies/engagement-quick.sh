#!/usr/bin/env bash
# A bounded lab quick check. Invoke through scripts/lab/lab quick -- timeout 270 ...
set -euo pipefail
cd "$(dirname "$0")/../.."
timeout 270 cargo build --release -p cubarium --example engagement_probe
timeout 270 cargo nextest run -p cubarium-voxel-sim -p cubarium-voxel-fauna
timeout 270 cargo run -p cubarium-voxel-sim --example params -- --write design/parameters.md
python3 scripts/studies/test_engagement.py
target/release/examples/engagement_probe config=config/engagement.toml arm=family-3 episodes=3 seconds=30
