---
status: resolved
date: 2026-09-16
owner: Fable
---
> **RESOLVED 2026-09-16.** Landed on main; see git log for the commits.

# Voxel round 2: two producers on the terrain

The first wave (cleared by Astra at 06b4b6f) gives a voxel strip with materials,
free water, pore water and an aquifer, and a presenter that draws it. This round
puts the two producers of `design/voxel-ecology-sketch-2026-09-16.md` §4 on it:
bloomcrown and umbrellafrond, seeded as founder stands, competing for light through
their own canopies and for pore water through the core, spreading by paid propagules,
their litter decomposing back to nutrient. No animals, no glowcap.

The decisive observation is the sketch's: re-draw only the generator's final weak
noise with a new seed, keep the landform, and the umbrellafrond patch should reoccupy
substantially the same hollows while bloomcrown stays on the same ridge. If the
patches move with the noise, the terrain coupling is decorative.

Astra's closing list from the first wave, which this round must satisfy:
debit shared root water through **one bounded core operation** and an explicit loss
term; query **actual supporting faces** and **unrounded water depth**; combine
**geometric sky visibility** from the core with **plant-owned canopy attenuation**;
keep establishment and propagules **paid**; turn any multi-height drainage anomaly
into a tiny fixture before touching the solver.

## Shared boundary

The plant layer is a separate crate, `crates/cubarium-voxel-flora`, whose skeleton
is on main: every public type, the config with placeholder values, the ledger, the
view, `Command::{Seed, Clear}` (implemented) and a no-op `Flora::step`. It reads the
world only through `VoxelView` and changes it only through `cubarium_voxel::Command`.
The presenter builds against that skeleton's public API; the model worker fills in
`step` behind it. **Signatures in the skeleton are fixed**; fields may be added, and
`SpeciesConfig`/`FloraConfig` may gain fields, but nothing public is renamed or
removed without Fable.

### Core additions (`cubarium-voxel`), owned by package E

```rust
// world.rs
Command::WithdrawPore { x: i64, y: u32, z: u32, volume_m3: f64 }
//   Take pore water from one voxel, capped by the stock there. `apply` returns the
//   accepted volume as a NEGATIVE number (the ChargeAquifer convention). Books
//   `Ledger::transpiration_out += accepted`. Non-finite or negative volume: refused
//   whole, nothing booked. Air (capacity 0): returns 0.

// ledger.rs
Ledger::transpiration_out: f64    // in net_in as a loss

// VoxelView
pub terrain_version: u64          // field; World bumps it when SetMaterial changes a voxel
fn is_support(&self, x: i64, y: u32, z: u32) -> bool
//   solid at (x,y,z), y+1 < height, and (x,y+1,z) not solid.
fn supports_in_column(&self, x: i64, z: u32) -> Vec<u32>   // ascending y
fn water_depth_m(&self, x: i64, y: u32, z: u32) -> f64
//   For support (x,y,z): walk up from y+1 while the cell is void and free > 0;
//   sum free × voxel_m. Unrounded. 0 if the cell above is dry.
fn soil_below(&self, x: i64, y: u32, z: u32) -> u32
//   Contiguous Soil voxels from y downward, y inclusive. 0 if (x,y,z) is not soil.
fn pore_water_m3(&self, x: i64, y: u32, z: u32) -> f64   // capacity × pore × voxel volume
fn sky_visibility(&self, x: i64, y: u32, z: u32) -> f64
//   Cosine-weighted fraction of a fixed fan of rays from the centre of the top face of
//   (x,y,z) that leave the world without entering a solid. Fan: zenith (weight 1),
//   eight azimuths at 45° steps at 60° elevation (weight sin 60°) and at 30° elevation
//   (weight sin 30°): 17 rays. A ray leaves the world at y >= height or z outside
//   0..depth; x wraps. March by voxel (DDA or fixed sub-voxel steps of 0.25 voxel;
//   either is fine, say which). Pure geometry: canopy is not here.
```

`Config::noise_seed: u64` (default 0 = derive from `seed` as today). The landform's
"weak correlated wobble" draws from its **own** RNG stream seeded from `noise_seed`
when non-zero, so changing it moves the noise and nothing else (ridge phase, strata,
pockets unchanged). Serde default keeps `voxel.example.toml` valid.

### Plant model (`cubarium-voxel-flora::Flora::step`), owned by package E

Carry `design/ecology-v1-contract.md` §4.0–4.8 (income, demands, maintenance from
income then reserve, growth order with reserve share and emergency reflush,
senescence, dieback, death, paid propagules) and §5's decomposition of litter and
dead wood to nutrient with energy to heat. Drop 3c fruit, 3f transport, 3g
diffusion. Replace the noise fields:

- **Light.** `L = sky_visibility(site) × Π_j exp(−shade_k · P_j / area_j)` over every
  other stand `j` whose crown covers this site's column and whose crown top
  (`support.y + crown_height(W_j)`) is strictly above this stand's crown top;
  `area_j = π · crown_radius(W_j)²` in voxel units, minimum 1. Crown cover: the
  site's (x, z) within `crown_radius(W_j)` of `j`'s (x, z), x wrapped. A bare site
  being considered for a propagule uses its own crown top = support face. Then
  `L_eff = L (1 + light_half) / (L + light_half)`. Cache `sky_visibility` per site and
  recompute only when `terrain_version` changes.
- **Water.** Root box: soil voxels with `|dx| ≤ rooting_radius`, `|dz| ≤ rooting_radius`,
  `support.y − rooting_depth < y ≤ support.y`, x wrapped, z clipped. `μ` = linear ramp
  of the capacity-weighted mean pore fraction over the box between `wilt_pore` and
  `sat_pore`. Demand this tick `= transpiration_m3_per_s · P · μ · DT`. Collect every
  stand's demand per voxel first (share a stand's demand across its box proportional
  to each voxel's pore water), then per voxel issue **one** `WithdrawPore` for the
  total, and split what the core accepted among the demanders proportional to demand.
  Book the accepted volume in `FloraLedger::transpired_m3`. Income uses `μ`; a stand
  that got less than it asked for does not get a second read.
- **Drowning and burial.** `water_depth_m(site) > drown_depth_m` kills the stand
  (§4.7 death: wood to dead wood, foliage and reserve to litter). A site whose support
  is no longer a support (`is_support` false after a terrain edit) loses its stand and
  ground: booked as `removed_*_out`.
- **Propagules.** Recipients are support sites within `hop` of the donor in x and z
  (any y — pick the highest support in each column for now; note it) that are bare or
  establishing **and** pass the species' establishment predicate (`root-box mean pore
  ≥ establish_pore_min`, `sky_visibility ≥ establish_light_min`, `water_depth_m ≤
  drown_depth_m`). A site with no qualifying donor stays bare. Establishing stands
  are frozen exactly as in §3.1/§4.8. Nutrient for construction is deposited in the
  recipient's ground, which is created with `initial_nutrient` the first time
  (booked `seeded_material_in`, as `Seed` already does).
- **Order within a tick.** Snapshot-then-commit for every cross-stand phase (shade
  reads the pre-tick crowns, water demands are collected before any withdrawal,
  propagules take one snapshot of donors and recipients). Iterate stands in site
  order; never `HashMap` iteration.
- **Conservation.** `view.material() − ledger.expected_material()` and
  `view.energy() − ledger.expected_energy()` are raw residuals with no rounding term.
  Keep them at f64 noise (≤ 1e-9 relative). `transpired_m3` must equal the core
  ledger's `transpiration_out` exactly when the flora is the only withdrawer.

### Presenter (`crates/cubarium/src/voxel`), owned by package F

The runner owns a `Flora` next to the `World`, steps it after every `world.step()`,
and passes `flora.view()` to the presenter with the world view. Stands draw as
pixel-art stacks in the existing elevated-orthographic projection at the same
slab/row ordering as terrain so they are occluded by nearer terrain and occlude
farther terrain and water correctly: a trunk of `crown_height` voxels rising from
the support's top face, a crown disc of `crown_radius` voxels at the top, colour by
species, crown fill by `foliage / (α·W)`, a wilt tint by `moisture`, and an
establishing stand as a small sprout. Reuse existing baked pixel art only if it reads
at 4 px per voxel in this projection; a hand-drawn placeholder is fine, textures come
later. Stdin gains `f X Z SPECIES [wood]` (seed), `c X Z` (clear), and `i X Y Z`
prints the stand and ground on that site if any. The end-of-run line adds the flora
residuals.

## Packages

**E — core boundary and plant model.** Opus, high. Main checkout. Files:
`crates/cubarium-voxel/**`, `crates/cubarium-voxel-flora/**`, plus an example
`crates/cubarium-voxel-flora/examples/two_producers.rs` that generates a world, seeds
a few founders of each species, runs N seconds, and prints per-species stand counts,
occupancy by support height quartile, and the two residuals. Land the core
additions first as their own commit (with their tests) so F can rebase onto them.

**F — presenter stands and runner wiring.** Opus, high. Worktree from the skeleton
commit. Files: `crates/cubarium/src/voxel/**`, `crates/cubarium/voxel.example.toml`
(a `[flora]` table is **not** needed this round; FloraConfig::default only). Nothing
under `crates/cubarium-voxel*/`. Until E lands, stands come from `Command::Seed` and
stay static, which is enough to place and draw them.

**G — test authoring and the decisive experiment.** After E and F: an independent
pass at high effort writing the tests the briefs specify but E did not, and running
the noise-reseed comparison.

## Tests each package must leave behind (short function tests only)

E, core: WithdrawPore caps at stock and books the ledger; refuses bad volumes; air
returns 0. `is_support` false for the skyline solid at the top of the world, true for
a roofed floor. `water_depth_m` sums fractional fills and stops at a dry cell.
`soil_below` stops at rock. `sky_visibility` = 1 on an open plain, ~0 on a floor under
a full roof with a 1-voxel gap, in between beside a wall; symmetric under x
reflection on a symmetric fixture. `terrain_version` bumps only on a real change.
`noise_seed` changes only the wobble: with the same `seed`, two worlds differing in
`noise_seed` have identical outlet/spring cells and identical bedrock below y=2.

E, flora: seeded stand on an open dry-ish plain earns income and grows; under a
taller stand's crown its light falls and it diebacks when it cannot pay maintenance;
a stand whose root box is at wilt has μ=0 and, over enough ticks, dies from unpaid
maintenance (short: use a tiny world and a high maintenance); two stands sharing a
root voxel split one withdrawal proportional to demand and the core books exactly the
sum; drowning kills bloomcrown at any depth and umbrellafrond only past its limit;
a terrain edit that buries a support books the stand as removed; a donor with a full
reserve establishes a neighbour and the neighbour's stocks equal what the donor
spent net of construction; both residuals stay at noise over 200 ticks with rain.

F: a seeded stand's trunk pixels appear above its support face and are hidden by a
nearer taller column; crown pixels of a farther stand are not drawn over a nearer
water surface; `f`/`c`/`i` parse.

## Rules

- Fast iteration: small tests, commit message is the report, run only touched crates
  (`cargo nextest run -p cubarium-voxel -p cubarium-voxel-flora`; `-p cubarium voxel`).
- Commit explicit paths only. Never `git commit -a`. Never `cargo fmt` the repo.
- No knob tuning. Placeholder rates are placeholders; list any you had to change to
  make a test meaningful in the commit message, and add them to `design/backlog.md`'s
  user-configurable parameter list.
- Always fresh: no snapshot of the flora this round; if a save format is needed later
  it starts at a new schema.
- Terrain never occludes terrain (landform rule); stands may occlude, that is the point.
- Do not touch `design/handoffs/README.md` (uncommitted user edit) or the cube.
