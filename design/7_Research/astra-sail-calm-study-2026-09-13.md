---
design_status: exploration
---

# Sail calm fin study — 2026-09-13

Recommendation: advance **brace** for Fable's independent native-motion review,
not automatic production adoption. It retains the crisp purple/gold sail silhouette
while replacing constant resting edge chatter with one visible adjustment and making
feeding fins supportive of the existing body chew. Reject **settle**: its zero rest
variance is an invisible authored hinge, not a better readable animation.

This is art-side rest/feed articulation, **not physiological quiet**. No positions,
core activity, intake, pause state, meal timing or biological policy are changed.
The actual presenter still chooses movement/rest/meal/bud from the same world state.
No production asset, sampler, host, core, schema or live process was edited.

## Source and bounded alternatives

Read canon and retrieved the prior sail work through Lore/Graft; actual sources verified.
The shipped [body-only disposition](astra-sail-stable-body-2026-09-13.md) and
[Fable review](fable-sail-candidate-review-2026-09-13.md) remain the control: move body
scale is already held, fin4 is inactive. Rest's small ±0.12–0.16 rad oscillation and
feed's ±0.2–0.1 oscillation still cross point-bake thresholds.

Exactly two source-motion alternatives were baked. Right fins mirror left signs:

| Clip | Brace | Settle |
|---|---|---|
| Rest, 4 s | angles 0, 0, .2, 0, 0 at 0, 2.5, 3, 3.5, 4 s | .12, .12, 0, .12, .12 at the same times |
| Feed, 2 s | steady .2 rad braced fins | .2, .2, 0, .2, .2 at 0, .25, .5, .75, 2 s |

Body/face, feed body chew, move, bud, palette, layering, 16-frame clocks, point
compositor and runtime sampling are unchanged. This intentionally trades continuous
tiny fin flutter for restful dwell and a braced eating silhouette. There is no
brightness/sharpening compensation or hidden fin4 filter. The occasional gesture is
an authored rest loop, not a newly observed behavioral event or synchronized core habit.

The cloned scenes and candidate packs are in `captures/sail-calm-2026-09-13-bake/`.
The [authored helper](../../art/studies/sail-calm/README.md) preserves the two variants
and exact commands. Rust renderer and optional real-host dependencies are frozen
schema12 **3147775**, not moving quiet/schema13 main. Source assets were cloned before
the subsequent unrelated crown metadata update; copied pack metadata is not a proposed
whole-pack production replacement.

## Measured and visible differences

Six seconds at 60 fps, three variants × all four states × adult/.6 × six placements:
144 sequences (48 records), including translation, seam, rim, vertex and held time.
Below is fixed-anchor alpha-area standard deviation, not a stand-alone quality score:

| State/scale | Original | Brace | Settle |
|---|---:|---:|---:|
| Rest adult | 1.20305 | .80783 | 0 |
| Rest .6 | .53469 | .14361 | 0 |
| Feed adult | 1.53969 | .26136 | 1.89449 |
| Feed .6 | .33452 | .08712 | .35407 |

Brace's mean absolute temporal second difference falls 49.2%/54.7% for adult/.6 rest
and 49.3%/61.7% for feeding. It also decreases in every other measured non-held
rest/feed placement. This is **not uniformly smaller motion**: native rest's largest
frame L1 luma change rises .08351 → .24985 (~3×), juvenile .03571 → .07750. The
deliberately coherent adjustment is more noticeable than each old flutter. Clip
blending remains continuous; the short gesture still exposes existing point-bake
quantization. Fable should judge that concentrated gesture at 1× before integration.

Brace retains solid fin definition rather than spreading it into translucent edges.
Measured outside the unchanged actual body/bud layer, rest solid fin pixels average
23.375 → 22.3125 (−4.5%, not the rejected roughly-half-coverage fin4 result); feed
27.5 → 31.375 because the braced pose is held. Native peak luma is unchanged in both
states. Mean luma changes rest −2.99%, feed +3.65%; juvenile −.074%/+ .761%.
These are pose/dwell consequences, not compensated light. Move and bud measurements
are exactly identical across all variants/placements/scales.

Visual inspection of source pose strips, native crowded frames and enlarged real
meal contacts supports brace: face stays crisp, fin tips remain distinct purple/gold,
and feed body's contraction still reads between steady fins. Settle's feed fold
adds another fin collapse during the first chew; area variation increases and it
does not look more composed. Its resting frames are all byte-identical despite
authored hinge keys, so it fails the intended visible occasional adjustment.
Playback viewers are supplied for independent temporal review; static/contact
inspection plus temporal measurements is not physical-LED viewing or a claim that
I watched every generated frame at playback speed. No render-cost improvement is
claimed: candidate uses the same atlas dimensions and unchanged runtime paths.

## Real meal evidence and verification

Four runs: seeds 1/8 × brace/settle, each 1,000 actual ticks from archived
`captures/hunter-openings-2026-09-13/seed-{1,8}/world-144000.cubw`, real standard feed
at elapsed 600. Each writes 933 old/new frame pairs; same world drives both presenters.
No forced sail, invented feeding, core quiet or scripted organism position.
Input snapshot schema9 is decoded by frozen schema12. Draw leaves ecology hash exact.

- Seed1 opening `7a81c8e5a08f4b48`, closing `b255587eeb6b53de`; 7,794 actual fed sail member-ticks.
- Seed8 opening `c6b5e8f078a671ac`, closing `dda40aa87ac237fe`; 7,977 actual fed sail member-ticks.
- Across variant runs, all 933 original encoded PNGs, sail observations, receipt,
  opening/closing hashes and fed counters match for each seed.
- Unbiased first onset selection: seed1 adult `8:4` at elapsed600, seed8 juvenile
  `5:3` at604. Intake is authentic; this does not prove the food came from manual care.
  The seed8 top-edge crop deliberately includes blank off-net space, not missing art.

Godot checks original sail atlas identity, all protected move/bud rows, all body/bud
layer pixels, layer/source contract and authored endpoints. Every other copied pack
file is byte-identical across variants. Renderer checks 576 clip-end/handoff-boundary
comparisons at epsilon 1e−7 (<1e−5 linear channel difference), finite/nonzero output,
held-time exact bytes, and ≤9 px sprite extent. Handoff checks are sampler blend
endpoints across all 12 directed state pairs; they do not claim every corresponding
biological transition occurred in the actual two seed runs. The latter exercise the
real meal controller and its actual transitions independently.

`cargo test` passes **9 executions, 7 unique tests** (two reused net tests run twice),
plus the render/bake and cross-run contact assertions above. Logs:
`captures/sail-calm-bake.log`, `sail-calm-render-final.log`, `sail-calm-tests-final.log`;
four `sail-calm-{brace,settle}-meal-seed{1,8}.log` files. Godot's initial wrong-path
invocation failed before work; successful bake emitted the known user-log permission
warning but exited0 and passed all assertions. No environmental failure is counted
as a biological or visual failure.

Evidence SHA256:

- measurements: `2b6719fcf113648e8d24641c70bee1d509c329778c162112026379b1ba4a754a`
- brace creatures PNG: `3b1c3e44483b711928e984b8ad2c5c6695b2e26756679961022ffb11b489bf22`
- control creatures PNG: `21ee9b341238304d1e8972f4b104d3325d595b0e431ebd22592a1721e12285de`

Next bounded step: Fable reviews the native `render-final/viewer.html` rest adjustment
and both `sail-calm-meal-review-seed{1,8}-2026-09-13/viewer.html` worlds, then either
adopts **brace's four fin tracks only** into the current source and regenerates the
two sail rows, or rejects the concentrated gesture. Do not copy this historical full
candidate pack over newer crown metadata. Keep move/bud/body and all other rows exact.
No additional variant or sampling sweep is justified by this study.
