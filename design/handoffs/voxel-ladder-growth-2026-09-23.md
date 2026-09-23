# Package L: organism sizes to the ladder, and the growth fix

Wrysk, 2026-09-23. He has seen every organism at true scale in a panel-sized box
(`scripts/blender/organism_lineup.py`, renders `runs/scale-lineup-2026-09-22/`) and
accepted the size ladder. He chose to move the bodies now even though the retrain in
flight learned the old sizes ("rip off the bandaid"); he tells the trainer thread
himself. This package lands on **main**, never on the `retrain` branch.

Read first: `design/organism-large-animals-2026-09-23.md` §1–2 and §7,
`design/organism-anatomy-2026-09-21.md` §3,
`design/art-direction/organism-scale-and-roster-2026-09-21.md` §4 and §7,
`design/handoffs/voxel-organism-decisions-2026-09-21.md` (decisions 1, 2, 5, 7).

Recommended worker: Opus, high effort. It touches flora, fauna, seeding, route
connectivity and the presenter at once.

## 1. Plant crowns in metres, at the ladder

Today `SpeciesConfig.crown_height_voxels` / `crown_radius_voxels` are in 0.25 m
reference voxels and `FloraConfig::for_voxel_size` rescales them
(`crates/cubarium-voxel-flora/src/lib.rs`: `crown_height` L852, `crown_radius` L859,
`crown_height_staged` L843, `for_voxel_size` ~L1827). State them **in metres** and stop
rescaling them per voxel size, so every preset (small 0.125 m, default 0.25 m, desktop
0.125 m) grows the same physical plant. Rooting, hop and substrate reach stay as they
are (out of scope).

| Species | Height m [min, max] | Radius m [min, max] |
| --- | --- | --- |
| springturf | 0.125, 0.1875 | 0.125, 0.25 |
| velvetpad | 0.125, 0.125 | 0.1875, 0.375 |
| stonecushion | 0.125, 0.1875 | 0.125, 0.15625 |
| glowcap | 0.125, 0.25 | 0.0625, 0.0625 |
| bloomcrown | 0.375, 1.0 | 0.125, 0.3125 |
| umbrellafrond | 1.0, 2.0 | 0.375, 0.75 |

(Vaulttree, lanternberry and siphonreed are package N, same rules.)

## 2. Growth fix

For a species whose first profile stage is a capped seedling (`height_m_max` set; today
bloomcrown and umbrellafrond):

- **Seedling** (wood fraction ≤ `w0`, stage 0's `wood_fraction_max`): height
  ≤ 0.125 m **and radius ≤ 0.125 m**. Today only height is capped, so a seedling's radius
  follows the full range: an umbrellafrond seedling is a flat star 0.8 m across.
- **After the seedling:** `h = 0.125 + (h_max − 0.125)·(t − w0)/(1 − w0)`, and the same
  for radius from 0.125. So growth starts from the seedling, not from the range's
  minimum. For these species the range's minimum is unused.
- Species without a capped seedling stage keep today's linear `min + t·(max − min)`.

Accepted consequence (Wrysk): a young umbrellafrond's lowest tier (0.5 of its height)
stays inside the frondgrazer's mouth band (≤ 0.5 m, below) until the frond is about 1 m
tall.

## 3. Animal bodies at the ladder

`crates/cubarium-voxel-fauna/src/body.rs` (adult dims ~L292 and ~L318):

| Founder | Adult L × W × H (m) | Was |
| --- | --- | --- |
| frondgrazer (browser) | 0.75 × 0.375 × 0.375 | 0.375 × 0.1875 × 0.1875 |
| littershredder (blind) | 0.375 × 0.125 × 0.125 | 0.19 × 0.0625 × 0.0625 |

Eye (0.8 H), mouth band [0, 1.33 H], reach 0.25 L and contact (0.5 H) are fractions and
follow automatically. The browser's mouth band becomes [0, 0.5 m]. Growth ∝
structure^(1/3), rates and energetics unchanged (decision 7: abstract units, no allometry).
`climb_m` stays (browser 0.25 m, shredder 0.125 m) unless the step rule breaks for the
taller body; if it does, report it rather than retuning.

## 4. What must still hold, and what to measure

- Mass balance and every existing conservation test. Snapshot schemas bump (flora and
  fauna); old worlds are refused, not migrated (standing rule).
- Occupancy, cone ray classes and route connectivity use the new body sizes. A 6-voxel
  browser may no longer fit through gaps its 3-voxel self used; **measure** the seeded
  route (RouteMap) and the startup acceptance on 8 seed bases × 3 presets
  (small, default, wide) before and after, medians and ranges. One-seed arms are noise.
- Report reach per species (share of each species' foliage inside the browser's band at
  t=0 and at 60 min), bites by species, and browsers/shredders alive at 60 min, 8 seeds.
- The presenter draws the model body and the new crowns without crashing. The look is
  package V's; don't polish it here.

## 5. Tests (authored first, separate pass, from this brief)

Short function tests only:
1. Crown height and radius in metres are the same physical size at voxel sizes 0.125 and
   0.25.
2. A capped-seedling species at `wf ≤ w0` has height ≤ 0.125 m and radius ≤ 0.125 m.
3. Just past `w0` a stand is ≈ 0.125 m in both, and at `wf = 1` it is at `h_max`, `r_max`.
4. The browser's adult mouth ceiling is 0.5 m; a bloomcrown adult's crown layer (0.55–1.0
   of 1 m) is out of band and its rosette (0–0.15) in band.
5. An umbrellafrond whose height is 0.8 m has its lowest tier in band; at 1.2 m it does
   not.

## Return (≤ 40 lines)

Commits, tests, the before/after table from §4, anything that broke because a body got
bigger (routes, gaps, drownings, seeding), and what you did not change and why.
