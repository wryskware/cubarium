---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Hunter stock bottleneck: diagnosis and one next experiment family

**Recommendation:** next test only the hunter's **acquisition reserve targets**:
candidate `seek_reserve_fraction = 0.80`, `perch_reserve_fraction = 0.90`, versus
the recorded 0.35/0.65 reference. Keep the reproductive gate at 0.80 reserve /
0.75 usable energy, and keep every resource import, paid cost, digestion rate,
recovery interval, capture rule, placement and seed unchanged. These are proposed
experiment values, not validated biology, a deployment recommendation, or canon.

This is a bounded read-only diagnosis. Canon and Lore were consulted; no profile,
core, harness, live process, or input snapshot was modified, and no simulation
was advanced. Root owns any implementation and execution after observer hardening.

## Complete-cohort evidence, not a selected survivor

Read the actual JSON/event schema under
`captures/hunter-profile3-two-hour-2026-09-13`, the complete report `c9c53b4`,
and the seed-6 opportunity replay report. Independently reducing all 72 per-arm
summaries found all technically complete, all numerical audits passing, all
closing at tick 288000. Reducing the four living-treatment event streams across
all twelve seeds found **48 hunter deaths, all Starvation, and zero Offspring**.
Those checks verify reported artifacts, not a rerun of the simulations.

The frozen baseline is core `9eacb7e`, profile 3. Its observer predates exact
funding events: no offspring is established across the cohort, but absence of
same-tick-funded-and-lost escrows is not established by that older observer.
Root's later seed-6 facultative replay separately matched closing hash
`8365334319777227972` and observed zero reproduction records. That one replay's
stock maxima are useful corroboration, not substituted for twelve-seed telemetry.

New reductions of all capture/attempt/death streams:

| Quantity | Specialist on | Facultative on |
| --- | ---: | ---: |
| Paid settled attempts | 854 | 1313 |
| Captures | 179 | 300 |
| Missed rolls after valid contact | 210 | 339 |
| OutOfReach | 463 | 673 |
| OutOfReach with prey nearer root than claw center | 430 | 633 |
| OutOfReach with prey farther than claw center | 33 | 40 |
| Unaffordable, unpaid refusals | 59 | 22 |
| Actual strike payments, energy units | 68.32 | 105.04 |
| Captured inventory, material units | 173.539620 | 287.839848 |
| Captured inventory, energy units | 217.650781 | 348.388811 |
| Mean founder lifetime, minutes | 25.392 | 37.861 |
| Captures per pooled living-hunter hour | 35.248 | 39.619 |

There were additionally one unmapped grasp in each variant and one lost target
in specialist-on. Valid-contact capture fractions were 46.0% and 46.9%, whereas
only 21.0% and 22.8% of all paid attempts captured. Neither misses nor the full
OutOfReach population should be described simply as escaped prey.

## The reserve target is structurally misaligned with the observed meal sizes

`World::step` starts a new hunt from Perched only when reserve is **below 0.35 of
4 = 1.4**. A hunter cannot acquire another carcass while carrying one. Capture
enters Handling; finishing the meal schedules 20 seconds of recovery; the next
acquisition again waits for reserve below 1.4. A specialist gets no field food.
Its adult reserve cannot rise during an empty-gut hunt; only the captured meal
then replenishes it, with oxidation continuously competing for that reserve.

From each captured prey's actual M/Q, the most material it could add as reserve
is `A = 0.6 * min(M, Q / 2)`, before headroom and ongoing consumption. Across
the complete cohort:

| Meal reserve contribution | Specialist | Facultative |
| --- | ---: | ---: |
| Mean upper bound per captured meal | 0.357327 | 0.342026 |
| Largest upper bound of any captured meal | 0.717556 | 0.722958 |

Consequently a specialist acquiring the **observed** meals from below 1.4 cannot
reach even 2.118 reserve after one meal before costs, let alone the **3.2**
reproduction threshold. Its opening reserve is 2.0, also below that gate. This is
a conditional bound from actual meal inventories and the existing state machine,
not a universal bound on every eligible prey a future trajectory could encounter.
Larger/richer prey might change it; importing one deliberately is not proposed.

The 0.65 perch threshold is **not a hard clamp on reserve**. It terminates an
already-active hunt when the parent is full, but digestion can go above it.
For the current adult specialist, there is no reserve intake during the active
hunt anyway. The more decisive barrier is repeatedly refusing a new acquisition
until reserve has fallen below 0.35. Facultative scavenging can in principle bridge
that interval, so the specialist bound is not a proof for the facultative arm.
Root's highest-capture example nevertheless made 96 captures and never exceeded
its opening reserve fraction 0.5.

## The energy economy is independently tight

From the saved configuration and decoded founder genome: adult S=2, Rmax=4,
Emax=4; maintenance per unit structure is 0.0025; sense radius is 12; ordinary
dry speed is approximately 0.252269 px/s. `World::step` charges each tick:

`(0.0025 * 2 + 0.006 * 2 * speed + 0.0002 * 12) * dt`.

Thus the nominal basal demand is **0.0074 energy/s = 26.64/hour**, of which
sensing is 8.64/hour. Dry resting effort 0.05 raises it to about 0.007551/s;
ordinary pursuit to 0.010427/s; the 1 px/s burst to 0.0194/s, plus the separate
0.08 paid strike entry. Wading reduces speed, and exhausted batteries clip actual
payment: these are source-derived demands, not measured per-hunter heat totals.
“Perched” still has small forward rest effort, not exactly zero translation.

The initial battery is 3 and reserve can yield at most `2 * 2 * 0.8 = 3.2`
additional usable energy by oxidation. Without food, 6.2 usable energy against
the resting demand predicts about 13.7 minutes. The complete attack-off means
were 13.689 minutes specialist and 14.115 minutes facultative, consistent with
this being a genuine paid survival budget, not hidden age death.

Of 479 captured prey, **412 carried less than or equal to 2 energy/material**,
so bulk carcass material does not all qualify for the nominal 60% assimilation.
Mean captured Q was only 1.216/1.161 per specialist/facultative capture. Applying
the existing digestion formula to every event gives these optimistic totals:

| Captured-food potential, before headroom/ongoing costs | Specialist | Facultative |
| --- | ---: | ---: |
| Reserve material A | 63.961501 | 102.607821 |
| Direct battery energy `0.5*(Q-2*A)` | 44.863890 | 71.586585 |
| Usable energy including later 80%-efficient oxidation, `1.6*A + battery` | 147.202291 | 235.759098 |

Strike payments alone exceed direct battery returns in both variants. Mean
strike payment per capture was 0.382/0.350 versus direct meal battery potential
0.251/0.239, before maintenance, sensing, movement or handling. Oxidizing reserve
therefore helps finance hunting instead of building parental stocks. Captured
food is not the facultative's entire intake; these calculations deliberately do
not invent a measured scavenging total.

Digestion processes at most 0.1 material/s and charges 0.002 energy/s while
carrying, before ordinary physiology. It may pause if handling is unaffordable
or reserve is full. The mean captured bodies require roughly 9.7/9.6 seconds
of processing at that maximum rate, followed by the 20-second quiet interval.
All 24 hunting founders died with empty guts; no retained terminal carcass
explains their final energy loss. Faster digestion is not the sole obvious fix.
Oxidation itself only activates below 0.5 Emax, so holding reserve does not
automatically charge the battery to the separate 0.75 Emax reproductive gate.

## Separate geometry inefficiency: preserve it as a measured limitation

**1063/1136 OutOfReach settlements (93.6%) are on the too-near side.** Maximum
absolute lateral body-coordinate displacement in those events was only 0.023 px;
none failed because lateral error exceeded the contact tolerance. This matches
the controller pointing directly toward the prey before forward motion.

There is a specific mechanism worth a later separate experiment: windup admission
uses symmetric `effector_distance <= tolerance + strike_speed * strike_seconds`.
That extra distance is not reachable on both sides by a forward-only root motion.
A prey already inward of the claw can be admitted, yet advancing the hunter makes
the gap worse. The forward stopping test is even farther inward,
`body.x < claw.x - tolerance - closing`, and no retreat/reposition is supplied.
Nearest eligible prey selection also need not prefer a prey in a reachable grasp
annulus; initially-too-close targets can occupy a stalk and time it out.

This is more than a generic “low capture chance” hypothesis. Nevertheless a
settlement record does not tell us its admission pose or prove each individual
near miss was caused by overshoot rather than prey motion. Only 435 of the 1063
near-side failures were within one additional pixel of valid contact at settlement.
Do not claim that reducing the burst by one pixel would recover all of them.
Neighbor sensing uses radius plus both body extents and truncates to 16 nearest
neighbors; merely enlarging the already-paid sense gene is not a demonstrated fix.

Do **not** bundle steering, contact radius, retreat, burst speed, capture chance,
or sense-cost changes into the proposed reserve-target experiment. It remains a
valid diagnostic of acquisition policy under current steering, not a clean test
of an already-efficient predator. If increased acquisitions mostly create more
near-side failures, that is evidence to address this separate family next, not
to waive funding costs or conclude that parental stock targets cannot work.

## One preregistered next family

Candidate numbers: **seek 0.80, perch 0.90**. The required strict ordering remains
valid. Raising seek to the unchanged reproductive reserve threshold permits
successive paid meals toward R=3.2 instead of deliberately waiting below R=1.4
again. Once reserve is sufficient, acquisition stops until it falls below target;
the higher perch value preserves the ordered hysteresis without clamping digestion.

Expected effects, explicitly hypotheses:

- Earlier acquisition is possible: the unchanged founder starts at 0.50 reserve,
  below the candidate's seek target. No food, energy or founder inventory is added.
- More acquisition opportunities at intermediate/high reserve may allow parental
  reserve to accumulate across meals. Mature reserve-gate opportunities should
  rise if acquisition pays for itself.
- Paid attempt counts and prey pressure may rise, and survival could **fall** if
  the same inefficient geometry spends energy faster. The energy gate can remain
  closed even if the reserve gate opens. Offspring are not promised.
- Facultative field scavenging can fall when earlier hunting occupies time that
  was previously target-free; the field intake gate is disabled during a hunt.
  Therefore more captures do not necessarily mean more total acquired energy.
- Funding remains exactly paid: child material 1.6 and usable funding debit 1.0
  at the default fractions/build cost; minimum age 1200 seconds, E gate 3.0,
  birth heat, gestation, reproduction interval, no-carried-gut condition, all
  movement/sensing/strike bills, and meal recovery stay unchanged.

After observer hardening, record an immutable candidate recipe and run the full
prescribed **12 seeds × 6 arms**, same mature inputs, controls, placements, care-off
schedule and two-hour horizon. Keep all failures/extinctions. Compare with the
complete reference cohort, using matching reference replays under the hardened
observer wherever newly required stock/funding metrics were absent; never credit
the old run with new exact measurements. No seed-specific rescue or winner choice.

Primary diagnostic outputs: actual Funded/Born/Refunded/Miscarried counts; mature
member-ticks meeting R and E gates separately and together; reserve/energy maxima
including explicit zero-member cases; paid energy/capture and captures per living
hour; near/far OutOfReach counts; founder/lineage survival and descendants reaching
adult size. Retain the unchanged numerical audits, prey integrals, per-form loss
and paired recovery windows. The reference already shows form-specific damage
despite stable pooled prey totals; more captures alone are not success.

The candidate supports the hypothesis only if stock opportunity improves without
simply exhausting hunters or prey faster. No births means the lineage still
fails the self-replacement objective even if reserve rises. Any later geometry
revision or energy-budget family needs its own separate recipe and full paired
comparison, not an unreported adjustment inside this experiment.
