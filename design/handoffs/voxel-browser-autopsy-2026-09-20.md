---
status: open
date: 2026-09-20
owner: Wrysk's thread; diagnosis first, any change after
---

# Browser autopsy: why the frondgrazer founders die on the living generated world

## What is known

On the generated world with the closed water budget (`runs/closed-generated.toml`;
`design/handoffs/voxel-water-cycle-2026-09-20.md`), plants now live: 146–157
stands after one simulated hour. The littershredder lineage survives on its
trained default (24 alive at 60 min, `voxel-decomposers-and-defaults-2026-09-20.md`
S1, commit c510382). The **frondgrazer** lineage dies out in both arms:

| arm (60 sim min) | frondgrazer alive | bites | assimilated |
| --- | --- | --- | --- |
| trained default (P3-C gen322) | 0 | 8,989 | 1.004 |
| `--founder-heuristic all` | 1 | 4,206 | 0.854 |

The trained browser's bites and intake stop moving at about minute 26; at
26 min Fable's earlier run still had 4 alive. So the browsers eat, breed
(births are paid, no cooldown; see `voxel-live-founders-2026-09-20.md`),
and then starve with food standing around them.

Earlier evidence, from the dry authored world (`design/7_Research/voxel-census-
2026-09-20.md`, D2): browsers there travelled up to 225 m and starved because
foliage collapsed under them; only 2.2 % of browser body-minutes had a stand
inside the 0.0625 m mouth reach (mean nearest stand 0.351 m). That world had
no living plants, so the D2 numbers do not transfer; the question is open.

Suspects, none measured:
1. **Mouth reach versus foliage height.** The browser bites foliage from a
   stand whose crown is within its mouth reach (`mouth_foliage_stand`,
   `crates/cubarium-voxel-fauna/src/body.rs`). On the generated world stands
   grow (wood → crown height); a crown may rise out of reach, or the
   generated species mix (umbrellafrond 149 of ~150 stands) may not be what
   the browser can eat. Check which species it bites (`bites_by_plant`) and
   which are within reach.
2. **The same-height motion rule.** A founder can only step to a support
   face at exactly its own standing height (D2 found 83 % of blocked blind
   headings were drops or edges). On slopes each browser may be confined to
   its birth patch; when that patch's foliage is cropped, it cannot leave.
3. **Boom crowding.** Full-store browsers breed on their first tick with no
   cooldown; 100 born in an hour. A patch that feeds 8 may not feed 60.
4. **Cone against the living canopy.** The 2 m cone reads foliage as first
   hits; a tall dense canopy could read as foliage everywhere and remove the
   gradient (D2 noted the cone reads a stripped crown as an occluder; the
   inverse — everything reads as food — is the case to check).

## Tools already on main

- `crates/cubarium/examples/voxel_founder_autopsy.rs` (23f81f7): per-minute
  per-lineage alive, deaths by cause (ledger `Departure`, schema 7), body,
  reserve, bites, and every animal's distance to its nearest food against
  its sensed reach; per-body rows with pose, travelled distance, held
  actions and what is ahead. It was written for the authored world with the
  heuristics; it needs the generated scene, the closed config, and the
  trained default (mirror `voxel_census`'s `[generated] [closed]` arguments
  and the built-in policy path in `crates/cubarium/src/voxel/mod.rs`,
  `built_in_driver`).
- `voxel_census 6 generated closed` for the plant side; `voxel_plant_autopsy`
  for per-stand crown/foliage state.
- Fauna ledger fields `bites_by_plant`, `eaten_by_plant`,
  `bites_by_founder`, `deaths_by_founder_cause`.

## What to answer

1. Deaths by cause and minute for the browser lineage on the generated
   closed world, trained default, 60 minutes.
2. At each death: distance to the nearest stand with foliage, whether that
   stand's crown is within mouth reach, its species, and whether the body
   could have stepped toward it under the same-height rule (count support
   faces at its standing height within 2 m).
3. Which species the browsers bite, how much foliage those stands hold over
   time near the browsers, and whether cropped crowns regrow within the
   horizon.
4. The cone reading at the moment intake stops: is there a foliage gradient
   or does every sector read foliage?
5. Whether the boom matters: rerun with the founder count halved in the
   seeder (a run-time argument, not a constant change) and compare.

## Rules

Diagnosis first: instrumentation and examples only until the cause is
stated with evidence; any rule change (reach, motion, births, cone) is a
separate decision. Explicit-path commits ending with `Co-Authored-By: Claude
Fable 5.1 <noreply@anthropic.com>`; never `git add -A`; always fresh;
tests ≤ 200 ticks; no bit-identical pins; runs may use all cores; `runs/`
is disposable; windows only through `./scripts/run-voxel.sh --background`
(its Hyprland signature pick is broken in agent shells; the png sink works).
Record the result in `design/7_Research/voxel-census-2026-09-20.md` as a
new section, and one proposed change with the evidence that would show it
worked.
