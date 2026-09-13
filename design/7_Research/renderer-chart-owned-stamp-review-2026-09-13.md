---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review — chart-owned endpoint stamping

## Scope and verdict

Reviewed public `cubarium_render::stamp_pose_in_chart` from `33d1119`, including
the approved vine endpoint use. The helper has sound ordinary input guards and
preserves the established coincident-center image semantics. Its query cap safely
turns an over-large retained-chart query into a no-op rather than passing an
illegal radius to `cubarium_surface::unfold_pixels`.

The independent adversarial sweep found **no** case in which the helper's current
candidate rule can reach a destination outside the center's physical nine-pixel
neighborhood. This is strong finite test evidence, not a proof for every
continuous same-face offset. The existing vine-specific physical-support test
therefore remains an appropriate caller gate; it must not be summarized as a
general theorem from the chart-radius cap alone.

No renderer, art, asset, or simulation behavior was changed by this review.

## Public API checks

- Both owner and center must be canonical and on the same face; empty poses,
  empty masks, invalid headings, non-finite/non-positive opacity, and an extent
  above nine are no-ops. Opacity is bounded to one.
- `query = radius + |owner - center|` is rejected above
  `MAX_LOCAL_RADIUS = 32`. Thus the public helper cannot trigger the surface
  rasterizer's radius panic through an arbitrary same-face offset.
- At coincident centers, the new integration test compares every canvas pixel
  against `stamp_layers_bent`, over every face, four vertices plus an interior
  point, three pose weights, a nonidentity bend, and a colored destination
  background. It is bit-identical, covering identity pose endpoints, blended
  pose sampling, and single source-over compositing.

## Physical-support audit

`chart_owned_patch_review` uses `cubarium_surface::unfold(center, destination,
9.0)` as the independent physical predicate. It checks every retained-chart
candidate before sampling, not merely every visible texel:

- an 8×8 owner/center lattice on every face (with over-32 queries excluded);
- 1,000 deterministic irregular pairs near each of the four vertices of every
  face; and
- actual blended, masked, bent 9×9 patches recentered from each corner toward
  the face interior.

All checks passed. The final image-level case asserts every nonzero canvas pixel
has a valid center-to-pixel surface unfolding of at most nine. This includes
the local ownership ambiguity at vertices that motivated the retained chart.
The sample is deliberately bounded; it cannot establish the universal claim for
all real-valued inputs.

## Narrow hardening option

The smallest API-level hardening, if the public helper must guarantee physical
support rather than require callers to gate it, is to discard each selected
`PixelImage` unless `unfold(center, pixel_center, 9.0)` succeeds. That leaves
the retained owner chart responsible for sampling/orientation while making the
physical budget executable for arbitrary inputs. It would only remove pixels
from a caller that violates the existing contract; the approved vine caller's
retained support test establishes that its current output would be unchanged.

This is a proposal, not an implementation request: it adds a surface lookup per
candidate and should be chosen only if the public API is intended to promise
that stronger property. Until then, preserve the documented caller requirement
and the vine's explicit vertex sweep.
