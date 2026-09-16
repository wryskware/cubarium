---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The disc model on the apex alone: what T's arm A bought the hunt was bought by the prey

Workstream W of ecology v1 round 5, to
[the brief](../handoffs/ecology-v1-apex-motor-isolation-opus-2026-09-16.md) as
extended by Astra's review of it: item 1 of
[U's "next, if this is pursued"](ecology-v1-apex-grasp-2026-09-16.md#7-next-if-this-is-pursued),
item 3 of the reconciled round-4 next steps, and the one measurement
[T's note](ecology-v1-motor-inertial-2026-09-16.md) could not make because its
arm A moved two things at once.

Evidence, not a decision. Nothing here is adopted, and this note deliberately
recommends no contract.

## 0. The short of it

`World::set_apex_motor_model` names the motor contract a **hunter member** runs
while every ordinary body keeps the world's own. Because no ordinary body is
ever handed apex contact geometry, the prey world the founders are introduced
into is *the same world* in both halves of a motor pair — which T's arm A could
not manage, and which this note proves by hash rather than by prey count.

Four arms, the full 2×2 of `(ordinary-body motor) × (apex motor)`, at one pinned
build, on P's eight-seed two-apex design:

| pooled, 16 runs | S/S (shipped) | S/I (apex disc) | I/S (prey disc) | I/I (T's arm A) |
| --- | ---: | ---: | ---: | ---: |
| whole-arm gap change per burst | **+0.212** | **+0.369** | **−1.056** | **−0.574** |
| captures per life | 2.094 | 2.375 | 2.750 | 2.906 |
| contacts | 140 | 169 | 210 | 207 |

- **The apex's envelope is not what closed T's gap. The prey's is.** Put the
  disc model on the apex alone and the burst closes *worse* than shipped
  (`+0.212 → +0.369` px); put it on the ordinary bodies alone and the burst
  closes harder than T's whole-world arm ever did (`+0.212 → −1.056` px), in
  **15 of 15** runs that recorded an attempt, `p = 0.001` — the only hunting
  limb anywhere in this 2×2 that separates from run-to-run variation.
- **On captures the apex envelope does buy something, and it is about a
  third.** `2.094 → 2.375` per life against the ordinary-body contract's
  `2.094 → 2.750`; neither separates from noise per run (`p = 0.75` and
  `p = 0.79`).
- **Both controls reproduce, under this build, not by citation.** The S/S arm is
  field-for-field U's `grasp.json`; the I/I arm is field-for-field T's
  `armA-inertial.json` on every simulated field.
- **The pre-introduction state hash is equal 16/16 within each prey world and
  0/16 across it.** The isolation is exact, not approximate.
- **Verdict: mixed, and decided on the limb the brief cared about.** By the
  brief's rule as written the apex-only arm recovers *none* of `Inertial`'s
  −0.57 px closure (it moves the other way) and 35 % of its captures, and
  neither limb separates from noise — so the envelope is refuted as the source
  of the closure. But the brief's second branch does not fit either: the arm
  does not "stay near `grasp-only`'s −0.03 px". The fourth cell settles what
  neither branch could: the closure is the *ordinary bodies'* contract.

## 1. Build and provenance

- Switch, tests, flag and the pre-introduction hash: `2af98a2`, on the brief
  commit `c04348a`, branch `worktree-agent-ac905550e5a38506e`.
- All four arms were produced by search build **`2af98a2apexmotor`**, one
  release binary built once with `CUBARIUM_SEARCH_BUILD` pinned and copied out
  of the shared target directory before any arm ran.
- Ecology: the two declared screen candidates, `baseline.toml` and
  `fast-leaf.toml`, re-derived and confirmed bit-for-bit at every seed
  (`matches_screen_candidate: true`, 16 of 16 in all four arms).
- Held fixed and identical across all four arms: the binary; the two
  configurations and their `config_hash`; the eight held-out seeds
  9001–9008; `FixedHunterProfile::lanternjaw_trial` derived from
  `evaluate::base_config(seed)` (so genome, phenotype, grasp and reach are the
  founder's own in every arm); two founders at `founder_age_seconds = 0.0`
  placed by `evaluate::apex_targets(seed, 2)`; introduction at tick 6,000, never
  restocked; horizon 180,000 ticks; `--pursuit-stop reach-envelope`;
  `--apex-turn-radius grasp` (the shipped rule); ledger and strike recorder on
  in every arm; 8 workers. Mass residual ≤ 3.8 × 10⁻¹⁰ in every row of every
  arm.
- The only two variables are `--motor` (what every ordinary body runs) and
  `--apex-motor` (what a hunter member runs).

```bash
cubarium-search apex-audit \
  --config runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml \
  --seeds 8 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --pursuit-stop reach-envelope --apex-turn-radius grasp \
  --motor {sweep,inertial} --apex-motor {sweep,inertial} --workers 8 \
  --out runs/ecology-v1-apex-motor-isolation/w-<motor>--a-<apex-motor>.json
```

Wall 57.9 s, 64.2 s, 61.7 s and 59.2 s: **243 s** against the brief's 5-minute
cap for four arms. The four JSON artifacts total **11.2 MiB** against the 20 MiB
cap; `runs/` is git-ignored by design and the command above reproduces them.

## 2. What the switch is

`World::set_apex_motor_model(Option<MotorModel>)`, and `motor::model_for_body`
is the **one place** the per-body choice is written:

```rust
match apex_override {
    Some(model) if apex.is_some() => model,
    _ => world,
}
```

`apex` is the `hunter::ContactGeometry` that `world::step` builds for a body,
and it is `Some` only when the world has a hunter profile *and* the body is a
member. All four of `world::step`'s per-body motor reads — the envelope radius
(≈ L1385), the resolver (≈ L1403), the bill (≈ L1454) and the rotation price the
budget split uses (≈ L1472) — now take their contract from `model_for_body`
rather than from the world's `motor_model` directly. With `apex_motor_model ==
None` that is `motor_model` for every body, arithmetic for arithmetic.

It is a transient in exactly the way `set_motor_model`, `set_pursuit_stop` and
`set_apex_turn_radius` are: never persisted, never hashed, **never a
`WorldConfig` field** — which would move `calibrate::config_hash` for every
retained row. A resumed world runs `None`.

It is a *selector*, not a one-way flag: `Some(Sweep)` in an `Inertial` world
puts the member back on the shipped envelope while every other body keeps the
disc. That is the fourth cell of Astra's 2×2, and it is pinned by a test as well
as run as an arm.

**Where the contract is recorded.** The brief allowed the strike record's motor
name or a new per-row field. The strike record lives in
`crates/cubarium-core/src/hunter/`, which this workstream must not touch, so it
is a new per-row and per-report field `apex_motor: Option<String>`, read through
`AuditRow::apex_motor_ran()` / `AuditReport::apex_motor_ran()`. `None` — and
absent, on every row written before the override existed — means "the world's
own `motor`", which is exactly what those rows mean.

**The pre-introduction hash (Astra's control).** Every row now carries
`pre_introduction_state_hash`: the complete `snapshot::state_hash` at the
introduction tick, taken **before** the founders are placed. It is read-only and
draws nothing. Two arms that share `--motor` must agree on it row for row,
because at that instant no member exists and the override is unreachable.

### The tests, written from the definitions

13: nine in `crates/cubarium-core/tests/apex_motor_isolation.rs`, four in
`crates/cubarium-search/tests/apex_motor_flag.rs`.

| test | what it fixes |
| --- | --- |
| `default_off_is_byte_identical_to_the_build_that_never_heard_of_the_override` | T's six pinned state hashes at 1,500-tick boundaries of 9,000 ticks of a two-apex, eight-neural-animal world, **printed by commit 2eb8a9f** before `MotorModel` existed. The untouched default, an explicit `None`, and an explicit `Some(Sweep)` must all reproduce them. U's fixture was rebuilt verbatim and run **green at `c04348a`, before any implementation was written** |
| `an_apex_members_four_reads_are_the_overrides_and_an_ordinary_bodys_are_the_worlds` | the envelope radius, the resolver, the bill and the rotation price under the override are the disc model's for the member (radius re-derived as `lobes/√2`, price 1.0) and the world's for an ordinary body (price `ROTATION_COST_SCALE`), each taken from the entry point `world::step` takes it from |
| `the_member_runs_the_override_and_every_other_body_runs_the_worlds_own` | the same claim **measured in the world**: three worlds byte-identical up to one scripted tick, the ledger opened for that tick alone. Every member's `motor_translation_billed`, `motor_turn_billed` and `bill_total` under `Sweep` + the override *equal* the whole-world `Inertial` ones and differ from plain `Sweep`; every ordinary body's equal plain `Sweep`'s and differ from whole-world `Inertial`'s. Non-vacuity is asserted on both sides |
| `a_sweep_override_in_an_inertial_world_puts_the_member_back_on_the_shipped_contract` | the same reading in the other direction — the 2×2's fourth cell in the code |
| `a_world_that_never_sees_an_apex_is_identical_under_the_override` | 3,000 ticks, hash-equal at every tick, with the override set and no apex ever introduced |
| `the_override_moves_the_same_two_apex_world` | not vacuous; both worlds still close their books |
| `the_default_override_is_none_and_none_is_the_worlds_own_contract`, `the_override_runs_either_contract_in_either_direction`, `the_override_is_a_transient_and_is_not_persisted` | the default, the selector, and that no snapshot carries it |
| `an_unrecognised_apex_motor_is_refused_rather_than_defaulted`, `the_contract_an_arm_names_is_the_contract_its_world_runs`, `the_record_says_which_contract_the_member_ran`, `a_record_from_before_the_override_reads_as_the_worlds_own_contract` | the flag, its independence from the other three transients, that the two arms differ in exactly one recorded field, and what a missing field means |

Green: `cargo test -p cubarium-core --release` **563 passed / 0 failed / 4
ignored**, `cargo test -p cubarium-search --release` **282 passed / 0 failed / 5
ignored**.

## 3. The controls, reproduced rather than cited

Astra asked for both controls to be re-run under this build rather than read out
of retained files.

| this build's arm | against | result |
| --- | --- | --- |
| `--motor sweep --apex-motor sweep` | U's `ecology-v1-apex-grasp/grasp.json` | **every report field and all 16 rows match field for field**, excluding only `elapsed_ms` and the two fields this build added. Verdict string equal |
| `--motor inertial --apex-motor inertial` | T's `ecology-v1-motor-inertial/armA-inertial.json` | **every simulated field of all 16 rows matches.** The one key that differs is `apex_turn_radius`: absent from T's record, which predates U's switch, and whose serde default *is* `grasp` — the same rule. Verdict string equal |

So `--apex-motor X` with `--motor X` is byte-identical to `--motor X` alone, at
both contracts, on a 180,000-tick 16-run arm, as `model_for_body` says it must
be.

### The isolation, by hash

| | pre-introduction `state_hash` equal | prey at introduction equal |
| --- | ---: | ---: |
| S/S vs S/I (same prey contract) | **16 / 16** | 16 / 16 (745 = 745) |
| I/S vs I/I (same prey contract) | **16 / 16** | 16 / 16 (628 = 628) |
| S/* vs I/* (different prey contract) | **0 / 16** | 0 / 16 (745 ≠ 628) |

The founders of an S/S and an S/I run are dropped into *the same world state*,
not merely into the same prey count. That is the condition T's arm A could not
meet and the whole reason this workstream exists.

## 4. The table

Eight held-out seeds, two candidates, 32 lives per arm. `grasp-only` is U's
retained `lobes.json`, unmodified, for reference. Separations in px of
`effector_distance`, speeds in px/s over the burst's own 1.0 s. **Gap change is
signed `+ = the gap grew`** (T's and U's convention).

| | `S/S` shipped | `grasp-only` (U) | `S/I` apex disc | `I/S` prey disc | `I/I` (T) |
| --- | ---: | ---: | ---: | ---: | ---: |
| **prey at introduction (Σ 16)** | **745** | **745** | **745** | 628 | 628 |
| prey at end | 847 | 832 | 878 | 798 | 784 |
| recorded paid attempts | 969 | 1,011 | 1,013 | 1,017 | 1,020 |
| held at the burst's start | 52 | 52 | 58 | **102** | 65 |
| delivered | 869 | 880 | 874 | 869 | 850 |
| no strike frame | 48 | 79 | 81 | 46 | 105 |
| mean initial gap | 10.71 | 11.29 | 11.66 | 10.52 | 11.25 |
| **delivered:** separation at start | 11.44 | 12.28 | 12.84 | 11.95 | 12.50 |
| **delivered:** gap change | +0.157 | −0.078 | **+0.354** | **−1.280** | −0.685 |
| **delivered:** hunter translation | 4.57 | 5.62 | 8.14 | 4.86 | 8.55 |
| **delivered:** whole motor `\|v\|+rot` | 12.46 | 16.09 | 24.24 | 12.54 | 24.00 |
| **delivered:** prey realised speed | 2.61 | 2.90 | 2.74 | 3.90 | 4.04 |
| **whole-arm gap change per burst** | **+0.212** | **−0.031** | **+0.369** | **−1.056** | **−0.574** |
| **contacts** (`resolved_in_reach`) | **140** | **165** | **169** | **210** | **207** |
| **captures** | **67** | **78** | **76** | **88** | **93** |
| **captures per life** | **2.094** | **2.438** | **2.375** | **2.750** | **2.906** |
| contact → capture | 47.9 % | 47.3 % | 45.0 % | 41.9 % | 44.9 % |
| `OutOfReach` / `GraspUnmapped` | 829 / 5 | 845 / 8 | 843 / 6 | 805 / **27** | 811 / 7 |
| apex lifetime mean / median / max | 13,164 / 12,282 / 23,201 | 14,812 / 12,766 / 38,036 | 13,615 / 10,636 / 37,812 | 14,189 / 12,656 / 29,274 | 14,541 / 11,178 / 46,449 |
| **lives past the 24,000-tick gate** | **0 / 32** | 4 / 32 | **2 / 32** | **1 / 32** | 3 / 32 |
| death cause | `Starvation` 32/32 | 32/32 | 32/32 | 32/32 | 32/32 |
| prey deaths by predation | 67 | 78 | 76 | 88 | 93 |
| readiness overlap (`ticks_two_ready`) | 0 | 0 | 0 | 0 | 0 |
| first refusing term (`reserve`) | 84.7 % | 85.0 % | 86.4 % | 82.9 % | 86.2 % |
| max reserve fraction reached | 0.500 | 0.500 | 0.500 | 0.500 | 0.518 |

The rotation term is `advertised_reach · |Δheading| / strike_seconds`, and
`advertised_reach` is the profile's 14.8249 px in **every** arm — a common
yardstick for how much the member turned, not the radius any model's envelope
used. Translation and the whole motor magnitude are means over different
admissible subsets, so the rotation is their difference: 7.89, 10.47, **16.10**,
7.68 and 15.45 px/s across the five columns.

### The apex's ledger, over 32 lives (e)

| line | `S/S` | `grasp-only` | `S/I` | `I/S` | `I/I` |
| --- | ---: | ---: | ---: | ---: | ---: |
| upkeep | 155.87 | 175.37 | 161.20 | 168.00 | 172.16 |
| strike, retreat and handling | 78.98 | 82.60 | 82.85 | 83.10 | 83.32 |
| **motor — translation and turning** | **8.55** | **9.51** | **15.28** | **9.57** | **16.18** |
| — translation | 3.872 | 5.160 | 7.251 | 4.273 | 7.621 |
| — turning | 4.677 | 4.355 | **8.026** | 5.298 | 8.562 |
| motor share of the whole bill | 3.51 % | 3.56 % | 5.89 % | 3.67 % | 5.96 % |
| whole bill | 243.39 | 267.49 | 259.32 | 260.67 | 271.67 |
| gut battery credit | 13.55 | 20.60 | 18.47 | 19.02 | 22.22 |
| gut reserve credit (m) | 19.65 | 30.30 | 26.53 | 27.03 | 31.90 |
| **E's usable-energy ratio** | **18.48 %** | **25.83 %** | **23.49 %** | **23.89 %** | **26.97 %** |

**The formula, used and not adapted.** `(gut battery credit + η_ox · e_r × gut
reserve credit) ÷ (upkeep + motor + strike and handling)`, `η_ox = 0.8`,
`e_r = 2.0`, summed over all 32 lives' `BodyBudget` records. Applied to the
retained rows it returns **18.48 %** and **26.97 %**, reproducing the figures
T's corrected note and U's note published — which is the check that this column
is E's measure and not a re-invention of it. (The same re-derivation reproduces
U's whole published table for the `sweep` and `inertial` columns to the printed
digit, including `+0.212 / −0.574`, `11.44 / 12.50`, `+0.157 / −0.685`, every
ledger line and every gap bin; the pipeline is validated against a known
answer before it is pointed at new arms.)

### Captures by the gap the attempt began at

| initial gap | `S/S` n / contacts / captures | `S/I` | `I/S` | `I/I` |
| --- | ---: | ---: | ---: | ---: |
| 0–4 px | 44 / 30 / 19 | 44 / 26 / 10 | 88 / 61 / 14 | 44 / 27 / 12 |
| 4–8 px | 238 / 89 / 39 | 232 / 107 / 52 | 283 / 124 / 59 | 245 / 117 / 51 |
| 8–12 px | 332 / 17 / 9 | 230 / 27 / 11 | 278 / 18 / 10 | 282 / 42 / 18 |
| 12–16 px | 221 / 4 / 0 | 291 / 8 / 2 | 174 / 6 / 4 | 272 / 17 / 10 |
| 16 px and beyond | 134 / 0 / 0 | 216 / 1 / 1 | 194 / 1 / 1 | 177 / 4 / 2 |

Read as a **rate per attempt**, the apex contract is the one that moves the
mid-and-long bins:

| contact rate / capture rate per attempt | `S/S` | `S/I` | `I/S` | `I/I` |
| --- | ---: | ---: | ---: | ---: |
| 0–4 px | 68.2 % / 43.2 % | 59.1 % / 22.7 % | 69.3 % / 15.9 % | 61.4 % / 27.3 % |
| 4–8 px | 37.4 % / 16.4 % | **46.1 % / 22.4 %** | 43.8 % / 20.8 % | 47.8 % / 20.8 % |
| 8–12 px | 5.1 % / 2.7 % | **11.7 % / 4.8 %** | 6.5 % / 3.6 % | 14.9 % / 6.4 % |
| 12–16 px | 1.8 % / 0.0 % | 2.7 % / 0.7 % | 3.4 % / 2.3 % | 6.2 % / 3.7 % |
| 16 px + | 0.0 % / 0.0 % | 0.5 % / 0.5 % | 0.5 % / 0.5 % | 2.3 % / 1.1 % |

## 5. The 2×2, and why the pooled closure moves the way it does

### The four cells and their differences

| pooled measure | apex effect \| sweep prey | apex effect \| inertial prey | **interaction** | ordinary effect \| apex sweep | ordinary effect \| apex inertial |
| --- | ---: | ---: | ---: | ---: | ---: |
| gap change per burst (px) | **+0.157** | **+0.482** | **+0.325** | **−1.269** | **−0.944** |
| captures per life | +0.281 | +0.156 | −0.125 | **+0.656** | +0.531 |
| contacts | +29 | −3 | −32 | **+70** | +38 |
| E's usable-energy ratio (pp) | +5.01 | +3.08 | −1.93 | +5.40 | +3.48 |
| mean lifetime (ticks) | +450 | +352 | −99 | +1,025 | +926 |
| lives past the age gate | +2 | +2 | 0 | +1 | +1 |

**The apex effect has the same sign in both prey worlds on every row, and so
does the ordinary-body effect.** The interaction is real and not small on the
gap (+0.325 px on an apex effect of +0.157), so T's whole-world gain is not the
sum of its parts and no clean apportionment of it exists — which is why this
note reports the two main effects and the interaction rather than "X % of T's
gain". But the ordering is not order-dependent: the apex contract makes the gap
*grow* under either prey contract, and the prey contract makes it *shrink* under
either apex contract.

### Paired per-run sign tests

Two-sided exact binomial over the 16 runs. `baseline/9006` records no paid
attempt at all in either inertial-prey arm (both members live ~16,400 ticks and
never strike), so it is dropped from the gap test in any pair that involves one
of those arms and the drop is stated.

| paired limb | apex effect, sweep prey | apex effect, inertial prey | ordinary effect, apex sweep | ordinary effect, apex inertial |
| --- | ---: | ---: | ---: | ---: |
| whole-arm gap change | 7/16, p = 0.80 | 4/15, p = 0.12 | **15/15, p = 0.001** | 10/15, p = 0.30 |
| captures | 4/10 (6 ties), p = 0.75 | 8/15, p = 1.00 | 8/14 (2 ties), p = 0.79 | 6/12 (4 ties), p = 1.00 |
| contacts | 8/14 (2 ties), p = 0.79 | 6/15, p = 0.61 | 10/15, p = 0.30 | 7/16, p = 0.80 |
| apex lifetime | 5/16, p = 0.21 | 4/16, p = 0.077 | 9/16, p = 0.80 | 10/16, p = 0.45 |
| E's usable-energy ratio | 6/16, p = 0.45 | 5/15, p = 0.30 | 9/16, p = 0.80 | 8/16, p = 1.00 |
| motor translation billed | **14/16, p = 0.004** | 12/16, p = 0.077 | 7/16, p = 0.80 | 7/16, p = 0.80 |
| motor turn billed *fell* | **0/16, p < 0.001** | **1/16, p = 0.001** | 6/16, p = 0.45 | 8/16, p = 1.00 |

**Exactly one hunting limb in this 2×2 separates from noise, and it is the
ordinary-body effect on closure: 15 of 15.** Everything the apex contract does
separately is a statement about the member's *own budget* — it bills more
translation (14/16) and unambiguously more turning (16/16, because the disc
model prices a radian at 1.0 rather than `ROTATION_COST_SCALE = 0.5`) — exactly
the pattern U found for the grasp switch, at a larger size.

### Why the apex-only arm closes *worse* while catching more

The apex on the disc model travels nearly twice as fast in a delivered burst
(4.57 → 8.14 px/s) and turns far more (rotation term 7.89 → 16.10), and its
contact rate rises in every bin from 4 px outward. It also **starts its bursts
from further away**: mean initial gap 10.71 → 11.66, separation at the burst's
start 11.44 → 12.84, and the 16 px-and-beyond bin goes 134 → 216 attempts while
0–12 px goes 614 → 506. Mean closure per burst is a composition-sensitive
statistic and the composition moved outward. Prey realised speed barely changes
(2.61 → 2.74), so this is not the prey escaping better; it is the member
committing to attempts it could not previously afford to start, most of which
still lose ground.

Under the *prey* disc model the mechanism is the opposite and much larger: prey
realised speed rises 2.61 → 3.90, the apex's own motor is untouched (whole motor
12.46 → 12.54, translation 4.57 → 4.86), and the gap nevertheless closes by
1.27 px more per burst. What the disc model does to an ordinary body is give it
√2 the pivot rate at √2 the price per radian under a quadrature envelope; the
visible consequence in the hunt is that prey spend their budget turning instead
of fleeing in a straight line, and the member closes on them. That is a
statement about the **prey's** contract that T's arm A could not separate and
this one can.

## 6. Sensitivity

U's conclusion reversed when `fast-leaf/9006` was dropped. The same drop here:

| arm, `fast-leaf/9006` removed | captures (per life) | contacts | gap change | usable energy |
| --- | ---: | ---: | ---: | ---: |
| `S/S` | 58 (1.933) | 121 | +0.326 | 17.74 % |
| `grasp-only` | 51 (1.700) | 117 | +0.147 | 19.31 % |
| `S/I` | 67 (2.233) | 146 | +0.477 | 22.20 % |
| `I/S` | 83 (2.767) | 203 | **−1.033** | 23.62 % |
| `I/I` | 85 (2.833) | 194 | −0.568 | 27.51 % |

Both of this note's findings survive it. The apex-only capture gain survives
(+0.30/life, where U's grasp switch reversed to −0.23/life), and the
ordinary-body closure result is essentially untouched (−1.056 → −1.033). The
heavy tail has moved: under `S/I` the dominant run is `fast-leaf/9005`
(27 captures, 53 contacts), not `9006`.

## 7. The verdict, by the brief's rule and Astra's third branch

The brief: *the envelope is the source of T's apex gain if the apex-only arm in
the identical prey world recovers most of `Inertial`'s −0.57 px closure and 2.9
captures per life and the paired tests separate it from noise; the thinner prey
world was the source if it stays near `grasp-only`'s −0.03 px and 2.4.*

- closure: `+0.212 → +0.369` against `−0.574`. **None of it is recovered** — the
  apex-only arm moves the gap the *wrong way*, and it is not near `grasp-only`'s
  −0.031 either.
- captures per life: `2.094 → 2.375` against `2.906`. **35 % recovered**, and
  indistinguishable from `grasp-only`'s 2.438.
- and neither limb separates from noise in the paired per-run test.

**Mixed, on the brief's two branches as written: refuted on closure, a third
recovered on captures, and the second branch does not describe the arm either.**
Astra's third branch is the one that fits, and the fourth cell says what neither
of the first two could:

> **The apex's envelope is not the source of T's closure gain. The ordinary
> bodies' contract is** — `I/S` alone produces −1.056 px, more closure than T's
> whole-world arm, in 15 of 15 runs at `p = 0.001`. On captures the two
> contracts contribute in the same direction, roughly one third to the apex
> (+0.28/life) and two thirds to the prey (+0.66/life), and neither separates
> from run-to-run noise at this sample size.

What remains unexplained, either way:

1. **Why the interaction is +0.325 px on the gap.** The apex effect is three
   times larger under an inertial prey world than under a sweep one. The
   composition argument of §5 predicts a positive interaction (a faster member
   in a world of faster-turning prey starts bursts from further still) but does
   not predict its size, and nothing here measures the attempt-initiation
   decision directly.
2. **The `I/S` cell's own anomalies.** 102 attempts held at the burst's start
   against 52–65 elsewhere, `GraspUnmapped` 27 against 5–8, and an 0–4 px bin of
   88 attempts against 44. A member on the shipped envelope inside a world of
   disc-model prey meets the contact geometry far more often and at short range,
   and this note does not explain why it converts so little of it (15.9 %
   capture rate in that bin against 43.2 % shipped).
3. **Nothing here says the apex is viable.** 32 of 32 starve in all four arms,
   readiness overlap is zero in all 64 runs, no member ever became ready, and the
   maximum reserve fraction reached is 0.500 in three arms and 0.518 in the
   fourth. `reserve below the stock fraction` is the first refusing term on
   83–86 % of watched member-ticks everywhere. The age gate opens a little in
   three of the four non-shipped cells and nothing opens behind it.
4. **Eight seeds still cannot carry a capture-limb verdict.** U's §6 finding
   stands: 32 lives and ~1,000 attempts across 16 runs is not enough for a 30–40 %
   effect at this dispersion. The one result here that *is* robust — the
   ordinary-body closure effect — is robust because it is 15/15, not because the
   sample grew.
5. **This is arm A only, and untrained.** No whole-world reading, no retraining,
   and the apex is not a neural animal in any arm. The `baseline` variety loss T
   flagged under `Inertial` has no counterpart measured here.

**No contract is recommended.** Fable and Astra decide that.

## Files

- Switch, tests, flag, pre-introduction hash: `2af98a2`.
  `crates/cubarium-core/src/motor.rs` (`model_for_body`),
  `crates/cubarium-core/src/world/mod.rs` (`set_apex_motor_model`),
  `crates/cubarium-core/src/world/lifecycle.rs`,
  `crates/cubarium-core/src/world/step.rs` (`body_model`, four call sites),
  `crates/cubarium-search/src/apex_audit.rs`,
  `crates/cubarium-search/src/main.rs`.
- Tests: `crates/cubarium-core/tests/apex_motor_isolation.rs`,
  `crates/cubarium-search/tests/apex_motor_flag.rs`.
- Arms: `runs/ecology-v1-apex-motor-isolation/w-{sweep,inertial}--a-{sweep,inertial}.json`
  (2.70–2.84 MiB each, 11.2 MiB total).
- Compared against `runs/ecology-v1-apex-grasp/{grasp,lobes}.json` and
  `runs/ecology-v1-motor-inertial/armA-{sweep,inertial}.json`, unmodified.
