---
design_status: exploration
last_reviewed: 2026-09-16
---

# SYNC-1 result: `main` merged into `tachyon-screen`

Written by Opus on 2026-09-16 against
[the SYNC-1 brief](../handoffs/flat-world-sync-main-opus-2026-09-16.md),
[FW-1's freeze](flat-world-fw1-2026-09-16.md) and
[FW-2's result](flat-world-fw2-2026-09-16.md).

`main` had moved **100** commits since this branch's base `2a1cedd` — ecology v1 rounds 2
through 5: the apex pursuit predicate and its adoption, two motor models, the apex turn
radius, the intake trace, the plant budget, the strike record, the depth census, two
factorials, the antithetic ES analysis, the founder roster and a second founding door. One
merge commit, two path-only follow-ups, and the cube re-proved unchanged against the new base.

**The headline is a single byte.** The same fixed-seed 6,000-tick default world, exported by
an unmodified `main` build at `2a1cedd` and again by an unmodified `main` build at its head
`15a2210`, produced two files that differ in **one byte** — the schema number in the header —
and agree in all 135,446 others. `main`'s hundred commits moved nothing a default world does.

| | |
|---|---|
| merge commit | `b02524f`, parents `b3a1481` (branch) and `cdcee03` (`main`) |
| new fixture | `cube-projection-v17-15a2210.cubw`, SHA256 `b891f1ab2768ead4b36f4c4c3695b6c8c6249c9852d97ab2ab7662f63034d28e` |
| projection hash | **`10304345502826573087`** — the number FW-2 pinned, unchanged |
| schema | **18** (both branches had claimed 17; see §2) |
| workspace | `cargo test --workspace --exclude cubarium-gpu`: 1,956 passed, 0 failed, 31 ignored |

## 1. The eight conflicts

The brief predicted five; `main` moved again between the dry run and the merge, and the real
count was eight. Every one is the same shape — `main` added code to a cube, FW-1/FW-2 made the
cube one topology of two — and both intents are kept in all eight.

### 1.1 `cubarium-core/src/snapshot.rs` — the schema number

- **`main`'s intent**: schema **17**, a semantics-only bump. The payload shape did not move at
  all; the *shipped pursuit stopping rule* did, and that rule is a `World` transient the bytes
  cannot carry, so 16 is refused by number to stop a half-space world resuming under the reach
  envelope.
- **This branch's intent**: schema **17**, a shape bump. `WorldConfig` gains `topology` and
  `world_scale`, `CONFIG_VERSION` goes 8 → 9, and 7..=16 are refused by name.
- **Resolution**: **schema 18**, refusing 7..=17 by name. See §2 — this is the one design call
  in the merge.

### 1.2 `cubarium-core/src/world/lifecycle.rs` — the founder loop

- **`main`'s intent**: the founder loop is extracted into `roster_keys(&config)` and
  `founder_body(&config, index, kind, born_tick)` so that the new `World::found_roster` door
  (`tests/found_roster.rs`) can place the same cohort into an already-running world.
- **This branch's intent**: the founder draw reads the topology — the face draw is *made and
  discarded* on a one-chart world so the counter block stays 0..4 and the cube's stream is
  byte-identical, and `(u, v)` scale by `topo.extent(face)` rather than `FACE_EXTENT`.
- **Resolution**: `main`'s extraction kept, with the topology moved **inside** `founder_body`,
  which reads it from the config it is already given. Both doors — `World::new` and
  `found_roster` — therefore place founders on whatever surface the config names, and the
  cube's draw stream is untouched. `World::new`'s now-unused `let topo` binding was dropped.

### 1.3 `cubarium-core/src/world/mod.rs` — the `impl World` block

- **`main`'s intent**: `set_motor_model` / `motor_model`, `set_apex_turn_radius` /
  `apex_turn_radius`, `set_apex_motor_model` / `apex_motor_model`.
- **This branch's intent**: `topology()`, `scale()`, `cell_count()`.
- **Resolution**: both, in that order. A pure additive collision.

### 1.4 `cubarium-core/src/fields.rs` — `EcoScratch::new`

- **`main`'s intent**: a new `plant: Option<Box<PlantBudgetRecord>>` field, `None` in every
  ordinary world (workstream M's per-cell plant budget).
- **This branch's intent**: every scratch array sized from `cells`, not `CELL_COUNT`.
- **Resolution**: `main`'s `plant: None` on this branch's cell-sized arrays. Separately,
  `PlantBudgetRecord::new` sized its own per-cell vector at `CELL_COUNT`; it now takes
  `p.len()`, the opening stock vector's length, which is this world's cell count by
  construction.

### 1.5 `cubarium-core/src/hunter/geometry.rs` — one import line

- **`main`**: `use serde::{Deserialize, Serialize};` (the strike record deserializes).
- **This branch**: `use cubarium_surface::Topology;` beside `use serde::Serialize;`.
- **Resolution**: both.

### 1.6 `cubarium-search/src/es/episode.rs` — the driver dispatch

- **`main`'s intent**: a new `Driver::Control(Control::Dwell(d))` arm, which turned the
  `if let` around `MobileScript` into a `match`.
- **This branch's intent**: `cell_of` takes `(Topology, Scale)`.
- **Resolution**: `main`'s `match`, with `cell_of(Topology::Cube, Scale::ONE, &o.pos)` in the
  arm — the spelling the search crate uses everywhere, since plan §1 keeps it cube-only.

### 1.7 and 1.8 — `tests/ecology_v1.rs` A7 and `tests/snapshot_hardening.rs`

Both sides re-pointed the same two assertions at 17 and wrote a paragraph saying why. Both
paragraphs are kept, `main`'s describing 17 and this branch's describing 18, and both
assertions now read 18. A7's `7..SCHEMA_VERSION` refusal loop covers 16 **and** 17 for free.

## 2. The one design call: schema 18

Two branches independently spent version 17, on two different things, and only one of them
moved a byte. Keeping 17 would have left **two incompatible payload shapes answering to one
number**: a world written by `main`'s build would have reached `decode_exact` and come back as
a *truncation* — its config is two fields short — instead of being refused by name as the
world it is. Wrysk's standing rule of 2026-09-15 is that a new schema refuses old worlds **by
name**, and a number that means two shapes cannot do that.

So the merged build is **schema 18**, refusing 7 through 17 by name, and the history now reads:

| schema | what |
|---|---|
| 16 | ecology v1 (appends `EcologyV1State`) |
| 17 | `main`: the reach-envelope pursuit rule — **semantics only**, shape identical to 16 |
| 18 | the ring world: `WorldConfig` gains `topology` and `world_scale`, config 8 → 9 |

`crates/cubarium-core/src/snapshot/v17.rs` freezes 17 for the one reader allowed inside an old
payload, the `CubeProjection` comparator. It is deliberately **not** a second copy of eighteen
struct definitions: 17's shape *is* 16's, so it is `WorldStateV17 = WorldStateV16` plus
`SCHEMA_V17`, `CONFIG_VERSION_V17` and a `decode_v17` that insists on its own number.
`v16::decode_v16`'s body became `decode_frozen(bytes, want)` and both call it.

`design/flat-world-plan-2026-09-16.md` §4 carries a blockquote recording this; everything else
in that section holds as written.

## 3. `main`'s new code that assumed the cube

Six sites. Five were caught by the compiler; the sixth (§1.2's `founder_body`) was a conflict.

| site | was | is |
|---|---|---|
| `hunter/strike.rs:112` | `StrikeFrame::gather(images: &[Vec<ChartImage>; 5], …)` | `gather(topo, images: &[Vec<ChartImage>], …)` — five charts is a cube's count; the slice is indexed by `Topology::chart_index` |
| `hunter/strike.rs:143` | `ContactEvidence::gather(images, …)` | `…(topo, images, …)` |
| `world/step.rs:1682` | `hunter::surface_reach(images, from, to, MAX_LOCAL_RADIUS)` | `surface_reach(topo, images, from, to, topo.max_local_radius())` |
| `world/step.rs:1788` | `cell_of(&o.pos)` (the intake trace) | `cell_of(topo, world_scale, &o.pos)` |
| `fields.rs:342` | `vec![PlantCellBudget::default(); CELL_COUNT]` | `vec![…; p.len()]` |
| `world/lifecycle.rs` | `founder_body`'s `FACE_EXTENT`, `Face::from_index` and `canonicalize()` | topology-aware (§1.2) |

In `cubarium-search`, which plan §1 keeps cube-only, `Topology::Cube` / `Scale::ONE` were
threaded through `factorial.rs`, `census.rs`, `es/scorecheck.rs`, `es/episode.rs` and
`evaluate.rs`, and `CELL_COUNT` became `CUBE_CELL_COUNT`. One of those is more than a rename:
`evaluate.rs:732` rebuilt the habitat as `Habitat::new(&cfg.habitat, cfg.seed)` and now passes
`cfg.topology, cfg.world_scale` — the world's own pair — so the reconstruction it checks
against tick-0 wood would be right on a ring too.

`main`'s new test files got the same mechanical treatment: `tests/intake_trace.rs`,
`tests/hunter_pursuit_predicate.rs`, `tests/hunter_strike_record.rs` in core and
`tests/diet_factorial.rs` in search.

**Nothing in core's non-test source sizes a per-cell vector from a constant any more**
(`grep CUBE_CELL_COUNT crates/cubarium-core/src` is empty outside `#[cfg(test)]`).

## 4. The cube proof, redone against the new base

FW-2's procedure exactly: a detached `git worktree` of `main` at its head, its own target
directory, the *same* self-contained generator (`examples/export_cube_fixture.rs`, SHA256
`d7a80029…`, unchanged), `WorldConfig::default()` with `seed = 20_260_916`, 6,000 `World::step`
calls. Debug and release produced byte-identical files.

- Fixture: `crates/cubarium-core/tests/fixtures/cube-projection-v17-15a2210.cubw`, schema 17,
  135,447 bytes, tick 6,000, 46 organisms, SHA256 `b891f1ab…`.
- `main`'s head when the export was taken was `15a2210`. The merge's second parent is
  `cdcee03`, one commit later, which differs from `15a2210` by **one line of one design
  document** and by nothing under `crates/` — so the binary is the merge base's code.

**Result.** `first_difference` is `None` against this build's own rerun, and the projection
hashes agree: `10304345502826573087`, the same number FW-2 pinned against `2a1cedd`.

**The old test did not have to be retired.** The brief expected it to stop being equal, since
`main` changed the ecology. It did not: the two fixtures' payloads are identical.

```text
$ cmp -l cube-projection-v16-2a1cedd.cubw cube-projection-v17-15a2210.cubw
     5  20  21
```

One byte, at offset 4 — the schema field's low byte. So both fixtures are kept, both
comparisons run, and `the_two_fixtures_differ_only_in_the_schema_they_declare` pins that
statement about the bytes directly rather than leaving it in prose. Every ecology change on
`main` in those hundred commits is opt-in, or reaches only a hunter member, and a default
world founds no hunter.

`tests/cube_projection.rs` now holds eight tests: the two comparisons, the one-byte fact, the
negative battery, the refusals (both fixtures, both mirrors), the postcard layout rule, the
weather parity claim and the pinned hash.

## 5. Which goldens moved, and why

**No cube frame golden moved.** FW-3's and FW-5's pixel pins, the two committed ring captures
and the whole art-presenter suite pass unchanged, which follows from §4: the world those
frames draw is byte-for-byte the world they were recorded from, and `main` touched neither
`cubarium-render` nor the host presenter.

What did move is a different set, and it moved because of **this branch**, not `main`: every
pin of a *whole-state or whole-config* hash. `state_hash` is FNV-1a over the postcard encoding
of `WorldState`, whose first field is `WorldConfig`; `calibrate::config_hash` is FNV-1a over
`serde_json` of `WorldConfig`. The ring world appends two fields to that struct and takes
`version` 8 → 9, so every such pin had to move and none of them is evidence about a world any
more. FW-2 had already hit this once (`hunter.rs::observations_do_not_move_a_profile…`); these
are `main`'s own pins meeting it for the first time.

Each was re-recorded **with its own evidence beside it**, never silently:

| pin | moved to | the evidence that the world did not move |
|---|---|---|
| `apex_grasp_radius.rs`, `apex_motor_isolation.rs`, `motor_inertial.rs` — `SWEEP_HASHES`, six 1,500-tick boundaries of the shared two-apex `paired_world` | `0x78bf522e3e2508ca`, `0x677b2b7fc059b58b`, `0x1a66ad824401871b`, `0x60b27006eab13f2a`, `0x9199d5c018f53825`, `0x10e84a774cc0c345` | the array gained a **third column**, `projection_hash(CubeProjection::from(state))` at the same six boundaries, and every value in it was **printed by `main` at `15a2210` before the merge** and asserted unchanged here |
| `pursuit_predicate_adoption.rs` — `HALF_SPACE_9K` | `0x239d68ba1ce45828`, `0xb7a3b79d119de6b3`, `0x35503a3b6894f34e`, `0xf46a48a37b85a0db`, `0x657a562fa16eb772`, `0xa689f67f0f9a7b52` | new `HALF_SPACE_9K_PROJECTION`, six values from `main`, not re-recorded; `marks()` now returns both vectors |
| `pursuit_predicate_adoption.rs` — `ENVELOPE_9K` | the same first three, then `0x7714d4b0ed2fdecd`, `0x7ecf395076d4d516`, `0x3f8ddf01f9e57099` | new `ENVELOPE_9K_PROJECTION`, likewise |
| `motor_provenance.rs` and `predicate_adoption_provenance.rs` — `Protocol::default().hash()` | `0x831ca195c697cec8`, from `0x65c51e05060f0d5a` | a new test strikes `"topology"`, `"world_scale"` and `version` 9 → 8 from the config's JSON and reproduces `config_hash` = `18166095531363627169`, `main`'s own value, **exactly** — so those three fields are the whole difference |

**18 projection hashes were obtained from `main`'s build and 18 matched.** The method was
validated first against a known answer: on `main`, `fnv1a(postcard(state)[1..])` — dropping the
one-byte `version` varint, which is all that separates `WorldConfig` from `ConfigProjection`
there — reproduces FW-2's pinned `10304345502826573087` for the fixture world exactly.

**One consequence worth saying plainly.** The protocol hash moving means **every ES checkpoint
and every exported policy written before the ring world is foreign to this build**, by
`config_hash`, exactly as every snapshot before it is refused by schema. That is the
always-fresh rule reaching the trainer, and it is the right answer — a policy trained on a
cube has not been trained on a ring — but it is a real cost and nothing before this merge said
it out loud.

Two more assertions moved for the schema alone, with no world behind them:
`tests/topology_world.rs` (`meta.schema` and the refusal range now read `SCHEMA_VERSION`),
`tests/ring_schema17.rs` (18, and 7..=17 refused; the *path* stays as plan §9 reserved it, with
a module note) and `cubarium/tests/run_persistence.rs`, whose resume-refusal test was renamed
and now relabels a real snapshot **16 and then 17**, because 17 is the one an actual `main`
build could have written.

## 6. What a ring world needs from `main` and does not have

Nothing `main` added is a new *per-cell* vector sized at the cube — the only one,
`PlantBudgetRecord`, is fixed in §3. What is left is scope rather than breakage:

1. **`World::found_body` canonicalizes against `Topology::Cube`** (`world/lifecycle.rs:466`).
   FW-2 named the cube there deliberately and the merge left it alone: it is the genome /
   neural founding door the trainer uses, and plan §1 keeps the trainer cube-only. On a ring it
   would canonicalize a point against the wrong surface. `found_roster`'s door, beside it, is
   topology-aware as of this merge. One word to change when the ring gets a trainer.
2. **`cubarium-search` is cube-shaped in its design, not just its call sites.**
   `factorial::ANCHORS` is eight `(Face, u8, u8)` spread over five faces and
   `the_eight_cells_are_distinct_separated_alive_and_spread_over_every_face` asserts
   `faces.len() == 5`; `too_close` compares two cells only within a face; `Landscape`'s three
   vectors are `CUBE_CELL_COUNT` long. All correct today and all meaningless on a ring.
3. **`StrikeFrame`'s `target_crossed_face` / `hunter_crossed_face`** (`hunter/strike.rs:256`)
   are `p.face.index()` comparisons. On a one-chart world they are always `false` — harmless,
   and not the same quantity as "crossed a seam".
4. **`telemetry`'s `population_by_face`, `producer_by_face`, `detritus_by_face`** are still
   `[_; 5]` (pre-existing, FW-2 left them). A ring reports one chart into slot 0.
5. **Every retained ES row, checkpoint and policy is foreign** (§5). Not a gap to fill — a
   consequence to have said.

## 7. Verification

- `cargo test --workspace --exclude cubarium-gpu --no-fail-fast`: **1,956 passed, 0 failed,
  31 ignored** over 147 test binaries (FW-2 recorded 1,655 / 0 / 25 at its freeze; the
  difference is FW-3's, FW-5's, FW-6's, GS-1's and `main`'s own new tests). Every FW-6 `ring_*` test passes
  (`ring_travel`, `ring_field`, `ring_raster`, `ring_canvas`, `ring_stamp_scale`, `ring_world`,
  `ring_weather`, `ring_schema17`, `ring_sinks`, `ring_care`, `ring_present`).
- `cubarium-gpu` is GS-1b's, still in flight, and was excluded on both sides of the merge.
- `cargo clippy --workspace --exclude cubarium-gpu --all-targets`, compared warning-for-warning
  against the same command on the `main` worktree: **53 warnings in 32 files here, 51 in 30 on
  `main`**. The two extra are `ring_canvas.rs` (`manual implementation of .is_multiple_of()`)
  and `ring_world.rs` (`useless conversion to the same type: f64`) — both FW-6 test files that
  the merge does not touch at all (`git diff b3a1481 HEAD --` on them is empty), so they came
  in with the branch, not with the merge. **No file this merge edited gained a warning, and
  none was lost.** The one clippy **error**, `neural/gru.rs:233` (`0 * HIDDEN` in a
  `#[cfg(test)]` weight setup), is pre-existing and fires identically on `main`; it stops the
  `cubarium-core` lib-test lint on both sides, so both runs cover the same targets.
- The merge was performed in a detached worktree of `b3a1481` under the task's scratch
  directory, because GS-1b held **uncommitted** edits to `crates/cubarium/src/runner/mod.rs`
  in `.claude/worktrees/tachyon-screen` and `main` touches that file too — `git merge` refuses
  outright when a file it must write has local changes, and taking that file out from under
  another worker to get past it is not an option. By the time the SYNC-1 line was ready GS-1b
  had committed (`fca9980`, `54745a0`), so the two lines met in an ordinary merge (`de16e69`)
  rather than a fast-forward. The only file both touched is `runner/mod.rs`; the two hunks do
  not overlap — `main`'s motor-contract check in `seed_neural_animals`, GS-1b's `--sink gpu`
  arm and per-frame `observe_view` — and both are present. `cargo check --workspace --exclude
  cubarium-gpu --all-targets` is clean on the integrated tree. **The main checkout
  `/home/wrysk/wryskware/cubarium` was never written to**; `main` was read through a detached
  worktree under scratch, removed afterwards.
- **`main` has moved on**: `a4ad52d` at the time of writing, four commits past the merge's
  second parent, all of them design documents and no change under `crates/`.

## 8. Commits

| commit | what |
|---|---|
| `b02524f` | the merge itself, with the eight resolutions |
| `99e367d` | the cube proof against the new base: the fixture, its provenance, the comparator |
| `94cb0d2` | schema 18 through the pins both branches held at 17, and the config-derived hashes re-recorded with their evidence |
| `96e66fc` | this report, and the plan's §4 note |
| `de16e69` | the integration merge into `tachyon-screen`, which had moved two GS-1b commits on while SYNC-1 ran |

(The numbers in §7 and §4 were measured on `96e66fc`, before GS-1b's two commits joined it;
`de16e69` changes no file either of the tables above names.)
