---
design_status: exploration
last_reviewed: 2026-09-16
---

# W2 result: cubarium as a service on the Tachyon

Brief `design/handoffs/tachyon-screen-w2-opus-deploy-2026-09-16.md`. Branch
`tachyon-screen`, worktree only, two path-only commits (`461c8b6`, `3d092f8`).
`cargo test --workspace --exclude cubarium-gpu`: **148 suites, 1959 passed, 0
failed, 32 ignored**. Nothing under `crates/**` was touched — no CLI refusal
message turned out to be wrong.

**The service is installed, enabled, and was presenting the live ring world at
60 fps as the unprivileged user `cubarium`. Restart-resume, the fresh-world
density and the black-panel behaviour are all verified. The boot-to-world check
is not: the one allowed `systemctl reboot` was issued at
2026-09-16T14:54:27Z and the board has not come back on the network since.**
That is the state to read this report in.

## 1. What is installed

| what | where |
|---|---|
| `useradd --system --home-dir /var/lib/cubarium --shell /usr/sbin/nologin --groups video,render cubarium` | `uid=996(cubarium) gid=993(cubarium) groups=993(cubarium),44(video),110(render)` |
| the binary | `/usr/local/bin/cubarium`, root:root 0755, 65,816,184 bytes |
| the world | `/var/lib/cubarium/state/`, cubarium:cubarium 0755 |
| the art | `/var/lib/cubarium/art/`, 6 files, cubarium:cubarium |
| the fresh-world config | `/var/lib/cubarium/world.toml`, root:root 0644 |
| the unit | `/etc/systemd/system/cubarium.service`, enabled (`multi-user.target.wants` symlink created) |
| this doc's operator half | `/usr/local/share/doc/cubarium/tachyon.md` |

`cube-screen-shim.service` was never stopped, never reconfigured and never
restarted by anything here; nothing was written to sysfs or debugfs; no `apt`.

Sources: `config/tachyon/cubarium.service`, `config/tachyon/world.toml`,
`scripts/tachyon-install.sh` (idempotent, runs on the board),
`scripts/tachyon-deploy.sh` (rsync + native build + install + restart),
`scripts/tachyon-status.sh`, `docs/tachyon.md`, one README section.

### The unit, as installed

```ini
[Unit]
After=cube-screen-shim.service network.target
Wants=cube-screen-shim.service

[Service]
Type=simple
User=cubarium
Group=cubarium
SupplementaryGroups=video render
WorkingDirectory=/var/lib/cubarium
Environment=RUST_LOG=info
Environment=CUBARIUM_EXTRA_ARGS=
CPUAffinity=4-7
ExecStart=/usr/local/bin/cubarium run \
    --state /var/lib/cubarium/state \
    --config /var/lib/cubarium/world.toml \
    --topology ring:640x360 \
    --world-scale 2 \
    --sink gpu \
    --gpu-target shim \
    --art /var/lib/cubarium/art \
    --fps 60 \
    $CUBARIUM_EXTRA_ARGS
KillSignal=SIGINT
TimeoutStopSec=30
Restart=always
RestartSec=3
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
```

**There is no wrapper.** Three of the brief's open questions closed against it:

* `CUBARIUM_EXTRA_ARGS` needs no script. systemd splits an unquoted `$VAR` on
  whitespace and expands a defined-empty one to no arguments at all, so the
  pending look decision is literally the one-line edit the brief asked for.
* `--gpu-target shim` is explicit rather than left to `GpuTargetKind::detect()`,
  which picks `window` when `/run/cube-screen-shim/frames.sock` does not exist.
  At boot that race would end in a window nobody can see; explicit fails loudly
  and `Restart=always` retries.
* the resume, below.

## 2. Resume semantics, and the code they come from

**A plain `cubarium run` already is "resume if present, else fresh".** No flag
selects it. `crates/cubarium/src/runner/mod.rs:131` `open_world`, with neither
`--fresh` nor `--require-resume`:

```rust
if !run.fresh {
    let loaded = state::load_newest(&run.state, &mut report);
    if let Some(loaded) = loaded { … eprintln!("cubarium: resuming {} at tick {tick}", …); return … }
    if !failures.is_empty() { anyhow::bail!("… {} snapshot file(s) are present and none of them loaded … \
                                             This is a damaged world, not an empty directory …") }
    anyhow::ensure!(!run.require_resume, …);
    eprintln!("cubarium: no loadable snapshot in {}; creating a new world", …);
}
```

Three branches, and the middle one is the reason this is safe to leave
unattended: a state directory whose snapshots exist but will not load is
**refused**, not replaced. Under `Restart=always` that is a restart loop and a
black panel — which is the correct outcome under *always fresh, never migrate*.
The failure the brief warned about, founding a new world on every restart, is
structurally impossible here: a new world needs a directory with no snapshot
file at all.

Two flags that look risky and are not:

* `--topology ring:640x360` is passed on every start including resumes. On a
  resume `open_world` compares it with the snapshot's own topology and bails by
  name on a mismatch ("a world's shape is fixed when it is created"); it can
  never reshape a world. `--world-scale` is not even compared — the fresh-world
  branch is the only place `run.scale()` is read.
* `--config world.toml` is read on every start, but on a resume
  `merge_operational` copies `capacity` and `weather.moving` out of it and
  nothing else. Editing the founders cannot disturb a live world.

### KillSignal=SIGINT is load-bearing

`crates/cubarium/src/run.rs:35` installs the clean-stop flag through
`ctrlc::set_handler`, and `crates/cubarium/Cargo.toml:32` is `ctrlc = "3"` with
**no `termination` feature**, so the flag is raised by SIGINT and by nothing
else. systemd's default SIGTERM would have killed the process between
checkpoints and discarded up to `capacity.checkpoint_seconds` = 60 s of world on
every restart and every reboot. With `KillSignal=SIGINT` the runner's
`run_world_until` takes its normal exit path and `checkpoints.queue(final_tick,
…)` writes a snapshot at exactly the tick it stopped on. Evidence in §3.

## 3. Evidence

All timestamps from the board's journal (`-0700` = PDT; the daemon's own lines
carry UTC).

### The world is founded with 67 founders

A throwaway headless run of the installed binary against the installed config:

```
cubarium: tick 200 pop 67 births 0 deaths 0 forms 28/14/11/14 fruit 0.00 water 61.4 residual 1.910e-11
{"tick":200,"population":67,…,"population_by_form":[28,14,11,14,0,0,0,0],…}
```

28 grazers / 14 gliders / 11 burrowers / 14 skimmers — the plan's 24 × 2.81 ≈ 67
spent kind by kind, because `founders.count` is ignored whenever
`founders.kinds` is non-empty. `population_by_face` is `[67,0,0,0,0]`: one
chart, so the TOML's `topology = { Ring = { w = 640, h = 360 } }` took.

### First start founds; the panel goes live

```
07:49:49 systemd[1]: Started Cubarium — the living ring world on the Tachyon panel.
07:49:49 cubarium: no loadable snapshot in /var/lib/cubarium/state; creating a new world
07:49:49 cubarium: --sink gpu on Adreno (TM) 643
07:49:50 shim socket: 3 slots attached at 1080x1920 XR24 pitch 4352, sRGB encode by the _SRGB attachment
14:49:50Z cube-screen-shim: handoff: imported 1080x1920 XR24 pitch 4352 into slot 1
14:49:50Z cube-screen-shim: frames arriving — live
```

### `--mirror-web` costs 72 % of the frame rate

That first start was at 11.8 then 16.4 fps, not 60. `FanOutSink::wants_pixels`
is true if any child wants pixels and the web sink wants pixels, so mirroring
makes the host pay for the whole CPU rasterisation and PNG encode that the GPU
sink exists to skip. A/B, same binary, same world config, same board, 40 s each,
run by hand as `cubarium` under `taskset -c 4-7`, only the flag differing:

```
--- nomirror ---
cubarium: --sink gpu drew 2384 frames: scene 5.50 ms, GPU 3.83 ms, submit+present 6.19 ms,
          3966 instances, 922 frame slot(s) dropped from over-full stamps (0.3867 per frame)
cubarium: tick 800 (800 ticks, 2384 frames in 40.04 s, 1x real time), population 67

--- mirror  --mirror-web --web-port 7393 ---
cubarium: --sink gpu drew 660 frames: scene 1.86 ms, GPU 3.84 ms, submit+present 5.63 ms, …
cubarium: tick 800 (800 ticks, 660 frames in 40.13 s, 1x real time), population 67
```

**59.54 fps against 16.45 fps, at an unchanged GPU 3.84 ms.** All of the loss is
CPU the panel is no longer getting.

**Design call (mine, and the one deviation from the brief):** `--mirror-web` is
out of the default `ExecStart` and into `CUBARIUM_EXTRA_ARGS`, documented with
these numbers and with the `ssh -L 7393:127.0.0.1:7393` line. The brief asked
for both "`--mirror-web` on loopback" and "the daemon log shows the dma-buf
client at ~60 fps" and they are not simultaneously satisfiable without a change
under `crates/**`, which I do not own. The panel is the product. Visible effect:
no viewer page unless an operator asks for one, and when they do the panel drops
to ~16 fps until they stop asking. The server binds `127.0.0.1` only
(`sink/web/mod.rs:274`) and refuses a foreign `Host`, so the tunnel stays the
only way in.

After removing the flag:

```
14:53:01Z cube-screen-shim: presented 60.2 fps | … | live | dma-buf client: 3 slot(s), 194 flips
14:53:06Z cube-screen-shim: presented 60.0 fps | … | live | dma-buf client: 3 slot(s), 494 flips
14:53:11Z cube-screen-shim: presented 60.0 fps | … | live | dma-buf client: 3 slot(s), 794 flips
14:53:16Z cube-screen-shim: presented 60.0 fps | … | live | dma-buf client: 3 slot(s), 1094 flips
```

### Restart resumes the same world at the same tick

```
07:53:33 cubarium: tick 1730 (697 ticks, 2085 frames in 34.90 s, 1x real time), population 67, residual 2.728e-12
07:53:33 systemd[1]: cubarium.service: Succeeded.
07:53:33 systemd[1]: Stopped Cubarium …
07:53:33 systemd[1]: Started Cubarium …
07:53:33 cubarium: resuming /var/lib/cubarium/state/world-1730.cubw at tick 1730
```

`Succeeded.` is the SIGINT path taken: the runner exited `Ok` rather than being
killed. `world-1730.cubw` is dated 07:53 and the resume line names it. **Zero
ticks lost.** The segment before the restart also gives an in-service frame
rate: 2085 frames in 34.90 s = **59.74 fps**.

A second restart, from the stop/start check below, resumed
`world-2132.cubw at tick 2132` the same way.

### The panel blacks on stop and returns on start

```
14:53:53Z cube-screen-shim: handoff: client disconnected
14:53:53Z cube-screen-shim: handoff: client disconnected after 20.2s and 1189 flips (… 0 framebuffer(s) still held)
14:53:53Z cube-screen-shim: idle — showing the Black screen
14:53:56Z cube-screen-shim: presented 59.6 fps | … | idle | source udp
--- systemctl start at 14:54:01Z ---
14:54:02Z cube-screen-shim: handoff: imported 1080x1920 XR24 pitch 4352 into slot 1
14:54:02Z cube-screen-shim: frames arriving — live
14:54:06Z cube-screen-shim: presented 59.8 fps | … | live | dma-buf client: 3 slot(s), 254 flips
07:54:01  cubarium: resuming /var/lib/cubarium/state/world-2132.cubw at tick 2132
```

Black within a second of the stop; live 0.6 s after the start, on the world it
had. 1189 flips in 20.2 s is **58.9 fps** by the daemon's own count.

## 4. The reboot

```
PRE-REBOOT 2026-09-16T14:54:23Z
  uptime -s                       2026-09-16 03:06:29
  snapshots                       world-1730.cubw, world-2132.cubw, world-2400.cubw
  last telemetry                  tick 2500, pop 67
  is-enabled                      cubarium: enabled, cube-screen-shim: enabled
REBOOT ISSUED 2026-09-16T14:54:27Z   (ssh root@192.168.68.68 'systemctl reboot')
```

The board did not return. Polled continuously to **2026-09-16T15:51:57Z — 57
minutes** — with no ICMP reply and an `INCOMPLETE` ARP entry the whole time, so
it is not answering at layer 2 either, not merely on a different port or
service. A sweep of 192.168.68.60–80 found no new host, `avahi-resolve` on
`tachyon-8968c731.local` timed out, and there is no `adb`, no USB device and no
serial port on this machine to reach it out of band.

**What this does and does not say.** It does not say the boot-to-world check
failed: nothing installed by this package runs before `multi-user.target`, the
unit was enabled and correct, and a service that crash-looped would not take the
network with it. The board rebooted normally at 03:06:29 local the same morning
with the daemon alone installed. The honest statement is that a `systemctl
reboot` of this board did not bring it back, cause unknown, and the first thing
to establish after a power cycle is whether a plain reboot brings this board
back **at all** before anything is concluded about `cubarium.service`. The one
suspicion worth naming is a shutdown-time hang in the msm KMS/GPU path with a
dma-buf client attached — which would be the daemon's and the driver's ground,
not the unit's — and it is a suspicion, not a measurement.

Because of this, the **ten-minute steady state (CPU core-s/s, peak RSS, fps) was
never run**. The only CPU and RSS readings taken were during the `--mirror-web`
run (81 % of one core, RES 93,908 KiB) and are not comparable to anything.
GS-1b's five-minute figure for the same rung as `particle` — 59.9 fps, 0.668 CPU
core-s/s, 57 MiB peak RSS — is the reference the re-run should be compared with.

## 5. Numbers, in one place

| what | duration | result |
|---|---|---|
| hand run as `cubarium`, no mirror | 40.04 s | 2384 frames = **59.54 fps**; scene 5.50 ms, GPU 3.83 ms, submit+present 6.19 ms, 3966 instances, 0.387 slots dropped/frame |
| the service, between restarts | 34.90 s | 2085 frames = **59.74 fps** |
| the daemon's own count | 20.2 s | 1189 flips = **58.9 fps**; its 5 s lines 60.2 / 60.0 / 60.0 / 60.0 / 59.8 / 59.0 / 59.8 / 59.8 |
| the same, with `--mirror-web` | 40.13 s | 660 frames = 16.45 fps |
| fresh world from `world.toml` | 200 ticks | pop 67, forms 28/14/11/14, residual 1.9e-11 |
| clean stop → restart | — | tick 1730 → `world-1730.cubw` → tick 1730; tick 2132 → tick 2132 |
| native release build on the board | — | **102 s**, `taskset -c 4-7`, incremental against a seeded target |
| ten-minute steady state | — | **not run** |
| boot to world | — | **not run**; board unreachable since 14:54:27Z |

The first build in an empty `/root/cubarium-deploy/target` is a cold workspace
build of tens of minutes. It was made incremental by seeding it once —
`cp -a /root/cubarium-gs1/target /root/cubarium-deploy/target` — which the
deploy script documents in a comment rather than doing by magic.

## 6. What is left

1. **Power-cycle the board and run the boot check.** Then: both services active
   and frames flowing within 60 s of the kernel, and
   `journalctl -u cubarium | grep resuming` naming a snapshot at or near tick
   2500. If a plain reboot turns out to hang this board generally, that belongs
   to the daemon's package, not this one, and the shelf piece's real
   requirement — survive a power cut — is then the thing to test instead.
2. **The ten-minute steady state**: CPU core-seconds per second from
   `/proc/<pid>/stat`, peak RSS, sustained fps, against GS-1b's 0.668 / 57 MiB /
   59.9.
3. **The look decision** is one `CUBARIUM_EXTRA_ARGS=` edit away:
   `--gpu-art-scale 2` (32-px plants) and `--gpu-bend-substep` (sub-texel wind).
   Both still open, both still Wrysk's.
4. **No hunter has been drawn on this panel**, and `--sink gpu` still refuses a
   cube by name. Unchanged from GS-1b.
5. The viewer costs the panel 43 fps. If it is wanted *while* the world is on the
   panel, the fix is in `crates/**` — a web sink fed from the GPU raster instead
   of one that forces the CPU rasterisation — and it is not in this package's
   scope.
