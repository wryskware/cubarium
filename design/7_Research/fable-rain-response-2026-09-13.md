---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Local rain response for small plants: one candidate, measured at 64 px

A *physical presentation* candidate: plants standing under rain that actually falls quiver
a fraction of a pixel and settle when it stops. It is not an ecology change and claims no
improved ecology. Everything here was built and measured in an isolated source copy of
release `9cf0e1d`; no production source, atlas, core, UI, live process or state was touched.

- Study: `art/studies/rain-response/` — `presenter-v2.patch` (the candidate),
  `presenter-v1.patch` (the retained first attempt), `rain_response.rs` (tests),
  `rain_response_capture.rs` (paired capture tool), `sheet.py`, `run.sh`.
- Evidence: `captures/rain-response-2026-09-13/{v1,v2}/…` (paired `old/`/`new/` native
  256×128 nets, `timeline.json`, `stages.json`, sheets). Build tree
  `captures/build-cache/{rain-study-src,fable-rain}`.
- Reproduce: `art/studies/rain-response/run.sh prepare && run.sh test && run.sh capture
  && run.sh sheets` (foreground; all output under `captures/`).

## Current implementation (what rain already does on screen)

`RenderView.rain` is the per-cell rate that fell *this tick*, in depth/s: the natural
weather term (`rain_rate` 0.6 × the moisture blob's excess over `rain_threshold`) plus the
active Standard shower's share (4 depth over 120 ticks, two graph hops, `apply_care`). The
presenter draws it as streaks, `ceil(rate × 3)` per cell capped at 6, so any nonzero rain
already shows at least one streak. The care flourish layer adds receipt effects for Feed
and Clean only; Rain has none because its receipt only schedules water. Plants take the
shared wind bend and nothing else. Measured on the recorded openings
(`captures/hunter-openings-2026-09-13`, 20 Hz):

| what falls | per-cell rate, d/s | footprint | duration |
| --- | --- | --- | --- |
| Standard shower, interior target | 0.235 centre, 0.118 one hop, 0.078 two hops (peak, t+3 s) | 13 cells | 6 s, smooth in and out |
| Standard shower, rim target 2 | 0.320 peak | 9 cells | 6 s |
| natural, seed 1 | ≤ 0.066 | ≤ 45 cells | one run of 5.1 min in 30 min |
| natural, seed 8 | ≤ 0.058, ≤ 0.086 | ≤ 16, ≤ 71 cells | 1.1 min, 7.8 min |

So natural rain is a slow drizzle that builds over minutes at a third of a shower's centre
rate; an applied shower is a short, local, three-times-harder burst.

## Proposed mechanism (the candidate, `presenter-v2.patch`)

**Driving signal: `RenderView.rain` itself**, the delivered rate, never the requested
dose. A tap that lands on a cell drives that cell exactly as natural weather at the same
rate would; the receipt is not consulted. It differs from the streaks in that the streaks
are the rain and this is the plant's answer to it, and from the flourish in that it is
not a receipt: it starts when water reaches the cell, follows the shower's envelope, and
outlasts it by a couple of seconds.

- Per cell a *quiver level* in [0, 1]: target `min(rate / 0.1, 1)`, first-order attack
  0.3 s, release 0.8 s, snapped to exactly 0 under 0.02 once no rain falls. Held with
  `prev/cur` like growth and interpolated by the frame fraction, so 60 Hz frames between
  20 Hz ticks are continuous. A repeated observation of the same tick moves nothing; a
  fresh presenter or a rewind snaps the level to the rain it sees (no invented onset).
- Per slot the quiver is added to the wind's *rooted horizontal `Bend` amplitude*:
  `tip · slot.wind · level · patter(t, phase)`, patter two weighted sines at 2.7 and 4.3 Hz
  with a per-cell hashed phase. Same root (1.5 px fixed rows), same 13 px length, same
  nine-pixel footprint and the same seam-carried stamp as the breeze; top-face radial
  species and tall columns are untouched, so nothing turns, nothing slides.
- Budget: `rain_tip = min(asked, budget/(1 + 0.10) − effective wind tip)` per family, from
  the measured art headroom. Asked: lanternstalk 0.9 px (budget 3.23, wind 0.45),
  reedspire 0.9 (4.38, 0.70), glowcap 0.7 (2.40, 0.12). Wind plus rain stays under a third
  of every budget.
- At level 0 the call is `slot_wind` bit for bit, so a quiet cube takes the identity path.

Excluded families and why: **tendrilfan** (0.31 px art budget, the wind already takes
0.29); **umbrellafrond / bloomcrown** (radial canopy: they turn in place, a bend would
translate a crown); **rootveil** (a crust); **all tall columns** (spiretree, glasscane,
vinecoil host: a vine must never slide against its trunk). Reedspire is the water-band
reed and only exists in flooded cells; none stood in the captured windows.

## Measured results

Tests (`run.sh test`, 6/6 pass in the isolated copy): no rain anywhere → old and new
canvases identical bit for bit across 700 windy and calm ticks; level rises to 63 % in
0.3 s, holds through a 6 s shower, settles to exactly 0 in 3–4.5 s; a repeated
observation is idempotent and a fresh presenter agrees with a rewound one; wind + rain
never exceeds any family's budget and still/radial families never quiver; a rained mature
lanternstalk changes no channel by more than 0.174 (of 1.0) between 60 Hz frames, and
presenters that observed the same ticks draw the same frame whether or not one held
frames; seam, top-vertex and rim rain draws bounded pixels, moves side-face plants at the
seam and nothing on the top face; the lowest rained pixel is 6.2 px below the anchor,
above the fixed root rows.

Captures (old = candidate off, new = on; same stepped world; numbers are for a 24–28 px
crop around the named cell, channel differences out of 255):

| case | plants under the rain | old vs new at peak | settle |
| --- | --- | --- | --- |
| v1, Standard at the harness's targets 0/1/2 (seed 1) | all three targets lie in the soil band and are **bare** (`stage: None`) but one seedling lanternstalk | 0–1 px, ≤ 1/255 | — |
| v2, same targets | same | ≤ 3 px, ≤ 2/255 | — |
| v2, tap over grown stalks, Left (1,10), stage 1 | 3 lanternstalk stage 1 + seedlings | 40–43 px, 12–21/255 | 22 px at +0 s, 6 px at +2 s, identical from +2.5 s |
| v2, natural peak, seed 8 Left (4,0), level 0.72–0.79 | 19 lanternstalk stage 0–1 in the run | 39–65 px, 9–16/255 | (drizzle continues) |
| v2, synthetic mature field, Front (8,6) | 5 lanternstalk stage 2 in the 3×3 | 214 px, 53/255; consecutive ticks 25–82/255 | 128 px at +0 s, 56 px/2 at +2 s, identical at +2.4 s |

Quiet frames: 60/60 before every shower and 153/153 after settling are byte-identical
between old and new. Frame-to-frame steps at 60 fps over the peak: worst channel step 60
(new) vs 53 (old) on the mature field — the falling streaks dominate the motion in both.

What the sheets show (`v2/*/sheet-*.png`, native and 4× nearest): on mature stalks the
tips shift an edge pixel's worth of light back and forth at a few Hz under the streaks,
local to the rained cells, and go still over about two seconds after the last streak. On
stage-0/1 plants the rooted profile (full amplitude only at the top of a 16-row tile)
gives ≤ 0.2 px of tip travel, visible in the diff row and barely in the crop. **The first
candidate failed** (v1: half the amplitude, glowcap still, full level at 0.2 d/s): at the
care contract's own three targets it changed at most one pixel by 1/255, because those
targets sit in the soil band, which on the seed-1 opening is bare. The one bounded
revision raised the asked amplitudes, brought the glowcap cap in (its art has 2.4 px of
room) and set full level at 0.1 d/s so the whole 13-cell patch is near full at the
shower's peak. No further variants were made.

## Physical cube: unverified

Everything above is native-frame and 4× nearest-neighbour inspection plus pixel
measurement. Nothing was shown on the cube or on a live viewer; an edge pixel moving by
a fifth of its brightness at 3–4 Hz may read as a shimmer or vanish on LEDs at room
distance. The natural-rain case also matters for ambient quiet: a seed-8 drizzle would
keep its stalks quivering at level ~0.8 for minutes under visible streaks. If that is too
busy, the one-constant gate is `RAIN_RESPONSE_RATE` (0.15 halves the natural level and
leaves a shower's centre and first hop at full).

## Visual tradeoffs

- Readable only where mature small plants stand; where a shower falls on bare soil band
  (the harness's fixed targets on seed 1), the streaks and water remain the whole
  acknowledgement, as today.
- Young plants barely answer by construction: the same rooted profile that keeps roots
  still limits tip travel to the tile's upper rows.
- The glowcap cap wobble is new character (a cap, not a stalk) and is the reviewer's
  call; dropping it is one table entry.
- Sub-pixel, bilinear motion at 64 px is an intensity shimmer, not a silhouette change;
  the diff rows read clearly, the crops subtly.

## Remaining gate for production

1. Root previews the isolated build (`run.sh prepare` then the usual host from
   `captures/build-cache/rain-study-src` with `CARGO_TARGET_DIR=…/fable-rain`) on a
   copied world with a tap over grown stalks, and judges the glowcap entry and the
   natural-drizzle level.
2. If accepted: port `presenter-v2.patch` into production `art_present.rs` (three per-cell
   fields, the observe step, one changed call site, the pure functions), move
   `rain_response.rs` to `crates/cubarium/tests/`, README wind section gains one
   paragraph. No core, atlas or UI change is needed.
3. Not in scope and not claimed: any ecological payoff, creature response, receipt
   animation, or change to what rain deposits.
