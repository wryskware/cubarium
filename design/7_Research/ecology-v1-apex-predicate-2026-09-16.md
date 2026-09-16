---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The pursuit stopping predicate, paired: the burst is delivered, and it still does not close

Workstream P of the ecology v1 next steps
([brief](../handoffs/ecology-v1-apex-predicate-opus-2026-09-16.md)), item 1 of the reconciled
round-3 next steps and Astra's "single most informative cheap experiment now"
([round-3 review](ecology-v1-round3-review-2026-09-16.md), P2 on N and next steps item 1).

N's [strike record](ecology-v1-apex-reach-2026-09-16.md) named the **pursuit stopping predicate
`inside`** in the hunt-intent pass — `c.body.x < geometry.capture_offset_body.x + c.tolerance`,
a one-sided forward half-space where its own comment and the rest of the file mean the reach
envelope `ContactMeasure::in_contact()`. It held on 408 of 449 paid attempts, dropping the
member to `rest_effort` and suppressing the burst it had just paid 0.08 e for, and N could say
nothing about what correcting it would do because no variant was run. This workstream ran the
variant, paired, one variable, identical seeds and introductions.

- **The predicate is what suppresses the burst. That half is settled.** Held at the burst's
  start falls from **89.4 % to 5.4 %** over 32 lives. The shipped arm spends 89.4 % of its paid
  bursts at 0.75 px/s of whole motor; the corrected arm spends 89.7 % of them at **12.46 px/s**.
  The stalk's occupancy falls from 11.3 % to 5.1 %, exactly as N cautioned it would.
- **Corrected, the apex closes on prey it could not reach before — and eats nearly twice as
  much.** Contacts rise **88 → 140** (9.8 % → 14.4 % of paid attempts), captures
  **38 → 67** (1.19 → **2.09 per life**), gut intake **0.337 → 0.614 m per life**, with mean
  lifetime up 12,440 → 13,164 ticks. Every one of the 9 captures that began at an 8–12 px gap
  is new: the shipped rule produced **zero** contacts from that bin in 186 attempts.
- **The gap still does not close, and the strike constants are not what is short.** Over the
  delivered bursts the hunter translates at **4.57 px/s** against a nominal
  `strike_speed_px_s` of 16.667, because **64 % of its boosted motor budget (7.96 of 12.46 px/s)
  is turn sweep** priced at the claw-derived 14.83 px radius. The prey it is chasing averages
  2.61 px/s, nowhere near its 10 px/s cap. Mean gap change per burst improves from −1.07 px to
  −0.21 px — it stops growing — but does not go positive.
- **It does not make the apex viable.** 32 of 32 still die of `Starvation`, at **12.9 %** of the
  intake their own bill needs against 7.7 % before; the oldest member reached 23,201 ticks
  against the 24,000 `may_reproduce` requires, and readiness overlap is still zero in all 16
  runs.

**No constant moved.** No apex profile value, no escape speed, no sense radius, no strike
duration, no mating radius, and no `WorldConfig` field. The variant is opt-in per `World` and
the default arm reproduces K's and N's retained rows to the attempt.

## Build and provenance

- The switch, its tests, the audit's new aggregations and the CLI: `d19150e`, the whole of this
  workstream's branch `worktree-agent-ab8b4ea24aafd6985`, on the brief commit `15e13c8`.
- All four artifacts were produced by search build id **`d19150ea6ee7`**, i.e. the tree at
  `d19150e`, clean, from one release binary built once and copied out of the shared target
  directory so no concurrent rebuild could change it under a running arm.
- Ecology: the same two declared screen candidates E, K and N used,
  `runs/ecology-v1-calibration/selected/baseline.toml` (config hash `fc1aefa33ebd70a1`) and
  `fast-leaf.toml` (`09e244392ec91768`), re-derived and confirmed bit-for-bit at every seed
  (`matches_screen_candidate: true`, 48 of 48).
- Two adults, never restocked, introduced at tick 6,000, horizon 180,000 ticks, 8 workers.
  Mass residual ≤ 3.9e-10 in every row.

```bash
# The pair the brief specifies: K's/N's death arm, four held-out seeds, one variable.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --pursuit-stop half-space \
  --workers 8 --out runs/ecology-v1-apex-predicate/half-space.json
cargo run --release -p cubarium-search -- apex-audit \
  … --pursuit-stop reach-envelope \
  --workers 8 --out runs/ecology-v1-apex-predicate/reach-envelope.json

# The same pair widened to all eight held-out seeds. Run *after* the pair above, because its
# contact counts (46 against 46) are too small to read; see "What this does not establish".
cargo run --release -p cubarium-search -- apex-audit … --seeds 8 --pursuit-stop half-space \
  --out runs/ecology-v1-apex-predicate/half-space-8.json
cargo run --release -p cubarium-search -- apex-audit … --seeds 8 --pursuit-stop reach-envelope \
  --out runs/ecology-v1-apex-predicate/reach-envelope-8.json
```

Wall time 30.3 s, 29.1 s, 64.3 s and 66.1 s: **190 s** of measurement against the brief's
4-minute cap, 8 workers. The four JSON artifacts total **8.0 MiB** against a 20 MiB cap;
`runs/` is git-ignored by design and the commands above reproduce them.

**The off arm reproduces N's death arm to the attempt.** All sixteen lifetimes, all sixteen
`Starvation` causes, `captures = 15`, `ticks_two_ready = 0`, the class histogram
`{resolved_in_reach 46, began_in_reach_resolved_out 30, prey_outran 217, began_out_of_reach 155,
target_lost 1}`, `held = 408`, and the whole ledger — upkeep 70.93 e, strike and handling
36.31 e, motor 1.85 e, 109.09 e spent — are N's and K's rows exactly. The switch being
byte-identical when off is that claim's measurement, not its assertion.

## 1. What the switch is

`crate::hunter::PursuitStop` names the two readings the source already contains:

| variant | the predicate | who means it |
| --- | --- | --- |
| `ForwardHalfSpace` **(default)** | `body.x < capture_offset_body.x · scale + tolerance` | what the shipped line tests |
| `ReachEnvelope` | `effector_distance ≤ tolerance`, i.e. `ContactMeasure::in_contact()` | what the comment above it, and the rest of the file, say |

`ContactMeasure::pursuit_holds(stop, capture_forward)` is the **one place either rule is
written**. The hunt-intent pass, the strike record's own reading (`StrikeFrame::pursuit_holds`)
and every test evaluate it there, so a record can never transcribe a rule the world did not run.
`StrikeRecord` carries `stop`, the rule the attempt was made under, so a held/delivered reading
is never taken under the reader's rule and two arms are distinguishable inside one file.

**Not a `WorldConfig` field, deliberately.** A config field would move `calibrate::config_hash`
for every existing TOML and break the provenance of every retained row. The switch is a
`World`-level transient (`World::set_pursuit_stop`) living on the strike-path recorder, next to
the record it exists to be measured by: never persisted, never hashed, never checkpointed,
never read back by the tick. It is independent of recording — the variant runs with the record
off, and turning the record on or off never moves the rule.

The whole behavioural change is one expression:

```diff
-let inside = organisms.get(t).and_then(admission).is_some_and(|c| {
-    c.body.x < geometry.capture_offset_body.x + c.tolerance
-});
+let stop = strikes.pursuit_stop();
+let inside = organisms
+    .get(t)
+    .and_then(admission)
+    .is_some_and(|c| geometry.pursuit_holds(stop, &c));
```

### The tests, written to the brief's definitions and run red first

Six in `crates/cubarium-core/tests/hunter_pursuit_predicate.rs`. Before the predicate was
wired, the four definitional ones passed and the **two behavioural ones failed**, with
`the burst was requested but not delivered: 0.0019 px/s against a held cap of 0.2102 px/s` and
`the envelope changed nothing in 9,000 ticks of a two-apex world`. That is the red the brief
asks for, and it is what distinguishes a wired switch from a named one.

| test | what it fixes |
| --- | --- |
| `the_two_rules_are_the_two_written_predicates_and_differ_where_the_note_says` | both rules on hand-built geometry at the four places they can disagree, including that they **agree** on a prey past the claws — the shipped rule's stated purpose — and both boundaries as written (`<` strict, `≤` not) |
| `naming_the_default_rule_is_byte_identical_over_nine_thousand_ticks_of_a_two_apex_world` | the state hash at every 500-tick boundary of 9,000 ticks, untouched against explicitly-default, with the paid attempts asserted non-zero so the run is evidence |
| `the_variant_moves_the_same_two_apex_world_and_every_record_names_the_rule_it_ran_under` | the variant is not vacuous, and every record carries its own rule |
| `a_prey_ahead_but_outside_reach_holds_under_the_half_space_and_bursts_under_the_envelope` | the deciding fixture: a prey `tolerance + 2 px` **short** of the claws on the grasp's own axis. Held, the paid burst cannot exceed `rest_effort · speed_max` = 0.21 px/s and resolves `OutOfReach`; delivered, it exceeds ten times that. **The observable is the hunter's realised speed over its own paid burst, not a re-reading of the predicate.** |
| `a_prey_inside_reach_still_holds_under_both_rules` | the case the comment is about, which the correction must not change: in the grasp, both rules hold, both capture, neither moves the hunter |
| `a_frame_can_be_read_under_either_rule_and_the_record_reads_under_its_own` | the same instant under the other rule, which is how the arms are compared without re-running either |

Seven more in `crates/cubarium-search/tests/apex_pursuit_pair.rs` fix the two new aggregations
as **partitions**: held + delivered + no-strike-frame = every recorded attempt; the split is
taken under the rule each record was made under; the gap bins are half-open `[lo, hi)`, cover
the line, report every bin including an empty one, and sum to the totals with nothing lost; an
attempt with no intent frame has no initial gap and is counted rather than guessed into a bin;
both survive the merge across seeds; and an unrecognised `--pursuit-stop` is refused rather than
silently defaulted to the shipped rule.

## 2. The paired table

Eight held-out seeds, two candidates, 32 lives per arm. Separations and gaps in px of
`effector_distance`; speeds in px/s realised over the burst's own 1.0 s; `closed` is
`strike − resolution` separation, so **positive closes**.

| | `half-space` (shipped) | `reach-envelope` (variant) |
| --- | ---: | ---: |
| paid attempts | 894 | 969 |
| **held at the burst's start** | **799 (89.4 %)** | **52 (5.4 %)** |
| delivered | 47 (5.3 %) | **869 (89.7 %)** |
| no strike frame | 48 | 48 |
| mean initial gap (`sep@int`, all attempts) | 10.52 | 10.71 |
| **delivered:** separation at the burst's start | 5.14 | 11.44 |
| **delivered:** gap closed over the burst | +0.51 | −0.16 |
| **delivered:** hunter translation | 2.70 | **4.57** |
| **delivered:** hunter turn sweep at the 14.83 px radius | 1.73 | **7.96** |
| **delivered:** whole motor magnitude `\|v\| + r·\|ω\|` | 4.44 | **12.46** |
| **delivered:** prey realised speed | 3.80 | 2.61 |
| **whole-arm gap change per burst** | **−1.07** | **−0.21** |
| **contacts** (`resolved_in_reach`) | **88 (9.8 %)** | **140 (14.4 %)** |
| misses | 50 | 68 |
| **captures** | **38 (1.188 / life)** | **67 (2.094 / life)** |
| contact → capture | 43.2 % | 47.9 % |
| `OutOfReach` | 804 | 829 |
| `GraspUnmapped` | 0 | 5 |
| unaffordable refusals (no burst paid for) | 112 | 121 |
| apex lifetime, mean / median / max (ticks) | 12,440 / 12,186 / 16,585 | 13,164 / 12,282 / **23,201** |
| death cause | `Starvation` 32/32 | `Starvation` 32/32 |
| prey deaths by predation | 38 | 67 |
| prey population at introduction → end | 745 → 864 | 745 → 847 |

### Phase occupancy, over 398,075 and 421,258 member-ticks

| phase | `half-space` | `reach-envelope` |
| --- | ---: | ---: |
| perched | 54.3 % | 56.8 % |
| recovering | 25.5 % | 27.9 % |
| **stalking** | **11.3 %** | **5.1 %** |
| strike | 4.3 % | 4.4 % |
| windup | 3.1 % | 3.1 % |
| handling | 1.5 % | **2.7 %** |
| dormant | 0 % | 0 % |

N's first caution is confirmed: the same `hold` governs the stalk, and correcting it more than
halves the stalk's occupancy. A member that no longer parks at rest-effort beside its target
either reaches it (handling nearly doubles) or ends the gesture and goes back to perching.

### K's ledger, over 32 lives

| line (e) | `half-space` | `reach-envelope` | Δ |
| --- | ---: | ---: | ---: |
| upkeep — `(maintenance·S + sense_cost·r_sense)·dt` | 147.29 | 155.87 | +5.8 % |
| strike, retreat and handling (`other_energy_paid`) | 72.29 | 78.98 | +9.3 % |
| **motor — translation and turning together** | **3.65** | **8.55** | **+134 %** |
| total raised and spent | 223.23 | 243.39 | +9.0 % |
| credited from the gut | 7.58 | 13.55 | +79 % |
| credited by oxidising the founder reserve | 119.65 | 133.84 | |
| gut material per life (m) | 0.337 | **0.614** | +82 % |
| break-even intake per life (m) | 4.360 | 4.754 | |
| **fraction of its own bill the body earned** | **7.7 %** | **12.9 %** | |

N's second caution is also confirmed and bounded: an apex that closes does spend more on motor,
but motor is 1.6 % → 3.5 % of the bill and the extra intake is five times the extra spend. The
ledger is **better**, not worse. It is still a body that starves: every one of the 32 ends with
reserve and battery at zero, and the correction moves it from earning 7.7 % of its bill to
12.9 %.

### Captures by the gap the attempt began at

Bins are half-open in px of `effector_distance` at the intent frame; the first edge is the
contact tolerance to the pixel, the last is `strike_speed_px_s · strike_seconds`.

| initial gap | `half-space` n / contacts / captures (per life) | `reach-envelope` n / contacts / captures (per life) |
| --- | ---: | ---: |
| 0–4 px (already in the claws) | 94 / 42 / 18 (0.562) | 44 / 30 / 19 (0.594) |
| 4–8 px | 257 / 46 / 20 (0.625) | 238 / 89 / 39 (**1.219**) |
| **8–12 px** | 186 / **0** / **0** (0.000) | 332 / **17** / **9** (0.281) |
| 12–16 px | 167 / 0 / 0 | 221 / 4 / 0 |
| 16 px and beyond | 190 / 0 / 0 | 134 / 0 / 0 |

This is the clearest single reading in the workstream. The shipped rule converts an attempt into
a capture **only when the prey was already in or beside the claws** — 38 of 38 captures come
from inside 8 px, and 186 attempts that began at 8–12 px produced not one contact. Corrected,
the 4–8 px band nearly doubles its captures per life and the 8–12 px band becomes productive at
all for the first time. **Past 12 px nothing changes**: 355 attempts, 4 contacts, zero captures
in the corrected arm. The correction buys the apex the band between its claws and about 12 px,
and nothing beyond it.

## 3. The verdict, against Astra's rule

> *"The diagnosis is confirmed if the corrected arm delivers the burst, closes the gap, and
> increases contacts/captures; it is refuted if the held fraction falls but closure and contact
> do not improve."*
> — [round-3 review](ecology-v1-round3-review-2026-09-16.md), next steps item 1

| limb | reading |
| --- | --- |
| delivers the burst | **yes, decisively.** Held at the burst's start 89.4 % → 5.4 %; the shipped arm spent 89.4 % of its paid bursts at 0.75 px/s of whole motor, the corrected one spends 89.7 % of them at **12.46** |
| closes the gap | **partly.** The mean gap stops growing — −1.07 → −0.21 px per burst, and **+0.05 px on the independent four seeds** — but does not go positive over all eight |
| raises contacts | **yes.** 88 → 140, 9.8 % → 14.4 % of paid attempts |
| raises captures | **yes.** 38 → 67; 1.19 → 2.09 per life |

**Confirmed.** The refutation branch requires that closure *and* contact fail to improve; both
improve, and the held fraction falls by a factor of sixteen. Against Astra's owner-facing
wording — *"materially reduces the held fraction, moves the gap toward zero, and produces
captures without worsening lifetime/energy margin"* — all four hold: lifetime rises 5.8 % and
the energy margin, measured as the fraction of its own bill the body earns, rises from 7.7 % to
12.9 %.

The confirmation is about **the predicate**, not about the apex. N's diagnosis that the rule
suppresses the burst is now measured by intervention rather than corroborated by a
non-randomised subset, and the direction of every consequence Astra named is the predicted one.
Nothing here says the apex can feed itself: it earns an eighth of its bill instead of a
thirteenth, and starves 32 times out of 32.

## 4. Are the strike constants adequate once the lunge is delivered?

**No — and raising them is not the indicated repair, because the burst never reaches them.**

Astra's arithmetic: a nominal 16.667 px/s hunter against a prey at its 10 px/s escape cap closes
6.667 px/s, needs 1.25 s for the mean 8.3 px gap, and has only `strike_seconds = 1.0`. The
measured delivered burst is nothing like either number:

| quantity, over the 869 delivered bursts | value |
| --- | --- |
| `strike_speed_px_s` (profile, unchanged) | 16.667 px/s |
| hunter's realised **translation** | **4.57 px/s** |
| hunter's **turn sweep** at `turn_radius_px` = 14.83 px | **7.96 px/s** (64 % of the budget) |
| whole motor magnitude `\|v\| + r·\|ω\|` | 12.46 px/s |
| prey's realised speed over the same bursts | 2.61 px/s (cap 10.00) |
| relative closure available (`4.57 − 2.61`) | **≈ 1.96 px/s** |
| mean gap to close at the burst's start | 11.44 px |
| time that closure needs | **≈ 5.8 s**, against a 1.0 s burst |
| gap actually closed | **−0.16 px** |

Three things follow, and they are separable.

1. **`strike_speed_px_s` and `strike_seconds` are not reached.** The boost raises the member's
   `speed_cap` to 16.667 px/s, and the member converts 4.57 of it into travel. Raising either
   constant raises a ceiling the body is not touching.
2. **The binding term is the shared motor envelope at the claw-derived turn radius.**
   `motor::turn_radius_px` prices a member's turn at `|capture_offset| + capture_reach` =
   14.83 px, so a burst spent turning at 30°/s books 7.96 px/s of the budget before it moves an
   inch. This is the same arithmetic N found consuming the *rest-effort* envelope; corrected, it
   consumes the *boosted* one. It is the named next term, and this workstream did not touch it.
3. **The escape multiple is exonerated again, on new evidence.** The prey the corrected hunter
   actually chases realises 2.61 px/s, a quarter of its 10 px/s ceiling, and the corrected arm's
   prey speed is *lower* than the shipped arm's (3.80) because the hunter now closes on prey
   that are not fleeing at all. Lowering `escape_speed_multiple` would change nothing; raising
   it would not be what stopped these bursts.

The 12 px sense gene is the other half of the same picture and is **not** decided here: the
corrected arm's attempts still begin at a mean 11.1 px gap, and the gap-bin table says an attempt
that begins past 12 px has never once become a capture in either arm. Whether the apex should
sense further, or should not commit at all past 12 px, is a target-selection question this
record can name but not answer.

## 5. What Wrysk would be approving

The one-line change of the predicate to its own comment:

```text
c.body.x < geometry.capture_offset_body.x + c.tolerance    →    c.in_contact()
```

**Visible whenever an apex is spawned from the viewer**, and these are the visible consequences,
not inferred ones:

- **The apex charges.** Today a lanternjaw that has paid for a lunge sits at 0.30 px/s six
  pixels short of its prey and lets it walk away; corrected it moves at 12.5 px/s of whole motor
  during the burst. The stalk shortens from 11.3 % of its life to 5.1 %, and handling — visibly
  carrying a carcass — nearly doubles.
- **It eats about twice as often**: 1.19 → 2.09 captures per life, 38 → 67 prey taken across
  sixteen worlds. Prey population at the end falls 864 → 847 across sixteen worlds, about 2 %:
  a real but small ecological effect at this predator density.
- **One apex lived 23,201 ticks**, 97 % of its own minimum reproduction age, against a previous
  best of 16,585 (69 %). No member reached the gate, and readiness overlap is still zero in all
  16 runs, so this is not a mating.
- **A new outcome becomes visible**: `GraspUnmapped`, 5 of 969 paid attempts. A charging body
  can put its grasp centre off the surface at the open rim, where the world honestly refuses to
  publish a capture it cannot draw. It never happened in the shipped arm because the body never
  charged.
- **It does not fix the apex.** Every introduced adult still starves, at 12.9 % of what its own
  bill costs. Approving this corrects a rule that is wrong on its own terms; it is not a
  decision that the lineage is viable, and the next term — the turn radius inside the motor
  envelope — is untouched.

Nothing else moved: no escape speed, no sense radius, no strike duration, no mating radius
(`step.rs:600-665` untouched), no apex profile constant, no `WorldConfig` field, no equation, no
snapshot, no neural path, and no change to the display's apex unless the predicate itself is
adopted.

## What this does not establish

- **Nothing about viability.** 32 of 32 starve. The correction roughly doubles the intake of a
  body that needs about eight times more.
- **The widening was not pre-registered, and the contact limb of the verdict rests on it.**
  The brief's four-seed pair was run first. On it, contacts are **flat** — 46 against 46 — while
  captures rise 15 → 21 and the whole-arm gap change improves −0.91 → −0.45 px. The pair was then
  widened to all eight held-out seeds *because* sixteen lives cannot separate a 10 % contact rate
  from a 14 % one, which is a decision taken after seeing the first result and should be weighed
  as such. What makes it more than a second look: the widened run contains four seeds the first
  pair does not (9005–9008, sixteen further lives), and on those alone the effect is larger than
  the eight-seed average in every direction — contacts 42 → 94, captures 23 → 46, whole-arm gap
  change −1.23 → **+0.05** px. That subset is an out-of-sample replicate of the same
  intervention; the eight-seed totals are not. Both are in the artifacts, and a reader who wants
  only the brief's pair should use `half-space.json` and `reach-envelope.json`.
- **Two configurations, one lineage, one introduction tick, one horizon.** Nothing was tried at
  `--introduce-tick` other than 6,000, at a horizon other than 180,000, with an age-eligible
  founder, or with any ecology parameter moved.
- **Nothing about the capture roll.** Contact → capture rises 43.2 % → 47.9 %, which is inside
  what 88 and 140 contacts can distinguish; `capture_probability` was not examined.
- **Nothing about the sense radius or target selection**, beyond the gap-bin table's observation
  that no attempt beginning past 12 px has ever become a capture.
- **`hunter_motor_strike` has a long tail by construction.** The sweep term prices the *total*
  heading change over the burst at the claw radius, so a member that reverses heading books up to
  14.83·π ≈ 46.6 px/s. The delivered arm's maximum is 50.67 px/s and its mean is 12.46; the
  maximum is a reversal, not a violation of the envelope, which is evaluated per tick.
- **The `began_in_reach_resolved_out` class becomes very small in the corrected arm** (4 of 502
  on the brief's pair). Its reading is not usable there; the 93 % that began out of reach is.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## The next task this implies

Named, not launched, and it is the same shape as the one N named.

**Price the turn honestly, then re-measure this same table.** The delivered burst spends 64 % of
its budget on `motor::turn_radius_px` = `|capture_offset| + capture_reach` = 14.83 px, a radius
taken from the claws rather than from the body that is actually rotating. The apex is the one
member whose effector reaches far enough ahead for that choice to dominate its motor envelope,
and it dominates in both arms — at rest effort before the correction and at full boost after it.
The cheapest decisive experiment is another paired arm on exactly these numbers: held fraction
(now near zero and expected to stay there), delivered translation against 16.667, relative
closure, the gap-bin table, and K's ledger. The instrument is already committed and inert, and
the arm costs 60 s.

Two cautions for whoever takes it. First, the turn radius is not apex-specific: it is
`motor`'s, and every organism is priced through it, so an arm that changes it is not a
one-lineage change and must be read against the whole world, not only the apex. Second, an apex
that both closes *and* turns cheaply will take prey faster than 67 per sixteen worlds, and the
prey side — already down 2 % here — is the instrument that says whether that is an ecology or a
crash. The decision about the predicate, and about whether to run that arm at all, is Fable's.

## Verification

`cargo test -p cubarium-core` **523 passed / 0 failed / 4 ignored**, of which this workstream
contributes the 6 in `crates/cubarium-core/tests/hunter_pursuit_predicate.rs`.
`cargo test -p cubarium-search` **224 passed / 0 failed / 3 ignored**, of which this workstream
contributes the 7 in `crates/cubarium-search/tests/apex_pursuit_pair.rs`.
`cargo build --workspace --tests` clean. `graft build` rebuilt the graph.

The shared `target/` stale-artifact race E, K and N all documented recurred twice in this
session, as `no PursuitStop in hunter` immediately after the same symbol had compiled.
`touch crates/cubarium-core/src/lib.rs` and rebuild clears it, and the release binary was copied
out of `target/` before any arm was run so a concurrent rebuild could not change it mid-run.
