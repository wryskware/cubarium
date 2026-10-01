---
design_status: exploration
last_reviewed: 2026-09-28
decision_refs: []
---

# A field guide to the Cubarium voxel ecology

Every living thing in the voxel world as the code builds it at `3ccc6daf`, plus the ones
designed but not built. This describes the implementation. It is not canon and decides
nothing.

- Numbers come from `design/7_Research/ecology-parameter-audit-2026-09-28/parameters.tsv`,
  which gives the source line of each.
- Names and roles come from the species dossiers
  (`design/art-direction/species-dossiers-2026-09-21.md`,
  `species-dossier-D15-latticevine-2026-09-23.md`).
- The audit (`design/7_Research/ecology-parameter-audit-2026-09-28.md`) says which numbers
  have a real-world basis. The short "Reality check" under each entry is its verdict for that
  species.

## How to read the numbers

- **Organic matter** is counted in abstract *org-units*. Plants and animals share the unit
  but not its scale: in plant terms a unit is ~0.04–0.25 kg of dry foliage, in animal terms
  ~200–600 kg of body (audit §3).
- Every transfer carries three currencies: organic matter, mineral and energy. All three
  are conserved exactly.
- **Time** is simulated seconds at 20 ticks/s. Biology runs ~3,000× faster than nature (an
  animal burns its body's worth in ~17 minutes). The day runs 12× (2 hours), and walking
  runs in real time.
- **Space** is metres. The live worlds use 0.125 m voxels. Seed hops and root depths are
  authored in 0.25 m "reference voxels" and doubled on the finer grid, so the metres stay
  the same.
- **"One browser's need"** is 1.25e-4 org-units of foliage per second: what an adult
  frondgrazer must eat to stand still.

---

## The stage

**Ground.**
- The desktop terrarium is 32 × 16 × 6 m (256 × 128 × 48 voxels, a 192 m² floor), a ring of
  tree and tower. The panel runs a 20 × 9 × 3 m version.
- Voxels are air, soil, rock or bedrock. Soil holds 35 % pore space and drains to field
  capacity at 65 % of it; plants can draw it down to the wilting point at 28 %. That leaves
  ~13 % of the soil's volume for roots, a loam-like figure.
- Water moves through soil very fast (0.0175 m/s), so rain soaks in where it falls and never
  runs off soil.

**Water.**
- The water budget is closed. Nothing enters or leaves; water cycles between the ground, a
  lake, a spring-fed stream and one lumped atmosphere store.
- The atmosphere holds about one downpour (~34 mm over the floor). It refills from the lake's
  surface and from plant transpiration, so plants make their own rain.
- The water table sits at the lake's level. Ground near the lake is wet from below;
  terraces are dry apart from rain.

**Weather** (climate preset `tropical_jungle`).
- Once a simulated minute, the weather may start a drizzle (~13 mm/h), a shower (~126 mm/h
  for ~5 min, ~10 mm) or a downpour (~250 mm/h, ~21 mm). The chance rises with humidity and
  in the afternoon.
- A dry gap of 5–15 minutes follows each event.
- Temperature is 26 °C ± 3 °C through the day, plus drifting fronts. No living thing reads
  temperature or humidity yet.

**Day.**
- A day lasts 2 hours: 80 minutes of sun and a 40-minute night.
- Animals keep a rhythm: the frondgrazer by day, the littershredder by night. Asleep they
  burn half as much, and hunger wakes them.
- Plants ignore the night on main; they photosynthesise around the clock.

**Light.** A plant's light is the sky it can see (a 17-ray fan) dimmed by the foliage of taller
stands above it. Equal-height neighbours never shade each other.

**Dead matter** lies in per-site pools:
- **litter** — fallen leaves, dung and dead seeds; decays 0.1 %/s.
- **dead wood** — dieback and fallen trees; 0.01 %/s.
- **carrion** — animal bodies; 0.5 %/s.

Decay and every death return the mineral to the soil pool **of the same site**, where the
next plant draws it.

---

## The food web

```
                             sunlight
                                │
     ┌──────────────────────────┼──────────────────────────────┐
     ▼                          ▼                              ▼
  GROUND COVERS            FORBS AND SHRUBS                 TALL PLANTS
  springturf               bloomcrown (basal rosette)       umbrellafrond, vaulttree:
  velvetpad                lanternberry (low skirt)         out of reach once grown
  stonecushion             and every young plant            siphonreed: never in reach
     │                          │                           latticevine (rock faces)
     └───── foliage in the 0–0.5 m band ─────┐                        │
                                             ▼                        │ nectar, fruit
                                        FRONDGRAZER                   ▼
                                             │               (bellwing, seedporter:
           leaf fall, dieback, dead seeds    │ dung, body     planned)
     ┌──────────────────────────────────────►▼
     │                               LITTER · CARRION ◄───────── dung, bodies
     │                                  │        │                    ▲
     │                                  ▼        │                    │
     │   DEAD WOOD ──► GLOWCAP ──caps──► LITTERSHREDDER ───────────────┘
     │   (logs fall      (also eats
     │    from trees)     litter)
     │
     └── decay returns mineral to the soil of the same site ──► plants
```

- Nothing yet eats the two animals; predators are planned.
- The glowcap and the littershredder compete for litter.
- Adult umbrellafronds, adult vaulttrees and siphonreeds have no consumer at all.

---

## Producers

Every plant runs the same economy.
- Foliage fixes organic matter from light, scaled by moisture, aeration and a mineral
  (Monod) term. The income pays wood upkeep first, then saves a fifth to reserve, then grows
  foliage and wood, each unit costing 1.2.
- Foliage regrows **bottom-up**, so a grazed rosette refills first, and sheds from the
  **top**.
- A stand dies when its wood falls below `alive_min`.
- Past `donor_min` it saves reproductive packages, which go out as seeds into a dormant bank,
  or as runners. The bank is checked every 30 s. A seed germinates only where the site passes
  the species' light, water, aeration, flooding and substrate gates, and wins a lottery
  weighted by seed count × seed mass.
- Every plant carries foliage twice its wood. Foliage lives ~17 minutes (1,000 s) unless
  stated otherwise.

### Springturf — *Caespes primus*
**The pioneer turf of open, moist, drained soil.**
- A low mat, 0.13–0.19 m tall and up to 0.25 m across, with no stem. The frondgrazer's staple
  and the thing that visibly gets cropped.
- Needs open sky: germinates only under 75 % of the sky and earns badly in shade. Roots a
  quarter-metre deep. Dies in 3 cm of standing water.
- Fast and short-lived. It regrows all its leaves in 100 s and burns out 250 s after its
  income stops. It sets seed 161 s after sprouting and is full size at 230 s, sending a seed
  every 9 s when funded.
- Seeds drop within 0.75 m into a 12-hour bank; 30 % of packages go out as runners.
- A ground cover, so woody seedlings can come up through it.
- Grazing: entirely inside a browser's reach, with 30 % kept as an ungrazeable base. Edible
  0.084, and it sustains 2.5 browsers' need.
- *Reality check:* its growth rate matches a fast herb on the biology clock. Its maturity is
  11–32× too fast, and one turf holds ~23 real days of a browser's food.

### Velvetpad — *Stratum umbrae*
**The shelter carpet of damp shade.**
- A flat pad 0.125 m tall and up to 0.75 m across, living under ledges and crowns. Drawn as a
  ground-surface tile.
- The most shade-tolerant plant (light half-saturation 0.1; germinates with 5 % sky). Wants
  damp but not waterlogged soil (wilts below a quarter of drained soil's water).
- Spreads by spores (1.5 m; 2 tries to find damp ground) and runners (30 %). Short 3-hour bank.
- A ground cover. Grazing: all in reach, 30 % refuge, edible 0.28, sustaining 4.9 browsers.
  A velvetpad carpet holds ~57 hours of one browser's food per square metre.
- *Reality check:* plausible shade physiology. Its density (up to 64 stands/m²) and
  productivity are ~10³× generous for its grazer.

### Stonecushion — *Pulvinus saxi*
**The cushion of bare rock.**
- A dome 0.13–0.19 m tall on a rock or bedrock face, rooted into a soil pocket beside or
  under it.
- Asks almost nothing of water (full moisture at a fifth of drained soil's water). Dies under
  2 cm of standing water.
- Slow: leaves live ~56 minutes, it regrows in ~67 minutes, and takes ~2.2 hours to set seed
  and ~3.2 hours to reach full size.
- Wind-dispersed (median 0.26 m) into a 12-hour bank.
- A ground cover with a 40 % refuge. It is the only plant a browser can out-eat: it sustains
  a fifth of one browser.
- *Reality check:* the slow, stress-tolerant role reads right. Wind dispersal is 40–400×
  shorter than real.

### Bloomcrown — *Corona solis*
**The sun forb-shrub of ridges and terraces.**
- A stem and a vaned crown up to 1 m tall. The adult keeps a basal rosette holding a quarter
  of its foliage for life: the floor food that lets a low browser eat a mature bloomcrown.
- Needs sun (light half-saturation 0.8; germinates only under 60 % of the sky). The most
  drought-hardy upland plant. Will not germinate where a quarter of its roots are
  waterlogged, and dies in a 5 cm pool.
- Sets seed ~34 minutes after sprouting and reaches full size in ~45 minutes. Seeds are
  twice the minimum package, dropped within 0.5 m into an 8-hour bank. They come up through
  ground covers at 0.1 per check.
- Grazing: young plants are entirely in reach. The adult's rosette offers 0.255, sustaining
  4.9 browsers, and its upper crown (0.55–1 m) is out of reach. Later nectar and pollen for
  the bellwing (planned).
- *Reality check:* a 1 m shrub holding 1.2 units of foliage (24 browser bodies) is the
  audit's headline unit mismatch.

### Umbrellafrond — *Pluvia tecta*
**The wetland frond of hollows.**
- A thick stem under two or three umbrella tiers, 1–2 m tall and up to 1.5 m across. It
  shades what stands under it, and velvetpad lives there.
- Shade-tolerant (0.15), deep-rooted (1 m). Needs ground wetter than drained soil: it wilts
  at field capacity. Immune to waterlogging (the stand-in for a wetland producer) and survives
  0.5 m of water.
- Spores to wet ground within 1.5 m, and rhizomes for half of its packages. 4-hour bank.
- Grazing: seedlings and young juveniles only; the adult's lowest tier escapes the band.
  Nothing eats a grown frond.
- *Reality check:* plausible wetland traits. Like every species, it carries foliage at twice
  its wood.

### Lanternberry — *Lucifructus*
**The fruiting shrub of grove edges and bright moist gaps.**
- A fan of 3–5 stems, 0.6–1.1 m tall, hung with lantern-shaped fruit. Until fruit exists in
  the model, a ripe parcel is drawn as hanging lanterns.
- Moderately shade-tolerant (0.4). Keeps 70 % of its reserve when reproducing. Seeds are 3×
  the minimum package, dropped within 0.5 m; 8-hour bank.
- Grazing: a low skirt (0.3–0.45 of its height, 3/14 of its foliage) stays in a browser's
  band for life. Edible 0.364, sustaining 8.2 browsers, the most of any plant. Fruit is meant
  for the seedporter (planned).
- *Reality check:* 20–100× too much foliage per unit wood for a shrub.

### Vaulttree — *Arcus caeli*
**The canopy builder.**
- A 2.25–3.5 m tree with a crown up to 2 m across, standing in deep aerated soil (roots 1.5 m
  deep). Establishes only in an open gap (90 % sky).
- Slow and long-lived: seeds after ~3.2 hours, full size after ~3.8 hours. Large reserve (1.5×
  wood). Seeds are 4× the minimum, carried by wind (median ~1 m); 6-hour bank.
- At death its wood falls as a **line of dead wood** as long as its crown was wide. That is
  the log the glowcap colonises, and the gap the next tree needs.
- Grazing: only seedlings and saplings are in reach.
- *Reality check:* at full size its income is likely capped below its upkeep by the
  per-stand mineral limit (audit §5), so it may never hold full size. Its crown passes 87 % of
  the light, where a real one passes 5–22 %.

### Siphonreed — *Fistula stagni*
**The emergent of the shallows.**
- Narrow clumps of hollow stems 0.75–1.5 m tall. Needs **settled standing water** of at least
  0.1 m on or beside its site, invests in aeration, and survives 0.5 m of water.
- Seeds travel by water to shore margins within 1.5 m; 70 % of packages go as rhizomes.
  10-hour bank.
- Grazing: never in a ground browser's band at any size, so nothing eats it.
- *Reality check:* plausible habitat traits. Its transpiration per crown area is ~200× real
  (it transpires per unit foliage).

### Latticevine — *Reticulum rupis*
**The rock-face cover.**
- An ivy-like cover rooted in a soil pocket at the foot of a cliff or on a ledge. It spreads
  face by face up walls, and down from overhangs, without a size cap, and roots sister plants
  where its runners reach soil.
- Each face runs its own small economy: income from side light, upkeep, leaves, drought
  dormancy after 10 dry minutes, and ~30 minutes per new face.
- Has flowering and fruiting phases. Nectar and fruit per face are meant for the bellwing and
  seedporter; its leaves are not browser food.
- *Reality check:* consistent leaf economics. Its energy density is half the other plants',
  an internal inconsistency. Built in `cover.rs` as a separate model rather than a stand.

### Plant traits side by side

Values are as authored; `—` is the base value. Water is on the available-water scale: 0 at
the wilting point, 1 at field capacity, 1.5+ wetter than drained. Light is 0–1 of open sky.

| trait | base | bloomcrown | umbrellafrond | springturf | stonecushion | velvetpad | lanternberry | vaulttree | siphonreed |
|---|---|---|---|---|---|---|---|---|---|
| crown height, m | 0.25–0.75 | 0.375–1.0 | 1.0–2.0 | 0.125–0.19 | 0.125–0.19 | 0.125 | 0.625–1.125 | 2.25–3.5 | 0.75–1.5 |
| crown radius, m | 0.125–0.375 | 0.125–0.31 | 0.375–0.75 | 0.125–0.25 | 0.125–0.16 | 0.19–0.375 | 0.25–0.44 | 0.625–1.0 | 0.06–0.125 |
| max wood (org) | 0.6 | — | — | 0.06 | 0.1 | 0.2 | 1.0 | 5.0 | 0.3 |
| assimilation /s per foliage | 0.004 | — | — | 0.008 | — | — | — | — | — |
| wood upkeep /s | 2e-4 | — | — | 2e-3 | 5e-5 | — | — | 1e-4 | 4e-4 |
| leaf life (s) | 1,000 | — | — | — | 3,333 | 2,000 | — | 2,000 | — |
| foliage regrowth /s per wood | 0.002 | — | — | 0.02 | 0.0005 | 0.004 | — | 0.001 | — |
| light half-saturation | 0.5 | 0.8 | 0.15 | 1.0 | 0.3 | 0.1 | 0.4 | 0.6 | 0.8 |
| germination light (sky) | 0.3 | 0.6 | 0.1 | 0.75 | 0.4 | 0.05 | 0.35 | 0.9 | 0.6 |
| wilts / full at water | 0.1 / 0.5 | 0.05 / 0.5 | 1.0 / 1.5 | 0.1 / 0.5 | 0 / 0.2 | 0.25 / 0.9 | 0.1 / 0.5 | 0.1 / 0.5 | 1.0 / 1.5 |
| germination water | 0.2 | 0.1 | 1.2 | 0.3 | 0.05 | 0.4 | 0.4 | 0.2 | 1.2 |
| waterlogging tolerance | 0.5 | 0.25 | 1.0 | 0.3 | 0.4 | 0.6 | 0.5 | 0.2 | 1.0 |
| drowns in (m) | 0 | 0.05 | 0.5 | 0.03 | 0.02 | 0.1 | 0.1 | 0.1 | 0.5 |
| root depth (m) | 0.5 | 0.5 | 1.0 | 0.25 | 0.5 | 0.5 | 0.75 | 1.5 | 0.5 |
| sprout → seeding | — | ~34 min | ~34 min | 161 s | ~2.2 h | ~16 min | ~32 min | ~3.2 h | — |
| seconds per seed (funded) | 300 | 300 | 300 | 9 | 600 | 90 | 150 | 300 | 200 |
| seed size (× minimum) | 1 | 2 | 1 | 1 | 1 | 1 | 3 | 4 | 1 |
| dispersal, reach | drop 0.25 m | drop 0.5 m | spores 1.5 m | drop 0.75 m | wind (median 0.26 m) | spores 1.5 m | drop 0.5 m | wind (median ~1 m) | water 1.5 m |
| runners / rhizomes | 0 | 0 | 0.5 | 0.3 | 0 | 0.3 | 0 | 0 | 0.7 |
| seed bank life | 8 h | 8 h | 4 h | 12 h | 12 h | 3 h | 8 h | 6 h | 10 h |
| ungrazeable base | 0.15 | — | — | 0.3 | 0.4 | 0.3 | — | — | — |
| edible in a browser's band (adult) | | 0.255 | 0 | 0.084 | 0.12 | 0.28 | 0.364 | 0 | 0 |
| browsers one adult sustains | | 4.9 | 0 | 2.5 | 0.2 | 4.9 | 8.2 | 0 | 0 |

Every species shares foliage = 2 × wood, a reserve of half its wood (vaulttree 1.5×), build
cost 1.2, mineral 0.02 per unit of tissue and energy 2.0 per unit.

---

## Decomposers

### Glowcap — *Lucerna saprophaga*
**The wood fungus of the decomposer grove.** Not a plant: it earns nothing from light.
- A fleshy folded fan growing out of a split log, 0.125–0.25 m tall. One colony is one stand.
- It establishes only where its box holds at least 0.02 of dead wood or litter. The box is
  its own site and neighbours, one row up and down.
- Eats that box down at 0.02 per unit of its wood per second, keeping 40 % as tissue. Income
  is ~3× upkeep. Grows to wood 0.1.
- Spreads by spores (1.5 m) that land only where substrate is; 2-hour bank. Diebacks when the
  box is exhausted; its wood goes back to the log.
- Its caps are littershredder food. Since it also eats litter, it competes with the shredder.
- *Reality check:* its carbon-use efficiency (0.4) is realistic; its uptake rate is a
  placeholder.

### The dead pools
- **Litter** feeds the littershredder and the glowcap and decays by itself.
  - Sources: leaf fall, dead seeds, a dead vine's leaves, animal dung (the gut is voided
    every 30 s), and a lost clutch.
  - *Reality check:* decays 2.6–10× faster than tropical leaf litter on the biology clock.
- **Dead wood** comes from dieback and from fallen vaulttrees (laid as a line). It is eaten by
  the glowcap and decays at a tenth of the litter's rate.
- **Carrion** is every dead animal's whole body. It feeds the littershredder and decays in ~200
  s (a 15 kg carcass takes 3–10 days in the tropics, which fits the clock).

---

## Consumers

Both animals are **founder lineages**. A trained brain (a small recurrent network) or a
hand-written heuristic drives a continuous body.

The body and its costs:
- **Moving.** The body turns and walks; motion is paid per requested metre and per turn,
  even when a wall blocks it.
- **Senses.** Contact whiskers (front, left, right, underside), wetness at the feet, taste at
  the mouth and a feed of its own state. The frondgrazer adds a vision cone; the shredder
  adds a litter-smell field.
- **Eating.** One bite per 0.25 s, sized by hunger, so a full animal eats nothing. A bite
  diminishes as the food at the mouth runs low.
- **Upkeep and growth.** Upkeep comes out of the reserve, then out of the body. The body grows
  out of the reserve at a capped rate.
- **Breeding.** After 15 minutes of unbroken surplus.
- **Death.** By starvation, drowning (under water deeper than its own height for 60 s) or age.
  Every death leaves the whole body as carrion.

### Frondgrazer — *Tardigradus pratensis*
**The ground browser of the meadow**, and the browser lineage. Diurnal.
- **Body.** Adult 0.75 m long, 0.375 m wide and tall: a saddle-shielded quadruped of roughly
  10–30 kg by its size. It grows from 0.005 to 0.05 org-units, with length scaling as the cube
  root of mass. A newborn is 46 % of adult length.
- **Diet.** Foliage of any photosynthetic plant inside its mouth band: 0 to 0.5 m above the
  ground it stands on, reaching 0.19 m ahead. Never fungus, never vine leaves.
- **Senses.** The eye is at 0.8 of its height, with a cone of 45 rays reaching 2 m, pitched
  −40° to +40° in three sectors. It sees foliage and other bodies. Taste reads the foliage at
  its mouth, saturating at 0.05.
- **Movement.**
  - Cruise 0.75 m/s (1 body length/s); turns at up to 2 rad/s.
  - Steps up or down 0.375 m (its own height). It cannot climb walls.
  - Wades 0.19 m of water; drowns under its own height after 60 s.
- **Energy.**
  - Upkeep 0.1 % of its body per second (0.05 % asleep). Walking at full cruise costs as much
    again.
  - A full reserve (half its body) lasts 500 s standing, 250 s walking and 1,000 s asleep. It
    wakes from sleep when half the reserve is gone.
  - Its mouth can take 16× its upkeep. Its tissue needs 0.05 mineral per unit and foliage
    carries 0.02, so mineral caps what it builds at 40 % of a bite.
- **Life cycle.**
  - Grows to breeding size in ~44 minutes.
  - To breed it needs 15 minutes with its reserve above 60 % of full. It then carries the
    young for 3 minutes (the cost paid in instalments) and gives birth to one.
  - It rests 30 minutes before the next surplus can count. First birth ~62 minutes after its
    own.
  - Lives a median 4 hours (±20 %) and has 3–5 young in a lifetime.
- **Eaten by** nothing yet. Its dung is litter and its body is carrion.
- *Reality check:*
  - Walking cost is realistic, if anything cheap.
  - Intake capacity is 4–10× too high, and gestation 20–30× short on the biology clock.
  - It can see 2 m while a full reserve walks 187 m.
  - Relative to the plants it is metabolically a ~10 g animal, which is why it parks
    (audit §1).

### Littershredder — *Segmenta caeca*
**The blind litter feeder**, and the blind lineage. Nocturnal.
- **Body.** Adult 0.375 m long, 0.125 m in section, of five overlapping plates with a blunt
  plough and two feelers: roughly 0.5–2 kg by its size. It grows from 0.003125 to 0.0125
  org-units.
- **Diet.** Litter, carrion and glowcap caps, taking whichever is richest at its ground-level
  mouth.
- **Senses.** No eyes. It uses contact, wetness and taste, plus a smell field that pools
  litter and carrion and spreads (2 s half-life, 40 % mixed into the neighbours every half
  second). The plume is detectable only within ~2 cells of a pile.
- **Movement.**
  - Cruise 0.375 m/s; steps 0.25 m.
  - **Climbs walls** at half pace for twice the cost, up risers and down cliffs.
  - Wades 3 cm; drowns under its own height (0.125 m) after 60 s.
- **Energy.**
  - Same upkeep and walking price as the frondgrazer, per unit of body.
  - Its mouth can take 20× its upkeep. Its tissue mineral equals litter's, so its food never
    limits it.
- **Life cycle.**
  - Reaches full size in ~46 minutes.
  - After 15 minutes of surplus it lays **one egg** (a third of its own body) on litter. The
    egg hatches after 5 minutes unless the water over it passes 0.2 m.
  - Egg to egg takes ~66 minutes. It lives a median 4 hours and lays 3–6 eggs.
- **Eaten by** nothing yet.
- *Reality check:*
  - Litter yield (0.5) is above the real 0.05–0.3.
  - One huge egg where arthropods lay 15–60 small ones.
  - It drowns in a minute where arthropods last hours.
  - For an ectotherm, walking is 3–8× too cheap.
  - A starter litter tile holds 16 of its bodies in food, so it booms on litter.

### Animal traits side by side

| trait | frondgrazer | littershredder |
|---|---|---|
| active | by day | by night |
| adult size (m) | 0.75 × 0.375 × 0.375 | 0.375 × 0.125 × 0.125 |
| body (org-units), newborn → adult | 0.005 → 0.05 | 0.003125 → 0.0125 |
| upkeep, /s per unit body | 0.001 (asleep 0.0005) | 0.001 (asleep 0.0005) |
| walking at full cruise, extra | 1× upkeep | 1× upkeep (wall: 2×) |
| cruise | 0.75 m/s | 0.375 m/s |
| step up / down | 0.375 / 0.375 m | 0.25 / 0.25 m, climbs walls |
| wades / drowns | 0.19 m / own height for 60 s | 0.03 m / own height for 60 s |
| bite, org /s when starving | 0.002 | 0.0005 |
| half-stock (bite halves) | 0.005 | 0.00125 |
| yield (effective) | 0.5 (0.4, mineral-limited) | 0.5 |
| reserve cap | 0.5 × body | 0.5 × body |
| growth cap, org /s | 9.5e-6 | 3.4e-6 |
| offspring | live birth: 0.01 each after 180 s gestation | 1 egg of 0.004, 300 s incubation |
| surplus to breed | reserve ≥ 60 % of cap for 900 s | ≥ 74 % for 900 s |
| rest between young | 1,800 s | 1,800 s |
| first young | ~62 min | ~61 min laid, ~66 min hatched |
| median lifespan | 4 h (±20 %) | 4 h (±20 %) |
| senses | cone 2 m, contact, wet, taste | smell field, contact, wet, taste |

---

## Designed, not built

These are written up in `design/organism-large-animals-2026-09-23.md` and
`design/predation-and-flight-2026-09-28.md`. Their placeholder masses use the same org-units
and inherit the plant:animal mismatch.

- **Lanternjaw** — a mesopredator ambusher (being built now as package P1).
  - A slim, coyote-scale body (1.0 × 0.25 × 0.45 m, `body_max` 0.035). It holds dead still
    under a luminous lure on its brow and costs a fifth of a frondgrazer's upkeep.
  - Strikes wound rather than kill outright; it takes ~3 hits to kill an adult frondgrazer.
  - Nocturnal. Lays buried dormant eggs that hatch at dusk once enough prey footsteps have
    passed overhead, and not while an adult is near.
  - Eats frondgrazers, littershredders and carrion.
- **Chorister** (*Lycochorus*) — a pack hunter.
  - 1.4 m with a counterweight tail, in packs of 3–5 with one breeder and a den.
  - Hunts frondgrazers alone and loftstriders as a pack. Pushes lanternjaws off carcasses.
- **Loftstrider** (*Grallator vellicans*) — the canopy browser. A 1.5 m hull on stilts that
  eats foliage 0.8–3.5 m up: the first consumer of adult vaulttrees and umbrellafronds, and a
  sloppy feeder that drops litter.
- **Bellwing** (*Campanula volans*) — a small flyer.
  - A 0.125 m bell-shaped body with a 0.375 m span, flying at 8 body lengths per second.
  - Feeds on nectar (latticevine, later bloomcrown) and carries pollen. Grounded by
    downpours; diurnal.
- **Seedporter** — a climber and glider of 0.5 × 0.25 × 0.2 m.
  - Climbs latticevine and woody plants, glides at a ratio of 2.5, and eats latticevine and
    lanternberry fruit, carrying their seeds.
  - Prey to the lanternjaw when grounded.

## Glossary

- **band (mouth band)** — the heights a mouth can reach above the ground an animal stands on.
- **body-turnover** — the time an animal takes to burn its own body's worth at rest: 1,000 s.
- **founder / lineage** — a controlled animal line; "browser" = frondgrazer, "blind" =
  littershredder.
- **graze refuge** — the share of a plant's foliage no bite can take.
- **org-unit** — the abstract unit of organic matter shared by every organism.
- **support face** — the top face of a solid voxel with air above it: where things stand and
  grow.
