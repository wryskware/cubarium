---
design_status: proposal
last_reviewed: 2026-09-21
decision_refs: []
source_revision: 59c2c2a (branch `edible-stock`, from main at 7dba001)
---

# The physical encounter contract, 21 September 2026

What a body at a pose on a support face can **stand on**, **reach with its mouth**,
**see** and **walk to**, written in metres and seconds, once, so that every consumer —
the tick, the seeder, the presenter and a diagnostic — asks the same question and the
discretisation is a conversion at the edge rather than a redefinition of the animal.

This is the deliverable A of `design/handoffs/voxel-edible-stock-2026-09-21.md`
(audit §10, order 0). It **implements none of it**: the decisions in
`design/handoffs/voxel-organism-decisions-2026-09-21.md` §1, §2 and §6 appear here as
the specification, and `crates/cubarium/examples/voxel_edible_stock.rs` measures the
live model against them without changing it. Every "today" row below is what the code
at the revision above actually does. Placeholder numbers stay placeholders
(`design/backlog.md`); nothing here is tuned.

Units: metres, seconds, m/s, degrees for authored angles and radians internally,
and the model's own organic / mineral / energy currencies. Organic units are **not**
kilograms and this document supplies no conversion (decisions §7).

---

## 1. Body

| Quantity | Contract | What today's code does instead |
| --- | --- | --- |
| Adult length `L` | Browser 0.375 m, shredder 0.19 m — the ladder's manifest lengths, not the anatomy document's doubled shell (decisions §1) | Browser 0.25 m, shredder 0.125 m (`crates/cubarium-voxel-fauna/src/manifest.rs:409,442`) |
| Adult width and height | The ladder's proportions: browser 6 × 3 × 3 voxels of 0.125 m, so `W = H = L/3` — 0.1875 m for the browser, 0.0633 m for the shredder (`design/art-direction/organism-scale-and-roster-2026-09-21.md:98–99`) | Width is `L/2` and there is **no height at all**: no manifest field, no consumer (`manifest.rs:177,410,443`) |
| Growth | `length ∝ (body / body_max)^(1/3)`, width and height in the adult's proportions; a newborn browser is 0.46 of adult length (decisions §1) | Geometry is constant. The manifest is a `&'static` per lineage and never reads `Animal::body`; a hatchling has an adult's footprint, mouth and eye |
| Footprint | A disc of radius `W / 2` at the standing surface | `footprint_radius = body_width_m / 2` (`body.rs:165`), which is `L/4`, not `W/2` of a stated width |
| Clearance | The body needs `ceil(H / v)` voxels of void over the face it stands on, plus whatever a lifted head adds | `headroom_voxels = 1 + mouth_reach_up_voxels` (`body.rs:597`) — the void is derived from the *mouth*, because no height exists to derive it from |
| Occupancy | The disc at the standing surface, `H` tall | One cell, `standing_y + 1`, for contact, occlusion and drawing (`body.rs:440`, `senses.rs:564`) |

**Standing surface datum.** `Site.y` indexes the **solid** support cell; its exposed
upper face — the surface a body stands on — is at `(y + 1) · v` metres
(`crates/cubarium-voxel/src/world.rs:190`, `is_support`). Every vertical anchor below is
measured from that surface, never from the support cell's base. The historical
0.1875 / 0.375 m eye elevations are offsets from the cell's *bottom* and describe a
different animal (audit §1).

---

## 2. Anchors

All heights are metres above the standing surface, all horizontal distances metres from
the footprint's edge.

| Anchor | Contract | What today's code does instead |
| --- | --- | --- |
| Eye | `0.8 × H` — 0.15 m for the adult browser (decisions §6) | `(standing_y + 1.5) · v`: **one and a half voxels**, so 0.0625 m on `small` and 0.125 m on `default`/`wide` — the same animal's eye is twice as high on a coarser grid (`senses.rs:676`, `cone_origin`) |
| Mouth band | One physical interval `[0, 1.33 × H]` over the standing surface — 0.249375 m for the adult browser, 0.0842 m for the adult shredder. No rearing, no climbing, no posture (decisions §2) | A whole-voxel layer range `standing_y + 1 ..= standing_y + 1 + mouth_reach_up_voxels`, with the up-reach converted from the 0.25 m reference grid: `[0, 0.375)` m on `small`, `[0, 0.5)` m on `default`/`wide` for the browser; `[0, v)` for the shredder (`body.rs:576,615`) |
| Horizontal reach | `0.25 × L` ahead of the footprint — 0.09375 m for the adult browser (decisions §2, "as today") | `mouth_reach_body_lengths · body_length_m` = 0.0625 m, because `L` is 0.25 m (`body.rs:517`, `manifest.rs:452`) |
| Mouth region | The swept footprint plus that reach: a capsule from the body centre to the reach tip, radius `W/2` | Five point probes — centre, tip, and the tip's forward and two lateral extremes — floored to columns (`body.rs:509`, `mouth_columns`) |
| Contact receptors | Front, left and right on the footprint's boundary at the body's own height; underside at the support face; wet at the foot | Exactly that, at layer `standing_y + 1` only, each arc probed at its centre and ±45° a hair outside the radius (`body.rs:440,482`) |
| Fan | Three sector centres −60/0/+60°, three yaw offsets −30/0/+30°, **five pitches −40/−20/0/+20/+40°**, range 2 m, aggregated into the same three sectors so the observation stays 37 inputs (decisions §6) | The same sectors and yaws with **three** pitches −20/0/+20° (`manifest.rs:369,458–461`) |

---

## 3. Encounter

One query, for a body of a stated geometry at a pose on a support face, over every
stand and pool within a radius. It answers four independent tests; a crown may pass one
and fail the other three (audit §2).

1. **Physical foliage slab.** A stand's foliage occupies the slab
   `[L_c · v, (L_c + 1) · v]` metres, where `L_c = site.y + crown_voxels(wood)`, over a
   disc of radius `crown_radius(wood) · v` metres centred on the stand's column.
   *Today:* exactly this, and it is the **whole** model — one cell-thick disc, no layers,
   no vertical distribution, and the stand's entire `foliage` is withdrawable or none of
   it is (`crates/cubarium-voxel-flora/src/lib.rs:658,665,682`; audit §2 "Current
   lollipop matrix"). Layer stocks are audit §10 order 2, not this package.
2. **Mouth.** The band intersects the slab by a positive amount, and the mouth region
   covers a cell of the disc. A touch at exactly the ceiling contributes nothing.
   *Today:* the slab's whole-voxel layer lies inside the mouth's layer range, and one of
   the five probe columns is a disc cell (`body.rs:630`, `mouth_foliage_stand` — the rule
   a bite and the taste receptor both use).
3. **Sight.** Some ray from the eye, within the fan and inside the range, reaches the
   slab with nothing nearer on it. Occlusion order: terrain, free water, another body,
   then the environment map — unchanged (`senses.rs:601`, `ray_first_hit_cell`).
   **Ground pools.** A litter, carrion or dead-wood pool has a physical height derived
   from its volume, and is an occluder only above that height; below it, a pool is not a
   wall (decisions §6). *Today:* any pool with a positive amount occludes the **whole**
   cell over its face, whatever it holds (`senses.rs:556`). The volume-to-height
   convention is **not authored anywhere** and is a backlog item; until it is, a
   diagnostic measuring the decided arm has no basis for calling a pool a wall, so it
   treats it as transparent and says so.
4. **Standing and route.** The body can stand on a face from which (2) holds, and that
   face is in the same walkable component as the body. A face is standable when it is a
   support face, its standing water is within the lineage's `wade_depth_m`, and the
   clearance of §1 is void above it (`habitat.rs:448` `browser_faces`; `step.rs:563`
   `faces_in_column`; `body.rs:602` `has_headroom`).

**The walkable component of a founder is level.** A founder's tick never writes
`site.y` (`step.rs`, `founder_act`), and each sub-step of the sweep refuses any centre
column that is not a support face at that same standing layer
(`body.rs:326`, `advance_candidate`). So a founder's neighbourhood is four-adjacent
columns **at its own height**, `x` wrapping and the strip's `z` ends walls — which is
what `browser_faces` states in as many words, and the adjacency
`crates/cubarium-voxel/src/walk.rs:24` walks the ring with. A founder cannot step up or
down by one voxel, ever; the heuristic species can, by `climb`
(`crates/cubarium-voxel-fauna/src/lib.rs`, `steppable`). This is a statement of the
implementation, not a proposal: whether a founder should be able to step is a separate
decision, and the measurements in
`design/7_Research/voxel-census-2026-09-20.md` ("Edible stock, 2026-09-21") are what
should inform it.

---

## 4. Discretisation

Stated once, applied at the consumer, never inside the body's definition.

| Physical quantity | Conversion at the consumer |
| --- | --- |
| A height above the surface | `surface_m = (standing_y + 1) · v`; a slab `[a, b]` selects the cells `floor(a/v) ..= ceil(b/v) − 1`, and a band selects a layer when it overlaps the layer's slab by a **positive** amount |
| A horizontal distance | Columns are floored: `floor(x / v)`, `x` wrapped by `rem_euclid(width)`, `z` outside `0..depth` rejected and never clamped onto a valid row |
| A disc of radius `r` metres | The cells whose centre offset satisfies `dx² + dz² ≤ (r/v)²`; the enumeration span is `floor(r/v)`, which is complete because an integer offset within the radius cannot exceed it |
| A ray | Fixed sub-step `0.25 · v` with a 64-step cap, so the marched range is `min(range, 16 · v)` — 2.0 m on `small`, 4.0 m on `default`/`wide`, against a 2 m cone range |
| An angle | Degrees authored, radians internally, `y` up |

**Invariance claim.** The same body at the same physical position on two grids of
0.125 m and 0.25 m encounters the same stands within **one cell** of error. This is a
claim about the contract, not about today's code, and today's code fails it in three
named places: the eye is anchored in voxels and not metres (§2), the mouth's up-reach is
a voxel count converted from a reference grid rather than a length (§2), and canopy
extinction divides foliage by a crown area measured in **cell²**, so halving the cell
width quarters optical depth at unchanged foliage (audit §1; `crates/cubarium-voxel-flora/src/step.rs:379`,
`area = max(π · radius_cells², 1)`). The third is audit §10 order 1's "shade-area units bug"
and is not fixed here.

---

## 5. What this package built

`crates/cubarium-voxel-fauna/src/encounter.rs` exposes the rules above as read-only
queries so a diagnostic can ask them of a face nobody stands on and of a body nobody
has: `standable_faces`, `mouth_columns_from_face` (the union over every heading),
`mouth_crown_layers_at` and `band_crown_layers`, `foliage_stands_in_layers` (the live
mouth's own scan with the layer range handed in), `crown_layer` / `crown_slab_m` /
`crown_columns`, `eye_origin_m` / `eye_above_surface_m`, `SightMap` (the live ray march,
returning the cell it struck, with the pool occluder switchable), and
`level_components`. Nothing in the tick changed: the equivalence evidence is
`crates/cubarium/tests/encounter_contract.rs` and the fauna crate's
`the_shared_scan_is_the_live_mouths_own_choice`.

The observer that measures the model against §§1–3 is
`crates/cubarium/examples/voxel_edible_stock.rs`, and its six-hour results per preset
are in `design/7_Research/voxel-census-2026-09-20.md`, "Edible stock, 2026-09-21".

## 6. What this document does not settle

Layer stocks and partial bites (audit §10 order 2); diet permissions (decisions §3);
the pool volume-to-height convention; the shade-area units; whether a founder should be
able to change its standing layer; every rate and threshold, which stay in
`design/backlog.md`. None of these lines is a canon ledger entry; Wrysk promotes them
when he wishes.
