---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Living-world next packages — progress record (Fable orchestrator)

Evidence for root's reconciliation, not decisions. Authorization:
`living-world-next-handoff-2026-09-13.md`; work order:
`living-world-next-brief-2026-09-13.md`; geometry advisories:
`astra-lanternjaw-production-plan-2026-09-13.md` and
`astra-lanternjaw-brief-review-2026-09-13.md`. Nothing here touches the simulation, saved
worlds, founders, the shim, `state/`, live processes or the no-art image. No deployment.
Wrysk reaffirmed the Lanternjaw choice; that selects the body, not a predator ecology.

Concurrent work in the same tree during this session — root's `runner.rs`, `sink/web.rs`
and `Cargo.toml`, Astra's `care_effects.rs`, the accounting session's `cubarium-core`
(schema 9) — is **not** in any commit below; every commit names its paths explicitly and
was serialized on `/tmp/cubarium-shared-care/git.lock`.

## Status

| package | state | commits |
| --- | --- | --- |
| Reed on a flooded top-face cell bends | delivered, tested | `b8b8a11` |
| Authored growth: nine side-species clips, pack v5 rebaked | delivered, tested, strips inspected | `5d7ea69` |
| Lanternjaw production rig, study route, captures, cost | delivered, tested, captures inspected | see "Commits" |
| Canopy top-down opening (umbrellafrond, bloomcrown) | **not authored** — the radial reveal mask remains | — |

## Package 1 — Lanternjaw production rendering

### Geometry (revised after Astra)

The first draft carried each part to its own anchor with `travel` and a rim re-pivot.
Astra's plan and follow-up review showed, with numbers checked against the compiled
surface crate, that two independent stamps near a top vertex read the same physical pixel
at two different body coordinates (Front (63, 1), heading (1, 0), part offset (3, 0),
pixel Top (63, 63): body (0.5, −1.5) from the root, (1.5, −1.5) from the carried anchor), so
a joint tears or a texel doubles, and that a travelled attachment reflects at the rim. The
shipped geometry is therefore **one root-owned `unfold_pixels` query per body**:

- `cubarium_render::stamp_rig(canvas, root, heading, states, opacity, scratch)` with
  `RigPart { sprite, offset, layer }`. The query radius is `rig_radius` = max over the
  active parts of `|offset| + sprite.extent()` plus `RIG_MARGIN` (0.5, covering the gap
  between the radial extent and a texel's true bilinear corner), bounded by the surface's
  `MAX_LOCAL_RADIUS` of 32; nine pixels stays the per-sprite material budget. A radius past
  32, or a non-finite one (a non-finite offset makes it NaN on purpose), panics in every
  build — a configuration error, never silently clamped (Astra §6.2).
- Parts with equal `layer` are one **material** and are summed (a hull cut into
  lattice-aligned pieces reconstructs the uncut hull exactly, because bilinear sampling is
  linear); layers composite source-over in ascending order; states are composed completely
  and then mixed, and opacity is applied once to the assembled body.
- The open rim needs no rule: the root query has no pixels past it, so a hanging part is
  cut where the surface ends. A top vertex shows the single localized cut any stamp shows.
- `Sprite::from_premultiplied`, `sample_at`, `can_reach`, `pivot`, `texel` were added;
  `stamp_rig_with_radius` is the doc-hidden generous-query oracle for tests.

### The body (`crates/cubarium/src/lanternjaw.rs`)

Code-native, eight per-frame rasterized parts (`FarLimb` 0, `Underside` 1, `Tail`/`Abdomen`/
`Thorax`/`Head` 2, `Glow` 3, `NearLimb` 4), the hull cut by template column with integer
offsets and pivots so all pieces share the body lattice. The study's template, rhythms,
envelopes, limb poses and colours are ported verbatim; colours are computed in sRGB exactly
as `fable.js` computes them and decoded once. The opaque structure keeps the study's mixes
toward `#0B0525` as opaque colours (Astra: dark shell and seams are intended); the fan,
lantern halo/glow, cocoon and far limb carry real alpha. Every position is unrounded (wave,
coil, lunge, limb joints, legs, anchor); the study's binary leg lift is the continuous
bump `sin(π u / 0.38)`. Not in the atelier pack; no fifth form; no founder; nothing in
`art_present.rs` selects it. `Mode::{Rest, Move, Hunt, Bud}` are choreography names only.

### Study route and captures

`cargo run --release -p cubarium --example lanternjaw_study -- --scene gallery|seams|walk
--ground black|study|soil|water --sink web|preview|png|shim [--web-port 7399] [--seconds N
--every K --out DIR]`. The gallery draws the four modes on Front beside the common
atelier lantern rig for scale; `seams` places a body across the Front/Right seam, one
crossing Right→Top, one rotating at a Top vertex, one heading into the bottom rim in `hunt`
and one facing −x on Back; `walk` travels a body through several seams at 1.5 px/s with a
`hunt` body on Top. Native PNG frames were captured for 12 s per scene and assembled into
×4 nearest-neighbour sheets: `captures/lanternjaw/{gallery,seams,walk,grounds}.png` and
`captures/lanternjaw/gallery.mp4` (12 s, 30 fps). Fable's reading of them: the hull, the
cyan chain, the fan and the folded limbs read at native scale and the body is about twice
the common rig; the strike shows in the 3.3–3.5 s frames with the warm accent; the seam
bodies are continuous across Front/Right and Right/Top; the rim body is cut at the bottom
edge, not reflected; the vertex body shows the documented localized cut and no doubled
hull; over soil and water the dark structure reads as a silhouette with the chain on it.
Desktop captures do not replace observation on the cube.

### Cost (release, this machine, with root's 12-hour runs active)

`cargo test --release -p cubarium --test lanternjaw_cost -- --ignored --nocapture`:

| place | mode | bodies | mean µs | worst µs |
| --- | --- | --- | --- | --- |
| mid-face | move | 1 | 41.6 | 246.4 |
| mid-face | move | 2 | 85.0 | 261.3 |
| mid-face | hunt | 1 | 39.9 | 46.3 |
| mid-face | hunt | 2 | 82.5 | 116.4 |
| seam | move | 1 | 49.0 | 53.9 |
| seam | move | 2 | 102.9 | 192.0 |
| seam | hunt | 1 | 49.5 | 74.5 |
| seam | hunt | 2 | 107.7 | 191.2 |

On a dense art-mode frame (200 organisms, mature fields, water, rain) the mean was
13.0 ms with 0, 1 or 2 bodies in Fable's run (increments −18 µs and −10 µs, inside the
noise of a loaded machine); the worker's single-threaded run measured 12.87 ms alone,
+146 µs for one body and +224 µs for two. The slice-2 figure for a comparable frame on a
quiet machine was 11.45 ms, so the baseline, not the rig, is what moved. A Lanternjaw
costs on the order of 0.3–0.7 % of the 16.7 ms budget; the worst single frames are
scheduler noise, not the rig.

Two implementation conventions worth knowing: a study coordinate names a *pixel*, so every
template cell is splatted at +0.5 on both axes (the hull is then symmetric about the
anchor, an integer anchor is the crisp phase, and `BOUND_BACK`/`BOUND_FRONT` are exactly the
fan's and the claw's texels); and `captures/` is gitignored, so the sheets and the mp4 are
kept on disk at the paths above and are not in any commit — regenerate them with the
example if they are gone.

### Tests

`crates/cubarium-render/tests/multipart.rs` (15) and `crates/cubarium/tests/lanternjaw.rs`
(16), written by an independent worker from the doc comments and the brief, including
Astra's retained fixtures: the marked-partition vertex case reads 0.5, not 0.75; the rim
fixture keeps Front (32, 59) as the flat placement does and no part pops; a one-part rig
equals `stamp_layers_bent_with_radius` at its own radius to within one f32 ulp — one
configuration of the sweep differs by float reassociation, every other is bit-identical (the plain
`stamp_sprite` differs only by the filter tail its legacy radius clipped — Astra §6.1);
opaque and translucent materials cut into two and four aligned pieces draw the uncut
sprite; layers order; state mixtures; seam light conservation to 1e-6 with matched lattice
phase; all four top vertices from each incident face; radius identity against 24; the
eight parts and layers; the lattice rule; footprint, `PART_EXTENT_MAX` 8 and
`rig_radius ≤ QUERY_RADIUS_MAX` 16 over 12 s at 60 fps in all modes; purity; hunt
periodicity; the warm accent only inside the strike envelope; blink steps; near limb over
the hull and far limb under it at the strike; a per-frame bound in `move`; heading (−1, 0)
is a half turn, not a mirror (Astra §5); sub-pixel anchor property; opacity applied once
without ridging an overlap.

Three properties the brief stated were corrected by the test pass rather than by the
implementation: heading (−1, 0) is a half turn about the anchor, not a mirror about its
column (Astra §5; the body is asymmetric about its mid-line, and the test asserts the mirror
does *not* hold); the "each pixel between its two integer-anchor neighbours" bracketing
bound is unsound because a half-pixel anchor lands on the body lattice's own breakpoint
(tested instead: a whole-pixel shift is a bit-identical translation, a half-pixel shift
changes the picture, and a single part changes by at most half its largest adjacent-texel
step); and hunt periodicity is texel-exact at every 60 fps instant of the cycle but only to
1e-6 at arbitrary decimals, because `seconds mod 6` is not exact in binary — the study's
"pixel-identical at t and t + 6" was earned by rounding, which fractional motion gives up.
Measured envelope of the suite: body x (−9.5, 13.5), y (−4.5, 4.5); worst part extent
6.908 of 8; worst query radius 15.47 of 16; the warm accent 10 frames at 3.25–3.40 s and
never outside `hunt`; the blink's largest per-frame step 0.275 of its swing; `move` mean
|Δ| per painted pixel 0.005 against a derived bound of 0.25.

Live-process host tests (`run_persistence`, `care_replay`) are load-flaky when several
`cargo test` invocations contend for the state lock at once; each passes alone. Not a
change of this session; noted for whoever runs the suite beside root's experiments.

### Living pose and juvenile scale (second slice, 2026-09-13)

Per Astra's `lanternjaw-ecology-animation-contract-2026-09-13.md`. The study's `Mode` and
`parts(seconds, mode)` remain the gallery; the ecology entry points are
`LivingPose { ambient, movement, attack: Option<AttackEpisode>, gut, cocoon }`,
`attack_channels(episode)` (a pure schedule over **attack time**: windup stretched over the
real windup, the cocked pose held through a strike until its final 120 ms cubic extension,
full contact reached exactly at the settlement boundary and **held past it**, the 200 ms
recoil only in `Recovering`/`Handling` and from the reach the previous phase actually
displayed, the far claw 45 ms behind in attack time and holding the previous phase's far
reach through the lag), `Channels` (the rasterizer's explicit inputs, produced either by
`Channels::study` — bit-identical to the gallery — or `Channels::living`), and
`Lanternjaw::draw_living(.., scale, ..)` which scales the **whole rig** through
`cubarium_render::stamp_rig_scaled` (body coordinates divided by the scale, query radius
multiplied by it; offsets, pivots, lattice, lunge and limbs shrink together). Admitted scale
`SCALE_MIN` 0.5 ..= 1; outside it, or non-finite, panics as a configuration error. No
cocoon without `Some(gestation)`; no gut breath without gut; a held real phase never strikes.

Delivered (commit named under "Commits"): `parts(seconds, mode)` is now literally
`rasterize(&Channels::study(seconds, mode))` and was proven bit-identical to the previous
rasterizer over 81 120 texels of a pre-refactor fixture (0 differing bytes) and, permanently,
over 567 840 texels by a unit test. `attack_channels` follows the normative schedule (the
strike accent rises from exactly 0 twenty milliseconds before the extension, inside the
hold — the one wording ambiguity the workers found, ruled in favour of the continuous
reading; at the study's keyframes reach/compress/lunge/charge/blink agree to 1e-9 and the
accent agrees within the phase that owns it). The study route gained `--scale` and a
`phases` scene: real phases held longer than six seconds (perched; stalking at movement
0.6; windup 0.6 s → strike 1 s → full extension **held** 2 s → recovering 5 s → handling
with gut 0.8; a funded escrow 0→1; a `SCALE_MIN` juvenile beside an adult). Sheets:
`captures/lanternjaw/phases.png` and `captures/lanternjaw/juvenile.png` (gallery at 0.5 /
0.7 / 1.0, and the seams scene at 0.6). Fable's reading: the attack row shows the
compressed charged coil, the cocked hold through the paid approach, the warm claw driving
out at 1.55–1.58 s, the extension visibly still with claws out at 2.58 and 3.58 s, the
recoil and the quiet fold after; the cocoon is absent at gestation 0 and fades up by 0.2;
the 0.6-scale body is continuous across the side/side and side/top seams and the top
vertex. Honest limits: at scale 0.5 the lantern chain reads as one lighter stripe (bilinear
minification of adult art; 0.7 keeps the rhythm), the 0.24 px gut breath is a brightness
change, not a silhouette, at 64 px, and the strike accent is over by the settlement frames
(`ACCENT_SECONDS` 0.24), so a held extension reads as a dark arm out front. The `seams`
scene's rim body is the study `Hunt` mode (adult), so the scaled rim cut is unit-tested but
not pictured.

Tests: `crates/cubarium/tests/lanternjaw_living.rs` (23) and
`crates/cubarium-render/tests/multipart_scale.rs` (12), independent; plus 15 in-module
lanternjaw unit tests; and `crates/cubarium/tests/lanternjaw_scale.rs` (5, Fable) sweeping
the core-admitted scales 0.2, 0.316227766…, 0.632455532… and 1: drawing at every scale, the
scaled footprint bound, light rising with scale near the area law, effectors linear to the
last bit with the near claw on the drawn limb, seam light conservation across Front/Right
and Right/Top at matched phase, one owner per pixel at a top vertex, and the rim cut with no
reflection.

**Minification is box-filtered, not point-sampled.** The first version of the four-scale
claw test had to widen its tolerance to half a source-texel stride (2.5 px at scale 0.2,
larger than the body) because one point sample per pixel simply missed a sub-pixel claw:
that would have admitted invisible, spatially fictitious contact. Instead
`stamp_rig_scaled` now averages `n × n` samples over each destination pixel below scale 1
(`n = ceil(1 / scale)`, offsets `(i + 0.5) / n − 0.5` along the body axes, query radius plus
`SUPERSAMPLE_REACH` 0.5), so a texel smaller than a pixel contributes its share of the
pixel's area; scale 1 is `n = 1` with a zero offset and stays bit-identical (the scale-1
identity sweeps pass unchanged). Measured on the settled strike: the pixel over the named
near claw carries near-limb light 0.026 / 0.092 / 0.252 / 0.236 at scales 0.2 / 0.316 /
0.632 / 1, the nearest painted limb pixel is 0.41 / 0.40 / 0.50 / 0.30 px from it (the test
tolerance is one pixel at every scale), and the light ratios track the area law (0.040 /
0.100 / 0.400 / 1.000 of the adult). Honest limit: at 0.2 the claw's light is 2–3 % of a
pixel — geometrically where the effector is, invisible on an LED — and a 0.2-scale body is
under four pixels long, a smudge with a brighter head. That is the core's admitted range
reported faithfully, not a claim of visible contact at the smallest sizes. Sheets made
after the range change and the filter: `captures/lanternjaw/juvenile-scales.png` (the
gallery at 0.2 / 0.316 / 0.632 / 1 and the seams scene at 0.6) and
`captures/lanternjaw/phases-min02.png` (the phases scene with a 0.2 juvenile beside the
adult); the earlier `juvenile.png` and `phases.png` (0.5 minimum, point-sampled) are kept
as they were. Fable's reading of the new sheets: 0.632 is a small but recognisable
Lanternjaw with its chain rhythm; 0.316 is an elongated violet blob with a pink head; 0.2 is
a two-pixel smudge with a brighter head; the 0.6 body is continuous across the Front/Right
seam and cut at the rim. Two of the tester's first-draft claims were their own errors
(the far claw is 45 ms *after* settlement, not at it — hence the `Effectors::far_claw`
wording; a walking leg's swing legitimately reaches the cocoon's rightmost column).

**Requirements reported back to the core / root's adapter (art is not shortened to fit
the placeholder):**

- The core trial's six-pixel jaw offset and 1.5 px reach are **not** the visible jaw. In
  adult body pixels (+x forward, +y to the clockwise side; painted texel centres, the
  lattice's +0.5 included, without the decorative wave): ingestion mouth `(8.5, 0)` folded
  and `(9.6, 0)` at full lunge; near claw at settlement exactly
  **`(13.279411764705882, 1.1)`** — `x = 12.3 + head_dx + 0.5` with `head_dx = −0.3 · (1 −
  13/17) + 1.1 · 0.5 = 0.4794117647058823`, `y = 0.6 + 0.5`; the far claw the same one
  pixel higher **45 ms after** settlement (at the boundary it is 62 % through its own
  extension, so a settlement check uses the near claw); a single grasp region enclosing
  both would be centred near `(13.0, 0.7)`. This is the agreed value (core commit `15e599d`
  and after adopt it); the earlier `y = 1.162368` was the decorated study measurement at
  one instant, wave included, and is superseded — the 0.062 px difference is inside any
  proposed tolerance but the exact-centres assertion must use 1.1.
  `cubarium::lanternjaw::effectors(scale)` returns these scaled. Any contact tolerance is
  the core's choice; a trial disk at `(6, 0)` r 1.5 sits behind the thorax and would
  capture visibly untouched prey.
- **Scale range reconciled to the core's** (Astra's hunter geometry review, gate 2):
  `SCALE_MIN` is 0.2, the trial's `body_scale_min`, so every valid `body_scale` (`max(0.2,
  (S / S_adult)^0.5)`: 0.316… at child fraction 0.1, 0.632… at the default 0.4) draws through
  the same whole-rig scaling as the effectors; nothing is clamped in the renderer alone. A
  non-finite scale still panics here; the core rejects NaN at admission separately. Tests
  cover 0.2, 0.316227766…, 0.632455532… and 1 for drawing, effectors, seams and coverage.
- Schema 11 `HunterView` (`phase_started_tick`, `phase_ends_tick`, `entered_from`,
  `episode`, `attack_counter`, `body_scale`, `ContactEvidence`) supplies what
  `AttackEpisode` needs: `elapsed` from `phase_started_tick` and the presenter's clock,
  `duration` from the ticks, `from` from `entered_from`, the episode key for continuity.
  The core's post-movement Handling/Recovering timestamps are currently one tick early
  (Astra gate 1); the renderer does **not** compensate — it draws the boundary it is given,
  and the fix belongs in core.
- Capture must be evaluated **once, at the strike's settlement boundary**, where the art
  holds full extension; during windup and the strike's hold the claws are folded/cocked and
  the effector is not there. If the core ever captures during extension, it must share the
  extension trajectory (`attack_channels` at the same attack time).
- The eight-pixel root-centred sense range and the pursuit stopping distance must be
  audited against a thirteen-pixel effector together (Astra's warning); the renderer does
  not and must not extend sensing.
- Adapter inputs: `elapsed = present_seconds(tick, f) − phase_started_tick · DT` (attack
  time, never a modulo), `duration` from the profile (`windup_seconds`, `strike_seconds`),
  `from` = the previous episode's `AttackChannels::reach()` at the instant of transition
  (keep the previous phase and settlement boundary through the one-tick interpolation
  interval, as the contract says), `movement` from real root speed over the profile's
  maximum, `gut = gut_material / gut_capacity`, `cocoon = HunterView.gestation`,
  `scale` from one authoritative mapping of structure (candidate `sqrt(S / S_adult)`,
  clamped to the admitted range) applied to `effectors` and the draw alike.
- Seam/rim contact must be evaluated by unfolding the prey from the hunter's **root** with
  the rig's own ownership (`unfold`), not by `travel`ling an effector offset, and never by
  reflecting an off-rim effector; the safe first policy is no capture when the grasp centre
  is off-surface, while still charging the attempt.

## Package 2 — Authored growth expansion

Commit `5d7ea69`. Nine hand-authored 4 s `grow01`/`grow12` clips for glowcap, rootveil,
lanternstalk (1→2 new), tendrilfan and reedspire, each in its own hidden `Grow<from><to>`
group reusing the stages' textures; `art/plants/author_grow.py` is the splice script;
the `.tscn` text is the source of truth. Pack stays v5 (34 plant rows, was 25). The bake
is byte-reproducible; every pre-existing stage/fruit row is byte-identical to the previous
atlas; the other four atlases are unchanged files; every species' wind budget and
limiting frame are unchanged. Conventions and per-clip descriptions are in
`art/PLANTS.md` "The ten authored steps": endpoints are RESET-neutral poses (for the two
rotating-sway species no loop sample has both pivots at rotation 0, so the presenter's
12 % endpoint blends carry the join — the pilot's stricter loop-sample property holds for
lanternstalk 0→1 only); every frame paints tile row 14 and never row 15 (the brief's
one-row root change for lanternstalk was a misreading — `stalk2` is nine rows, 6–14);
berries never appear; nothing fades to near-nothing before its replacement is up (a
correction Fable made after the first strips showed the 1→2 lantern vanishing to a dot).

Inspected: 8× strips of all ten clips in `/tmp/growth-strips/` (not committed; regenerate
from the atlas rows). Tests: `crates/cubarium/tests/art_growth_pack.rs` (11, generic over
every transition), `art_growth_clip.rs` adapted (9), the loader lists updated;
`cargo test -p cubarium` green at commit time.

## Reed on the top face

Commit `b8b8a11`. `slot_wind` turns a top-face slot in place only when its species has
`spin_deg > 0`; a reed in a flooded canopy cell bends along its own tile's horizontal axis
like a side-face plant, rooted at its ripple row. `crates/cubarium/tests/art_wind_top.rs`
(5). `art/README.md` "Wind" documents it.

## Commits, commands, artefacts

- `b8b8a11` reed rule; `5d7ea69` growth pack; `5a59b8c` Lanternjaw rig, study route,
  tests and cost test; `3f2182f` presenter doc qualifiers and the refined endpoint test;
  `56cf476` this record, the brief, the roadmap status and the README pointer; `4cffab7`
  the one-ulp qualifier and test-pass corrections.
- Second slice: `7b8ad4f` `stamp_rig_scaled` with box-filtered minification and its 12
  independent tests; `5554c48` the semantic living pose, real attack phases, the core's
  admitted scale range 0.2..=1, `effectors`, the `phases` scene and `--scale`, with the 23
  living-pose tests, the 5 four-scale tests and 15 unit tests. Whole render + host suite at
  `5554c48`: **559 passed, 0 failed, exit 0** (`cargo test -p cubarium-render -p
  cubarium`, run once in the foreground; log `/tmp/lw-full.log`).
- Validation: `cargo test -p cubarium-render -p cubarium` (every suite green at each
  commit); `./scripts/art-bake.sh` twice with `cmp`; the ignored cost test above; the PNG
  captures above.
- Artefacts: `captures/lanternjaw/*`, `assets/atelier/{pack.json,plants.png}`,
  `art/plants/author_grow.py`.

## Limitations and next work

- **Canopy opening is not authored.** umbrellafrond and bloomcrown keep the radial reveal
  mask; a top-down "opening" convention (petals/frond scaling about a fixed centre) is the
  named remainder of package 2.
- The Lanternjaw is a body, not an animal: no presenter wiring, no mode cross-fade in the
  live world (the `stamp_rig` state mixture exists for it), no founder, no ecology. The
  next package is the paid-predator experiment on copied worlds (pursuit, attack, meals,
  digestion, single offspring, prey recovery, conservation audits) before any live
  introduction — `astra`'s fixed-hunter plan is the candidate recipe.
- The hunt strike is a 6 s scripted loop for the study only; real attacks must be
  event-triggered with their own progress.
- Colour mixes are the study's sRGB mixes; on the cube the composite over real ground is a
  linear-light source-over, so translucent parts differ slightly from the browser study.
  Hardware legibility at room distance is not established by these captures.
- The worst-frame cost figures were measured on a machine running root's 12-hour care
  experiments; re-measure on a quiet machine before quoting a distribution.
- Everything in the handoff's "Remaining full-goal scope" stays open: feed deposits, rain
  aftermath, cleanup acknowledgement, creature/plant responses and rituals, stronger
  tall-plant wind, coverage-AA comparison, matched longer care/autonomous runs, tunable
  input dose, the predator ecology, persistent plant age, droplets and nibbles.
