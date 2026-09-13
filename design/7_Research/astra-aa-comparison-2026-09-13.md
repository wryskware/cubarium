---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Conditional antialiasing: native authored-coverage comparison

**Keep the production default.** A fixed 4×4 coverage bake reduces visible area
pulsing, especially on the sail, but softens important highlights and changes the
skimmer's apparent body substantially. This is useful evidence for a selective
source-art follow-up, not a justification for blurring the atlas or supersampling
the cube. No runtime, atlas, ecology, live process or shim was changed.

Lore/Graft retrieval and the actual source distinguish this issue from the already
addressed [animation roadmap](../animation-roadmap.md) timing work. This study
does not reopen Fable's selected Lanternjaw silhouette or its separate minification
filter.

## What is already filtered

- `art/bake.gd::raster`: transformed cutouts are sampled at each output pixel center,
  nearest source texel via `floor`; this part has **no spatial area integration**.
  Partial alpha from source art/modulation already survives the RGBA8 bake.
- `crates/cubarium-render/src/sprite.rs`: sRGB decode, premultiplied linear color,
  bilinear spatial sampling with transparent borders, blended neighboring atlas
  poses, one source-over per destination sample, and material-coordinate reveal
  masks/rooted inverse bend. That handles smooth subpixel placement and existing
  temporal blending; it cannot recover coverage absent from a baked frame.
- `crates/cubarium-render/src/multipart.rs`: the large Lanternjaw has its own
  scale-aware, blended adjacent box grids. Ordinary small `stamp_pose` sprites at
  scale 0.6 still use bilinear point sampling, not that area filter.
- Viewer enlargement is nearest-neighbor display of the same encoded pixels; it
  does not generate new detail or repair upstream flicker.

## Matched experiment

New [study helpers](../../art/studies/aa/README.md) render the actual lanternstalk
and reedspire `stage1` rigs (24 samples / 3 s), sail `move` (16 / 2.4 s), and skimmer
`move` (16 / 1.6 s). Baseline strips match shipped atlas rows **11, 34, 5, 13 byte for
byte**. The one alternative is an offline fixed 4×4 grid per texel, resolved in
linear premultiplied color; rig times, cutout sources, layer order and legacy
RGBA8 inter-layer compositing are identical. The small fixed grid still quantizes
coverage and is not a mathematical exact-area oracle.

A second fresh Godot invocation reproduced all eight baseline/candidate strips
byte-exactly. Godot could not write its default `user://` log under the sandbox;
the study output paths succeeded and both invocations exited zero.

Both atlases pass through the actual current `Pose`/`stamp_pose` and surface
helpers at 60 Hz: 360 frames each of rooted motion, smooth subpixel translation
and rotation, 0.6 scale, Front/Right crossing, reflected open-rim approach, a fixed
Top vertex placement, and a held pose. This is 4×7×2 matched sequences. Translation
of plants is a diagnostic sampling stress, not a proposed biological behavior.
The same source clip advances in both modes. The six-second excerpt replay cut
is excluded from differences; it is not an authored clip-loop endpoint test.

Complete sequences, native/enlarged viewer, representative boundary nets,
measurements and browser captures are saved under
`captures/aa-study-2026-09-13-render-v2/`; paired bake strips and timings are under
`captures/aa-study-2026-09-13-bake/`. Earlier render output is exploratory scratch,
not the reported result. Browser profiles and build products are not evidence.

## Results: fixed native anchor, moving authored clip

All values below use the full five-face linear canvas. Area is integrated alpha,
not a thresholded pixel count. “Second difference” is mean frame-to-frame change
of the luma difference, a temporal roughness proxy—not a perceptual flicker score.

| Rig | Alpha area range: current → coverage | Area standard deviation | Second difference change | Mean peak luma change |
| --- | --- | --- | --- | --- |
| Lanternstalk | 25.35–26.35 → 25.19–25.44 | 0.290 → 0.068 | −39.2% | −9.2% |
| Reedspire | 24.00–24.00 → 23.92–24.04 | ~0 → 0.024 | −16.3% | −11.2% |
| Sail | 46.00–60.00 → 49.30–53.74 | 4.375 → 1.350 | −70.2% | −18.3% |
| Skimmer | 52.00–61.00 → 61.88–65.50 | 2.069 → 0.922 | −35.4% | −21.9% |

The sail's area standard deviation falls **69.1%**, while mean area changes only
**+2.1%**; this is the strongest reason to investigate its transformed wing edges.
Its mean >=0.5-alpha silhouette nevertheless contracts 50.75→49.28 pixels. The
skimmer gains **18.0%** mean alpha area and 55.53→59.56 pixels of half-alpha
silhouette. That is a materially different look, not a free smoothness improvement.
The reed already conserves area; adding coverage introduces slight area variation.

Native browser crops at frames 0/45/90 remain recognizable. Enlarged crops show
rounded/dimmer sail highlights and more diffuse skimmer limbs and antennae. The
current sharper pixels retain useful readable structure. The candidate's luma
roughness reduction alone is therefore insufficient for adoption. Visual review
used decoded static frames and synchronized browser sampling, not human
room-distance viewing of a physical cube or a subjective continuous-video study.

Subpixel/rotated and seam runs retain the same trend: sail second differences fall
about 64%, but softened peaks remain. At 0.6 scale, sail area standard deviation
falls 1.688→0.612; a coverage bake is still not a general minification solution.
Rim scenes intentionally lose geometry beyond the open surface; their area
changes cannot be interpreted as sampling loss alone. The fixed vertex case and
boundary nets are representative checks, not an exhaustive owner/transport proof.

## Roots, support, quiet frames, cost

All 80 source poses in each mode load under the existing 9 px stamp budget.
Maximum extents, current→candidate: lanternstalk 7.878→8.856, reed 8.171→8.171,
sail 6.355→6.730, skimmer 8.171→8.724. Do not infer wind headroom by subtracting
these from nine: the actual material-profile support check matters. For the tested
plant clips its `bend_headroom(1.5,13,0)` remains 7.845 / 11.432 respectively.
This is **not** certification of all growth/fruit stages or an applied-wind capture.

Root anchors never move in the rooted case. Reed root-band coverage stays constant
in both modes; lanternstalk's bottom/support band has zero baseline variation and
candidate alpha standard deviation 0.00087. That tiny added coverage variation
must not be described as improved rooting. All four rigs' paired held-pose runs are
finite, nonempty and byte-exact across time within each mode. The alternative does
change the static image; there is no claim of baseline/candidate byte identity.

Release paired microbenchmarks (six measured rounds after warm-up, clear/encode/IO
excluded) at the rooted anchor, microseconds per stamp:

| Rig | Current | Coverage | Change |
| --- | --- | --- | --- |
| Lanternstalk | 4.741 | 5.019 | +0.278 |
| Reedspire | 4.943 | 4.953 | +0.010 |
| Sail | 3.016 | 3.440 | +0.425 |
| Skimmer | 4.858 | 5.622 | +0.764 |

Runtime shader/filter work is unchanged; extra supported texels/query extent still
cost time. Seam costs are higher in both modes. These shared-machine microbenchmarks
are not a full populated-world 16.7 ms admission test. Offline baking costs about
16–18× for these tiny clips (25–33 ms per candidate clip versus 1.4–1.8 ms baseline),
which is not itself a runtime problem.

An isolated local Chromium run sampled 362 animation callbacks over 6016.4 ms:
60.003 Hz, p95 16.8 ms, maximum 16.8 ms, all 360 frame indices visited. This checks
the comparison viewer's cadence only. Five focused tests pass (three study tests,
two shared net-layout tests); full runs also assert finite light, nonblank coverage,
valid sprite construction, no travel fallback and held-time byte identity.

## Disposition and the next testable step

1. **Reject blanket 4×4 coverage baking as the production default.** Do not add
   whole-cube supersampling, screen-space blur, temporal accumulation, or jitter.
2. Keep the reed and current skimmer unchanged. Lanternstalk gain is modest relative
   to softened detail; it does not outrank clear authored interaction/growth work.
3. If pursuing this ticket, make one **sail-only source/bake study**: preserve a
   crisp static face/body layer while integrating the rotating wing layers before
   compositing. Compare against the same fixture and quiet frames; aim to retain
   most of the area-variation improvement without the measured 18% peak loss.
   Do not simply sharpen or brighten the final frame to conceal changed coverage.
4. Before any asset admission, review every sail state and inter-state endpoint,
   scales used by the actual presenter, face/seam/rim output, and populated-world
   frame timing. Run an on-cube visual check separately. The existing helper is
   a reproducible gate, not approval to alter the pack.

## Source provenance

Study started at main `ad8c7a7`; the independent Cargo package excludes concurrently
edited host/meal code. Godot 4.7.2. Relevant exact git blob IDs:

| Source | Blob |
| --- | --- |
| `art/bake.gd` | `5db18ff06195fc0ee094ef6eaa466ca5b83fd1ce` |
| `crates/cubarium-render/src/sprite.rs` | `a56ca08f4647a4b165e96e4747c95101bc26421e` |
| `crates/cubarium-render/src/multipart.rs` | `a6aa7bac5b26e9b923e357238b0acc1323f2c9cb` |
| `crates/cubarium-surface/src/raster.rs` | `87e28ae170a05527eedcab317ffd2c3620c71f48` |
| `assets/atelier/pack.json` | `2925eaf4463451f2c9b39c027c6ab2b9d841097d` |
| `assets/atelier/creatures.png` | `8393bd36683f08f47d9a4f9e48666dcc4ac239f5` |
| `assets/atelier/plants.png` | `9de58a7d43f8fec3c4a4d53ee4c2f79bac03936e` |
