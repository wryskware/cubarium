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
GPU writes the buffer the display controller scans out. The only privilege the
service has is two supplementary groups — `video` for the socket and the
read-only connector probe, `render` for `/dev/dri/renderD128`, which the Vulkan
loader opens and which fails with `vkCreateInstance: A host memory allocation
has failed` if you forget it. The daemon is never stopped or reconfigured by
anything here; if it restarts, the client dies with it and `Restart=always`
brings the client back.

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
2. If snapshot files are **present but none load**, the run is *refused*:
   "this is a damaged world, not an empty directory, so no new world is created
   here." With `Restart=always` that shows up as a restart loop and a black
   panel, which is the intended outcome — the standing rule is *always fresh,
   never migrate*, and silently founding a new world every boot is the one
   failure this whole arrangement exists to avoid. Move the files aside
   deliberately.
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
Environment=CUBARIUM_EXTRA_ARGS=
```

in the unit. systemd splits an unquoted `$VAR` on whitespace and expands an
empty one to no arguments at all, so this is a one-line edit plus `systemctl
restart cubarium`. It exists for the two open viewing-session decisions:

* `--gpu-art-scale 2` — 32-pixel plants instead of 16. `S` currently scales the
  cell grid and leaves the art the size it was authored; the ring-world plan §6
  says `S` should multiply the sprite tile too. The two have not been
  reconciled and this flag shows both.
* `--gpu-bend-substep` — lets the wind's displacement land between source texels
  at `S >= 2` instead of rounding to a whole one. With the shipped pack's
  measured bend budgets (0.3–1.3 texels, derived against the *cube's* nine-pixel
  footprint) the default is often no visible breeze at all.

Either can also go in a drop-in: `systemctl edit cubarium`, then
`[Service]` / `Environment=CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2`.

## The viewer, and why it is off by default

`--mirror-web` serves the same frames as a page on `127.0.0.1:7393`. It is
**not** in the default `ExecStart`, because `FanOutSink::wants_pixels` is true
if any child wants pixels, and the web sink wants pixels — so mirroring makes
the host pay for the whole CPU rasterisation and PNG encode that the GPU sink
otherwise skips, which is the entire reason the GPU path is faster than the CPU
one. Measured on this board, 640x360 S=2, 40 s each, everything else identical:

| | frames | seconds | fps | GPU ms |
|---|---|---|---|---|
| without `--mirror-web` | 2384 | 40.04 | **59.5** | 3.84 |
| with `--mirror-web` | 660 | 40.13 | **16.5** | 3.84 |

The GPU cost is unchanged; all of the loss is CPU the panel is no longer
getting. The panel is the product and the viewer is a convenience, so the
viewer is opt-in. To watch it for a session:

```sh
# on the board
sudo systemctl edit cubarium     # [Service]
                                 # Environment=CUBARIUM_EXTRA_ARGS=--mirror-web --web-port 7393
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
