---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
---

# Terrain generation — worker briefs

Plan: [terrain-generation-plan-2026-09-21.md](../terrain-generation-plan-2026-09-21.md),
reviewed and accepted for implementation by Fable on 2026-09-21 with the
decisions below. Branch `terrain-generation`, worktree
`.claude/worktrees/terrain-generation`. Slices land in the plan's order; each is
one commit series on this branch, the commit message is the report.

## Decisions made at review (apply to every slice)

- **Recipe placement.** A serde `Recipe` in `cubarium-voxel` holds every
  generation-only parameter in metres (wavelengths, relief, persistence,
  lacunarity, octave count, later erosion budget) plus named per-pass seed
  streams. `Config` gains a `landform: Landform` field: `Landform::Ridge` is the
  present `generate::landform` unchanged and stays the default, so no fixture,
  harness or test moves in slice 1; `Landform::Staged(Recipe)` is the new
  pipeline. Selected from `[world.landform]` in the host's voxel TOML. Runtime
  water/ecology settings stay where they are in `Config`.
- **Physical units are the contract.** Feature size is fixed in metres by the
  recipe, not by `width` or `voxel_m`. Doubling `width` must add landforms;
  halving `voxel_m` must resolve the same landforms finer. Octaves finer than
  about two voxels are dropped, not aliased.
- **Topology.** X is periodic in every pass, including domain warping and any
  later erosion flux; front/back are walls. Periodic noise must be exactly
  periodic at the ring circumference (lattice period = circumference in metres),
  not a crossfade.
- **Diorama constraint stays.** `visibility_pass` and the front-low/back-high
  landform rule (memory: camera 30°, front never occludes back) still apply
  after the new relief, as an explicit habitat-preparation step. The three
  existing camera/overhang tests must pass for staged rings too.
- **Three named presets** ship with slice 1, as constants on `Recipe`:
  `small` (Tachyon: 160 × 48 × 24 at 0.125 m, 20 m circumference),
  `default` (128 × 48 × 24 at 0.25 m, 32 m) and `wide` (256 × 48 × 24 at
  0.25 m, 64 m). They differ in resolved octaves and landform count, never by
  squeezing the wide landscape into the small ring.
- **No erosion in slice 1.** Material layering (soil from slope, rock, strata,
  bedrock floor, pockets) is carried over from the ridge generator onto the new
  heightfield so the ring is habitable and renderable; slice 2 replaces the
  slope-derived soil with eroded/deposited sediment.
- **Fast iteration policy applies** (`WORKING_POLICY.md`, 2026-09-16 and
  2026-09-17): tests exercise one function on a small world for at most a few
  hundred ticks; run only the crate you touched; no report files, no captures
  in the repo; recipes live in source.

## Slice 1 — staged native ring generator

Owner: generator worker (Opus, high). Files: `crates/cubarium-voxel/src/`
(`generate.rs`, `config.rs`, new `recipe.rs` / `noise.rs`), the host TOML
loader in `crates/cubarium/src/voxel/mod.rs` only as far as `[world.landform]`
needs it, and `config/tachyon/voxel.toml` left on `Ridge` (the panel switches
in slice 3).

Deliverable: `Landform::Staged(Recipe)` producing periodic multi-octave relief
in metres (broad relief, ridged noise in rocky regions with modest periodic
domain warp, intermediate spurs/hollows, restrained fine detail; 4–6 resolved
octaves on `default`), voxelised through the existing layering and
`visibility_pass`; three presets; `cubarium voxel --sink png` renders a
staged ring when the TOML selects it.

Tests to write **before** the generator (they define done; keep each under a
second):

1. Seam: for every preset and three seeds, the surface height and material
   column at `x = width-1` and `x = 0` differ by no more than the largest
   neighbouring step elsewhere on that ring, and the noise field evaluated at
   `x` and `x + circumference` is bit-equal.
2. Bounds: every surface height lies in `[FLOOR_Y, height-5]`; the floor row is
   bedrock; no `Air` below a solid in any column (no overhang), as the ridge
   test already asserts.
3. Same seed, same ring: two `World::new` calls with an identical staged
   `Config` yield identical `material` arrays.
4. Physical invariance: (a) the `default` recipe at `width = 128` and
   `width = 256`, same `voxel_m`, has the same distribution of heights within
   ±10 % in the shared 32 m and roughly twice the count of local maxima over
   the ring; (b) the same recipe at `voxel_m = 0.25` and `0.125` (width
   doubled to keep the circumference) has surface heights in metres that
   agree within one coarse voxel at every coarse column.
5. Camera: the three existing tests in `generate.rs` (`no_surface_cell_is_hidden…`,
   `the_front_stands_below_the_back…`, `…no_overhang`) run against each staged
   preset as well as `Ridge`.
6. Ridge unchanged: with `Landform::Ridge` the default `Config` still produces
   the byte-identical `material` array to `main` for seed 1. (One-off check
   during development against a pre-change build; do not commit a hash pin.)

Visual check for the integrator: one PNG frame per preset for seed 1 and 7,
written to the scratch directory named in the message, produced with
`cargo run -p cubarium -- voxel --sink png --seconds 0.1 --out <dir>` and a
TOML per preset. Not committed.

Return (≤40 lines): commits, which tests were red first, any place the plan's
requirements conflicted with the code, and the PNG paths.

## Slices 2–5

Briefed after slice 1 is integrated. Slice 2 (erosion and soil) will be a
separate authoring pass for its fixtures (sediment accounting, periodic flux,
tiny slope/basin fixtures) before implementation.
