---
status: open
date: 2026-09-22
owner: Fable (organism line), under Wrysk's delegation of the nine decisions
package: 4 of design/handoffs/voxel-organism-decisions-2026-09-21.md §9 / audit §10
---

# Package 4 — startup acceptance

## Why

After packages 0–3 (observer, step rule, bodies in metres, layers, water units,
diets) the shredder lineage holds six hours on `default` (12 alive, census
"Diets, 2026-09-22") and the browser lineage is gone by hour three on every arm.
The census says why, twice: the seeded browsers' walkable component on `default`
holds no bloomcrown, so reach rose with layers and the **route did not move at
all** (0.223, "Layers, 2026-09-22"); and `browser_faces` still admits a face by
the **crown-top voxel** against the mouth band, not by the layers the tick's
bite actually reads. D5 adds the third: the seeder judges water and eligibility
**before the first shower** — the first shower then drowned 14 of `small`'s 27
stands and 3 of `default`'s 87 — and on the driest reading of the world.

Decisions §8 (binding here): *a seeded world is accepted only if each animal
lineage's founders stand in a walkable component holding edible stock of at
least N founder-hours of upkeep with living producers of it, N a backlog
placeholder. Rejection re-draws the seed like the lake gate does.*

## What to build

Read first: decisions doc §§1–3, 8; audit
`design/7_Research/organism-systems-audit-2026-09-21.md` §7 and §10 row 4;
`crates/cubarium/src/voxel/habitat.rs` (`seed_with_founder_counts`,
`browser_faces`, `watch_the_stream`); `crates/cubarium/src/voxel/mod.rs`
(`ambient_world`, `generate_with_a_lake`, the host's `habitat::seed` call near
l. 793); `crates/cubarium-voxel-fauna/src/encounter.rs` (`standable_faces`,
`walkable_components`, `reachable_layers_of`, `mouth_crown_layers_at`);
`crates/cubarium/examples/voxel_edible_stock.rs` (its `route` computation).

1. **Seed after the first shower has fallen and drained.** A fresh world's
   startup pre-roll runs headless, before anything is seeded: settle as today,
   then **the world opens with a shower** — the first shower is due at the
   start of the pre-roll instead of at the drawn 300–900 s (later showers are
   drawn exactly as today). The pre-roll ends when that shower has finished and
   the water has settled again (reuse `World::settle`'s convergence, capped).
   `watch_the_stream`'s "deepest water each face saw" spans the **whole
   pre-roll**, shower included, so a species is only offered a face it could
   have stood through that shower on. Establishment and eligibility gates read
   the drained world. Decided because the alternative — the drawn first shower
   — costs up to 18,000 ticks of pre-roll, which on the Tachyon board is
   minutes of startup per try. Visible effect: none on screen; the world the
   viewer first sees is the drained one, with life on it.
2. **The checked face is the seeded face.** `FloraCommand::Seed` and
   `FaunaCommand::IntroduceFounder` today take `{x, z}` and resolve the highest
   support, while the seeder checks every support face (caves, shelves). Make
   the seeder's commands name the face it checked (carry `y`, or refuse when
   the resolved face differs — your call; carrying `y` is preferred because it
   lets cave floors be founder sites). Live propagation and other callers keep
   their behaviour.
3. **Browser faces by layer, not crown top.** `browser_faces` admits a face
   when the adult body's mouth, standing there, reaches a **foliage layer with
   stock** of a stand the browser's diet accepts — the same function the
   tick's bite uses (`reachable_layers_of` / the diet gate), not
   `crown_voxels` against the band.
4. **The acceptance check, one function shared with the observer.** Lift the
   observer's route computation into the library (host `habitat` or fauna
   `encounter`, your call) so the seeder and `voxel_edible_stock` call the
   same thing. For each lineage, per walkable component (that lineage's body,
   wade depth and `climb_m`):
   - **browser**: foliage stock in layers its mouth reaches from faces in the
     component, of vascular stands (all living by construction);
   - **shredder**: litter + carrion + glowcap cap tissue reachable from faces in
     the component, **and** at least one living producer of the renewable part
     rooted in the component (a litter-shedding stand; a glowcap for caps).
   A component is **habitable** for a lineage when that stock is at least
   `N · founders_placed_in_it · upkeep_per_hour`, upkeep read off
   `FounderPhysiology` (basal maintenance of the adult body; say exactly which
   number you use). **N = 1 founder-hour**, an authored placeholder: add it to
   `design/backlog.md` §1 in the house row format. Founders are placed only on
   faces in habitable components (browsers additionally on faces from item 3;
   shredders on the existing dry-soil-with-starter-litter rule). Keep the
   browser's "at least two stands in the component" rule.
   The world is **accepted** when both lineages place their full founder count
   in habitable components. Also **report, not gate**: per component, the
   producers' foliage (browser) / litter (shredder) production per hour at
   seeding against the lineage's upkeep per hour — package 6 needs it.
5. **Rejection re-draws the terrain seed**, as the lake gate does. The draw
   loop runs lake gate → pre-roll → seed → acceptance, and **the accepted world
   is the one returned, already seeded** — never seeded a second time. Bound
   the habitat tries separately from `LAKE_SEED_TRIES` (8 is fine); when none
   passes, keep the best (the highest worse-lineage stock/upkeep ratio) and say
   so loudly, like the lake gate. An **asked-for** seed (`--seed`) is honoured
   whatever its verdict, with a warning. A resumed world is never re-seeded.
   `voxel_census`, `voxel_founder_autopsy`, `voxel_edible_stock`,
   `voxel_plant_autopsy` go through the same path for `preset=<name>`.
6. Fauna/flora snapshot schemas: bump whatever the command or seeding change
   touches; fresh worlds only, never migrate.

## Tests (write these first, in their own commit, before the implementation)

Short function tests, ≤ 200 ticks, no pinned hashes. Configs may shorten the
shower to fit.

- A stand and a founder seeded on the **lower** face of a roofed shelf land on
  that face (`site.y` equals the checked y), not on the roof.
- A browser face is admitted when the crown top is out of the mouth band but a
  basal foliage layer with stock is in it (adult bloomcrown rosette), and
  refused when the only in-band layer has zero stock.
- Acceptance fails for a lineage whose component holds less than N
  founder-hours of reachable edible stock, passes at or above it; a shredder
  component with litter but no living producer rooted in it fails.
- Two food patches joined only by bare walkable ground are one component.
- The pre-roll: the seeded world has had its first shower fall and end before
  any stand exists; a face that flooded past a species' drown depth **during**
  the shower (and dried after) is not offered to that species.
- The draw loop: with a stub acceptance that fails the first seed, the second
  is returned already seeded; an asked seed that fails is returned with the
  warning.
- Ledgers (world water, flora, fauna) close through pre-roll and seeding.

## Measurements (after the implementation, in the census doc as "Startup
acceptance, 2026-09-22")

- Per preset (small, default, wide), seed base 1: tries drawn and why each
  was rejected; pre-roll ticks and desktop wall seconds (release, one process);
  per species stands and stands per m²; unmet niches (species with an eligible
  face count of zero after the pre-roll); per lineage the accepted components'
  stock/upkeep ratio and production/upkeep ratio; total imported material
  (logs, starter litter, founder bodies) in organic units.
- `voxel_edible_stock 0` per preset: `f_route_seeded` before and after.
- `voxel_founder_autopsy 60` per preset and `voxel_census 6 preset=default`:
  browsers and shredders alive per hour, deaths by cause, bites by food —
  against main before this package (build `before` from `git archive` of the
  merge base in a scratch tree, as the diets package did).
- If `small` rejects every seed, that is a finding for the terrain line
  (`voxel-terrain-note-small-water-2026-09-22.md`), not a threshold to lower.

## Constraints

Work only in your worktree; `CARGO_TARGET_DIR` inside it; all cores but one are
fine for runs. Explicit-path commits only (never `git add -A` / `commit -a`);
end each with the `Co-Authored-By` line you were given. Do not tune any rate,
density target or threshold: new numbers are placeholders in backlog §1. Do not
touch the trained policy files or their digest; do not retrain. Do not edit
`design/handoffs/README.md`, `design/README.md`, `config/tachyon/*`,
`scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or the untracked
`design/organism-anatomy-2026-09-21.md`. No new stands (reed, lanternberry,
vaulttree) — later package. No knob in the normal display. Run the full
workspace suite (release) before reporting.

## Decision authority

Yours: the command shape for item 2, where the shared acceptance function
lives, the drain criterion's cap, the habitat-try bound, report formats. Mine
(stop and report instead): anything that would change a decision in the
decisions doc, a diet, a body, a rate, or the weather schedule beyond the
opening shower.

## Return (≤ 40 lines)

Commits (hash, one line each); the choices you made under your authority and
why; the measurement tables' headline rows; anything that surprised you or that
contradicts this brief, with the evidence.
