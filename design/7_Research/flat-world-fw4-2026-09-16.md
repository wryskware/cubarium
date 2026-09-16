---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-4 result: the host by topology — `Output`, the sinks, the CLI, the care chain

Written by Opus on 2026-09-16 against
[the FW-4 brief](../handoffs/flat-world-fw4-opus-host-2026-09-16.md),
[FW-2's result](flat-world-fw2-2026-09-16.md),
[FW-3's result](flat-world-fw3-2026-09-16.md),
[the ring-world plan](../flat-world-plan-2026-09-16.md) §3, §4 and §9, and FW-6's
`ring_sinks.rs`, `ring_care.rs` and `ring_present.rs`. Worktree
`.claude/worktrees/tachyon-screen`, branch `tachyon-screen`.

## Summary

1. `cubarium run --fresh --topology ring:320x180 --sink png` produces a 320×180
   PNG of a live ring world, and `--topology ring:640x360 --world-scale 2`
   produces a 640×360 one. Both pictures are in §6.
2. The cube is **byte-identical**: the same seed, the same tick count, the same
   `f = 0` frame, plain presenter and art presenter, at 600 and 3,000 ticks —
   four PNGs, four frame hashes, all four the same as at `c51233f`, which is
   before FW-4 *and* before FW-5.
3. `FrameSink::submit` takes `Output<'_>`; the trait is still object-safe and
   `FanOutSink` still fans out. The shim sends format 2 strips, the PNG sink
   writes the raster at its own size, the web sink serves it behind the same
   8-byte sequence, and `/status` carries `topology`, `w`, `h` and `scale`.
4. The host `CareTarget` is `u16`. **A journal written before the widening
   replays onto the same cells**, proven against four record lines copied
   verbatim from a pre-FW-4 file rather than regenerated.
5. Four commits. `cargo test --workspace --exclude cubarium-gpu` is **1,672
   passed, 1 failed, 25 ignored**; the one failure is a `size_of` pin in FW-6's
   `ring_care.rs` that the widening the brief asks for makes impossible, and
   §8.1 gives the exact line.
6. Measured split in §7. On this desktop the ring at 320×180 costs `R = 5.81 ms`
   against the cube's 1.98, and at 640×360 (`S = 2`) 23.09 ms.

| commit | what |
|---|---|
| `ed6a0d9` | `Output`, the four sinks, the CLI, the runner, the care chain's shape |
| `0948f08` | the `u16` widening and the pre-widening journal proof |
| `7006f32` | the viewer's ring mode and `/status` |
| `2650346` | the CLI tests, the runner's end-to-end ring tests, one demo refusal |

## 1. The `Output` API

```rust
// crates/cubarium/src/sink/mod.rs
pub enum Output<'a> { Cube(&'a Frame), Ring(&'a Raster) }

impl<'a> Output<'a> {
    pub fn name(self) -> &'static str;     // "cube" | "ring"
    pub fn size(self) -> (u16, u16);       // 64x64, or the raster's
    pub fn bytes(self) -> &'a [u8];        // tightly packed RGB8
    pub fn frame(self) -> Option<&'a Frame>;
}

pub struct WorldShape { pub topology: Topology, pub scale: Scale }
impl WorldShape {
    pub const CUBE: WorldShape;
    pub fn new(topology: Topology, scale: Scale) -> WorldShape;
    pub fn name(self) -> &'static str;
    pub fn is_ring(self) -> bool;
    pub fn chart_size(self) -> (u16, u16);
    pub fn raster(self) -> Option<Raster>;
}

pub trait FrameSink {
    fn submit(&mut self, out: Output<'_>) -> Result<()>;   // the only changed signature
    fn observe_tick(&mut self, _tick: u64) {}
    fn observe_counts(&mut self, _population: usize, _neural: usize) {}
    fn should_quit(&mut self) -> bool { false }
    fn finish(&mut self) -> Result<()> { Ok(()) }
}

// new constructors, both defaulting to the cube so no existing caller moved
PreviewSink::new(scale, capture_dir, shape) -> Result<PreviewSink>   // the one that did
WebSink::with_world(port, note, source, care, shape) -> Result<WebSink>
CareService::for_world(epoch, journal, topology) -> CareService
Journal::open(dir, epoch, build, topology) -> Result<Journal>
CareTarget { face: u8, u: u16, v: u16 }
CareTarget::validate_on(&self, topo: Topology) -> Result<(), &'static str>
```

`Output` is `Copy`, because it is a pair of references: `FanOutSink` hands *the
same borrow* to every child rather than a per-child copy of 61,440 or 172,800
bytes, which is what its "no child can observe different bytes" invariant was
always about.

**`WorldShape` is carried beside the frames, not derived from them**, because
three places need it before the first frame exists: `/status` answers a page that
has not polled `/frame` yet, `PreviewSink` must refuse a ring *at construction*
rather than after a window is on screen, and the care routes validate a target
against a world the viewer may not have drawn yet.

### Per sink

* **`ShimSink`** — the same worker, the same newest-frame mailbox, the same
  backoff. Only the datagram changed: `CubeClient::send` (`encode_full`, format
  0) for a cube, `CubeClient::send_raster` (`encode_raster`, format 2, **all
  strips of one image under one `seq`**) for a ring. `sent` still counts images,
  not datagrams. The strip counts the wire actually carries, from the format's
  own arithmetic (`(MAX_DATAGRAM − HEADER_BYTES − STRIP_HEADER_BYTES) / row`):

  | image | row | rows/strip | datagrams | bytes |
  |---|---|---|---|---|
  | 320×180 | 960 | 68 | **3** (68, 68, 44) | 172,800 |
  | 640×360 | 1,920 | 34 | **11** | 691,200 |
  | 1920×1080 | 5,760 | 11 | **99** | 6,220,800 |

  The 320×180 row is exactly what FW-6's
  `a_320_by_180_raster_round_trips_through_format_2` asserts, independently.
* **`PngSink`** — a ring is written as its own `w×h` PNG, one PNG pixel per world
  pixel; a cube is the same 256×128 net. Both go through one encoder
  (`write_rgb8_png`), so a ring capture and a cube capture differ only in their
  size. `write_net_png` keeps its signature for the fourteen examples that call
  it.
* **`WebSink`** — `/frame` is the 8-byte little-endian render sequence then the
  image bytes: 61,440 for a cube, `w·h·3` for a ring. Before the first submit it
  answers a black one *of the right size*. `/status` gains four names:

  ```json
  {"world_tick":0,"population":0,"neural_animals":0,"render_seq":0,
   "frames_served":0,"topology":"ring","w":320,"h":180,"scale":1.0,
   "source":{…}}
  ```

  A cube host reports `"cube"`, 64, 64, 1.0 — the chart, not the net, because the
  page's cube mode does not use `w`/`h` and a lie there would be worse than a
  number it ignores.
* **`PreviewSink`** — refuses a ring at construction, naming `--sink web` and
  `--sink png`. Both halves of that window are pictures of a cube: the unfolded
  five-face net, and a ray-cast of an actual cube whose camera has no meaning on
  a flat world.

### The viewer

`index.html` picks its mode from the first `/status` answer that says `ring`,
hides the cube and net panels, and draws one `<canvas>` sized `w×h` in world
pixels, displayed at the largest whole scale that fits (`floor(min(1180/w,
620/h))`, at least 1) with `image-rendering: pixelated`. Nearest and integer by
construction. The three.js renderer is not merely hidden but **not run**: a WebGL
draw per frame for a panel nobody is looking at is exactly the cost that shows up
as a dropped world frame on the board.

Care aims the same way it always did: a click on the ring image is `(face 0, x,
y)` at 1:1, and the crosshair goes on its own overlay canvas in world pixels,
never into the frame bytes — so what the shim gets and what the page shows stay
one image. A host with no `topology` in `/status` is a pre-FW-4 cube host, which
is what this page already did, so its absence needs no special case.

### What FW-9 should assume cubarium emits

Per §9's FW-4 row, the layer set, declared: **cubarium emits RGB8 and nothing
else.** `Canvas::encode_raster` writes one `w·h·3` buffer, wire format 2 carries
no layer id, and no auxiliary channel (emissive, water mask, height/stratum,
rain) exists anywhere in the host today. Adding one is a change to `cube-proto`'s
strip header and to `cubarium-render`'s encoder, neither of which is FW-4's, and
FW-9 should plan on introducing the first layer itself rather than on turning one
on.

## 2. The CLI and TOML surface

```
--topology cube | ring:WxH      # e.g. ring:320x180; run and demo
--world-scale S                 # ring only; a cube is pinned to 1
```

On `run` both are `Option`, apply to a **new** world only, and override the
config file exactly as `--seed` does. The TOML keys are FW-2's and unchanged:

```toml
topology = "Cube"                              # or, omitted entirely
topology = { Ring = { w = 320, h = 180 } }     # capital R; FW-2 §1
world_scale = 2.0
```

`ring:WxH` is the panel's own spelling rather than serde's, and
`TopologyArg: Display` round-trips it so a refusal can quote back what was typed.
Refusals, all before any world is touched:

| what | why |
|---|---|
| `--sink preview` with `--topology ring:*` | both halves of the window are cube pictures; names `--sink web` and `--sink png` |
| `--world-scale ≠ 1` without a ring | the cube's 32-pixel local radius and 9-pixel stamp budget are proofs about a 64-pixel chart |
| `--topology`/`--world-scale` with `--require-resume` | one asks for a world to create, the other for one that exists |
| a ring `Topology::validate` refuses (e.g. `ring:321x180`) | the surface contract's own message, quoting the flag |
| a resume whose snapshot topology differs by name | §3 below |
| `--neural` on a ring | §8.3 |
| `demo --scene patch\|all` on a ring | §8.2 |

**The resume refusal is load-bearing and was nearly silent.** `merge_operational`
copies `capacity` and `weather.moving` out of a `--config` and *nothing else*, so
a `--config` or `--topology` naming a different surface would have been ignored
without a word — the operator would have believed they had asked for a ring and
been watching a cube. It now fails by name, quoting both shapes and the snapshot
path, and pointing at `--fresh` with a state directory of its own. A resume that
names the *right* topology carries on from the tick it stopped at; one that names
none takes the snapshot's. All four are tested
(`runner::tests::a_resume_refuses_a_topology_the_snapshot_does_not_have`).

## 3. The journal compatibility proof

`CareTarget.{u,v}` are `u16`. **No wire changed shape.** Both readers — the
journal's `parse_record` and `POST /care`'s `parse_care_request` — have always
read `u` and `v` as JSON integers; only the accepted range grew, from
`u8::try_from` to `u16::try_from`. The three accepted record kinds keep their
discriminators and their key order, so `accepted`, `accepted_dose_v1` and
`accepted_apex_v1` are the same bytes they were.

The proof is `care::journal::tests::a_journal_written_before_the_widening_still_replays_onto_the_same_cells`.
Four lines, **copied verbatim from a pre-FW-4 file and never regenerated by this
build**:

```
{"rec":"epoch","epoch":"stamp-0","build":"0.1.0+pre-widening"}
{"rec":"accepted","seq":1,…,"kind":"feed","target":{"face":0,"u":0,"v":0}}
{"rec":"accepted_dose_v1","seq":2,…,"target":{"face":4,"u":63,"v":63},"dose_permille":1500}
{"rec":"accepted_apex_v1","seq":3,…,"targets":[{"face":1,"u":12,"v":34},{"face":3,"u":52,"v":8}]}
```

Opened against a cube world, all three accepted records recover: the same
targets, including `(4, 63, 63)` — the largest target the old byte could hold and
the one a narrowing mistake would have wrapped to zero — the same dose, the same
client, the same second target, `truncated_bytes() == 0`, and `replay_plan`
returning `[(1,100), (2,140), (3,180)]` in order.

Beside it, `a_ring_target_past_the_cubes_63_round_trips_and_a_cube_refuses_it`:
`(face 0, u 300, v 170)` is written on a 320×180 ring, the file literally
contains `"target":{"face":0,"u":300,"v":170}`, it reopens to the same target —
and the *same file* opened against a cube world is refused by name, because a
journal is validated against the world it belongs to and nothing migrates one
surface onto another.

### The rest of the care chain

* `CareTarget::validate_on(topo)` bounds a target by `topo.extent(face)` and
  `topo.has_chart(face)` instead of the literal 64 and 4. `validate()` remains as
  the no-argument spelling, defined as `validate_on(Topology::default())` — the
  cube — for the callers with no world in hand.
* `CareShared` carries the world's topology. The apex draw ranges over *this*
  world's charts and pixels instead of 5 and 64 (it would otherwise put every
  apex in the ring's leftmost 64 columns); it still consumes exactly three
  draws per target in the same order, so the cube's placements are unchanged.
* `Journal::open` takes the topology; every recovered target is range-checked
  against the world it belongs to.
* `care_effects.rs` unfolds on the canvas's own surface, caps `QUERY_RADIUS = 8`
  at `topology.max_local_radius()` so a small ring clamps instead of panicking,
  and checks a receipt's `cells` against the world's runtime cell count instead
  of `CUBE_CELL_COUNT`.
* `POST /care` validates against the shape the viewer is showing:
  `a_care_target_is_validated_against_the_world_the_viewer_is_showing` accepts
  (200, 120) and (319, 179) on the ring, refuses (320, 90), (160, 180) and
  `Face::Top`, and refuses the same (200, 120) on a cube host whose own (32, 32)
  still works.

## 4. The cube regression

Deterministic by construction — no clock, no wall time: a fixed seed, a fixed
tick count, `f = 0`, one `Canvas::encode`, one `net_rgb8`, one PNG. Run in two
isolated `git archive` copies with their own target directories, one at
`c51233f` (the branch head before FW-4 *and* before FW-5) and one at `7006f32`.

| presenter | ticks | frame FNV-1a | PNG FNV-1a | before == after |
|---|---|---|---|---|
| plain (M2) | 600 | `237446f4571abbaa` | `10cdd4419727b9c8` | **identical** |
| plain (M2) | 3,000 | `dbda2742ea357364` | `c850a8ce54f0447b` | **identical** |
| art (`assets/atelier`) | 600 | `e167c7d95e6f7544` | `fccd4ff674642154` | **identical** |
| art (`assets/atelier`) | 3,000 | `bd54b0a8e3ab3004` | `4b0842c5d2732cf4` | **identical** |

`cmp` on all four PNG files reports no difference. Because the baseline predates
FW-5 as well, this is joint evidence: neither package moved the cube image.

The harness is `crates/cubarium/examples/fw4_cube_golden.rs`, a scratch tool
written into both copies and **not committed**, on FW-3's pattern.

## 5. Verification

* `cargo test --workspace --exclude cubarium-gpu --no-fail-fast`: **1,672
  passed, 1 failed, 25 ignored** (FW-2/FW-3 recorded 1,655/0/25 at their freeze).
  The one failure is §8.1.
* FW-6's `ring_sinks.rs`: **6 passed, 0 failed**, unchanged.
* FW-6's `ring_present.rs`: **3 passed, 0 failed, 1 ignored**, unchanged. (The
  ignored one is FW-5's.)
* FW-6's `ring_care.rs`: **4 passed, 1 failed**, unchanged — §8.1.
* `cargo clippy -p cubarium --all-targets`: in the files FW-4 owns, exactly the
  four warnings `c51233f` already had, all in `runner/mod.rs` (two
  `empty_line_after_doc_comments`, one `expect` after `is_some`, one collapsible
  `if`). One new lint was raised and answered in place:
  `clippy::large_enum_variant` on `Command`, because `Demo` and `Run` both grew
  and crossed the 200-byte threshold. Allowed with its reason rather than
  silenced: a parsed command line exists once per process, and the indirection
  clippy suggests is not available, because `#[derive(Subcommand)]` needs each
  variant's field to implement `clap::Args` and `Box<Run>` does not.
* Every run above was made in an isolated `git archive` copy with its own target
  directory, because FW-5 was editing `present.rs` and `art_present/**` in the
  shared working tree throughout (FW-3's §8 process note applies again).

## 6. The captures

`cubarium run --fresh --seed 1 --sink png --speed 5 --seconds 20`, plain M2
presenter, release build.

| command | PNG |
|---|---|
| `--topology ring:320x180` | `final.png`, IHDR **320×180**, 8-bit truecolour |
| `--topology ring:640x360 --world-scale 2` | `final.png`, IHDR **640×360** |
| `demo --sink png --scene body --topology ring:320x180` | `final.png`, IHDR **320×180** |
| `demo --sink png` (cube, unchanged) | `final.png`, IHDR 256×128 |

Both ring captures show the world: the night floor, the producer ramp reading
brighter toward the bottom, and the 24 founders as coloured bodies. The 640×360
`S = 2` capture shows the effect FW-2 §4 predicted and is the clearest picture of
it: the habitat is bit-identical to the 320×180 world, but every organism length
in `WorldConfig` is in pixels, so the bodies are *half the size relative to the
world* and the animals cross half as much world per second.

The same runs with `--art assets/atelier` complete without panicking but draw the
art in the top-left 64×64 and repeat bands across the rest — "whatever it can
today", which is FW-5's remaining work and not a defect in the sink path.

The runner test `a_fresh_ring_world_renders_and_captures_at_its_own_size` makes
the same statement without a human: it runs both scales through the whole loop
and reads the PNG's own IHDR, where a cube capture would say 256×128.

## 7. The measured split

Desktop (Ryzen 9 9950X3D), **one pinned core** (`taskset -c 30`), a shared and
busy machine, release build in an isolated copy. Seed 1, warmed 3,000 ticks —
FW-3's world — then medians over `n = 300` frames (plain) or `n = 200` (art). The
four phases are exactly the runner's: `World::step`, `presenter.draw`,
`Canvas::encode`/`encode_raster`, and `FrameSink::submit` into a `ShimSink`.
Milliseconds.

### Plain (M2) presenter — the one that is correct on a ring today

| world | cells | canvas px | tick | draw | encode | sink | **R = draw+encode+sink** |
|---|---|---|---|---|---|---|---|
| cube | 1,280 | 20,480 | 0.133 | 1.935 | 0.039 | 0.001 | **1.975** |
| ring 320×180 `S=1` | 3,600 | 57,600 | 0.274 | 5.644 | 0.166 | 0.002 | **5.813** |
| ring 640×360 `S=2` | 3,600 | 230,400 | 0.292 | 22.405 | 0.670 | 0.012 | **23.087** |

A second pass on the cube read 0.136 / 1.968 / 0.039 / 0.001 — about 1.7 %, which
is the machine's noise floor and the precision these numbers deserve.

* **`encode` is linear in pixels and cheap**: 1.9, 2.9 and 2.9 ns per pixel.
  FW-3's sRGB table is what makes the ring's 230,400-pixel encode 0.67 ms instead
  of the ~9 ms the old `powf` would have cost.
* **`sink` is one clone into the newest-frame mailbox** — 61,440, 172,800 and
  691,200 bytes for 0.001, 0.002 and 0.012 ms. The strip encoding and the three
  or eleven `sendto` calls happen on the shim worker's thread and are not in this
  budget; they do occupy a core, which matters on four A78s.
* **`draw` is everything**, 97 % of `R` at every size, and it scales with canvas
  pixels rather than cells: 2.81× the cube's pixels costs 2.92× its draw, and 4×
  those pixels costs 3.97× again.

### Art presenter (`assets/atelier`)

| world | tick | draw | encode | sink | R |
|---|---|---|---|---|---|
| cube | 0.171 | 7.028 | 0.039 | 0.002 | 7.069 |
| ring 320×180 `S=1` | 0.291 | 5.013 | 0.166 | 0.003 | 5.182 |
| ring 640×360 `S=2` | — | **panics** | — | — | — |

**Read these with care and do not pick `--fps` from the ring row.** The art
presenter does not draw a correct ring image yet (§6), so its ring cost is the
cost of drawing the wrong thing; and at `S = 2` it panics in
`cubarium-render/src/cells.rs:105` with a `PixelCells` table built at the cube's
20,480 pixels. Both are FW-5's. The cube row is also not comparable to FW-3's
5.19 ms: FW-3 pinned a quiet core and interleaved three repeats, and a second
pass here read 8.110, so the honest statement is 7–8 ms on a busy machine.

### The load arithmetic, and what it says about `--fps`

Against §6's `20·tick_ms + fps·R ≤ 1000`, desktop, plain presenter, no band
split:

| world | 60 fps load | verdict | one-core ceiling |
|---|---|---|---|
| cube | 121 ms (12 %) | clears | ~505 fps |
| ring 320×180 `S=1` | 354 ms (35 %) | clears | ~171 fps |
| ring 640×360 `S=2` | 1,391 ms (139 %) | **fails 1.39×** | ~43 fps |

**Projected to one A78**, using the ratio FW-0 and FW-3 measured on this code
(an A78 is 1.72–1.74× slower than this desktop core) — no device access in this
package, so this is arithmetic, not a measurement:

| world | projected `R` | 60 fps load on one A78 | with FW-3's 1.94× four-band split on `draw` |
|---|---|---|---|
| ring 320×180 `S=1` | ~10.1 | ~615 ms (61 %) | ~360 ms (36 %) |
| ring 640×360 `S=2` | ~40.2 | ~2,420 ms — fails | ~1,270 ms — still fails; ~47 fps |

So: **`--fps 60` for a 320×180 ring at `S = 1`**, which clears on a single A78
with the M2 presenter and does not need the band split at all; **640×360 at
`S = 2` is a 30 fps proposition on the CPU** even with four bands, which is what
GS-1's Vulkan renderer exists for. Both rows must be re-measured once FW-5's ring
art lands, because the shipped world runs `--art` and the art presenter's ring
cost is not yet a real number.

## 8. What §3, §4 and the briefs had wrong, or left unsaid

### 8.1 FW-6's `ring_care.rs` pins the shape FW-4 is asked to change

`the_host_care_target_is_still_the_cube_chart` ends with

```rust
assert_eq!(std::mem::size_of::<HostCareTarget>(), 3, "face + u8 + u8");
```

which the widening the brief's deliverable 3 asks for makes impossible: the type
is 6 bytes. The test's own comment two lines above says so — "The type itself is
the thing FW-4 has to widen: `u8` cannot even name pixel 200 of a 320-pixel ring
row" — so this is a pin of the *before* state, not a claim FW-4 violates.

Everything else in that test, which is what its docstring actually promises
("every assertion here about a **cube** target must still hold afterwards"),
passes unchanged: `(0,0,0)` and `(4,63,63)` validate, `face 5`, `u = 64` and
`v = 64` are refused. The shape of `validate()` was chosen to keep them: it is
the no-argument spelling, defined against `Topology::default()`. The alternative
— making `validate` take a topology — would have failed to *compile* that file
and taken the whole integration-test target down with it.

**This is one line for FW-6 to re-record**, `6` for `3` and a renamed message.
FW-4 did not edit it.

### 8.2 `demo --scene patch` panicked on a ring; §3 does not mention the fixtures

§3's table is about sinks and the canvas, and the FW-4 brief says "the presenter
is asked to draw whatever it can today". For `run` that is true — the M2
presenter draws a real ring image (§6). For `demo` it is not: `Scenes` builds the
patch fixture's `ScalarField` at the cube's **1,280** cells and deposits into it
through `Topology::Cube`, while the substrate pass now reads the cells of the
canvas it is drawing on (FW-5's `f83f42a`). On a 3,600-cell ring those disagree
and the pass indexes past the field — `index out of bounds: the len is 1280 but
the index is 1280`.

A panic is not an acceptable answer to a command line, so `--scene patch` and
`--scene all` are refused on a ring by name, pointing at `--scene body`/`vertex`
(which carry no field and do draw) and at `run --sink png`. `scene.rs` is FW-5's
file, so this is a refusal and not a repair. Before `f83f42a` the same command
would have drawn the cube fixture's field onto the ring's first 64×64 instead of
panicking, so the regression is FW-5's to close if the fixtures should be
ring-aware at all.

### 8.3 `--neural` is a cube control and nothing said so

Every constant in `seed_neural_animals` is a cube's: one copy per face in `Face`
index order, aimed at cell (8, 8) of a 16×16 chart, searched outward inside
`CELLS_PER_FACE_EDGE`. None of them means anything on a ring, and honouring them
would have seeded the whole cohort into one corner of it while looking like it
worked. Refused by name. Neither §3 nor §9 mentions `--neural`.

### 8.4 `run`'s default sink refuses a ring, which is correct but surprising

`--sink` defaults to `preview`, so `cubarium run --fresh --topology ring:320x180`
with nothing else fails at argument validation. That is the plan's rule working,
but it means the shortest ring command line is an error; the refusal names both
alternatives for exactly that reason. Worth a line in the README when W2 writes
the device's command line.

### 8.5 §3's "`/status` gains `topology`" needs three more names

`w`, `h` and `scale`. A page cannot size a canvas or a `/frame` body from a word.
`scale` is there because the viewer's caption and the future panel layout both
want to say what world scale is being shown, and because it is free.

### 8.6 §3's `PngSink` row says "`net.rs` untouched" and it is

`net.rs` has no change in this package at all. What moved is `png.rs`, which now
has one encoder both topologies go through.

### 8.7 A ring world's `Raster` never round-trips through a `Frame`

Worth stating because the plan's table reads as if a sink might convert: it never
does. `Output` is matched, not coerced, `PngSink` keeps whichever it was given
for `final.png`, and `WebSink::newest()` returns `None` on a ring rather than a
black cube frame.

## Commands

```
# the isolated copies (FW-5 was editing the shared tree throughout)
git archive c51233f | tar -x -C <base>;  git archive HEAD | tar -x -C <fw4>

# the cube regression, deterministic, in each copy
cargo build --release -p cubarium --example fw4_cube_golden
./target/release/examples/fw4_cube_golden 600  <out>
./target/release/examples/fw4_cube_golden 3000 <out> assets/atelier
cmp <base>/cube-3000.png <fw4>/cube-3000.png

# the split
taskset -c 30 ./target/release/examples/fw4_split cube         1 3000 300
taskset -c 30 ./target/release/examples/fw4_split ring:320x180 1 3000 300
taskset -c 30 ./target/release/examples/fw4_split ring:640x360 2 3000 300

# the captures
./target/release/cubarium run --fresh --seed 1 --topology ring:320x180 \
    --sink png --seconds 20 --speed 5 --every 30 --state <s> --out <o>
./target/release/cubarium run --fresh --seed 1 --topology ring:640x360 \
    --world-scale 2 --sink png --seconds 20 --speed 5 --every 60 --state <s> --out <o>
./target/release/cubarium demo --sink png --scene body --topology ring:320x180 \
    --seconds 2 --every 30 --out <o>

cargo test --workspace --exclude cubarium-gpu --no-fail-fast
cargo clippy -p cubarium --all-targets
```

## Addendum, same day: the art presenter's ring numbers, after FW-5's second commit

FW-5 landed `9d8d8ec` ("the art presenter is laid out by the world's own raster")
while this report was being written, which changes two statements above. Both are
left standing as what was true at FW-4's own commits, and corrected here.

1. **The art presenter now draws a correct ring image at both scales.** §6's "the
   art in the top-left 64×64 and repeat bands" and §7's "panics in
   `cells.rs:105`" are both fixed by that commit. A 320×180 `--art
   assets/atelier` capture shows the stratified world: canopy along the top,
   stalks and columns through the middle, litter along the floor.
2. **The cube regression still holds at `c336e20`**, re-run against the same
   `c51233f` baseline: all four PNGs byte-identical, all four frame hashes
   unchanged. That is now joint evidence for FW-4 and *both* FW-5 commits.
3. `cargo test --workspace --exclude cubarium-gpu --no-fail-fast` at `c336e20`:
   **1,672 passed, 1 failed, 25 ignored** — the same single `size_of` pin of §8.1
   and nothing else.

The measured split, art presenter, same conditions as §7 (`taskset -c 30`, seed
1, warmed 3,000 ticks, `n = 200`, `n = 150` at `S = 2`), milliseconds:

| world | tick | draw | encode | sink | **R** |
|---|---|---|---|---|---|
| cube | 0.148 | 6.250 | 0.036 | 0.002 | **6.288** |
| ring 320×180 `S=1` | 0.313 | 16.481 | 0.166 | 0.004 | **16.652** |
| ring 640×360 `S=2` | 0.316 | 24.416 | 0.659 | 0.019 | **25.093** |

The `S = 2` row is the surprise and it is good news: 4× the pixels costs **1.51×**
the draw, not 4×, because the art presenter's cost is dominated by per-slot work
over the same 3,600 cells and only the composite grows with the raster. That is
the same "the limit is the presenter, not the pixels" finding FW-3 §5 reached
from the band split, seen from the other side — and it means the choice between
320×180 and 640×360 is much cheaper than §6's `∝ S²` law predicts for the art
image.

### What this says about `--fps`, which is the number the board needs

Desktop, one core, `20·tick + fps·R ≤ 1000`:

| world | 60 fps load | one-core ceiling |
|---|---|---|
| cube | 380 ms (38 %) | ~158 fps |
| ring 320×180 `S=1` | 1,005 ms (**100.5 %**) | ~59.7 fps |
| ring 640×360 `S=2` | 1,512 ms (151 %) | ~39.6 fps |

Projected to one A78 at FW-0/FW-3's measured 1.73× (arithmetic, not a
measurement — no device access in this package), and with FW-3's **measured**
1.94× four-band split applied to `draw` only (`encode` and `sink` stay serial):

| world | one A78, no split | four A78s, FW-3's split | verdict |
|---|---|---|---|
| ring 320×180 `S=1` | 1,738 ms, ~34 fps | **911 ms (91 %), clears 60 fps** | 60 fps needs the split, with 9 % to spare |
| ring 640×360 `S=2` | 2,613 ms, ~23 fps | 1,387 ms, ~43 fps | **30 fps**, or GS-1's Vulkan renderer |

**Recommendation for W2's `--fps`**: `--fps 60` at `ring:320x180`, `S = 1`, with
FW-3's four-band presenter; `--fps 30` if 640×360 at `S = 2` is chosen on the CPU
path. 91 % of the wall second is tight enough that the first row must be
confirmed on the board before it is written into the unit file — the shim
worker's own thread also wants a core, and these numbers are one pinned desktop
core scaled by a ratio.
