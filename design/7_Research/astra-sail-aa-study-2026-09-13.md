---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Sail-only coverage: crisp face preserved, movement result mixed

**Do not replace the production sail atlas yet.** Fin-only coverage is a better
candidate than the previous whole-body filter: it preserves the bright face/body
and greatly reduces rest/feed area pulsing. But the moving state's painted-area
variation increases, exposing a separate point-baked body-squash problem. All
production art, other species, host, core and live processes remain unchanged.

This is the bounded follow-up to the [first AA comparison](astra-aa-comparison-2026-09-13.md).
It is an art/sampling study, not canonical direction or an ecological change.

## One explicit source-layer candidate

The actual `art/creatures/sail.tscn` has four Sprite2D layers in this order:

| Layer | Source | Study sampling |
| --- | --- | --- |
| `LeftFin/Sprite` | `sail_fin.svg` | Fixed 4×4 spatial coverage |
| `RightFin/Sprite` | `sail_fin.svg`, vertically flipped | Fixed 4×4 spatial coverage |
| `Body/Sprite` | `sail_body.svg` | Original point sampling |
| `Bud/Sprite` | `bud.svg` | Original point sampling |

The helper rejects an unexpected layer count, order, path or texture. Fins are
resolved first in linear premultiplied color; the point-baked body/bud composite
is then placed over them with the existing source-over convention. Opaque body
pixels are copied exactly. Uniform subpixel blocks bypass unnecessary floating
color conversion, preserving their exact RGBA codes. There is no exposure, contrast,
palette, sharpening or brightness compensation.

“Crisp body” does **not** mean freezing its authored motion: move retains its
vertical 0.95→1.05 squash, feed retains its `(1,1)`→`(1.08,0.94)` squash, and bud
retains its original growth. The first implementation attempt incorrectly took
the constant body through the coverage resolve and lost occasional one-code RGB
values; the body-preservation assertions rejected it. That draft and the following
pre-uniform-copy draft are scratch, not alternative candidates being selectively
reported. The final fixed policy is in `art/studies/aa/bake_sail.gd`.

## Preserved evidence and checks

- Bake: `captures/sail-aa-2026-09-13-bake-final/`.
- Complete comparison: `captures/sail-aa-2026-09-13-render-final/`, including
  `viewer.html`, `measurements.json`, native frame sheets, boundary nets, and
  browser captures. Earlier similarly named directories are development scratch.
- Reproduction commands and source contract: [study README](../../art/studies/aa/README.md).

All four baseline strips exactly match production creature rows 4–7. Across all
64 sampled poses, the bake checks the point-only body/bud image unchanged at
every nontransparent pixel, and checks **1756 opaque body/bud texel instances**
unchanged in the final composite. RGB stored under zero alpha is immaterial and
is not required to be byte-identical. Source-loop endpoints match the first pose;
the nonlooping bud endpoint matches its last pose.

The same production sprite/surface stamps render 48 paired sequences: four states
× two scales (1 and 0.6) × six placements (fixed anchor, smooth translation/rotation,
Front/Right seam crossing, reflected open-rim approach, fixed Top vertex, and quiet
held pose). Each sequence has 360 frames at 60 Hz. Rest/move/feed loop normally;
bud clamps and holds after its five-second authored duration. Six-second excerpt
replay cuts are excluded from temporal differences.

There are **96 clip endpoint checks** across modes/scales/placements and **288
inter-state edge checks**: all 12 directed state pairs, both scales, fixed/seam/rim,
both bake modes, start/end of a 0.3 s fade. Maximum linear-channel difference
across ±1e−7 s is **1.44169e−6**. Loop equality and held-bud endpoints are exact.
The inter-state sheets include the outgoing bud's last pose; they are pure
sampling fixtures, not a test of ArtPresenter's complete meal/gestation history.

All frames are finite and nonblank; held frames remain byte-exact over time within
each mode. The existing five study tests and six sail-target tests pass (five are
reused, **not eleven independent new tests**). The original broad comparison's
eight bake strips and 32 frame sheets were independently reproduced pixel-exactly
after the shared study helpers were extended. No renderer/default was changed.

## Native-scale results

Fixed native anchor, moving authored clip; alpha-area standard deviation and
frame second-difference luma are temporal proxies, not perceptual scores:

| State | Mean alpha area, old → fin-only | Area standard deviation | Mean temporal second difference | Mean peak luma |
| --- | --- | --- | --- | --- |
| Rest | 51.375 → 51.194 | 1.203 → 0.189 (−84.3%) | −80.8% | Exact unchanged |
| Move | 50.750 → 49.889 | 4.375 → 4.786 (**+9.4%**) | −18.3% | Exact unchanged |
| Feed | 50.253 → 50.733 | 1.540 → 0.349 (−77.4%) | −30.7% | Exact unchanged |
| Bud | 50.831 → 50.831 | 0.967 → 0.967 | Exact unchanged | Exact unchanged |

The entire bud is rendered-identical in these fixtures, not merely similar in
mean light. Rest/feed soften the moving gold/purple fin margins without smearing
the turquoise face. Their >=0.5-alpha silhouettes still contract: rest 51.38→47.24
pixels, feed 50.20→47.20. That is a real edge/readability tradeoff even though the
bright body remains exact. Native and enlarged captured frames show the same
recognizable sail; this does not by itself prove preferable continuous motion on
LEDs at room distance.

At scale 0.6, fixed-anchor rest/feed area SD improves about 90.3%/82.2%, while move
SD worsens 16.1%. Smoothly translating/rotating juvenile feed still loses **5.54%**
mean peak luma because destination bilinear filtering mixes body edges with the
changed neighboring fins. Thus “opaque body texels preserved” must not be inflated
into “every destination highlight is identical at every transform.” Adult
translated peak changes are much smaller (rest +0.007%, move −0.193%, feed −0.727%).
Seam/rim/vertex output uses the unchanged production owner/transport helpers;
representative nets were retained, not a claim of exhaustive topology proof.

## The remaining moving-body alias is now measured

Hiding both fins exposes the unchanged point-baked moving body. Its alpha area at
the 16 authored samples is:

```text
19, 19, 28, 37, 37, 37, 28, 19,
19, 19, 28, 37, 37, 37, 28, 19
```

The source's ±5% vertical scale crosses output sampling boundaries and nearly
doubles the painted body area, despite a much smaller continuous geometric change.
Rest's body remains 28 pixels; feed alternates 28 and 22. Fin filtering cannot
remove this body pulse while also keeping those body texels unchanged. The
fin-only result reduces edge roughness but does not establish an all-state
coverage-stability win. The previous whole-body filter hid more of this issue by
softening the body itself; that is precisely the compromise this study avoids.

## Support and cost

All poses remain under the existing 9 px limit. Rest/feed/bud maximum extent stays
6.355 px; move expands 6.355→6.730 px. No scaling shrink, larger production query
contract or clipped stamp was introduced.

Paired release microbenchmarks, mean microseconds per fixed native stamp:

| State | Point | Fin-only |
| --- | --- | --- |
| Rest | 3.089 | 3.100 |
| Move | 3.003 | 3.423 |
| Feed | 3.048 | 3.077 |
| Bud | 2.834 | 2.832 |

Move costs about 0.42 µs more due to its expanded support; other differences are
small enough to treat cautiously on the shared machine. Both modes use the same
runtime filter. The candidate bake takes about 21 ms per 16-pose clip versus
1.4–1.7 ms baseline. Neither number is a populated-world frame-budget measurement.

Final isolated Chromium sampling: **60.002 Hz**, p95/max **16.8 ms**, 360 unique
frame indices over 362 callbacks. Visual inspection covered representative native
and enlarged all-state frames, juvenile frames, and feed→bud midpoint; numerical
checks cover the broader matrix. This is not physical-cube or subjective full-video
validation. Browser process used a new profile and was closed after capture.

## Recommendation

Keep the pack unchanged. Retain this fin-only implementation as a promising
**rest/feed** candidate, with its silhouette cost visible in the saved comparison.
Do not silently ship a per-state policy: that would be a separately reviewed
candidate. Before admitting the full sail, the next narrow **source-rig** question
is how to express its body breathing without the 19↔37-pixel point-sampling flip—
for example, revise the body's authored squash/alignment while retaining the face
and fin articulation. That changes authored pixels and needs a fresh matched
study; it was deliberately not mixed into this candidate. It is more directly
motivated than adding blanket runtime blur or changing other species.

Exact source git blobs: sail rig `ff8a28381b9a64e4ddefabdfd1a26777d58045f9`,
fin SVG `73b6d914cb9908faf768d639c5a87929e383e8bb`, body SVG
`98a5a3b350134726cf91e508ce99db0968635946`, bud SVG
`1071e2980c9de0eedc281e848a17e3f9e1e159bf`. Godot 4.7.2; production
renderer/atlas blobs are unchanged from the preceding AA report. Default Godot
`user://` log writes were sandbox-denied, but study outputs and validation exited
zero. Only study source and evidence are committed.
