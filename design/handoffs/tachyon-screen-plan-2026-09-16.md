---
design_status: leaning
last_reviewed: 2026-09-16
decision_refs: []
---

# Tachyon screen: the cube world on a flat HD display

> **Note 2026-09-18.** This plan of record is kept as the live panel contract.
> The GS-0/GS-1 spike research and flat-world briefs it cites were deleted with
> the dated flat-era material; those references are Git history.

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
| SoC / CPU | Qualcomm QCM6490, 8 cores: 4× Cortex-A55 at 1.96 GHz + 3× Cortex-A78 at 2.40 GHz + 1× Cortex-A78 at 2.71 GHz (measured from sysfs 2026-09-16; an earlier "6 usable A55" reading came from a restricted nproc), 7.3 GB RAM; Adreno GPU and Hexagon NPU present, unused by this plan |
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
   change. The board has 8 cores (4× A78 + 4× A55) and 7 GB; a daemon-sized build is minutes.
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
10. **Frame rate and speed are measured, not assumed.** The board is four
   Cortex-A78 and four Cortex-A55 cores. W2 measures achieved render fps and sim tick rate and
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

## Revision, 2026-09-16 (later): one flat world, no cube on the panel

Wrysk's direction after the panel came up: the display shows **a single
1920×1080 logical face, one 2D world with edges, no cube**. The panel's
native mode is 1080×1920 portrait and is not to be changed; the shim rotates.

What stands from above: the device facts, decisions 2, 4, 5, 6, 8, 9, 10,
the W1 daemon's KMS/ingest/idle/service work (verified at 60 fps on
`msm_drm`), and W3's tests. What is superseded: decision 1 (the wire format
gains an additive raster-strip format) and decision 3 (the `net`/`cube`
layouts remain as the cube-frame test path but the Tachyon config uses a
new `raster` mode with `rotation = 90`).

New packages:

| id | what | where | model / effort |
|---|---|---|---|
| W1b | raster strip format in `cube-proto`, raster ingest + `raster` layout + rotation in the shim, panel verification at 1080×1920 | led-cube-shim worktree | Opus, high (W1 resumed) — [brief](tachyon-screen-w1b-opus-raster-2026-09-16.md) |
| FW-A | cubarium coupling audit and the flat-world design: a topology switch (cube stays supported for the LED cube; flat `W×H` with solid edges for the panel), phased implementation plan | cubarium worktree, read-only | Opus, high |
| FW-1..n | the flat world itself, per FW-A's plan | cubarium worktree | decided after FW-A |
| W2 | cubarium on the device (unchanged in spirit, now with the flat world) | cubarium worktree | after FW-n |

Default world raster for the panel: **320×180** (6× integer blocks on the
1920×1080 logical canvas), configurable in the world config; a fresh world
per the standing rule. The edge rule is solid walls (specular reflection,
as the cube's open bottom rim already does), not wraparound.

### Addendum: treat 1920×1080 as a new world (Wrysk, 2026-09-16)

The 320×180 default above is withdrawn as a default; it is one candidate.
Wrysk: the source artwork is already higher resolution than the screen and
the tiny sprite sheets are a downscale for the 64×64 faces, so the world
should be rescaled and the artwork re-baked at a higher resolution, pixel-art
style kept, using the real estate; biome and terrain variation are wanted.
FW-A now also evaluates the world resolution (480×270, 640×360, 960×540,
1920×1080) against the 4× A78 + 4× A55 cores, the art re-bake path and its scale
constants, and a bounded first-version biome/terrain package separable from
the topology work.

### Note: the GPU as the escalation path (2026-09-16)

The board has an Adreno 643 (608 MHz) with Qualcomm's proprietary GLES 2/3,
EGL, Vulkan and OpenCL userspace on `/dev/kgsl-3d0` (packages `adreno-gles`,
`adreno-vulkan`), and Mesa 21.2 beside it. So GPU rendering is available
headless. The first flat world stays on the CPU rasterizer because it is
deterministic and byte-pinned by tests, and because FW-0 has not yet measured
the CPU cost on the A78 cluster. If the measured budget does not fit, or when
the panel-resolution real estate wants effects the sim raster cannot carry,
the lever is a hybrid: sim and sprite stamping on the CPU at world resolution,
the daemon (or a presenter stage) doing upscale, rotation and post-effects on
the GPU at panel resolution (EGL surfaceless, render into a dma-buf imported
into KMS; a pbuffer + readback is the fallback). Wrysk reserves the Hexagon
NPU for organism networks or voice.

### Approved: the GPU hybrid, and the longer-term shaded renderer (Wrysk, 2026-09-16)

Wrysk approved the hybrid as a package after the flat world ships: sim and
sprite stamping on the CPU at world resolution; the display daemon renders on
the Adreno (EGL surfaceless, dma-buf into KMS) doing the integer upscale, the
rotation and panel-resolution post-effects. To give the shaders something to
work with, the raster wire format should be able to carry **auxiliary layers**
beside RGB (candidates: emissive, water mask, height/stratum, rain), so the
daemon can shade water, glow and weather without knowing the world. That is
FW-9, briefed after FW-5 lands, and it replaces the daemon's CPU gather when
present (CPU gather stays as the fallback).

Longer term (not scheduled): a rendering mode with shading, blending and
anti-aliasing, where the pixel-art look is a style rather than a constraint.
That drops byte-identical frame parity between desktop, cube and panel for
anything shaded, so it lives behind a renderer choice per display, with the
CPU rasterizer kept for the LED cube and for tests.

### Ring topology (Wrysk, 2026-09-16)

Wrysk allowed the left and right edges to join. Adopted: the panel world is
a **ring** (`Topology::Ring { w, h }`): one chart whose left/right edge is a
seam to itself with identity transport, top and bottom solid with the
existing rim reflection, embedded as a cylinder so the existing 3D noise and
the existing spherical weather model work unchanged. This removes the
horizontal-wall reflection, the corner rule and the planar weather spec
from the flat-world plan.

### FW-0 result and the presenter budget (2026-09-16)

Measured on the board (`design/7_Research/flat-world-fw0-2026-09-16.md`):
one cube frame costs 15.3 ms on an A78 (13.9 ms of it in `ArtPresenter::draw`,
near population-independent, so it is the per-cell background passes), a world
tick costs 0.53 ms, and the real loop confirms it: the cube world holds 59.9 fps
at 60 and saturates at 64 fps. An A78 is only 1.74× slower than the desktop
core on this code, and 4.84× faster than an A55.

Consequence: with the presenter as it is, the ring at 320×180 tops out near
23 fps on one core and 640×360 at S = 2 is out of reach even with a perfect
four-core split. The board is not the problem; the presenter's full-frame
passes are. Before the scale is chosen, a presenter budget pass (FW-P) profiles
`ArtPresenter::draw` by pass and proposes what to cache or move: slowly changing
layers rendered at the tick rate or on dirty cells rather than at 60 fps,
row-band parallelism (FW-3), and, if still short, the background passes on the
GPU as part of FW-9 rather than "longer term".

Device gotcha for every later package: `core_ctl` isolates idle big cores, so
`taskset -c 7` fails with `EINVAL` on an idle board; pin to `4-7` or retry
under load (`render_bench --pin` does).

### Full GPU render stack (Wrysk, 2026-09-16)

After FW-0's numbers Wrysk chose a full GPU render stack for the panel over
the hybrid: cubarium does sim plus GPU rendering on the Adreno; the daemon
stays the KMS owner (bring-up, idle, page flips) and receives frames as
dma-buf file descriptors over a Unix socket (zero-copy), with the UDP raster
path kept as the remote/fallback route. The CPU rasterizer stays for the LED
cube and for byte-exact tests. First step is a spike (GS-0) on the board:
which driver renders headless (Qualcomm Vulkan/GLES or Mesa), whether a
rendered image can be exported as a dma-buf and scanned out through KMS, and
the readback fallback's cost. FW-P still runs; its per-pass breakdown is the
shader list.

### FW-P result (2026-09-16)

`design/7_Research/presenter-budget-2026-09-16.md`: 98 % of the frame is
background (plants 52 %, water 15 %, ramp 11 %, columns 9 %); the per-frame
part (bodies, rain) is under 2 %. Mechanical waste worth ~4.7 ms of the 15.3
(pixel→cell recomputed per pixel, unfolds recomputed per stamp, `powf` in the
encode) goes to FW-3 with the row-band split (measured 3.7–3.9× on four
cores). Verdict: 320×180 at 60 fps is reachable on the CPU with FW-3 alone;
640×360 at 60 fps needs plant/column sway at the tick rate (Wrysk's call) or
the GPU stack. The per-pass table is the GPU shader list.

### GS-0 result and the GPU packages (2026-09-16)

`design/7_Research/gpu-scanout-spike-2026-09-16.md`: Qualcomm's proprietary
Vulkan (loaded by `ash::Entry::load()`, no ICD manifest needed) renders
headless; a `VK_IMAGE_TILING_LINEAR` `B8G8R8A8` image exported as a dma-buf
imports into KMS with a modifier-free `AddFB2` (pitch 4352) and page-flips at
the panel's 60.37 Hz at 0.12 CPU core-seconds per second, contents verified
byte-identical to the headless reference. Mesa cannot drive the GPU on this
kernel; wgpu works but cannot export dma-bufs without dropping to hal; GLES via
EGL on a GBM display works but adds moving parts; the readback fallback runs at
30 fps and is a diagnostic only. Traps recorded in the spike report: never
`dlopen` the GLES blob yourself, never enable the DRM-modifier extension (UBWC
only, unscannable here), `eglGetDisplay(default)` segfaults.

Decision: the panel's renderer is a Vulkan renderer in cubarium built on
`ash` 0.38 + `drm` 0.15, rendering the ring world straight into scanout memory.
Re-cut of the plan:

| id | what | notes |
|---|---|---|
| GS-1 | `cubarium-gpu` crate: Vulkan device, sprite-atlas and field textures, instanced quads, the full-screen background shader from FW-P's pass table, a linear-image ring exported as dma-bufs; a desktop window path for development (Vulkan on the desktop) and the scanout path on the board | replaces FW-5 as the panel's presenter; FW-5 shrinks to the CPU ring presenter needed for PNG captures, tests and the web viewer |
| GS-2 | the daemon imports dma-bufs handed over a Unix socket (`SCM_RIGHTS`) with (w, h, fourcc, pitch, offset), page-flips them, keeps bring-up/idle; UDP raster stays as fallback | the bonus GS-0 did not test; one small spike inside the package |
| — | the 20 Hz sway question is moot on the GPU path: per-frame sway is free there | CPU path keeps FW-P's levers in FW-3 for the cube |

### Daemon stays; cubarium runs unprivileged (Wrysk, 2026-09-16)

Wrysk confirmed keeping `cube-screen-shim` as the KMS owner (bring-up, reopen
on panel loss, black on sim exit) with cubarium as a dma-buf client, and wants
cubarium to run as a normal user. On the board `/dev/kgsl-3d0` is world-rw
and `/dev/dri/renderD128` is group `render`; the handoff socket becomes group
`video`, mode 0660 (GS-2 follow-up). W2 therefore installs a system user
`cubarium` in groups `video` and `render`, `cubarium.service` with
`User=cubarium`, state under `/var/lib/cubarium` owned by that user, and no
capability beyond that. The daemon keeps root.

### FW-2 result and two ring-world calls (2026-09-16)

FW-2 landed: schema 17, `CubeProjection` against a fixture from an unmodified
`main` (6,000 ticks, no field differs, hash pinned), weather bit-identical on
ring and cube from one seed, ring worlds at 320×180 and 640×360 running with
the same 3,600-cell environment. TOML spelling is `Ring`, not `ring`.

Two effects measured on the ring, for Wrysk to see on the panel before any
knob moves: the floor row is a moat (18 % of all standing water in 1/45 of the
cells; a pond along the bottom of a side-view terrarium, which may well be a
feature), and at the unchanged 24 founders the ring is 2.81× thinner than the
cube. Decision (Fable): the ring's default founder count scales with cell
count (24 × 2.81 ≈ 67) so a fresh ring world starts as dense as a fresh cube;
this is a fresh-world default in W2's config, not a tuning of a running world.
`evap_floor` stays where the backlog keeps it until the panel is seen.

### GS-1 result (2026-09-16)

`design/7_Research/gs1-vulkan-renderer-2026-09-16.md`: `crates/cubarium-gpu`
renders a synthetic ring scene on the Adreno through GS-2's socket at 60 fps
for 320×180 S=1, 640×360 S=2 and 960×540 S=3 (0.14–0.16 CPU core-s/s; GPU
1.6/3.7/6.8 ms); 1920×1080 S=6 runs at 30 fps because the present pass reads
8 MB per frame. `Scene` = tick-rate `Fields` + frame-rate `SpriteInstance`
layers; `Scene::push` handles the seam by pushing a second instance one
circumference away; integer `scale` only (S = 1.5 refused; the pixel-art
rule). Decisions (Fable): Stage B adds two more frame slots so multi-pose
stamps composite as the CPU does; `canopy_top` = 0.67 as the plan's default;
the bend budget on the ring and the quarter-turn direction are settled by
Wrysk looking at the panel. Stage B (adapter from `RenderView`, hunters as
per-part instances, a GPU sink in the runner) follows FW-4.

### Viewing session 1 (Wrysk, 2026-09-16)

Wrysk watched GS-1's synthetic ring scene at 640×360, S = 2, on the panel:
"looking really good so far". Orientation and look accepted. Wind: the
higher resolution deserves smoother sway than whole-texel steps; deferred
("a later problem"). Recorded for Stage B / the art pass: sub-texel bend at
S ≥ 2 (the bend displacement may move in 1/S steps without breaking the
S×S block rule for everything else), and the bend budgets on the ring are
open, not bound by the cube's 9 px footprint.

### SYNC-1 and GS-1b results (2026-09-16)

SYNC-1 merged `main` (100 commits, ecology v1 rounds 2–5) as `b02524f`
plus an integration merge `de16e69`; eight conflicts, both intents kept; the
cube proof redone against `main`'s head `15a2210` gives the same projection
hash `10304345502826573087` (the two fixtures differ in one byte, the schema
field). Design call accepted: **schema 18** (both lines had spent 17 on
different things; a number must mean one shape to refuse by name). Consequence
noted: every ES checkpoint and exported policy written before the ring world is
foreign to this build by `config_hash`; consistent with always-fresh. A second
sync is owed before the final merge (`main` is 17 commits further, touching
two of the eight conflict sites).

GS-1b: the live ring world renders on the GPU via `--sink gpu` (adapter,
three `FrameSink` hooks, fidelity test: bilinear sampling matches the CPU
within 0.5 per channel, the pixel-art sampler differs by 16 and all of it is
the sampler); 640×360 S=2 at 59.9 fps for five minutes as `particle`;
960×540 at 46 fps (the adapter's walk, not the GPU, is the limit). Open:
the art scale on the panel (`--gpu-art-scale 2`), the ring bend budgets, a
hunter never yet drawn on the GPU, the adapter split for 960×540.

### W2 result (2026-09-16)

`design/7_Research/tachyon-w2-deploy-2026-09-16.md`: `cubarium.service` runs
as the unprivileged user `cubarium` (groups `video`, `render`, CPU affinity
4–7, `KillSignal=SIGINT` because the host's ctrl-c handler only listens for
SIGINT) and a plain `cubarium run` already means resume-if-present-else-fresh
by construction, so no wrapper. Fresh rings found 67 founders spent kind by
kind in `world.toml`. Restart resumes at the same tick; the panel blacks on
stop and returns on start; 59.7 fps as the service. `--mirror-web` forces
the CPU rasterisation and costs 72 % of the frame rate, so it lives in
`CUBARIUM_EXTRA_ARGS` for operators; a web sink fed from the GPU raster is
a later item. **The board did not return from its `systemctl reboot` at
14:54Z** and needs a physical power cycle; the boot-to-world check and the
ten-minute steady state are still owed, and the first question after power
returns is whether a plain reboot hangs this board at all (suspect: a
shutdown-time hang in the msm KMS/GPU path with a dma-buf client attached).

### GS-1c result (2026-09-16)

`design/7_Research/gs1c-renderer-followups-2026-09-16.md`, commits
`cda60f2`, `fd61086`, `c5994f9`, `cc45d39`, `f52d2c1`, `bf7e0b5`, `68a608e`
(all path-only; Fable checked each `--stat`). Integration check by Fable:
148 test binaries green across the workspace excluding `cubarium-gpu`, the
GPU crate green, the board's two services active with `cubarium` running as
its own user at `--gpu-art-scale 2`, the daemon reporting 60.0 fps with
58,197 flips since attach (about sixteen minutes of continuous 60 fps, which
covers the ten-minute steady state W2 owed).

What landed: the 32-pixel look at the 60 fps cap (fill 13.13 → 3.54 Mpx per
frame through an opaque box, mask clamp, early discard, a pad sized to the
bend's own reach, and the runner's duplicate `ArtPresenter` no longer observed
every tick); ring bend budgets measured against `min(9·S, max_local_radius)`
(cube bit-identical; on the panel only tendrilfan and glasscane change, and
`WIND_RESPONSE` is now the binding constraint, a viewing-session number for
Wrysk); `--gpu-bend-substep` default on a ring at S ≥ 2; a hunter drawn on the
GPU and diffed against the CPU (145 vs 144 pixels in the same box, review pair
under `crates/cubarium-gpu/tests/golden/`); FW-4's demo guard lifted (all four
scenes run on a ring); `--gpu-web-rate <fps>` serves the viewer from the GPU
readback at no measurable cost (59.8 fps against 16.5 for `--mirror-web`,
which stays the only route to the care buttons).

One correction to W2's unit: `Environment=` splits its own line on whitespace,
so the assignment must be quoted (`Environment="CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2"`)
while the `$CUBARIUM_EXTRA_ARGS` reference in `ExecStart` must not be. Fixed in
`config/tachyon/cubarium.service`, on the board, and in `docs/tachyon.md`.

Left open (from the report): the adapter's walk is the frame's largest CPU
term (5.5–6.3 ms against 4.0 ms of GPU), so the row-band split of the adapter
is the next lever and what 960×540 needs; ground cover overlaps fourfold at
`--gpu-art-scale 2` (41 % of the sprite fill) if pixels are ever needed back;
`--sink gpu` still refuses a cube; the `systemctl reboot` test with AutoBoot
waits for Wrysk to be near the board. No renderer work is queued: the next
step on the look is Wrysk's answers to the landscape questions.

### SYNC-2 and the merge to main (2026-09-16)

`main` at `ac03da2` (41 commits: ecology v1 round 5, workstreams X/Y/Z) merged
into `tachyon-screen` as `775e0be`; `main` fast-forwarded to it. Two conflicts
(lifecycle.rs, episode.rs) and three call sites main had added against the
cube-only signatures. The action-adapter suite's pinned whole-state hash test
was deleted at Wrysk's direction rather than re-recorded (a world's bytes are
not a promise this project makes; the remaining pins are issue #13). 152 test
binaries green, 2,019 tests.

The led-cube-shim branch is not merged yet: its main checkout carries another
session's uncommitted `Cargo.toml`/`Cargo.lock` edits that the branch also
touches. The landscape direction is held for a fresh thread. Every open item
from this plan is now a GitHub issue (wryskware/cubarium #1–#17,
wryskware/led-cube-shim #1–#2); `design/backlog.md` points there.
