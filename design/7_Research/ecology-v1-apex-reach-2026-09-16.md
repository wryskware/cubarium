---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Why an apex strike ends out of reach: the burst is never requested

Workstream N of the ecology v1 next steps
([brief](../handoffs/ecology-v1-apex-reach-opus-2026-09-16.md)), step 4 of the reconciled next
steps, from K's named next task and Astra's round-2 review (P3 on K, next steps item 4).

K's [eligibility audit](ecology-v1-apex-eligibility-2026-09-16.md) localised an introduced
apex's starvation to intake, and intake to **reach**: of 449 paid attempts over sixteen lives,
402 ended `OutOfReach`, 31 `Missed`, 1 `TargetLost` and 15 captured. It said in its own words
that it could not tell whether the strike geometry, the pursuit controller, the prey's escape
speed or the sense radius was responsible. This workstream measured each paid attempt at the
three instants that decide it, and the answer is none of the first three as a *constant*:

- **The paid burst is not requested in 91 % of paid attempts.** The pursuit's own stopping rule
  held the member still — at its `rest_effort` of 0.05, and with no strike boost pushed — on
  **408 of 449** attempts in the death arm and **459 of 501** in the age-eligible probe.
- **A held burst moves the hunter 0.002 px/s, median**, against the `strike_speed_px_s` of
  **16.667** the stalk's own admission window assumed when it committed. 80.4 % of held bursts
  delivered `≤ 0.21 px/s`, which is `rest_effort · cruise` to the pixel.
- **The gap does not close. It grows.** Mean separation to close at the burst's start: **8.32 px**
  (probe 9.86). Mean separation actually closed over the burst: **−0.91 px** (probe −2.07).
- **The escape multiple is not the binding term.** The prey's realised speed over the burst
  averaged **3.29 px/s** (probe 5.59) and reached its cap of exactly **10.00 px/s**
  (`escape_speed_multiple 2.0 × speed_max 5.0`) at most. A *delivered* 16.667 px/s burst closes
  6.67 px/s against a prey at its cap, and 13.4 px/s against the average one — either of which
  clears the 8.3 px gap inside the 1.0 s burst.

The implicated stage is the **pursuit controller**, and the rule the evidence points at is named
in §6 below. **No constant moved in this workstream**, and the record that produced these
numbers is inert and off by default.

## Build and provenance

- The record, its tests and the audit's classification: `5ef88d1`; the body-frame coordinate,
  the hunter's own turn and the hold counters: `b144605` (this workstream's branch,
  `worktree-agent-a9e1fe1c020e6a1bd`, on `1525604`).
- Both artifacts were produced by search build id `b144605636c1`, i.e. the tree at `b144605`,
  clean, from a release binary built once and copied out of the shared target directory so no
  concurrent rebuild could change it under a running experiment.
- Ecology: the same two declared screen candidates E and K used,
  `runs/ecology-v1-calibration/selected/baseline.toml` (config hash `fc1aefa33ebd70a1`) and
  `fast-leaf.toml` (`09e244392ec91768`), re-derived and confirmed bit-for-bit at every seed
  (`matches_screen_candidate: true`, 16 of 16).
- Held-out seeds 9001–9004, two adults, never restocked, horizon 180,000 ticks, 8 workers.

```bash
# 1. K's death arm exactly, with the per-attempt strike record on.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --workers 8 --out runs/ecology-v1-apex-reach/death-reach.json

# 2. The age-eligible probe arm, which adds 52 more paid attempts.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 24000 --founder-age-seconds 1200 \
  --workers 8 --out runs/ecology-v1-apex-reach/probe-aged-reach.json
```

Wall time 29.5 s and 28.0 s: **57.5 s** of measurement against the brief's 6-minute cap,
8 workers. The two JSON artifacts total **2.6 MiB** against a 20 MiB cap; `runs/` is
git-ignored by design and the commands above reproduce them.

**Stage 1 reproduces K's death arm to the tick.** All sixteen lifetimes, all sixteen
`Starvation` causes, `captures = 15`, `ticks_two_ready = 0`, and the attempt histogram
`{captured 15, out_of_reach 402, missed 31, target_lost 1}` are K's rows exactly. The record is
inert; this is that claim's measurement, not its assertion.

## 1. What the record is

`crate::hunter::StrikeRecord` carries one `StrikeFrame` at each of the three instants of one
paid attempt, and nothing between them:

| frame | the boundary | what happened since the previous frame |
| --- | --- | --- |
| `intent` | the member entered `Windup` | — |
| `strike` | the member entered `Strike`, i.e. paid | `windup_seconds` = 0.6 s = 12 ticks |
| `resolution` | the settlement pass resolved it | `strike_seconds` = 1.0 s = 20 ticks |

The frames are exactly those durations apart by construction, so every realised speed below is a
displacement over a duration the record knows rather than one it infers. Each frame is gathered
through the **same `ContactEvidence::gather` the settlement itself uses**, so the intent geometry
is measured by the authority that decides the outcome, not by a second looser one.

Per frame: both bodies' position and heading; the surface distance from the hunter root to the
prey (`root_distance`); the distance from the **scaled grasp centre** to the prey
(`effector_distance` — this is what "separation" means throughout this note); the contact
tolerance `capture_reach_px · scale + prey_extent`; the advertised reach
`(|capture_offset_body| + capture_reach_px) · scale`; the prey's position in the hunter's own
body basis (`body_forward`, `body_side`) and the grasp's own forward coordinate
(`capture_forward`); whether the prey was in reach (`effector_distance ≤ tolerance`, which *is*
`ContactMeasure::in_contact`); and whether the grasp centre is on the surface at all.

Per record: the outcome and the energy actually charged; both bodies' realised speed and heading
change over the windup and over the burst; whether the target's identity changed, its handle went
stale, or either body changed cube face; and the class its own geometry implies.

**Inert and opt-in.** Off by default, turned on per `World` by `World::record_strike_attempts`.
Every site is one `bool` test when off; gathering a frame is pure, consumes no draw, writes
nothing the tick reads back, and appears in no snapshot and no state hash. Fixed by
`crates/cubarium-core/tests/hunter_strike_record.rs`: a two-apex world hashes identically at
every 500-tick boundary of **9,000 ticks** with the recorder on and off, and the test asserts the
record count is non-zero so the run is evidence rather than a tautology.

### The tests, written to the brief's definitions

| test | what it fixes |
| --- | --- |
| `a_stationary_prey_in_the_grasp_captures_and_all_three_frames_say_it_was_in_reach` | a hand-built attempt on a frozen prey placed at the claw captures; all three frames report separation 0 to 1e-9, `in_reach`, scale 1.0, the advertised reach, and frame ticks exactly 12 and 20 apart; the record's key reconciles with the world's own `Attempt` event |
| `a_prey_just_beyond_the_grasp_records_out_of_reach_with_the_separation_the_geometry_predicts` | the same attempt with the prey 0.75 px past the tolerance resolves `OutOfReach`; every frame's separation, tolerance and overshoot equal the values recomputed from `capture_offset_body`, `capture_reach_px` and the prey's own extent, to 1e-9 |
| `a_windup_that_never_pays_for_a_burst_leaves_no_record` | an unaffordable refusal consumes no counter and publishes no record |
| `every_class_is_decided_by_the_geometry_that_defines_it` | all five classes and the stated priority, from hand-built frames |
| `an_attempt_with_no_intent_frame_is_unreadable_and_counted` | a mid-attempt switch-on is filed `Unreadable`, never guessed at |
| `the_recorder_is_inert_by_state_hash_over_nine_thousand_ticks_of_a_two_apex_world` | inertness, and one record per paid attempt the world published |

The second test is the one that bites hardest: relaxing `in_reach` to `effector_distance ≤ 2 ·
tolerance` in the implementation turns it red with `ResolvedInReach` where `BeganOutOfReach` is
expected (checked by mutation, not assumed).

Six more tests in `crates/cubarium-search/tests/apex_strike_reach.rs` fix the aggregation: one
bucket per class and one per outcome with no attempt lost or double-counted, means taken only
over the attempts that carried the quantity, extremes preserved so a deciding row is findable,
the fixed table order, the merge across seeds, the counted raw-record cap, and the hold rule
counted from the body-frame coordinate.

## 2. The classification

Stated thresholds, this workstream's, evaluated in a fixed priority so every attempt gets exactly
one class:

1. **`target_lost`** — the target's identity changed, its handle went stale, or the prey was
   already claimed and removed. No resolution geometry exists, so no other class can be decided.
2. **`resolved_in_reach`** — `effector_distance ≤ tolerance` at the resolution frame. The strike
   *arrived*; whether it then captured is the capture roll's business, not reach's.
3. **`began_in_reach_resolved_out`** — in reach at the intent frame and out of it at resolution.
   **Cadence or resolution.**
4. **`prey_outran`** — out of reach at intent and the separation **grew** strictly across the
   paid burst. **The escape envelope.** No epsilon: a tie counts as "did not grow", which is the
   conservative reading for the escape hypothesis.
5. **`began_out_of_reach`** — out of reach at intent and the gap never closed, although it did
   not grow. **Target selection or the pursuit controller.**

### Death arm — 449 paid attempts, 16 lives

Separations in px; `fwd@int` is the prey's forward body coordinate at the intent frame, against
a grasp that sits at `capture_forward` = 13.28 px; `held` is the attempts whose **strike** frame
satisfied the pursuit stopping rule (§5); `prey v` and `hunt v` are realised px/s over the burst;
`hunt |m|` is the whole motor magnitude `|v| + r·|ω|` the burst delivered.

| class | n | e paid | sep@int | sep@str | sep@res | over@res | fwd@int | held | prey v | hunt v | hunt \|m\| |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `resolved_in_reach` | 46 | 3.680 | 3.74 | 3.40 | 2.94 | −1.05 | 13.60 | 35 | 2.64 | 1.24 | 2.11 |
| `began_in_reach_resolved_out` | 30 | 2.400 | 2.79 | 3.66 | 5.23 | 1.27 | 12.85 | 28 | 4.62 | 1.43 | 2.17 |
| `prey_outran` | **217** | 17.360 | 12.27 | 12.58 | 14.62 | 10.66 | 6.21 | 213 | 3.76 | **0.12** | 0.46 |
| `began_out_of_reach` | **155** | 12.400 | 12.39 | 12.81 | 10.25 | 6.27 | 7.90 | 131 | 2.58 | 1.12 | 2.10 |
| `target_lost` | 1 | 0.080 | 10.09 | 12.80 | — | — | 9.73 | 1 | — | 0.10 | 0.46 |

`46 + 30 + 217 + 155 + 1 = 449`, and `30 + 217 + 155 = 402` — exactly K's `OutOfReach` count, and
`46 = 15 captured + 31 missed` — exactly the attempts that reached contact. The two tallies
reconcile without adjustment.

### Age-eligible probe arm — 501 paid attempts

| class | n | e paid | sep@int | sep@str | sep@res | over@res | fwd@int | held | prey v | hunt v | hunt \|m\| |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `resolved_in_reach` | 41 | 3.280 | 6.13 | 5.47 | 3.63 | −0.39 | 13.29 | 24 | 5.44 | 3.11 | 3.85 |
| `began_in_reach_resolved_out` | 29 | 2.320 | 2.99 | 3.83 | 5.27 | 1.28 | 12.76 | 27 | 5.81 | 2.48 | 3.88 |
| `prey_outran` | **273** | 21.840 | 13.08 | 13.30 | 16.83 | 12.83 | 4.66 | 269 | 6.01 | **0.19** | 0.57 |
| `began_out_of_reach` | **158** | 12.640 | 13.85 | 14.99 | 8.81 | 4.82 | 5.86 | 139 | 4.85 | 1.48 | 3.11 |

The probe replicates the death arm on every reading: 460 of 501 `OutOfReach` (91.8 % against
89.5 %), the same two dominant classes in the same order, the same held fraction.
`dropped = unreadable = omitted = 0` in both arms, so both tables are complete.

**The two cadence classes together are 7.5 % of the reach failures** (30 of 402; probe 29 of 460,
6.3 %). Cadence and resolution are not the story. **93 % of reach failures began out of reach**,
which is the pursuit branch of Astra's split.

## 3. Captures against out-of-reach, on the same fields

| field | Captured (15) | Missed (31) | OutOfReach (402) |
| --- | ---: | ---: | ---: |
| separation at intent (px) | **3.93** | 3.65 | **11.61** |
| separation at the burst's start | 3.60 | 3.29 | 11.98 |
| separation at resolution | 3.10 | 2.87 | 12.43 |
| contact tolerance | 3.98 | 3.99 | 3.97 |
| advertised reach | 14.82 | 14.82 | 14.82 |
| prey forward body coordinate at intent | **12.59** | 14.09 | **7.36** |
| prey realised speed over the burst (px/s) | 3.14 | 2.40 | 3.37 |
| hunter motor magnitude over the burst (px/s) | 1.72 | 2.30 | 1.21 |
| held by the pursuit rule at the burst's start | 13 / 15 | 22 / 31 | **372 / 402** |

The probe arm reads the same way: Captured sep@int 5.52 and fwd@int 14.03, against OutOfReach
12.71 and 5.58.

**A capture is not a lunge that worked. It is a prey that was already in the claws.** The
captured attempts began at 3.93 px of separation — inside the 3.98 px tolerance, within rounding
of it — and with the prey's forward coordinate at 12.59 px, i.e. already level with the grasp at
13.28 px. The reach failures began at 11.61 px with the prey 7.36 px forward: **6 px short of the
claws**, which is exactly the gap the burst exists to close. The prey's own speed distinguishes
the two groups by almost nothing (3.14 against 3.37 px/s). The tolerance and the advertised reach
are identical across all three outcomes to two decimals, so neither the grasp's size nor its
placement separates a capture from a refusal.

## 4. What the burst actually delivered

Over the 408 held attempts of the death arm:

| quantity | value |
| --- | --- |
| `strike_speed_px_s` (profile) | **16.667 px/s** |
| `strike_closing_px = strike_speed_px_s · strike_seconds`, which the stalk's admission window adds to the tolerance | **16.667 px** |
| hunter's realised burst speed, **median** | **0.002 px/s** |
| hunter's realised burst speed, mean / p95 / max | 0.36 / 2.50 / 7.99 px/s |
| fraction at or below `rest_effort · cruise` ≈ 0.21 px/s | **80.4 %** |
| hunter's whole motor magnitude `\|v\| + r·\|ω\|`, mean | 0.87 px/s |
| mean gap to close at the burst's start (attempts that began out of reach) | **8.32 px** |
| mean gap actually closed over the burst | **−0.91 px** |
| prey realised burst speed, mean / max | 3.29 / **10.00** px/s |

Three deciding rows, each the median of its class in the death arm, printed from the artifact:

```
prey_outran         baseline/9002  ac=26  ticks 16748→16760→16780
                    sep 11.34 → 12.42 → 14.00   tol 4.00   fwd@strike 10.17  held=true
                    prey 2.88 px/s   hunter 0.00 px/s   hunter turn 0.77°     OutOfReach

began_out_of_reach  fast-leaf/9001 ac=23  ticks 15500→15512→15532
                    sep 13.13 → 12.88 → 12.32   tol 4.00   fwd@strike  5.81  held=true
                    prey 0.46 px/s   hunter 0.00 px/s   hunter turn 0.50°     OutOfReach

resolved_in_reach   fast-leaf/9001 ac=1   ticks 11570→11582→11602
                    sep  3.71 →  3.26 →  2.13   tol 4.00   fwd@strike 11.36  held=true
                    prey 1.02 px/s   hunter 0.00 px/s   hunter turn 0.80°     Missed
```

In all three the hunter's realised speed over its own **paid burst** is `0.00 px/s`. The second
row is the whole diagnosis in one line: a prey 5.81 px in front of a grasp that closes 13.28 px
out, a 9.13 px gap the 16.667 px burst was bought to close, and a hunter that did not move.

## 5. Why the burst is not requested — the rule, named

The hunt intent pass computes, for a member that is hunting a target it senses:

```rust
// crates/cubarium-core/src/world/step.rs, "the pursuit stopping distance"
let inside = organisms.get(t).and_then(admission).is_some_and(|c| {
    c.body.x < geometry.capture_offset_body.x + c.tolerance
});
let hold = inside || m.phase == HunterPhase::Windup;
…
d.1.effort = if hold { rest_effort } else { 1.0 };
if m.phase == HunterPhase::Strike && !inside {
    boosts.push((m.id, profile.strike_speed_px_s));
}
```

The comment above it says the predicate is "the reach envelope … which is what `in_contact`
already means everywhere else in this file", and its stated purpose is that "a prey already
*inside* the reach envelope is not approached further — walking onto it would put it behind the
claws". What the code tests is not the envelope but a **forward half-space**: `body.x` below
`capture_offset_body.x · scale + tolerance` = **13.28 + 3.97 = 17.25 px**. A prey short of the
claws satisfies it as readily as one inside them, and walking onto such a prey brings it *into*
the claws, not behind them.

The apex's sense radius is 12 px and its grasp closes 13.28 px forward, so **almost every prey it
can hunt at all is "inside"** by this test. The record measures exactly that:

| arm | paid attempts | held at intent | held at the burst's start | mean `fwd@int` of the reach failures |
| --- | ---: | ---: | ---: | ---: |
| death | 449 | 408 (90.9 %) | **408 (90.9 %)** | 7.36 px |
| probe | 501 | 450 (89.8 %) | **459 (91.6 %)** | 5.58 px |

The consequence is two-fold and both halves are measured. `hold` sets `effort` to
`drives.rest_effort` = **0.05**, so `speed_cap` for an apex that cruises ≈ 4.2 px/s becomes
≈ **0.21 px/s** — and no burst is pushed, so nothing lifts it back. Then the motor envelope
`|v| + r · |ω| ≤ speed_cap` is evaluated at the apex's turn radius, which `motor::turn_radius_px`
takes from the claws rather than the lobes: `|capture_offset| + capture_reach` = **14.83 px**. A
turn of 0.8°/s sweeps `14.83 × 0.014 = 0.21 px/s` and **exhausts the entire rest-effort budget**,
leaving 0.00 px/s for travel. That is the three deciding rows above: turns of 0.5–0.8° and speeds
of 0.00.

**Corroboration, with its confound stated.** In the 41 death-arm attempts where `inside` was
false at the burst's start, the burst *was* pushed: the delivered motor magnitude rose to
5.73 px/s mean (probe 5.42) and 26.8 % of them resolved in reach against 8.6 % of the held ones
(probe 40.5 % against 5.2 %). This comparison is **not randomised and the two subsets differ by
construction**: `inside` false means the prey is already at or past the claws' forward position,
and those attempts sit at 5.32 px of separation against 11.48 px for the held ones. So it
corroborates that a delivered burst arrives; it is not the deciding evidence. The deciding
evidence is the kinematic table in §4, which needs no subset at all: a 16.667 px burst was bought
449 times, 8.32 px of gap needed closing, and −0.91 px was closed.

## 6. The verdict

**The pursuit controller, not the strike kinematics and not the escape multiple — as the
cause of the present near-zero motion.** (Scoped after Astra's round-3 review: the escape
multiple and strike constants are exonerated for the *current* held bursts; whether they are
adequate for a corrected, actually delivered one-second lunge is not established — a nominal
16.7 px/s hunter against a 10 px/s prey closes only 6.7 px/s and needs 1.25 s for the mean
8.3 px gap against `strike_seconds` = 1.0, and the few delivered bursts were clipped to 5.7
px/s mean by the shared motor envelope. The paired arm decides that.)

- **Strike kinematics are not implicated as constants.** `strike_speed_px_s` = 16.667 and
  `strike_seconds` = 1.0 would close the mean 8.32 px gap in 0.5 s against a motionless prey and
  in 1.25 s against one at its escape cap — longer than the 1.0 s strike, so their adequacy
  for a delivered lunge is untested; what is established is that they are not applied in
  91 % of paid attempts, and where applied they are further clipped by the shared motor envelope
  at a 14.83 px turn radius (delivered 5.7 px/s mean, 13.31 px/s max).
- **The escape multiple is exonerated as the cause of the current motion, not yet as adequate
  after the correction.** `escape_speed_multiple` = 2.0 puts the
  prey's ceiling at exactly 10.00 px/s, which the record confirms as the observed maximum. The
  mean realised prey burst speed is 3.29 px/s (probe 5.59). A delivered burst out-closes the prey
  at its ceiling by 6.67 px/s. Raising or lowering this constant changes nothing while the
  hunter is travelling at 0.002 px/s.
- **Cadence and resolution are a minority term.** 30 of 402 reach failures (7.5 %; probe 6.3 %)
  began in reach and resolved out. The 0.6 s windup during which the hunter holds by design does
  cost ground — separation grows 0.37 px over it on the `OutOfReach` attempts — but it is an
  order of magnitude short of the 8.32 px that goes unclosed.

**The single rule the evidence points at, named and not changed:** the **pursuit stopping
predicate `inside`** in `crates/cubarium-core/src/world/step.rs`'s hunt intent pass —
`c.body.x < geometry.capture_offset_body.x + c.tolerance`. It is a one-sided test on the forward
coordinate where its own comment and the rest of the file mean the reach envelope,
`ContactMeasure::in_contact()`. It was true at the burst's start on 408 of 449 paid attempts, and
being true it both drops the member to `rest_effort` and suppresses the burst the member has just
paid 0.08 e for.

Two constants sit behind it and are named only so the review can see they are **downstream**:
`drives.rest_effort` = 0.05, which is the effort a *held* member spends its paid burst at, and
the apex turn radius of 14.83 px in `motor::turn_radius_px`, which the 0.21 px/s that leaves is
entirely consumed by. Neither would matter if the burst were requested. **This workstream changed
none of the three, nor any apex profile constant, radius, readiness term, equation, snapshot or
neural path; `FixedHunterProfile::lanternjaw_trial` holds exactly the constants it held at
`2a1cedd`, and the display's apex is untouched.**

## What this does not establish

- **Nothing about what fixing the rule would do.** No variant was run. That a delivered burst
  arrives more often is corroborated by a non-randomised 41-attempt subset whose geometry differs
  by construction (§5), not measured by an intervention. Whether an apex that actually closes
  would then *feed itself* — 15 captures against the ~10 per life its upkeep needs, by K's
  arithmetic — is a separate question this record cannot answer.
- **Nothing about the sense radius.** The 12 px sense gene against a 13.28 px grasp is what makes
  `inside` true for essentially every sensed prey, so the two interact; but the record does not
  separate "the rule is wrong" from "the rule would be right if the hunter sensed further", and
  no sensing variant was run.
- **Nothing about the capture roll.** 46 attempts reached contact and 15 of them captured
  (32.6 %); `capture_probability` was not examined and is not implicated by anything here.
- **Nothing about the grasp's placement or size.** `tolerance` and `advertised_reach` are
  identical across captures, misses and reach refusals to two decimals, so this evidence neither
  accuses nor exonerates `capture_offset_body` and `capture_reach_px` — it only says they do not
  distinguish the outcomes.
- **Two arms, four held-out seeds, two configurations, one lineage.** Both arms are K's, with the
  record added. No ecology parameter, introduction tick other than 6,000 and 24,000, or horizon
  other than 180,000 was tried.
- **The `began_in_reach_resolved_out` class is small and therefore weakly estimated** (30 and 29
  attempts). Its reading — that cadence is a minority term — would not survive a factor-of-three
  revision, though the 93 % that began out of reach would.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## The next task this implies

Named, not launched.

**Make the pursuit close, then re-measure this same table.** The rule in §6 is a one-line
predicate whose intended meaning is already written above it and already implemented as
`ContactMeasure::in_contact()`. The cheapest decisive experiment is a paired arm — the same two
candidates, the same four held-out seeds, the same introduction ticks — with `inside` read as the
envelope rather than the half-space, scoring on exactly the numbers this note reports: the held
fraction, the mean gap closed over the burst, the class histogram, and the captures per life
against the ~10 K's arithmetic says a life needs. Because the record is inert and already
committed, that arm costs one more 30 s run per variant and its result is directly comparable to
the rows above.

Two cautions for whoever takes it. First, the same `hold` governs the **stalk**, so changing it
changes how a member approaches as well as how it lunges; the phase occupancy K measured
(stalking 13.2 %, strike 4.5 %) should be re-read, not assumed. Second, an apex that closes will
spend more on motor — 1.7 % of the bill today — and K's ledger is the instrument that says
whether the extra captures pay for it. The two should be read together, and the decision about
the predicate is Fable's, not this workstream's.

## Verification

`cargo test -p cubarium-core` **508 passed / 0 failed / 4 ignored**, of which this workstream
contributes the 6 in `crates/cubarium-core/tests/hunter_strike_record.rs`.
`cargo test -p cubarium-search` **182 passed / 0 failed / 1 ignored**, of which this workstream
contributes the 6 in `crates/cubarium-search/tests/apex_strike_reach.rs`. `graft build` rebuilt
the graph.

The shared `target/` is used by several concurrent workers and the stale-artifact race E and K
both documented recurred twice in this session, as `no StrikeClass in hunter` immediately after
the same symbol had compiled. `touch crates/cubarium-core/src/lib.rs` and rebuild clears it.
