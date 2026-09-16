---
design_status: exploration
last_reviewed: 2026-09-16
---

# W1 (Opus, high): `cube-screen-shim`, the Tachyon display daemon

Read [the plan](tachyon-screen-plan-2026-09-16.md) first; it carries the device
facts, the architecture decisions and the normative layout spec. This brief is
the work order. Fresh context. No nested agents.

## Objective

A daemon that owns the Tachyon's DisplayPort output at the KMS level, accepts
`cube_proto` frames over UDP, and paints them on the panel as an unfolded net
(default) or a fixed-camera ray-cast cube. Built natively on the Tachyon,
installed as a root systemd service, verified with a test pattern on the panel
when the panel is visible to the kernel.

## Where you work

- Repo: `/home/wrysk/vuzic/led-cube-shim`, **worktree**
  `/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`, branch
  `tachyon-screen`, based on `main` at `5afbcda`. Work only in the worktree.
  Do not touch the main checkout (it has unrelated uncommitted work).
- Device: `ssh root@tachyon-8968c731.local` (key auth works, no password).
  You may install rustup there, copy sources there, build there, install the
  binary and the unit there, and restart the service there. That is the whole
  grant. Do not apt-upgrade, change networking, or touch anything unrelated.
- Read before writing: `docs/ARCHITECTURE.md`, `crates/cube-kms/src/lib.rs`
  and `card.rs`, `crates/led-cube-shim/src/{run,display,ingest,idle,config,
  patterns,limiter}.rs`, `crates/cube-map/src/mapper.rs` (the LUT pattern to
  mirror). Use `graft skeleton`/`graft ask` in that repo; it is indexed.
- The ray-cast to port: `/home/wrysk/wryskware/cubarium/crates/cubarium/src/raycast.rs`
  (read-only; copy the needed ~190 lines into the new crate with a comment
  naming the source, and keep its round-trip test).

## Deliverable

1. New crate `crates/cube-screen-shim` (binary `cube-screen-shim`), added to
   the workspace `members`. Depends on `cube-proto` (serde feature),
   `cube-kms`, and the workspace deps. It does **not** depend on `cube-map`
   unless you find a clean reuse of `ColorCorrection`.
2. Subcommands, mirroring the cube daemon: `run`, `outputs`,
   `test-pattern <faces|grid|solid:RRGGBB>`, plus `layout` that prints the
   computed placement (panel mode, scale, origin, viewport) without opening
   the display, for debugging over ssh.
3. Config at `/etc/cube-screen-shim/config.toml` (also `--config`), every
   key optional, unknown keys warned. Sections and defaults:
   ```toml
   [output]  connector = "DP-1"          # card auto-scanned; optional card = "/dev/dri/card0"
   [layout]  mode = "net"                # "net" | "cube"
             scale = "auto"              # or an integer ≥ 1
             gap = 1                     # source pixels between net cells
             cube_yaw_deg = 45.0
             cube_pitch_deg = 22.0
             cube_distance = 5.0
             cube_fov_deg = 38.0
   [color]   brightness = 1.0, gamma = 1.0, white_balance = [1.0, 1.0, 1.0]
   [ingest]  bind = "0.0.0.0:7392"
   [idle]    after_secs = 3.0, fade_secs = 1.0, screen = "black"   # "black" | "logo"
   ```
   Ship the example as `config/cube-screen-shim.toml` and the unit as
   `config/cube-screen-shim.service` (system unit, `User=root`,
   `After=network-online.target`, `Restart=always`, `RestartSec=2`,
   `ExecStart=/usr/local/bin/cube-screen-shim run`, `Environment=RUST_LOG=info`).
4. A `Layout`/`Mapper` type: built once from (panel W×H, config) into a LUT
   of `u32` source offsets (or `EMPTY`), with `source_of(px, py)` and
   `render(frame, dst, stride)` / `render_scaled`. Implement the two modes
   exactly as the plan's layout spec says. Nearest-neighbour. Background
   black.
5. Run loop: ingest thread (latest-frame-wins, stale `seq` dropped, counters
   for received/stale/bytes) and a present thread paced by `present()`
   (vsync). Idle fade to black after `after_secs`, `fade_secs` long. Log a
   line every 5 s: presented fps, received, stale, idle phase. If the output
   is lost (`KmsError::Lost`) or absent at start, retry opening every second
   forever; the service must come up before the panel is plugged in and
   attach when it appears.
6. Docs: a section in `docs/ARCHITECTURE.md` (or a new `docs/SCREEN.md`
   linked from it) with the contract, the Tachyon facts from the plan, the
   build-on-device procedure, and the install steps. Keep the cube's
   sections untouched.

## Constraints and decisions already made

- `cube_proto` wire format is unchanged. Do not add a new payload format.
- `cube-kms` changes are allowed but must be backward compatible; the LED
  cube daemon's tests stay green (`cargo test --workspace` in the worktree).
  Expected needs: connector `DP-1` by name works already; full-screen
  region; the driver is `msm_drm` on `/dev/dri/card0`. If the legacy page
  flip with event fails on this driver, fall back (atomic commit or
  `drmModeSetCrtc` + vblank wait) behind the same `present()` API and say
  so in the report.
- Dumb-buffer memory is write-combined: write rows sequentially, never read
  from the mapped buffer. Fill `s×s` blocks per source pixel or run-length
  the LUT; measure, and report the render time per frame on the device at
  the panel's native mode. Target ≤ 8 ms/frame at 1080p. Only redraw when a
  new frame arrived or the idle fade is changing.
- Never write to `/sys/class/drm/card0-DP-1/status` (a reboot followed the
  last time). Reading sysfs is fine.
- If `DP-1` stays `disconnected` (check `modetest -M msm_drm -c`) you
  cannot verify on the panel. Finish everything else, verify the KMS path
  as far as `outputs` reporting `DP-1` disconnected, and report that the
  panel was never seen. You may try **once**: `echo on >
  /sys/kernel/debug/dri/0/DP-1/force` and re-check; if the board reboots or
  the connector still shows no modes, stop there and report. Do not iterate
  on kernel/USB-C bring-up; that is Wrysk's physical setup, not yours.
- Native build on the device: `curl https://sh.rustup.rs -sSf | sh -s -- -y
  --profile minimal` as root, then `rsync -a --exclude target --exclude
  .git <worktree>/ root@tachyon-8968c731.local:/root/led-cube-shim/` and
  `cargo build --release -p cube-screen-shim`. Install to `/usr/local/bin`,
  unit to `/etc/systemd/system`, `systemctl daemon-reload && systemctl
  enable --now cube-screen-shim`. Record the exact commands in the docs.
- No new heavy dependencies. `drm`, `serde`, `toml`, `clap`, `log`,
  `env_logger`, `anyhow`, `thiserror`, `ctrlc`, `png` are the ceiling.
- Commit on the `tachyon-screen` branch in the worktree, small logical
  commits, message body explains why. Do not merge to `main`.

## Decision authority

Yours: module structure, whether to extract shared modules from
`led-cube-shim` into a small lib crate (only as a pure move with its tests
green) or to adapt copies (say which and why), the LUT representation, the
render strategy, log wording. Not yours: the wire format, the layout spec,
the config key names above, anything in the cubarium repo.

## Verification you owe

- `cargo test --workspace` and `cargo clippy --workspace` in the worktree
  (desktop, x86_64) clean; `cargo build --release -p cube-screen-shim` on the
  Tachyon succeeds.
- `cube-screen-shim layout` output for 1920×1080 in both modes (net must
  give scale 7, origin `(53, 88)` with gap 1 (net 259×129 source px, 1813×903 panel px): check that arithmetic against
  the spec and report if the spec is wrong rather than fitting it silently).
- On the device: `outputs`; if the panel is connected, `test-pattern faces`
  and a description (or a photo Wrysk takes) of what is on the screen; the
  service enabled and running; `journalctl -u cube-screen-shim -n 20`.
- A loopback smoke test on the device: a tiny sender (the Python client
  `clients/python/cubeclient.py` is fine) pushing frames at 60 fps for 30 s;
  report received/stale/presented counters and CPU use (`top -b -n 1`).

## Return format

A report at `docs/reports/screen-shim-w1-2026-09-16.md` in the worktree and
the same text as your final message: what was built (file list), every
decision with its reason, the measurements above, what was verified on the
real panel versus only on the desktop, anything left undone and why, and the
exact commands to rebuild and redeploy. Evidence, not adjectives.
