---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Sail candidate: stable moving body + fin4

**Updated disposition after the bounded ablation below: use body hold only.**
Production now keeps move's constant body scale but sets the fin-coverage metadata
false; the optional kernel remains available and inactive. The initial combined
candidate and evidence below are retained as history, not the final recommendation.

Recommend this bounded candidate for root's visual review. It removes the moving
body's accidental 19↔37-texel point-bake pulse while retaining the recognizable
crisp cyan face and opposing fin articulation. It is not a blanket AA default,
an ecological change, or a claim of physical LED validation.

## Intentional art trade

One explicitly combined candidate, **stable-body-plus-fin4**:

- Move's five Body:scale keys become `(1,1)` instead of vertical `.95↔1.05`.
  We intentionally give up that tiny squash. The large rasterized expansion it
  caused was not proportional to the authored motion. Opposing fin rotations
  remain ±.2 radians at the same times, preserving the swimming/flapping gesture.
- The already measured fin4 policy covers only the two rotating lower fin layers.
  Body and bud remain point sampled. No palette, exposure, sharpening, source
  SVG, clip duration, feed squash, rest motion or bud growth is changed.
- `Sail` explicitly opts in through `metadata/sail_fin_coverage4`. The exporter
  validates the four exact ordered sprite paths and texture sources before using
  the offline kernel. Other rigs execute the original point raster unchanged.

This follows the [fin-only study](astra-sail-aa-study-2026-09-13.md), which was
correctly rejected on its own because moving-body area variation got worse.

## Measured and visible result

Fixed native anchor, same 360-frame 60Hz trajectory:

| State | Alpha-area SD, original → candidate | Mean temporal second difference | Mean luma change |
| --- | --- | --- | --- |
| Rest | 1.203 → .189 | −80.8% | −1.09% |
| Move | 4.375 → 1.070 | −71.3% | −3.01% |
| Feed | 1.540 → .349 | −30.7% | +.22% |
| Bud | .967 → .967 | Exact same rendered image | Exact same |

Move's body is now exactly 28 texels in all 16 poses. Its total area range is
47.827–51.569 instead of 46–60; mean area 50.750→49.920 (−1.64%). Fin movement
still changes the silhouette. At scale .6, move area SD falls 72.4% and temporal
second difference falls 72.0%. Mean juvenile move luma falls **8.23%**, a real
tradeoff, not silently compensated. The fixed native moving face's peak is now
constant at the original maximum .764428; its time-mean peak rises 3.60% because
the point-baked face no longer loses coverage during squash. Rest/feed peaks
remain exact at that placement, not necessarily under every translated/rotated
bilinear mixture. The earlier fin-edge softness/half-alpha silhouette trade
remains: coverage does not preserve a binary jagged fin outline.

Viewed native/enlarged all-state captures, juvenile captures, rim and handoff
captures: the moving face is steadier and the purple/gold fins remain distinct.
The result looks less like the whole fish inflating. Actual meal-world crops are
subtler because background and neighboring organisms dominate; this is a cleaner
existing gesture, not a conspicuous new biological acknowledgement.

## Exact scope and validation

- Complete regenerated pack differs only in creature rows 4–7. All other creature
  rows and ground/habitat/pack.json/plants/tall files are **byte-exact**. Bud row's
  stored transparent RGB changes, but rendered bud is exact. Normal exporter
  rerun reproduces the entire candidate pack byte-for-byte; eight comparison
  strips also reproduce exactly. Only `assets/atelier/creatures.png` is installed.
- All 64 source poses check opaque point-body/bud preservation against the new
  point source (1756 instances); unchanged rest/feed/bud point bakes match the
  original atlas. All source-loop/nonloop endpoints pass.
- Same 48 paired sequences as the prior study: four states, scales1/.6,
  fixed/translated/seam/rim/vertex/held. 96 clip-endpoint checks and 288 directed
  cross-state edge checks pass; largest ±1e−7s channel change1.441687e−6.
  Finite/nonblank geometry and held-frame byte identity pass. Six unique Rust
  study tests pass, with five reused in the second target (11 executions).
- Extent maximum6.730px, below9. Native fixed stamp microseconds old→candidate:
  rest3.047→3.094, move3.030→3.425, feed3.054→3.090, bud2.846→2.850. This measures
  stamping only, not whole-world frame time. The 4×4 work happens offline, about
  21–22ms per 16-frame clip; there is no runtime supersampling addition.

Evidence: `captures/sail-stable-2026-09-13-{bake,render-final,pack-verification.json}`.
The full native comparison sheets and `render-final/viewer.html` are saved.
Headless viewer cadence60.0027Hz, p9516.7ms/max16.8ms,360 distinct excerpt indices;
this is local browser playback, not cube scanout. Earlier `render/` is scratch.

## Actual meals, unchanged biology

The new independent `sail_meal_comparison` runs each recorded seed1/8 opening once
for1000 ticks, applies the same real standard feed at elapsed600, and draws two
packs through the **same current meal controller enabled in both presenters**.
No forced forms/actions or analytical overlays. Each produces933 paired native
frames covering elapsed590–900; draw-time ecology hashes are unchanged. Actual
fed sail member-ticks are7794/7977 across the1000-tick trajectories. Closing hashes
are `b255587eeb6b53de` / `dda40aa87ac237fe`, matching the earlier actual trajectories.
Intake is not proof of manual-crumb provenance.

`captures/sail-stable-2026-09-13-meal-seed{1,8}/` retains complete old/new frame
streams locally, reports and all sail observational rows. Contact sheets select
the **first** real sail meal onset after600, with offsets−1,0,1,3,6,9,15,30 frames;
selection is recorded, not visually cherry-picked. Full local file viewers are
included. Seed1 viewer was exercised and onset screenshots inspected after fixing
its initial1866-request preload to batches of32. Browser callbacks60.0027Hz but
only329 distinct indices in that six-second probe: do not call it proof that every
60Hz source frame was displayed. No new model or asset filter is inferred from it.

Concatenated PNG SHA256, filename-sorted933 frames per stream (full streams retained
locally, not duplicated into git; committed reports/contact/screens and regeneration
helper preserve the review and provenance):

| Stream | SHA256 |
| --- | --- |
| seed1 old | `9d139baf7f791cec6136ec461a4353d3ea7c0ae3a5d2cb360168b8e8def9e123` |
| seed1 new | `5d5f3718788889fec501ed76997c1652c0551f029e2425b3798138aea0535085` |
| seed8 old | `8773afa9d5ca17948a1d69a46209f3e4ed54b1c28a08f931ddd5d92309c69d21` |
| seed8 new | `08dea218858d7bb03e1710b744159919baf1082815454376f9250211871b22b5` |

Candidate creature PNG SHA256:
`fcc5079f17d7f7d5d9e30524b511274dd255bfdac8dbc1ab9f3dc3a0aa7091ad`.
No host/core/schema/live operation belongs to this candidate. Root owns deployment
and should still review the actual64px world preview and full-frame budget.

## Bounded body-hold-only ablation and final art revision

Following [Fable's independent review](fable-sail-candidate-review-2026-09-13.md),
compared exactly one ablation: the same constant-body rig with fin metadata off
on an isolated loaded instance. No2× kernel, palette adjustment or parameter sweep.
The original and combined recordings remain untouched. The completed three-way
viewer is `captures/sail-body-only-2026-09-13-ablation/viewer.html`; black-background
16-pose source strips are in the sibling `-ablation-black/` directory.

**Choose body hold only for the current production candidate.** The fixed-size face
no longer inflates, while the angular purple/gold fin shape remains clearly
articulated. In the three-way native/enlarged still sequences, body-only keeps the
rest/feed fin definition consistent with bud and the established art. Combined
coverage visibly changes those quiet fins into gradients, with no clear advantage
in the two crowded actual meal worlds. This is an aesthetic trade informed by
images and measurements, not a conclusion that the smallest area SD must win.
Rest/feed point-bake twinkle is deliberately retained; it is not claimed solved.

At the fixed anchor, original / body-only / combined:

| State | Native alpha-area SD | .6-scale alpha-area SD | Native pixels at alpha≥.9 (time mean) |
| --- | --- | --- | --- |
| Rest | 1.203 / 1.203 / .189 | .535 / .535 / .052 | 50.675 / 50.675 / 41.000 |
| Move | 4.375 / 1.272 / 1.070 | 1.381 / .282 / .381 | 47.861 / 48.750 / 42.722 |
| Feed | 1.540 / 1.540 / .349 | .335 / .335 / .060 | 48.800 / 48.800 / 39.850 |
| Bud | .967 / .967 / .967 | .376 / .376 / .376 | 50.783 / 50.783 / 50.783 |

Body-only reduces native move area SD70.9% and79.6% at .6 scale, relative to the
original (combined75.5%/72.4%). Its native move temporal second difference
is .03904 versus original.08213 and combined.02354: the filter still provides
additional temporal smoothing, but that is not automatically preferable character.
Body-only and combined have the **same fixed-anchor mean peak** in every state
at both scales. The moving face is stable and peaks at the original maximum.

Important attribution correction: original/body-only/combined juvenile move mean
luma is3.63900 /3.36929 /3.33940. The body correction alone is−7.41%; adding fin4
is another−.89% relative to body-only. Thus most of the earlier combined−8.23%
does **not** come from fin softness. Native move luma is10.19827 /10.01500 /9.89131.
There is still no brightness compensation.

The source partition uses the actual point-baked Body+Bud mask, not color matching.
Outside it, rest solid-fin pixels average23.375→13 and feed27.5→17.375 for body-only
versus combined (−44%/−37%); fin alpha sums are23.375→23.194 and27.5→27.982.
These differ from Fable's heuristic absolute counts but support its real definition
concern. Original move's fin-only partition is intentionally omitted because its
body mask differs. Whole-sprite source counts and all16 per-pose partitions are
saved in `source-solid.json`; no source alpha sum is called light output.

Validation and evidence:

- `reduce_sail_ablation.mjs` checks all48 state/scale/scene triples, exact shared
  original metrics, and exact body-only non-move output before saving `comparison.json`.
  No scene subset is silently selected. Body-only's96 clip and288 handoff endpoint
  checks pass; all six unique study tests pass (11 executions). Quiet frames remain
  byte-exact; rooted/seam/rim/vertex use unchanged production geometry.
- Actual meal helper linked against **frozen a88b15b host/core** under
  `/tmp/cubarium-sail-rollout-a88b15b`, not Opus's changing main. Both seeds again
  produce933 pairs,7794/7977 actual fed sail member-ticks, and the same closing hashes.
  Original PNG streams reproduce their earlier concatenated hashes exactly. Both
  triple meal contact sheets assert identical selected IDs/positions/meal histories
  and identical original crops before assembling original/body-only/combined.
- Body-only new-stream SHA256 (filename-sorted933 PNGs): seed1
  `43ba9895573f8beacc25c06d95c0553b93fcccbfacc95bd0d9a97e73d75443ca`, seed8
  `6de7029b48a5e72443bf872fe58762cbd6afc9c56eff4e6ec0db724824958b77`.
  Full streams remain local in `captures/sail-body-only-2026-09-13-meal-seed{1,8}/`;
  reports, selections and contact evidence are committed, not duplicated raw streams.
- Three-way browser playback60.0027 callbacks/s, p9516.7ms/max16.8ms,360 distinct
  source indices in the six-second probe. Reviewed still captures, not a human
  continuous-motion or physical LED observation. Initial transparent source-sheet
  draft uses the image viewer's white background; the explicit black-composite
  `-ablation-black` sheets are the meaningful source-art review.
- The authorized production revision changes only `sail.tscn` metadata true→false
  and regenerated `creatures.png`, retaining all body-hold keys and the inactive
  kernel. Normal exporter now reproduces the isolated body-only pack byte-exactly.
  Against the original, **only row5 (sail move) differs**; all other rows and pack
  files are byte-exact. `-final-verification.json` enforces that narrower condition.
  Final PNG SHA256 `21ee9b341238304d1e8972f4b104d3325d595b0e431ebd22592a1721e12285de`.

No host/core/schema/live changes in this revision. Root owns deployment and the
whole-world copied-preview budget check. The combined option remains preserved
for future display-specific judgment, not adopted by this revision.
