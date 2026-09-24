# Package N: vaulttree, lanternberry and siphonreed

Wrysk, 2026-09-23: add the three planned stand species. **After package L** (crowns in
metres, the growth fix, animals at the ladder): this package uses L's fields and rules.
Lands on main.

Read first: `design/organism-anatomy-2026-09-21.md` §3 (their layer tables) and §5,
`design/art-direction/species-dossiers-2026-09-21.md` D11–D13 (body plans and the model
notes at the end of each), `design/theoretical-biosphere-2026-09-16.md` §4 (their roles),
`design/organism-large-animals-2026-09-23.md` §2 (growth fix),
`design/handoffs/voxel-ladder-growth-2026-09-23.md`,
`design/handoffs/voxel-terrain-note-standing-depth-2026-09-22.md` (water_depth_m counts
water in transit).

Recommended worker: Opus, medium effort, escalating on evidence. Every rate is a
**placeholder** and goes in `design/backlog.md` §1; no tuning passes.

## Species

| | Vaulttree | Lanternberry | Siphonreed |
| --- | --- | --- | --- |
| Role | canopy builder of deep aerated soil; establishes only in a gap | fruiting shrub of grove edges and moist bright gaps | emergent of saturated soil and shallow standing water |
| Height m | 2.25 → 3.5 (grows from the seedling, L §2) | 0.625 → 1.125 (from the seedling) | 0.75 → 1.5 |
| Radius m | 0.625 → 1.0 | 0.25 → 0.4375 | 0.0625 → 0.125 |
| Profile | anatomy §3: seedling ≤ 0.1 (capped rosette) · juvenile ≤ 0.4 · adult: trunk, limbs (Trunk p 0.85), lobes (p 0.45), drape (p 0.9) | seedling ≤ 0.2 (capped) · adult: stem fan (Trunk r 0.4, p 0.7) + foliage 0.3–1.0 p 0.5 | one stage: porous column, p 0.7 |
| Life | slow, long-lived, large reserve, wood up to about 5 (bloomcrown is 0.6) | repeated paid fruit (the parcel, for now) after reserve recovers | local paid spread; aeration tolerance at an allocation cost |
| Light | establishment gate ≥ 0.9 | light need between frond and bloomcrown | high |
| Water | deep aerated root zone, stresses on long saturation | moist | needs standing water on or beside the site |

## Model additions

1. **The fall (vaulttree).** At death, `wood` goes to dead wood along a line of sites:
   `round(crown_radius / voxel)` sites in a direction hashed from the site, each site an
   equal share. A site without support gives its share to the nearest supported one on
   the line, or the origin. The log the glowcap colonises is as long as the crown was
   wide. Conservation: the line's total equals the wood at death.
2. **Standing-water gate (siphonreed).** `water_depth_min_m` on the species: it
   establishes and stays healthy only where **settled standing water** on its site or a
   4-neighbour is at least that deep. Not `water_depth_m` as it is, which counts water in
   transit (the note above). Use the settled measure if the water line has one, or add a
   small reader over settled faces; report which. Its drown depth is large: it stands in
   water.
3. **Aeration at a cost (siphonreed).** Saturation-tolerant (`establish_saturated_max`
   1.0, like umbrellafrond) but paying for it with higher maintenance. Umbrellafrond's
   doc comment explains why the tolerance must not be "slightly below 1".
4. **Gap establishment (vaulttree): no new rule.** Germination light is geometric sky
   visibility with no living canopy in it (`step.rs` `Gates`), so "needs a gap" means
   "needs open sky" in this package. Report whether that already keeps vault seedlings
   out from under crowns. Making it canopy-aware is a separate rule (Astra R6.2) and
   Wrysk's call.

## Ecology notes (expected, measure, don't force)

- The browser eats all vascular foliage in its band (decision 3), now [0, 0.5 m].
  - Vault seedlings and juveniles under about 0.8 m are food.
  - **Lanternberry never escapes fully:** its foliage starts at 0.3 of its height, below
    0.5 m at every size. So an adult keeps a **browse line**: cropped below 0.5 m, full
    above. That is a real shrub pattern and is accepted; report it.
  - Siphonreed standing in water deeper than the browser wades is out of reach; on a
    saturated bank it is food.
- Nothing eats adult vault lobes until the loftstrider (a later package).
- The seeder places them where their gates pass. Startup acceptance counts them as
  producers.

## Measure

8 seed bases × 3 presets. Per species: seeded count, establishment, alive at 60 min and at
6 h, reproduction events, and each vault death's dead-wood line length. Plus glowcap
colonies on vault lines, and bites per species. Report any preset where a species can
never establish (no deep soil, no standing water); that is a terrain fact for the
landscape thread, not a tuning target.

## Tests (authored first, separate pass)

The fall conserves wood and lays `round(r/voxel)` sites; unsupported shares fall back; a
siphonreed establishes beside settled standing water and not beside transit water only;
seedling and adult profiles match anatomy §3; the lanternberry browse line (foliage below
0.5 m reachable, above not).

## Return (≤ 40 lines)

Commits, tests, the measurement table, which placeholders went to the backlog, and every
place the model could not express the anatomy document.
