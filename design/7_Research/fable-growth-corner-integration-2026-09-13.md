---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Integrating the retained final-position cap owner (bounded, opt-in)

Root approved bounded implementation of Astra's growth-corner candidate (`b0523e3`,
reviewed in `fable-growth-corner-review-2026-09-13.md`). This report records exactly what
was changed, what was tested, what was captured and what remains unverified. Nothing here
is new canon; no generic surface behaviour, physics, schema, world or live process changed.
Root reviews, builds the release and deploys.

## What changed (production)

- `crates/cubarium/src/art.rs`: `TallPlant.corner_cap_owner: bool`, the versioned selector
  `CORNER_CAP_OWNER_V1 = "final_position_v1"`, and the allow-list
  `CORNER_CAP_OWNER_HOSTS = ["spiretree", "glasscane"]`. The loader accepts the selector
  only on a `crown` row, only with that exact string, only if the row loops, only on a
  listed host, and only if the plant has a crown (so a cap). Anything else is a load error
  with a named reason, never a silent fallback. An absent key is `false`.
- `assets/atelier/pack.json`: `"corner_cap_owner": "final_position_v1"` on the spiretree
  and glasscane crown rows (rows 2 and 5). No atlas byte changes; the vinecoil row is as it
  was. **The baker does not yet emit this key** (`art/bake.gd` adds only `vine_strips`), so
  the next rebake would drop it unless root adds the matching line there; I did not edit
  the baker.
- `crates/cubarium/src/art_present.rs`, private `draw_column`: the cap stamp, and only the
  cap stamp, passes through the studied gate: plant flagged, column `cx` in {0, 1, 14, 15},
  cap centre `v < CORNER_CAP_HANDOFF_V` (10, i.e. `height > 7`). Then the owner is
  `(u, CORNER_CAP_FINAL_V)` = `(u, 2)` and the existing `stamp_pose_in_chart` composites
  the same pose, heading, bend, opacity and mask about the actual centre. Otherwise, and
  whenever owner equals centre (height 9), `stamp_layers_bent` exactly as before. Base,
  trunks and vines are untouched; layer order is unchanged; nothing is cached.
- `crates/cubarium/src/corner_cap_present_tests.rs` (new, 11 tests, included from
  `art_present.rs` under `cfg(test)`).
- `crates/cubarium/tests/art_wind.rs`: one line, `corner_cap_owner: false`, in its
  hand-built `striped_tall()` literal so the suite compiles with the new field. No
  behaviour change; reported because it is not my file.

Cost, measured with the review's probe (pinned original vs the candidate): ≤ 13 µs per
corner column at the costliest heights, no change for interior columns.

## Tests (all run foreground, target dir `captures/build-cache/corner-cap`)

`cargo test -p cubarium --lib`: 240 passed, 2 ignored. The new module:

1. shipped pack flags exactly the two host caps; a manifest without the key loads
   texel-identical clips, tail rows and vine pieces (flag only);
2. **the unflagged path still reproduces the original pop** on selected Left0 and the
   two forced spires (> 0.15 linear at height 53/6, on the Top face), and removing the
   cap removes it (causal control retained as a red case);
3. flagged columns are bit-exact to the unflagged path at heights ≤ 7 and on every
   interior column at every height;
4. flagged columns are continuous (< 1e-6) at the handoff and at 53/6, 8.5, 8.75 with
   the vine drawn;
5. handoff continuity for every authored frame and midframe (48 phases), all four
   corner slots, all four sides, both host caps, at ± the full family bend budget, and the
   mature endpoint bit-exact to the original at every one of those (4608 cases);
6. mature light preserved on every selected corner column;
7. every flagged cap pixel lies within nine physical pixels of the centre (> 10 000
   pixels checked) and `extent + budget + 8 ≤ 32`, the surface helper's query limit;
8. the Top-corner pixel trace from the review holds on production code: original lights
   Top (1,0), (2,0), (3,0) in single frames at 8.5, 8.75 and 53/6; flagged reaches the same
   values a quarter-height early via the crown's dim rim, identical at 8 and 9, and every
   difference over the 241-frame sweep is on Top (1..4, 0);
9. the flag changes nothing the presenter observes: paced tall heights, targets, stages
   and column lists are equal between flagged and cleared presenters over 400 ticks;
10. the shipped manifest carries the selector on the two crown rows only;
11. unknown or malformed selectors, a trunk row, a non-host name, `loop: false` are
    rejected with the named reason; removing the key from one row turns only that cap off.

Targeted host suites, all green: `art_wind` (21), `art_wind_top` (5), `art_motion` (26),
`art_plants` (13), `art_growth_clip` (9), `art_growth_pack` (12), `astra_wind_regressions`
(6), `astra_motion_regressions` (4), `run_present` (2), `art_mode` (25), `art_bands` (6);
`art_wind_capture` and `animation_load` are ignored-only. No quiet-harness or core suite
was touched or needed: nothing outside the presenter's cap stamp changed.

**Astra's study suite now fails one test by design of its extractor**
(`art/studies/growth-corner`, unchanged): its `build.rs` extracts `draw_column` from the
*working tree*, which now carries the capability, so its "original" no longer pops and
`selected_column_retains_the_discontinuity…` fails (5 passed, 1 failed). I did not touch
those files. The honest fix is to pin the extraction to `a9eb064` via `git show`, which is
what my review package's `build.rs` now does (its cost probe still compares the true
original against the candidate). Root's call for Astra's study.

## Paired capture through the actual host presenter

Tool: `art/studies/growth-corner-fable-review/capture.rs` (`corner_cap_capture`): two
`ArtPresenter`s observe the same views and draw the same frames, one with the shipped
pack, one with the capability cleared in memory; real wind packets and art phase from
`present_seconds`; native nets, 8× seam strips and a per-frame timeline.

**No recorded opening traverses the corner.** Scanning an hour of world time (72 000
ticks) of the seed-1 and seed-8 openings, no selected corner column (Front14, Back14,
Left0) exceeds height 3 (Left0 on seed 8 reaches 3 at 21 900 ticks). So the capture uses
the presenter's own paced growth (`TALL_GROW_PX_PER_S`) on a synthetic rich field where
the chosen column's cells turn rich at tick 500: **a synthetic trajectory, not ecological
footage or actual cadence**, and ecology was not tuned to make it occur.

| capture (`captures/growth-corner-fable-review-2026-09-13/…`) | column | heights | wind amplitude | old path steps ≥ 64/255 | new path | old vs new |
| --- | --- | --- | --- | --- | --- | --- |
| `integrated-left0-wind` (423 frame sets, ticks 860–1000) | Left0 glasscane+vine | 6.77 → 9.00 | −0.042 … 0 (this column's host maximum is ≈ 0.05 px: its budget is the vine's) | 100 at h 8.756, 88 at h 8.850 | none; largest 61 = ordinary reveals, identical in both | 160 frames differ, ≤ 2 px, only Top (1..4, 0), h 7.78–8.89 |
| `integrated-back14-wind` (423 frame sets) | Back14 spiretree, no vine | 6.77 → 9.00 | −0.27 … 0 (its maximum ≈ 0.33) | none (this cap never switched) | none | byte-identical throughout |

Under the host's real phase the old pops fall at 8.756 and 8.850 rather than the fixed-phase
study's 8.5/8.75/8.83, which is the ownership switch landing on different cap frames; the
flagged path shows none. Wind at the Left0 pop instants was 0 (a lull in a ≈ 0.05 px
column); the study tests cover ± full budget statically, and Back14 climbed under
−0.27 px with zero difference from the original. Sheets: `top-corner-pop-12x.png` in the
Left0 capture, per-frame `strips/`.

## Visual limits

- Stills and numbers only; no playback watched, nothing on the cube.
- The synthetic field is rich everywhere, so the Top corner crops are busy; the pop and
  its absence are single top-row pixels, best read from the timeline and the pixel trace.
- No recorded world grew a corner column through 7–9 within an hour; a real traversal
  remains uncaptured.
- The 8× strips' "top" region is the Top pixel nearest the column's top anchor by
  embedding, which for Left0 is the Top (0,0) corner.

## For root

- Add `corner_cap_owner` emission to `art/bake.gd` for the two host crowns before any
  rebake, or the manifest key is lost.
- Decide whether Astra's study extractor should pin `a9eb064`.
- Release: schema unchanged; the manifest change is additive and ignored by older readers.

## Commands

### Root release-readiness follow-up

The two named integration gaps are closed. The shipped spiretree and glasscane
scenes now explicitly carry `corner_cap_owner` metadata; `art/bake.gd` emits it
only for that opted-in crown and refuses an unsupported selector or host. A
custom scene without the metadata stays unflagged. The manifest's key order now
matches the baker's deterministic output. A fresh headless bake under
`captures/crown-owner-rebake-verified-2026-09-13` is **byte-identical for all five
atlases and pack.json**, with no error in its log. An earlier trial's missing-meta
log was corrected by checking `has_meta` before reading; its output is retained.

Astra's original study now pins just its extracted private column body to
`a9eb064`, preserving the original failure even after production changes; current
relative helpers/assets remain explicit dependencies. Root reran all six study
tests: six pass in 36.94 seconds, including the original red-case assertions.
No original capture was rewritten. These checks do not replace the clean-release
workspace test or copied-world/live rollout verification.

```
CARGO_TARGET_DIR=captures/build-cache/corner-cap cargo test --offline -p cubarium --lib corner_cap
CARGO_TARGET_DIR=captures/build-cache/growth-corner-fable-review cargo build --offline --release --manifest-path art/studies/growth-corner-fable-review/Cargo.toml
captures/build-cache/growth-corner-fable-review/release/corner_cap_capture --scan --world captures/hunter-openings-2026-09-13/seed-8/world-144000.cubw --ticks 72001
captures/build-cache/growth-corner-fable-review/release/corner_cap_capture --synthetic --grow-from 500 --from 860 --to 1000 --face 3 --cx 0 --out OUT
```
