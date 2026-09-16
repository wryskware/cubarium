---
design_status: leaning
last_reviewed: 2026-09-16
decision_refs: []
---

# Tachyon screen: the cube world on a flat HD display

Plan of record for phase 1. Written by Fable on 2026-09-16 after probing the
device and reading the LED cube shim. Workers get their own briefs (linked at
the end); this file is the shared contract they are held to.

## Goal

A standalone shelf piece: the Tachyon boots, brings up the panel, and runs the
Cubarium world on its own, with nothing else on the network. Same shape as the
LED cube: a daemon on the device owns the panel at the DRM/KMS level and accepts
`cube_proto` frames over UDP; cubarium runs on the same device as a second
service and sends to loopback. Wrysk stated this on 2026-09-16: no wifi
streaming from the desktop, cubarium itself runs on the device. Touch is out of
scope for phase 1.

## What the device is (measured 2026-09-16, do not re-derive)

| fact | value |
|---|---|
| host | `root@tachyon-8968c731.local`, wlan0 `192.168.68.68` |
| SoC / CPU | Qualcomm QCM6490, 6 usable cores (Cortex-A55 class), 7.3 GB RAM |
| OS | Ubuntu 20.04.6 headless, kernel 5.4.219 (Qualcomm downstream), glibc 2.31 |
| DRM | `/dev/dri/card0`, driver name `msm_drm`; `modetest -M msm_drm` works, plain `modetest` does not |
| connectors | `DP-1` (the only real output) and `Virtual-1` (forced off) |
| framebuffer | no `/dev/fb0`; no compositor running; `init_display.service` (Weston) is disabled and dead |
| toolchain | gcc, python3, strace present; no cargo/rustup |
| video path | **DisplayPort over USB-C**, not a native HDMI port. Hotplug comes through the PMIC/UCSI alt-mode path. A panel with an HDMI input is behind a USB-C to HDMI adapter |
| state at probe time | `DP-1` reported `disconnected`, no EDID, no modes, on two consecutive boots |
| touch | an I2C device `v2-touchscreen-pane` exists but registers no input node; there is no touch input device today (phase 2 problem) |

Two cautions from the probe:

- Writing `detect` to `/sys/class/drm/card0-DP-1/status` was immediately
  followed by a reboot of the board (00:52). Cause unproven, but no worker
  writes to connector `status` again. The debugfs `force`/`edid_override`
  knobs exist and are a documented last resort (see W1).
- The panel is not visible to the kernel right now. Until DP-1 reports
  `connected` with a mode list, nothing can be verified on the real screen.
  Wrysk needs to confirm the adapter is in the USB-C port that carries DP
  alt-mode and the panel is powered, ideally plugged in before boot.

## Architecture

```
 cubarium.service: cubarium run --sink shim --addr 127.0.0.1:7392   (on the Tachyon)
        │  cube_proto datagram: 5 × 64×64 RGB8 (61,456 B)
        ▼  UDP over loopback
 ┌─ cube-screen-shim.service (on the Tachyon, root, systemd) ─────────────┐
 │ ingest: UDP, latest-frame-wins, stale seq dropped     (as led-cube-shim) │
 │ layout: panel pixel → (face,x,y) LUT: "net" or "cube", integer scale     │
 │ kms:    DRM master on card0/DP-1, 2 XRGB8888 dumb buffers, vsync flips   │
 │ idle:   no frames → fade to black (logo optional)                        │
 └──────────────────────────────────────────────────────────────────────────┘
```

Decisions, with the reasoning:

1. **Reuse `cube_proto` unchanged as the wire format.** The screen is a
   different *layout* of the same five faces, exactly as the LED modules are
   a layout of them. Cubarium's `ShimSink` therefore needs no change; the
   on-device service points `--addr` at loopback. A future flat (non-cube) world
   would add a second payload format to the shim's ingest, not change this
   daemon's shape.
2. **The daemon lives in the `led-cube-shim` repo** as a new crate
   `crates/cube-screen-shim`, reusing `cube-proto` and `cube-kms`. Rationale:
   `cube-kms` already does DRM master, dumb buffers and paced page flips, and
   the ingest/idle/limiter logic is there to adapt. Cubarium stays a client.
3. **Layout is a precomputed LUT** (panel pixel → source face pixel or
   background), like `cube-map::Mapper`. Two modes in phase 1:
   - `net` (default): the unfolded cross, `Top` above `Front`, the row
     `Left Front Right Back`, integer nearest-neighbour upscale to the
     largest scale that fits, centred, black elsewhere.
   - `cube`: a fixed-camera ray-cast cube in a centred square viewport,
     ported from `cubarium/src/raycast.rs` (yaw 45°, pitch 22°, distance 5,
     FOV 38°), LUT traced once at startup.
   The same LUT is the inverse map touch will need in phase 2.
4. **Build natively on the Tachyon** (rustup, `cargo build --release`). No
   cross toolchain exists on the desktop and installing one is a system
   change. The board has 6 cores and 7 GB; a daemon-sized build is minutes.
5. **Run as a root system service.** Headless image, `/dev/dri/card0` is
   `root:video`, no logind session. Unit at
   `/etc/systemd/system/cube-screen-shim.service`, `Restart=always`,
   binary in `/usr/local/bin`.
6. **Bind ingest on `127.0.0.1:7392`.** Cubarium runs on the same board, so
   the shim never needs to listen on the network; the config can widen it for
   a desktop-driven test. There is no auth in `cube_proto` and none is added.
7. **No power limiter, gamma 1.0, brightness 1.0** by default. AMOLED needs
   none of the LED cube's electrical caps; keep `[color]` for taste.
8. **Fresh, never migrate** applies: nothing in cubarium's world schema
   changes.
9. **Cubarium is a second systemd service on the device** (`cubarium.service`,
   root, after `cube-screen-shim.service`), running `cubarium run --sink shim
   --addr 127.0.0.1:7392` with a persistent `--state` directory so a reboot
   resumes the same world (resume is an existing feature; a schema bump still
   refuses old worlds, per the standing rule). Build natively on the board
   first; set up cross-compilation only if native release builds prove too
   slow to iterate on, and say so with the measured build time.
10. **Frame rate and speed are measured, not assumed.** The board is six
   Cortex-A55 cores. W2 measures achieved render fps and sim tick rate and
   picks `--fps` (60 if it holds, else 30) and leaves `--speed` at 1 unless
   the sim cannot keep real time.

### Layout spec (normative, W3 tests against this)

Source is a `cube_proto::Frame`: faces `Front=0, Right=1, Back=2, Left=3,
Top=4`, each 64×64, `x` right, `y` down, in the client-facing convention of
`docs/ARCHITECTURE.md` (Top image's bottom edge adjoins Front's top edge).

`net`: a 4×2 grid of 64×64 source cells with a gap `g` source pixels
(default 1) between cells. Row 0: `[empty, Top, empty, empty]`. Row 1:
`[Left, Front, Right, Back]`. Net size in source pixels: `w = 4·64 + 3·g`,
`h = 2·64 + g`. Scale `s = max(1, min(⌊W/w⌋, ⌊H/h⌋))` for panel `W×H`
(config may pin `s`). Origin `(ox, oy) = (⌊(W − s·w)/2⌋, ⌊(H − s·h)/2⌋)`.
Panel pixel `(px, py)` maps to cell `(cx, cy)` and source
`(sx, sy) = (⌊(px − ox)/s⌋ − cx·(64+g), ⌊(py − oy)/s⌋ − cy·(64+g))` when
`0 ≤ sx, sy < 64` and the cell is populated; otherwise background. Faces are
not rotated or mirrored.

`cube`: viewport side `v = min(W, H)`, origin centred. For each viewport
pixel centre, cast the camera ray exactly as `Camera::ray` and `cast` do in
`cubarium/src/raycast.rs` and take `Hit::pixel()`; background where the ray
misses. The port must keep that file's round-trip test (every one of the
20,480 face-pixel centres inverts to itself).

Both modes: background is `0x000000`. Sampling is nearest-neighbour. Output
is XRGB8888 (bytes B, G, R, X) into the back buffer's stride.

## Work packages

| id | what | where | model / effort | after |
|---|---|---|---|---|
| W1 | `cube-screen-shim` daemon: crate, config, layouts, ingest, idle, KMS on `msm_drm`, systemd unit, native build + deploy on the Tachyon, `outputs` and `test-pattern` verified on the panel if it is up | `led-cube-shim` worktree `.claude/worktrees/tachyon-screen` (branch `tachyon-screen`) | Opus, high | now |
| W2 | Cubarium on the device: native build, `cubarium.service`, persistent state dir, deploy/status scripts, docs, measured fps/tick rate/CPU, the `--fps` choice, a boot-to-world check | `cubarium` worktree `.claude/worktrees/tachyon-screen` (branch `tachyon-screen`) | Opus, high | W1 |
| W3 | Independent test authoring for W1: layout LUT math against the spec above, ingest/stale-seq behaviour, idle phases, config defaults; no reading of W1's tests first | `led-cube-shim` worktree | Opus, high | W1 |

Fable owns integration: reads the diffs, runs both workspaces' tests, watches
the panel, and decides the merge. Merge to `main` in both repos waits for a
quiet moment, per Wrysk.

## Verification

- `cargo test --workspace` green in both worktrees (the LED cube daemon's
  tests must stay green after any `cube-kms` change).
- On the Tachyon: `cube-screen-shim outputs` lists `DP-1` with a mode;
  `cube-screen-shim test-pattern faces` shows F/R/B/L/T upright with the red
  dot top-left and green dot top-right in every cell (the same pattern that
  verified the cube).
- End to end on the device: both services enabled, `systemctl reboot`, and
  within a minute the world is on the panel with no hands on it. The shim log
  reports the presented fps and received/stale counters; cubarium's log
  reports render fps and tick rate.
- A photo or capture of the panel is the acceptance evidence for the visual
  parts; the numbers that matter are render fps and sim ticks per real second
  on the board.

## Out of scope for phase 1

Touch input (no kernel input device exists yet), a flat non-cube world,
camera animation in `cube` mode, HDR/colour management, audio, running the
services as a non-root user, streaming frames from the desktop over wifi,
any change to cubarium's world schema.

## Briefs

- [W1: the screen shim daemon](tachyon-screen-w1-opus-shim-2026-09-16.md)
- W2 and W3 briefs are written after W1 reports.
