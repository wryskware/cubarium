#!/usr/bin/env bash
set -euo pipefail
engage_tree=$(cd "$(dirname "$0")/../.." && pwd)
cd "$engage_tree"
timeout 270 cargo build --release -p cubarium --example engagement_probe
cd /home/wrysk/wryskware/cubarium
"$engage_tree/target/release/examples/engagement_probe" "config=$engage_tree/config/engagement.toml" arm=family-3 seed=1 episodes=1 video=/home/wrysk/wryskware/cubarium/art/gen/runs/engage-2026-10-12/family-3-seed1.mp4
ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,pix_fmt,width,height,r_frame_rate -show_entries format=duration -of json /home/wrysk/wryskware/cubarium/art/gen/runs/engage-2026-10-12/family-3-seed1.mp4
