---
design_status: leaning
last_reviewed: 2026-09-16
decision_refs: []
---

# Voxel first wave: shared boundary and four work packages

Wrysk said go on 2026-09-16 after the [handoff review](fable-voxel-world-first-wave-2026-09-16.md).
Resolved in that thread: voxels are the model, not the look; the art stays pixel art;
Godot is out for this stage; the desktop frontend is the existing web viewer and PNG
sinks; the ringworld strip wraps in x; depth is a config and the full world is deep;
a uniform fixed voxel grid; the water model below. Follow
[WORKING_POLICY.md](../../WORKING_POLICY.md) fast-iteration rules: small function
tests, no reports, commit message is the report, commit explicit paths only.

## The boundary (owned by Fable, in `crates/cubarium-voxel`)

`cubarium-voxel` never draws and never reads the clock. Public surface, already
compiling as a stub:

- `Config { width, height, depth, voxel_m, seed, rain_m_per_s, evaporation_m_per_s, water_substeps }`
  with `index(x: i64, y, z)` (x wraps) and `coords(i)`. Order: x fastest, then z, then y.
- `Material { Air, Bedrock, Rock, Soil }` with `pore_capacity()` and `permeability_per_s()`.
- `World::new(config)` generates; `World::empty(config)` is air over one bedrock row.
  `step()`, `apply(Command) -> f64 accepted`, `view() -> VoxelView`, `save() -> Vec<u8>`,
  `load(&[u8])` (schema tag, refuses other tags, no migration).
- `Command { RainPulse, AddWater, SetMaterial, ChargeAquifer, SetOutlet }`.
- `VoxelView { config, material, free, pore, tick, ledger, aquifer_m3 }` with
  `material_at`, `free_at`, `pore_at`, `surface_y(x, z)`, `stored_m3()`.
  `free` is a fraction of void volume; `pore` a fraction of pore capacity.
- `Ledger { rain_in, user_in, evaporation_out, outlet_out, displaced_out, initial_stored }`
  with `expected_stored()`; residual is `stored_m3() - expected_stored()`.

Add fields when needed and say so in the commit; do not rename or reorder what exists.

## Package A: core (generator, water, ledger, save/load) — `crates/cubarium-voxel/`

Generator: broad ridge and receiving basin first, soil pockets and rock layers, one
overhang and one covered passage, weak correlated noise last, all periodic through
x = 0. Derive soil depth from slope and deposition, not from the elevation noise.
Every carve rechecked for isolated voids.

Water per tick: prescribed rain onto top-exposed air cells; evaporation from exposed
free-water surfaces; free-water substeps: (1) fall into void below, (2) find every
connected region of water-bearing void cells (6-neighbour, including under roofs) and
settle it to one common surface level by sorting the region's cells by y and filling
bottom up, splitting volume across cells at the surface level; infiltration from free
water into soil below by permeability and remaining capacity; drainage from saturated
soil downward and into the aquifer where soil meets bedrock; spring discharge from the
aquifer into a chosen low outlet cell when aquifer head exceeds it, `Q = k * max(head - h_outlet, 0)`;
one named outlet cell that exports free water to `outlet_out` when open. Every transfer
debits once and credits once. Traversal order must not pick a direction. State limits
plainly in the module doc: no inertia, no current, instantaneous settling.

`SetMaterial` moves displaced water to the nearest available void space before booking
any `displaced_out`.

Tests, each a few substeps on a tiny `World::empty` fixture: spill threshold on beds
[0,1,0] with 0.6 / 1.4 / 3.2 units; a U-tube equalizes; a roofed passage fills and the
far side rises; a dry ridge stays dry until overtopped; the same fixture shifted across
x = 0 gives identical stores; residual stays below 1e-9 after rain, evaporation,
infiltration, spring and outlet all fire; save/load round-trips; a wrong schema tag is
refused; generator output at three seeds has no isolated void, a ridge above the
basin, and the seam columns continuous.

Example `examples/basin.rs`: generate the default world, add a rain pulse, step a few
hundred ticks, print the ledger and stores every 50 ticks, write one side-on PNG
(nearest opaque voxel per column, water tinted) with the `png` crate. Runs in seconds.

## Package B: presenter and `cubarium voxel` subcommand — `crates/cubarium/src/voxel/`, `cli.rs`, `run.rs`, `lib.rs`

`cubarium voxel [--config path] [--sink web|png] [--seconds N] [--speed X] [--load path]`.
Loop: fixed 20 Hz ticks with the existing clock, render at the sink rate, ring raster of
`ring:WxH` chosen from config and the pixel scale. Stdin line commands, read on a thread:
`p` pause/resume, `s` step, `+`/`-` speed, `r [m3]` rain pulse, `w path` save,
`l path` load, `i x y z` print material, free, pore, `o` toggle outlet, `q` quit.

Presenter: slightly elevated orthographic projection of the voxel strip into a ring
`Canvas`, drawn back to front (z descending), each visible voxel face as a pixel-art
block of `px_per_voxel` pixels: top faces lit, front faces in the strata palette
(`design/appearance.md`, `crates/cubarium/src/present.rs` palette constants), autotiled
edges from neighbour occupancy so stair-steps read as slopes, free water as a translucent
surface whose fill height comes from `free`, atmospheric tint fading with z. Tilt,
`px_per_voxel` and depth are config with defaults 30 degrees, 4 px, from the world.
Wrap: draw columns beyond the strip edge from the wrapped column so the seam is
continuous. Screen: the whole strip fits the raster width; if the projected height
exceeds the raster, crop from the top and say so once.

Works against the stub crate: use `World::empty` plus `SetMaterial` to author a ridge,
a hollow and an overhang for your own checks until Package A lands. Tests: projection of
a known voxel lands on the expected pixel; back-to-front order hides a voxel behind a
nearer one; a wrapped column draws identically at x and x + width; the seam pixel
columns match. A few frames, no long runs.

## Package C: art storyboard — `art/studies/voxel-storyboard/`

A Python (PIL) script that renders a hand-authored strip at native 1920x1080 as pixel
art in the current direction (`design/appearance.md`, `design/landscape-plan-2026-09-16.md`
visual finding, sprites from `assets/atelier/` per `pack.json`): ridge, hollow with a
pool, terrace, one overhang, a stream falling into the pool. Matrix: tilt 25 and 35
degrees, depth 16 and 32 voxels, `px_per_voxel` 3 and 4. Paste a spiretree, a glowcap
patch, two creatures at different depths so scale reads. Output PNGs to the study
directory, small and committed; one `README.md` with the matrix and what each cell
shows. No simulation; the projection is `sx = x * s`, `sy = base - y * s - z * s * tan(tilt)`.
The point is to look at it and pick tilt, scale and depth.

## Package D: ecology sketch — `design/voxel-ecology-sketch-2026-09-16.md`

One page. A compact candidate community for the strip: two producers with different
light and soil-water needs, one decomposer with an explicit substrate, one browser, one
detritivore, one predator, with paid local propagules and diets by identity. Name the
fields terrain must expose per surface cell or voxel: light, soil water, slope, water
depth, cover, substrate. Say which existing core mechanisms carry over
(`design/ecology-v1-contract.md`, `design/ecological-niches-reconsideration-2026-09-15.md`)
and which do not. No training, no numbers beyond orders of magnitude, `exploration`.
