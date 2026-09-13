---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Next hunter experiment: paid battery charging, before geometry

Recommendation: test **one hunter-only oxidation activation threshold, 0.80
Emax**, against the unchanged 0.50 reference, both on the completed
`reserve-targets-v1` background (seek 0.80 / perch 0.90). This is a physiological
policy experiment, not a free battery, reproduction discount, or accepted balance.
No implementation, schema change, new cohort, or live action accompanies this note.

This revises the *next-step priority* in the [complete reserve-results report](astra-hunter-reserve-complete-results-2026-09-13.md),
not its evidence. Retain the signed strike-admission proposal as a separate later
family; do not combine it with charging.

## Why this is the more direct next intervention

In the complete twelve-seed candidate, mature reserve-ready member-ticks were
52200 specialist and 66402 facultative, while mature battery-ready and joint-ready
member-ticks were zero. Every founder eventually starved; there was no funding
or birth. The observed obstacle is therefore more specific than unsuccessful
strikes alone. Raising the controller's storage target directly tests whether
paid reserve conversion can open the missing battery gate.

Actual source at `fbdcbd6` retains the relevant physiology of frozen experiment
`b547ad0`: `world.rs:995` onward pays movement/maintenance/sensing; digestion
precedes physiology; `world.rs:1404` oxidizes only below the global 0.50 Emax;
growth then runs before the actual reproduction predicate/payment.
`hunter.rs:1475` requires adult size, age at least 1200 seconds, R at least 0.80
Rmax, E at least 0.75 Emax, no escrow/meal/target/hunting phase, and the local
interval. Read the source at those commits, not a future moving line number.

The 0.50 controller **cannot recharge to 0.75 by oxidation alone** (the tiny
one-tick overshoot is nowhere close). This is not universal biological
impossibility: digestion can directly charge above 0.75, and candidate founders
did exceed that fraction before minimum reproductive age. Earlier/global stock
maxima are not mature simultaneous readiness evidence. Likewise, post-step
opportunity counters are not the funding transaction: successful funding itself
reduces stocks before those counters observe them.

## Exact single-family policy

For authoritative hunter members carrying the explicit candidate policy, replace
only the oxidation activation threshold with 0.80 Emax. Apply the same fixed
threshold at all ages/phases, including descendants; no age switch, reserve floor,
hysteresis, reproduction-conditioned recharge, or phase exception in this family.
Ordinary organisms and baseline hunters retain the configured threshold exactly.

Keep the existing paid conversion block unchanged:

```text
burn = min(oxidation_rate * DT, actual reserve)
reserve -= burn; local nutrient += burn
released = reserve_energy_density * burn
battery_gain = min(released * oxidation_efficiency, battery headroom)
heat += released - battery_gain
```

No changes to oxidation rate/efficiency, intake, gut capacity/digestion/handling,
maintenance/sensing/movement/strike payment, body/geometry/steering, reserve
targets, founder stocks, growth, offspring payment, gates, gestation, mortality,
care, RNG or capacity. The 0.80 target places a 0.20-energy margin above this
trial's 3.0 battery gate; it is not a guarantee against future expenditure.
Existing one-tick threshold overshoot remains bounded by the existing conversion
step and Emax cap, not corrected by adding a new drain or energy gift.

## Feasibility: enough currency is not enough charging time

For this exact adult founder, S=2, Rmax=4, Emax=4. Defaults give e_r=2,
oxidation efficiency 0.8, and burn ceiling 0.01 material/second. Thus maximum
usable charging is **0.016 energy/second** (0.0008 per 20 Hz tick), with
0.004 energy/second conversion heat and 0.01 material/second returned to N.

Approximate fully paid dry adult rates, using the actual genome/config and no
other intake or expenditure:

| State | Upkeep/motion E/s | Maximum net charging E/s |
| --- | ---: | ---: |
| Quiet rest-effort movement | 0.00755136 | 0.00844864 |
| Full ordinary pursuit | 0.01042723 | 0.00557277 |
| 1 px/s strike movement | 0.01940000 | -0.00340000 |
| Quiet movement plus handling | 0.00955136 | 0.00644864 |

These include 0.005 maintenance and 0.0024 sensing E/s; dry ordinary speed is
approximately 0.252269 px/s. A strike additionally costs 0.08 at paid entry.
Handling adds 0.002 E/s, but real digestion also supplies resources, omitted
from the table. Wading, actual motion and juvenile growth change these rates.

At the gate R=3.2, E=3.0, the exact offspring recipe is escrow S=0.8, R=0.8,
E=0.6 plus 0.4 construction heat. Funding pays **1.6 reserve and 1.0 battery**,
leaving parent R=1.6, E=2.0. This is affordable: parent chemical inventory 9.4
becomes parent 5.2 + escrow 3.8 + heat 0.4. Escrow structure retains its booked
chemical energy until birth releases it; it is not ordinary adult structure.
Thus the fixed gate already covers the full payment, without lowering anything.

But reaching that gate is costly. From R=4, E=2, ignoring upkeep would require
only 1/1.6=0.625 reserve to reach E=3, leaving R=3.375: apparently enough.
With quiet upkeep and maximum oxidation, the same charge takes approximately
118.36 seconds and burns **1.18362 reserve**, leaving R=2.81638: below the
3.2 reserve gate. Even perfectly still motion would not remove this deficit,
because maintenance+sensing remain. For the dry quiet case, starting full R=4
requires E at least about **2.32411** to reach E=3 before R drops below 3.2,
absent new intake. These are rate estimates, not a simulated trajectory or a
claim that observed separate R/E maxima occurred together.

Joint readiness therefore needs suitable starting stocks and/or intake while
charging; raising the target alone need not yield births. As a second check,
a gate-funded parent at R=1.6/E=2 could pay 120 seconds of quiet gestation with
continuous oxidation, ending near R=0.4/E=3.014 before any further transactions.
That conditional calculation shows no inevitable gestation-energy impossibility;
real hunting, handling, intake and mortality must be measured, not assumed quiet.

## Honest minimal persistence and recipe representation

**Use an explicit semantic profile version 4 for this one fixed policy**, retaining
the existing shape, if the following compatibility tests pass. Version 3 remains
the default constructor and retains config-driven oxidation. New validation
explicitly supports versions 3 and 4, rejecting every other version; a named
method/accessor maps v3 to the existing config threshold and v4 to fixed 0.80.
Do not simply bump the default `PROFILE_VERSION`, reinterpret v3, or silently
convert saved baseline trials. Geometry and every other v4 meaning remain v3's.

Source supports this approach: current `hunter.rs:336` rejects any version other
than 3; `HunterState::validate` calls it, `WorldState::validate` validates hunters,
and `snapshot::decode_snapshot` validates the decoded state. Initialization and
`World::from_state` also validate. An old reader therefore rejects same-shaped v4
at the semantic check rather than resuming it as v3. This is source evidence;
an actual frozen old-reader rejection fixture is still an implementation gate.
Current snapshot schema 12 concerns care-dose shape, not this fixed policy.
No new shape means no additional snapshot schema/mirror is intrinsically needed.

Art `hunter_present::validate_profile` checks actual role, effectors and scale,
not a version shortcut. Unchanged v4 geometry should pass that capability check
*after* core validation; retain unknown-policy rejection in core. Prove both
versions render the same supplied state/pose rather than changing the silhouette.

Give the experiment a separate recipe label, e.g. `reserve-targets-charge80-v1`.
Its profile diff from `reserve-targets-v1` is **only version 3 to 4**; the manifest
must also spell out the resolved activation policy/threshold so a number is not
mistaken for a geometry update. The existing reduction script correctly accepts
only the earlier reserve-target experiment: add a narrowly tested new comparison
contract, not a permissive ignore-list. Defaults, controls/import receipts, all
other profile/config fields and original input snapshots must remain verifiable.
Use full state hashes for restart/policy identity, not the legacy ecology hash
that deliberately projects away the hunter extension.

This version scheme is suitable for one frozen experimental constant, not an
extensible matrix of tunable policies. If adjustable thresholds or orthogonal
policies are introduced later, an explicit persisted field/tag and honest frozen
old-shape migration are preferable to endlessly encoding combinations in versions.

## Bounded implementation and evaluation gates

First prove paid activation in (0.50,0.80), no activation at/above 0.80, original
step/rate/headroom bounds, exact reserve-to-N and conversion-heat accounting,
zero-reserve behavior, membership isolation, and unchanged offspring payment.
Use a real mature local fixture to show charging can enable a fully paid escrow
when stocks permit; also retain a stock-limited fixture that does not magically
become ready. Save/reload mid-charge and mid-gestation must match uninterrupted
full state/event streams. Prove frozen v3 continuation/default identity, old-reader
v4 rejection, unsupported-version rejection, and both art capabilities.

Then rerun reference and candidate in the same frozen executable, same twelve
opening worlds and all six arms. Off hunters also receive their recipe's charging
policy: they expose its starvation/intake effects without attacks. Only untouched
and non-member budget ecology should remain exactly identical; do not demand
unchanged off-hunter trajectories. Record actual additional oxidation above 0.50,
burned reserve, gained E and heat as bounded observer diagnostics without changing
world RNG/state solely for measurement. Keep per-step mature R/E/joint opportunity,
exact funded/born/refunded/miscarried streams, survival/descendants, paid strikes,
prey/form burden and paired local recovery. Preserve all failures/censoring.

Primary mechanism test: does paid charging open mature battery and joint gates,
and can real eligible parents fund/complete offspring? More battery alone is not
success if reserve readiness collapses or all founders still starve. Charging
may also free reserve headroom for digestion, consume reserves needed for juvenile
growth, trigger earlier hunting through existing reserve thresholds, and increase
prey burden. Those are consequences to measure, not covert additional tuning.

Signed admission remains valuable for reducing avoidable paid failures, but its
settlement near/far evidence does not establish the admission counterfactual, and
cost savings need not overcome the controller's 0.50 replenishment target. Paid
charging directly intervenes on the observed mature battery barrier and has a
clear conservative failure interpretation: if joint readiness still fails, the
coupled intake/conversion/upkeep budget remains unresolved. That is why this
family should precede the geometry cohort, without promising lineage viability.
