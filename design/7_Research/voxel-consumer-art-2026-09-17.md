---
design_status: paused
last_reviewed: 2026-09-17
decision_refs: []
---

> **SUPERSEDED (Wrysk, 2026-09-17).** This look was produced by an agent thread
> and is not the art direction. The art direction is
> `design/art-direction/Cubarium_Art_Direction_v0.1.md`. Nothing below is canon.

# The two consumers' look: glowcap cluster and frondgrazer at 4 and 8 px per voxel

Fable art thread for `design/handoffs/voxel-round5bc-consumers-briefs-2026-09-17.md`
("Art" package). Wrysk's rule: the artwork pass is a Fable thread's; the workers
of packages N (glowcap) and O (frondgrazer) implement what is written here and
make no taste calls of their own. Mocks: `art/studies/voxel-consumers/`
(`python3 consumers.py`, under a second; PNGs in `out/`, recommended first:
`recommended_px4.png`, `recommended_px8.png`, then `variants_grazer.png`,
`variants_glowcap.png`, `states.png`).

Everything below is stated against what is actually on screen: the strata and
plant palettes of `crates/cubarium/src/voxel/{present,stand}.rs`, the shading
rules of `present.rs` (`TOP_GAIN` 2.4 lit caps, `RIM`, `EDGE_DARK`, plant
`PLANT_TOP_GAIN` 1.5, `PLANT_RIM`, `CROWN_UNDER`), the chosen camera (tilt 30°,
`rise` 2 px at `s` = 4, 5 px at `s` = 8) and the appearance doc's hierarchy
(substrate dark, bodies stronger silhouettes with a readable directional core,
one warm accent for feeding). Colours are sRGB hex; the presenter mixes them in
linear light as it does everything else.

## Why these two colours and no others

The five plants already occupy the Outrun family's saturated directions:
magenta (bloomcrown, `#E04A96`), turquoise (umbrellafrond, `#33D2AE`), cyan
(springturf, `#42C5F8`), electric violet (velvetpad, `#7B5CF0`) and the one
low-chroma lilac (stonecushion, `#B9A8D6`). Every lit top face of terrain is a
bright violet (soil × 2.4 toward the cyan light), and the plants' lit caps clip
toward cyan-white or white. So a new organism cannot be told apart by another
bright saturated hue *or* by white. What is left is **value** (a dark body among
lit surfaces), **a hue the world does not contain** (green), and **treatment**
(a halo, a silhouette, a warm flash). The grazer takes the first, the glowcap the
second and third.

## Glowcap

**Ecology it must read as:** a heterotrophic fungus clustered on an identifiable
log; luminous but restrained; caps are the foliage stock, so a starving or drying
fungus loses caps first.

**Voxel decomposition.** The model gives a glowcap crown height in `[0.5, 1]`
and radius `[0.5, 1]`, so it is a **one-cell crown with no trunk**, sitting on
its support like a turf. Its support is the site holding the dead wood (a log,
below). Parts: `Part::Crown { style, heart: true }` for that one cell, drawn
by a new saprotroph branch of the plant painter (below), plus the log as a
ground decoration on the site.

**Palette.**

| Part | sRGB | Why |
| --- | --- | --- |
| gills (the crown cell's front face) | `#2C2160` | dark indigo, one step above bedrock: the cap is seen from below, its underside is not a lit surface; this is what makes a glowcap read as a cap over a stalk rather than a lit block |
| cap glow (the crown cell's top face) | `#A8FF5C` | foxfire lime. The only green in the world; luminous fungi are green in nature, which is the biosphere's "research-grounded alien flourish". Far from the springturf cyan and the umbrellafrond turquoise in hue, and it clips nowhere because the painter does **not** apply `PLANT_TOP_GAIN` to it (it is already the light) |
| wood (mycelium, never drawn; what a shed crown falls back toward) | `#3A2E5C` | keeps `CROWN_BARE` meaningful: a capless glowcap is a dark stub on the log |

`variants_glowcap.png` shows the in-family alternative (`#FF8DF0` pink): at 4 px
it is confusable with a bloomcrown crown seen small, so it is rejected. The
no-halo variant is nearly identical at 4 px and the halo is cheap, so the halo
stays as the "paid glow" the biosphere asks for.

**Cluster.** Caps are drawn on the top face only, as `2 px`-wide caps separated
by `1 px` of gills colour:

- `s` = 4: fill < 0.5 → one cap at columns 1–2; else two caps at 0–1 and 2–3
  with no gap (four pixels do not fit a gap; the two caps read through the rim
  row below them, which is lit only under a cap).
- `s` = 8: fill < 0.34 → one cap at 3–4; < 0.67 → two at 1–2 and 5–6; else three
  at 0–1, 3–4, 6–7.

where `fill = foliage / (α · wood)` exactly as `stand.rs` computes it for
plants. The front face is gills colour × 1.15 under a cap column and × 0.85
between caps, with the normal `PLANT_RIM` rim row (which, because the cap
colour is bright, gives each cap a lit lip).

**Glow treatment (the exact pixels).** After the cell is painted, the one-pixel
ring around the cap's top-face rectangle (the rectangle is `s` columns × `rise`
rows; the ring is the `2·(s + rise) + 4` pixels bordering it) is mixed toward
the cap colour by `0.35 · fill`, over whatever is already there (log, ground,
another cap). No bloom pass, nothing beyond one pixel, nothing on the front
face. At `s` = 4 this is a faint lime rim on the log's top; at `s` = 8 it reads
as a soft glow under the caps. Wilt (`μ` → 0) desaturates the cap colour toward
its luminance by up to 0.5 and darkens by up to 0.25 (the plant rule), and the
halo follows the cap colour.

**The log.** Dead wood is a per-site stock, not an object, so the presenter
draws a log wherever `Ground::dead_wood ≥ LOG_MIN` (presenter constant,
placeholder 0.05, backlog row): **one voxel high, on the support face, in the
site's own cell**, extended to the neighbouring cells in `x` while they also
hold dead wood (so several dead stands in a row become one log). Colours:

| Part | sRGB |
| --- | --- |
| bark rim (front face, row 0) | `#7A4A6E` |
| front face, rows 1…s−2 | gradient from `#7A4A6E` (row 0) to `#3A1C36` (last row): a lying cylinder, bright back, dark belly |
| cut end (the outer column of an end cell) | `#3A1C36` |
| top face | `#6E3E62`, flat, **without** the terrain's `TOP_GAIN`/cyan tint: a dead log is not lit like soil, which is what separates it from the ground at a glance |

A glowcap on a log sits at `support.y + 2` (log at `+1`); a glowcap on bare
dead-wood residue below `LOG_MIN` sits at `+1` like a turf. The log is a
`Part::Log` in `stand.rs`'s `Part` enum so the same traversal culls and hazes
it; it is opaque like a trunk.

## Frondgrazer

**Ecology it must read as:** a low, broad browser with a directional head,
deliberate cropping pauses, walking between patches; starvation visible.

**Voxel decomposition.** A body of **2 × 1 × 2 voxels**: two along `x`, one
high, two deep (the animal's site cell and the cell behind it in `z`). The
**head** is the leading cell in `x` (the direction of the last step; a fresh
animal faces `+x`). No legs, no tail cell: at 4 px they are noise, at 8 px they
are a later refinement. Parts: `Part::Body { style, head: bool, near: bool }`,
four cells, drawn by a new `animal.rs` painter through the same front/top
rectangles as a plant cell.

**Palette** (treatment A in `variants_grazer.png`; B's pink core is lost at 4
px and C's pale body collides with stonecushion):

| Part | sRGB | Why |
| --- | --- | --- |
| body, front face | `#22285C` | deep indigo-blue, darker than every lit surface and bluer than bedrock, so the animal is a **dark silhouette** on lit ground, which is the one thing no plant is |
| body, top face | `#4A5AA8` | a lit back, still well under the ground's lit violet |
| core (head accent) | `#E8FAFF` | ice: the eye. A 2 × 2 px block at `s` = 4 (2 × `rise` at `s` = 8) on the head cell's **near** cap, leading half |
| feeding flash | `#FF9B50` | the appearance doc's one warm accent, only while intake is nonzero |

**Shape.** The front face of each body cell is painted with a **pill mask**: the
outer corner pixels of the two end cells (`dx` = 0 on the trailing cell, `dx` =
s−1 on the leading cell, at `dy` = 0 and `dy` = s−1) are left unpainted, so the
silhouette is rounded. Front-face shading: per row, `mix(top, body, 0.35 +
0.65 · dy/(s−1))` (belly darker than back), end columns × 0.8. The far row's
cap gets a **spine**: its back row (`dy` = 0 of the cap) is mixed 0.18 toward
`LIGHT_SRGB`, so from the elevated view the body reads as a low back rather
than a box. The normal `PLANT_RIM` rule applies to the front top row.

**States.**

- **Resting / walking:** as above. A **step** is one support face per
  `step_period_s`, no interpolation this round; the body's four cells move
  together and the head re-aims to the step direction.
- **Cropping:** the core turns `#FF9B50` **and** the head cell's front rim row
  turns `#FF9B50` (the muzzle down on the turf). `states.png`, middle column.
  The crown being eaten loses fill through the model, so the turf visibly thins.
- **Starving:** `starve = 1 − reserve / (reserve_cap · body)` clamped; body and
  top desaturate toward luminance by `0.5 · starve` and darken by `0.3 ·
  starve`, the plant wilt rule with the darkening raised so a starving animal
  reads grey. The core never dims: identity survives hunger (appearance doc).
- **Dead:** the carrion deposit is a ground stock; draw nothing this round
  (backlog row: a carrion mark, a darker `Sprout`-like fleck in `#3A1C36`).

**Later interpolation (not this round).** When motion is smoothed, move the
whole body along the actual path between the two faces' positions, including
the height change, and round the body's origin **once** per frame; never round
the four cells independently (the appearance doc's shimmer warning). Pace: one
body length per second at cruise (`pace-in-body-lengths`), so a 2-voxel body
crosses a face in 0.5 s; cropping pauses of a few seconds are the rhythm.

## What the workers implement

**Package N (glowcap), `stand.rs`:**

1. `Part::Log` (opaque, no style), placed by `Stands::rebuild` on every site
   whose `Ground::dead_wood ≥ LOG_MIN`, at `support.y + 1`; a glowcap stand's
   crown cell goes at `+2` when its site has a log, else `+1`.
2. `palette(Species::Glowcap)` = (`#3A2E5C`, `#A8FF5C`, `#A8FF5C`); `Style`
   gains a `saprotroph: bool` (or the painter checks the species) so the
   crown painter takes the saprotroph branch: front = gills `#2C2160` × (1.15
   under a cap column, 0.85 otherwise); cap = `#A8FF5C` (no `plant_lit`) in cap
   columns per the fill table above, gills colour elsewhere; then the halo ring.
   Wilt as plants. `seed_style(Glowcap)` = the same palette (a spore bank draws
   the usual sprout mark in the cap colour).
3. `LOG_MIN` presenter constant, backlog row.
4. Presenter test: the glowcap stamps one crown cell with the lime top and a
   dark front, its cap columns follow the fill table at `s` = 4 and 8, and the
   log appears under it when dead wood is over the threshold.

**Package O (frondgrazer), new `crates/cubarium/src/voxel/animal.rs`:**

1. `Bodies` (the fauna twin of `Stands`): per animal four cells with
   `Part::Body { style, head, near }`; styles carry `(body, top, core,
   cropping: bool, starve: f32)` per animal.
2. The painter: pill mask, belly gradient, end columns × 0.8, spine on the far
   cap's back row, core on the head's near cap (leading half, `2 × rise` px),
   cropping swap (core and head rim row to `#FF9B50`), starve desaturation, the
   standard rim and haze. Bodies are stamped **after** plants in the same slab
   traversal (an animal stands in front of the turf it crops, and the model
   never puts two things in one cell).
3. GPU: one new part-id range in the voxel texture's part/style plane for
   `Body` (and one for `Log`, coordinated with N), with the style table row
   carrying the four colours and the two state scalars; the slab-walk shader
   reproduces the pill mask and the spine by `dx`/`dy` exactly as `animal.rs`
   does. The fidelity example gains a body and a log in its hand-built strip.
4. Presenter test: a body stamps four cells, the head is the leading cell in
   the facing direction, the core sits on the near head cap, and cropping and
   starving change only the pixels this note says they change.

## Open questions for Wrysk (two)

1. **Green.** Foxfire lime `#A8FF5C` is the recommendation and the only colour
   outside the Outrun family in the whole world. If you want the world to stay
   strictly in-family, the fallback is the pink `#FF8DF0` in
   `variants_glowcap.png` (middle), at the cost of reading like a small
   bloomcrown from a distance.
2. **The grazer's size.** Two voxels long is 8 px at the chosen camera and 16
   px on the panel candidate. A one-voxel grazer (4 px) would be a moving dot
   with an eye; three voxels reads as megafauna next to a one-cell turf. Two is
   the recommendation; say if you want it smaller for the shelf.
