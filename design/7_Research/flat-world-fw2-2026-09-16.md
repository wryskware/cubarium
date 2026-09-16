---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-2 result: topology and scale through the world (`cubarium-core`)

Written by Opus on 2026-09-16 against
[the FW-2 brief](../handoffs/flat-world-fw2-opus-core-2026-09-16.md),
[FW-1's freeze](flat-world-fw1-2026-09-16.md) and
[the ring-world plan](../flat-world-plan-2026-09-16.md) §4, §5, §5a and §9.

A ring world runs at both scales, a cube world is **proven unchanged by value**
against a snapshot written by a build with no topology in it, and schema 17
refuses every older world by name.

Three commits on `tachyon-screen`:

| commit | what |
|---|---|
| `1ba6dd5` | the topology and the scale through the world |
| `2d3dc6f` | schema 17, the frozen `v16` mirror, and the `CubeProjection` comparator |
| `ecf0c5a` | a ring world runs, and the tests that say what that means |

## 1. The config surface

Two fields, appended after `mechanisms` so a schema 16 config stays the exact
postcard prefix the frozen mirror decodes (`config.rs:52,59`).

```toml
version = 9
seed = 1

# A cube — the default. Either spelling works, and omitting the field entirely
# gives this.
topology = "Cube"
world_scale = 1.0

# A ring at S = 1, 320 x 180 pixels = 80 x 45 cells.
topology = { Ring = { w = 320, h = 180 } }
world_scale = 1.0

# The same world at S = 2, for a 640 x 360 panel. Same 3,600 cells, same noise,
# twice the pixels and twice the art.
topology = { Ring = { w = 640, h = 360 } }
world_scale = 2.0
```

**§5's TOML example is wrong as written.** The plan (§7, and repair 4 finding 5)
writes `topology = { ring = { w = 320, h = 180 } }` in lower case. `Topology`
derives serde with no `rename_all`, so the accepted spellings are `"Cube"` and
`{ Ring = { … } }` — `topology.Ring = { w = …, h = … }` also parses. Lower case
is refused with a TOML parse error. Making the plan's spelling work would be a
one-line `#[serde(rename_all = "snake_case")]` in **FW-1's** crate, which FW-2
does not own; flagged rather than taken, because FW-6's and FW-4's fixtures may
already spell it the current way.

`WorldConfig::validate` checks the pair **first** (`config.rs:683`), because
every bound below it is a length on the surface that pair describes, and the two
radius checks that named the cube's `MAX_LOCAL_RADIUS` now take
`Topology::max_local_radius()` (`config.rs:686,896,949`) — plan repair 3
finding 4. `CONFIG_VERSION` 8 → 9.

`World::{topology, scale, cell_count}` at `world/mod.rs:96,103,110`;
`RenderView.{topology, scale}` at `view.rs:41,44`, filled at
`world/view.rs:76-77`.

## 2. Every height, embedding and cache site converted

`embed()` and `height()` are asked for separately, which is what §5a exists for.
They are the same number on the cube — `Topology::height(Cube)` *is*
`embed(Scale::ONE)[1]`, `geometry.rs:305` — so every substitution below is
bit-exact there, and that is what the projection in §4 confirms.

| site | was | is |
|---|---|---|
| `habitat.rs:94` | `Topology::Cube.embed(ONE, &cell.center(…))` | `topo.embed(scale, &center)` — the noise sample position |
| `habitat.rs:105` | `let y = p[1]` | `let y = topo.height(&center)` — light, moisture |
| `habitat.rs:115` | `y + basin_gain·n_b` | unchanged formula, `y` now the height scalar — terrain |
| `world/step.rs:464` | `Topology::Cube.embed(ONE, &o.pos)[1]` | `topo.height(&o.pos)` — the classic controller's height channel |
| `world/step.rs:465` | `up_direction(o.pos.face)` | `up_direction(topo, o.pos.face)` |
| `world/step.rs:3087-3088` | the same two | the same two — the **neural** controller's `SelfState` |
| `world/view.rs:163` | `Topology::Cube.embed(ONE, &o.pos)[1]` | `topo.height(&o.pos)` — telemetry's mean height by form |
| `world/lifecycle.rs:400` | `up_direction(face)` | `up_direction(topo, face)`; ring returns the constant `(0, −1)` |
| `controller.rs:234` | `obs.up * (w_depth · (h_pref − obs.height))` | **unchanged**, and correct given the two above |
| `pairs.rs:73` | `Topology::Cube.chord_sq` | `topo.chord_sq` — exact on a ring, a bound on the cube |

`up_direction` is a match, not a gradient computation, on a ring:
`height = 1 − 2v/h` falls with `v`, so uphill is `−v` everywhere. Checked beside
the cube's in `world::tests::the_depth_term_points_up_the_side_faces_and_vanishes_on_top`.

**Runtime cell count.** `1,280 = 2^8·5` is the cube's count and cannot be
factored 16:9 with square cells, so it appears nowhere as the world's size any
more. Sized from `topology.cell_count(scale)`: `Habitat`'s four arrays
(`habitat.rs:29-44`), `World`'s five weather/habitat caches and `images`
(`world/mod.rs:46-58`, built at `world/lifecycle.rs:212-236`), `Fields::new`,
`EcologyV1State::new`, `EcoScratch::new`, the six `vec![0.0; …]` scratch arrays
in the intake pass (`world/step.rs:1633-1642`), `sense_rings`,
`care::footprint`, `field_dump`, `telemetry` and `cell_neighbors`.
`EcologyV1State`'s `Default` is replaced by `EcologyV1State::empty(cells)`: a
default could only ever have been right for one topology. `water::step`,
`Weather::sample` and `Fields::react` take slices instead of `&[f64; 1280]`.

`World::images` is a `Vec` with one entry per chart, indexed by
`Topology::chart_index` — five on a cube, one on a ring. The four
`ChartImage`-taking hunter/pair functions take the topology with it
(`pairs::build`, `hunter::{surface_reach, measure_contact, body_point,
ContactEvidence::gather}`).

**Founders** (`world/lifecycle.rs:78-85`). The face draw is made and
**discarded** on a ring, so the counter block per founder stays 0..4 whatever
the topology and the cube's stream is byte-identical; only the extent `(u, v)`
scale by moves from `FACE_EXTENT` to `topo.extent(face)`.

**Weather is untouched**, as §5a requires. No draw changed, so stream parity is
exact by construction rather than engineered — and
`cube_projection::weather_draws_identically_on_a_ring_and_on_a_cube` asserts it
directly: a cube world and a 320×180 ring world at the same seed hold the
**same `Weather` value** at tick 0 and after 2,500 ticks (two simulated minutes,
so the per-minute random walk has fired twice).

**The two core resolvers** (`care.rs:166`, `hunter/state.rs:65`) take the
topology and bound `u`/`v` by `Topology::extent` instead of the literal 64. A
`(200, 170)` target is cell (50, 42) on a 320×180 ring, cell (25, 21) on the
same world at `S = 2`, and refused on a cube; `Face::Top`, a negative
coordinate, a NaN and `u = 320` are all refused on a ring.

**`WorldState::validate`** (`world/state.rs:112-262`): the topology first, then
every per-cell serialized vector — the six `Fields` channels **and ecology v1's
five pools**, which the plan's §4 list implies but does not spell out — against
`cell_count()`; every organism's chart (`has_chart`) and canonical position
against the topology; and `CareState::validate` taking the runtime cell count so
the persisted shower footprints are range-checked against the world instead of
`CUBE_CELL_COUNT` (`care.rs:318`).

## 3. The projection result

The comparator is `crates/cubarium-core/tests/cube_projection.rs`; the fixture is
`tests/fixtures/cube-projection-v16-2a1cedd.cubw` with its provenance beside it.

- **Fixture**: one snapshot from an unmodified `main` build at `2a1cedd` — the
  commit `tachyon-screen` branched from, so the only difference between the two
  builds is this branch's own work. Built in a detached `git worktree` under the
  task's scratch directory with its own target dir; the main checkout's working
  tree was never touched.
- **Run**: `WorldConfig::default()`, `seed = 20_260_916`, **6,000 `World::step`
  calls**. Five simulated minutes at `TICK_HZ = 20`: the weather's per-minute
  walk has fired four times, and the world has committed births (24 founders →
  46 organisms, 78 births and 39 deaths by tick 18,000 in the longer run), so
  the slot allocator's free list is exercised rather than pristine.
- **Result**: `first_difference(before, after) == None` — no field differs — and
  both projections hash to **`10304345502826573087`**, pinned in
  `the_projection_hash_is_pinned`.
- The versions are asserted **separately**: fixture schema 16 / config 8, this
  build schema 17 / config 9.
- `decode_snapshot` still refuses the fixture with
  `UnsupportedSchema(16)`, and `decode_v16` refuses this build's own snapshot
  with `UnsupportedSchema(17)`. The product refuses old worlds; only the
  comparator looks inside one.
- The comparator can fail: a `1e-12` change to one cell of `N` and a `1e-6`
  change to `producer.growth` are both caught, and named (`fields`,
  `config.producer`). FW-6 owns the full six-perturbation battery.
- Debug and release produce a byte-identical fixture, so the comparator is
  profile-independent and the `debug_assertions` audits inside the step are
  confirmed to change nothing.

The frozen mirror carries its own `WorldConfigV16` — unlike `v7..v14`, which
reuse the live `WorldConfig` and are only frozen while the config's shape holds
still. It is `version` followed by `ConfigProjection`, which is byte-for-byte the
flat schema 16 config because postcard writes a struct as its fields
concatenated with no framing; `a_v16_config_is_version_then_the_projection`
pins that rule on its own rather than leaving it on trust.

## 4. The ring steady-state numbers

`cargo run --release --example ring_steady_state -- <ticks> 1`, defaults
otherwise. 320×180 at `S = 1` and 640×360 at `S = 2` are both **3,600 cells**;
the cube is 1,280.

### 3,000 ticks (the brief's run)

| | cells | pop | P | F | D | De | N | w | wood | reserve | mass residual | ticks/s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cube | 1,280 | 24 | 123.85 | 0 | 509.52 | 1000.97 | 780.43 | 181.97 | 138.19 | 44.40 | −3.2e−12 | 9,701 |
| ring 320×180 S=1 | 3,600 | 24 | 286.96 | 0 | 1807.75 | 3598.20 | 2338.25 | 507.64 | 325.03 | 102.71 | +3.0e−11 | 3,731 |
| ring 640×360 S=2 | 3,600 | 24 | 286.89 | 0 | 1808.02 | 3599.40 | 2338.42 | 507.64 | 325.01 | 102.71 | +2.7e−11 | 3,679 |

Population is flat at the 24 founders on all three: the first birth in a default
world lands after tick 3,000. Field totals scale with the cell count (2.81× the
cube's, which is 3,600/1,280) — the ring is the same ecology on a larger surface,
not a different one. Mass is conserved to 1e−11 on the ring exactly as on the
cube.

### 18,000 ticks (fifteen simulated minutes — long enough to be a population)

| | pop (min/max) | births | deaths | P | D | N | w | wood | carrion |
|---|---|---|---|---|---|---|---|---|---|
| cube | 63 (24/77) | 78 | 39 | 193.79 | 130.15 | 987.96 | 17.84 | 172.45 | 14.52 |
| ring S=1 | 50 (24/76) | 68 | 42 | 453.14 | 510.12 | 3281.34 | 427.83 | 394.82 | 17.66 |
| ring S=2 | 47 (24/62) | 55 | 32 | 455.79 | 517.01 | 3282.53 | 427.83 | 395.20 | 9.98 |

**Two things for Wrysk, both predicted by §5 and both confirmed:**

1. **The bottom wall is a moat.** At 18,000 ticks the ring's bottom cell row
   holds `w = 78.3` of the world's 427.8 — 18% of all standing water in 1/45 of
   the cells — while the canopy row holds `0.000`. The cube's whole world holds
   17.8. §5's "expect standing water along the bottom row and re-check
   `evap_floor`" is exactly right, and `evap_floor` has not been re-tuned here:
   that is a knob, and `design/backlog.md` owns knobs.
2. **The stratification reads.** The top row carries the foliage (`P = 16.1`
   against `0.19` in the bottom row) because `light_height_gain` is positive and
   `height` falls with `v`. A test asserts both, on a 6,000-tick world.

**`S = 1` and `S = 2` are the same *environment* and not the same *world*.** The
habitat is **bit-identical** between them — all 3,600 cells of `light_base`,
`embed` and `height` agree to the bit, because `θ`, `r` and `y_e` are all
invariant under `(w, h, S) → (2w, 2h, 2S)`. The animals are not, because every
organism length in `WorldConfig` is in **pixels**: `sense_radius = 6` is 1.5
cells at `S = 1` and 0.75 at `S = 2`, and `speed_max` is px/s either way, so a
body crosses half as much world per second. §7 already stages "which defaults
`world_scale` multiplies" as a separate decision; this is the measurement that
says it is a real one and not a formality. The field columns above differ only
in the fourth digit at 3,000 ticks because 24 animals barely touch 3,600 cells;
by 18,000 ticks the populations have diverged (50 against 47).

## 5. Verification

- `cargo test --workspace --exclude cubarium-gpu`: **1,655 passed, 0 failed, 25
  ignored** (FW-1 recorded 1,514 / 0 / 23 at its freeze; the difference is FW-2's,
  FW-3's and FW-6's new tests). `cubarium-gpu` is GS-1's, still in flight, and
  does not touch this crate.
- `cargo clippy --workspace --exclude cubarium-gpu --all-targets`: **no new
  warning in any file FW-2 touched**, checked file by file against the
  pre-change build. The one clippy **error** in the crate,
  `neural/gru.rs:233` (`0 * HIDDEN` in a `#[cfg(test)]` weight setup, "this
  operation will always return zero"), is **pre-existing**: that file is
  byte-identical to `2a1cedd` and the same clippy run on the `main` worktree
  reports it too. Not touched, because silencing an unrelated lint in the same
  commit as a topology change is how a real regression gets hidden.
- Every pre-existing core test passes with its assertions unchanged except the
  four below, each of which reads a version number or the config and none of
  which is a world moving:

| test | why it moved | re-recorded as |
|---|---|---|
| `hunter.rs::observations_do_not_move_a_profile_three_hunter_world` | hashes the **schema 12 projection**, which carries `config` | `7424989416957462349` and `14793747053647923190` |
| `ecology_v1.rs` A7 | asserted `SCHEMA_VERSION == 16` | `== 17`, renamed off "sixteen"; its `7..SCHEMA_VERSION` refusal loop now covers 16 for free |
| `snapshot_hardening.rs` | asserted `SCHEMA_VERSION == 16` | `== 17`, renamed, plus `SCHEMA_V16 == 16` |
| `snapshot.rs::every_older_schema_is_refused_by_name` | the refusal list | `SCHEMA_V16` appended |

The claim each of those tests makes is unchanged. That the worlds themselves did
not move is what §3 above shows, against a build with no topology in it.

New tests: `tests/cube_projection.rs` (6) and `tests/topology_world.rs` (8),
plus the ring arm of `world::tests::the_depth_term_points_up…`.

## 6. Files touched outside `crates/cubarium-core/**`

All in `crates/cubarium/**` (host), all mechanical follow-through of
`RenderView`'s two new fields and the two resolvers' signatures, every one
naming `Topology::Cube` and `Scale::ONE` explicitly so the host stays cube-only
until FW-4 and FW-5. Nothing else in those files was staged: FW-3 was editing
several of them at the same time, so the commit was assembled hunk by hunk and
`git show --numstat` was checked against the marker lines.

| what | files |
|---|---|
| `topology`/`scale` inserted into a `RenderView` literal | `src/present.rs`, `src/art_present/tests.rs`, `src/corner_cap_present_tests.rs`, `examples/{ecology_sheet,shoulder_sheet}.rs`, `tests/{animation_load,art_bands,art_ecology,art_growth_clip,art_growth_pack,art_mode,art_motion,art_plants,art_water,art_wind,art_wind_capture,art_wind_top,astra_motion_regressions,lanternjaw_cost,meal_present}.rs` (20 sites) |
| `CareTarget::resolve` gained `(topo, scale)` | `src/care_effects.rs`, `examples/care_compare/local.rs`, `examples/ambient_compare/region.rs` |
| `measure_contact` / `body_point` gained `topo` | `examples/hunter_compare/spatial.rs` |

## 7. What §4 and §5 proved wrong, or left unsaid

1. **The TOML spelling is `Ring`, not `ring`** (§1 above). §7's example and
   repair 4 finding 5 are both wrong as written.
2. **§4's list of per-cell serialized vectors is short.** It names "every
   `Fields` vector and every ecology v1 vector" but its own citation is
   `fields.rs:19-32`, which is the four `Fields` channels the old `validate`
   checked. The extended check here covers **eleven**: `n, p, d, de, f, w` plus
   `wood, plant_reserve, dead_wood, carrion, carrion_energy`. `f` and `w` were
   previously unchecked for length at all, and `EcologyV1State::check` only
   checked its pools against each other.
3. **`EcologyV1State: Default` had to go.** §4 says the fixed-size arrays live
   in `Habitat` and `World` and are rebuilt on load — true — but it does not
   mention that `EcologyV1State` and `EcoScratch` carried cube-sized `Default`
   impls. A `Default` for a per-cell type can only be right for one topology, so
   it is now `EcologyV1State::empty(cells)` and `EcoScratch::new(cells)`. This is
   a small public API removal in `cubarium-core`; nothing in the workspace used
   it.
4. **`Fields`'s two `#[serde(default = "dry")]` attributes are gone.** `dry()`
   returned a cube-length vector, which is a wrong answer on a ring and an
   unreachable one either way: postcard is not self-describing, so a missing
   trailing field is a decode error, not a default. Removing them changes no
   encoding.
5. **`RenderView` gaining required fields is a 20-site edit in the host.** §3
   asks for `RenderView.topology` without noting that a struct literal has no
   way to opt out. Worth knowing before FW-5 adds anything else to it.
6. **The `RingTooNarrow` message reads oddly at the degenerate end.** An 8×8
   ring is refused with "ring width 8 is below 8", because `needed` is
   `2·max(r, 0) + 2·CELL_PIXELS` and `r` has gone to zero. The refusal is right
   and FW-1's item 3 anticipated it; only the sentence is confusing. FW-1's
   crate, not touched here.
7. **`FootprintExceedsLocalRadius` stays**, per the coordinator's decision of
   2026-09-16. It never fires on the §6 ladder or on the cube, and it is the
   check that stops a validated world panicking the first time anything is
   stamped.
8. **Not done here, and not FW-2's**: the canopy `canopy_top` threshold (§5's
   open call, presentation, FW-5), `evap_floor` against the new bottom wall
   (a knob), and organism density — the same 512-organism cap over 3,600 cells
   instead of 1,280. §5 recommends keeping the cap and raising `founders`
   proportionally; the runs above are at the **unchanged** default of 24
   founders, so the ring is 2.81× thinner than the cube in ecological terms and
   the numbers in §4 should be read that way.
