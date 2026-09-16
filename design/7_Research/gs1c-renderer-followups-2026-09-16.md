---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1c result: the renderer follow-ups on the live panel

Brief `design/handoffs/gs1c-opus-renderer-followups-2026-09-16.md`. Worktree
`.claude/worktrees/tachyon-screen`, branch `tachyon-screen`, six path-only
commits each checked with `git show --stat`. All five items are done.

**The panel is running the decided look.** `cubarium.service` presents
`ring:640x360 S=2 --gpu-art-scale 2` at **60.00 fps** by the daemon's own flip
count (7,200 flips in 120 s; its five-second lines read 59.8–60.2), at 0.615 CPU
core-seconds per second and 56 MiB, resumed at tick 11623 on the world it had
with its population of 122. It was **42.7 fps** when this package started.

(Two design-only commits from the LP-A package, `b7bf089` and `a718e95`, landed
on this branch while this one ran. They touch `design/**` only and nothing here
overlaps them.)

## 0. The measurements, and how they were taken

Board `root@192.168.68.68`, Adreno 643. Every run below is the installed binary
as the unprivileged `cubarium` user under `taskset -c 4-7`, through
`cube-screen-shim`'s frame socket, with `cubarium.service` stopped for the
duration and started again afterwards. The harness is `/root/gsmeasure.sh`
(label, ring, scale, seconds, extra flags), state under
`/var/lib/cubarium/state-test`, art at `/var/lib/cubarium/art`, `--fresh` each
time, so every rung is a 24-founder world of the same age — the service's own
world has 67 and is what §6's service numbers measure.

`cube-screen-shim` was never stopped, reconfigured or restarted; nothing was
written to sysfs or debugfs; no `apt`; the board was not rebooted.

## 1. Item 1 — 32-pixel art at 60 fps

`--gpu-art-scale 2` at 640×360 `S = 2`, 60 s each:

| | frames | fps | scene ms | GPU ms | submit+present ms |
|---|---|---|---|---|---|
| **before** (`1324c99`) | 2564 | **42.7** | 6.07 | 8.50 | 11.20 |
| the opaque box, the mask clamp, the early discard | 3255 | 54.2 | 5.36 | 4.79 | 7.86 |
| + the pad is the bend's reach | 3543 | 59.0 | 5.31 | 4.06 | 6.84 |
| + the instances written straight into the mapped buffer | 3580 | 59.6 | 5.48 | 4.00 | 6.19 |
| + the run's dead presenter observe removed | 3595 | **59.9** | 6.29 | 4.00 | 6.26 |

59.9 is the `--fps 60` cap: 3,595 of a possible 3,602 frames in 60.04 s. The
real ceiling is better measured without the panel's pacing, `--gpu-target
headless --fps 240`, 20 s:

| | fps at `--gpu-art-scale 2` | fps at 1 |
|---|---|---|
| before | 65.7 | 102.1 |
| after | **77.7** | — |

### Where the fill went, and what moved it

`--gpu-fill-profile` (new, a diagnostic) sums, over every instance of every
frame, the raster pixels its quad covers, the pixels a whole-tile quad would
have covered, and the pixels its *painted* texels cover. At 640×360 `S = 2`
with `--gpu-art-scale 2`:

| | quad Mpx/frame | × the 0.23 Mpx raster |
|---|---|---|
| a quad around the whole tile (GS-1b) | **13.13** | 57 |
| around the frames' **opaque box**, clipped by the mask | 5.46 | 23.7 |
| with the pad reduced to the bend's own reach | **3.54** | 15.4 |
| the painted texels themselves — the floor | 1.57 | 6.8 |

44 % of what the quads now rasterise is art that can paint. By layer, at the
end: GroundCover 1.45, Plants 1.77, Tall 0.27, Rain 0.01, Bodies 0.04 Mpx.

Four timestamp slots instead of two split the GPU's own frame:

| | uploads | world raster | present (1080×1920) |
|---|---|---|---|
| before | — | ~7.7 (inferred) | 0.83 |
| after | 0.01 | **3.17** | 0.83 |

The split is between render passes and not inside one, because this is a tiler:
everything recorded inside a render pass is deferred to that pass's binning and
resolve, so a timestamp between two draws would measure nothing.

### The five changes

1. **The quad is the art's opaque box.** `Atlas` measures each frame's
   `[x0, x1) × [y0, y1)` of texels with `α > 0`, and how many it paints, in the
   pass that already measured its extent. `SpriteInstance` (120 → 136 bytes,
   eleven vertex attributes) carries the union over the frames holding weight;
   `sprite.vert` builds its quad around that. A scratch frame — a rig part,
   rasterised fresh each frame — has no atlas measurement and keeps the whole
   tile.
2. **The mask clips the quad too.** A trunk strip reveals four rows of a
   sixteen-row tile; a growing crown a disc about its pivot. Both are a clamp on
   the box in the vertex shader, and `NO_MASK_FLOOR`/`NO_MASK_REVEAL`'s ±1e9
   make an unmasked stamp a no-op by arithmetic rather than by a branch.
3. **The mask is evaluated before the frames** in `sprite.frag`, so a fragment
   the mask rejects costs no `texelFetch`.
4. **The pad is the bend's own reach.** In nearest sampling a fragment paints
   iff `floor(src)` lands in the box, so `y` needs no pad at all and `x` needs
   `|amplitude|`, plus half a texel when the bend is rounded to a whole one and
   *nothing* when there is no bend — which is every ground tile and every rain
   mark. `--gpu-filter bilinear` keeps the whole texel of support on both axes
   it always had, because that mode is the fidelity comparison and its quad must
   not move.
5. **The run's own `ArtPresenter` is not observed when nothing draws from it.**
   This is the one that was not about fill. `--sink gpu` carries its own
   presenter and is handed the same `RenderView` at the same two instants, so
   `runner`'s was observed every tick and drawn never. `ArtPresenter::observe`
   on a 3,600-cell ring measured ~8.8 ms a tick on the board — 2.9 ms of every
   60 Hz frame, paid twice. `wants_pixels` is a property of the sink's kind and
   not of its connections, so the decision is made once at start-up.

### The picture did not move

Three synthetic goldens byte-identical through every commit
(`cargo test -p cubarium-gpu`), and every row of `gpu_fidelity` is the number
GS-1b published, to three decimals:

| | mean \|Δ\| | > 8 | worst |
|---|---|---|---|
| bilinear 320×180 S=1 f=0 | 0.501 | 0.53 % | 40 |
| bilinear 320×180 S=1 f=0.5 | 0.501 | 0.51 % | 40 |
| bilinear 640×360 S=2 | 0.499 | 0.54 % | 31 |
| bilinear 320×180, 40 ticks | 0.873 | 2.65 % | 129 |
| **nearest** 320×180 S=1 | 15.996 | 56.89 % | 206 |

The nearest row is the sensitive one here, because the tightened quad is the
nearest path and the goldens are a synthetic scene. It is also how the pad's
slack was sized: at an EPS of 0.001 source texels it read **16.003**, which was
one sprite row lost to the rasteriser's 8-bit sub-pixel grid snapping a quad
edge the wrong way. At 0.05 texels — at least 12 sub-pixel units at every stamp
scale this renderer draws, at most a tenth of a raster pixel — it is 15.996
again. EPS is slack for the sub-pixel grid, not for float error, and the comment
in the shader says so.

### What is left on item 1

* **Ground cover is 1.45 Mpx of the 3.54, and it is inherent to the look**: at
  `--gpu-art-scale 2` the 8-px lattice is spaced `8·S = 16` raster pixels apart
  while each tile covers `8·2S = 32`, so the tiles overlap fourfold. The box
  cannot help — a ground tile is painted corner to corner. Changing it would
  change the picture Wrysk chose.
* **The residual ~2.0 ms/frame** is the GPU sink's *own* per-tick
  `ArtPresenter::observe`. Halving the whole adapter walk is GS-1b's item 3 (the
  row-band split applied to the adapter), and it is what 960×540 needs anyway.
* The frame is now scene 5.5–6.3 ms of CPU against 4.0 ms of GPU: the next
  headroom is CPU, not fill.

## 2. Item 2 — the bend budgets on a ring

`ArtGeometry::bend_footprint()` is `min(9·S, Topology::max_local_radius)`. On a
cube that is `min(9, 32) = 9`, so the cube's budgets are the same `f64` they
were and every cube frame is unchanged — pinned bit for bit against the free
functions rather than against literals.

**Two constraints, and the brief names the larger one.** `9·S` is the shared
stamp budget: `Sprite::from_rgba` refuses art past it and `stamp_layers_bent`
clips its pixel walk at it (`sprite.rs`, "the *second* budget check site"), so an
amplitude admitted beyond it is one the CPU presenter does not visit.
`max_local_radius` is what `unfold_pixels` accepts — 32 on a cube, 90 at
`ring:320x180`, 180 at `ring:640x360`. GS-1 and this brief name the second as
"the room a ring has", and it is far the larger; but it is the **first** that
binds a stamp, and honouring only the second would admit bends the CPU clips and
the GPU does not, which is a difference between the two pictures rather than
more wind. What a ring actually gains is the `S`: the cube is pinned to `S = 1`
and the panel's rung is `S = 2`, so its bound is **18 px**.

### The per-clip budgets, in tile pixels

| asset | cube (and ring S=1) | **ring 640×360 S=2** | tip wanted | effective tip, cube → panel |
|---|---|---|---|---|
| glowcap | 2.399 | 14.876 | 0.12 | 0.120 → 0.120 |
| rootveil | 5.429 | 33.808 | 0.00 | — |
| lanternstalk | 3.228 | 13.808 | 0.45 | 0.450 → 0.450 |
| **tendrilfan** | **0.314** | 10.604 | 0.55 | 0.286 → **0.550** |
| umbrellafrond | 0.330 | 11.672 | 0 (spins) | — |
| bloomcrown | 2.072 | 12.900 | 0 (spins) | — |
| reedspire | 4.384 | 14.876 | 0.70 | 0.700 → 0.700 |
| spiretree (tall) | 1.314 | 11.097 | 0.90 | 0.900 → 0.900 |
| **glasscane** (tall) | **0.465** | 10.987 | 0.50 | 0.423 → **0.500** |
| vinecoil (tall) | 3.545 | 14.825 | 0 (shares its host) | — |

**The visible effect is exactly two species**: `tendrilfan` leans 0.286 → 0.550
px (+92 %) and `glasscane` 0.423 → 0.500 (+18 %). Everything else was never
clipped and does not move. The finding worth recording is the negative one:
**raising the bound to the ring's 180 px would change no amplitude at all**,
because every clip is now past the tip its species asks for and
`WIND_RESPONSE`'s 0.12–0.9 px is the binding constraint. That number is a
viewing-session choice, not a footprint, and it is where any further wind has to
come from.

A ring at `S = 1` keeps the cube's nine pixels and therefore the cube's budgets
exactly, so FW-5's two committed ring goldens are untouched.

The measurement is `Sprite::bend_headroom`'s own and not a second copy of its
criterion: a sprite's bound is the `Scale` it was built at, so a frame is rebuilt
from its own premultiplied texels at `Scale::new(bound/9)` and asked the same
question, and at 9 px the call short-circuits to the frame directly. Budgets are
re-measured in `relayout`, so a presenter built for a cube and fitted to a ring
gets the ring's room instead of carrying the cube's across — the runner's
`ArtPresenter::new(pack)` path had that hole.

**`--gpu-bend-substep` is now the default on a ring at `S ≥ 2`**
(`GpuSink::substep_default`), with `--no-gpu-bend-substep` to turn it off. At
`S = 1` a whole-texel bend and a sub-texel one are the same picture. The
fidelity test names `Some(false)` explicitly so its numbers stay the ones four
reports quote.

## 3. Item 3 — a hunter on the GPU

There is no command line for this. `World::start_hunter_trial` is the only route
a hunter enters a world (`world/hunter.rs`: "place **exactly one** founder of
the fixed lineage"), and `cubarium run` has no flag that reaches it — `--neural`
seeds ordinary trained animals, not predators. The fixture in
`crates/cubarium/tests/gpu_fidelity.rs` starts
`FixedHunterProfile::lanternjaw_trial(cfg)` at the centre of a `ring:320x180`
world before its 3,000 ticks.

**The rig is drawn, and in the right place.** Withholding the hunters from the
same view and diffing:

| | pixels the rig changes | bounding box |
|---|---|---|
| CPU presenter | 145 | `[154..166] × [86..104]` |
| `--sink gpu` | **144** | **the same box** |

One pixel apart, and that one is the sampler: the CPU's bilinear tap tints a
neighbour the GPU's nearest one leaves alone.

**It composites as the single-query rig does.** In `--gpu-filter bilinear`,
where nothing but the rig's own compositing is left between the two renderers:

| | mean \|Δ\| | > 8 | worst |
|---|---|---|---|
| the whole frame, with a hunter | **0.502** | 0.53 % | 40 |
| the same world's frame with no hunter | 0.501 | 0.53 % | 40 |
| **the rig's own 8 px neighbourhood** | **0.423** | 2.23 % | 18 |
| the whole frame, nearest (what the panel draws) | 15.989 | 56.90 % | 206 |

GS-1b's argument — `stamp_rig_scaled` composites the whole rig through one query
while the GPU draws part over part, but every Lanternjaw texel is opaque or
clear so the art cannot express the difference — now has a number behind it, and
the rig's own neighbourhood is *below* the frame's average rather than above it.

`crates/cubarium-gpu/tests/golden/hunter-{cpu,gpu}-320x180.png` is that frame
from both renderers, with a README line saying it is a **review pair and not a
golden**: nothing compares it automatically, and
`CUBARIUM_GPU_FIDELITY_DUMP=<dir>` re-records it.

## 4. Item 4 — FW-4's guard is lifted

`run_demo` names the demo's own shape (`Scenes::on(topology, scale, kind, seed)`
in place of `Scenes::new`), the fifteen-line `bail!` in `Demo::validate` is gone,
and the test that asserted the refusal asserts acceptance at both ladder rungs
and on the cube. `Scenes::new` *is* `Scenes::on(Cube, ONE, ..)`, so a cube demo
builds the fixture set it always did.

Verified: `demo --scene all --topology ring:320x180 --sink png --seconds 2` runs
(39 ticks, 120 frames, 60.0 fps, a written capture), and so do `patch`, `body`
and `vertex` at `ring:320x180` and at `ring:640x360 --world-scale 2`, and all
four on the cube. The ring `all` capture shows the four deposit centres FW-5
chose for a ring — the wrap, each rim and one in the open.

## 5. Item 5 — a viewer that does not cost the panel

`--gpu-web-rate <fps>` serves the same page on the same port as `--mirror-web`,
fed from the raster the GPU has already drawn: one `vkCmdCopyImageToBuffer` of
640×360×4 and one RGBA→RGB pass, **5.9 ms**, at the rate the operator picks.

Board, 640×360 `S = 2` with `--gpu-art-scale 2`, 40 s each, same binary:

| | frames | fps | served |
|---|---|---|---|
| no viewer | 2395 | **59.8** | — |
| `--gpu-web-rate 2` | 2395 | **59.8** | the frame fetched is 691,208 bytes, 100 % non-zero |
| `--mirror-web` | 661 | **16.5** | — |

Over 60 s with a client polling `/frame` and `/status` twice a second:
**59.83 fps and 100 frames served**, against 59.88 with no viewer at all — a
cost of 0.05 fps, where `--mirror-web` costs 43.

The two are refused together: one port, and an operator who asks for both is
asking for the cheap one and does not know the other exists. `--gpu-web-rate` is
also refused beside any sink but `gpu`, by name, and outside `0..=60`.
`--mirror-web` remains the only way to the **care** buttons, which are drawn
onto the CPU canvas this sink does not have, and the refusal and the docs both
say so.

## 6. The service, as it was left

`config/tachyon/cubarium.service` and the installed unit both carry Wrysk's
decision:

```ini
Environment="CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2"
```

**Both quotes matter, and in opposite directions.** W2 recorded half the rule.
`Environment=` splits its *own* line on whitespace into separate assignments, so
an unquoted `Environment=CUBARIUM_EXTRA_ARGS=--gpu-art-scale 2` sets the
variable to `--gpu-art-scale` alone, drops the `2`, and the service crash-loops
with `error: a value is required for '--gpu-art-scale'`. That happened on the
board and was fixed before the unit was left running. `$CUBARIUM_EXTRA_ARGS` in
the `ExecStart` is deliberately *un*quoted, because there systemd splits an
unquoted variable back into separate arguments.

With that in place the service is active, resumed, and presenting:

| what | value | GS-1b's reference at `--gpu-art-scale 1` |
|---|---|---|
| resume | `resuming /var/lib/cubarium/state/world-11623.cubw at tick 11623`, population 122 | — |
| the daemon's flip count, 120 s | 7,200 flips = **60.00 fps** | 59.9 |
| its five-second lines | 60.2 / 60.0 / 60.0 / 60.0 / 60.0 / 59.8 / 60.2 / 60.0 | — |
| CPU, whole process, 120 s | **0.615 core-seconds per second** | 0.668 |
| peak RSS | **56 MiB** | 57 |
| `cube-screen-shim` | never stopped, never reconfigured; `live`, `dma-buf client: 3 slot(s)` | — |

The CPU figure is *lower* than GS-1b's for the cheaper look, on a world with
five times the population, and the reason is item 1's fifth change: the run is
no longer observing a presenter nothing draws from. It is a two-minute sample
rather than W2's owed ten-minute one, and it is taken on the service's own
67-founder world at tick 11,623.

Two lines the driver prints at every start-up — `gbm_create_device(192): Info:
backend name is: msm_drm` and `Crit Edge BB#18` / `BB#164` — are Qualcomm's
shader compiler, not ours; they were there before this package and are
unchanged by it.

`--gpu-bend-substep` is **not** in `CUBARIUM_EXTRA_ARGS`, because item 2 made it
the default at this rung.

## 7. The tree

`cargo test --workspace --exclude cubarium-gpu --no-fail-fast` and
`cargo test -p cubarium-gpu` are green; `cargo build --workspace --all-targets`
has no error at every commit. Six commits, all `git commit -m … -- <paths>` and
each checked with `git show --stat`:

| commit | item |
|---|---|
| `cda60f2` | 4 — the demo fixtures run on a ring |
| `fd61086` | 1 — the fill, and the duplicate presenter |
| `c5994f9` | 2 — the bend budget against the world's own footprint |
| `cc45d39` | 3 — a hunter on the GPU, drawn and measured |
| `f52d2c1` | 5 — a viewer that costs 0.1 fps instead of 43 |
| `bf7e0b5` | the service: the 32-pixel look and the quoting rule |

## 8. What is left, in the order I would take it

1. **The adapter's walk is now the frame's largest term** — scene build 5.5–6.3
   ms against the GPU's 4.0. It is GS-1b's item 3 (FW-3's row-band split applied
   to the adapter rather than to the rasteriser), it is what 960×540 needs, and
   it is what would give the panel headroom instead of the cap.
2. **W2's boot-to-world check is still owed.** The board was power-cycled by
   hand this morning after W2's reboot hang and the no-reboot rule stands, so
   that check needs a deliberate decision about whether a plain reboot hangs this
   board at all — which belongs to the daemon's package, not this one. The
   steady state W2 also owed is taken above, at two minutes rather than ten.
3. **Ground cover is 41 % of the sprite fill** and it is the look, not a bug:
   the 8-px lattice is spaced `8·S` apart and each tile covers `8·2S`. If the
   panel ever needs the pixels back, that is where they are.
4. **`--sink gpu` still refuses a cube by name**, and the corner-cap handoff is
   still not ported. Unchanged from GS-1b, and both are cube-seam machinery a
   ring does not have.
5. **The wind's remaining headroom is `WIND_RESPONSE`, not the footprint.** Every
   clip of the shipped pack now has ten to thirty pixels of room and asks for at
   most 0.9. If the breeze still reads as too small on the panel, the tips are
   the knob, and they are a viewing-session number.
