---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Why an introduced apex dies, and what refuses it when the age gate is open

Workstream K of the ecology v1 next steps
([brief](../handoffs/ecology-v1-apex-eligibility-opus-2026-09-16.md)), step 4 of the reconciled
next steps, from Astra's review (finding 4, apex paragraph).

E's [opportunity audit](ecology-v1-budget-2026-09-16.md) established that two introduced apex
adults were never simultaneously able to reproduce, and that the reason was upstream of the
10 px mating radius: every adult died at 43–59 % of the `reproduce_min_age_seconds` its own
profile demands. It left two questions open, in its own words: it did *not* measure whether the
stock fractions would also have failed, nor why an introduced adult dies at ~11,000 ticks
against a 7,200 s natural lifespan. Both are now measured.

- **What kills it: starvation, and specifically a total failure to feed.** Sixteen of sixteen
  introduced adults died of `Starvation`. Not one of them ever took a single unit of material
  from any field channel, and the fifteen captures the sixteen of them managed between them
  returned 4.19 m in total against the ~4.26 m each one alone needed to break even. An introduced apex
  lives for as long as the inventory it was founded with pays for, and then dies.
- **The age gate is not the binding one.** With both adults introduced already past
  `reproduce_min_age_seconds`, readiness still never opened, in eight runs and 206,763
  member-ticks. The term that refused them first was the **reserve stock fraction**, on 79.9 %
  of those ticks, and the age term was never once reached. The reserve of an introduced apex
  **never rose above the 0.5 of `R_max` it was founded with**, in any life of any run, against
  the 0.8 `may_reproduce` demands.

Astra's three branches were *readiness never opens*, *opens but no pair forms*, and *pairs form
and fail distance*. This probe reached the first, and the audit can now name the term.

## Build and provenance

- The door, the ledger wiring and the audit's new records: `ec06d68`.
- All three artifacts were produced by search build id `ec06d6859be3`, i.e. the tree at
  `ec06d68`, clean.
- Ecology: the same two declared screen candidates E used —
  `runs/ecology-v1-calibration/selected/baseline.toml` (config hash `fc1aefa33ebd70a1`) and
  `fast-leaf.toml` (`09e244392ec91768`). Every row re-derived the declared candidate at its own
  seed and confirmed the loaded configuration is bit-for-bit that candidate
  (`matches_screen_candidate: true`, 24 of 24).
- Held-out seeds 9001–9004, two adults, never restocked, horizon 180,000 ticks, 8 workers.

```bash
# 1. Death diagnosis: E's arm exactly, with the per-body ledger on.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --workers 8 --out runs/ecology-v1-apex/death.json

# 2. The eligibility probe: both adults introduced already age-eligible.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 24000 --founder-age-seconds 1200 \
  --workers 8 --out runs/ecology-v1-apex/probe-aged.json

# 3. Its matched control: the same tick, the same everything, age zero.
cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 24000 --founder-age-seconds 0 \
  --workers 8 --out runs/ecology-v1-apex/probe-control.json
```

Wall time 27.9 s, 27.9 s and 28.5 s: **84.3 s** of simulation against the brief's 8-minute cap,
8 workers. The three JSON artifacts total **132 KiB** against a 20 MiB cap. `runs/` is
git-ignored by design; the tables below carry what they say and the commands above reproduce
them.

**Why the probe introduces at tick 24,000 and not at 6,000.** The world's only source of age is
`Organism::born_tick`, and `WorldState::validate` holds every organism to `born_tick <= tick`.
An age of 1,200 s is 24,000 ticks, so at E's introduction tick of 6,000 it is not representable
— the founder would have had to be born 18,000 ticks before the world began. The door refuses
that rather than silently clamping it. The probe therefore introduces at the earliest tick at
which the requested age is real, **24,000**, and run 3 is the matched control that separates
"the world at tick 24,000 is a different world" from "the founders were older": same tick, same
seeds, same everything, age zero. That control is what makes the probe readable, and it is
recorded below.

## The introduction door's optional age

`World::introduce_hunters_with_age(profile, targets, age_seconds)` places founders as if they
had already lived `age_seconds`. `World::introduce_hunters` **is** that door at age zero, and
now calls it. The age moves exactly one value on the placed body, `born_tick`. Stores, geometry,
heading draws, member records, imports, receipts, the dormancy and encounter policies it enables
— all unchanged. No apex constant moved, the mating radius did not move, and no equation,
snapshot or neural path was touched.

It refuses, before any value changes, an age that is not a finite non-negative number, an age
past this world's `organism.max_age_seconds`, and an age past the world's own age.

Four core tests fix that contract
(`crates/cubarium-core/tests/hunter_introduction_age.rs`), written before the door existed and
run red first:

| test | what it fixes |
| --- | --- |
| `introduction_at_age_zero_is_byte_identical_to_the_current_door` | the same `state_hash` at introduction **and** after 300 further ticks, and receipt-for-receipt equality |
| `an_aged_founder_passes_the_age_term_and_fails_nothing_else_a_young_one_passes` | with stores topped to both fractions, every non-age term of `may_reproduce` reads the same on an aged and an age-zero founder, and only the age term differs |
| `the_age_moves_the_birth_tick_and_no_other_value` | the whole `Organism` compares equal once `born_tick` is rewritten; member records, imports, mass residual and `check_invariants` unchanged |
| `an_impossible_age_is_refused_without_placing_anything` | NaN, infinity, negative, past-lifespan and past-the-world's-age all refused with the state hash and `founders_placed` untouched |

Two independent confirmations came out of the runs themselves. Stage 1 went through the new
code path (`introduce_hunters` → `_with_age(0.0)`) and reproduced **every one of E's eight
published rows exactly** — all sixteen lifetimes, `ticks_two_ready = 0`, zero candidate pairs,
mass residuals ≤ 2.94e-10, final populations 43–71. And stages 2 and 3 differ in **zero fields**
of their entire JSON records outside `founder_age_seconds`, `introduced_age_ticks`,
`age_at_end_ticks`, timings and the verdict sentence: the age changed nothing about the life,
only what the age term reads.

## What the audit now records

Per introduced member, for the life the audit watched: death cause and the whole
`BodyBudget`; the last [2,000 ticks](#the-tail-window) of that budget as a difference of two
readings of the same body; hunt-phase occupancy including dormancy; paid attempts by outcome;
captures with the material and energy they put in the gut; the highest stock fractions ever
reached; and `hunter::may_reproduce` taken apart into its nine terms, counted per tick in the
predicate's own order.

The decomposition is **checked, not trusted**: every sampled tick also calls `may_reproduce`
itself and increments a `mismatch` counter if the conjunction disagrees. Across all three runs
and 605,239 member-ticks, `mismatch = 0`. Nothing below would be readable otherwise.

The ledger is on by default for this command and changes no dynamics; stage 1 reproducing E's
rows exactly is that claim's measurement, not its assertion.

## 1. What kills an introduced adult

Sixteen lives, E's arm, `runs/ecology-v1-apex/death.json`.

| config | seed | # | lived | cause | paid attempts | captures | gut credit m | strikes e | upkeep e | oxidation e | last 2,000: credit e | last 2,000: spent e |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 9001 | 0 | 10,575 | Starvation | 30 | 1 | 0.087 | 2.41 | 3.91 | 3.34 | 0.000 | 1.711 |
| baseline | 9001 | 1 | 13,819 | Starvation | 12 | 0 | 0.000 | 0.96 | 5.11 | 3.20 | 0.000 | 1.084 |
| baseline | 9002 | 0 | 10,888 | Starvation | 24 | 0 | 0.000 | 2.00 | 4.03 | 3.20 | 0.000 | 1.674 |
| baseline | 9002 | 1 | 13,436 | Starvation | 36 | 2 | 0.759 | 2.92 | 4.97 | 4.41 | 0.000 | 1.327 |
| baseline | 9003 | 0 | 11,369 | Starvation | 48 | 1 | 0.470 | 2.98 | 4.21 | 3.95 | 0.151 | 1.792 |
| baseline | 9003 | 1 | 14,101 | Starvation | 51 | 4 | 1.258 | 3.83 | 5.22 | 5.21 | 0.413 | 1.631 |
| baseline | 9004 | 0 | 12,995 | Starvation | 16 | 0 | 0.000 | 1.28 | 4.81 | 3.20 | 0.000 | 0.747 |
| baseline | 9004 | 1 | 12,706 | Starvation | 18 | 0 | 0.000 | 1.44 | 4.70 | 3.20 | 0.000 | 1.559 |
| fast-leaf | 9001 | 0 | 12,859 | Starvation | 47 | 3 | 0.754 | 3.16 | 4.76 | 4.41 | 0.124 | 1.625 |
| fast-leaf | 9001 | 1 | 13,456 | Starvation | 41 | 2 | 0.573 | 2.43 | 4.98 | 4.12 | 0.081 | 1.019 |
| fast-leaf | 9002 | 0 | 10,315 | Starvation | 34 | 0 | 0.000 | 2.24 | 3.82 | 3.20 | 0.000 | 1.654 |
| fast-leaf | 9002 | 1 | 10,685 | Starvation | 30 | 0 | 0.000 | 2.08 | 3.95 | 3.20 | 0.000 | 1.519 |
| fast-leaf | 9003 | 0 | 10,541 | Starvation | 31 | 1 | 0.162 | 2.57 | 3.90 | 3.46 | 0.367 | 1.650 |
| fast-leaf | 9003 | 1 | 10,893 | Starvation | 32 | 1 | 0.128 | 2.33 | 4.03 | 3.40 | 0.000 | 1.580 |
| fast-leaf | 9004 | 0 | 10,534 | Starvation | 27 | 0 | 0.000 | 2.16 | 3.90 | 3.20 | 0.000 | 1.732 |
| fast-leaf | 9004 | 1 | 12,541 | Starvation | 20 | 0 | 0.000 | 1.52 | 4.64 | 3.20 | 0.000 | 1.627 |

Lifetimes are ticks alive after introduction, and are E's eight rows to the tick.

**Death cause: `Starvation`, 16 of 16.** No `Age`, no `Collapse`, no `Predation`, no dormancy
exhaustion (`dormant` occupancy is 0 in every life). `organism.max_age_seconds` is 7,200 s =
144,000 ticks and the longest life reached 14,101, so the lifespan rule was never within a
factor of ten of firing. Injury is zero: `injury_structure = 0` everywhere, and every body
ended at its full adult structure of 2.0 with **both** its reserve and its battery at zero.

**The intake side is the whole story, and it is empty.** `served` is **0.0 in all four field
channels in all sixteen lives** — an apex's genome carries `diet = 0.0` and its profile
`scavenge_fraction = 0.0`, so predation is its only channel, by construction. Predation
delivered **4.19 m of gut reserve credit across all sixteen lives together**: mean 0.26 m per
life, median zero, from 15 captures.

**The arithmetic of the life.** A founder is placed with `S = 2.0`, `R = 2.0` (0.5 of `R_max`
4.0) and `E = 3.0` (0.75 of `E_max` 4.0). Oxidising the whole reserve yields 3.2 e, so the
founder inventory is worth **6.2 e** and no more. Against that:

| line | over 16 lives (e) | share of everything spent |
| --- | --- | --- |
| upkeep — `(maintenance·S + sense_cost·r_sense)·dt` | 70.93 | **65.0 %** |
| strike, retreat and handling (`other_energy_paid`) | 36.31 | **33.3 %** |
| motor — translation and turning together | 1.85 | **1.7 %** |
| total raised and spent | 109.09 | |

Upkeep is a **constant**: the tail window records `upkeep_billed = 0.74000 e` per 2,000 ticks on
every one of sixteen lives, because structure and sense radius never change. That is
3.70e-4 e/tick a body owes before it does anything at all. Everything these bodies actually
spent, upkeep and hunting and motor together, is 109.09 e over 191,713 member-ticks, or
5.690e-4 e/tick. At 1.6 e per unit of oxidised material (2.0 m of founder reserve yielded
3.2 e), break-even intake is **2.31e-4 m/tick to cover upkeep alone** and **3.56e-4 m/tick to
cover the whole bill** — over a mean life of 11,982 ticks, 2.77 m and **4.26 m**. Each life took
in **0.26 m**: **9 %** of its upkeep and **6 %** of its whole bill. At the observed yield of
0.279 m per capture, one capture funds 1,208 ticks of upkeep or 785 ticks of everything, so a
mean life needed about **ten** captures to pay its maintenance and **fifteen** to pay for its
hunting as well. The sixteen lives averaged **0.94**.

**Hunting is the second bill, and it bought nothing.** 497 attempts were made, 449 of them paid
(48 were refused as `Unaffordable` and cost nothing): **402 ended `OutOfReach`** — the jaw was
not in reach of the target after both creatures had moved — 31 `Missed` in contact on a failed
capture roll, 1 `TargetLost`, and **15 captured**. That is a **3.3 % capture rate on paid
attempts, and 89.5 % of them failing on reach**. At `strike_energy_cost = 0.08 e` each, those
paid attempts are the 36.31 e above: a third of every joule these bodies ever held, spent on
lunges that mostly did not arrive. The clearest single case is `fast-leaf/9002` member 0: 28
paid attempts, 0 captures, 2.24 e of strike cost against a 6.2 e inventory — **36 % of its life
spent on hunting that returned zero**. Without those attempts the same body's constant upkeep
alone would have carried it to about 16,700 ticks instead of 10,315.

**Phase occupancy** over 191,713 member-ticks: perched 52.0 %, recovering 26.0 %, stalking
13.2 %, strike 4.5 %, windup 3.1 %, handling 1.2 %, dormant 0 %. A quarter of the life is the
5 s recovery after each attempt.

<a id="the-tail-window"></a>
**The last 2,000 ticks.** Mean credit over the final 100 s of life: 0.060 e of oxidation,
0.011 e from the gut, **0.000 e from any field channel in every one of sixteen lives**. Mean
spend over the same window: 0.765 e of motor bill plus 0.731 e of strikes. In nine of sixteen
lives the tail credit is **exactly zero** — the reserve was already gone, so there was nothing
left to oxidise, and the body was drawing down the last of its battery while still paying for
strikes. It was not a body that was earning and lost; it was a body that never earned.

### The diagnosis, in one paragraph

An introduced apex adult dies of **starvation at the moment its founder inventory runs out**,
and nothing else is close. It takes nothing from any field channel by construction, its only
channel is predation, and predation returns 6 % of what its life costs — 9 % of its maintenance alone: 15 captures
across sixteen lives, 4.19 m in total against the ~4.26 m *each* life needed. Its bill is 65 % a constant
upkeep it cannot reduce (0.74 e per 2,000 ticks, fixed by adult structure and a 12-unit sense
gene) and 33 % strike costs on 449 paid attempts of which 89.5 % never reached the prey; motor
travel is 1.7 % and is not the problem. The deciding rows are `served = [0,0,0,0]` and
`death_cause = Starvation` in all sixteen budgets, `oxidation_reserve_burned = 2.0` with
`end_reserve = end_energy = 0` — the body converted its entire founder reserve into energy and
then stopped — the constant `upkeep_billed = 0.74000` in every tail window, and the attempt
histogram `{captured 15, out_of_reach 402, missed 31, unaffordable 48}`. Neither injury
(`injury_structure = 0`), nor age (14,101 ticks against a 144,000-tick lifespan), nor dormancy
(0 ticks) appears anywhere in the record.

## 2. The eligibility probe

Both adults introduced at age 1,200 s = `reproduce_min_age_seconds`, i.e. already past the gate
on their first tick; `runs/ecology-v1-apex/probe-aged.json`, with
`runs/ecology-v1-apex/probe-control.json` as the age-zero control at the same tick.

| config | seed | 2 adults | 2 mature & perched | **2 ready** | candidate pairs | fail radius | min ready px | first term to refuse (member-ticks) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 9001 | 13,075 | 7,179 | **0** | 0 | 0 | — | reserve 22,768, target 3,279, carrying 519 |
| baseline | 9002 | 13,137 | 6,425 | **0** | 1,180 | 1,180 | — | reserve 22,433, target 4,986, carrying 709 |
| baseline | 9003 | 10,530 | 5,362 | **0** | 0 | 0 | — | reserve 26,239, target 4,114, carrying 560 |
| baseline | 9004 | 10,346 | 5,246 | **0** | 0 | 0 | — | reserve 18,711, target 5,083, carrying 524 |
| fast-leaf | 9001 | 11,535 | 5,250 | **0** | 0 | 0 | — | reserve 19,822, target 5,383, carrying 575 |
| fast-leaf | 9002 | 10,167 | 5,245 | **0** | 0 | 0 | — | reserve 18,391, target 4,758, carrying 557 |
| fast-leaf | 9003 | 10,253 | 5,244 | **0** | 0 | 0 | — | reserve 17,220, target 4,637, carrying 260 |
| fast-leaf | 9004 | 12,423 | 5,246 | **0** | 0 | 0 | — | reserve 19,562, target 5,048, carrying 625 |

Totals over 206,763 member-ticks: **simultaneously-ready ticks 0**, `max_ready` **0**, candidate
pairs **1,180**, minimum ready-pair distance **undefined — no two members were ever ready at
once, so no distance was ever consulted**, matings **0**, and therefore gestations, births and
emergences **0** (`max_members_alive` is 2 in every run: no offspring was ever created). Ages at
death ran 34,167–44,383 ticks against the 24,000 `may_reproduce` requires: **every member was
past the age gate for its entire life**, and 16 of 16 still died of `Starvation`.

**The branch this reaches is Astra's first: readiness still never opens.** And the term
decomposition names what closed it:

| first term to refuse | member-ticks | share |
| --- | --- | --- |
| reserve below `0.8 · R_max` | 165,146 | **79.9 %** |
| holding a hunting target | 37,288 | 18.0 % |
| carrying a carcass | 4,329 | 2.1 % |
| energy below `0.75 · E_max` | 0 | 0 % |
| **below the minimum reproduction age** | **0** | **0 %** |
| ready | 0 | 0 % |

`fail_age = 0` is not a rounding: the age term sits *after* the reserve term in
`may_reproduce`'s conjunction, and the reserve term refused first on every tick it could have
mattered. The reason is the flattest fact in the whole workstream: **`max_reserve_fraction` is
0.500000 in every life of all three runs.** An introduced apex never, on any tick, held more
reserve than the 0.5 of `R_max` it was founded with. The 0.8 the profile demands was not merely
unmet — it was never approached from below, because the reserve only ever falls. The same is
true of the battery: `max_energy_fraction` is 0.750000 everywhere, exactly the founder fraction,
so the energy term passed only for as long as the founder's own charge lasted.

**The control says the age moved nothing but the age.** Run 3 is run 2 with `age_seconds = 0`
and is otherwise identical. Its records differ from run 2's in **zero fields** outside
`founder_age_seconds`, `introduced_age_ticks`, `age_at_end_ticks`, timings and the verdict
sentence: the same lifetimes to the tick, the same 23 captures, the same 1,180 candidate pairs,
the same mass residuals. So every difference between this probe and E's arm is the introduction
tick, and every difference the *age* made is confined to the predicate term it was meant to open.

**One correction to E's reading, which its own evidence could not have caught.** E recorded zero
candidate pairs in every run and concluded the two adults never sensed each other. At an
introduction tick of 24,000 they do: `baseline/9002` formed **1,180** candidate pairs, in both
the aged arm and its age-zero control. A candidate pair requires mutual sensing, so those two
adults did find each other; they were refused on distance, which the pass evaluates *before*
readiness, so `fail_radius = 1,180` is an artefact of evaluation order and **not** evidence
about the radius. The deciding number remains the census one, `ticks_two_ready = 0`, which the
pair pass does not touch. Meeting is therefore not impossible in this world — it is simply moot
while nobody is ever ready.

## What this does not establish

- **One intervention, four seeds, two configurations.** Only the age at introduction was moved.
  No apex constant, no stock fraction, no mating radius, no ecology parameter changed.
- **Nothing about whether a *fed* apex could mate.** The probe opened the age gate and found the
  reserve gate behind it. It did not open the reserve gate, and says nothing about what happens
  when one is opened — whether two ready adults would then meet, or what distance they would
  stand at. `min_ready_distance_px` is still undefined, in every run ever recorded.
- **Nothing about the radius.** It has still never been the deciding term, in any run of E's
  audit or this one. It also has not been exonerated: a world with ready adults has never
  existed to test it in.
- **Nothing about a different introduction tick or a longer horizon.** The probe had to
  introduce at 24,000 to make the age representable, and the control shows that tick, not the
  age, is what produced the candidate pairs. A third tick could produce different sensing.
- **Nothing about why 89.5 % of paid attempts end `OutOfReach`.** The ledger localises the
  failure to intake, and the attempt histogram localises intake to reach rather than to the
  capture roll or to prey eligibility. It does not say whether the strike geometry, the pursuit
  controller, the prey's escape speed, or the 12-unit sense radius is responsible.
- **Nothing about the grazer population**, which was healthy throughout: 38–71 organisms alive
  at every horizon, mass residuals ≤ 4.34e-10, no run collapsed.
- **The display world's apex profile is unchanged by this work.** `FixedHunterProfile::
  lanternjaw_trial` holds exactly the constants it held at `2a1cedd`. The age door is a new
  optional argument used by one diagnostic command; the viewer's apex spawn control does not
  pass it and still introduces adults at age zero, as it always has.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## The next task this implies

Named, not launched.

**Diagnose the apex's reach, not its eligibility.** The measured obstacle has moved twice now:
from the mating radius to the age gate (E), and from the age gate to the reserve stock gate and
the intake that would have to fill it (here). The reserve gate is not a constant worth arguing
about while intake is 6 % of break-even; lowering `reproduce_reserve_fraction` from 0.8 would
put the gate below a level the body still never reaches, because its reserve only falls. The
cheapest informative measurement is the one the attempt histogram already points at: with the
ledger on, record for each paid attempt the hunter–prey separation at windup, at the start of
the strike and at settlement, against `|capture_offset| + capture_reach` and the prey's realised
escape speed. Three outcomes separate cleanly — the strike never closes the gap it was aimed at;
it closes it and the prey outruns it; or the grasp lands outside the reach the geometry
advertises. That decides whether the next move is the strike kinematics, the escape multiple, or
the pursuit controller, and it is a diagnostic rather than another parameter search. It is the
apex-side twin of the grazer intake diagnostic (workstream H) and should be read beside it.

## Verification

`cargo test -p cubarium-core` **480 passed / 0 failed / 3 ignored** (E's 476 plus this
workstream's 4); `cargo test -p cubarium-search` **119 passed / 0 failed / 0 ignored** (including
this workstream's 3 new unit tests in `apex_audit`). `graft build` rebuilt the graph
(6,798 nodes, 14,102 edges).

The shared `target/` is used by several concurrent workers, and the stale-artifact race E
documented recurred twice in this session: `cargo test -p cubarium-core` failed with
`no method named introduce_hunters_with_age` immediately after the same command had passed,
pointing at another worktree's copy of the same file. `touch crates/cubarium-core/src/lib.rs`
and rebuild clears it. The three experiment runs were made from a release binary built once,
clean, at `ec06d68` and copied out of the shared target directory, so no concurrent rebuild
could have changed the binary under a running experiment; its stamp `ec06d6859be3` appears in
all three artifacts.
