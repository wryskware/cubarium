---
status: open
date: 2026-09-21
owner: Wrysk runs it as a fresh Astra thread; read-only audit, no code changes
---

# Organism systems audit: units, anatomy, diet, energetics, sensing and training, considered together

## Why an audit and not another package

Four diagnoses in two days each fixed the one variable the previous one
pointed at, and the frondgrazer lineage is still extinct within the hour on
every landscape that ships. The chain, in order, with its evidence:

| package | what it found | note |
| --- | --- | --- |
| D1 | the authored world was dry; plants had zero income | `design/7_Research/voxel-census-2026-09-20.md` |
| route B | closed water cycle; plants 5/6 then 6/6 at 6 h | `design/handoffs/voxel-water-cycle-2026-09-20.md` |
| D2 | blind heuristic froze; trained centres shipped as default | census note; `voxel-decomposers-and-defaults-2026-09-20.md` |
| D3 | browsers starve at the mouth-access boundary (crown must sit at exactly the head layer) | census note; `voxel-browser-autopsy-2026-09-20.md` |
| reach | mouth reaches one voxel up; retrained; live world then overshoots (60 deaths) | `voxel-browser-reach-2026-09-21.md` |
| reproduction | gestation escrow and egg clutches; no boom; still extinct at 35 min | `voxel-reproduction-2026-09-21.md` |
| D4 | the cone is mostly clear at death; the nearest crown is in range, in the ±20° band and unoccluded at 4 of 51 deaths; on the panel the eye is 0.19 m high and crowns sit at +36° | census note §D4; `voxel-cone-autopsy-2026-09-21.md` |

Wrysk's framing (2026-09-21): organisms must be designed in real units, not
voxel counts; an animal should have more than one food source and must not
starve because one plant grew too tall; the plant anatomy and the animal
anatomy may simply not be ecologically compatible; and there is solution
space nobody in these threads has raised. This audit is to consider all of
that at once, and to review the proposed new body designs against it.

## What to read first

1. `design/organism-anatomy-2026-09-21.md` — Wrysk's proposal (status:
   proposal): layered plant profiles by growth stage, animals at model size,
   a raised-neck browser with reach up 1, the six live species plus three new
   ones, and the five model packages it implies (§6).
2. `design/art-direction/organism-scale-and-roster-2026-09-21.md` §4 — the
   size ladder (voxels of 0.125 m; world height 72; browser body 0.25 → 0.375
   m; shredder 0.125 → 0.19 m) and the levers A/B/C.
3. `design/art-direction/species-dossiers-2026-09-21.md` — the picture each
   organism is meant to make.
4. The evidence table above, in order; D3 and D4 in full.
5. `design/0_Canon/README.md` and the relevant `DECISIONS.md` entries; the
   anatomy document and the ladder are proposals, not canon. Preserve modality.
6. `design/backlog.md` — every placeholder number that is waiting for a GUI
   knob; the audit must not propose tuning them.

## What the model holds today, in the places the audit will check

- **Plants** (`crates/cubarium-voxel-flora/src/lib.rs`, `step.rs`): a stand
  is a trunk column plus one crown disc one cell thick at
  `crown_voxels(wood)`; `crown_height_voxels` / `crown_radius_voxels` per
  species are authored in voxels ([1,3], [1,3], [2,5], [0.5,1], [0.5,0.5],
  [0.5,1]); `FloraConfig::for_voxel_size` rescales them and the root box to
  metres when the cell is smaller than the 0.25 m reference. Foliage is one
  number per stand with no position. Glowcap is a saprotroph on dead wood and
  litter. Trophic roles: `Trophic`.
- **Animals** (`crates/cubarium-voxel-fauna/src/{manifest.rs,body.rs}`): the
  manifest mixes units. In metres: `body_length_m` 0.25 / 0.125,
  `body_width_m`, `cone_range_m` 2.0, `mouth_reach_body_lengths` 0.25,
  `cruise_m_per_s`. In voxels: `mouth_reach_up_voxels` (1 / 0), the mouth
  probe columns, the cone origin at `standing_y + 1.5` voxels, the footprint
  and standing rule, the contact receptors. Nothing rescales the animal per
  cell size, so the browser is a different animal on the 0.125 m panel than
  on the 0.25 m default. The presenter draws the body at twice the model
  length (`crates/cubarium/src/voxel/animal.rs`).
- **Energetics** (`body.rs`, `FounderPhysiology`): browser body_max 0.05,
  birth_body 0.03, birth_cost 0.01; shredder 0.0125 / 0.0125 / 0.00625.
  Newborn budget from the constants: 1,000 s at rest, 500 s at cruise
  (125 m for a browser, 62.5 m for a shredder). Motor cost is the
  physics-heuristic disc model. Assimilation and maintenance are per unit
  body per second.
- **Sensing** (`senses.rs`): browser `Cone(3)` of nine rays, yaw sectors,
  pitch −20/0/+20°, first hit classed Foliage / Occluder / Body (terrain,
  water, stripped crown and ground pools all read Occluder; the fine census
  in `browser_cone_census` is diagnostic only); `Contact(4)` on solids at
  body height only; `Chem(litter)` field for the shredder; taste, wet, light.
  Observation vectors: shredder 23, browser 37.
- **Diet**: the browser bites foliage from any stand whose crown layer lies in
  `standing_y+1 ..= standing_y+1+reach_up` within the mouth probe; the
  shredder eats litter. Neither has a second food. Carrion exists on drown
  and removal and nothing eats it.
- **Reproduction** (`step.rs`, `SpeciesConfig.reproduction`): browser
  gestation escrow (floor 0.005, hold 120 s, gestation 180 s, interval 300 s);
  shredder egg clutches (clutch 1 forced by the reserve ceiling, incubation
  300 s). All placeholders, listed in the backlog.
- **Seeding** (`crates/cubarium/src/voxel/habitat.rs`): stands by area
  fraction with per-species min/max, founder sizes 0.30–0.85 of cap, 8 + 8
  animal founders; the seeder does not consult the establishment predicate.
- **Training** (`cubarium-search` `es/voxel`, `voxel-imitate`): Stage A/B
  arenas on a flat 0.125 m sheet with one crown at the head layer; Stage B
  successor at ≥ 2 m; imitation seed from the hand heuristic; the shipped
  centres are `crates/cubarium/assets/policies/*.json`.
- **Landscapes** (`crates/cubarium-voxel/src/recipe.rs`): presets `small`
  (panel, 0.125 m, 160×48×24, height 72 in the newest merge), `default`
  (0.25 m, 128×48×24), `wide`; each with its own water inventory, scheduled
  showers, terraces, pools and a river. The diagnostics take `preset=<name>`.

## The questions, to be answered together

1. **Units.** Which organism quantities are in voxels today and which in
   metres; a single rule for which belong where; what "the same animal on any
   cell size" requires (eye height, mouth reach, footprint, contact, arena
   layouts, the observation vector's own scaling). Does the anatomy proposal
   ("the manifest body is the animal", ladder in voxels of 0.125 m) satisfy
   that rule or restate the mixture in a different place?
2. **Trophic compatibility.** A matrix: each consumer's mouth (today, and as
   proposed in anatomy §4) against each plant species at each growth stage
   (today's lollipop, and the §3 profiles) on each shipped preset: reachable
   foliage share, at what stand size it escapes, what fraction of a seeded
   and of a six-hour world is edible. Does every lineage keep a diet through
   succession? Which lineages have one food, and what a second food would be
   for each (turf for the browser; litter is the shredder's; carrion has no
   consumer; fruit and fallen foliage do not exist).
3. **Energetics in real units.** Take the constants as they are and state
   them as rates for a 0.25 m and a 0.375 m animal: seconds to starvation at
   rest and at cruise, metres of search those buy, against the measured
   inter-crown distances (D4: 0.8 m on small, 1.7 m on default) and the
   edible fraction from question 2. Is starvation inside the hour a search
   failure or an arithmetic certainty? Where allometry would say the numbers
   are incoherent, say so with the reference; do not propose values.
4. **Sensing as anatomy.** Whether the browser's cone geometry (eye height,
   ±20° band, 2 m, nine rays, first-hit classes) can find the food that
   question 2 says exists, on terrain with relief; what a real analogue of
   this animal senses (foliage odour, contact with stems, looking up); whether
   the observation vector should be defined so a policy transfers across cell
   sizes and landscapes. Same for the shredder's litter field.
5. **Plant anatomy versus animal anatomy.** Review the anatomy proposal's
   layered profiles and staged shapes: do they make the grazed meadow and
   succession possible, what do they cost the light and mass-balance rules,
   and where do they interact with questions 1–4 in ways the document did not
   consider (bottom-up regrowth against bites, porosity in the cone, the
   basal rosette as the only browser food, escape sizes, the raised neck).
6. **Life history and size.** Gestation, egg, birth sizes and intervals
   against body size and food density; whether the browser's and shredder's
   rules and the ladder's sizes are one animal.
7. **Density and seeding.** Stands per square metre and founder sizes
   against each lineage's home range and reach; whether seeding should place
   by the establishment predicate; whether the seeder or the world should
   guarantee an edible floor at t = 0.
8. **The training world.** Why an arena that shares no geometry with the
   shipped landscapes was expected to transfer; what an arena that is a slice
   of the shipped world needs (relief, pools, stripped crowns, bodies, crowns
   at every height, scheduled showers); what the imitation seed should copy;
   the order of retraining relative to the anatomy packages.
9. **The solution space nobody raised.** Name at least five angles that
   none of the documents above consider, with the evidence that makes each
   plausible. Examples of the kind, not a list to confirm: plant growth that
   responds to browsing; animals whose diet changes with body size; fallen
   foliage and fruit as pools; browsers that climb or rear; grazing that
   keeps turf as the floor food; herd or trail behaviour; plants defended
   above a height; carrion and egg predation; the panel's own scale forcing
   a smaller browser rather than a taller one.
10. **The order.** One dependency-ordered sequence of packages with explicit
    interfaces, what to measure after each (the census and autopsy arms exist
    and take ~2.5 min per 60 simulated minutes), and which choices are
    Wrysk's. Say which of the anatomy document's five packages survive the
    audit unchanged, which change, and what precedes them.

## Constraints on the auditor

- Read-only. No code, no tuning proposals for backlog placeholders, no
  design changes: proposals and their evidence only.
- Canon binds; proposals do not. Do not turn a leaning into a requirement.
- Real units in every statement about an organism (metres, seconds, kg or
  organic units as the model has them, m/s); voxel counts only when saying
  what the code does today.
- Cite the file and line, or the research note and table, for every claim
  about the model or the measurements. Where a number must be measured to
  answer, the diagnostics may be run: `cargo run --release -p cubarium
  --example voxel_founder_autopsy -- 60 preset=small` (and `preset=default`,
  `generated closed`), `voxel_census` likewise, `voxel_plant_autopsy`; `graft
  ask "<question>" --source` before opening files. All cores are available.
- Fresh worlds only: any schema change is a new world, never a migration.
- Do not touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`.

## Deliverable

`design/7_Research/organism-systems-audit-2026-09-21.md` with front matter
`design_status: exploration`, sections matching questions 1–10, the trophic
matrix as a table, and at the end a single **Decisions for Wrysk** list, each
a choice with its consequences stated, not a recommendation dressed as a
fact. Evidence and reasoning throughout: the reader will check the citations.
