---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, high effort
---

# Package 2: plants have layers; foliage has a height

## Why

With bodies in metres (`voxel-body-anchors-2026-09-22.md`) the low browser
cannot eat a one-cell crown above 0.25 m, and that is where bloomcrown's
foliage is on default and most of the canopy on wide: reachable foliage fell
to 0.42–0.50 of the stock and browsers on default stopped biting bloomcrown
at all. A plant is a lollipop today: one crown disc, one cell thick, holding
all its foliage. Decisions §4 and §5
(`voxel-organism-decisions-2026-09-21.md`) give plants layers with explicit
per-layer tissue, a renewable basal rosette on adult bloomcrown, ground
rosettes for woody seedlings, and canopy escape at the juvenile transition.
The audit's §5 (`design/7_Research/organism-systems-audit-2026-09-21.md`)
lists the five interactions a layer model must settle: persistent lower
depletion, conservation across transitions, light receivers, porosity's
sight meaning, and grazing not automatically making a meadow. Wrysk's
anatomy proposal (`design/organism-anatomy-2026-09-21.md` §2, §3) is the
vocabulary and the authored profiles; this package implements it for the
six live species and none of the new ones.

## The model

**Profile.** A species carries `profile: [(wood_fraction_max, Profile)]`,
staged by `wood / wood_max`, first entry whose threshold ≥ the fraction
applies. A `Profile` is an ordered list of `Layer { kind: Trunk | Foliage |
Drape | Mat, band: [from, to] as fractions of the stand's physical crown
height, radius: fraction of the physical crown radius, share: fraction of
the stand's foliage *capacity* (foliage layers sum to 1), porosity }`, plus an
optional `height_m_max` for the stage (decisions §5: woody seedlings are
ground rosettes no taller than 0.125 m, overriding the interpolated height
while the seedling stage applies). Physical crown height and radius come from
the existing `crown_height` / `crown_radius` on the reference cell scaled by
`for_voxel_size`, in metres, as today. Profiles for the six species are the
anatomy document's §3 tables, with the decisions' corrections: adult
bloomcrown basal rosette share 0.25 for life; seedling stages capped at
0.125 m; velvetpad and stonecushion stay browser food (§3) so no diet flag.

**Stock.** Each stand holds `layer_stock: Vec<f64>` in organic, one per
foliage-bearing layer of its current profile, summing to `foliage` at all
times; `foliage` stays the scalar the ledgers, water and mineral rules read.
Capacity per layer = share × the stand's foliage capacity (find the growth
model's own cap; do not invent one).

**Bites.** `take_foliage` takes from the layers whose band intersects the
requester's mouth band (the fauna side passes the band in metres; the
encounter helpers already compute intersection), lowest first, and reports
what it took per layer so the fauna ledger books exactly what left the
stand. A bite from below leaves the upper stock untouched: after taking 0.10
from a 0.25 / 0.75 stand, it is 0.15 / 0.75, never 0.225 / 0.675.

**Regrowth** fills layers bottom-up in profile order to each layer's
capacity, paying construction and mineral as today. Senescence and dieback
lose from the top foliage layer first. **Stage transitions** re-bin the
existing total into the new profile bottom-up; they never create tissue.
**Death** deposits the sum, as today.

**Light.** A recipient stand's income is assessed per foliage layer at that
layer's physical height, weighted by the layer's share of the stand's stock;
an occluding layer is any Foliage/Drape/Mat layer of another stand whose
band lies above that height and whose footprint contains the site, with
attenuation `exp(−k·(1−p)·stock/area)` over the layer's physical area in m²
(package 1b's units). No self-shading. For a single-layer species the
reference-grid result must equal today's digit for digit.

**Sight.** The cone marks every cell of a foliage-bearing layer with stock
> 0 as `FoliageCrown`, stock 0 as `StrippedCrown`, Trunk cells as
`Occluder`. Porosity is a light property only; a porous canopy is still
leaves to an eye. Record this as a known simplification in the contract.

**Presenter.** `stand::parts_of` draws the trunk cells and each layer's
disc; interim glyphs, no art decisions.

**Encounter.** `FloraView::layers(stand)` yields `(band_m, radius_m, kind,
stock, porosity)`; the observer reports reachable stock per layer, which
closes package 0's "could not measure 1".

## Deliverables

1. Tests, ≤200 ticks, authored from this brief before implementing:
   conservation across a bite, a regrowth interval, a stage transition and a
   death; the 0.25/0.75 bite example; bottom-up regrowth; adult bloomcrown's
   reachable share under the browser band is its rosette's stock; a seedling
   bloomcrown is wholly reachable; a juvenile umbrellafrond has no reachable
   layer; single-layer species shade identically to before on the reference
   grid; a cone ray sees a rosette layer cell as foliage; the observer's
   per-layer sum equals the stand total.
2. The model above in `cubarium-voxel-flora` (`lib.rs`, `step.rs`), the
   fauna bite path, the encounter helpers, the observer, the presenter.
   Flora snapshot schema bump; fresh worlds only; refuse old ones.
3. Profiles for the six species as authored placeholders, listed in
   `design/backlog.md` §1 as a block, and the anatomy document's §3 tables
   annotated with what was implemented (a short "implemented as" line per
   species; do not rewrite Wrysk's document).
4. Measurement, after: `voxel_edible_stock 0 preset=small|default|wide`
   (reachable / route fractions before → after, and per layer), then
   `voxel_edible_stock 6 preset=small` and `preset=default`, and
   `voxel_founder_autopsy 60 preset=small` and `preset=default`. Append to
   the census note as "Layers, 2026-09-22", with the sentence the numbers
   support on each preset.

## Constraints

- No knob tuning beyond the authored profiles; no golden hashes; fresh
  worlds; tests ≤200 ticks; the six live species only.
- The trained-policy digest and the shipped policy JSON stay untouched.
- Worktree `.claude/worktrees/plant-layers`, branch `plant-layers` from main;
  `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every commit ends
  with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`.
- `graft ask "<question>" --source` before opening files; `graft callers
  take_foliage --depth all`, `graft callers reachable_foliage --depth all`
  and `graft callers light_per_stand --depth all` before changing them. No
  windows.

## Verification

`cargo nextest run --workspace --exclude cubarium-gpu` green; the tests in
deliverable 1; both ledgers' residuals ≤ 1e-9 relative over the 6 h runs;
Fable re-runs one arm.

## Return (≤40 lines)

Before/after reach and route table with the per-layer split, the two
autopsy arms and the two 6 h observer runs, how stage transitions re-bin,
what the reference-grid identity test covers, the commit list, and anything
the model could not express.

## Integration note (Fable, 2026-09-22)

Landed c78e5a6..cfcb347 (flora layers with per-layer stocks and a new flora
snapshot at schema 1; mouth, cone and presenter read layers; observer per
layer; a same-build `lollipop` control arm; census section "Layers,
2026-09-22"). Workspace suite green (2149). Fable re-ran `voxel_founder_autopsy
60 preset=default` and the t = 0 observer and reproduced exactly: starved 34
(23 shredder, 11 browser), drowned 2, umbrellafrond 7,417 bites, bloomcrown
0, 2 browsers and 13 shredders alive at 60; reach 0.5611, route 0.2234.

| | small | default | wide |
| --- | --- | --- | --- |
| reachable foliage at t = 0, lollipop → layers | 0.42 → 0.59 | 0.50 → 0.56 | 0.49 → 0.60 |
| route-connected | 0.42 → 0.59 | 0.22 → 0.22 | 0.37 → 0.39 |
| browser, 60-min arm | extinct 30 → 53 min; bloomcrown bites 640 → 3,380 | 1 → 2 alive; frond seedlings 2,269 → 7,417 bites | – |
| browser, 6 h observer | extinct either way (plant collapse predates layers) | extinct min 90 → 210; reachable share 0.07 → 0.41 | – |

All the gain is the new low tissue: the adult bloomcrown rosette is 100 %
reachable on every preset. Two things the numbers now make measurable and
which the next packages own: default's route is flat because no seeded
browser's component holds a bloomcrown (seeding, package 4), and browsers on
default now eat umbrellafrond seedlings hard, the audit's "universal seedling
browsing prevents canopy recruitment" risk (coupled assessment, package 6;
not a tuning matter). Small's six-hour plant collapse predates this package
and is a separate line (the small preset's water or seeding, not anatomy).

Worker's calls, kept: the reference-grid identity is asserted with p = 0 and
as `historical^(1−p)` with the authored porosity, so the three ground mats
shade slightly less than before (their p is a backlog placeholder); porosity
has no sight meaning (contract §8, a model decision left open); `layer_stock`
is a fixed `[f64; 4]` because `Stand` is `Copy`; the flora snapshot exists
but nothing writes it yet (the host still reseeds plants on restart, so
fresh-worlds holds trivially; host wiring owed). Not benchmarked: tick cost
against the pre-layer build (light is per layer with an envelope prefilter).

The worker had copied Wrysk's untracked anatomy document into the branch to
annotate it; Fable removed that copy so the untracked file stays Wrysk's,
and the annotations live here instead:

## Implemented as, per species of the anatomy document's §3

- **Springturf**: one `Mat 0–1.0, r 1.0, share 1.0, p 0.3`, exactly as
  written. Reachable everywhere on all three presets; the mat's disc cell is
  `crown_voxels(wood)`, so its picture and its reach are unchanged.
- **Velvetpad**: one `Mat 0–1.0, r 1.0, share 1.0, p 0.6`, as written.
  Decisions §3 keeps it on the browser's diet: no diet flag, no exclusion.
- **Stonecushion**: the two bands as written. At the shipped
  `crown_height_voxels [0.5, 0.5]` both round into the first cell over the
  face, so the dome is one cell tall in the picture and both layers are floor
  food; two layers in the stock, which is what a grazer thins first.
- **Bloomcrown**: the three stages as written, with decisions §5's
  corrections: the adult basal rosette holds 0.25 for life and the seedling's
  physical crown height is capped at 0.125 m. On the shipped
  `crown_height_voxels [1, 3]` (the ladder's 3–8 v is a later package) the
  adult's rosette sits in the first cell over the face and its crown three
  cells up: at t = 0 a browser reaches 100 % of bloomcrown on small (0 %
  before), 66 % on default (53 %), 54 % on wide (33 %).
- **Umbrellafrond**: the three stages as written, seedling capped at
  0.125 m. The default "no" kept: no hanging Drape, so adult and juvenile are
  out of a 0.25 m mouth's reach and only a seedling is food. On default the
  three tiers are 0 % reachable; on wide 8.7 %, all seedlings.
- **Vaulttree, lanternberry, siphonreed**: not implemented (six live species
  only). The profile machinery carries `Drape` and a porous `Trunk`, so the
  vaulttree needs no new layer rule when it arrives, only its dead-wood line
  and establishment gate; siphonreed's `water_depth_min` site rule is a
  model addition not made here.
- **Glowcap**: one `Foliage 0–1.0, r 1.0, share 1.0, p 0.5`, as written. The
  interim glyph is unchanged and still drawn as its own special case.
