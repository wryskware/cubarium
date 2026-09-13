---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Astra: smooth motion and readable growth

Implementation proposal for the work Wrysk requested, not a new canonical decision.
Priorities are continuous existing motion, readable sprout → stalk → height →
bud/flower development, then wind. Judge the first slice at native 64×64; retain
fine details for the web viewer and a possible future higher-resolution/LCD cube.

## What the current code actually does

- `crates/cubarium/src/clock.rs` already runs ecology at 20 Hz and output at 60 Hz.
  Body travel interpolates the last completed tick, deliberately one tick behind.
- `art/bake.gd` samples plant and tall clips only four times. Their three-second
  loops therefore hold each pose for 750 ms. `art.rs::Clip::at` selects one frame.
  Ground texture similarly holds four samples over six seconds.
- `art_present.rs::draw_with_fruit` uses integer-tick animation time except for
  rain. Rain then floors its position and uses a boolean canopy sparkle.
- `next_stage` and `next_tall` resolve all thresholds in one call; sufficient
  biomass can produce a mature plant or complete trunk column immediately.
  Both `observe` and `draw` apply them. That is safe only because they currently
  are idempotent, and must change before adding time-dependent growth.
- The renderer samples linear premultiplied RGBA and delegates topology to
  `unfold_pixels`. Each individual stamp, including filtering, must fit within
  the existing nine-pixel radius. Tall plants achieve height through many stamps.

Sources inspected: the functions above, `crates/cubarium-render/src/sprite.rs`,
`art/plants/author.py`, `art/PLANTS.md`, and the current shim README and
`docs/ARCHITECTURE.md`. The shim still owns five 64×64 RGB buffers and hardware
mapping; this proposal does not require a transport change.

## Recommended first implementation

Keep the existing Godot source rigs and Rust presentation path. Bake denser
samples and interpolate adjacent samples in the renderer. Start with 24–48
samples for a three-second plant loop and inspect moving outlines before
settling the count. Four-frame interpolation alone can turn movement into a
long dissolve between visibly different silhouettes. Dense samples keep the
displacement small; interpolation fills the remaining temporal gaps. Creature
loops should receive the same treatment, while gestation remains driven by
actual gestation progress and nonlooping clips clamp at their final sample.

Implement a sampled-pose descriptor `(first, second, weight)` and a stamp path
that samples both sprites, mixes their linear premultiplied RGBA, and performs
one source-over operation. Two ordinary partially opaque stamps are not the
same operation: at a midpoint their shared opaque pixels expose 25% of the
background. Use the maximum endpoint extent and preserve unique seam/vertex
ownership. This needs neither per-frame sprite allocation nor a second surface
unfold. Keep a direct single-sprite fast path at exact samples.

Choose one presentation-time helper and use it for looping clips, water shimmer,
ground breathing, and rain. For alignment with body travel, prefer
`(tick.saturating_sub(1) + clamp(f, 0, 1)) * DT`, with tick zero held at zero;
document the choice because existing rain uses `tick + f`. Verify continuity at
the tick boundary and hold the supplied fraction on pause. Do not advance from
wall time inside a draw call. Fractional rain coverage and a smooth short
sparkle envelope can remove integer stepping without introducing new weather.

Runtime part rigs are a useful later option for directional wind and richer
artwork. They require a new exported part/track contract, root transforms,
cross-part ordering, and more seam-safe stamps. That work is worthwhile once
wind needs coherent deformation, but denser baking makes existing art reviewable
sooner and retains the source rigs for that extension.

## Growth needs time and an anchored silhouette

Keep field thresholds and hysteresis as the target morphology. Add renderer
state for visual growth: previous/current progress, target, and last observed
tick. Update once per distinct tick; draw only interpolates the two stored
values. Repeated `observe`, repeated draws, and 30/60/120 Hz output must leave
the same visual growth state. Missing ticks and backward/replaced snapshots
need an explicit initialization/reset path rather than accidental catch-up.

For a new plant, show a sprout before extending its stalk, then open the larger
leaf/bulb silhouette. Candidate review timings are roughly 1–2 simulated
seconds per small-plant transition and 1–2 pixels/second of tall extension.
These are art tuning values, not ecological growth rates. Each transition
should keep its ground contact fixed. A whole-sprite scale about the center
looks like floating; a long dissolve between complete stages looks like a
replacement. Prefer authored growth poses with a stable base: extend a narrow
stalk, unfold leaves, enlarge the head, then reveal the bud/flower accent.
Fable can first author lanternstalk and one tall species as the review examples,
then apply the same convention to the other existing plants.

Tall columns should track continuous height in pixels. Keep completed trunk
segments fixed, reveal the newest segment from its lower end, and move the
crown along the same continuous height. Do not scale the entire column or jump
the crown four pixels when a segment threshold changes. Trunk and vine parts
must share column phase so their overlapping periodic texture does not shear.
Use the surface travel/transport helpers when moving an anchor across a face
edge; do not invent face-local wrap rules. Each part retains the nine-pixel
stamp budget even though the assembled column is much taller.

Fruit imagery must remain conditional on actual fruit. Start its appearance
only while the field supports it; remove the food accent promptly when fruit
vanishes, even if a decorative leaf-opening transition is still running.
Plant growth here visualizes field change; it does not create simulated plants,
food, delayed resource accessibility, or a new snapshot schema.

For startup or loading an existing rich world, initialize visual growth from
that snapshot's target so every launch does not regrow a forest. Subsequent
new growth is rate limited. This preserves ecological replay but does not make
the exact in-flight visual transition survive restart. Record that limitation;
persisted visual growth would be a separate change if exact image replay is
needed. A scripted bare-to-rich fixture is the way to inspect all growth stages.

## Concrete validation and retained ideas

The review artifact should include a native-resolution 60 fps loop of sparse
plants and all creature modes, a bare-to-rich growth sequence, fruit appearing
and disappearing, and a crown at a side/top seam. Provide nearest-neighbor
enlargements alongside native frames so both readability and errors are visible.
Show the final dense world too: smooth motion can still become visual noise.

Meaningful checks: identical opaque endpoints retain opacity through blending;
loop wrap and nonloop endpoints are continuous/correct; repeated draws do not
advance growth; render rate does not affect growth; resource loss cancels fruit;
seam/vertex ownership survives fractional motion; all baked intermediate frames
fit the extent budget; bake output is reproducible. Measure representative
release rendering before/after, including a crowded rainy scene, against the
16.7 ms output-frame budget. A desktop capture cannot verify the physical cube.

After that review, coherent wind is the next art/system experiment: a shared
surface direction with height-dependent stem bend, gentle branch lag, and
species-specific stiffness. Keep roots planted and avoid independent segment
wiggles. Retain droplets on leaves, localized nibbling, leaf scars, dew trails,
flower opening detail, and leaf undersides in the ideas backlog. Their possible
value on the web viewer or an LCD cube is real, but their 64-pixel readability
does not justify putting them ahead of continuous silhouettes and visible growth.

## Bake and source follow-up for Fable

Denser time samples alone retain spatial quantization: `bake.gd::raster` inverse-
transforms one output pixel center and floors its source UV. A slowly rotating
one-pixel stem can therefore hold the same pixels through many baked frames,
then change suddenly; temporal blending only spreads that jump across the last
sample interval. Prefer offline coverage sampling (for example a fixed 4×4
subpixel grid), compositing transformed cutouts at each sample and resolving
linear premultiplied color before PNG encoding. This preserves the hard-edged
source artwork while recording fractional coverage, rather than applying a
general blur on top of the runtime's existing bilinear filter. It increases
bake time, not runtime samples. Check all resulting nonzero-alpha texels against
the nine-pixel extent: new antialias coverage can expose previously hidden
over-budget edges. Inset the offending artwork rather than relaxing geometry.

The actual lanternstalk scene registers Sprout, Stalk1, Stalk2, and Fruit at the
same `(0.5, 7)` root pivot, a useful growth invariant. Whole-stalk rotation still
rotates the painted root edges around that point. Keep the lowest one or two
root rows in a stationary part if contact visibly crawls, and articulate above
them. Its rotation tracks use linear quarter-cycle keys with values such as
`0, 0.10, 0, -0.10, 0`; denser sampling preserves their sharp velocity reversal.
Use smooth authored easing or sine-sampled sway, with bounded extrema, and keep
opening/growth tracks distinct from repeating sway. Edit checked-in scenes as
the current source of truth; the author scripts describe themselves as one-time
generators and should not silently overwrite subsequent scene edits.

Tall scenes currently animate brightness only. More samples smooth their pulse
but cannot supply stem motion. Their periodic overlapping trunk artwork and
shared part pulse are deliberate; leave bending for the coherent wind pass.
The measured release baseline supplied by the integrating agent is 6.855 ms per
frame for 622 plants and 200 organisms. Favor bake improvements and the single-
unfold blended stamp so the first slice retains room within 16.7 ms; measure
the resulting crowded scene rather than treating that baseline as a guarantee.
