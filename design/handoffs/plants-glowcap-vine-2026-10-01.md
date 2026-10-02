# Plants: glowcaps colonise the dark, latticevine founds on tall walls and grows faster — 2026-10-01

Wrysk, 2026-10-01: "can we make glowcaps somehow really love growing in caves and dark
overhangs? like they can just colonize in there." / "not enough or bad founding locations
for latticevines. founding spots should prioritize the longest vertical faces … increase
the growth rate. in the world i have running live … i cant really tell where any
latticevine is." He approved the proposal below ("go ahead").

Worktree `.claude/worktrees/plants`, branch `plants` off main. Commit with explicit paths;
never merge, never deploy, never touch the live world. Follow `WORKING_POLICY.md` (fast
iteration, **every number is a knob**: serde field + doc with unit + registry row, regenerate
`design/parameters.md`). Plausible defaults, no tuning to today's ecology. The `roster`
branch also edits `cubarium-voxel-flora` (glassfilm in `lib.rs`/`step.rs`): keep hunks
local so the merge stays small.

## A. Latticevine (`crates/cubarium-voxel-flora/src/cover.rs`)

1. **Tallest walls first.** `Flora::seed_vine_founders` (cover.rs ~1911) shuffles every
   eligible foot/lip candidate and takes the first that keep `founder_spacing`. Instead give
   each candidate its wall height — eligible faces counted up the wall from a foot (`up`,
   stairs included) or down from a lip (straight down or `stair_down`) — and take
   candidates tallest first, the keyed shuffle only breaking ties. Spacing and the
   existing minimum heights stay.
2. **Founders scale with the world.** New `VineConfig::founders_per_m2` (founders per m² of
   floor, `width · depth · voxel_m²`), default **0.0625** (the 192 m² desktop terrarium →
   12). Founders = `max(founders, round(founders_per_m2 · area))`; `founders` (6) stays as
   the floor.
3. **Grows ~3× faster.** `VineConfig::assimilation` 9e-5 → **2.7e-4**. Update the default's
   doc arithmetic (pace per face, doubling time) to match.

Not in this package: the co-training `K_bio` gene scales the vine's upkeep but not its
income (`flora.latticevine.assimilation` is missing from its path). That is the cotrain
thread's crate; leave it, it is noted for them.

## B. Glowcap in the dark (`SpeciesConfig`, gates, `step.rs` income, spores, founding)

A glowcap today lives only on dead wood + litter in its box, and needs soil in the root box
for water, so caves (no wood, often rock) are closed to it. Add a **dark habitat**. New
`SpeciesConfig` fields, all **off (0) for every species except glowcap**, so other species
and a glowcap with them zeroed behave exactly as today:

- `dark_sky_max` (glowcap **0.45**), `dark_sky_full` (**0.1**): terrain sky visibility
  (`VoxelView::sky_visibility`, the memo flora already keeps) at/above which a site is not
  dark, at/below which it is fully dark. `darkness = clamp((max − sky)/(max − full), 0, 1)`.
- `dark_uptake_per_s` (**0.008**): seep income — organic matter per unit of mycelium per
  second at darkness 1 and μ 1 (the same as a log at full moisture, 0.4 · 0.02, so the same
  ~3× margin). Income adds to the log income, is capped like photosynthesis at what the
  stand can spend this tick (maintenance + build + reserve room), and **brings its own
  mineral at `n_tissue`** (drip water carries both). Book it as a new external input in
  `FloraLedger` (organic, mineral, energy = `energy_density ·` organic) beside the light
  input, so every ledger closure still holds.
- `dark_damp` (**0.8**, available-water scale, 1 = field capacity): a dark site reads at
  least `dark_damp · darkness` of available water — cave air is saturated, rock weeps. It
  applies to the gates and to μ, **including a box with no soil voxels** (a dark rock face
  passes the pore gate on it). The glowcap transpires nothing, so no water is created.
- `dark_establish_min` (**0.5**): darkness at/above which the substrate gate opens without
  wood or litter.
- `dark_spore_bias` (**4.0**): spore landings favour dark faces — weight `1 + bias ·
  darkness` (or the equivalent for how `spore_target` actually picks). 0 must give today's
  picks exactly.
- `dark_founder_share` (**0.5**): that share of the glowcap founders goes to the darkest
  faces that pass the gates (spaced as founders are now); a dark founder gets **no** laid
  dead wood. The rest found as today.

Drowning, the aeration ceiling and everything else about the glowcap stay as they are.

## Tests first (one commit, failing), then the implementation

Small function tests only, a few hundred ticks at most:
1. darkness at sky 0, `dark_sky_full`, the midpoint, `dark_sky_max`, 1 → 1, 1, 0.5, 0, 0.
2. A glowcap on a fully roofed **rock** face with no soil, wood or litter passes
   `establishment_gates`; the same face with the dark knobs zeroed does not; an open face
   with no wood is still refused.
3. A dark glowcap with no substrate grows over a few hundred ticks and the organic, mineral
   and energy ledgers close with the seep line in them.
4. `dark_spore_bias` 0 reproduces today's landing sites; > 0 lands a larger share on dark
   faces over many packages.
5. Vine founders on a fixture with a 6-face wall and a 3-face wall, `founders` 1 → roots on
   the 6-face wall; the per-m² count with its floor.
6. Registry coverage passes with the new fields.

`cargo nextest run -p cubarium-voxel-flora` (and any crate whose tests you touched).

## Checks and return

- **t = 0** on `config/desktop/terrarium.toml`, seeds 1–4: dark support faces
  (darkness ≥ 0.5), how many pass the glowcap gates before/after, rock vs soil; the
  founders' wall heights against the candidates' height distribution.
- **Short run:** seeds 1–4 × 2 h, main vs branch, with
  `params=/home/wrysk/wryskware/cubarium/runs/live-view/a-g12/mean.toml` (Wrysk's live
  genome; `voxel_census` takes `params=`). Glowcap stands dark vs lit and vine covered
  faces + m² at 30/60/120 min. Stop a seed early once settled. At most 12 logical CPUs.
- **One picture:** a headless GPU capture (`--gpu-target headless --gpu-capture DIR`) of the
  branch at minute 60 showing a cave/overhang with glowcaps and a wall with vine; absolute
  paths. Never open a window (AGENTS.md).
- Return ≤ 40 lines: commits, the numbers above, anything that refused a decision here.
  Stop your wait loops; leave no background shells.
