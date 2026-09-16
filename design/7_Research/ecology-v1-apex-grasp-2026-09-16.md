---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The grasp-only apex turn radius: the geometry is not what the apex is short of

> **Corrections after Astra's round-4 addendum (2026-09-16).** (1) The
> "correctness fix" reading is withdrawn. The two 9 px readers
> (`neural_observation`, `neural_decision`) serve neural animals only; an apex
> member runs the hunter intent path and is never neural, so no apex is "told"
> one radius and "bounded" by another, and under `Sweep` charging the outermost
> contacting point is internally consistent with that model. `Lobes` is an
> **experimental alternative**, not an established defect correction, and its
> default stays off. (2) "Robust" is too strong. The age-gate change is
> replicated beyond the influential run (two of the three crossing worlds are
> not `fast-leaf/9006`), but thirteen of sixteen paired worlds tie, so it is
> **the only behavioural change replicated beyond the influential run**, not a
> statistically separated one, and it is scoped to those four lives, not the
> apex generally; E's ratio at 12/16 is p = 0.077 and only translation billed
> reaches the nominal 0.05 line amid several inspected outcomes. (3) "Lower
> bound", "the disc model is needed for the gain" and "not closer to
> viability" are withdrawn. The direction of T's thinner-prey-world
> contribution is unknown (fewer, faster prey could help or hurt contact
> opportunity), so this workstream establishes only that `Lobes` does not
> reproduce most of the **observed, confounded** T arm; the apex remains
> nonviable (32/32 starve) while its earned share and age survival both
> improve. The prose below is repaired in place where it made those claims;
> the numbers stand.


Workstream U of ecology v1 round 4, to
[the brief](../handoffs/ecology-v1-apex-grasp-opus-2026-09-16.md): item 1 of the
reconciled round-4 next steps, Astra's "single most informative cheap experiment
now", and the intermediate
[T's note](ecology-v1-motor-inertial-2026-09-16.md) itself recommends before any
adoption of the inertial contract.

Evidence, not a decision. Nothing here is adopted.

## 0. The short of it

Under the shipped `Sweep` motor an apex member's turn radius is its **14.8 px
grasp**, not its **9 px lobes**, because `Sweep` charges the outermost
*contacting* point. This workstream makes that one choice a `World` transient and
runs P's eight-seed two-apex pair with it off and on — **the same motor contract,
the same pursuit rule, the same seeds, and, for the first time in this family of
experiments, the identical prey world at introduction** (745 against 745, row for
row).

- **The off arm is byte-identical to the retained rows.** Every field of every one
  of the 16 rows matches P's `reach-envelope-8` *and* T's `armA-sweep`, including
  every lifetime, every strike class, every gap bin and every ledger line.
- **The switch is real but small, and it is not the closure.** Pooled, the
  whole-arm gap change per burst goes `+0.212 → −0.031` px against `Inertial`'s
  `−0.574`, and captures per life `2.094 → 2.438` against `2.906`: **31 % and
  42 %** of the distance to the disc model.
- **That third does not survive the loss of one run.** Drop `fast-leaf/9006`
  alone — a single run whose contacts go 19 → 48 — and pooled captures **fall**
  58 → 51, contacts fall 121 → 117, and the gap still grows (`+0.326 → +0.147`).
  The paired sign tests over 16 runs are 9/16 on the gap, 9/14 on captures and
  11/16 on contacts: all consistent with no effect.
- **One limb is replicated beyond the influential run, and it is the one Astra's rule did not name.** Four of 32
  lives cross the 24,000-tick reproduction age gate under the switch, in **three
  separate runs**, against **zero of 32** with it off — and two of those three are
  the same runs `Inertial` crossed in. Usable energy rises in 12 of 16 runs,
  translation billed in 13 of 16, while turn billed *falls* in 11 of 16.
- **Verdict: refuted on the closure and capture limbs, by Astra's rule as
  written.** Geometry is not the principal apex motor defect. What the observed,
  confounded `Inertial` arm bought the hunt, the grasp correction alone does not
  buy; how much of that was the envelope and how much the thinner prey world is
  workstream W's question.

## 1. Build and provenance

- The switch, its tests and the CLI flag: `8d24519`, on the brief commit
  `a0a5957`, branch `worktree-agent-a6ee2f2c55eba3cae`.
- Both arms were produced by search build **`8d24519grasp`**, one release binary
  built once with `CUBARIUM_SEARCH_BUILD` pinned and copied out of the shared
  target directory before either arm ran, so no concurrent rebuild could change it
  under a running arm.
- Ecology: the two declared screen candidates, `baseline.toml` and
  `fast-leaf.toml`, re-derived and confirmed bit-for-bit at every seed
  (`matches_screen_candidate: true`, 16 of 16 in each arm).
- Two adults, never restocked, introduced at tick 6,000, horizon 180,000 ticks,
  8 held-out seeds × 2 candidates = 32 lives per arm, 8 workers, ledger and strike
  record on. Mass residual `0.0` in every row of both arms.

```bash
cubarium-search apex-audit \
  --config runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml \
  --seeds 8 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --pursuit-stop reach-envelope --motor sweep \
  --apex-turn-radius grasp|lobes --workers 8 \
  --out runs/ecology-v1-apex-grasp/{grasp,lobes}.json
```

Wall 55.4 s and 57.1 s: **113 s** against the brief's 3-minute cap. The two JSON
artifacts total **5.5 MiB** against a 20 MiB cap; `runs/` is git-ignored by design
and the commands above reproduce them.

## 2. What the switch is

`cubarium_core::motor::ApexTurnRadius::{Grasp, Lobes}`, and
`motor::turn_radius_px_in_with` is the **one place either rule is written**.
`turn_radius_px_in` is `Grasp` — the shipped rule — so every existing caller is
unchanged by construction and the two-argument `turn_radius_px` still is what it
always was.

| | radius in the `Sweep` rotation term | the apex's number |
| --- | --- | ---: |
| `Grasp` **(default, shipped)** | `max(lobes, \|capture_offset\| + capture_reach)` | 14.825 px |
| `Lobes` | `lobes` | 9.000 px |

`World::set_apex_turn_radius` is a transient in exactly the way
`set_motor_model` and `set_pursuit_stop` are: never persisted, never hashed,
**never a `WorldConfig` field** — which would move `calibrate::config_hash` for
every retained row. A resumed world runs `Grasp` until told otherwise.
`apex-audit --apex-turn-radius` selects it and the rule is recorded on the report
and on every row; a report written before the switch existed carries no rule and
reads as `grasp`.

**No ordinary body is reachable by it, by construction.** Only a hunter member is
ever handed contact geometry at all; without it both rules return the lobe extent.
That is measured, not asserted: 3,000 ticks of a world with no apex in it are
hash-identical under both rules.

Everything else about `Sweep` is untouched: the additive envelope
`|v| + r·|ω| ≤ cap`, `ROTATION_COST_SCALE = 0.5`, the strike's own 14.8 px reach,
the escape speeds, the pursuit rule and every profile constant.

### What the apex gains, stated

The grasp is 1.647× the lobes, so at the same budget the member **pivots 1.647×
faster** and its radian gets **cheaper** by the same factor (`Sweep` prices a
radian at `k·r`, and `r` is what fell). That is a different bargain from
`Inertial`, which gave the apex 2.33× the pivot at 0.859 of the price.

`world::view::neural_observation` and `world::step::neural_decision` read
`turn_radius_px_in(o, None, …)` — the lobe extent — but both serve neural animals
only; an apex member runs the hunter intent path and is never neural, so there is
no apex that is told one radius and bounded by another (Astra, addendum P1). The
switch is an experimental alternative to `Sweep`'s outermost-contacting-point rule,
not a correction of it.

### The tests, written from the definitions

13: ten in `crates/cubarium-core/tests/apex_grasp_radius.rs`, three in
`crates/cubarium-search/tests/apex_grasp_flag.rs`.

| test | what it fixes |
| --- | --- |
| `the_shipped_rule_is_byte_identical_to_the_build_that_never_heard_of_the_switch` | six pinned state hashes at 1,500-tick boundaries of 9,000 ticks of a world with two apex adults and eight neural animals, **printed by commit 2eb8a9f** before `MotorModel` and so before this switch existed; both the untouched default and an explicitly-named `Grasp` must reproduce them. That fixture was rebuilt verbatim and run **green at `a0a5957`, before any of the implementation was written** |
| `the_switch_gives_an_apex_member_its_own_lobe_extent` | 9 px and 14.8 px re-derived from the profile, not written down; `Grasp` is `max(lobes, grasp)` and `Lobes` is the lobes exactly |
| `an_ordinary_bodys_radius_is_untouched_by_the_rule` | every ordinary body of a live world, under both motor contracts |
| `a_world_without_an_apex_is_identical_under_both_rules` | 3,000 ticks, hash-equal at every tick |
| `the_observation_the_envelope_and_the_bill_read_the_same_number` | the three numbers agree under the switch and disagree without it, with the bill's `k·r·|ω|` taken from the resolver rather than from the algebra |
| `the_apex_pivots_the_grasp_ratio_faster_and_pays_less_per_radian` | ×1.647 pivot, ×0.607 per radian |
| `the_rule_cannot_move_the_inertial_model` | it is a `Sweep` rule; `Inertial` already drops the grasp |
| `the_switch_moves_the_same_two_apex_world`, `the_switch_is_a_transient_and_is_not_persisted`, `the_rule_names_itself_and_the_shipped_grasp_is_the_default` | not vacuous; no snapshot carries it; the names |
| `an_unrecognised_apex_turn_radius_is_refused_rather_than_defaulted`, `the_rule_an_arm_names_is_the_rule_its_world_runs`, `a_record_from_before_the_switch_reads_as_the_shipped_rule` | the flag, its independence from the other two transients, and what a missing field means |

Green: `cargo test -p cubarium-core --release` 554 passed / 0 failed / 4 ignored,
`cargo test -p cubarium-search --release` 278 passed / 0 failed / 5 ignored.

## 3. The paired table

Eight held-out seeds, two candidates, 32 lives per arm. `sweep` and `grasp-only`
are this workstream's pair; `inertial` is T's arm A **from its retained rows, not
re-run**. Separations in px of `effector_distance`, speeds in px/s over the
burst's own 1.0 s. **Gap change is signed `+ = the gap grew`** (T's convention;
P's "closed" column is its negation).

| | `sweep` (shipped) | `grasp-only` (this switch) | `inertial` (T, for comparison) |
| --- | ---: | ---: | ---: |
| **prey population at introduction (Σ 16 runs)** | **745** | **745** | 628 |
| prey population at end | 847 | 832 | 784 |
| recorded paid attempts | 969 | 1,011 | 1,020 |
| held at the burst's start | 52 (5.4 %) | 52 (5.1 %) | 65 (6.4 %) |
| delivered | 869 (89.7 %) | 880 (87.0 %) | 850 (83.3 %) |
| no strike frame | 48 | 79 | 105 |
| mean initial gap | 10.71 | 11.29 | 11.25 |
| **delivered:** separation at the burst's start | 11.44 | 12.28 | 12.50 |
| **delivered:** gap change over the burst | +0.157 | **−0.078** | −0.685 |
| **delivered:** hunter translation | 4.57 | **5.62** | 8.55 |
| **delivered:** hunter rotation term | 7.90 | **10.47** | 15.45 |
| **delivered:** whole motor `\|v\| + rot` | 12.46 | **16.10** | 24.00 |
| **delivered:** prey realised speed | 2.61 | 2.90 | 4.04 |
| **whole-arm gap change per burst** | **+0.212** | **−0.031** | **−0.574** |
| **contacts** (`resolved_in_reach`) | **140 (14.4 %)** | **165 (16.3 %)** | **207 (20.3 %)** |
| **captures** | **67 (2.094 / life)** | **78 (2.438 / life)** | **93 (2.906 / life)** |
| contact → capture | 47.9 % | 47.3 % | 44.9 % |
| `OutOfReach` / `GraspUnmapped` | 829 / 5 | 845 / 8 | 811 / 7 |
| apex lifetime mean / median / max (ticks) | 13,164 / 12,282 / 23,201 | 14,812 / 12,766 / **38,036** | 14,541 / 11,179 / 46,449 |
| **lives past the 24,000-tick age gate** | **0 / 32** | **4 / 32** | 3 / 32 |
| death cause | `Starvation` 32/32 | `Starvation` 32/32 | `Starvation` 32/32 |
| prey deaths by predation | 67 | 78 | 93 |
| readiness overlap (`ticks_two_ready`) | 0 | 0 | 0 |
| first refusing term | `reserve` 356,680 / 421,258 (84.7 %) | `reserve` 402,983 / 473,984 (85.0 %) | `reserve` 401,094 / 465,299 (86.2 %) |
| max reserve fraction reached | 0.500 | 0.500 | 0.518 |

The rotation term is `advertised_reach · |Δheading| / strike_seconds`, and
`advertised_reach` is the profile's 14.8249 px in **every** arm — it is a common
yardstick for how much the member turned, not the radius the model's envelope
used. Translation and the whole motor magnitude are means over different
admissible subsets, so the rotation row is their difference.

### The apex's ledger, over 32 lives (e)

| line | `sweep` | `grasp-only` | `inertial` |
| --- | ---: | ---: | ---: |
| upkeep | 155.87 | 175.37 | 172.16 |
| strike, retreat and handling | 78.98 | 82.60 | 83.32 |
| **motor — translation and turning** | **8.55** | **9.51** | **16.18** |
| — translation | 3.872 | **5.160** | 7.621 |
| — **turning** | 4.677 | **4.355** | 8.562 |
| motor share of the whole bill | 3.51 % | 3.56 % | 5.96 % |
| whole bill (upkeep + motor + strike and handling) | 243.40 | 267.49 | 271.67 |
| gut battery credit | 13.55 | 20.60 | 22.22 |
| gut reserve credit (m) | 19.65 | 30.30 | 31.90 |
| **E's usable-energy ratio** | **18.48 %** | **25.83 %** | **26.97 %** |

**The formula, used and not adapted.** The ratio is
`(gut battery credit + η_ox · e_r × gut reserve credit) ÷ (upkeep + motor + strike
and handling)` with `η_ox = 0.8` and `e_r = 2.0`, i.e. `0.8 · 2.0 = 1.6` times the
reserve credit, summed over all 32 lives' `BodyBudget` records. Applied to the
retained rows it returns **18.48 %** and **26.97 %**, reproducing the 18.5 % and
27.0 % T's corrected note published — which is the check that this column is E's
measure and not a re-invention of it.

**The mechanism, in one line of that table.** Turn billed *falls* 4.677 → 4.355
while translation rises 3.872 → 5.160. The member turns considerably more (the
rotation term rises 7.90 → 10.47 at the common yardstick) and still pays less for
turning, because a radian now costs `k · 9` instead of `k · 14.8`; the saving and
more goes into travel. That is the switch working exactly as its geometry says it
should.

### Captures by the gap the attempt began at

| initial gap | `sweep` n / contacts / captures | `grasp-only` n / contacts / captures | `inertial` n / contacts / captures |
| --- | ---: | ---: | ---: |
| 0–4 px | 44 / 30 / 19 | 39 / 25 / 14 | 44 / 27 / 12 |
| 4–8 px | 238 / 89 / 39 | 238 / **104** / **49** | 245 / 117 / 51 |
| 8–12 px | 332 / 17 / 9 | 259 / **26** / 9 | 282 / 42 / 18 |
| **12–16 px** | 221 / **4** / **0** | 301 / **10** / **6** | 272 / 17 / 10 |
| 16 px and beyond | 134 / 0 / 0 | 174 / **0** / **0** | 177 / 4 / 2 |

The 12–16 px band is the one qualitative change: under the shipped rule 221
attempts from that band produced four contacts and **no** capture; under the
switch 301 produce ten contacts and six captures. (T's own table prints
`221 / 0 / 0` for that cell where its §3 reproduction paragraph prints
`221/4/0`; the retained rows say four contacts and zero captures, and T's
sentence — "never once ended in the claws" — is about the captures and stands.) Beyond 16 px the switch still
produces nothing, where `Inertial` produces two. The member's motor magnitude in
the 12–16 band rises 15.03 → 16.92 under the switch and 15.03 → 28.23 under
`Inertial`, which is the size of what the two interventions actually buy.

## 4. What the pooled table hides

The pooled columns above are the honest arithmetic over 969 and 1,011 attempts,
and they are **dominated by one run**.

| pooled measure | all 16 runs | excluding `fast-leaf/9006` |
| --- | ---: | ---: |
| captures | 67 → **78** | 58 → **51** |
| captures per life | 2.094 → 2.438 | 1.933 → **1.700** |
| contacts | 140 → 165 | 121 → **117** |
| whole-arm gap change per burst | +0.212 → −0.031 | +0.326 → **+0.147** |
| E's usable-energy ratio | 18.48 % → 25.83 % | 17.74 % → **19.31 %** |
| mean apex lifetime (ticks) | 13,164 → 14,812 | 13,096 → 13,705 |

`fast-leaf/9006` alone goes 74 → 145 paid attempts, 19 → 48 contacts and 9 → 27
captures under the switch, carrying more than the whole pooled capture gain. It is
not a measurement error — it is a heavy tail, a member that ate well and lived
62,828 ticks across its two lives — and dropping it is a sensitivity statement,
not a correction. But a conclusion that reverses when the largest of sixteen
observations is removed is not a conclusion about the intervention.

The paired per-run view says the same thing directly.

| paired limb | runs improved | two-sided sign test |
| --- | ---: | ---: |
| whole-arm gap change | 9 / 16 | p = 0.80 |
| captures | 9 / 14 (2 ties) | p = 0.42 |
| contacts | 11 / 16 | p = 0.21 |
| apex lifetime | 11 / 16 | p = 0.21 |
| **E's usable-energy ratio** | **12 / 16** | **p = 0.077** |
| **motor translation billed** | **13 / 16** | **p = 0.021** |
| motor turn billed *fell* | 11 / 16 | p = 0.21 |

The two limbs that are not noise are the two that measure the **member's own
budget**, not its luck against prey: it spends more of its budget on travel, and
it turns the energy it does catch into a larger fraction of its own bill. The
hunting limbs — contacts, captures, closure — do not separate from run-to-run
variation at this sample size.

### The one behavioural change replicated beyond the influential run

**Zero of 32 lives ever crossed the 24,000-tick reproduction age gate with the
switch off. Four of 32 cross it with the switch on, in three different runs** —
`baseline/9002` (24,830), `baseline/9005` (25,578) and `fast-leaf/9006` (38,036
and 24,792). Two of those three runs are the same runs T's `Inertial` arm crossed
in (`baseline/9002` at 26,608, `baseline/9005` at 25,342), so this is not the
outlier's doing: drop `fast-leaf/9006` and two lives still cross where none did
before. Thirteen of sixteen paired worlds tie, so this is a replicated
descriptive change in three worlds, not a statistically separated one, and it
is a statement about those four lives.

T reported that under `Inertial` "the barrier has moved from *never lived long
enough* to *never stored enough*". **The grasp correction alone moves it**, in a
prey world identical to the shipped arm's. The first refusing term is `reserve`
in both arms and stays so, at 84.7 % and 85.0 % of watched member-ticks, and the
maximum reserve fraction any member reached is 0.500 in both: the age gate stops
being the binding term and nothing else opens behind it.

## 5. The verdict, by Astra's rule

Astra's rule: *confirmed that geometry is the principal apex motor defect if the
switch recovers most of `Inertial`'s −0.57 px closure and 2.9 captures per life in
the identical prey world; refuted if it stays near `Sweep`'s +0.21 px and 2.1.*

The prey world was identical — 745 at introduction in both arms, row for row,
which is the condition T's own arm A could not meet (628 against 745, 15 of 16
rows differing). Against that clean comparison:

- closure: `+0.212 → −0.031` against `−0.574` — **31 % recovered**, and `+0.147`
  with one run removed;
- captures per life: `2.094 → 2.438` against `2.906` — **42 % recovered**, and
  `1.700`, *below the shipped arm*, with one run removed;
- and neither limb separates from noise in the paired per-run test.

**Refuted.** The switch does not recover most of `Inertial`'s closure or its
captures; it moves a third of the way pooled, and that third rests on a single
run. **Geometry — the grasp counted as a turn radius — is not the principal apex
motor defect.**

**What this supports:** the geometry correction alone does not reproduce most of
the **observed, confounded** T arm. Whether the rest belongs to the disc envelope
or to T's thinner prey world is not decided here. Two qualifications:

1. **T's arm A is still confounded and this does not fix it.** `Inertial` was run
   with a 16 % thinner, faster-turning prey population, and part of its 207
   contacts and 93 captures is that world rather than that motor. The comparison
   above is `grasp-only` measured cleanly against a column that is not clean.
   The direction of that world's contribution is unknown — fewer, faster-turning
   prey could help or hurt contact opportunity — so the third recovered here is
   not a bound in either direction on what the envelope explains. Separating that
   needs an `Inertial` arm in which only the apex runs the disc model, and its
   complement, read as a 2 × 2 (workstream W).
2. **The switch is an experimental alternative, not a correction.** Under
   `Sweep` the grasp is charged because the model charges the outermost
   contacting point, which is internally consistent; no apex reads the 9 px
   figure (see §2). `Lobes` is byte-identical for every body that is not an apex
   member, costs 0.3 pp of the apex's own motor share and is the setting under
   which four lives crossed the age gate, but that is a reason to keep it
   available for experiments, not to ship it. Its default stays off whether or
   not `Sweep` remains the contract.

## 6. What this does not establish

- **Not that the apex is viable.** 32 of 32 still starve under the switch,
  though its earned share (18.5 → 25.8 %) and age survival both improve. Readiness overlap is zero in all 16 runs, no member ever became
  ready, and the maximum reserve fraction reached is 0.500 against the stock
  fraction the profile requires. The age gate opening means four members lived
  long enough to be refused for a different reason.
- **Not a whole-world reading.** This is arm A only. `--apex-turn-radius` cannot
  reach an ordinary body by construction, so there is no arm B to run — but that
  also means nothing here says anything about the ecology at large, and the
  `baseline` variety loss T flagged under `Inertial` has no counterpart to check.
- **Not a trained result.** Both arms run the world's own controllers; no policy
  was retrained, and the apex is not a neural animal in either.
- **Not a statement about eight seeds being enough.** It is precisely the finding
  of §4 that they are not, for the hunting limbs: a design that can be reversed by
  one run of sixteen cannot confirm or refute a 40 % effect. A capture-limb result
  at this effect size needs either many more seeds or a within-run measure with a
  larger `n` than 32 lives.
- **Not `GraspUnmapped`.** It rises 5 → 8 attempts, on a base of ~1,000; that is
  not a signal and the switch touches no grasp mapping.

## 7. Next, if this is pursued

1. **The clean `Inertial` attribution.** Arm A with the disc model applied to the
   apex only, so the prey world is the shipped one at introduction, exactly as
   this workstream's pair is. That is the one measurement that would say how much
   of `Inertial`'s hunting gain is its envelope and how much is its thinner world,
   and it is the same cheap eight-seed shape as this.
2. **A capture-limb design that can carry a verdict.** 32 lives and 1,000 attempts
   across 16 runs is not enough for a 40 % effect with this dispersion. More
   held-out seeds, or a per-attempt paired reading rather than a per-run one.
3. **The `Lobes` setting stays an experimental transient.** It is not a
   correctness question (§2) and §5 says it is not an ecological lever; it is
   decided, if ever, with the motor contract.

## Files

- Switch, tests and flag: `8d24519`.
  `crates/cubarium-core/src/motor.rs` (`ApexTurnRadius`,
  `turn_radius_px_in_with`), `crates/cubarium-core/src/world/mod.rs`
  (`set_apex_turn_radius`), `crates/cubarium-core/src/world/lifecycle.rs`,
  `crates/cubarium-core/src/world/step.rs` (one call site),
  `crates/cubarium-search/src/apex_audit.rs`,
  `crates/cubarium-search/src/main.rs`.
- Tests: `crates/cubarium-core/tests/apex_grasp_radius.rs`,
  `crates/cubarium-search/tests/apex_grasp_flag.rs`.
- Arms: `runs/ecology-v1-apex-grasp/{grasp,lobes}.json` (2.70 and 2.82 MiB).
- Compared against `runs/ecology-v1-apex-predicate/reach-envelope-8.json` and
  `runs/ecology-v1-motor-inertial/armA-{sweep,inertial}.json`, unmodified.
