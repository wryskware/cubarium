# cubarium host — M1 contract

The host owns the clock, output sinks, and development scenes. It never contains
geometry: every spatial operation goes through `cubarium-surface`, every pixel
through `cubarium-render`. M1 ships geometry fixtures, not an ecosystem, and the
fixtures are explicit development modes.

## Command

```
cubarium demo [--scene body|vertex|patch|all] [--sink preview|shim|png]
              [--seconds N] [--seed N] [--addr 127.0.0.1:7392]
              [--out captures/] [--every N] [--scale N]
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--scene` | `all` | Which fixture(s) to run |
| `--sink` | `preview` | Where frames go |
| `--seconds` | `0` (until closed) | Stop after this much wall time; required for `png` |
| `--seed` | `1` | Seed for the fixture's own deterministic PRNG (SplitMix64) |
| `--addr` | `127.0.0.1:7392` | Shim daemon address for `--sink shim` |
| `--out` | `captures` | Directory for PNG captures |
| `--every` | `30` | With `png`, save one capture every N rendered frames |
| `--scale` | `4` | Preview window pixel scale |

Exit code 0 on clean stop, nonzero with a message on a fatal error (window
creation failure, unwritable capture directory). Losing the shim is not fatal.

## Clock

Simulation ticks at a fixed 20 Hz (`dt = 0.05 s`, integer tick counter);
rendering at 30 Hz. Real time drives both from one monotonic clock. If the host
stalls, run at most four catch-up ticks, then drop render work and log the lag
once per second at most. Sleeping between frames must not busy-wait. Ticks are
never fast-forwarded after a suspend: on resume the clock re-bases and reports
the pause. Rendering shows the state of the last completed tick (no interpolation
in M1; render views are cloned snapshots of scene state, never a live reference).

## Sinks

`FrameSink::submit(&Frame)` consumes an encoded frame. Sinks:

- `preview`: a `minifb` window on the main thread. Layout at scale `s`: the
  unfolded net on the left (Top above Front; Left, Front, Right, Back in a row,
  each 64·s square, 1-pixel dark separators between faces) and a rotatable cube on
  the right (square viewport 256·s). The cube is ray-cast in software per
  viewport pixel against the unit cube `[-1,1]^3` with the bottom open (rays that
  hit only the -Y plane show background); the hit face and `(u,v)` come from the
  inverse of the `cubarium_surface::face_frame` embedding, sampled nearest-pixel
  from the same encoded frame bytes the shim receives. Mouse drag rotates (yaw
  about +Y, pitch clamped to ±85°); the initial view is from slightly above the
  Front/Right corner. Keys: `n`/`c` toggle net/cube, `s` writes a capture PNG,
  `q`/Escape quits. No labels, grids, or overlays inside the face images; the
  optional face letters draw only in the separators and only while `l` is held.
- `shim`: a worker thread owning a `cube_proto::CubeClient` and a newest-frame
  mailbox (`Mutex<Option<Frame>>` + `Condvar`); `submit` replaces the mailbox
  frame and never blocks on the socket. On send error: log once, back off
  100 ms doubling to 5 s, reconnect, keep the newest frame. The simulation loop
  never waits on this thread.
- `png`: writes `frame_NNNNNN.png` of the net layout at scale 1 (Top above
  Front; Left, Front, Right, Back in a row; black elsewhere), every `--every`
  frames, plus `final.png` on exit.

Frames reaching every sink are the identical `Frame` bytes produced by one
`Canvas::encode` per rendered frame.

## Scenes

All scenes are deterministic given `--seed`; wall time never enters scene state.

- `body`: one asymmetric body (core lobe radius 1.6 at the origin, head lobe
  radius 1.0 at `(+2.4, 0)`, side lobe radius 0.9 at `(-1.4, +1.3)`) moving at
  4 px/s with heading that turns by a slowly varying rate (Ornstein–Uhlenbeck on
  turn rate, from the seeded PRNG) so that over minutes it crosses every seam and
  reflects off the rim. Color warm white `[0.9, 0.7, 0.4]`. A trail of at most
  160 segments, 8 s maximum age, dim blue-green `[0.1, 0.35, 0.3]`. The heading
  is transported by `Travel::map` every tick.
- `vertex`: four static copies of the same body anchored 2.5 px diagonally
  inside each top vertex on the Top face (`(2.5, 2.5)`, `(61.5, 2.5)`,
  `(61.5, 61.5)`, `(2.5, 61.5)`), headings rotating at 0.25 turn/min, and one
  more body straddling the Front/Right seam at Front `(63.2, 40)`. This is the
  ownership-discontinuity fixture for visual review.
- `patch`: a scalar field with diffusion rate 0.15 per tick, decay 0.5 %/tick,
  and a deposit of amount 40, radius 10 every 12 s cycling through four centers:
  Front `(60, 32)` (Front/Right seam), Right `(50, 3)` (Right/Top twisted seam),
  Back `(61, 3)` (Back/Top/Left vertex region), Front `(20, 61)` (rim clipping).
  Rendered with `draw_field(scale 6, color [0.12, 0.5, 0.2], filter on)`.
- `all`: patch, then body trail, then bodies, in that draw order.

## Verification the host must ship

- An integration test that starts a local UDP receiver, drives the shim sink,
  decodes datagrams with `cube_proto::decode`, and asserts the payload equals the
  encoded `Frame` bytes for several frames (sequence numbers increasing).
- A test that the png sink's net layout places face `(x, y)` pixels at the
  documented offsets.
- A test that the cube ray-caster's face/`(u,v)` inversion round-trips
  `face_frame` embeddings for pixel centers of every face.
- `cubarium demo --scene all --sink png --seconds 20` produces captures with
  nonzero pixels on more than one face; the orchestrator reviews them visually.

## M2 addition — `run`

```
cubarium run [--config world.toml] [--state state/] [--sink preview|shim|png|none]
             [--speed N] [--seconds N] [--seed N] [--fresh] [--telemetry FILE]
             [--addr ..] [--out ..] [--every N] [--scale N]
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--config` | built-in defaults | TOML `WorldConfig` (missing fields take defaults; unknown fields are errors) |
| `--state` | `state` | Directory for snapshots and the journal |
| `--sink` | `preview` | As for `demo`; `none` runs headless |
| `--speed` | `1` | Simulated seconds per wall second; `0` means as fast as possible (headless only) |
| `--seconds` | `0` | Stop after this much *simulated* time (0 = until closed; required for `png`) |
| `--seed` | from config | Overrides `config.seed` when creating a fresh world |
| `--fresh` | off | Ignore existing snapshots and create a new world |
| `--telemetry` | `<state>/telemetry.jsonl` | JSON-lines telemetry file (appended) |
| `--fields` | `<state>/fields.jsonl` | Field dump file, written only when `capacity.field_dump_seconds > 0` |
| `--events` | `<state>/events.jsonl` | Life-event log (births, deaths), written only when `capacity.event_log` is true |

Startup: unless `--fresh`, load the newest valid snapshot in `--state` (trying
older ones on failure, logging each reason); otherwise create a new world from
the config. The loaded world's config wins over `--config` except for
`capacity` and `weather.moving`, which are operational and may change.

Loop: the same clock as `demo`, with `World::step` per tick and `render_view`
→ canvas per frame. `--speed N > 1` runs N ticks per wall tick; `--speed 0`
loops without sleeping and without rendering. Every `checkpoint_seconds` of
simulated time and on clean shutdown, a snapshot is written by a worker thread
(atomic temp-file + rename + directory fsync, keep the newest 8, never remove
the newest valid one, disk errors logged without stopping the world). Telemetry
samples are appended every `telemetry_seconds`. Ctrl-C triggers a clean
shutdown with a final snapshot.

Presentation (`design/m2-world-spec.md` "Presentation"): producer substrate
with the seam-aware filter, detritus flecks, bodies from the view's lobes with
hue mapped to a low-saturation warm-to-cool ramp, brightness by mode and
feeding, juvenile bodies scaled by the view flag, short trails (12 segments,
3 s). Nothing else on the ambient image.

Required verification: a headless `run --sink none --speed 0 --seconds 600`
completes with nonzero population and a mass residual below `1e-6`; two runs
with the same seed produce identical telemetry hashes; a run interrupted by
`--seconds 120`, then resumed from its snapshot for 120 more seconds, matches
an uninterrupted 240-second run's state hash exactly; a corrupted newest
snapshot (flipped byte) is skipped and the previous one loads; the PNG sink
shows substrate and bodies.
