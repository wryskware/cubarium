---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# Fable review of the recurrent-organism plan — 2026-09-14

Review of [recurrent-organism-plan.md](../recurrent-organism-plan.md) and
[movement-and-foraging-plan.md](../movement-and-foraging-plan.md) against the R0a
implementation (`7a1669e`, `e9a3dd4`) and the two R0a reports. Evidence, not a decision;
nothing here promotes canon. Sources are labelled: **owner** (Wrysk's stated intent),
**source** (current tree), **measured** (R0a reports), **inference** (my arithmetic on
measured constants), **recommendation**.

## Verdict

**Ready with named revisions.** The plan's direction, ownership split (world owns
physics and physiology, policy owns effort), private per-animal memory, whole-policy
inheritance, and bounded milestone sequence are sound. Two findings must be resolved
before R1 starts, one of them a redesign of the shipped envelope, not a tuning:

1. The R0a envelope is not a shared movement budget. It lets a unit adult sweep its rim
   at 13× its top travel speed, so the owner's "no spinning faster than you can move" is
   not implemented for the reference body, and the whole rotation-cost debate is a symptom.
2. The R0a food fixture confirms stationary feeding starves, but the decisive quantity for
   training, the yield of a *moving* grazer, was not measured. It can be estimated from the
   measured constants, and the estimate says mobility is a ~10× intake gain. One scripted
   probe should measure it before the training horizon and fixture are frozen.

Deployment status: no Cubarium runner process was found at review time. The newest state
file is `state/world-115200.cubw`, the snapshot the rotation-cost report resumed. R0a is
merged on `main`, but I could not verify it is running on the cube.

## Findings, prioritized

### F1 — The envelope is a union of two ceilings, not a shared budget (blocker)

**Source.** `motor.rs:137-141`: `capability = speed_cap + REFERENCE_RADIUS_PX · turn_rate_max`,
with `speed_cap = effort · speed_max / wading` (`world/step.rs:1063-1070`), `speed_max` 0.3 px/s
and `turn_rate_max_deg` 90 (`config.rs:436,481`). For a unit adult (r = 2.5 px) the
available magnitude is 0.3 + 3.93 = 4.2 px/s. Only 0.3 of that can ever be translation
because `speed_req` is separately clamped to `speed_cap`; the remaining 3.9 px/s is a
rotation allowance that exists whatever the effort. A Resting body at effort 0.05 has
0.015 px/s of travel and 3.9 px/s of rim sweep. A threatened prey (escape ceiling 240°/s,
`profile.rs:257`) may sweep 10.5 px/s, two body lengths per second.

**Consequence.** The plan's §2 bound `|v| + r|ω| ≤ u_available` was implemented with
`u_available` defined so that the bound never binds below 2.5 px. The rotation-cost
report's own numbers confirm it: with kinematics only (`k = 0`) the population is 93
versus 96 pre-R0a, because nothing changed for ordinary bodies. The 13× sweep-to-travel
ratio it calls a pricing problem is the envelope problem. An RNN trained under this
envelope is free to spin because spinning is physically cheap in capability terms and only
expensive by the `k` price, which is exactly the lever the report asks Wrysk to lower.

**Correction.** Define `u = speed_cap` (effort, wading and burst already applied) and bound
`|v| + r|ω| ≤ u`, with the genome's `turn_rate_max` retained only as an upper clamp. Wading
then slows turning as well as travel, and effort throttles both, so rest is still by
construction. Consequences at the current pace: unit adult full-effort pivot 6.9°/s (half
turn in 26 s), resting 0.3°/s, escaping prey 14°/s, apex (v 0.25 px/s, claw radius 14.8 px)
1°/s. Rotation at full rate then costs at most `move_cost · speed_max` = 0.0018 energy/s
against 0.0062/s of upkeep, so `ROTATION_COST_SCALE` stops being an ecological lever; keep
0.5 as shipped. The rod-mean argument for 0.5 is coherent for the linear-in-speed cost the
bill uses, but the report's ecological claims (a fifth off the population, "not decline")
rest on one saved world, one legacy controller that turns constantly, and one 2-hour run;
they describe the legacy controller's turning habit under an inflated envelope, not the
price of honest turning.

The apex cannot hunt by pivoting under any honest contract at the current pace. Its
8-second stalk timeout assumes 16°/s or better. Treat the legacy hunter as the integration
reference the plan already says it is: raise `stalk_timeout_seconds` for the hunter suite,
keep capture timing tests, and let R3 learn approach-based alignment. Do not restore the
allowance to save the heuristic.

### F2 — Mobile grazing is the unmeasured quantity that sets the training fixture (prerequisite)

**Measured.** One immobile unit adult on one cell took 8.3e-4 m/s and starved at 454 s; the
patch pinned at P ≈ 0.198 under the 0.2 gate; recovery reached 64% of undisturbed after
300 s and was still rising.

**Inference** (constants from `config.rs`, `step.rs:1358-1400`, `fields.rs:128-134`). One
unit of P yields ≈1.36 energy (0.6 to reserve at density 2.0, oxidized at 0.8, plus 0.5 of
the 0.8 spare). Upkeep is 0.005 + 0.0002·6 = 0.0062 energy/s, so a unit adult needs
≈0.0046 m/s of P. The stationary drip is 18% of that: starvation is arithmetic. A fresh
cell holds ≈0.54 and can be cropped to 0.2 in ~25 s (0.34 m ≈ 0.46 energy); a 4 px hop at
0.3 px/s takes 13 s and costs 0.023. The cycle nets ≈+0.005 energy/s versus −0.005/s
sitting still. Mobility is roughly a 10× intake gain, and a grazer needs a fresh cell about
every 40 s. Regrowth is logistic in P (`fields.rs:131`), so a pinned patch regrows at about
half the undisturbed rate and full recovery takes 10–15 min; a patch cropped to zero would
take far longer. These are estimates from one cell whose light level the report does not
state; they are not world averages.

**Consequence.** The diagnosis holds: stationary feeding starves and recovery outlasts a
visit. The producer model, 4 px cells and own-cell reach can stay; a cell is one body
length, which is honest enough for mouth reach. What must change is not regrowth but
ownership: the leave rule is currently the controller's `feed_min` gate
(`controller.rs:185-188`), which the RNN world removes. With it gone the world-side type-II
term still lets a policy hold graze effort at 1 and pin a patch near zero, where logistic
regrowth is slowest.

**Correction.** (a) Run one scripted R0 probe, permitted by the plan's "simple scripted
probes of physical capability": a unit adult that hops to the neighbouring cell when own-cell
P falls below a threshold, on the same stripped fixture with a 3×3 patch. Report net energy
per cycle, cells visited per minute and recovery of the abandoned cells. (b) Add a
world-owned cropping floor: a mouth cannot take a cell below a small residual stock. This is a
physical rule (stubble), not a behaviour, and it protects regrowth from a policy that has no
reason to stop chewing. (c) Feeding effort is currently free in the bill
(`motor.rs:377-382`); either price handling at a small rate or make the cropping floor
carry the "unprofitable patch" signal. Prefer the floor; it is one constant.

### F3 — The 600 s episode barely outlasts no-intake survival (tuning, must fix before R2)

**Inference.** The fixture consumer starts with 1.5 energy and 0.5 reserve (≈2.5 usable),
which covers ≈400 s of upkeep with no intake; the measured 454 s death with a trickle
agrees. In a 600 s episode a policy survives by eating ≈0.9 m of P, about two and a half
fresh cells, so sitting on the start patch and taking one hop nearly passes. The plan's own
rule says to repair this at the checkpoint. **Correction.** Use 1,800 s (36,000 ticks) and
halve the starting stores. At the M1 harness's measured 145 k ticks/s aggregate, the 2,048
episode learning screen becomes ≈74 M ticks, about 9 minutes before neural overhead; the
20-minute cap holds. Compute is not the constraint here; sample count is.

### F4 — The optimizer: plumbing screen fine, learning screen should use a smooth estimator (tuning)

The plan's summaries of Ni et al., Cho et al., Such et al., Salimans et al., Lehman et al.,
Heess et al., Mouret & Clune and Tallec & Ollivier match the papers as I know them; I did
not re-fetch them because none would change the recommendation. What matters from Such et
al. is scale: their GA used populations of a thousand or more and on the order of a billion
frames. The proposed learning screen evaluates 512 policies once each. A 32-slot elitist GA
with mutation only is a weak optimizer at that budget for a 9.7 k-parameter recurrent policy.
**Correction.** Keep the GA exactly as specified for the plumbing smoke. For the first
learning screen use antithetic evolution strategies over the same episodic evaluation and the
same population size; it is a mutation-and-selection method in the plan's sense, needs no new
runtime, and estimates a gradient from the same 32 evaluations instead of discarding 31 of
them. Recurrent PPO stays the named fallback with its own dependency and equivalence checks.
The safe-mutation divergence measurement is cheap; keep it as measurement, do not rescale
until plain mutation is shown to be destructive. Hidden state at zero per episode is fine
for episodic evaluation. Selection: primary = minimum over layouts of ticks alive, which is
dense, is not gameable by resting (no intake caps life at the no-intake baseline) and orders
generation zero; then audited terminal stores. Freeze that before the held-out set is opened,
as the plan says.

### F5 — Observation schema: current sensors are the wrong shape, and two proposed inputs are dead (tuning before R1)

**Source.** Today's `Observation` (`controller.rs:12-35`) carries own-cell P/F/D_eff,
*normalized* gradients, crowding repulsion, height, up and OU noise. The plan correctly
forbids unit-normalizing food gradients; the current sensor does exactly that. No internal
state, intake feedback, motion feedback, water or other-body sectors exist yet.
**Corrections.** Start smaller than 64–96: a v1 of roughly 30 values (own cell ×3, six
sectors × two rings of P and D_eff with magnitude, six sectors of body occupancy with
relative size, reserve, energy, gut, developmental stage, water, height/up, last intake,
last resolved speed/turn, one effort-capability value). Drop "contact/blocked-motion" from
v1 because no contact solver exists; a dead input is worse than none. Drop lateral effort
from the action set; the resolver moves along heading and a masked channel would be dead
weight. Seven outputs remain: thrust, turn, graze, fruit, scavenge, attack, reproduce. Keep
10 Hz updates. Version and hash the schema as planned; accept that founders trained on v1
are development artifacts that will be retrained under the R3 schema with mate cues. That
removes the "ABI frozen before mating" risk without freezing anything early.

### F6 — Lifecycle: inheritance rules will fight policy inheritance (must specify in R3, decide the principle now)

**Source.** Ordinary births copy one genome with sparse mutation (`step.rs:2092-2098`).
Apex offspring carry the profile genome exactly, no mutation. Apex mating averages every
gene (`encounter.rs:349-392`), which collapses variance, and pairs the nearest two perched
adults within `MATING_RADIUS_PX` automatically (`step.rs:481-566`), the rule the plan says
must not stand in for choice. Dormant apex emergence is rule-driven (`dormancy.rs`).
**Corrections.** For two-parent births take body and policy from the same RNG-chosen parent;
do not average genes under a policy that came from one side. Enable mutation for apex
lineages when policies become heritable, or there is no variety to select. Mate consent in R3
is both parties' reproduce output above threshold within the radius; the world still owns
funding, gestation and cooldown. Dormancy and emergence stay world-owned physiological
exceptions and must be listed as such in the R3 ownership audit. The R3 objective is lineage
size at a horizon of at least three generations (gestation 30 s, bud age 120 s, growth to
adult on the order of a minute, so ≥1,000 s) with a survival floor, never terminal stores.
Add one cheap transfer check in R2's held-out set: four copies of one policy in one arena,
because the training fixture has one animal and the live world has fifty to a hundred.

### F7 — Premature or missing infrastructure (sequence)

Defer MAP-Elites until two viable policies exist. Defer snapshot self-containment and
legacy migration to the end of R1, keeping legacy worlds on the legacy controller. Drop GPU
readiness from scope. Keep the 512-body stress check; it is cheap. Missing prerequisites
before R1: F1's contract, F2's probe and cropping floor, and the sector sampler for F5.

## Keep / change / defer

| Item | Verdict |
| --- | --- |
| World owns physics, physiology, reproduction cost; policy owns effort | Keep |
| GRU32, one layer, f64, fixed lifetime weights, private hidden state, 10 Hz | Keep |
| Whole-policy inheritance with mutation at paid births | Keep; same-parent body and policy |
| Resolver at one boundary, seam transport unpaid, tests in `motor_foundation.rs` | Keep |
| Envelope `capability = speed_cap + 2.5·ω_max` | **Change** to `u = speed_cap` (F1) |
| `ROTATION_COST_SCALE` 0.5 | Keep; no longer a balance lever after F1 |
| Producer model, 4 px cells, own-cell reach, regrowth constants | Keep for now |
| Controller-side `feed_min` leave rule | Change: world cropping floor (F2) |
| 600 s episodes, fixture starting stores | Change: 1,800 s, halved stores (F3) |
| GA for plumbing smoke | Keep |
| GA for the learning screen | Change: antithetic ES, same budget (F4) |
| 64–96 inputs, 8 outputs incl. lateral and contact | Change: ~30 inputs, 7 outputs (F5) |
| Gene-averaging recombination, unmutated apex offspring | Change in R3 (F6) |
| MAP-Elites grid, safe-mutation rescaling, GPU readiness | Defer |
| Legacy apex stalk timeout | Raise for the suite; non-gating |

## Milestone sequence

1. **R0b (next handoff, below):** honest envelope, cropping floor, mobile-grazing probe,
   native-size demo. Checkpoint.
2. **R0c:** observation/action schema v1 with the sector sampler and scripted probes of each
   input; no learning.
3. **R1:** GRU runtime, replay and reference tests, performance screen; legacy migration last.
4. **R2:** plumbing smoke with GA; learning screen with ES at the revised horizon; held-out
   set including the four-copy arena and the memory diagnostic.
5. **R3:** diets, apex, paid reproduction, lineage objective, ownership audit.
6. **R4:** M1 evaluation extended to the new biology.

## Behavioural coverage and exceptions

Neural in the first complete version: locomotion, search, habitat response, feeding and
departure, rest, pursuit and attack timing requests, escape, reproductive readiness and mate
consent. World-owned and to be listed as such: reach, contact, capture resolution, digestion,
maintenance, gestation and birth funding, growth, damage, death, dormancy and emergence,
the cropping floor, encounter cooldowns. Deferred capabilities: lateral locomotion, contact
sensing, parental care, communication, lifetime weight learning, morphology–controller
coevolution.

## Decisions for Wrysk

1. **Envelope contract.** Adopt `|v| + r|ω| ≤ effort-scaled translational capability` with no
   rotation allowance. *Preferred: yes.* The alternative, a named allowance factor, keeps
   today's turn rates for small bodies but re-creates the spinning the plan exists to remove.
2. **Pace.** Keep 0.3 px/s for R0b and judge from the native-size demo whether the world reads
   too slow; raise `speed_max` only then, scaling `move_cost` so full-effort travel costs the
   same per second. *Preferred: keep for R0b; decide at the checkpoint.* Under the honest
   contract the legacy apex becomes an ambush-only reference until R3 either way.
3. **First learning optimizer.** Antithetic ES for the learning screen, GA for plumbing.
   *Preferred: yes.* This is an optimizer choice inside the mutation-and-selection family the
   owner asked for, not an MLP arm or a survey.
4. **Apex inheritance principle.** Same-parent body and policy; mutation on for apex lineages.
   *Preferred: yes; specify in R3.*

## Next bounded handoff: R0b

**Scope.** (1) Replace `MotorLimits::capability` with the effort-scaled translational
capability; keep `turn_rate_max` as a clamp; update the `motor_foundation.rs` invariants and
the envelope tests accordingly, stating the reason. (2) Raise the legacy stalk timeout so the
hunter suite exercises capture timing; mark it non-gating. (3) Add a world-owned cropping
floor constant and its test. (4) Add the scripted mobile-grazing probe as an example beside
`food_stock_flow.rs`, 3×3 patch, hop-on-threshold, at most four arms, 36,000 ticks each.
(5) Rerun the six-run population measurement from the rotation-cost report unchanged.
(6) Native-size development demonstration of travel, pivot and rest; inspect, do not archive.
Exclusions: no schema, no GRU, no training, no pace change, no M1 edits, no reset.

**Acceptance.** Every body in the mixed-size world satisfies `|v| + r|ω| ≤ effort·speed_max/wading`
(plus burst) every tick; a Resting body's sweep is below 0.02 px/s; the six-run population at
`k = 0.5` is within a few organisms of the pre-R0a 96 at 300 s (if it is not, the price
still matters and the report says why); the probe reports net energy per cycle positive with
the fresh-cell yield, hop cost and recovery listed; the no-intake control dies inside 1,800 s;
the demo shows a turn that is visibly no faster than travel.

**Spending checkpoint.** One worker, one targeted review, two repair cycles. Report actual
usage. Stop and report if the envelope change breaks more than the hunter timing tests.

**Unresolved evidence needs.** The fixture cell's light level and how typical it is; the
world-average gross production; whether the pace should change; the memory diagnostic
design for R2.

## Usage

The harness exposed a remaining-context counter that fell from 15,000,000 to about
14,830,000 tokens during this review, roughly 170 k context tokens including cached
re-reads. Billed usage is unavailable.
