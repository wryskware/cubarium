---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Astra review of in-progress animation primitives

Follow-up verification by the root orchestrator: after Fable's fixes, all seven
`astra_regressions` renderer tests and all four `astra_motion_regressions` host
tests pass. The complete host/renderer `--all-targets` suite also passed before
the final additional animation test files were introduced. The findings below
record the earlier in-progress state; consult the final
[implementation record](animation-slice1-2026-09-13.md) for dispositions and limits.

Reviewed an in-progress working-tree state while Fable was still implementing
the slice. Findings need comparison with the completed code; they are not a
claim about the eventual integration. Scope: `Pose`, `Mask`, `stamp_pose`, and
`Sprite::subtract` in `crates/cubarium-render/src/sprite.rs`, plus sampling and
cap construction in `crates/cubarium/src/art.rs`. No source edits or test runs.
The presenter still used the older column path during this inspection.

The primary compositor is mathematically sound: invert heading and scale,
sample and interpolate linear premultiplied RGBA, multiply all four channels
by reveal coverage, then apply one source-over operation. This preserves shared
opaque pixels. It uses one existing surface unfold and the maximum participating
extent, so the new interpolation does not itself change seam ownership or the
nine-pixel stamp budget. Looping and nonlooping clip sampling have the intended
wrap/clamp behavior for valid loaded clips.

## 1. Reveal starts with a discontinuity

`stamp_pose` exits for `reveal <= 0`, but radial coverage is
`clamp(reveal - radius + 0.5, 0, 1)`. At the pivot, coverage jumps from zero to
approximately 0.5 as reveal changes from zero to an arbitrarily small positive
value. A one-pixel opaque sprite with its pivot on the output pixel center is
a concrete reproduction. This contradicts the claimed continuous reveal.

Minimal fix: cap or multiply the initial coverage by a continuous start envelope
that tends to zero with reveal (for example `clamp(reveal, 0, 1)`), or redefine
the radial ramp so zero radius has zero coverage at reveal zero. State the
chosen ramp precisely. The axial path can encounter the analogous issue for
bilinear support sampled below the tile's bottom edge; applying an initial
envelope to both variants handles that case as well.

Focused check: stamp the one-pixel fixture at reveal values `0`, `1e-6`, `0.25`,
and the fully revealed value. The tiny-positive image should tend to zero, and
coverage should remain monotonic and reach the unmasked image. Include one
subpixel-offset axial fixture with nonzero filtered coverage below the bottom.

## 2. Color equality cannot identify crown structure

`CROWN_CAP_ROWS = 4..16` and `Sprite::subtract` remove every equal RGBA texel in
that region. They also remove genuine crown artwork whose color happens to
match the trunk. In the checked-in spiretree source, crown tile pixel `(7, 4)`
is mint, as is trunk tile pixel `(7, 4)`, so it is deleted. That pixel is part of
the dome; the explicit trunk-only tail begins at row 8.

At whole-cell alignment an underlying identical trunk texel can hide this
deletion. At fractional crown height the stationary trunk has a different
vertical phase or partial coverage underneath. The deleted dome pixel can then
expose a trunk-colored or translucent hole. Whether the completed presenter
conceals particular holes depends on its new-segment mask; color subtraction
alone does not establish the claimed "crown's own art only" semantics.

Preferred fix: author a cap-only sprite or explicit structural removal mask.
For spiretree, restricting removal to its documented trunk-only tail `8..16`
is a small safe art-preservation fix. Glasscane's cane threads between bulbs,
so an explicit removal mask is more precise than one shared row range. Keep
the dome/bulb ownership independent of palette equality and pulse brightness.

Focused check: inspect cap-only frames over a contrasting checkerboard, then
composite them over a fixed trunk at fractional offsets `0`, `0.25`, `0.5`,
and `0.75` pixels. Dome interiors must remain intact; no repeated shifted tail
should remain. Review one frame away from pulse extrema as well.

## 3. Cap construction assumes synchronized part timing

`load_tall` subtracts trunk and crown frames by matching frame index, but accepts
different positive durations for those clips. The proposed cap and the trunk
then play at different phases, violating the equality on which subtraction
depends. Current authored parts use equal durations; this is an accepted-pack
contract gap rather than an observed current-art failure.

Minimal fix while retaining subtraction: validate matching trunk/crown timing
and document the synchronized-part contract. A tiny pack-metadata fixture with
different crown duration should be rejected with an actionable error. Explicit
cap artwork would remove the color-equality dependency, though joined animated
parts still need coherent timing.

## Minor consistency point

`Pose::extent()` returns the larger endpoint radius even at weight 1, whereas
`stamp_pose` correctly selects the second endpoint alone. Align the helper with
the stamp's endpoint behavior before callers use it to calculate allowed scale;
otherwise they can unnecessarily shrink a small second pose because the unused
first sprite is larger. A two-endpoint extent assertion is sufficient.

## Presenter follow-up while integration was still in progress

The following findings are additional to the already-reported tick-zero clock
reset, repeated-observe completion boundary, and growth being sampled only at
20 Hz. No suites were run. Reversal in `advance_growth` itself preserves the
same pair of stages and the same upper-stage weight by swapping endpoints and
using `1 - g`; that portion is coherent.

### Trunks pop at birth and at each added segment

`draw_column` chooses `ceil(height)` tiles. Every included trunk uses full
`MOTIF_OPACITY`, and its initial axial reveal is already 9 pixels (first trunk)
or 12 pixels (later trunks). As height changes from zero to a tiny positive
value, a large piece of trunk therefore appears immediately; the base/cap fade
does not fade that trunk. At `height = integer + epsilon`, the newly added tile
immediately stamps its bottom twelve rows over the old column. Periodic equal
colors do not make repeated source-over neutral: two overlapping opaque texels
at opacity 0.85 have combined coverage 0.9775 instead of 0.85. Translucent glass
is even more sensitive. Wilting removes the same extra contribution at the
integer boundary; newly included odd vine tiles behave similarly.

Fix the ownership of overlapping trunk pixels: render each structural strip
once, with a continuous reveal for the newest strip. A first-segment opacity
envelope can address birth, but by itself does not solve later overlap jumps.
The base and crown must participate in the same ownership convention. Focused
image checks should compare `0` versus `epsilon`, and `k - epsilon`, `k`, and
`k + epsilon` for both spiretree and glasscane, with and without vine. As epsilon
shrinks, the maximum pixel difference should tend to zero on both growth and
decline. Use a contrasting background so opacity changes are measurable.

### Cross-clip fades restore held poses and can snap at their boundaries

`stage_pose` uses `Clip::sample` when fruit is exactly zero or one, but switches
to discrete `Clip::at` endpoints for `0 < fruit < 1`. Beginning a fruit fade at
a time between atlas samples therefore drops an already-interpolated stage to
its previous baked pose, then holds animation frames throughout the fade. The
fade endpoint jumps back to interpolation. This occurs during every fruit
appearance and disappearance, independent of the 20 Hz growth issue. Body
state fades use the same discrete fallback.

Keep each clip's temporal interpolation during a cross-clip fade, with up to
four sampled sprites combined before the single source-over. Cache/reuse the
description and measure cost on the existing crowded fixture; do not replace
it with four ordinary partially transparent stamps. Check continuity at fruit
weights `0`, `epsilon`, `1 - epsilon`, and `1` at a fixed nonsample animation
time. Repeat for a body state transition.

Rapid body-state changes have a second discontinuity: `observe_bodies` replaces
`prev` with the last target state, losing the pose currently on screen. For
example, A→B is only one-third complete when B→C begins; the new transition
starts from pure B instead of the prior two-thirds-A/one-third-B pose. Retarget
from the actual blended pose or use a transition rule that preserves it. A
scripted A→B→C sequence spaced more closely than `BODY_FADE_SECONDS` exposes it.

### Rewound views inherit the previous world's hysteresis

On a backward tick, `snap` is true, but `observe_with_fruit` computes the new
target with `next_stage(self.growth[index].target, ...)` and
`next_tall(self.tall[i].target, ...)` before snapping. Old targets can survive
inside the hysteresis interval, so restoring the same view into an existing
presenter and a fresh presenter yields different initial plants/heights.

When resetting to a replaced/rewound view, derive target morphology from the
same empty baseline as first initialization, or explicitly restore its own
saved visual state. Check a rich view followed by an earlier view whose density
lies just below a rise threshold but above its hysteresis exit, and compare
against a new presenter initialized directly from that earlier view.

### Fruit-display semantics need a deliberate disposition

The integrated implementation deliberately keeps `Growth::fruit` above zero
for up to `FRUIT_DROP_SECONDS = 1` after the actual fruit field becomes empty.
`draw_with_fruit` also ignores its supplied fruit field after initialization.
Thus empty resource state can still show food, and calling that helper with
`None` after a fruity initialization cannot suppress it. This conflicts with
the proposal's promptly gated food accent, although a short disappearing accent
could be an intentional artistic choice. Decide explicitly: keep any decorative
fade separate from the food signal, or document that the accent can lag by one
simulated second and make the helper's observe requirement unambiguous. Include
an empty-fruit capture immediately after observation; do not count eventual
fade-out alone as validation of truthful food imagery.

## Saved independent regression targets

- `crates/cubarium-render/tests/astra_regressions.rs`: seven public-API image
  checks. Its first isolated run passed four and reproduced three failures:
  radial tiny-positive reveal peak 0.500001, subpixel axial reveal peak
  0.18750076, and endpoint extent 3.207 instead of 1.207. Fable owns the fixes;
  rerun this target against completed source.
- `crates/cubarium/tests/astra_motion_regressions.rs`: four public presenter
  checks for tick-zero hold, same-tick stage-completion idempotence, rewind
  initialization equivalence, and 30/60/120 draw-rate independence. The first
  isolated run was blocked before compiling the test by an in-progress
  production error: `art_present.rs` referenced the removed
  `FRUIT_DROP_SECONDS` constant. No conclusion about pass/fail follows from
  that blocked build. The integrating agent requested that tall continuity
  fixtures remain with Fable's source/visual work to keep this follow-up bounded.

Commands: `cargo test -p cubarium-render --test astra_regressions` and
`cargo test -p cubarium --test astra_motion_regressions`. Only these new targets
were run by this review; no production source or existing tests were modified.
