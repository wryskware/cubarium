---
status: open
date: 2026-09-21
owner: Fable (orchestration); one Opus worker, read-only diagnosis
---

# Cone autopsy: why the trained browser's cone goes blank on the living world

## Why this package exists

The frondgrazer's search behaviour is shipped: the P3-C wander seed plus ES on
the Stage B arena (8/8 reacquisitions, `voxel-senses-phase2-briefs-2026-09-19.md`
note 6), retrained with the vertical mouth reach (gen390, .840,
`voxel-browser-reach-2026-09-21.md`). It is the built-in default founder
(`crates/cubarium/assets/policies/frondgrazer-p3d-reach-gen390.json`).

It does not transfer. With gestation in place the browsers no longer strip the
canopy (`voxel-reproduction-2026-09-21.md`: 13 deaths, foliage never below 10),
yet the lineage is extinct by minute 35 with 10–12 organic of foliage standing.
D3 (`design/7_Research/voxel-census-2026-09-20.md`) measured: at death every
browser had a live stand inside its 2 m cone range (mean 0.957 m) and all three
sectors read **zero foliage**. In the arena a blank cone means open flat ground;
the ray tracer (`crates/cubarium-voxel-fauna/src/senses.rs`, `ray_first_hit`)
also reports a wall for three things the arena never contained:

- any solid voxel (a slope ahead ends every ray in that sector),
- any cell with `free > 0` (standing water, or a film after a shower),
- a grazed crown with `foliage == 0` (`cone_occupancy` marks it `Occluder`).

All three collapse into `Class::Occluder`, so the policy's input cannot tell
them apart, and neither can our instrumentation. This package measures which
wall the browser is looking at, on the world that actually ships.

## The world that ships is not the world the autopsies ran on

`cubarium voxel` with no TOML now generates the staged `default` preset (0.25 m
cells, `Preset::find("default").config()`, 3d80bb4); the Tachyon panel runs
`preset = "small"` (0.125 m, `config/tachyon/voxel.toml`). Both carry their own
water inventory and scheduled showers (`Recipe.water`, 3e2aa01). The examples
`voxel_founder_autopsy` and `voxel_census` still build `Config::default()`, which
is the old ridge at 0.125 m with the flora harness's closed budget. Every number
in D3 and the reproduction note is from that ridge.

The trained centres were trained at 0.125 m. The browser's manifest is in metres
(`cone_range_m` 2.0, `body_length_m` 0.25, mouth reach 0.0625 m) but its mouth
probe and vertical reach are in voxels (`mouth_columns`, `mouth_reach_up_voxels`,
`crates/cubarium-voxel-fauna/src/body.rs`), and the cone origin is
`standing_y + 1.5` voxels. On the 0.25 m preset those are different animals.
Measure it; do not fix it.

## Objective

Read-only. Say, with numbers, why the browser's cone reads no foliage while
foliage stands within range, on the shipped landscapes, and whether its wander
covers ground there.

## Deliverables

1. **Landscape arm for the diagnostics.** `voxel_founder_autopsy` (and
   `voxel_census`, same code path) accept `preset=<small|default|wide>`: build
   the world with `cubarium_voxel::Preset::find(name).config()` exactly as the
   host does (`crates/cubarium/src/voxel/mod.rs`, `VoxelConfig::default()` and
   `ambient_world_config`), the recipe's water as given, no harness overrides,
   same seeder (`habitat::seed_with_founder_counts`), same settle
   (`Senses::settle`), same built-in founders (`install_default_founders`).
   Keep `generated closed` unchanged as the ridge baseline. If the host does
   anything else to the world before the first tick (outlet, settle ticks, the
   viability check), the example must do the same; read `mod.rs` and say what
   you matched.

2. **Ray-class census.** A diagnostic in `cubarium-voxel-fauna` (`pub` in
   `lib.rs`, beside `browser_cone_readings`) that runs the browser's exact ray
   fan from its exact origin and returns, per ray, the first hit as one of:
   `Clear`, `Terrain`, `Water`, `StrippedCrown`, `FoliageCrown`, `Body`, with
   distance. It must reuse the cone geometry (manifest, origin, substep,
   `RAY_STEP_CAP`) so it sees what the policy sees; refactor `ray_first_hit`
   to expose the fine class internally if that is the cleanest way, but the
   `Class` the policy's `SectorReading` is built from must not change, and the
   readings must be unchanged (an assertion in a ≤200-tick test: the fine
   census maps onto the coarse reading exactly).

3. **Per-minute CONE rows, extended.** For every living browser: per sector
   clear / terrain / water / stripped / foliage / body fractions and the mean
   distance of the nearest foliage hit; plus the aggregate over browsers.

4. **At each browser death** (ledger `Departure`, by cause): the same ray
   census; the nearest living crown by straight line, its planar distance, its
   crown layer relative to the browser's head layer (voxels, signed), whether
   that crown lies inside the pitch band (±20°) at that distance, and if a ray
   toward it is blocked, by which fine class at what distance.

5. **Wander coverage** per browser per minute: planar metres travelled,
   distinct columns visited, share of ticks with `|turn| > 0.1`, legal exits
   (`exits` exists in the example), and the share of minutes with all three
   sectors at zero foliage before the death.

6. **Three arms, 60 simulated minutes each**, default founder counts:
   `preset=small` (the panel), `preset=default` (the ambient default),
   `generated closed` (ridge baseline; must reproduce the reproduction note's
   13 browser deaths within the usual seed noise, else stop and report). Report
   one table: browser deaths, extinction minute, foliage standing at each death
   cohort, dominant first-hit class at death, mean nearest-crown distance and
   height offset, wander metres per minute.

7. **Research note.** Append a `## D4 — cone at the wall` section to
   `design/7_Research/voxel-census-2026-09-20.md`: the table, the one-line
   diagnosis, and "proposed, not made" candidates ranked by the evidence. Any
   fix — occlusion rule for thin water, training on landscape slices, pitch
   band, voxel-scaled manifest — is Wrysk's decision, not yours.

## Constraints

- Read-only on simulation semantics: no change to what any controller
  senses, to `body.rs`, `controller.rs`, `manifest.rs` values, `config.rs`, the
  seeder, water, or flora. Examples, a diagnostic function, and a test only.
- No knob tuning; no byte-identical or golden-hash tests; new tests ≤200 ticks.
- Work in worktree `.claude/worktrees/cone-autopsy`, branch `cone-autopsy`
  from main at `4b985d8`; `CARGO_TARGET_DIR` inside the worktree. Commit with
  explicit paths only (never `git add -A` or `commit -a`); end every commit
  message with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`.
- All cores are yours for the runs (`-j 24`, three processes in parallel).
- Do not open display windows.

## Verification

`cargo nextest run -p cubarium-voxel-fauna -p cubarium` green; the coarse-vs-fine
mapping test; the ridge arm reproducing the reproduction note's browser deaths
(13 ± seed noise) and residuals ≤ 1e-9 on both ledgers.

## Return format (≤40 lines)

The table from deliverable 6; the diagnosis in one sentence; which fine class
dominates at death per arm; the wander numbers; the commit list; what you could
not measure and why. Evidence over conclusions: I will re-run one arm.

## Integration note (Fable, 2026-09-21)

Landed: 9f7a6cb (fine ray classes behind the unchanged coarse mapping, a
200-tick test that the census maps onto the policy's reading exactly), 2f1cfed
(`preset=<name>` arm in both examples, built as the host builds it through a
new `voxel::ambient_world`; BCONE/CONEX/WANDER/DEATHCONE rows), 3344d50 (D4 in
`design/7_Research/voxel-census-2026-09-20.md`). 757 tests green in the two
crates. Fable re-ran `preset=small` for 60 min and reproduced the worker's arm
exactly: 30 stands seeded, 12 browser deaths all starved (first 14.8 min, last
52.1), one browser alive at 60, residuals ~1e-11.

Corrections to this brief's premises, both found by the worker: since 3d80bb4
the host's `VoxelConfig::default().world` already *is* the `default` preset,
and `World::new` writes a staged recipe's water over the harness rain, so
`generated closed` has been a 0.25 m staged world (not the ridge) since the
terrain merge; and the "foliage 8.05 → 0.43" line in Fable's baseline message
was the litter column (foliage there is 4.15 → 0.12).

Finding: the cone is mostly **clear** at death in all three arms; it reads no
foliage because the nearest living crown is almost never at once within 2 m,
inside the ±20° pitch band and unoccluded (4 of 51 deaths). On the panel's
`small` (0.125 m) the crowns are scaled to metres by
`FloraConfig::for_voxel_size` while the browser's eye (1.5 voxels) and mouth
reach are in voxels, so 8 of 12 unoccluded crowns sit at +36° above an eye
0.19 m off the ground. On `default` (0.25 m) the nearest crown averages 1.67 m
away, out of range for 11 of 24, and ground pools are the commonest blocker.
Water is minor everywhere. Wander covers ground (3–10 m per browser-minute, up
to 271 m in a life); it is not penned. Candidates ranked in D4; the first one,
vertical geometry in metres, is a decision for Wrysk because it is the sensor
interface. Not measured: whether a visible crown would be eatable (D3's
mouth boundary is untouched).
