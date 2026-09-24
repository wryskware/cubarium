---
design_status: proposal
last_reviewed: 2026-09-21
decision_refs: []
---

# Organism scale audit and broad roster (Wrysk's species session, part 1)

Written in Wrysk's own design session, 2026-09-21. Everything is a proposal until Wrysk
marks it. Cites `Cubarium_Art_Direction_v0.1.md` (pillars 1, 4, 6, 7, 9), the roles in
`design/theoretical-biosphere-2026-09-16.md` §4, and the simulation as it stands on main
(`crates/cubarium-voxel-flora/src/lib.rs` species constructors,
`crates/cubarium-voxel-fauna/src/manifest.rs`, `crates/cubarium/src/voxel/animal.rs`,
`crates/cubarium-voxel/src/recipe.rs` `Recipe::SMALL`, `config/tachyon/voxel.toml`).

Diagram: `art/gen/runs/2026-09-21-scale-audit/compare.png` (today over proposed, both to
scale on a 1920×1080 frame; `today-1080p.png`, `proposed-1080p.png` separately).

## 1. What a voxel is on the screen

| Quantity | Tachyon panel today | Note |
| --- | --- | --- |
| World | 160 × 48 × 24 voxels, 0.125 m | 20 m around, 6 m tall, 3 m deep |
| Raster | 4 px/voxel, 640 × 360, ×3 nearest → 1920 × 1080 | 240 rows are world (192 height + 48 depth rise), **120 rows are empty sky by configuration** |
| One voxel on screen | 12 × 12 screen px | on the 5.5″ AMOLED ≈ 0.76 mm; on a 24″ 1080p monitor ≈ 3.3 mm |
| One logical sprite pixel | 3 screen px | the unit any Stage 2 sprite is drawn in |
| Terrain surface | ≈ 0.9 m (basin floor) to ≈ 4.5 m (back ridge) | 7 to 36 of the 48 voxels; air over the ground averages ≈ 2.5 m = 20 voxels |

## 2. What the simulation authors today, in voxels

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

## 3. Verdict on the "thin layer of ants" worry

It is real, and it is a plant problem more than an animal problem.

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

### The size ladder (proposed, voxels of 0.125 m; screen px at 12 per voxel)

| Stratum | Role | Height × width | In sim today | Change |
| --- | --- | --- | --- | --- |
| ground | springturf clump | 1–1.5 × 2–4 | 0.5–1 × 1–2 | crown [1.0, 1.5], radius [1, 2]; sprites are **clumps**, not blades |
| ground | velvetpad carpet | 1 × 3–6 | 0.5–1 × 2–4 | radius [1.5, 3]; autotiled carpet |
| ground | stonecushion | 1–1.5 × 2–3 | 0.5 × 1–2 | crown [1, 1.5] |
| ground | glowcap on dead wood | caps 1–2 over a 1.5-tall wood tile | 0.5, one cell | crown [1, 2]; dead-wood tile family |
| ground | litter, carrion | marks, 0.5 | marks | sprites by pool amount |
| mid | bloomcrown | 3–8 × 2–5 | 1–3 × 1–3 | crown [3, 8], radius [1, 2.5] |
| mid | lanternberry shrub | 5–9 × 4–7 | not in sim | new stand species + fruit product |
| mid | siphonreed | 6–12 × 1–2, clumps | not in sim | new emergent stand (wet sites, pool edge) |
| canopy | umbrellafrond | 8–16 × 6–12 | 2–5 × 2–5 | crown [8, 16], radius [3, 6] |
| canopy | vaulttree | 18–28 × 10–16, crown with gaps | not in sim | new stand species; dies to a multi-site dead-wood line (model decision) |
| animals | frondgrazer | 6 × 3 × 3 shell (body 0.375 m) | 4 × 2 × 2 | manifest body 0.25 → 0.375 m |
| animals | littershredder | 3 × 1 × 1 (body 0.19 m) | 2 × 1 × 1 | manifest 0.125 → 0.19 m |
| animals | capgnawer | 2 × 1.5 | not in sim | later |
| animals | ripple snail | 1.5 × 1.5 | not in sim | later; needs glassfilm |
| animals | lanternjaw | 5 × 2 | fixed lineage in cubarium-core only | later |
| air | bellwing | 2 × 3 wingspan, flies 4–20 up | not in sim | later; the first thing that fills the air column |
| air | seedporter | 4 × 2, climbs trunks | not in sim | later; needs vaulttree/lanternberry |

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
| **bloomcrown** | wood (size), foliage/α·wood (fullness), moisture (wilt), stage (alive / dieback / dead), later: reproductive product (bloom) | an upright stem carrying one distinct crown mass; the sun plant of open ground | single-plant sheet, 4 sizes × 3 fullness × wilt; **stand-group variants** (2–3 stems in one sprite) for meadows |
| **umbrellafrond** | wood, foliage, moisture, aeration stress | a tall stem with a broad, drooping, layered umbrella; the wet-hollow plant | single sheet, 4 sizes × fullness; crown edge droop = wilt |
| **springturf** | wood (clump size), foliage (cropped vs full), stage | dense low tufts with a visibly cropped margin after grazing | **clump-pattern sheets**: 3 patterns × 3 sizes × cropped/full; autotile neighbours to read as one lawn |
| **stonecushion** | wood, moisture (the only plant that is "fine" while dry) | a compact pale dome hugging rock, roots into a crack | 2 sizes × 2 patterns, drawn **with the rock lip** it sits on |
| **velvetpad** | wood, foliage, light (it only lives in shade) | a broad flat soft pad under ledges and crowns | **carpet autotile** (edge/inner/corner) by coverage, not per plant |
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
| **frondgrazer** | State (rest / walk / cropping), body (size), reserve (gaunt vs full), starving flag, heading | low broad shell, an upturned cropping cleft, six planted pads | sheet F1 (draft) with Astra's clarifications; size from body 0.005 → 0.05 |
| **littershredder** | State, body, heading | segmented low crawler, stop-and-shred | blind: no eye at all; feelers forward |
| lanternjaw (planned) | ambush / strike / handling | compact, long stillness, brief strike | a paid glow if kept |
| capgnawer (planned) | nibbling visit | rounded body on caps | |
| bellwing (planned) | hover / land / sip, flight | paired wings, hover-land-sip rhythm | the air-column animal |
| seedporter (planned) | climb / carry | long tail or carrying posture on trunks | |
| ripple snail (planned) | slow crawl on film | flat spiral or domed shell, cleared trail | |

## 6. Environment: what is simulated and what the art can show

- **Terrain**: material voxels (soil, rock, bedrock), strata by hardness bands, benches,
  grottos, soil up to 0.9 m deep. Art: material autotiles by contour and layering
  (§05), cavity interiors, exposed strata on the cut face. Soil **depth** is real;
  soil **quality** is one mineral number per site and litter/dead-wood pools. A humus or
  soil-quality field (Wrysk's example) would give the ground its own visible gradient
  (dark rich hollows, pale thin ridges) and is a model addition worth recording.
- **Water**: free water per voxel, pore water in soil, transit water on faces, aquifer,
  closed budget with a lumped atmosphere. Art: pool surface skin vs body (kit S8),
  wet-face darkening from transit water, seep lines where pore water is saturated.
- **Weather**: scheduled showers every 5–15 min, per-preset volume; there is no cloud
  object. Art: rain streaks driven by the shower, a cloud band whose density follows
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

Implementation packages that fall out of 1 and 4 are owed after the dossiers, not
before: they change what a sprite claims, so the sheets are written to the ladder.

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
