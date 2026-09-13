---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw: bounded production rendering

Astra implementation advisory for Fable 5.1. Wrysk prefers Lanternjaw's visual
direction and authorizes continued implementation; the geometry and sequencing
below are recommendations, not new canon or approval of a predator ecology.
Reviewed the current source before production work on this package. No live
state, process, transport, source art or renderer was changed for this review.

## First deliverable

Build a reusable multipart body in the production renderer, exercised by an
isolated scripted capture route. Preserve Fable's long low hull, separated cyan
lantern chain, tail fan and folded weapons. Do not shrink the entire creature to
fit one existing stamp. Keep scripted rest/move/strike/bud studies distinct from
real hunting, meals, gestation or offspring. Do not introduce a fifth default
form or alter saved genomes' fallback appearance as a side effect.

## Geometry: one body chart, several bounded materials

The current sprite stamp's nine-pixel limit and the surface query limit are
different: `Sprite::from_rgba`/`stamp_layers` enforce nine; surface
`MAX_LOCAL_RADIUS` is 32. The sprite comment implying the surface is only proven
for nine is inaccurate. Do not raise the sprite limit or use the hidden
larger-radius test helper as the production solution.

Recommended implementation:

1. Sample one root anchor and tangent heading through the existing movement
   interpolation. Sample all articulation at that same simulated instant.
2. Express each part's pivot, rotation, scale and any attached joint in the root's
   body-local chart. Compose parent-to-child transforms there, before projection.
   A claw follows its elbow, which follows its shoulder; it never independently
   interpolates between face coordinates.
3. Compute a conservative root query radius from the transformed, filtered
   support of every participating part. For rigid parts a sufficient bound is
   `max(length(part_offset) + part_support * part_scale)`. Account for lunge,
   articulation, glow and every participating transition pose. Assert finite
   bounds and a result at most 32; reject invalid assets/configuration visibly
   during loading or tests, not by silently dropping art at draw time.
4. Call `unfold_pixels(root, radius, scratch)` once for this body. For each returned
   destination pixel, turn `pixel.local - root.chart()` into body coordinates,
   inverse-transform into each relevant part and sample it. Every part uses that
   SAME root-owned chart image. Keep a cheap part-local bounds rejection before
   sampling. Reuse allocations and precompute static support.
5. Resolve authored material ownership/depth, then source-over the resulting body
   sample onto the world. Temporal pose/state mixtures belong inside that
   compositing step, not as two partially opaque whole-body redraws.

This needs a narrow multipart sampling/compositing entry point in the renderer;
calling public `stamp_sprite` independently for every shifted part is not
equivalent. Each part's local filtered support must still fit nine pixels. The
group radius is an enumeration bound, not permission for an oversized material.
For new part validation, bound the full bilinear support: a painted texel can
contribute within one pixel of its center on EACH axis. The existing radial
`TEXEL_SUPPORT` approximation is not an exact corner bound. Validate rotated,
fractional poses against a deliberately generous valid query, not just atlas
texel centers or `Sprite::extent()` alone. A full filter-tail bound need not force
smaller art: place pivots and split materials appropriately.

Why shared ownership matters: `unfold_pixels` selects the shortest valid chart
image independently for its anchor. Two individually correct stamps can select
different images around a top vertex and tear a joint or double shared coverage.
A single root-owned query gives one coordinate interpretation to all parts.
It retains the existing documented localized cut at a cube vertex: a flat rigid
rig cannot wrap positive curvature without some distortion/cut. Promise common
ownership and deterministic cuts, not globally perfect rigid wrapping. Ordinary
edge crossings should remain continuous; capture the vertex cases explicitly.

At the open bottom, movement's `travel` reflects the ROOT trajectory and maps
its heading. Static artwork should clip at that open boundary, as an ordinary
sprite does. Do not `travel` every attachment offset: its rim reflection folds
out-of-bounds claws/tail pixels back onto the body. The shared root chart query
naturally has no bottom-face image. Preserve the presenter's existing root-turn
behavior; changing ecological rim steering is outside this rendering package.

## Material partition and silhouette

The study's painted hull runs from x=-9 to x=+8; the extended claw reaches x=12.3
before lunge/filter support. Its authored columns are the design reference, not
a single 16×16 tile waiting to be resized. A practical partition is rear/tail,
middle plates, head, and independently articulated forelimb segments. Place
splits at the authored dark plate divisions; choose pivots near each piece's
painted center. Walking legs attach to their actual body columns, not the root.
Three hull pieces are a starting point, not a required asset count: retain more
column-level articulation if three rigid pieces erase Fable's traveling wave.

Use the source template/semantic parts directly or bake their material frames;
do not rasterize a large already-composited image and crop it every live frame.
Do not add a general skeletal exporter unless this narrow rig actually needs it.

Distinguish two forms of overlap:

- A cut through ONE continuous material is ownership, not translucent layering.
  Partition source texels once, preserve their common sampling lattice, and
  reconstruct that material's premultiplied coverage before source-over. Two
  separately filtered halves source-overed onto one another can leave a dark or
  translucent cut; overlapping duplicate texels can create a bright ridge.
  Test the split flat material against an unsplit reference.
- Genuinely separate depths may overlap intentionally. Preserve the study order:
  far arm, walking legs/cocoon, hull and lanterns, near arm. Represent that ordering
  explicitly. Do not add shared alpha twice merely because a joint belongs to
  both segments; give its cap to one owner or combine the limb coverage first.

Fable's study uses opaque colors mixed toward `#0B0525` to suggest translucency.
That is not actual transparency and can show dark background-colored blocks over
water/soil. Preserve dark opaque shell/seams where artistically intended; author
real alpha for halo/fan translucency rather than attempting an ambiguous inverse
conversion of every dark pixel. Convert source sRGB through the renderer's
existing linear-premultiplied path exactly once. Compare black, study-purple,
soil and water backgrounds. Keep the cyan sockets separate at 1×; glow should
not turn the hull into a continuous bright sausage. Keep forearms dark at rest,
with their brief strike accent supplying the readable surprise.

## Animation without inherited stepping

Keep the study's silhouette and timing intent, not its rounding. `fable.js`
rounds the root, per-column displacement, line points and feet; gait lift is a
binary threshold. Carry continuous joint/column transforms into production and
use eased foot lift/planting. Do not try to solve this only with more atlas frames
whose source geometry has already snapped to whole pixels.

Use the presenter's simulated fractional clock (`present_seconds`) and existing
heading interpolation. Draw frequency must not advance a strike, blink or gait.
Keep deterministic individual phase without deriving phase from wall time.
Preserve calm rest: a small traveling body wave and quiet lantern pulse; stronger
tail wave during locomotion. The current study's 6-second hunt cycle and short
coil/release/recoil are suitable EXPLICIT study controls only. Later real attacks
must be event-triggered with their own progress, not periodic implied successes.
The existing 45-ms far-arm lag is elapsed simulated time, not "1.5 frames".

Blend state changes without detaching joint chains or ghosting two full opaque
bodies. If using pose-image mixtures rather than joint interpolation, compose
each state's complete depth-ordered body sample first, then premultiplied-mix
those samples and source-over once. Hold exact endpoints, allow interruption
from the currently displayed pose, and do not reset phases on repeated observe
or draw. A study cocoon is not evidence of a biological birth.

## Acceptance checks before handoff

- Flat-face ownership oracle: split an opaque and a translucent test hull; compare
  with the unsplit material at integer/subpixel anchors and diagonal headings.
  Shared joints must neither reveal extra background nor gain opacity. Separately
  verify intentional front-over-back arm ordering.
- Bounds: every material/pose/filter corner within its own nine-pixel support;
  group query within 32 for rest, maximal body wave, compression, full strike,
  recoil and bud. Include interpolated extrema, not only keyframes. Compare the
  computed query with a larger legal reference query to detect lost support.
- Topology: asymmetric marked hull/joints across every side-side and top-side
  edge, all four top vertices approached from each incident face, and every
  bottom rim. Exercise fractional anchors and headings toward/across/along each
  edge. Each physical pixel has one root chart owner. Joints share that owner;
  no unexpected duplicate light, independently reflected appendage or wrong
  orientation. Record the localized vertex-cut appearance, not merely a hash.
- Timing: repeated draw at one `(tick,f)` is identical; 30/60/120-Hz draw schedules
  agree at common instants; tick-zero hold, pause/resume and state interruption
  preserve endpoints. Inspect 60-fps native output for body/foot/claw stepping.
- Compatibility: normal existing organisms render unchanged when this body is
  not selected; saved out-of-range `form` values retain their former fallback.
  An isolated selector must not append a form to the default pack count. Test
  that the capture route does not mutate snapshots or create founders.
- Delivery: production-rendered native-resolution stills and normal-speed clips
  showing rest/move/strike, a common body alongside for scale, topology cases and
  varied backgrounds; nearest-neighbor enlargement is supplementary. Measure
  one/two-body incremental cost and total dense-world frame time in release,
  including vertex-heavy cases. Preserve the 16.7-ms target; report method and
  distribution, not only one favorable frame. Hardware legibility remains a
  separate observation, not established by a desktop capture.

## Next authored plant growth priorities

1. Finish lanternstalk 1→2: stalk extension/flare before the recognizably round
   bulb opens. Keep its bottom contact fixed. Match both idle endpoints and their
   current sway using the existing endpoint blend; a static authored endpoint
   cannot exactly match every arbitrary idle phase without that blend.
2. Side species through the existing optional `grow<from><to>` mechanism:
   glowcap stem then cap opening; rootveil anchored root spread then unfurling;
   tendrilfan unfolding lobes around fixed attachment; reedspire narrow shoot
   extending before blades separate. Favor an observable silhouette sequence over
   uniform scaling or revealing a complete mature drawing. Keep fruit distinct
   from maturity and immediately subject to the real fruit gate.
3. Canopy opening around a fixed center if the package permits; do not reuse a
   side-facing upward-growth convention on radial top art. A flooded top-face
   reed's small rotation response is separable from a new growth convention.

For each authored transition: nonlooping exact endpoints, fixed painted contact,
continuous reversal, no repeated-observe restart, unchanged established-world
startup, all transition frames included in wind headroom, nine-pixel filter
support and reproducible bake. Inspect actual exported strips at 1×. Use the
mask fallback for unfinished species; say which ones remain. Larger tree wind,
coverage-AA comparison and interaction details stay separate follow-ups rather
than blockers to these authored silhouettes. Droplets/nibbles remain deferred.

## Evidence and scope

Lore retrieval located the candidate rationale and integration cautions; verified
against [Fable's study](../../art/studies/megafauna/fable.js),
[its rationale](../../art/studies/megafauna/fable.md),
[integration notes](../../art/studies/megafauna/integration-notes.md),
[animation roadmap](../animation-roadmap.md),
[sprite sampling](../../crates/cubarium-render/src/sprite.rs),
[pixel ownership](../../crates/cubarium-surface/src/raster.rs),
[surface unfolding](../../crates/cubarium-surface/src/unfold.rs),
[travel](../../crates/cubarium-surface/src/travel.rs), and
[presenter](../../crates/cubarium/src/art_present.rs).
Consulted canon/ledger and the current sibling shim's
`docs/ARCHITECTURE.md` face/protocol contract: reuse its face conventions and
geometry; no hardware mapping or output changes are proposed.
The roadmap's historical statement that optional care is unimplemented is stale
relative to current README; it does not limit or describe this package's status.
