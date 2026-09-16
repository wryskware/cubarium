---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream G: the foliage shoulder, a soil-band dead-wood cue, and the dead column's test

Evidence for [the follow-up brief](../handoffs/ecology-v1-presentation-2-opus-2026-09-16.md),
on top of [workstream B's result](ecology-v1-presentation-2026-09-15.md), answering
[Astra's review](ecology-v1-next-review-2026-09-15.md) finding 7. Everything below is a
*presentation* decision. **No core change of any kind**: no equation, ordering, parameter or
field of the simulation moved, and no file outside `crates/cubarium/src/art_present/`,
`crates/cubarium/tests/`, `crates/cubarium/examples/` and `design/7_Research/` was touched.

**The physical cube was not inspected**, and neither was a viewer. The sheet is the evidence,
as the brief says.

## 1. The shoulder

### The table

`design/7_Research/assets/ecology-v1-shoulder-2026-09-16.png` (25 KB, 586 × 452). To
reproduce, from the repository root:

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target \
  cargo run --release -p cubarium --example shoulder_sheet -- \
  --out design/7_Research/assets/ecology-v1-shoulder-2026-09-16.png
```

It is deterministic and prints every number below to stdout.

| shoulder | worst frame step | standing control | **excess** | first frame the stand moves (of 180) | loss then | the ramp leaves 1 at | an **ungrazed** average-light stand | its wobble's worst step | wobble excess |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **0.85** (ships) | 0.0651 | 0.0651 | **0.0000** | **never** | never | **30.2 %** | mix 0.000, not on screen | 0.0616 | 0.0000 |
| **0.95** | 0.0650 | 0.0651 | **−0.0001** | 132 | 22.4 % | **22.0 %** | mix 0.001, on screen | 0.0611 | −0.0004 |
| **1.0** | 0.0647 | 0.0651 | **−0.0005** | 108 | 18.3 % | **17.9 %** | mix 0.014, on screen | 0.0603 | −0.0007 |

- **Flicker** is B's own bound: the worst per-channel step between two consecutive frames of a
  strip-and-reflush run at three frames a tick, against a standing control, allowed 0.05 of
  excess. Every shoulder is at or below zero. Raising the shoulder *lowers* the worst step,
  because a stand that is already travelling toward the wood tone moves less per frame than a
  stand that is still whole.
- **First frame the stand moves** runs a controlled 30 % foliage loss over 180 frames and
  draws each frame **twice from the same view** — once at the shoulder under test, once at a
  shoulder of 10⁻⁹, which draws every stand whole. Both presenters see the same stocks, so the
  sway clip, the wind, the growth pacing, the ground cover, the producer wash and the flecks
  are identical between them, and the frames are compared **encoded to 8 bits**, the way the
  cube shows them. What is left is the shoulder's own effect and nothing else.
- **The ungrazed average-light stand** is the cost of raising it: B0's average class carries
  `P / W = 0.930`, which is *below* 0.95 and 1.0. "on screen" means the encoded frame actually
  differs from the whole-canopy one. **The wobble** arm moves that stand's `P` by ±5 % over 120
  ticks and measures the same flicker excess.

### The loss ladder (rows 1–3 of the sheet)

The bright stand's `mix` — how far its sprite has travelled from leaf to bare wood — at each
loss and each shoulder. 0 is the image this presenter drew before ecology v1 existed.

| lost | `P` | `P / W` | at 0.85 | at 0.95 | at 1.0 |
| --- | --- | --- | --- | --- | --- |
| 0 % | 0.4789 | 1.217 | 0.000 | 0.000 | 0.000 |
| 10 % | 0.4310 | 1.096 | 0.000 | 0.000 | 0.000 |
| 20 % | 0.3831 | 0.974 | 0.000 | 0.000 | 0.002 |
| 30 % | 0.3352 | 0.852 | 0.000 | **0.030** | **0.059** |
| 40 % | 0.2873 | 0.730 | 0.054 | 0.136 | 0.179 |
| 60 % | 0.1916 | 0.487 | 0.391 | 0.481 | 0.520 |
| 80 % | 0.0958 | 0.243 | 0.801 | 0.837 | 0.851 |
| 100 % | 0.0000 | 0.000 | 1.000 | 1.000 | 1.000 |

### Two corrections to the finding, which the recommendation rests on

**Most of the dead zone is not the shoulder's to give back.** B0's bright stand carries
`P / W = 1.217` and the fullness ratio clamps at 1, so the first **17.9 %** of its loss cannot
move the sprite *at any shoulder at all*. Of the 30.2 % that 0.85 holds, only 12.3 points
belong to the shoulder; the rest belongs to the stand being over-leaved for its wood.

**Depletion was never invisible — only the stand's own sprite was still.** Whatever the
shoulder, the picture first changes an encoded pixel at **frame 3, 0.51 % of the foliage
lost**: the ground cover, the producer wash and the detritus flecks all read `P` directly and
none of them has a shoulder (B's decision 7). Astra's "masks depletion" is true of the plant
sprite and false of the cell.

**And on the real trajectory the shoulder barely matters.** Rows 4–6 of the sheet are B's own
row 4 — one pinned grazer on a real stepped `World` — at the three shoulders, and they are
hard to tell apart: the grazer takes `P / W` from 1.217 to 0.226 in 75 simulated seconds,
straight past all three shoulders within one sample interval. The trajectory reproduces B's
numbers exactly, grazer removed at tick 918.

| tick | s | `P` | `W` | `P / W` |
| --- | --- | --- | --- | --- |
| 0 | 0 | 0.4789 | 0.3934 | 1.217 |
| 600 | 30 | 0.1827 | 0.3936 | 0.464 |
| 1 500 | 75 | 0.0889 | 0.3936 | 0.226 |
| 3 000 | 150 | 0.1394 | 0.3936 | 0.354 |
| 6 000 | 300 | 0.2049 | 0.3936 | 0.520 |
| 12 000 | 600 | 0.2551 | 0.3936 | 0.648 |
| 24 000 | 1 200 | 0.4098 | 0.3936 | 1.041 |
| 36 000 | 1 800 | 0.5086 | 0.4308 | 1.181 |

### The recommendation: 0.95. The default is unchanged at 0.85.

It halves the shoulder's own share of the dead zone — 30.2 % → 22.0 % of foliage loss before
the stand moves — **for a measured cost of nothing**. B's stated worry about raising it (at
0.90 "the average class has only a 3 % margin and may flicker") is not borne out: at 0.95 an
ungrazed average-light stand draws `mix = 0.001`, and its ±5 % steady-state wobble adds
−0.0004 of frame step against a standing control, against a bound of 0.05.

**1.0 is where I would stop.** It buys another 4 points of dead zone, but an *ungrazed*
average-light stand then draws `mix = 0.014` and that is on screen: the cube would be showing
bare wood on a stand nothing has eaten, which is reporting depletion that is not happening —
the opposite failure to the one being fixed, and a worse one.

**This is a recommendation, not a change.** `FOLIAGE_FULL` still ships at 0.85, and the
decision is Wrysk's after he sees it.

### How to see it on the cube

`CUBARIUM_FOLIAGE_FULL` moves the shoulder for a viewing session: a float, clamped to
`0.5..=1.0`, read **once per process** when the first `ArtPresenter` is built, with one stderr
line if it is out of range or not a number. Unset — which is how everything ships and how
every test and capture runs — the display draws at `FOLIAGE_FULL` exactly.

```bash
CUBARIUM_FOLIAGE_FULL=0.95 ./scripts/run-cube.sh
```

There is no CLI flag and no runtime setting: the shoulder decides what a stage sprite's
*colour* is at a given fullness, so moving it under a running presenter would be a cut in the
middle of the one thing this presentation is not allowed to cut. The sheet uses a separate,
purely construction-time hook, `ArtPresenter::with_foliage_full`, which the environment does
not reach.

**This is the one deviation from the brief**, which asked for the override to be "for the
sheet only … not a runtime setting". Fable widened it mid-task so the owner could judge the
shoulder where the judgement actually gets made. Recorded here because the brief's own wording
says otherwise.

## 2. The soil band's dead-wood cue

### The rule

```text
soil_snag = clamp(dead_wood_density − wood_density, 0, 1)
```

— both terms the same cube-root [`wood_fraction`] the structural bands read. The brief's rule
is "`Wd > 0` and `W = 0`", and this is exactly that at both ends: full strength where a stand
is wholly dead, **nothing** where there is no dead wood, and **nothing** where a living stand
at least as large stands in the same cell.

**Why the difference and not a hard gate.** A stand dying below the horizon has `W` falling to
zero while `Wd` rises, and `W` reaching zero is precisely the instant `Wd` is *largest*. A hard
`W == 0` gate would therefore stamp the whole mark in one frame — the loudest cut on the cube,
and a cut is the one thing contract §12 forbids. The difference makes the mark **fade in over
the dieback**. It also keeps B's own test
`the_structural_read_does_not_disturb_the_soil_or_the_water_band` true unchanged: its fixture
holds `W = Wd = 0.3934` in a soil cell and still draws nothing.

### The mark

The cell's **own soil plant, stage 0, cut to its bottom 2.5 tile rows** by `Mask::Axial`, in
the dead tone `0x5A5E6E` through `wood_shade()`, at
`SOIL_PLANT_OPACITY · SOIL_SNAG_OPACITY · soil_snag` (`SOIL_SNAG_OPACITY = DEAD_WOOD_OPACITY =
0.70`). Two rows and the fade of a third — a **stub**, half a cell tall, smaller than any
living plant in the band even at stage 0. It takes the slot's own wind bend and the slot's own
phase, so it leans with everything around it. **No new asset**: the cell's species pick is
already a pure function of the cell.

It is stamped **over** the band's scenery, not under it. That is deliberate and it is the one
place I departed from the convention B set above the horizon (dead under living): the litter a
stand's own decay produces is drawn from `D + C` at the same slot, and under it the only
record that the stand died could be hidden by the consequence of its dying.

**Litter is untouched.** `D + C` still drive the soil band's plants, its detritus flecks and its
ground wash on their own; `Wd` is not folded into `litter_density`, and a tested assertion says
so.

**Visible effect.** Below the horizon, a cell whose stand has died now carries a short grey-blue
stub among its mushrooms, brightest just after the stand dies and fading to nothing as `Wd`
decomposes. A cell that never had a stand, or whose stand is alive, looks exactly as it did.
A face whose lower rows have died reads as stubble rather than as ordinary litter.

## 3. The dead tall column

No code changed; the claim is now a pixel test
(`a_dead_tall_column_stands_in_ash_at_a_height_from_its_dead_wood`). It asserts that a column
whose foliage cells hold `W = 0, Wd > 0` draws, is distinct from empty ground, stands at
exactly `tall_target(column_dead_density(...))` segments, carries **no crown**, is ash-toned,
and fades monotonically to exactly the empty image at `Wd = 0`.

**Isolating the column from the cells it stands in** is the only hard part, and it is worth
recording: a side face has **no pixel row that the topmost foliage cell's own plant cannot
reach**, so there is no region the column alone owns. The test instead differences two stocks
that both saturate every cell's own silhouette (`Wd = 0.25` and `Wd = 0.6`, both above the
stage-2 threshold, where `stage_opacity` is the flat band ceiling). Every per-cell pixel is then
identical between the two and the pixels that differ are *exactly* the column's extra trunk
segments — and, on the living side, its crown, which is how "no crown" is asserted as a strict
pixel-count inequality rather than by eye.

One geometric fact fell out of it: a nine-segment column's last trunk segment **unfolds onto
the top face**, painting `(Top, 28..31, 62..63)` for the pilot column on Front. The test's strip
says so explicitly.

## 4. Tests

Written as their own pass, from the brief's own definitions, **before the drawing they judge
existed** — the two that failed on the first run failed on their own measurement design (an
absolute saturation comparison that the background dominated, and a crown-height claim voided
by the geometry above), not on the implementation.

| the brief's claim | the test |
| --- | --- |
| the mark is present for dead wood, absent at `Wd = 0`, separate from the litter ramp, and small | `the_soil_band_marks_a_dead_stand_and_leaves_the_litter_ramp_alone` |
| monotone in `Wd`, exactly soil at zero | `the_soil_mark_is_monotone_in_its_stock_and_reaches_soil_at_zero` |
| pairwise distinguishable from litter-only and from empty, and ash rather than a plant colour | `the_soil_mark_is_pairwise_distinct_from_litter_alone_and_from_empty_ground` |
| never under a living stand, and continuous through a dieback | `the_soil_mark_never_shows_under_a_living_stand_and_fades_in_as_one_dies` |
| the dead tall column | `a_dead_tall_column_stands_in_ash_at_a_height_from_its_dead_wood` |
| the shoulder hook is the shipped ramp at the shipped value, and monotone in the shoulder | `the_foliage_shoulder_is_overridable_for_a_study_and_ships_unchanged` |
| the environment variable's reading, with the string injected rather than the process environment touched | `the_shoulder_environment_variable_moves_the_viewing_session_and_nothing_else` |

`crates/cubarium/tests/art_ecology.rs` is **17 → 24 tests**. B's seventeen all still pass
unchanged; none was edited, relaxed or deleted.

| crate | passed | failed | ignored |
| --- | --- | --- | --- |
| `cubarium` | 596 (+7) | 0 | 19 (+1) |
| `cubarium-render` | 101 | 0 | 0 |
| `cubarium-core` | 467 | 0 | 2 |

The `+1` ignored is a new release-only timing arm (below). No existing suite needed a change of
any kind: nothing in this work moves a stamp's footprint, changes a fixture's fields or alters
the image of any cell that is not carrying dead wood below the horizon.

## 5. Per-frame cost

`crates/cubarium/tests/animation_load.rs`, release, one thread, B's own crowded fixture.
**The host was running other workers' builds**, and a single-run comparison was badly
misleading — the first paired measurement showed a rise of 0.6–1.0 ms on *all four* arms,
including one the change cannot touch. The numbers below are an **interleaved A/B**: the two
test binaries built and kept side by side, run alternately for three rounds, minimum per arm.
On the same host a single non-interleaved run put the baseline's own worst bucket at 23.5 ms.

| fixture | before (b70624d) | after | Δ mean |
| --- | --- | --- | --- |
| growing / dry / full canopy | 8.999 mean, 9.983 worst | 8.988, 9.968 | **−0.011** |
| growing / dry / half-grazed | 9.084, 10.048 | 9.082, 10.057 | **−0.002** |
| growing / dry / half-grazed over dead wood | 13.814, 15.826 | 13.823, 15.855 | **+0.009** |
| wilting / wet / full canopy | 10.195, 10.844 | 10.167, 10.814 | **−0.028** |

All four are inside run-to-run noise and far inside the brief's 0.5 ms bound. **None of B's
four arms actually draws the new mark**, because each holds `W = W_max` wherever `Wd > 0`, so a
fifth arm was added for it:

| fixture | | |
| --- | --- | --- |
| growing / dry / **every soil cell dead** (new) | 11.216 mean, 12.169 worst | +2.18 ms over the same scene with a living soil band |

320 marks — every cell of the soil band carrying a dead stand over an otherwise ungrazed scene,
which is the mark's own worst case and, unlike the "half-grazed over dead wood" arm, is a state
material conservation permits. It costs about 7 µs a mark, the ordinary per-stamp cost; the
`Mask::Axial` cut makes the mark small, not cheap. It leaves 4.5 ms of the 16.67 ms budget, and
a realistic world with a few dozen dead soil cells pays a tenth of it.

`soil_snag` takes the dead stock *before* the living one, so in a world where nothing has died
the whole pass is one field compare per soil cell — which is why the four existing arms did not
move at all.

## 6. What was not done

- **The cube was not looked at, and neither was the viewer.** The brief said no viewer run was
  required and the sheet is the evidence. Every claim above is a pixel measurement or a test.
- **No world was run long enough to watch a stand die below the horizon.** The mark is tested
  and measured, but every dead soil cell I have seen is synthetic — the same gap B recorded.
- **The shoulder was not changed**, and no other mapping constant was either.
- **The soil mark's shape is the band's own plant, cut.** A purpose-authored snag sprite would
  read better than a cut mushroom and would need a new asset; the brief said not to add one
  unless unavoidable, and it was not.
- **The water band still shows nothing for dead wood.** A reed stands by depth; the brief asked
  only for the soil band.
- **`plant_reserve` is still not drawn**, and the corner-cap retained-chart stamp still has no
  toned path — both unchanged from B's list, both still unreachable.
