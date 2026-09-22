# Cubarium on the Tachyon

The shelf piece: power the board and, with no hands on it, the panel shows the
live ring world within a minute of the kernel, and it is the *same* world it
was showing before the power went out.

## Architecture

Two processes, and the split between them is a privilege split.
`cube-screen-shim` (root, `/etc/systemd/system/cube-screen-shim.service`) owns
KMS: it brings the DP-1 panel up at 1080x1920, re-opens it if the panel is lost,
page-flips whatever it is given, and shows black when nobody is giving it
anything. It listens on a Unix socket, `/run/cube-screen-shim/frames.sock`,
handed to group `video` at mode 0660 after `bind` and before `listen`, and it
accepts **one client at a time**.

`cubarium` (this repo, `/etc/systemd/system/cubarium.service`) is that client
and nothing more. It runs as the unprivileged system user `cubarium`, simulates
the world, renders it on the Adreno 643 with Vulkan straight into scanout-linear
images, exports those images as dma-bufs and hands the file descriptors to the
daemon over the socket with `SCM_RIGHTS`. No copy of a frame is ever made: the
GPU writes the buffer the display controller scans out. The service's privileges
are two supplementary groups — `video` for the socket and the
read-only connector probe, `render` for `/dev/dri/renderD128`, which the Vulkan
loader opens and which fails with `vkCreateInstance: A host memory allocation
has failed` if you forget it — and the ambient `CAP_SYS_NICE` its loop thread
uses for a utilisation floor (below). The daemon is never stopped or reconfigured by
anything here; if it restarts, the client dies with it and `Restart=always`
brings the client back.

## Where the threads run

The QCM6490 is big.LITTLE with a prime: cpu0-3 are Cortex-A55 (`cpu_capacity`
381), cpu4-6 Cortex-A78 (889), cpu7 the prime A78 (1024, 2.7 GHz). The panel's
tick rate is the **main thread's** budget — it steps the world, packs the frame
and hands it to the presenter — so where that one thread runs is most of the
performance. Left to the scheduler (2026-09-22 review, `perf` on the board) it
spent ~30 % of its samples on A55s, migrated 646 k times and averaged 1.35 GHz;
the unit's old `CPUAffinity=0-6` also shut the prime out. Pinned to cpu7 by hand
its worst step fell from 62-66 ms to 17-29 ms.

So, as built:

* **The unit** masks the service to `1-7` (every core but one A55) and, before
  start, writes `core_ctl` `min_cpus` — cpu7's cluster 1, the cpu4 cluster 3 —
  so Qualcomm's core control does not park the prime or an A78 (a parked CPU
  refuses an affinity of only itself with `EINVAL`). Those two are the only
  sysfs writes; they last until reboot.
* **`cubarium voxel`**, at the top of its loop (`voxel/placement.rs`), reads
  `/sys/devices/system/cpu/cpu*/cpu_capacity` for the CPUs in its mask, pins
  the loop thread to the highest (falling to the next big core if one refuses),
  and moves every other thread of the process — presenter, tick pool, web
  encoder, stdin, ctrl-c — onto the **other big cores, 4-6**. The tick pool is
  fork-join over equal column chunks, so a chunk on an A55 is the straggler the
  whole scope waits for. The snapshot writer, spawned later, moves itself there
  too. On a machine whose CPUs all report one capacity (the desk) nothing is
  pinned and nothing is said.
* The loop thread then sets **`uclamp.min` 1024** (`sched_setattr`), so
  schedutil runs cpu7 at full clock while the loop is runnable (it held a
  pinned, half-idle loop at ~1.06 GHz). The loop sleeps between ticks and
  frames, so this is fast-while-working, not hot all the time.

The start-up log says which of this happened, in one line:

```
cubarium voxel: the loop is on cpu7 (capacity 1024 of 381..1024), 8 other thread(s) on cpu 4,5,6; uclamp.min 1024
```

`no uclamp.min (<error>)` there means the kernel refused the floor; "no big
core would take the loop" means every big core was parked. To check the clock
the loop actually gets: `perf stat -e cycles,task-clock -t <main tid> -- sleep 10`
(cycles / task-clock ≥ 2 GHz is the target). If the floor took and the clock
is still low, suspect the cgroup: on 5.4 a task's `uclamp.min` is capped by its
cgroup's `cpu.uclamp.min`, which is `0.00` for `system.slice/cubarium.service`
(later kernels treat the cgroup value as a floor instead). Raising it is a
`/sys/fs/cgroup` write, which the unit is not allowed to make.

The 60 s snapshot is cloned on the loop and encoded, written and `fsync`ed on a
background thread; the stop snapshot (SIGINT) waits for any write in flight and
is written synchronously.

## Install

From a development checkout, with the board on the network:

```sh
./scripts/tachyon-deploy.sh                # rsync, build, install, restart
TACHYON=root@192.168.68.68 ./scripts/tachyon-deploy.sh
```

Use the **IP**. `tachyon-8968c731.local` resolves to a link-local IPv6 address
that ssh refuses.

`tachyon-deploy.sh` rsyncs the checkout to `/root/cubarium-deploy`, builds
`cubarium` there natively (`taskset -c 4-7 cargo build --release -p cubarium
--bin cubarium`; never a single core — `core_ctl` isolates idle big cores), then
runs `scripts/tachyon-install.sh` on the board and restarts the service.

The **first** build in an empty `/root/cubarium-deploy/target` is a cold
workspace build and takes tens of minutes on the A78s. Seed it once from any
tree already built with the same toolchain and it is an incremental one:

```sh
ssh root@192.168.68.68 'cp -a /root/cubarium-gs1/target /root/cubarium-deploy/target'
```

`tachyon-install.sh` is idempotent and is what actually puts things in place:

| what | where | owner |
|---|---|---|
| the user | `useradd --system --home-dir /var/lib/cubarium --shell /usr/sbin/nologin --groups video,render cubarium` | — |
| the binary | `/usr/local/bin/cubarium` | root:root 0755 |
| the world | `/var/lib/cubarium/state/` | cubarium:cubarium 0755 |
| the art | `/var/lib/cubarium/art/` (the baked atelier pack) | cubarium:cubarium 0755 |
| the fresh-world config | `/var/lib/cubarium/world.toml` | root:root 0644 |
| the unit | `/etc/systemd/system/cubarium.service`, enabled | root:root 0644 |
| this document | `/usr/local/share/doc/cubarium/tachyon.md` | root:root 0644 |

It never touches `/var/lib/cubarium/state`. The world lives there and the
installer is not allowed to have an opinion about it.

## Resume: what happens on every start

**A plain `cubarium run` already means "resume if there is a world, found one if
there is not."** No wrapper computes it and no flag selects it; this is what
`runner::open_world` (`crates/cubarium/src/runner/mod.rs:131`) does when neither
`--fresh` nor `--require-resume` is given:

1. `state::load_newest(&run.state, …)` reads the highest-tick snapshot that
   loads. If one does, the run resumes it and says so on stderr —
   `cubarium: resuming /var/lib/cubarium/state/world-1730.cubw at tick 1730` —
   and that line in the journal is the check that a restart did not found a new
   world.
2. If snapshot files are present but every one of them is refused **for its
   schema** (an older world under a newer build), the run deletes them and
   founds a fresh world, saying so:
   `cubarium voxel: discarded 5 snapshot(s) of schema 13 in
   /var/lib/cubarium/state; founding a fresh world (schema 15)`. Wrysk,
   2026-09-22: the panel's worlds are disposable and never backed up. A file
   that is unreadable, truncated or invalid for any *other* reason still
   refuses by name — that is a damaged world, not a stale one — and shows up
   as a restart loop and a black panel until the files are moved aside.
   `scripts/tachyon-reseed.sh` stops the service, deletes the saved worlds and
   starts it again (`--seed N` pins a seed in a drop-in, `--no-seed` unpins).
3. Only a directory with **no snapshot file at all** founds a new world:
   `cubarium: no loadable snapshot in /var/lib/cubarium/state; creating a new
   world`, built from `/var/lib/cubarium/world.toml` plus the `--topology` and
   `--world-scale` on the command line.

Two consequences worth knowing:

* `--topology ring:640x360` is passed on **every** start, including resumes.
  On a resume it is compared against the snapshot's own topology and a mismatch
  is refused by name (`open_world`, "a world's shape is fixed when it is
  created"); it is never used to reshape a world. `--world-scale` is not even
  compared — a resumed world keeps the scale it was created with.
* `--config world.toml` is read on every start too, but on a resume
  `merge_operational` copies only `capacity` and `weather.moving` out of it.
  Editing the founders below therefore cannot disturb a world that exists.

### The clean stop is SIGINT, not SIGTERM

`ctrlc` is built without its `termination` feature, so the flag that asks the
runner to stop cleanly — write a final snapshot at the exact tick, then exit —
is raised by **SIGINT only**. A SIGTERM would kill the process between
checkpoints and throw away up to `capacity.checkpoint_seconds` (60 s) of world.
The unit therefore sets `KillSignal=SIGINT` and `TimeoutStopSec=30`, and a
`systemctl restart`, a `systemctl stop` and a reboot all end in a snapshot at
the tick the world stopped on. Measured: stopped at tick 1730, `world-1730.cubw`
written, resumed at tick 1730. Nothing is lost across a restart.

## The world

`ring:640x360`, `--world-scale 2`: the same 3,600-cell ecology as
`ring:320x180`, drawn at twice the resolution, on a 1080x1920 panel.

`/var/lib/cubarium/world.toml` carries the one fresh-world decision that is not
a default: **67 founders, not 24.** At the cube's founder count the ring is
2.81x thinner than the cube (`design/handoffs/tachyon-screen-plan-2026-09-16.md`,
"FW-2 result and two ring-world calls"), so the ring's fresh-world default
scales with cell count: 24 x 2.81 ~= 67. `founders.count` is ignored whenever
`founders.kinds` is non-empty, so the call is spent kind by kind — burrower
4→11, grazer 10→28, glider 5→14, skimmer 5→14 — with every other field of
`FounderKind::defaults()` transcribed verbatim, because an omitted field means
"take the v1 founder value", not "take the default kind's value". Verified on
the board: a fresh world from this file reports `pop 67 … forms 28/14/11/14`.

## The look knob

```
Environment="CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2"
```

in the unit — a one-line edit plus `systemctl restart cubarium`. It can also go
in a drop-in: `systemctl edit cubarium`, then `[Service]` /
`Environment="CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2"`.

**Quote the assignment, and do not quote the reference.** `Environment=` splits
its own line on whitespace into separate assignments, so an unquoted
`Environment=CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2` sets the variable to
`--gpu-art-scale` alone and drops the `2`; the service then crash-loops with
`error: a value is required for '--gpu-art-scale'`. In `ExecStart`, by contrast,
`$CUBARIUM_EXTRA_ARGS` is deliberately *unquoted*, because there systemd splits
an unquoted variable on whitespace back into separate arguments. An empty value
expands to no arguments at all.

**`--gpu-art-scale 2` is the decided look** (Wrysk, 2026-09-16, after seeing both
on the panel): 32-pixel plants instead of 16. `S` scales the cell grid and leaves
the art the size it was authored; the ring-world plan §6 says `S` should multiply
the sprite tile too, and this is that. It was 42.7 fps when the decision was
taken and GS-1c's fill work brought it to 59.9 over 60 s, so it costs the panel
nothing now.

The other two viewing-session knobs are settled and need no flag:

* the **sub-texel wind** is the default on a ring at `S >= 2`
  (`--no-gpu-bend-substep` turns it off), and
* the **bend budgets** are measured against the world's own footprint rather than
  the cube's nine pixels, so at this rung every species draws the full tip its
  response asks for (`tendrilfan` +92 %, `glasscane` +18 %, the rest unchanged).

## The viewer, and which one to ask for

Two flags serve the same page on `127.0.0.1:7393`, and only one of them is free.
They are refused together.

**`--gpu-web-rate 2` is the one to use.** It serves the raster the GPU has
already drawn, copied back at the rate given — one `vkCmdCopyImageToBuffer` of
640x360x4 and one RGBA→RGB pass, 5.9 ms, twice a second.

`--mirror-web` puts a web sink beside the GPU sink in a `FanOutSink`, whose
`wants_pixels` is true if any child's is — so mirroring makes the host pay for
the whole CPU rasterisation and PNG encode that the GPU sink otherwise skips,
which is the entire reason the GPU path is faster than the CPU one. Measured on
this board at 640x360 S=2 with `--gpu-art-scale 2`, 40 s each, everything else
identical:

| | frames | seconds | fps |
|---|---|---|---|
| no viewer | 2395 | 40.08 | **59.8** |
| `--gpu-web-rate 2` | 2395 | 40.08 | **59.8** (100 frames served) |
| `--mirror-web` | 661 | 40.12 | **16.5** |

The GPU cost is unchanged in all three; all of `--mirror-web`'s loss is CPU the
panel is no longer getting. `--mirror-web` is still the only way to get the
**care** buttons, which are drawn onto the CPU canvas this sink does not have.

To watch it for a session:

```sh
# on the board
sudo systemctl edit cubarium     # [Service]
                                 # Environment="CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2 --gpu-web-rate 2"
sudo systemctl restart cubarium

# on your machine
ssh -N -L 7393:127.0.0.1:7393 root@192.168.68.68
# then open http://127.0.0.1:7393/
```

The server binds `127.0.0.1` and nothing else (`sink/web/mod.rs:274`) and
refuses a `Host` header that is not `127.0.0.1:<port>` or `localhost:<port>`, so
the tunnel is the only way in. Remove the drop-in (`systemctl revert cubarium`)
and restart to get the panel back to 60 fps.

## Status and logs

```sh
./scripts/tachyon-status.sh          # both services, 20 lines each, DP-1, top
ssh root@192.168.68.68 'journalctl -u cubarium -f'
ssh root@192.168.68.68 'journalctl -u cube-screen-shim -f'
```

The daemon's own line is the frame-rate truth:

```
presented 60.0 fps | … | live | dma-buf client: 3 slot(s), 1094 flips
```

`live` versus `idle — showing the Black screen` is whether a client is attached.
Stopping `cubarium` blacks the panel within a second; starting it brings the
world back in under one.

## Measured

Board `tachyon-8968c731`, Adreno 643, `CPUAffinity=4-7`, release build, user
`cubarium`, `ring:640x360 --world-scale 2 --fps 60`, no `--mirror-web`.

| what | how long | result |
|---|---|---|
| hand run as `cubarium`, `taskset -c 4-7` | 40.04 s | 2384 frames = **59.54 fps**; scene 5.50 ms, GPU 3.83 ms, submit+present 6.19 ms, 3966 instances, 0.387 frame slots dropped per frame |
| the service, between two restarts | 34.90 s | 2085 frames = **59.74 fps** |
| the service, as the daemon counts it | 20.2 s | 1189 flips = **58.9 fps**; its five-second lines read 60.2, 60.0, 60.0, 60.0, 59.8, 59.0, 59.8, 59.8 |
| the same run **with** `--mirror-web` | 40.13 s | 660 frames = 16.5 fps |
| clean stop and restart | — | stopped at tick 1730, `world-1730.cubw` written, resumed at tick 1730; and 2132 → 2132 |
| fresh world from `world.toml` | 200 ticks | `pop 67 … forms 28/14/11/14`, mass residual 1.9e-11 |

`--fps 60` is a cap, so 59.5–60.0 is the cap and not a ceiling.

**Two rows are missing and are owed** (2026-09-16): the ten-minute steady-state
CPU core-seconds-per-second and peak RSS, and the boot-to-world check. The one
allowed `systemctl reboot` was issued at **2026-09-16T14:54:27Z** and the board
did not return to the network — no ICMP and no ARP reply for fifty minutes, and
there is no serial or ADB path to it from here. Nothing installed by this
package runs before `multi-user.target`, and the board had rebooted normally
earlier the same day (03:06:29 local) with the daemon alone installed, so the
first thing to establish after a power cycle is whether a plain `systemctl
reboot` brings this board back *at all*, before anything is concluded about
`cubarium.service`. For reference, GS-1b measured the same rung as user
`particle` at 59.9 fps, 0.668 CPU core-s/s and 57 MiB peak RSS over five
minutes; the only CPU and RSS readings taken here were during the `--mirror-web`
run and are not comparable.

## What is not here

* **No hunter has been drawn on this panel.** The GPU rig path compiles and has
  never run against a world with a hunter profile.
* **`--sink gpu` refuses a cube** by name: it draws one raster, and a cube is
  five charts with seams.
* `960x540 S=3` does not reach 60 fps — the limit is the adapter's walk over the
  cells, not the GPU.
* `RUST_LOG=info` is in the unit because the brief asks for it, but the
  `cubarium` binary has no logging framework: everything it says it says on
  stderr unconditionally. The variable is inert here and live for the daemon.

## Voxel world on the panel (VR-2, 2026-09-21)

`cubarium voxel` running as the system service into `cube-screen-shim`'s dma-buf socket.

### Geometry and panel fit

* World: `160 x 48 x 24` voxels at `0.25 m/voxel`.
* Camera: elevated orthographic 30° (`rise = 2`), `px_per_voxel = 4`.
* World raster: `640 x 360` (`160 x 4 = 640` width; 240 rows terrain + 120 rows sky to reach `raster_height = 360`).
* Panel transform: rotated 1 quarter turn (90°) and upscaled 3× nearest-neighbour:
  `360 x 3 = 1080` (panel width) and `640 x 3 = 1920` (panel height).
  **Fills 100 % of the Waveshare 1080x1920 panel with zero black bars.**

### Measured on the device

Board `tachyon-8968c731`, Adreno 643, `CPUAffinity=4-7`, release build, user `cubarium`:

* **Display flip rate**: 52–54 fps sustained presentation through `cube-screen-shim`.
* **GPU frame time**: ~9.0 ms/frame (upload 0.08 ms, slab walk 8.99 ms) on Adreno 643.
* **Host CPU load**: ~35–40 % of one Cortex-A78 core; system >85 % idle.
* **Memory footprint**: ~49 MiB RSS.
* **Web viewer**: `--gpu-web-rate 2` serves the live GPU raster at 2 fps to `http://127.0.0.1:7393/` (tunnel with `ssh -N -L 7393:127.0.0.1:7393 root@192.168.68.68`).

