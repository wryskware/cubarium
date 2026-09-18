---
status: resolved
date: 2026-09-16
owner: Fable
---
> **RESOLVED 2026-09-16.** Landed on main; see git log for the commits.

# Voxel round 3: three substrate corrections before any new species

Round 2 (main at 1642223, brief `voxel-producers-briefs-2026-09-16.md`) put two
producers on the terrain and showed, with the landform held and only the
generator's noise re-drawn, that both species reoccupy exactly the same columns
(Jaccard 1.000) while a different landform moves them (0.83 / 0.62). Its own
qualifications and Astra's theoretical biosphere
(`design/theoretical-biosphere-2026-09-16.md`, §1, §3, §6) point at the same three
holes in the plant model, all inherited from ecology v1. This round fixes them while
the flora crate is 2,400 lines. **No new species, no animals.** Wrysk chose this
middle path over "keep going" and "everything at once" on 2026-09-16.

1. **Frozen establishing stands are not a seed bank.** Propagule material sits on a
   site forever, cannot die, and needs one donor's full attention for 300 s to cross
   `alive_min`. Nothing in the round-2 experiment ever established or died.
2. **Saturation only ever helps a plant.** Umbrellafrond gets the basin for free and
   bloomcrown has nothing to lose there, so bloomcrown's establishment predicate
   passes 94 % of columns. Chesson's test (can the rare species recover while the
   other stands?) needs the reverse to hold somewhere.
3. **Respired wood becomes fertilizer.** The v1 material ledger books maintenance
   and construction respiration into the site's nutrient `N`. Organic matter and
   mineral nutrient are the same number. Astra: "it must not teach that respiring
   a kilogram of wood produces a kilogram of fertilizer."

Do them in the order 3, 1, 2: the ledger change first, because the other two book
into it.

## Shared boundary

Everything stays in `crates/cubarium-voxel-flora` and its tests; the core
(`cubarium-voxel`) does not change this round unless a correction below needs a
query it lacks, in which case add the query with a test and say so. Public paths
in `lib.rs` may gain fields and types; renames only where this brief names them.
The presenter (`crates/cubarium/src/voxel`) is touched only to keep it compiling
against added fields and to print the new stocks in `i X Y Z`.

### Correction 3: organic matter and one limiting mineral

Every living and dead stock becomes **organic matter** `O` (carries energy at the
tissue's density, as today's "material" does) plus a fixed **mineral content** per
unit of tissue, `n_tissue` (one number per species; wood, foliage and reserve share
it this round). The site holds a mineral pool `Ground::mineral` (rename of
`nutrient`). Rules:

- Growth of `ΔO` tissue draws `n_tissue · ΔO` mineral from the site's pool and is
  capped by it: `A_pot` gains the term `mineral / n_tissue` alongside the existing
  Michaelis–Menten on the pool.
- Respiration (maintenance, construction cost `c_g`, reflush) removes organic
  matter to **heat** and to a new ledger term `FloraLedger::respired_out` (organic
  matter leaving the closed system; the boundary Astra calls the CO2 supply). It
  releases **no** mineral: the tissue still standing keeps its mineral.
- Death and litterfall move organic matter and its mineral together into litter
  and dead wood (`Ground::litter_mineral`, `Ground::dead_wood_mineral`).
- Decomposition of litter and dead wood respires their organic matter
  (`respired_out`, energy to heat) and releases their mineral to the site pool
  at the same fraction. Mineral is conserved exactly; organic matter is
  conserved up to the named boundary flow.
- Assimilation creates organic matter from light: `A` is booked as
  `FloraLedger::fixed_in` (organic matter entering; today's `light_in` is its
  energy). Two residuals replace one: `organic − expected_organic` and
  `mineral − expected_mineral`, both raw, both at f64 noise. `FloraView::material`
  becomes `organic`, and `mineral` is a new sum.
- `initial_nutrient` becomes `initial_mineral`. `Seed` books the founder's
  organic matter and mineral as seeded.

Keep `n_tissue` a placeholder per species (say 0.02) and add it to the backlog's
parameter list. Nothing is tuned.

### Correction 1: paid dormant seed cohorts

Replace `Stage::Establishing` with a **seed bank per site**:
`Ground::seeds: Vec<SeedCohort { species, organic, mineral, age_ticks }>` (sorted by
species then age; at most a few cohorts per site, merge same-species cohorts within
one tick of age). Rules:

- A donor's paid propagule package lands as a cohort, not as a frozen stand. The
  package is what §4.8 already pays (donor reserve minus construction, which is
  respired); a cohort carries organic matter and its mineral.
- Cohorts age. Each tick a cohort loses `seed_attrition_per_s · DT` of itself to
  litter (paid decay, not deletion). A cohort older than `seed_max_age_s` goes to
  litter whole. Both per species.
- Germination: when the site passes the species' establishment predicate (as
  today, plus correction 2's aeration bound) **and** the site's cohorts of that
  species together hold at least `alive_min / w_frac` organic matter, one stand is
  born from them: `wood = w_frac · S`, `foliage = p_frac · S`, `reserve = q_frac · S`,
  where `S` is the pooled cohort organic matter, and the cohorts are consumed. Its
  mineral moves with it. Ledger counter `establishments += 1`.
- Sites with a living stand still receive cohorts (they wait for the gap).
  Cohorts on a site whose support disappears are removed and booked.
- `Stage` keeps `Alive` only; delete `Establishing` and every branch on it. The
  presenter's sprout glyph moves to "a site with a seed cohort" so the picture
  still shows waiting propagules.

### Correction 2: root-zone aeration

Each stand carries `aeration_stress: f64` in `0..=1`. Each tick, over its root
box, take the fraction of root voxels at `pore ≥ saturated_pore` (a species
constant, placeholder 0.95); stress rises by `stress_rate_per_s · DT · fraction`
and relaxes by `relax_rate_per_s · DT · (1 − fraction)`, clamped. Effects:

- Income multiplier `(1 − stress)`, alongside `μ` and light. A waterlogged
  bloomcrown earns nothing and diebacks on unpaid maintenance, which is the death
  the sketch wanted ("roots drowned") without a new kill switch.
- Establishment predicate gains `stress-free`: germination requires the site's
  current saturated fraction to be at most `establish_saturated_max` (species).
- Umbrellafrond's tolerance is a slow `stress_rate` and a high
  `establish_saturated_max`; bloomcrown's the reverse. Placeholders; backlog.
- Standing-water drowning (`drown_depth_m`) stays as it is.

## Tests each correction must leave behind (short function tests only)

- 3: a stand that respires for N ticks with no income loses organic matter, the
  site pool gains no mineral, `respired_out` equals the loss; a decomposed litter
  cohort releases exactly its mineral; growth stops when the pool's mineral is
  exhausted while light and water are ample; both residuals at noise over 200 ticks
  with rain and a founder.
- 1: a donor's package appears as a cohort with the donor's debit equal to the
  cohort plus construction; a cohort on a site failing the predicate ages and
  decays to litter and never becomes a stand; a cohort on a passing site
  germinates at the threshold tick into a stand whose stocks equal the pooled
  cohorts; a stand is born on a site that a living stand vacated by dying.
- 2: a bloomcrown over a saturated root box reaches stress 1 and earns nothing;
  it relaxes after the water table drops (use ChargeAquifer negative); an
  umbrellafrond on the same box stays under its stress ceiling; bloomcrown cannot
  germinate on a saturated site and umbrellafrond can.

## Then

- Rerun the round-2 experiment (`two_producers compare`) long enough for a
  second generation (establishments > 0 and deaths > 0 in every arm; expect
  several hundred simulated seconds in `--release`; say how long). Report the same
  overlaps plus, per species, the fraction of its stands that are descendants
  rather than founders, and Chesson's probe: seed one species alone, let it fill,
  then add one founder of the other in its best habitat and report whether the
  newcomer's descendants establish.
- Astra round on the slice (`task --resume-last` on the voxel review thread).

## Packages

**H — the three corrections**, Opus high, main checkout, files under
`crates/cubarium-voxel-flora/**` plus the minimal presenter compile fix and
backlog rows. Three commits minimum, one per correction, ledger first.

**I — independent test pass and the experiment rerun**, Opus high, worktree from
H's last commit, same rules as round 2's G.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; no knob tuning (placeholders
named in commit messages and the backlog); always fresh; do not touch
`design/handoffs/README.md` or the cube; never HashMap iteration.
