---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Rain response: temporal and native-64 review of v2, one bounded alternative, and restart

Follow-up to `fable-rain-response-2026-09-13.md` (candidate v2, commit `8dab65c`; root's
safety correction `16fb78b`). Question: does v2's quiver read at native 64 px, and does it
add the kind of sustained shimmer the reviewer wants less of, especially under a natural
drizzle and on the glowcap cap? One bounded alternative (v3) was built and measured on the
same fixtures. Nothing in production, the live host or the earlier study's frozen copies
and captures was touched.

**Recommendation: do not keep v2 as it is. Revise to v3 with the glowcap entry dropped,
and gate production on root watching the playback pages; if the mature-stalk lean is not
visible there at native size, defer the whole feature.** Reasons and numbers below.

- Review: `art/studies/rain-response-review/` — `presenter-v3.patch` (v3 on top of
  `presenter-v2.patch`), `rain_review_capture.rs` (paired capture with an optional
  restarted presenter), `rain_restart.rs` (reconstruction tests), `motion.py`,
  `playback.py`, `run.sh`.
- Frozen copies: `captures/build-cache/rain-review-src-{v2,v3}` (from `9cf0e1d`), targets
  `captures/build-cache/fable-rain-review-{v2,v3}`.
- Evidence: `captures/rain-response-review-2026-09-13/{v2,v3}/<fixture>/{old,new,restart}`
  native nets, `timeline.json`, `motion-*.json`, `sheet.png`; playback pages under
  `captures/rain-response-review-2026-09-13/playback/*.html`.

## What was compared, on identical fixtures

Three presenters draw the same stepped frames: **original** (response off, byte-identical
across v2 and v3: 3855/3855 frames), **v2** (committed candidate: 0.9 px quiver at 2.7 and
4.3 Hz on lanternstalk and reedspire, 0.7 px on glowcap, full level at 0.1 d/s, attack
0.3 s, release 0.8 s), and **v3** (same driver, level, families, budgets, root and
timing; only the motion shape differs: 60 % of the rain tip is a *steady lean* to the
slot's hashed side for as long as rain falls, the remaining 40 % a slow sway at 0.9 and
1.4 Hz). v3 is 57 patch lines over v2, passes the study's six tests unchanged, and stays
within the same budget bound because the two shares sum to at most 1.

| fixture | plants under rain | what it exercises |
| --- | --- | --- |
| young | seed 1 opening, tap over Left (1,10): 3 lanternstalk stage 1 + seedlings | real openings; restart 0.5 s after the last sample |
| mature | synthetic field at cap, Front (8,6): 5 lanternstalk stage 2 | ceiling of readability; restart at the peak |
| mature-restart-after | same, restart 0.25 s after the last sample | the dropped decay tail |
| glowcap | synthetic, Front (11,12): 4 glowcap stage 1–2 | the cap wobble |
| natural-sustained | seed 8 at its drizzle peak, 15 s, level 0.72–0.79, stalks stage 0–1 | the minutes-long case |
| natural-settle | seed 8, the drizzle's last 5 s and 5 s after | natural settling |

## Measured motion (crop of 24 px around the named cell, channel units of 255)

*static* is the mean |candidate − original| per frame and how many crop pixels change by
≥ 16; *added flicker* is the frame-to-frame change of (candidate − original) at 60 fps,
i.e. the motion the response adds on top of the streaks, which cancel; *original flicker*
is the streaks and wind alone, for scale.

| fixture (window) | static mean v2 / v3 | px ≥ 16 per frame | max | added flicker mean v2 / v3 | added max | original flicker mean / max | dominant Hz v2 / v3 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| young, whole shower | 0.09 / 0.14 | 0.6 / 1.3 | 65 / 61 | 0.04 / 0.03 | 29 / 17 | 0.61 / 81 | 2.7 / — |
| young, peak 2 s | 0.21 / 0.34 | 1.4 / 3.5 | 53 / 48 | 0.09 / 0.06 | 24 / 17 | 0.81 / 68 | 2.9 / 1.0 |
| mature, peak 2 s | 1.48 / 1.84 | 21 / 28 | 96 / 92 | 0.58 / 0.21 | 42 / 17 | 0.90 / 53 | 2.9 / 1.5 |
| glowcap, peak 2 s | 1.50 / 1.93 | 16 / 23 | 65 / 67 | 0.57 / 0.20 | 36 / 12 | 0.89 / 76 | 2.9 / 1.0 |
| natural-sustained, 15 s | 0.12 / 0.15 | 0.1 / 0.0 | 36 / 19 | 0.06 / 0.04 | 13 / 4 | 0.68 / 52 | 2.7 / 0.3 |
| natural-settle, 10 s | 0.00 / 0.00 | 0 | 0 | 0 | 0 | 0.91 / 131 | — |
| young, 1 s before the tap | 0.00 / 0.00 | 0 | 0 | 0 | 0 | 0.27 / 28 | — |

Reading: v3 changes a still frame more (static up 25–40 %) and moves it less between
frames (added flicker down 60–65 % in mean and in worst step, dominant frequency from
~2.9 Hz to 1.0–1.5 Hz). On the real young stalks and under the natural drizzle both are
below the streaks' own motion by an order of magnitude; on mature stalks v2's added
motion is two thirds of the streaks' and v3's a quarter. A natural drizzle tapers to
0.004 d/s over minutes, so there is no settling moment to see at its end (level ≤ 0.05).

## What the crops show (judged at 4× nearest, native rows and playback not judged)

Old and new crops look the same at a glance on every fixture; the response is a change of
shade on one or two edge pixels of each stalk, not a silhouette change. In v2 those
pixels alternate several times a second; in v3 they hold a shifted shade while it rains
and return afterwards. On the glowcap the diff is a thin band along the cap's rim in
both variants; the cap does not read as wobbling or leaning, only its edge shimmers. On
the young real stalks neither variant is visible in the crop. The native (1×) rows on the
sheets were too small for my tooling to judge; nobody has watched the playback pages;
nothing was shown on the cube. The claim "visible" therefore rests on the numbers and the
4× crops only.

## Presenter reconstruction while raining and just after (tests, both variants pass)

Presentation history is not persisted; a brand-new presenter snaps its level to the rain
it first sees. Measured with `rain_restart.rs` on a grown synthetic field, where a restart
with no rain changes nothing near the stalk (0.000), so all differences are the
response's own:

- **Restart in saturating rain:** the fresh presenter holds level 1.0 against the
  continuous one's 0.9989; the rain part of the frame is 0.081 vs 0.081 (v2) and 0.308
  vs 0.309 (v3) of 1.0. Same picture.
- **Restart while the level is still rising** (weak rate, target 0.5): the snap jumps
  ahead by 0.30 of level; the rain-part difference is 0.003 (v2) / 0.041 (v3) of 1.0 and
  the levels meet within 0.85 s.
- **Restart 0.25 s after the rain ends:** the fresh presenter has no level and *is* its
  own still image; the continuous one is still at level ≈ 0.73 and settles. The cut tail
  is at most 0.164 (v2) / 0.107 (v3) of 1.0 in any channel, shrinks, and both agree from
  3.15 s after the rain. In the captures the same restart differs by 13/255 (v2) / 8/255
  (v3) on its first frame and is identical after 2.15 s.

Restart therefore nearly matches after sustained saturating rain, but is **not exact
throughout rain**: a restart during the rising level skips part of the attack. It is
also **not exact in the tail: the decay is dropped and the plant is cut to still**,
a one-time bounded difference in the direction of quiet. On a
real opening a restart already changes 1900 net pixels by ≥ 8/255 for reasons unrelated
to rain (growth snapped to target, bodies re-posed), which dwarfs the rain tail. No
schema change is proposed to persist the level; the cut is preferable to a persisted
history for a presentation detail.

## Recommendation, explicitly

1. **Do not keep v2.** Its readability on mature stalks comes with added motion at
   2.7–4.3 Hz that is two thirds of the streaks' own, sustained for as long as it rains;
   under a natural drizzle that is minutes. This is the shimmer to reduce, not add.
2. **Revise to v3, minus glowcap.** Same driver (`RenderView.rain`), same rooted
   geometry, no ecology change, more change per still frame, and less added motion.
   In the mature peak crop its added-motion mean is about 36% of v2's (0.21/0.58),
   or about a quarter of the original streaks-and-wind motion (0.21/0.90), not a
   quarter of v2. The slower sway is a candidate for less visible flicker. Drop the glowcap
   entry: on the cap the response is only rim shimmer in both variants. The recommended
   package is `presenter-v2.patch` + `presenter-v3.patch` with `RAIN_RESPONSE` reduced to
   lanternstalk and reedspire (one line; unmeasured as such, but the glowcap entry does
   not affect any lanternstalk fixture above).
3. **Gate on watching, then decide keep or defer.** Open
   `captures/rain-response-review-2026-09-13/playback/mature.html` and `young.html`
   (columns original / v2 / v3; native at 2× and a crop at 4–8×; pause, step, speed).
   If the mature stalks' lean is not visible at native size in playback, **defer**: at
   64 px the streaks and water already carry the shower, and a response nobody can see is
   not worth a production change.

## Commands

```
art/studies/rain-response-review/run.sh build v2 ; run.sh build v3     # frozen copies already prepared
art/studies/rain-response-review/run.sh test v2  ; run.sh test v3      # 6 study + 4 restart tests each
art/studies/rain-response-review/run.sh capture v2 ; run.sh capture v3 # refuses existing outputs
art/studies/rain-response-review/run.sh measure ; run.sh pages
```

To reproduce from scratch, set `RAIN_REVIEW_SRC`, `RAIN_REVIEW_TARGET` and
`RAIN_REVIEW_OUT` to fresh paths; `prepare v3` builds the v3 copy from the base commit and
both patches. All output stays under `captures/` (ignored).

## Limits

- Stills and numbers only; no playback was watched and the cube was not used.
- Fixtures are one recorded opening per case plus a synthetic field; reedspire (water
  band) was never under rain in any fixture.
- The v3 lean side is the slot's hashed side, not the wind's; a lean along the wind
  might read better or worse, and was not tried (bounded to one alternative).
- The 4× crops are nearest-neighbour enlargements of the native net; the sheets' native
  rows exist but were not judged.
