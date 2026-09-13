---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Multipart minification review: `7b8ad4f`

Bounded source/math review of the committed renderer before the living presenter
adapter. The inspected multipart/sprite sources matched `7b8ad4f`; no production
files, art, core, existing tests, live processes or other workers' build directories
were changed. Public-API fixtures are saved in
[astra-minification-regressions-2026-09-13.rs](assets/astra-minification-regressions-2026-09-13.rs).

## Most visible: the filter jumps at adult scale

A centered opaque 1×1 sprite, pivot `(0.5,0.5)`, root Front `(32.5,32.5)`, heading
`+x`, paints pixel `(32,32)` at **1.0** for scale 1 and **0.5625** for scale
`1 − 1e-9`. The geometry barely changes; `n` changes from 1 to 2, abruptly replacing
the point sample with a full-width box average. This is a 43.75% center-light jump,
not an invisible filter tail. It conflicts with smoothly growing into adult size.
Other reciprocal-integer sample-count boundaries merit epsilon sweeps too.

Preserve scale-1 identity without this jump by specifying a continuous filter
schedule. One bounded option over the admitted juvenile range is to blend the
two neighboring quadrature-grid results continuously as `1/scale` crosses integers,
including the point grid at 1. Compose each sub-sample's complete material/depth
stack before averaging. This is an approximate, smoothly varying reconstruction
filter near adult scale, not an exact box integral; document the choice honestly.
An alternative continuous reconstruction method is fine. Test both image differences
and support across every grid transition, not only exactly 0.5 and 1.

## Actual query clipping despite the existing margin

`SUPERSAMPLE_REACH = 0.5` bounds one coordinate, not the Euclidean displacement
of the `n×n` grid: the corner reach is `sqrt(2) * (0.5 − 0.5/n)`.
However, that alone does NOT establish clipping: `RIG_MARGIN` has spare support.
The conservative spare beyond the true bilinear corner is

```text
scale * (1/sqrt(2) + 0.5 + RIG_MARGIN − sqrt(2))
= scale * (1 − 1/sqrt(2)).
```

At scale 0.316 (`n=4`), spare ≈0.09255 exceeds the sampling-reach shortfall
≈0.03033. The existing bound remains sufficient under this estimate; the rotated
and subpixel fixture sweep agrees. At scale 0.2 (`n=5`), spare ≈0.058579 is LESS
than the ≈0.065685 shortfall: the possible gap is ≈0.007107 pixels.

Concrete public-renderer case: the same 1×1 sprite at root Front
`(31.902,31.902)`, heading `+x`, scale 0.2. Pixel `(32,32)` has displacement
`(0.598,0.598)`, outside automatic radius ≈0.841421. A generous legal query,
obtained by adding a fully transparent part at body offset `(5,0)`, paints
**0.0000039999923 per channel** there; the normal query paints zero. Transparent
padding contributes no material. This is a genuine clipped bilinear tail, though
far too small to justify claiming a visible LED defect from this fixture alone.

Use the actual grid's radial corner reach (or conservative `1/sqrt(2)`), recomputed
for the chosen filter schedule, and validate the resulting query against 32.
Do not change the per-sprite 9-pixel budget or silently cap the query.

## Body-axis square is not the destination pixel's square

The implementation adds `heading*ox + side*oy` before transforming into body
coordinates. It therefore rotates the sampling footprint with the animal. A
destination pixel is chart-axis aligned; its box samples should instead add
`Vec2(ox,oy)` in the selected root chart, then transform into the shared body basis.
Cube chart transitions are right-angle axis permutations, so their pixel squares
remain axis aligned. This does not require independently carried part anchors.

Concrete case: centered 1×1 sprite, scale 0.2, heading `(1,1)`, root Front
`(32.5,31.68)`. Its entire rotated bilinear support ends at
`y = 31.68 + sqrt(2)*0.2 ≈31.962843`, below pixel `(32,32)`'s lower boundary 32.
An actual destination-box average is exactly zero, yet this sampler paints
**0.00040692737 per channel** there by reaching outside that box. Body-oriented
filtering could be an intentional artistic filter, but it is not the claimed
destination-pixel area average. Prefer the physical pixel footprint for this task.

## Generic positive-scale API has no work bound

The API accepts every finite positive scale, but `n = ceil(1/scale)` gives
quadratic work. Scale `1e-6` requests a trillion samples per enumerated pixel;
very small positive values additionally overflow `n*n` in checked builds, while
release wrapping/cast saturation does not create a useful work bound. These
pathological calls were NOT executed. This is a direct loop/arithmetic finding.

The admitted hunter range can have a strict known sample cap, but the renderer's
generic public contract must say and enforce one too: reject an unsupported
minification range before multiplication/iteration, or provide a bounded alternative
filter with explicit quality limits. Do not silently enlarge the animal to satisfy
the budget. Checked sample-count arithmetic alone prevents overflow, not a frame
stall. This issue is separate from support and does not imply current normal-sized
Lanternjaw draws request pathological scales.

## Reproduction and interpretation

An isolated temporary Cargo package at `/tmp/cubarium-astra-minification-jf4SpR`
uses the saved Rust fixture as its library and path dependencies on this repository's
`cubarium-render` and `cubarium-surface`. Running
`cargo test --offline --manifest-path /tmp/cubarium-astra-minification-jf4SpR/Cargo.toml -- --nocapture`
completed in under a second: **1 passed, 3 failed**. The failures are the minimal
adult-transition, clipped-tail and rotated-footprint counterexamples above, not
unrelated suite failures. The 0.316 support sweep passes. Fixtures are intentionally
outside the production suite while Fable owns the correction; move/adapt them into
owned integration tests when implementing the agreed filter contract.

These findings preserve whole-rig scaling, root-owned topology, Fable's silhouette,
material/depth compositing and the useful improvement over point-only juvenile
sampling. They request a corrected bounded, continuous sampling policy, not a
return to omitted tiny parts or an artificial minimum visible creature size.
