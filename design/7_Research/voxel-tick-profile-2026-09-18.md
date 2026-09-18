# The voxel tick, measured — 2026-09-18

First performance pass on the voxel side. **Nothing was fixed and no rule or number moved**:
this is one commit of measurement code behind a `profile` cargo feature, one bench harness,
one `perf record`, and a ranked plan. Every number below is from this machine, one desktop
core, `--release` with `debug = 1`.

```text
cargo run --release -p cubarium-voxel-fauna --features profile --example bench \
  -- [ticks] [grazers] [warmup_s] [seed] [sample_every]
```

**The condition, declared.** The generated default world — 128 × 48 × 24 voxels of 0.25 m,
147,456 cells, 3,072 columns, `water_substeps` 4, `seed` 1, `noise_seed` 0 — with the
harness rain of 2e-4 m/s and the outlet open, conditioned 50 s world-only, then eight
founders of each of the **six** species at half their own `wood_max` on their own
gate-passing skyline faces, eight declared logs of 1.0 laid first so glowcap has a
substrate, and 2,000 coupled ticks measured. Two arms: 0 grazers and 4 (which became 28 by
the end of the window). Founders are placed by `grazed.rs`'s strided gate-passing rule and
not by `two_producers.rs`'s `Habitat` table — same six species, same counts, same logs, the
same *scale* of world and not the same sites, because that table lives in the flora crate's
harness. Nothing here observes the ecology, so the placement only has to load the tick
honestly.

**How the phases are timed.** A `profile` feature in `cubarium-voxel` (forwarded by the
flora and fauna crates' own `profile` features) wraps each phase call in an `Instant`
timer accumulating nanoseconds into a global table, and adds work counters. With the
feature off every call site is `#[cfg]`-ed away and the module does not exist. The timers
cost ~40 ns per phase per tick — 1e-3 of today's tick, 2e-2 of the 50 µs target, and the
same for every phase. A nested phase's time is inside its parent's (`substep loop` contains
infiltrate, fall and equalize). The active-set counts and the harness-observer timings are
sampled every 100 ticks **outside** the tick timing, and `sample_every` 0 turns them off for
a clean `perf` run.

## Per-phase wall time per tick

4-grazer arm (2,000 ticks, sim wall 23.44 s = **11.72 ms/tick = 85 ticks/s = 4.3× real
time**; the 0-grazer arm is 9.15 ms/tick = 109 ticks/s = 5.5× real time). Wrysk's ~15 ms
figure is the same tick with a harness on top.

| phase | µs/tick | % of tick | calls/tick |
| --- | --- | --- | --- |
| **World::step (total)** | 7709 | 65.8 | 1 |
| rain | 64.6 | 0.6 | 1 |
| evaporate | 0.02 | 0.0 | 1 |
| **substep loop (total)** | 7388 | 63.0 | 1 |
| infiltrate | 519 | 4.4 | 4 |
| fall | 1654 | 14.1 | 4 |
| **equalize** | **5215** | **44.5** | 4 |
| drain | 137 | 1.2 | 1 |
| water_table | 120 | 1.0 | 1 |
| spring | 0.04 | 0.0 | 1 |
| outlet | 0.03 | 0.0 | 1 |
| **Flora::step (total)** | 45.2 | 0.4 | 1 |
| prune_unsupported | 0.74 | 0.0 | 1 |
| sky cache | 0.06 | 0.0 | 1 |
| drown | 1.48 | 0.0 | 1 |
| light | 7.13 | 0.1 | 1 |
| drink | 22.2 | 0.2 | 1 |
| feed (substrate) | 8.25 | 0.1 | 1 |
| grow | 3.00 | 0.0 | 1 |
| decompose | 1.44 | 0.0 | 1 |
| seed_bank | 0.14 | 0.0 | 1 |
| propagate | 0.37 | 0.0 | 1 |
| **Fauna::step (total)** | 3964 | 33.8 | 1 |
| terrain | 0.14 | 0.0 | 1 |
| maintenance | 0.12 | 0.0 | 1 |
| **sense** | **3961** | **33.8** | 1 |
| act | 1.86 | 0.0 | 1 |
| births | 0.12 | 0.0 | 1 |
| deaths | 0.57 | 0.0 | 1 |

Two phases are the tick: **equalize 45 %** and **fauna sense 34 %**. The whole plant layer,
six species and 48 stands, is **0.4 %** — 45 µs. Every ecological rule this project has
argued about for six rounds costs less than one percent of the tick.

## `perf record` — top symbols, tick only

`perf record -F 999` over 800 ticks with 4 grazers and the samplers **off**
(`sample_every` 0), self time, no children:

| % | symbol |
| --- | --- |
| 35.8 | `cubarium_voxel::water::step` (equalize, fall and infiltrate inline into it) |
| 25.0 | `BinaryHeap<Reverse<(usize, u8, usize)>>::pop` |
| 17.0 | `cubarium_voxel::water::push_neighbours` |
| 5.3 | `VoxelView::is_support` |
| 2.9 | `slice::sort` of `equalize`'s seed list |
| 2.8 | `FloraView::reachable_foliage` |
| 1.8 | `cubarium_voxel_fauna::step::step` |
| 1.4 | `VoxelView::water_depth_m` |
| 1.1 | `cubarium_voxel::water::transfer` |
| 0.9 | `cubarium_voxel::water::neighbours` |
| 0.6 | `round`, 0.6 `floor`, 0.6 `sky_visibility`, 0.3 `RawVecInner::finish_grow`, 0.2 `cfree` |

That is every symbol above 0.15 % of the process; the top 30 has nothing else in it — the
remainder is libc allocator internals and unresolved `libc.so.6` addresses, none over
0.26 %. `perf.data` is not committed (0.65 MB of machine-local samples); the command is
`perf record -F 999 -- target/release/examples/bench 800 4 50 1 0`.

**Read it as:** the region flood fill is ~68 % of the process (`water::step`'s inlined
bodies plus `BinaryHeap::pop` plus `push_neighbours`), `is_support` and `water_depth_m` are
the animal layer's candidate-face walk, `reachable_foliage` is its scoring, and the
allocator shows up because `equalize` allocates eight `Vec`s the size of the grid **per
substep** — about 2 MB of fresh zeroed memory four times a tick.

## The work each phase was handed, and the sets it could have iterated instead

Per tick, 4-grazer arm; the active sets are means of 20 samples.

| count | per tick | | active set | size | of the 147,456-cell grid |
| --- | --- | --- | --- | --- | --- |
| fall: cells visited | 577,536 | | cells that are not solid | 78,698 | 53.4 % |
| infiltrate: cells visited | 577,536 | | cells holding free water | 19,524 | 13.2 % |
| equalize: cells scanned for seeds | 589,824 | | of those, with room below | 16,748 | 11.4 % |
| equalize: cells holding free water | 77,930 | | cells holding pore water | 11,842 | 8.0 % |
| equalize: regions filled | 1,039 | | of those, over field capacity | 2,672 | 1.8 % |
| equalize: cells inside those regions | 87,652 | | sites with a `Ground` | 60.6 | 0.04 % |
| water_table: cells scanned | 147,456 | | stands | 48 | 0.03 % |
| drain: cells scanned | 147,456 | | animals | 28.1 | 0.02 % |
| root/mycelium box voxel reads | 4,182 | | | | |
| germination predicates evaluated | 184 | | | | |
| sky-visibility rays cast | **0** | | | | |
| `reachable_foliage` queries | 2,849 | | | | |
| candidate faces scored | 2,821 | | | | |

The grid is scanned **1.9 million cell-visits per tick** to move water that lives in 19,524
cells. The sky cache already does the right thing: zero rays in 2,000 ticks, because
`terrain_version` never moved.

## What the harness observers cost, which is not the tick

| observer | per observation | in ticks |
| --- | --- | --- |
| eligible-set scan: 3,072 skyline columns × 6 species = 18,432 germination predicates | **140.7 ms** | 12 ticks |
| identity tracking: one `reachable_foliage` per animal plus the foliage of every stand in reach | 16 µs | 0.001 |

7.6 µs **per predicate** — a root-box walk plus a sky-visibility ray each. `two_producers`
and `replacement` run that scan once per observation, not per tick, so it is not in the tick
figure; but a study that observes every 100 ticks is paying 12 % on top of its own run, and
one that observed every tick would be paying 12×. Identity tracking is free.

## The plan, as an entity/component/system split (issue #17)

Wrysk is right about the shape. **The dense grid stays a resource** — `material`, `free`,
`pore` as they are, O(1) indexed, because material is static and every rule needs random
access. What changes is that **systems stop iterating the resource**. The sparse things
become sets of entities with components, maintained incrementally by the systems that dirty
them, and each phase iterates the smallest set that can possibly change:

| phase | set it should iterate | size today | grid visits today |
| --- | --- | --- | --- |
| rain | sky cells (one per column, static until terrain changes) | 3,072 | 3,072 column searches |
| evaporate | open-water cells ⊂ wet set | 0 at defaults | 3,072 column searches |
| infiltrate | wet cells with a permeable cell below | ≤ 19,524 | 577,536 |
| fall | wet cells with room below | 16,748 | 577,536 |
| equalize | **wet regions** (see below) | 260 regions / substep | 589,824 + 87,652 |
| drain | pore cells over field capacity | 2,672 | 147,456 |
| water_table | unfilled cells under the table | ≈ band × plane | 147,456 |
| spring, outlet | one cell each | 1 | already O(1) |
| flora: all phases | stands, ground sites, seed banks | 48, 61 | already O(active) |
| fauna: sense | animals × candidate faces, with **stands indexed by column** | 28 × ~100 | 2,849 × 48 stand tests |
| fauna: rest | animals | 28 | already O(active) |

**Three phases are O(grid) by design, not by accident**, and each needs its own answer:

- **Region finding** (`equalize`). Connected-void regions are a property of the *terrain*,
  which changes only when `terrain_version` does. Today they are rediscovered by a
  `BinaryHeap` flood fill four times a tick, 1,039 regions and 87,652 region-cells per tick.
  An incremental version computes the void regions once per terrain version (union-find or
  one labelling pass), stores each region's cell list bucketed by `y`, and then equalises a
  region by solving its level from its water total — arithmetic over a region's rows, no
  walk at all — for the regions that hold water. The row-batching rule and the
  `free_transfer_cap` relaxation are preserved exactly; what disappears is the rediscovery.
- **Sky visibility** (flora light, germination gates). Already cached on
  `terrain_version` and already free in a steady world (0 rays/tick). The eligible-set
  *harness* scan is what pays for it, not the tick.
- **Roof/shade and `is_support`** (fauna candidate faces, presenter). Column-wise geometry,
  also a function of terrain alone: a per-column table of support faces, rebuilt on
  `terrain_version`, replaces the per-call vertical walks that put `is_support` and
  `water_depth_m` at 6.7 % of the process.

### Ranked, with estimates

Each estimate is "the phase becomes proportional to its active set, with the same per-item
cost as today"; that is the conservative reading, since a dense sequential scan has better
locality than a sparse walk. Baseline 11.72 ms/tick (4 grazers).

1. **Incremental regions for `equalize`** — persistent void regions keyed by
   `terrain_version`, wet-region-only equalisation, and the eight per-substep grid-sized
   `Vec`s replaced by reused buffers. Removes the 68 % of the process the flood fill and its
   allocator traffic hold. **5.21 ms → ~0.3–0.5 ms**, tick 11.7 → **~6.9 ms**. Highest
   payoff, and the only step that needs real care: the level solve and the row-batch order
   are the rule, and a rebuilt region must produce bit-identical water.
2. **Stand index by column + per-face score memo for `sense`** — stands in a `Vec` bucketed
   by column, so a `reachable_foliage` query touches the ~9 columns of its reach box instead
   of all 48 stands, and candidate faces scored once per tick instead of once per animal
   that considers them (2,849 queries for 28 animals today). **3.96 ms → ~0.1–0.2 ms**, tick
   → **~3.0 ms**. Pure indexing; no rule touched, and it must return the same sorted set.
3. **Wet/pore active sets for `fall`, `infiltrate`, `drain`, `water_table`, `rain`** — the
   sets above, maintained by `add_free`/`take_free`/`add_pore`/`take_pore`, which are already
   the only writers. 1.65 + 0.52 + 0.14 + 0.12 + 0.06 ms of grid scanning becomes
   proportional to 16.7k / 19.5k / 2.7k / band / 3,072 items. **2.49 ms → ~0.35 ms**, tick →
   **~0.9 ms = 1,100 ticks/s = 55× real time**.

That is the algorithmic tier: **~13× on today's number, and about 55× real time**, with no
rule change and no threads. The rest of the road:

4. **Data layout** — `f32` for `free`/`pore` (the ledger residual tolerance is 1e-9
   relative, so this needs a conservation check, and it halves the bytes moved), `u32`
   indices, region cell lists in `y`-sorted runs so a level solve is a linear pass, and no
   allocation in the tick at all. Estimate **2–3×** on what is left: ~0.35 ms/tick ≈ 140×
   real time.
5. **Threads, last** — the water phases split by row band with the seams resolved in a
   second pass (regions are the awkward part: a region spanning two bands must be owned by
   one), the flora and fauna layers split per stand and per animal. **4–8× on a desktop**,
   ~1–2× on the embedded target this is for. Desktop ~1,000× real time is reachable here;
   **on one embedded core it is not**, and the honest statement is that 1000× on embedded
   needs either a smaller world, fewer substeps or a coarser water model — all three are
   rule or condition changes and none of them is mine to make.

**Target arithmetic, stated plainly.** 1000× real time is 20,000 ticks/s = **50 µs/tick**.
With 19,524 wet cells and 4 substeps that is 0.6 ns per wet-cell-visit — under one cycle per
cell per substep. Steps 1–4 land at ~0.35 ms (140×); threads on a desktop reach ~50–90 µs
(600–1000×). So: **100× is an engineering exercise (steps 1–3, plus 4 for margin); 1000× on
one embedded core is a model decision, not an optimisation.**

### On using an ECS crate

Implementation-agnostic by design: every step above is a set plus an index, and both a real
ECS and hand-rolled sparse sets provide that. Two concerns worth weighing before adopting a
crate:

- **Determinism.** This project's rules are keyed streams and sorted `Vec`s specifically so
  that no iteration order can reach a result ("never HashMap iteration", and the lottery
  sorts its candidates before drawing). An archetype-based `Query` iterates in storage
  order, which changes with insertion history, component sets and crate version, and
  parallel system execution reorders reductions. Any set whose *sum* or *draw* matters would
  still have to be sorted by a stable key (site index, stand id, animal id) before use —
  which is what the code does today. That is a discipline the crate does not enforce.
- **Dependency tail.** `bevy_ecs` brings `bevy_ptr`, `bevy_utils`, `bevy_tasks`, `ahash`,
  `fixedbitset` and friends, uses unsafe pervasively and assumes a thread pool — heavy for a
  crate that today is `serde` + `postcard` and is aimed at embedded. `hecs` is small (one
  hashbrown) and unopinionated but gives the same iteration-order caveat and no scheduler.
  Hand-rolled sparse sets (a `Vec` of items plus a dense index array over cells, which is
  exactly the shape `Ground` and `Stand` already use) keep `#![forbid(unsafe_code)]`, keep
  the determinism guarantees in the open, and add nothing to the tail. **My
  recommendation: hand-rolled for the water sets and the stand/animal indices now; revisit a
  crate if the entity graph ever grows relationships this world does not have.** The choice
  is Fable's.

## What this measures and what it does not

Measured: where one coupled tick's time goes on this world, on one core, with the sets that
a sparse tick would iterate instead. Not measured: any other world size or `water_substeps`
value, any embedded target, cache behaviour (no counters beyond cycles), and whether an
incremental region scheme reproduces the water bit for bit — that is the first thing any
implementation of step 1 has to prove, against the existing conservation residual and the
round-3/4/5 fixtures. No rule, number, preset or observation changed in this commit.

---

# Addendum, same day: the local water model, measured

Wrysk approved a first-pass **local** water model and dropped the bit-identical
requirement: the contract is conservation and the same qualitative behaviour, measured
statistically. `equalize` is gone; the rule that replaced it is in `water.rs`'s module doc
and its commit. This is what it cost and what it bought, same machine, same conditions as
above.

## Before and after, mean ± spread over five seeds (1,000 ticks each)

| arm | before | after | faster |
| --- | --- | --- | --- |
| 0 grazers | 8.81 ± 2.31 ms/tick, 121 ± 35 ticks/s (6.0× real time) | **2.68 ± 0.07 ms/tick, 373 ± 10 ticks/s (18.6×)** | 3.3× |
| 4 grazers | 11.60 ± 2.39 ms/tick, 89 ± 20 ticks/s (4.5×) | **5.06 ± 0.11 ms/tick, 198 ± 4 ticks/s (9.9×)** | 2.3× |

The spread collapsed with the mean: the old solver's cost depended on how much of the
world happened to be wet and how the regions fell out, and the local exchange's does not.
`community 60` went from 17.7 s to **5.9 s** for 1,200 coupled ticks.

## Per-phase, after (4 grazers, 2,000 ticks)

| phase | µs/tick | before | share now |
| --- | --- | --- | --- |
| **World::step** | **2401** | 7709 | 38.7 % |
| rain | 133 | 65 | 2.2 % |
| substep loop | 2111 | 7388 | 34.1 % |
| — infiltrate | 64 | 519 | 1.0 % |
| — fall | 396 | 1654 | 6.4 % |
| — **exchange** (was equalize) | **1650** | 5215 | 26.6 % |
| drain | 33 | 137 | 0.5 % |
| water_table | 123 | 120 | 2.0 % |
| **Flora::step** | 40 | 45 | 0.6 % |
| **Fauna::step** (sense) | 3757 | 3964 | 60.6 % |

**The tick is now the animal layer's sensing**, which this package was told not to touch:
step 2 of the plan (stands indexed by column, per-face score memo) is the next 3.8 ms.
Water is 2.4 ms of which the exchange is 1.65, and the counts say why: 1,412 wet cells and
1,416 columns per substep instead of 147,456 cells scanned four times.

## The active sets, after

| set | after | before |
| --- | --- | --- |
| cells holding free water | **1,995** | 19,524 |
| of those, movable by `fall` | 210 | 16,748 |
| cells holding pore water | 11,862 | 11,842 |
| over field capacity (`drain`) | 2,681 | 2,672 |

## What changed in the behaviour, statistically

| quantity, `community 60` | before | after |
| --- | --- | --- |
| stored water (m³) | 217.3650 | 217.3791 |
| aquifer head (m) | 2.945 (−0.0390 over 60 s) | 2.901 (−0.0566) |
| water residual | 1.3e-10 | 6.7e-11 |
| flora residuals | 1.4e-14 | 1.4e-14 |
| mean root-box pore, the six species | 0.143 / 0.155 / 0.159 / 0.191 / 0.204 / 0.257 | 0.143 / 0.155 / 0.159 / 0.192 / 0.205 / 0.257 |

**The soil the ecology reads is the same to three decimals**, and the world holds the same
water. Three differences worth stating:

1. **Ten times fewer cells hold free water** at the same stored volume (1,995 against
   19,524). The old solver spread a region's water as a film across every cell it reached;
   the local exchange leaves fewer, deeper puddles. The old module doc called that film a
   known artifact — a reader that treats any free water as standing water saw it
   everywhere — so this is a change in the right direction, but it does move
   `water_depth_m` on support faces and therefore the drowning gate's input.
2. **The world drains slightly faster** (head −0.057 m against −0.039 m over 60 s): water
   reaches the outlet as a body rather than being re-levelled into it instantly.
3. **A closed, surcharged passage does not settle flat.** A local rule with no pressure
   solve leaves the surface above a flooded roof uneven by a few tenths of a cell and
   relaxing; the fixture states it instead of asserting it away. Everything else the old
   fixtures pinned — spill thresholds, the symmetric spill, the mirrored shelf, the seam
   shift, the U-tube's levels — comes out the same.

## Next, unchanged from the plan above

Step 2 (sensing: stands indexed by column plus a per-face score memo, 3.8 → ~0.15 ms) is
now the whole tick, and it is Wrysk's own thread. After it, the tick is ~1.3 ms — about
15× real time for the water alone at 2.4 ms — and the remaining water cost is `exchange`
1.65 ms over 1,412 wet cells per substep, which is 290 ns per wet cell: data layout (step
4) is the next lever there, then threads.

---

# Addendum, same day: the schedule, and what threads bought

Wrysk (2026-09-18) decided the sim runs on a real ECS crate with multithreading early
(`design/handoffs/voxel-schedule-brief-2026-09-18.md`), against this note's own
recommendation; the two cautions it raised were kept and are honoured in the code. The tick
is now `cubarium_voxel_sim::Sim::step`: one `bevy_ecs` world holding `World`, `Flora` and
`Fauna` as **resources** (the voxel grid stays dense — never one entity per voxel), one
`Schedule` whose systems are the phases `.chain()`ed in the order above, and a
single-threaded *system* executor on purpose. **No rule, number, preset or phase order
moved.** The bench moved with it:

```text
cargo run --release -p cubarium-voxel-sim --features profile --example bench \
  -- [ticks] [grazers] [warmup_s] [seed] [sample_every] [threads]
```

The compute pool is process-global, so one process per thread count.

## The table: 1,000 ticks, 5 seeds, mean ± half-spread

Same declared condition as the rest of this note — the generated default world, harness
rain, outlet open, 50 s of world-only conditioning, then eight founders of each of the six
species and eight declared logs. Seeds 1..5, samplers off. The 4-grazer arm carries **17.8
animals** and **1,696 wet cells per substep** as means over the window.

| grazers | threads | ms/tick | ticks/s | × real time | vs 1 thread |
| --- | --- | --- | --- | --- | --- |
| 0 | 1 | 1.662 ± 0.061 | 602 ± 22 | 30.1 | 1.00× |
| 0 | 4 | 1.361 ± 0.109 | 737 ± 57 | 36.9 | 1.22× |
| 0 | 8 | 1.338 ± 0.098 | 750 ± 53 | 37.5 | 1.24× |
| 0 | 16 | 1.347 ± 0.090 | 744 ± 49 | 37.2 | 1.23× |
| 4 | 1 | 4.071 ± 0.094 | 246 ± 6 | 12.3 | 1.00× |
| 4 | 4 | 2.032 ± 0.056 | 492 ± 14 | 24.6 | 2.00× |
| 4 | 8 | 1.747 ± 0.067 | 573 ± 22 | 28.7 | 2.33× |
| 4 | 16 | 1.735 ± 0.046 | 577 ± 15 | 28.8 | 2.35× |

Against the local-water addendum above (2.68 ms with 0 grazers, 5.06 ms with 4, both at one
thread): **1.6× and 1.2× before any thread**, then **2.0× and 2.9×** with
16 threads. 18.6× → 37× and 9.9× → 29× real time.

**Two things did that, and only one of them is threads.** The larger single win is not
parallel at all: `exchange`'s scratch (`head`, `room_target`, `run_top`) is now indexed
`col * height + y` instead of the world's `y * plane + col`. The old scan strode 3,072
cells — 24 KB — between consecutive entries of one column, so every step of it touched a
new cache line. Re-indexing took `exchange` from 1,800 to 650 µs/tick on a 300-tick probe
with nothing else changed. Cache locality, measured, before any thread.

## Per-phase at 16 threads, 4 grazers, mean of 5 seeds

`World::step`'s own `WorldStep`/`Substeps` frames never open under the schedule — it runs
the phases one at a time — so the water total is the sum of its leaves and those two rows
are absent rather than zero.

| phase | 1 thread µs/tick | 16 threads µs/tick | 16/1 |
| --- | --- | --- | --- |
| rain | 132.2 | 134.7 | 0.98× |
| infiltrate (×4) | 70.5 | 73.6 | 0.96× |
| fall (×4) | 493.7 | 487.9 | 1.01× |
| **exchange (×4)** | **831.6** | **496.7** | **1.67×** |
| drain | 19.6 | 20.3 | 0.97× |
| water_table | 118.8 | 119.5 | 0.99× |
| evaporate, spring, outlet | 0.0 | 0.0 | — |
| **water, total** | **1666.5** | **1332.8** | **1.25×** |
| Flora::step (total) | 38.5 | 42.2 | 0.91× |
| **Fauna::step (total)** | **2362.9** | **355.0** | **6.66×** |
| — **sense** | **2361.2** | **351.6** | **6.72×** |
| — act, births, deaths, terrain, maintenance | 1.7 | 3.4 | — |

## What parallelised, what did not, and why

Two phases split, and they are the two the profile named:

- **`exchange`'s column scan** — 1.67× at 16 threads on top of the 2.8× the layout already
  bought. Each column's scan writes only that column's own `height` scratch entries and
  reads only the world's arrays, so the ascending column list is cut into `threads`
  equal-count chunks and each chunk owns one contiguous, disjoint `&mut` span: plain
  `split_at_mut`, no unsafe, no reduction. It stops at 1.67× because what remains is the
  **strided reads** of `material` and `free`, which are memory-bound and do not get faster
  with more workers, plus the fold-free spawn overhead at 1,694 columns per substep.
- **fauna `sense`** — 6.72×, from 2.36 ms to 0.35 ms, which is most of the whole win. Every
  animal's plan is a pure function of the world, the pre-bite plant layer and that animal,
  so the animals are cut one chunk per worker; each chunk returns its plans **with its own
  chunk number** and they are folded in that order, so the finishing order cannot reach the
  result. 16 workers barely beat 8 because 17.8 animals means one animal per chunk and a
  one-animal tail.

Everything else is serial, each for a stated reason:

- **`rain`, `evaporate`, `infiltrate`, `fall`, `drain`** (0.72 ms together at 16 threads:
  86 % of the water that is still serial, and `fall` alone is 0.49 ms) all write free or
  pore water through `add_free`/`take_free`/
  `add_pore`/`take_pore`, and those maintain the two `CellSet`s — one shared `Vec` plus a
  dense slot array, with swap-removal. Two threads inserting into that cannot both be
  right. **Splitting the active sets per band is a data-structure change, not a scheduling
  one**, and it is the next lever on the water side after `fall`'s remaining 0.49 ms.
- **`water_table`** is blocked by a **rule**, not by a structure: its own doc says "within a
  step the fill runs bottom-up in index order, which is also the order a scarce stock is
  shared in". Parallelising it would change who gets the last of a nearly empty aquifer.
- **`spring`, `outlet`** are one cell each.
- **`flora`** does have a clean read-then-apply split — `light_per_stand` and `drink`'s
  root-box read are per stand and touch nothing else — but the whole plant layer is **38 µs
  of a 1,735 µs tick (2 %)**, at the scale of the spawn overhead itself, and
  `light_per_stand` writes the sky cache while reading it (`sky_at` takes `&mut Vec`), so it
  would need a cache-filling pass first. Left serial, and measured that way: it is 0.91× at
  16 threads, which is noise.

**The tick is a dependency chain and no two phases ever run together.** That is why the
schedule's executor is single-threaded and why the table tops out near 2.35×: at 16 threads
0.88 ms of the 1.74 is phases that never split at all (0.84 water, 0.04 flora), against
0.85 ms for the two that do, and Amdahl does the rest. The next 2× is algorithmic
(band-split active sets, `f32` stores, no allocation in the tick — steps 3 and 4 of the plan
above), not more workers.

## Determinism, and what the tests assert

Bit-exactness was dropped by Wrysk on 2026-09-18 and is not claimed. What is asserted, as
short function tests in `crates/cubarium-voxel-sim/tests/schedule.rs`: the schedule agrees
with the three-call sequence to 1e-12 relative on stored water, aquifer head and
per-species mean root-box pore; stored water, that pore reading and **every animal's id and
the face it stands on** agree across 1, 4 and 16 threads after 200 coupled ticks within
1e-6 relative; and the water and both flora residuals close. No hash is pinned. In fact
neither parallel phase contains a reduction — a column's scan and an animal's plan are
independent and nothing is summed across chunks — so the agreement observed is exact; the
tolerance is what is *asserted*, for a future parallel phase that does fold.

## Dependency tail and build time

`Cargo.lock` grows from 180 to 236 packages. Of those 56, the ones actually **compiled** are
the ~50 `bevy_ecs` 0.19 and `bevy_tasks` 0.19 reach with `default-features = false`:
`bevy_platform`, `bevy_ptr`, `bevy_utils`, `bevy_ecs_macros` (with `syn`, `quote`,
`proc-macro2`, `toml_edit`, `winnow`), `fixedbitset`, `indexmap`, `hashbrown`, `foldhash`,
`slotmap`, `nonmax`, `arrayvec`, `smallvec`, `thread_local`, `derive_more`, `disqualified`,
`variadics_please`, `log`, and `bevy_tasks`'s `multi_threaded` executor
(`async-executor`, `async-channel`, `async-task`, `concurrent-queue`, `event-listener`,
`futures-lite`, `crossbeam-utils`, `parking`, `fastrand`). **`bevy_reflect` is off**, so
`glam`, `wgpu-types`, `uuid`, `erased-serde` and `downcast-rs` are in the lock and never
compiled — this note checked `target/release/deps` for them and found none. So is
`backtrace`, and so is bevy's own multi-threaded *system* executor.

Release build time, `--workspace --all-targets`, `-j 24`, this machine: **11.2 s** with the
tail already built, **21.4 s** with the 56 new packages and the five cubarium crates
cleaned. The tail costs about **+10 s wall (+206 s CPU), once**. `cubarium-voxel/parallel`
and `cubarium-voxel-fauna/parallel` were off by default when this was measured and only the
sim crate turned them on. **Since 2026-09-18 they are default features** (Wrysk: "make it
parallel by default"): the plain `World::step` and `Fauna::step` split their read-only
phases across `cubarium_voxel::default_threads()` (every core the OS reports), so the study
harnesses and the crates' own tests run parallel too, and `step_with(1)` is the serial run.
An embedded build that wants no `bevy_tasks` in its tail sets `default-features = false` on
those two crates.

### Bench rerun — 2026-09-18 at 528a3ac

`cargo run --release -p cubarium-voxel-sim --features profile --example bench -- 1000 4 50 <seed> 100 <threads>`, seeds 1..5, one process per run.

| threads | ms/tick (mean ± half-spread) |
| ---: | ---: |
| 1 | 4.105 ± 0.057 |
| 4 | 2.057 ± 0.062 |
| 8 | 1.787 ± 0.097 |
| 16 | 1.750 ± 0.089 |
