---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review of the growth-corner crown-ownership candidate (Astra, `b0523e3`)

Reviewed: `design/7_Research/astra-growth-corner-review-2026-09-13.md`, the study under
`art/studies/growth-corner/` (README, `build.rs`, `main.rs`, `evidence.json`,
`viewer.html`) and its committed captures under `captures/growth-corner-2026-09-13/`.
Provenance re-checked: the working tree's `art_present.rs` hashes to the SHA256 the study
names and `git diff a9eb064` on the presenter, `art.rs` and `pack.json` is empty. Nothing in
production, the study, its captures or the live host was edited; my helpers live in
`art/studies/growth-corner-fable-review/` and write only to
`captures/growth-corner-fable-review-2026-09-13/`.

**Disposition: accept for bounded integration**, with the opt-in and custom-art guards
listed at the end. The candidate removes a real pop, changes nothing else in the image,
costs nothing measurable, and does not introduce a ghost crown or early growth.

## What was actually viewed

- The study's `selected-left-handoff-endpoint-8x.png` (heights 7, 8, 53/6, 9; original
  and candidate Top corner and Left crown) and `rejected-v10-owner-endpoint-8x.png`.
- My own 8× nearest strips (`frames/corner-strips-8x.png`, 19 frames across the sweep;
  `frames-pop/corner-strips-8x.png`, 15 frames around the pops) of three regions per
  frame: Top corner (Top 0..12 × 0..16), Left crown (Left 0..12 × 0..16) and the Back
  seam edge (Back 52..64 × 0..16), original row, candidate row, |diff|×4 row.
- Full paired nets at 3× for frames 0, 120, 180, 220, 240.
- Numeric pixel traces of the Top corner row for every frame where the pair differs.

Not viewed: the study's `viewer.html` in a browser. The repository-root development
server on port 5173 serves the viewer and the frame PNGs (HTTP 200 for both), but no
WebSocket client is available here to drive the isolated Chromium debugger, so no page
was opened and no playback was watched. Every judgement below is from stills and numbers.
The sequence is the study's synthetic 4 s linear height 7→9 sweep at fixed art phase and
zero wind; it is not ecological footage and says nothing about actual growth cadence.

## Measurements (`frames.py` over the 241 committed pairs)

| quantity | original | candidate |
| --- | --- | --- |
| frame-to-frame steps ≥ 64/255 | 3 (frame 180 h8.5: 179; 210 h8.75: 179; 220 h8.833: 121) | 0 |
| frame-to-frame steps ≥ 32/255 | 4 | 0 |
| largest step anywhere | 179 | 24 (ordinary quarter-height reveals at frames 1, 30, 31, 60, 61 …, identical in both) |
| first frame with a lit Top pixel | 61 (h 7.508) | 61 |
| lit Top pixels at frames 120 / 180 / 240 | 4 / 11 / 20 | 4 / 12 / 20 |

Original and candidate differ on 104 of 241 frames (121 to 224, heights 8.008 to 8.867),
never by more than 2 pixels per frame, and **every differing pixel across the whole sweep
is on the Top face's corner row: Top (1,0), (2,0), (3,0), (4,0)**. The Left face (crown
and vine) and the Back face (the crown's other half across the seam) are byte-identical
in every frame, so crown-to-vine registration is untouched by construction and by
measurement. Frames 0 to 120 (heights 7 to 8) and frame 240 (height 9) are identical.

The trace of what those pixels do is the whole story. In the original, Top (1,0) is black
until frame 180 and then (22,115,179) in one frame; (2,0) does the same at 210 and (3,0)
at 220. In the candidate the same pixels take the same final values, but each arrives the
way the pixel before it did: as the crown's dim rim (17,12,69) about 30 frames earlier,
ramping through (22,113,177) to the same value. The candidate's Top row at any frame is
the original's Top row shifted by one pixel of continuous travel; nothing is drawn that
the original does not also draw a quarter-height later, and nothing is lit that stays lit
into the endpoint differently. That is the opposite of a ghost crown, and it is not
surprising growth: the crown's top edge slides over the corner instead of appearing pixel
by pixel.

## Visual judgement of the crops

At 8× the original's Top corner gains a bright cyan pixel out of nothing at 8.5, again at
8.75 and 8.83; the candidate's corner shows the same cyan tip already dimly present a
quarter-height earlier and brightening. The Left crown (orange bud on the cyan cap beside
the checkered vine) and the Back seam half are the same image in both rows on every
strip. The mature crown at height 9 is the recognisable original. The rejected v10 owner's
sheet shows the missing mature Top lobe Astra describes; the final owner restores it.
Nothing is softened, brightened or shifted on the side faces.

## Cost (`growth_corner_cost`, release, single thread, 400 draws per sample)

| column | height | original µs | candidate µs |
| --- | --- | --- | --- |
| selected Left0 glasscane+vine | 7.5 | 138.5 | 151.0 |
| selected Left0 | 8.0 | 144.9 | 152.4 |
| selected Left0 | 8.5 / 8.83 / 9.0 | 156.0 / 156.8 / 157.1 | 159.6 / 157.7 / 157.0 |
| selected Front7 (interior, ordinary path) | 6 to 9 | 51 to 75 | identical |

All three selected near-corner columns together at height 8: 349 µs original, 369 µs
candidate. The larger owner query adds at most 12.5 µs to one column and about 20 µs per
frame in total; interior columns are unchanged. This is a probe measurement, not a host
frame budget, but there is no cost concern.

## Guards required for production, and what remains unmeasured

1. **Explicit opt-in per shipped cap asset**, not a family-name rule: a pack capability or
   a presenter-side list naming the two shipped host caps. A custom cap without the flag
   keeps the legacy path, so third-party art cannot silently pick up a corner owner whose
   support was never validated.
2. **Validate the opted cap at pack load or in a test**: single layer at scale 1 (the
   study's `candidate_stamp` asserts this), owner-to-centre distance ≤ 8, query radius
   ≤ 17 (the surface helper's limit is 32), and the study's dense handoff check (every
   authored frame and midframe, ± full family bend budget) ported as a host test.
3. **Keep the gate exactly as studied**: cap layer only, `cx` ∈ {0, 1, 14, 15}, only while
   the cap centre is above `v` 10 on its face; trunks, vines and bases untouched; generic
   shortest-chart ownership unchanged.
4. **Retain the original pop as a red fixture** (the study's first three tests) and add
   the Top (1..4, 0) pixel trace above as an expectation, so a later ownership change
   cannot silently reintroduce the switch.
5. **Rerun the full column wind, growth, root and quiet suites** after transplanting only
   the local owner choice, keeping `draw_column` private.
6. **Unmeasured, not blocking:** the temporal capture is zero-wind and fixed-phase; wind
   is covered statically at ± full budget by the study's tests but no windy paired
   sequence exists. One real-world paired capture of a recorded opening with the Left0
   column growing through heights 8 to 9 under wind would close that gap at integration.
   No playback was watched by anyone in this review, and nothing was shown on the cube.

## Commands

```
python3 art/studies/growth-corner-fable-review/frames.py captures/growth-corner-2026-09-13/selected-left-60fps OUT_DIR [--frames …]
CARGO_TARGET_DIR=captures/build-cache/growth-corner-fable-review cargo run --offline --release --manifest-path art/studies/growth-corner-fable-review/Cargo.toml
```
