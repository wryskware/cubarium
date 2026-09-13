---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Astra slice 2: a gentle shared breeze and one growth-art pilot

Implementation status: the proposal and review history below informed the shipped
second pass. All eleven independent Astra wind regressions now pass; failure reports
later in this document describe issues subsequently fixed. See the
[implementation record](animation-slice2-2026-09-12.md) for current behavior and limits.

Implementation proposal following Wrysk's favorable slice-1 review and instruction
to continue. This extends that reviewed image; it is not a canon promotion.
The visible result should be a few neighboring stems leaning together, rooted
trees giving slightly at their tops, and periods of rest. Native 64×64 remains
the first review size. Fable owns implementation and source artwork; Astra
provides this plan and a subsequent independent review.

## Current footing and bounded deliverable

The current renderer has `stamp_layers`, temporal interpolation through fruit
and body fades, continuous presentation time, fractional growth interpolation,
and nonoverlapping tall trunk strips. Reuse those paths. Slice-1 measurements
report about 9.86 ms for the crowded scene and 10.453 ms with all effects, against
16.7 ms; the added work has roughly six milliseconds of measured headroom, not
a new performance guarantee. The source bake still nearest-samples transformed
cutouts, so adding more atlas frames alone does not remove spatial quantization.

Deliver a lightweight bend in the existing CPU renderer, one deterministic wind
sampler in the presenter, and species response parameters. Keep ecology at 20 Hz,
output at 60 Hz, and wind derived only from `present_seconds`. No simulated
weather/force state, snapshot change, physics engine, or general rig exporter
is required. Preserve the calm palette, existing small silhouettes, and the
source artwork the user just liked.

## Wind timing and a direction that actually joins across faces

Use one shared, slowly varying scalar gust strength, with a small spatial phase
term from the existing embedded position. Evaluate it once per plant root or
column per frame, never per destination pixel. Candidate envelope: a roughly
30-second packet with 4–6 seconds easing in, 6–10 seconds of mild movement,
4–6 seconds easing out, and the remaining interval exactly quiet. Use smooth
endpoint slopes. A longer secondary period may modulate amplitude slightly to
avoid an obvious metronome. During a quiet interval, the added wind is exactly
zero; existing creature motion and subdued bioluminescence can continue.

Do not seed independent face phases or normalize a nearly zero direction.
For a first seam-compatible circulation, let `a=u/32-1`, `b=v/32-1` in the
existing face charts. On every side face choose chart vector
`W = (-(1-a*a), 0)`. On Top choose
`W = (-b*(1-a*a), a*(1-b*b))`. Multiply by the shared scalar and leave the vector
unnormalized. Its values match after the existing tangent transport at all
side/top edges; side/side edges and top vertices have zero vector, and Top's
center is also calm. These bounded calm regions are intentional. They avoid a
singularity and are preferable to a chart-direction jump. Verify the claim
against the actual seam helpers rather than copying a second seam table.

A naive 3D swirl `(-Z,0,X)` projected independently onto each face does *not*
meet that condition: on Top it retains an edge-normal component absent on the
adjacent side. The polynomial field above deliberately removes that mismatch.
For the scalar's optional spatial variation, use embedded coordinates directly
with a small coefficient; do not introduce an angular branch cut.

For side-face plants, project the chart wind onto the authored tile's horizontal
axis (`slot.heading`), keeping the existing orientation jitter. For the radial
canopy species, a restrained rotation about the stationary center is a bounded
first response to this circulation, around 1–2 degrees. Do not translate whole
canopy plants. Keep the canopy response small enough that their baked sway and
the added motion do not combine into constant spinning.

## One rooted bend, shared by all pieces of a column

Add an optional deformation argument or sibling API around `stamp_layers`.
Default deformation is identity, with a fast path preserving the current image.
The first deformation is a horizontal row displacement: in unscaled sprite
coordinates, forward mapping is `x' = x + D(H)`, `y' = y`. Its inverse is exact:
sample `x = x' - D(H)`, `y = y'`. There is no iterative solve, fold, or vertical
rescaling. Mix all clip samples as now and composite once. Evaluate growth masks
in the original, undeformed material coordinates so growth remains attached to
the plant. This deformation preserves vertical row ownership.

Use a smooth bounded bend profile with zero value and derivative at the root,
for example `D(H) = A * smoothstep(clamp(H/L,0,1))`, where `A` already contains
wind strength and species response. `L` is an authored fixed mature bend length,
not the changing current growth height: otherwise existing lower trunk pixels
would slide whenever a new segment grew. Zero or negative height stays fixed.
For a small side plant, the root is the actual shared stage contact around tile
row 15; leave its lowest one or two painted root rows still. Its effective `L`
is roughly the root-to-tip distance. Inspect lanternstalk first, then tendrilfan
and reedspire; low soil mats may receive no visible bend.

For a tall column, use one root, one wind sample, one stiffness, and one global
height coordinate for base, every trunk strip, cap, and vine. For tile index `i`
and tile coordinate `y`, the existing geometry gives `H = 4*i + 8 - y` pixels
above the horizon anchor. Use that same `D(H)` everywhere, including the cap's
fractional index `height + 1`. Suggested mature length is about 48 pixels.
Keep the current nonoverlapping `Mask::Strip` convention; independent per-tile
rotation or independent phases would reopen joins. A vine on a tree shares its
supporting column's displacement; do not let it slide against the trunk.

The cap can therefore move differently across its own rows while remaining on
the same curve as the stem. Apply the bend in the parent column's unfolded
chart, even to pixels owned by Top. Recomputing a canopy wind response for the
Top fragment of one crown would split a single plant's motion at the seam.

## Extent budgeting is part of the design

The nine-pixel local-surface radius remains a hard bound for *each stamp*. A
new displaced texel needs a larger footprint; merely changing the source sample
while unfolding the old radius clips the moving tip. A simple safe initial
bound is `max_participating_pose_extent * scale + max_abs_displacement`. Apply
the established bilinear-support convention consistently. Masking a tile does
not automatically shrink its declared footprint.

Before enabling wind, calculate and report a stable amplitude budget for every
species across all its stage, fruit, and optional growth samples. Bound the
requested amplitude by the minimum verified headroom of that entire family,
including intermediate pose blends. A column's parts use a common budget so
they cannot bend by different amounts. Store/tune these budgets explicitly;
do not vary them frame by frame with the current sample extent. Never respond
to a windy over-budget pose by skipping the sprite, scaling the whole plant,
or silently accepting a clipped footprint. Nonfinite wind takes the identity
path. The zero-wind image must remain unchanged.

Starting desired peak tip travel, before those measured limits: lanternstalk
0.35–0.5 px, tendrilfan/reedspire 0.5–0.7 px, glowcap/rootveil 0–0.15 px;
spiretree crown 0.8–1.0 px, glasscane 0.4–0.6 px. Treat these as artistic
starting points, not promises. Compare actual achievable movement at native
resolution. If conservative bounds leave an asset effectively motionless,
record it and make a small targeted source-art inset at the limiting tip or
leaf if worthwhile; preserve the overall plant's size and identity.

One optional optimization if tall headroom is too small: recenter a stamp on
`D(H_at_tile_center)` and bound only the residual bend within its tile. Move
that anchor with `travel` and transport its heading with the returned tangent
map, then include the anchor displacement in the inverse sample transform so
the actual root remains fixed. This substantially reduces local radius growth
for a long gently bending column, but requires separate seam/root proofs. Do
not add it unless measured headroom blocks a readable simpler implementation.

Species stiffness should primarily scale displacement, with at most a small
response lag such as 0–0.2 seconds for soft heads. All structural parts of one
plant share the same lag. Fixed per-plant amplitude variation around ±10% can
keep a patch organic; large independent phase offsets would destroy the shared
breeze. Existing authored sway can remain at first, but reduce excessive sway
tracks in the pilot if their combined tip travel defeats quietness or the bound.

## A small authored-growth pilot fits without exporting general rigs

Include lanternstalk `stage0→stage1` and, if the first reads well, `stage1→stage2`
as optional nonlooping baked growth clips. Extend pack metadata with named
transition rows and durations while keeping old packs/fallback masks supported.
Use the existing Sprite2D and AnimationPlayer authoring contract: fixed root,
stalk extending upward, head enlarging/unfolding after its support appears.
Author checked-in scenes directly; the one-time Python generators are not the
current source of truth. No droplet/nibble work is needed for this pilot.

Map the presenter's existing continuous stage progress onto the growth clip;
reverse it when growth reverses, and retain the existing target and fruit rules.
Wind is applied after the growth pose is sampled, so it naturally continues
during growth. A single growth clip need not encode every sway phase. To avoid
an endpoint cut to/from a differently phased idle loop, use short normalized
layer blends near the first and last 10–15% of progress: the actual source-stage
loop at the beginning, the growth clip through the middle, the actual target
loop at the end. `stamp_layers` already supports that. Preserve full endpoint
images and the same progress on reversal; do not restart the growth clip on a
wind packet or frame draw. Compare against the existing reveal on a sparse
fixture before extending the convention to every plant.

If a nearest-sampled thin stalk still visibly jumps in the pilot, targeted
offline coverage sampling in `bake.gd` can be assessed there. A whole-atlas
antialiasing/style change is not required for the wind pass.

## Acceptance evidence and implementation order

First implement the wind sampler and zero-cost identity path; next prove the
rooted bend and bounds on a few still sprites; next wire small plants and the
single shared tall-column curve; then assess the lanternstalk growth pilot.
Keep Fable's work bounded and await every delegated worker before integration.

Independent checks should cover identity at zero wind, root pixel invariance,
smooth positive/negative gust endpoints, identical output for the same simulated
time across 30/60/120 Hz histories, and no ecology/snapshot changes. Sweep every
actual pose through its maximum positive/negative wind and combined growth/fruit
states, with seam and vertex placements, checking support is inside the declared
radius and no stamp disappears. A synthetic striped column is useful for proving
trunk/crown/vine registration at fractional heights and wind extrema. Test the
wind vector on both sides of every seam after transport, including its explicitly
zero vertices and quiet intervals.

Capture a sparse native-resolution 30–40 second loop containing a quiet interval
and a full mild gust, plus a short dense-world/crown-seam view and the growth
pilot. Include enlarged nearest-neighbor versions for inspection. Re-run the
existing crowded and all-effects release draw timings; aim to keep this pass
near 12 ms on the same machine and strictly under 16.7 ms for those fixtures.
If expensive per-pixel trigonometry or fresh allocation appears in the hot path,
move it into per-root/frame parameters before reducing visual quality.

The current shim contract was read in `/home/wrysk/vuzic/led-cube-shim/README.md`
and `docs/ARCHITECTURE.md`: five 64×64 RGB buffers, Front/Right/Back/Left/Top order,
with hardware mapping, yaw and physical Top correction owned by the shim.
Reuse Cubarium's existing `face_frame`, `SurfacePoint::embed`, `travel`, and
`unfold_pixels` extensions of that contract. The higher-resolution/LCD idea
remains useful future context; this slice does not alter its transport boundary.

## Lanternstalk source advisory for Fable

The checked-in `art/plants/lanternstalk.tscn` is a feasible small growth pilot.
Sprout, Stalk1, Stalk2, and Fruit already share root `(0.5, 7)` in scene units,
equivalent to tile `(8.5, 15)`. Their lowest painted row is tile row 14. Preserve
that contact and the existing `Stalk2`/`Fruit` geometry relationship. The sprout
is 3×3; the middle stalk is 3×5 with a 5×5 bulb; the mature stalk is 5×9 with a
7×7 bulb. Middle and mature stalk sprites are positioned relative to the root
at `(0,-2.5)` and `(0,-4.5)`, while bulb centers are `(0,-6.5)` and `(0,-9.5)`.
The bulb deliberately overlaps the stalk top, by one row in stage 1 and three
rows in stage 2. Preserve contact during extension rather than moving the bulb
on an unrelated track and briefly exposing a gap.

Extend the stem first, then open/enlarge the bulb. If scaling a stem part, scale
about its bottom: its negative center offset must change with its vertical scale
so its lower edge stays at the root. Scaling the whole stage group would squash
or enlarge its bulb too; scaling the centered stalk Sprite2D without moving it
would move the root. The mature stalk also has a different root flare and width,
so merely stretching the middle stalk cannot reproduce the final art. Blend to
the actual mature part near the end or author a few intermediate shapes.

The current RESET covers visibility, rotation, and modulation, but has no need
to reset scale or position. Any new growth tracks touching those properties must
add their neutral values to RESET, including new child groups. Otherwise the
existing bake's RESET-before-each-sample loop can retain growth transforms and
contaminate later stage/fruit samples. Zero scale is singular and `raster` skips
that part; use deliberate initial visibility/opacity rather than depending on
a nearly singular transform to make a first bud appear smoothly.

Both endpoint shape and endpoint light need review: stage 0 pulses its whole
sprite between 0.7 and 1, whereas stage 1/2 pulse only the bulb. The new growth
clip's neutral modulation will not match an arbitrary running stage phase;
the endpoint layer blends described above are needed for brightness as well
as sway continuity. Growth rows must be sampled inclusively with `i/(n-1)` and
marked nonlooping; the current plant loop's `i/n` sampling omits the endpoint
and rejects nonloop clips. Keep new transition rows on a separate bake branch.
Check the first/last growth frames against their intended neutral-stage images,
then the rendered entry/exit against moving, pulsing stages with wind enabled.
Fruit stays a separate real-resource signal, not an unconditional final frame
of the growth sequence.

## Critical review of Fable's slice-2 work brief

Reviewed `animation-slice2-brief-2026-09-12.md` before its public wind types landed.
These are corrections to the proposed math, for workers A/B and the integrator:

1. **A bent texel center does not bound its bent filter support.** The brief's
   `hypot(displaced_center) + 1.207` headroom omits the variation of `D` across
   the source texel's bilinear support in the vertical direction. The support
   shears as it bends. Use the original conservative `extent + |amplitude|`
   bound, or derive a tighter bound over the *whole* filtered support. Under
   the existing radial-support convention, a safe additional allowance follows
   from `max |D'| = 1.5*|amplitude|/length`: a support radius `R` around a texel
   center can grow to at most `R*(1 + max|D'|)`, by the triangle inequality.
   An explicit bound over source-support rectangles can be tighter. Validate
   against actual sampled support at rotated/subpixel placements, not merely
   by rechecking texel centers with the same headroom formula.

2. **Use one consistent admission and unfolding bound.** A tight per-texel
   budget may permit `extent + |amplitude| > 9`; consequently the brief's
   proposed assertion `extent + |amplitude| <= 9` is incompatible with it.
   `min(9, extent + |amplitude|)` is not a proof that no support was clipped.
   Either use the conservative bound for both admission and unfolding, or
   expose the rigorously bounded bent radius and use that same quantity for
   both. Always retain the identity path and reject invalid input explicitly;
   never let an accepted windy pose disappear because a different later bound
   silently rejects it.

3. **Masks use material coordinates.** With destination tile point `p`, the
   source/material point is `q=(p.x-D,p.y)`. Apply the mask at `q`. Using `p`
   is equivalent for axial/strip masks, but not radial masks: their radius uses
   x as well as y. A radial reveal otherwise cuts the bent object against a
   stationary circle and no longer follows its original material. Keep this
   invariant even if the first caller only bends side-face axial plants.

4. **Budget the final amplitude, including all multipliers.** A family tip
   budget capped to headroom and subsequently multiplied by up to 1.1 slot
   variation can exceed that headroom. The proposed ±15% peak modulation also
   conflicts with `wind_strength`'s promised `[0,1]` range unless normalized.
   Include these maxima when constructing a stable family budget, and bound
   actual projected wind magnitude before claiming its amplitude is admitted.
   Columns must include the cap's continuous base index throughout growth,
   not just integer trunk indices. For the stated monotonically increasing
   bend profile a maximum-height envelope can bound that continuous interval;
   state the reasoning and include growth endpoints in the verification.

5. **Flutter must not introduce jumps at hold boundaries.** The brief places
   sinusoidal flutter on the hold, while rise/fall ease to/from one. A plain
   piecewise implementation therefore jumps at seconds 5 and 13 whenever the
   flutter value is not one. Construct one continuous packet envelope and
   multiply continuous bounded modulation through its entire active interval,
   or window the hold flutter to zero modulation at both hold boundaries.
   Check all four packet boundaries, not only entry to and exit from quiet.

The scalar spatial delay shifts the quiet interval slightly by location, but
with the stated small phase/lag bounds there remains a long shared quiet
interval. Keep tests explicit about whether they check the global sampler or
the delayed per-root sampler. No production edits accompanied this review.

### Independent renderer regressions against landed code

`crates/cubarium-render/tests/astra_wind_regressions.rs` contains four independent
observable checks. Its first isolated run passed zero-wind bit identity and
fixed root rows, and failed two concrete cases:

- A constant one-pixel bend did not translate a radial material mask with its
  sprite: the mask was evaluated before inverse x displacement.
- A valid 1×1 white sprite with pivot `(0.5,8.2)` received headroom
  `2.3993205036665652` for root 0, length 1, base 0. A known 4%-coverage bilinear
  tail forward-mapped past radius 9 and vanished in the draw. The fixture aligns
  a real destination pixel with that tail and observes black instead of 0.04;
  it does not copy the headroom calculation.

Command: `cargo test -p cubarium-render --test astra_wind_regressions`. These
results describe an in-progress implementation, not the final slice. Production
ownership remains with Fable's worker; rerun the target after the corrections.

Follow-up against the landed fixes: the original four renderer checks now pass.
A fifth, `admitted_headroom_preserves_filter_corners_under_a_constant_bend`,
still fails. It uses a valid white texel centered at `(0,5)` relative to its
pivot and constant displacement throughout its support. Its known 1%-coverage
corner at source `(0.9,5.9)` is **inside the original sprite extent**, but the
admitted amplitude `5.977389456930901` moves it past radius 9 and it disappears.
Sampling `D` one pixel above the texel fixes the varying-displacement issue,
but the unchanged 1.207 margin still does not bound the square corner of the
bilinear kernel. This fixture needs no larger-radius helper or geometry change.
The actual current `cubarium_surface::MAX_LOCAL_RADIUS` is 32; the independent
render stamp bound remains 9 and should remain so.

`crates/cubarium/tests/astra_wind_regressions.rs` now contains six host checks.
Its isolated first executable run passes gust bounds/shared quiet, all packet
boundaries including flutter hold boundaries, seam agreement through real
`travel` tangent maps, and final amplitude budgeting including slot variation.
It reproduces two remaining issues: `plant_bend_budget` omits a synthetic wider
growth-transition frame, and actual lanternstalk root row 14 moves under wind
(`PLANT_BEND_ROOT=1` does not hold its center at height 1.5). Command:
`cargo test -p cubarium --test astra_wind_regressions`. The test follows the
current two-argument `effective_tip` API and applies the public slot-variation
range afterward. All results remain provisional during implementation.
