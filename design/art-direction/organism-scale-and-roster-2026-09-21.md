---
design_status: proposal
last_reviewed: 2026-10-08
synced_at: e66b92138e8a3aab6c5b15729b7195a0911f18a1
decision_refs: []
---

# Organism scale audit and broad roster (Wrysk's species session, part 1)

> **Sim sync (2026-10-08):** ecology below is checked against `e66b92138e8a3aab6c5b15729b7195a0911f18a1`.
> `Sim state` distinguishes shipped code defaults from the named desktop terrarium
> preset; founder status means configured availability, not an observed population.
> Other presets, saved worlds and trained brains can differ. Config keys refer to
> `config/desktop/terrarium.toml` where stated; `design/parameters.md` lists code
> defaults, not that preset's overrides. Numbers are tunable facts of this revision.
> The look, body proposals and visual-review notes are retained. Missing ecological
> intent is labelled *not in the sim yet*; implementation does not confer canon.

Written in Wrysk's own design session, 2026-09-21. Everything is a proposal until Wrysk
marks it. Cites `Cubarium_Art_Direction_v0.1.md` (pillars 1, 4, 6, 7, 9), the roles in
`design/theoretical-biosphere-2026-09-16.md` §4, and the simulation as it stands on main
(`crates/cubarium-voxel-flora/src/lib.rs` species constructors,
`crates/cubarium-voxel-fauna/src/manifest.rs`, `crates/cubarium/src/voxel/animal.rs`,
`crates/cubarium-voxel/src/recipe.rs` `Recipe::SMALL`, `config/tachyon/voxel.toml`).

Diagram: `art/gen/runs/2026-09-21-scale-audit/compare.png` (today over proposed, both to
scale on a 1920×1080 frame; `today-1080p.png`, `proposed-1080p.png` separately).

## 1. Historical screen audit (2026-09-21)

| Quantity | Tachyon panel today | Note |
| --- | --- | --- |
| World | 160 × 48 × 24 voxels, 0.125 m | 20 m around, 6 m tall, 3 m deep |
| Raster | 4 px/voxel, 640 × 360, ×3 nearest → 1920 × 1080 | 240 rows are world (192 height + 48 depth rise), **120 rows are empty sky by configuration** |
| One voxel on screen | 12 × 12 screen px | on the 5.5″ AMOLED ≈ 0.76 mm; on a 24″ 1080p monitor ≈ 3.3 mm |
| One logical sprite pixel | 3 screen px | the unit any Stage 2 sprite is drawn in |
| Terrain surface | ≈ 0.9 m (basin floor) to ≈ 4.5 m (back ridge) | 7 to 36 of the 48 voxels; air over the ground averages ≈ 2.5 m = 20 voxels |

## 2. Historical authored dimensions (2026-09-21)

The tables below are the original pre-ladder measurement, retained for comparison.
Current envelopes and sources are in the per-species Sim state entries below.

Plants (`crown_height_voxels` × diameter from `crown_radius_voxels`, min wood → max wood):

| Species | Height | Diameter | Screen height at max | On the 5.5″ panel |
| --- | --- | --- | --- | --- |
| umbrellafrond | 2 → 5 | 2 → 5 | 60 px | 3.8 mm |
| bloomcrown | 1 → 3 | 1 → 3 | 36 px | 2.3 mm |
| springturf | 0.5 → 1 | 1 → 2 | 12 px, one cell, no stem | 0.8 mm |
| velvetpad | 0.5 → 1 | 2 → 4 | 12 px | 0.8 mm |
| stonecushion | 0.5 | 1 → 2 | 6 px | 0.4 mm |
| glowcap | 0.5, one cell always | 1 | 6 px | 0.4 mm |

Animals (the drawn shell is twice the manifest body; sensing, collision and the mouth
keep the manifest footprint):

| Founder | Model body | Drawn shell | Screen size | Reach |
| --- | --- | --- | --- | --- |
| frondgrazer (Browser) | 0.25 × 0.125 m | 4 × 2 × 2 voxels | 48 × 24 px | 1 voxel out, 1 up; climbs 1; wades 0.05 m, drowns at 0.2 m |
| littershredder (Blind) | 0.125 × 0.0625 m | 2 × 1 × 1 voxels | 24 × 12 px | as above |

Dead wood, litter and carrion are per-site scalar pools, not objects (see the handoff).

## 3. Historical verdict on the "thin layer of ants" worry (2026-09-21)

This is the retained pre-ladder diagnosis, not the 2026-10-08 sim state. The ladder,
canopy roles, flight and climbing now exist; the original evidence follows:

- **Vegetation is capped at 5 voxels** over an air column of about 20. Nothing living
  stands taller than 5 % of the frame. In the R1 mockup the stalks reach 35 % of the
  frame and canopies span 10 % of its width; that is what "forest verticality" meant.
- **Four of the six plants are sub-voxel.** Turf, pad, cushion and cap are 6–12 screen
  px tall: a texture, not a form. They cannot carry the growth states the sheets want.
- **The plant : animal ratio is inverted.** Umbrellafrond at 5 voxels is barely taller
  than the grazer's shell is long (4). In R1 the plants are 6–8 animal-lengths tall. The
  animals themselves are roughly at R1 scale (R1's creatures are ≈ 40–50 px on 1080).
- **The air column is unoccupied.** Nothing flies, climbs or hangs; there is no role in
  the sim that lives above 0.6 m. A third of the frame is empty by configuration.
- Terrain is fine. Relief 1.7 m plus a 1.5 m back rise plus benches and grottos gives
  masses, ledges and a cutaway; the landform rule (front low, peaks behind) holds.

## 4. Levers, and the recommendation

| Lever | Effect | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Size ladder in the model's authored geometry.** Raise `crown_height_voxels` / radius per species; raise the two manifest body lengths; add the three tall roles (vaulttree, lanternberry, siphonreed). | Plants 3–28 voxels, forest strata, canopy shade becomes a real gradient. | Every number is a stated placeholder, so this is legitimate; light competition changes (bigger crowns, fewer individuals) and needs a short rebalance run. | **Do it.** This is the only lever that creates verticality. |
| **B. World height 48 → 72** (9 m). Terrain is authored in metres, so the ground stays and gains 3 m of air. | Uses the 120 empty sky rows for canopy and flyers. | 1.5× voxels for water and light columns; measure on the board. | **Do it with A.** |
| **C. Logical resolution 4 → 6 px/voxel** (raster 960 × 540, ×2 nearest). 160 × 6 = 960 exactly; 72 × 6 + 24 × 3 = 504 of 540 rows. | A 6-voxel grazer is 36 px long, a 14-voxel frond 84 px: sprites can carry parts. Screen size per voxel is unchanged (12 px). | Presenter raster change only; the sprite renderer does not exist yet, so nothing is thrown away. | **Recommend.** 8 px/voxel would need 1280 wide and lose the whole ring. |
| D. `px_per_voxel` 8 on the current raster | Doubles everything | Shows 10 m of the 20 m ring; scrolling or a second panel | No |
| E. Halve `voxel_m` | 8× voxels | 8× simulation cost on the board | No |

Under A + B + C the working sprite scale stays what the kit already uses for judging
(8 px/voxel logical sheets, checked at 6 and 4), and the design sheets in
`species-designs-draft-2026-09-21.md` keep their voxel dimensions; only the voxel counts
per species change.

### The retained size ladder (look proposals) against the current sim

| Stratum | Role | Retained proposed height × width | In sim at sync | Implementation |
| --- | --- | --- | --- | --- |
| ground | springturf clump | 1–1.5 × 2–4 | 1–1.5 × 2–4 | implemented; wind/burr/runners and curing; sprites are **clumps**, not blades |
| ground | velvetpad carpet | 1 × 3–6 | 1 × 3–6 | implemented early-colonizer cover; autotiled carpet |
| ground | stonecushion | 1–1.5 × 2–3 | 1–1.5 × 2–2.5 | implemented |
| ground | glowcap on dead wood | caps 1–2 over a 1.5-tall wood tile | 1–2 × 1 caps | implemented, separate fungus slot; dead-wood tile family |
| ground | litter, carrion | marks, 0.5 | pools; carcass identity | implemented; sprites by pool amount |
| mid | bloomcrown | 3–8 × 2–5 | 3–8 × 2–5 | implemented, separate nectar |
| mid | lanternberry shrub | 5–9 × 4–7 | 5–9 × 4–7 | implemented, separate fruit |
| mid | siphonreed | 6–12 × 1–2, clumps | 6–12 × 1–2 | implemented on ≥0.1 m settled water |
| canopy | umbrellafrond | 8–16 × 6–12 | 8–16 × 6–12 | implemented, low aeration stress |
| canopy | vaulttree | 18–28 × 10–16, crown with gaps | 18–28 × 10–16 | implemented, fall-line wood |
| animals | frondgrazer | 6 × 3 × 3 shell (body 0.375 m) | physical 6 × 3 × 3 | implemented; historical shell intent retained |
| animals | littershredder | 3 × 1 × 1 (body 0.19 m) | physical 3 × 1 × 1 | implemented |
| animals | capgnawer | 2 × 1.5 | physical 2 × 1.68 × 1.52 | live founder |
| animals | ripple snail | 1.5 × 1.5 | physical 1.52 × 1.52 × 0.504 | live amphibious founder; glassfilm food |
| animals | lanternjaw | 5 × 2 | physical 8 × 2 × 3.6 | live dormant-clutch founder |
| air | bellwing | 2 × 3 wingspan, flies 4–20 up | physical 1 × 1 × 0.5; span 3 | live flyer; band max 20 voxels |
| air | seedporter | 4 × 2, climbs trunks | physical 4 × 2 × 1.6 | live climber/glider |

Vertical fill under the ladder: ground band 1–2 voxels dense, mid band to 9, canopy to
28 of a 72-voxel world with ≈ 40 voxels of air over the mean ground. That is the R1
proportion (canopy at a third of the frame) without changing what a voxel is.

## 5. Broad organism roster: what each sprite reflects

The rule (art direction pillar 1, §05): a sprite family is indexed by simulation state
and nothing else; ornament is fixed per family. Columns: the state the sprite reads,
the one feature that tells the niche, and the sheet strategy (Wrysk's cohesion rule:
prefer one composite sheet over layered singles wherever the composition is stable).

### Sessile

| Role | Reads from state | Niche feature (silhouette) | Sheet strategy |
| --- | --- | --- | --- |
| **bloomcrown** | wood (size), foliage/α·wood (fullness), moisture (wilt), stage (alive / dieback / dead), separate nectar; reproductive parcel | an upright stem carrying one distinct crown mass; the sun plant of open ground | single-plant sheet, 4 sizes × 3 fullness × wilt; **stand-group variants** (2–3 stems in one sprite) for meadows |
| **umbrellafrond** | wood, foliage, moisture, aeration stress | a tall stem with a broad, drooping, layered umbrella; the wet-hollow plant | single sheet, 4 sizes × fullness; crown edge droop = wilt |
| **springturf** | wood (clump size), foliage (cropped vs full), stage | dense low tufts with a visibly cropped margin after grazing | **clump-pattern sheets**: 3 patterns × 3 sizes × cropped/full; autotile neighbours to read as one lawn |
| **stonecushion** | wood, moisture (the only plant that is "fine" while dry) | a compact pale dome hugging rock, roots into a crack | 2 sizes × 2 patterns, drawn **with the rock lip** it sits on |
| **velvetpad** | wood, foliage, light (lit or shaded damp ground) | a broad flat soft pad under ledges and crowns | **carpet autotile** (edge/inner/corner) by coverage, not per plant |
| **glowcap** | wood (cap count/size), stage (button / fruiting / spent), substrate dead wood amount | few restrained luminous caps growing out of a split in dead wood | **composite dead-wood sheet**: wood autotile (lone / middle / end-L / end-R) × decay stage (amount eaten) × cap state (none / buttons / fruiting / spent) |
| **dead wood** (pool) | amount per site, neighbour sites with wood | a fallen trunk section; ends are torn | the composite above; a stand's death depositing a **line** of wood is a model change we recommend with vaulttree |
| **litter** (pool) | amount, energy | a scatter of fallen tissue, fine to coarse | 3-density marks, autotiled so patches read as one drift |
| **carrion** (pool) | amount | a collapsed body, then bone | 2 stages |
| lanternberry (planned) | wood, fruit product | mid shrub with hanging luminous fruit | fruit count from product stock |
| siphonreed (planned) | wood, water depth at site | upright clump of narrow stems standing in water | clump sheet × depth of water line |
| vaulttree (planned) | wood, foliage, gaps | a sparse branching vault, crown broken into lobes with sky between | 5 sizes; crown lobes drop as foliage falls |
| glassfilm (planned) | film thickness, grazed trails | a thin coloured coating on wet surfaces with cleared trails | material overlay, not a sprite |

### Animals (each: side, three-quarter toward, three-quarter away; poses from `State`)

| Role | Reads from state | Niche feature | Notes |
| --- | --- | --- | --- |
| **frondgrazer** | State (rest / walk / cropping), body (size), reserve (gaunt vs full), starving flag, heading | low broad shell, an upturned cropping cleft, six planted pads | sheet F1 (draft) with Astra's clarifications; historical size annotation from body 0.005 → 0.05 (current body quantities in Sim state) |
| **littershredder** | State, body, heading | segmented low crawler, stop-and-shred | blind: no eye at all; feelers forward |
| lanternjaw (live founder) | patrol / ambush / strike / handling | compact, long stillness, brief strike | a paid glow if kept |
| capgnawer (live founder) | nibbling visit | rounded body on caps | |
| bellwing (live founder) | hover / land / sip, flight | paired wings, hover-land-sip rhythm | the air-column animal |
| seedporter (live founder) | climb / carry | long tail or carrying posture on trunks | |
| ripple snail (live founder) | slow crawl on film | flat spiral or domed shell, cleared trail | |

## Current roster and source links

Nine stand species, nine animal founders, latticevine cover and glassfilm are live
mechanisms. “Model-only” no longer describes any named animal here. Availability
is distinct from a successful founding or a surviving population.

## frondgrazer

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 8 bodies; code default 8 bodies (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- diet: vascular foliage only; includes reachable stand layers and vine leaves, excludes fungus (source: `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- size: adult length × width × height 0.75 × 0.375 × 0.375 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## littershredder

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 8 bodies; code default 8 bodies (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- diet: litter and carrion only; cap tissue is excluded (source: `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- size: adult length × width × height 0.375 × 0.125 × 0.125 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## capgnawer

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 6 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.capgnawer.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2047`)
- diet: glowcap caps and exposed mycelium (CapTissue); litter at weaker yield; no carrion (source: `crates/cubarium-voxel-fauna/src/body.rs:2047`)
- size: adult length × width × height 0.25 × 0.21 × 0.19 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2047`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## loftstrider

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 2 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.loftstrider.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2099`)
- diet: vascular foliage within its high mouth band; some bite material spills to litter (source: `crates/cubarium-voxel-fauna/src/body.rs:2099`)
- size: adult length × width × height 2.2 × 2.2 × 3.4 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## lanternjaw

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 3 dormant clutches; code default 3 dormant clutches (sources: `crates/cubarium-voxel-fauna/src/body.rs:1930`; `crates/cubarium-voxel-fauna/src/body.rs:1930`)
- diet: carrion from kills or scavenging; eligible prey list is littershredder, frondgrazer, capgnawer, ripple-snail, bellwing and grounded seedporter (sources: `crates/cubarium-voxel-fauna/src/body.rs:1930`; `crates/cubarium-voxel-fauna/src/body.rs:1539`)
- size: adult length × width × height 1.0 × 0.25 × 0.45 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:1930`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## bellwing

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 6 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.bellwing.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- diet: nectar from bloomcrown and latticevine; mineral rides in nectar, not a separate pollen meal (source: `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- size: adult length × width × height 0.125 × 0.125 × 0.0625 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2167`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## seedporter

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 4 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.seedporter.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2251`)
- diet: lanternberry and latticevine fruit; carrion via opportunistic bellwing catches (catch_bellwings enabled) (source: `crates/cubarium-voxel-fauna/src/body.rs:2251`)
- size: adult length × width × height 0.5 × 0.25 × 0.2 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2251`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## Veilhound (`chorister`)

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 1 pack (4 adults); code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.chorister.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2328`)
- diet: carrion from hunting/scavenging; prey list frondgrazer, lanternjaw, loftstrider, bellwing and grounded seedporter (source: `crates/cubarium-voxel-fauna/src/body.rs:2328`)
- size: adult length × width × height 1.4 × 0.35 × 0.6 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2328`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## ripple-snail

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder in the desktop terrarium: 8 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.ripple-snail.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2387`)
- diet: glassfilm only, including wet land/shore film; no vascular foliage (source: `crates/cubarium-voxel-fauna/src/body.rs:2387`)
- size: adult length × width × height 0.19 × 0.19 × 0.063 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2387`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)

## springturf

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: sunlit drained ground; establishment light ≥0.75, preset available water ≥0.2; drought curing/dormancy lives on stores (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- size: authored crown height × diameter 0.125–0.1875 × 0.25–0.5 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)

## velvetpad

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: damp lit or shaded ground, saturation tolerated at establishment; early colonizer displaced by sun-loving turf (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)
- size: authored crown height × diameter 0.125 × 0.375–0.75 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)

## stonecushion

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: rock/bedrock support with reachable soil water; establishment available water ≥0.05, light ≥0.4, drowning threshold 0.02 m (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)
- size: authored crown height × diameter 0.125–0.1875 × 0.25–0.3125 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)

## bloomcrown

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: open bright ground; light_half 0.8, establishment light ≥0.6, available water ≥0.1, saturated share ≤0.25; pool threshold 0.05 m (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- size: authored crown height × diameter 0.375–1.0 × 0.25–0.625 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)

## umbrellafrond

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: wet hollows; deep rooting_depth 4; establishment available water ≥1.2, light ≥0.1; saturation accepted but stress_rate_per_s 0.01, drown_depth_m 0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)
- size: authored crown height × diameter 1.0–2.0 × 0.75–1.5 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)

## glowcap

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: shade, caves and detritus patches; fungus slot shares a site with vascular plant; dark-biased establishment and dampness (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- size: authored crown height × diameter 0.125–0.25 × 0.125 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)

## vaulttree

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: lit gaps, deep aerated soil: rooting_depth 6, establishment light ≥0.9 and saturated share ≤0.2 (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- size: authored crown height × diameter 2.25–3.5 × 1.25–2.0 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)

## lanternberry

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: bright moist gaps; establishment water ≥0.4, light ≥0.35; wilt/full available water 0.1/0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- size: authored crown height × diameter 0.625–1.125 × 0.5–0.875 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)

## siphonreed

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- habitat: wet sunlit shallows; settled water depth at its own site ≥0.1 m; establishment available water ≥1.2/light ≥0.6, drown_depth_m 0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)
- size: authored crown height × diameter 0.75–1.5 × 0.125–0.25 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)

## latticevine

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live separate face-cover system, outside Species::ALL stand list; founders on eligible walls (sources: `crates/cubarium-voxel-flora/src/cover.rs:2251`; `crates/cubarium-session/src/lib.rs:62`)
- habitat: exposed rock/bedrock side and downward faces; soil roots at foot/top; stepped adjacency (sources: `crates/cubarium-voxel-flora/src/cover.rs:217`; `crates/cubarium-voxel-flora/src/cover.rs:290`)
- size: no face-count or reserve cap; extent constrained by resources and eligible faces (source: `crates/cubarium-voxel-flora/src/cover.rs:1312`)

## glassfilm

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live Ground film pool, seeded at founding in the desktop terrarium; code seeding default false (sources: `config/desktop/terrarium.toml` → `flora.seed_film_at_founding`; `crates/cubarium-voxel-flora/src/lib.rs:3230`)
- food_habitat: photosynthetic film on lit shallow submerged faces (0.01–0.6 m water), wet ground (pore ≥0.9) or damp shore (pore ≥0.6 within 0.5 m); dry film dies to litter (sources: `crates/cubarium-voxel-flora/src/film.rs:99`; `crates/cubarium-voxel-flora/src/film.rs:207`; `crates/cubarium-voxel-flora/src/lib.rs:6268`)
- consumers: ripple-snail, with grazing refuge 0.1 of capacity (sources: `crates/cubarium-voxel-fauna/src/body.rs:2387`; `crates/cubarium-voxel-flora/src/lib.rs:6239`)
- life_stages: per-site stock grows/spreads, is grazed and dries; no individual stand body or breeding stage (source: `crates/cubarium-voxel-flora/src/film.rs:207`)

## 6. Environment: what is simulated and what the art can show

- **Terrain**: material voxels (soil, rock, bedrock), strata by hardness bands, benches,
  grottos, soil up to 0.9 m deep. Art: material autotiles by contour and layering
  (§05), cavity interiors, exposed strata on the cut face. Soil **depth** is real;
  soil **quality** is one mineral number per site and litter/dead-wood pools. A humus or
  soil-quality field (Wrysk’s example) would give the ground its own visible gradient
  (dark rich hollows, pale thin ridges) and is *not in the sim yet*.
- **Water**: free water per voxel, pore water in soil, transit water on faces, aquifer,
  closed budget with a lumped atmosphere. Art: pool surface skin vs body (kit S8),
  wet-face darkening from transit water, seep lines where pore water is saturated.
- **Weather**: the named desktop terrarium uses the day cycle and tropical climate,
  with a dry season enabled; the old scheduled-shower description is historical.
  A cloud as an individual sim object is *not in the sim yet*. Art: rain streaks driven by the shower, a cloud band whose density follows
  `atmosphere_m3` (the real lumped atmosphere) so clouds are honest even before route C.
- **Light**: geometric sky visibility plus canopy shade per site. Art: shade under
  crowns from the same numbers; volumetric shafts are decoration and stay so.

None of this is generated in the organism pass; it is here so the organism designs
sit against the right ground.

## 7. Decisions (Wrysk, 2026-09-21: "agree with all four", plus item 5)

1. **The size ladder** (§4): A + B + C as written. Authored plant geometry and the two
   manifest body lengths rise to the ladder; world height 72; raster 6 px per voxel,
   960 × 540, ×2 nearest. Sprites are authored at **6 logical px per voxel** (the raster
   grid) and checked at 4.
2. **Concept sheets now** for the live six plus littershredder, and for the four
   unsimulated roles that fill the frame: vaulttree, lanternberry, siphonreed, bellwing.
   Seedporter, capgnawer, ripple snail, lanternjaw, glassfilm deferred.
3. **Composite-sheet families first**: dead wood × decay × glowcap; springturf clump
   patterns; velvetpad carpet.
4. **A dying vaulttree deposits dead wood along a line of sites** (model change, to be
   packaged with the vaulttree species).
5. **Litter × terrain** (Wrysk's addition): litter is drawn as an axis of the terrain
   surface tiles, lying in the ground's contours, never as a sprite floating on it.

**Implementation sync:** the ladder and vaulttree fall are implemented. The owner’s
historical wording and visual choices above remain unchanged; this review neither
adds accepted ledger entries nor promotes these proposals to canon.

**Confirmed against a reference (Wrysk, 2026-09-21, later).** Shown a 1:1 1080p
comparison of pixelated candidates at the ladder (grazer 6 voxels, 6 px/voxel ×2)
against a raised ladder (12 voxels) and against 12 px/voxel native
(`art/gen/runs/2026-09-21-scale-audit/pixelated-scale-compare-1080p.png`), beside
Wrysk's concept reference (`wrysk-concept-ref.png`, whose creature is a genuinely
larger animal, over 2 m tall): **the ladder as decided stands.** The concept's pixel
grain (about 2 screen px) is the ×2 grain already chosen. Revisit only if the finished
sprites read too small in the live scene.

**Seen at scale (Wrysk, 2026-09-23).** After the Blender lineup of every organism in a
panel-sized world box (`scripts/blender/organism_lineup.py`,
`runs/scale-lineup-2026-09-22/`): **the ladder sizes stand**, the growth fix is agreed
(seedlings capped in width too; tall species grow up from the seedling), the lanternjaw
grows to a coyote-scale mesopredator, and a canopy browser for adult vaulttrees is added
as a new niche, with a pack hunter to follow as a separate decision. Designs and numbers:
`design/organism-large-animals-2026-09-23.md`.
