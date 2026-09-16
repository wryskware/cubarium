---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 2: results of workstreams H, I, J and K (2026-09-16)

Fable's consolidated result for the reconciled next steps of
[ecology-v1-next-steps-results-2026-09-16.md](ecology-v1-next-steps-results-2026-09-16.md)
("Next recommendation", the order Astra set in
[its review](ecology-v1-next-steps-review-2026-09-16.md)). Briefs:
[H](../handoffs/ecology-v1-intake-opus-2026-09-16.md),
[I](../handoffs/ecology-v1-ladder-opus-2026-09-16.md),
[J](../handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md),
[K](../handoffs/ecology-v1-apex-eligibility-opus-2026-09-16.md), committed at
`2a1cedd`. All four ran in parallel: H on `main`, I, J and K in worktrees. Wrysk's
standing instruction for this round: keep going, tune nothing, backlog the
operator controls and the artwork pass ([backlog](../backlog.md)).

**Disposition, in one paragraph.** Each of the four questions the last round
left open now has a measured answer, and three of the four move the obstacle
rather than remove it. The trained forager does not eat because it does not
*stay*: its mouth is open whenever it stands on food and its bite is never
clamped, but it stands on food 8 % of the time against the scripted control's
95 %, and walks off stands it has barely touched although the "food here"
scalar is in its observation; whether its frozen weights, hidden state or
update cadence use that scalar was not tested, so the training score is the
leading hypothesis for the next move, to be falsified first. The introduced
apex starves: it captures on 3 % of paid strikes and earns 6 % of its bill, and
even when introduced already past the age gate it never becomes ready because
its reserve only falls; the next question is its reach, not its eligibility.
The finer movement-price ladder is refuted under its pre-registered rules, and
in refuting it the campaign found that most of ecology v1's counted "depletion
events" had no grazing observed at probe resolution: 971 of 1,355 depleted
cells never held a prey body at any one-second probe, the depleted set is the
dimmest band of the habitat at every price, and the nine recoveries are
plant-side events in three cells. The depletion/recovery cycle
§4.4 was written for has not yet been observed, and the display shows a world
seeded above its own equilibrium in its dim cells — as a leading hypothesis:
the classification rests on one-second occupancy probes, attributed rather
than measured bites, and a derived static critical `L·μ` the contract does not
name, so what is established is that most counted crossings had no *observed*
grazing at probe resolution. J's factorial, once counterbalanced after review,
reverses the sign of F's skimmer association: with the diets exchanged on the
same cells the founder's 0.60 still beats 0.85 in 29 of 32 within-slot pairs
and in all four worlds, because the 0.2 threshold shuts the litter channel and
foliage does not make it up. Arm C's yield gap (0.41 against 0.90 digestible
per served) is a diet-locus difference, not a body cost; body and habitat are
not separable in this roster, with `depth` the leading mechanism. Astra's review
([ecology-v1-round2-review-2026-09-16.md](ecology-v1-round2-review-2026-09-16.md))
and the repair are recorded below. None of this is "ecosystem healthy"; all of
it narrows what the next design change must be.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| H intake diagnostic | Opus 5 high | `main` (`52fe697`…`e9c64fa`) | 5 s / 6 min | 23 MiB / 40 | core 487, search 120 |
| K apex death + eligibility | Opus 5 high | worktree, merged `9de1bf2` | 84 s / 8 min | 132 KiB / 20 | core 491, search 123 |
| I grazer-gated ladder | Opus 5 high | worktree, merged `4065012` | 3.3 min / 8 min | 4.3 MiB / 40 | search 152 |
| J form × diet factorial | Opus 5 high | worktree, merged `c38e8a7` | 100 s / 10 min | 128 KiB / 20 | core 502, search 174, host 596 |

Fable's verification: J's whole campaign re-run and compared row for row
(12 of 12 identical, timing keys excluded), and after review the
counterbalanced arm run beside it (arm A reproduces by hash on 4 of 4 seeds); H's whole experiment re-run from its ignored test and
compared field for field with the retained aggregate (identical, timing keys
excluded); K's age-eligible probe re-run and compared (identical); two interior
ladder prices re-run for one seed and matched by `final_state_hash` (4 of 4); I's
own check that its shared prices reproduce F's arm-0 rows (24 of 24) and that
the ledger and the per-cell record leave the world's hash unchanged. Merges were
conflict-free; after each merge the affected suites were re-run. Three workers
independently hit the same shared-`target/` stale-artifact race (a build failing
on a symbol the just-passed tests used, pointing at another worktree's copy of
the file); `touch crates/cubarium-core/src/lib.rs` clears it every time. Two
worktrees were handed out at an ancestor of `main` and the workers reset them to
the brief's commit before starting; both said so.

## H — why generation 9 does not eat

Full note: [ecology-v1-intake-2026-09-16.md](ecology-v1-intake-2026-09-16.md).
Core gained a per-tick intake trace inside the budget recorder (opt-in, inert by
hash test, ≤ 1.2 % cost with the ledger, zero when off); the search side an
ignored-test entry point (`es::intake::run`, promotable to a subcommand in one
line; a subcommand would have collided with three open worktrees).

| driver (medians, 12 `fast-leaf` layouts) | alive | on-food fraction | mouth open when on food | served / requested | credit / bill |
| --- | --- | --- | --- | --- | --- |
| mobile script | 11/12 | **0.952** | 0.989 | 1.000 | 1.03 |
| initial centre | 0/12 | 0.046 | 1.000 | 1.000 | 0.12 |
| generation 9 | 0/12 | **0.083** | 1.000 | 1.000 | 0.24 |

**Verdict (b): it is rarely on food.** (a) "efforts off on food" is refuted:
8,365 of 8,365 on-food ticks had the matching mouth open, zero `effort_zero`
in 100,101 traced ticks. (c) "bite clamped" is refuted: served equals requested
to 1.000000000 over 611,253 ticks; every clamp term is zero for both policies.
The fifteen-fold intake gap decomposes as 48.9× fewer on-food ticks against
3.2× richer bites. Sharper: it is **residence, not detection**. On the corridor
layout generation 9 stands on 23 of 27 food cells yet spends 88 % of its life
off them; its ticks per food cell versus per bare cell are 1.5× there and
exactly 1.0× on another layout against the control's 67×; median dwell per
visit 93 ticks against 2,299; every recorded departure left a stand still above
`feed_min` with about 4 % removed; 37 % of its life is spent on side faces with
no food painted. The observation does not lack the signal (`v[0] = P_here /
P_max` plus 36 ring-sector food scalars and what it ate): it stands on 0.6 with
`v[0]` = 0.4 and walks off. **Named next move: a score change** — the current
`t_min + 0.25·stores` pays for staying only thousands of ticks downstream of the
bite; the ledger's income / bill is a dense replacement. Whether it replaces or
joins `t_min` is Fable's call, and no training runs before that call. Secondary,
recorded: both policies saturate the shared mouth at Σ effort = 1 and split it in
thirds on every tick (a constant, not a response), so two thirds of the mouth
goes to channels empty 94 % of the time — worth about 3× on the bite, not the
49× the contact deficit needs. What this does not establish: why the policy
ignores `v[0]` (a policy-side sweep would), or the score's landscape.

## K — what kills an introduced apex, and one eligibility probe

Full note: [ecology-v1-apex-eligibility-2026-09-16.md](ecology-v1-apex-eligibility-2026-09-16.md).
The introduction door gained an optional founder age (refuses an age the world
cannot represent rather than clamping; age 0 is byte-identical to the old door
by state hash); `apex-audit` gained the per-body ledger and per-member records;
the display's apex profile and the mating radius are unchanged.

**Death diagnosis.** Starvation in 16 of 16 lives, and nothing else is close.
The apex genome has `diet` 0 and the profile no scavenging, so predation is its
only channel, and predation returned 4.19 m of gut credit across all sixteen
lives together against about 4.26 m each needed to break even: 6 % of its bill.
The bill is 65 % a fixed upkeep (3.7e-4 e per tick from adult structure and a
12-unit sense gene), 33 % strike costs (449 paid attempts, 402 ended
`OutOfReach`, 31 missed, 15 captured: a 3.3 % capture rate), 1.7 % travel. Nine
of sixteen lives earned exactly zero in their last 2,000 ticks while still paying
about 1.5 e. No injury, no dormancy, and age 14,101 against a 144,000-tick
lifespan. Sharpest case: 28 paid strikes, 0 captures, 36 % of the life's
inventory spent on hunting that returned nothing.

**Probe: readiness still never opens.** Both adults introduced at age 1,200 s
(the minimum reproduction age) at tick 24,000, the earliest tick at which that
age is representable, with a matched age-zero control at the same tick that
differs in no field but the age. Simultaneously-ready ticks 0 in 8 runs over
206,763 member-ticks; matings 0. The first refusing term of `may_reproduce`,
checked against the real predicate on every sampled tick with zero mismatches
over 605,239 member-ticks: **reserve below 0.8·R_max on 80 % of ticks**, holding
a target 18 %, carrying a carcass 2 %, energy 0, age 0. An introduced apex never
once held more reserve than the 0.5·R_max it was founded with. **Correction to
E:** at tick 24,000 the two adults do sense each other — 1,180 candidate pairs
in both arms — so meeting is not impossible, only moot while nobody is ready;
E's "never sensed each other" was true of tick 6,000. The `fail_radius` count
is an evaluation-order artefact, not evidence about the radius. **Named next
task: the apex's reach, not its eligibility** — per paid attempt, hunter–prey
separation at windup, strike start and settlement against capture reach and the
prey's realised escape speed, to separate strike kinematics, escape multiple and
pursuit controller. Lowering the reserve fraction is pointless while intake is
6 % of break-even.

## I — the finer price ladder, and what "depletion" has been counting

Full note: [ecology-v1-ladder-2026-09-16.md](ecology-v1-ladder-2026-09-16.md).
Pre-registration committed before any row (`c7e498d`), 28 definition tests red
against a stub first (`7164352`), then `depletion.rs` (the per-depleted-cell
record: `L·μ`, `P/P₀` trajectory, last visit, post-depletion visits and
attributed bites, first recovery and re-depletion), the founder-grazer brood
gate, and E's ledger on the search side (`--ledger`). Sixty rows, arm 0, six
seeds, 0 extinctions.

| config | price | range ÷ control | depletions | recoveries | grazer broods / seeds bred | verdict |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.00036 | 1.00 | 11.3 | 0.17 | 1.5 / 2 of 6 | control |
| baseline | 0.0006 | 0.60 | 36.0 | 0.33 | 0.0 / 0 | neither |
| baseline | 0.0009 | 0.56 | 41.2 | 0.17 | 1.0 / 1 | partial |
| baseline | 0.0012 | 0.18 | 56.8 | 0.33 | 0.0 / 0 | partial |
| baseline | 0.0018 | 0.18 | 56.2 | 0.33 | 0.0 / 0 | partial |
| `fast-leaf` | 0.00036 | 1.00 | 0.8 | 0.00 | 8.8 / 6 of 6 | control |
| `fast-leaf` | 0.0006 | 1.03 | 0.8 | 0.00 | 7.7 / 6 | partial |
| `fast-leaf` | 0.0009 | 0.98 | 2.5 | 0.00 | 4.7 / 3 | neither |
| `fast-leaf` | 0.0012 | 0.67 | 5.5 | 0.17 | 1.5 / 1 | neither |
| `fast-leaf` | 0.0018 | 0.39 | 15.3 | 0.00 | 0.0 / 0 | partial |

**The ladder is refuted**: no price gives both a breeding founder grazer in
≥ 4 of 6 seeds and range below 60 % of the control. In `fast-leaf` the first
rung where range moves (0.0012) already leaves the grazer breeding in 1 of 6;
paying 1.7× for travel at 0.0006 buys zero concentration. At baseline the
founder grazer mostly does not breed even at the shipped price (2 of 6), which
corrects F's reading: the price removes the two seeds that did. The ledger
measures what F could only infer: founder grazers that die without breeding
served 0.01–0.08 m in a whole life against 10–22 m for one that feeds, and
never default on a bill; they starve by running the reserve out having barely
eaten.

**What the depletion counter has been counting.** Of 1,355 depleted cells over
60 runs, **971 never held a prey body at any probe of the run**; of the 384 ever
visited, the median gap between last visit and the depletion crossing is 48–145
simulated minutes; 6 cells have any attributed post-depletion bite. The
classification is one-sided: recovered 9, pressure 1, plant-limited 0, marginal
1,345 (every depleted cell has `L·μ` below the derived critical value; the
brightest that ever depleted is 0.378 against a critical 0.447 baseline / 0.345
`fast-leaf`). The decisive row: one baseline seed depletes the identical 14
dimmest cells at three prices, then the same next-dimmest 66 at the two higher
ones. The price does not pick out where grazers congregate; it moves a
brightness threshold down the habitat, through the world's nutrient pool
(790 → 530 as leaf eaten collapses; a correlate, not a demonstration). The nine
recoveries are three dim cells recovering at nearly the same tick at four
prices: plant-side events. **Reading, as a leading hypothesis** (softened after review): most counted
crossings had no grazing observed at one-second probe resolution, and the
depleted set is the dim band; the proposed mechanism is a cell seeded at
`0.4·P_cap` (§11, `producer.initial_fraction`) in a habitat that cannot hold
that foliage, losing it slowly with little or nothing eating it. What would
confirm it: a herbivore-absent run of the same worlds in which the same cells
cross at the same times with a measured negative plant budget (actual light and
`N`, `P` and `Q`, production and loss, exact withdrawal). What would refute it:
the crossings disappear, or their measured plant budget is positive. A's and F's
counters are correct as counters; whether "depletion" is the right name for
what they count is what that run decides. Stated limits: "plant-limited" is empty by threshold (no depleted cell was
above critical, so the plant equation in an adequate cell is untested); the
critical value is derived from the contract's §13 break-even and the contract
names no such constant; `L·μ` is the static habitat; per-cell served is
attributed at one-second probes, not measured. **Named, not proposed:** making
the opening foliage a function of the cell's own break-even would remove the
never-visited depletions; whether §11 changes is Fable's decision. Named next
measurement: per-cell nutrient and light at the depleted cells' probes, one arm
with the herbivores absent at tick 0, the counter split by ever-visited, and
the plant reserve `Q` recorded beside `P`.

## J — the controlled form × diet factorial

Full note: [ecology-v1-diet-factorial-2026-09-16.md](ecology-v1-diet-factorial-2026-09-16.md).
Core gained a third founding door, `World::found_animal_with_genome` (both
public doors now share one private founding; an out-of-bounds genome is
refused, not clamped); the search side a `factorial` subcommand. Reproduction
was switched off for the eight clones only through the ES fixtures' scripted
seam (`bud: Some(false)`), so the 24 legacy founders kept breeding and
competition was real; 0 clone births in 96 clones. Tests first: 27 red against
a stub (`d0daef1`). `fast-leaf`, 4 training seeds, horizon 90,000, no apex.

**The brief's habitat stratification could not be built, and the refusal is a
finding**: the deepest pools hold no food at all — ecology v1 grows foliage only
in a cell that carries wood, and `water.algae_light` raises a living cell's
light floor rather than creating a producer — and on one seed only 6 of 1,280
cells are both wet and foliated. So placement is eight fixed anchors across the
five faces, resolved to the nearest living cell, and habitat is measured per
clone rather than assigned.

| arm | comparison | established (≥ 750 s) | median life | what the ledger shows |
| --- | --- | --- | --- | --- |
| A, diet within the skimmer body | `diet` 0.60 | **13 / 16** | 885 s | 1.48 m foliage + **4.31 m litter** served |
| A | `diet` 0.85 | **1 / 16** | 378 s | 1.32 m foliage, **0** litter; 15 of 16 served < 0.3 m in a life |
| B, body at `diet` 0.85 | burrower / grazer / glider / skimmer | 2 / 2 / 2 / **0** of 8 | 436 / 347 / 369 / 385 s | not resolvable at four seeds (p = 0.47) |
| C, roster pairing | burrower 0.10 vs skimmer 0.60 | 8 / 8 vs 7 / 8 | 1,076 vs 826 s | digestible / served **0.90 vs 0.41**: same food, half the yield |

**Verdict: the diet leg, reversed; the body leg unmeasured.** (Corrected after
review.) Arm A as first run had its diets fixed to slots, and the low-diet
slots were the richer cells on every seed, so Fable added the swapped arm
(`--arms As`, same cells, diets exchanged). Within-slot across the two arms the
low diet wins 29 of 32 pairs and establishes 22 of 32 against 2 of 32, in all
four worlds; the clone-level Fisher test is withdrawn and the world is the
replicate (4 of 4).
F's "foliage-diet skimmers survive 84 %" is most plausibly survivorship: with
reproduction on over 150 minutes, lineages that win the foliage lottery are the
ones that leave descendants, born into the patch their parent won — the
confound F named. J's cold-founding experiment makes that explanation
plausible; it does not identify the causal process in F's descendant census. Arm C's yield gap is a diet-locus difference (the two roster diets' capacities),
not a body cost: arm C moves form and diet together. At a foliage diet the body
leg is not resolvable and J declined to report it as one; the body leg is
therefore unmeasured and the depth-only factorial is its test. Habitat is the
leading coupling, not the shown one: the skimmer spends 65 % of its probes in
water against the glider's 11 %, and 39 of the 40 deepest cells across the
seeds carry no foliage; arm B cannot separate `form` from `depth`, and form
bundles size, speed, swimming and metabolism too. A structural finding the arms were not designed
for: the two food channels fail differently — of 64 pure-foliage clones, 51
served under 0.5 m and died on the no-intake floor while 11 served over 5 m
(all-or-nothing); all 32 detrital clones served 1.1–14.3 m and none ever reached
the horizon or a positive margin. A detrital diet buys a long certain decline; a
foliage diet is a lottery that mostly ends in six minutes. **Named next:** move
`depth` and nothing else in arm A's design (0.10 vs 0.55 crossed with `diet`,
about 25 s); what distinguishes the 8 lottery winners (they visited 271–595
cells, the 51 that never ate 30–108); and whether the detrital trickle can fund
a body in `fast-leaf` at all, which is a calibration question.

## What this does and does not establish

- Established by measurement: the policy's failure is leaving food, not
  effort or clamping (the food signal is present in the observation; whether
  the frozen weights, hidden state or cadence use it was not tested); the apex
  dies of a 3 % capture rate and never fills its reserve, whatever its age; the
  movement price cannot concentrate grazing without starving the grazer first;
  most counted depletion crossings had no grazing observed at probe resolution
  and fall in the dim band; the counted recoveries are plant-side.
- Established by J with the counterbalanced arm: for the skimmer body the
  founder's generalist diet is the better of the two, in every world and
  within cells; arm C's yield gap is the two roster diets', not the body's;
  foliage foraging from a cold start is all-or-nothing and detrital foraging
  is a certain slow decline.
- Open: why the policy leaves food (a score gradient is the leading
  hypothesis; cadence and recurrent dynamics are not excluded); why a strike
  ends out of reach (kinematics, escape or pursuit); whether the counted
  depletions are over-seeding (the herbivore-absent plant-budget run decides);
  whether `depth`, not the body, couples the skimmer to litter; whether any
  detrital diet can fund a body in `fast-leaf`.
- The cube is untouched by this round: build `77c42e8`, `fast-leaf`, shipped
  movement price, shoulder 0.95 by override. Nothing here changes a number the
  display produces. What the display's dim cells are showing, on I's reading,
  is a slow decline from an over-seeded start, not grazing.

## Review and repair (Astra, 2026-09-16)

Astra's review is
[ecology-v1-round2-review-2026-09-16.md](ecology-v1-round2-review-2026-09-16.md).
Disposition: retain H's trace, I's refutation and K's ledger as evidence; do not
accept the full causal reading yet. Repair 1, on `main`:

- **P1, J's arm A confounded diet with cell.** Fable added the swapped arm and
  ran it (above): the effect follows the diet within slots in all four worlds.
  The clone-level Fisher test is withdrawn; the world is the replicate.
- **P1, J's arm C is a diet-locus yield difference, not a body cost.**
  Corrected in J and here; "one statement because of depth" softened to "not
  separable here".
- **P2, I's mechanism overstated.** "Never visited" is "never observed at a
  one-second probe"; bites are attributed, not measured; the critical `L·μ` is
  a static proxy the contract does not name. The over-seeding reading is kept
  as the leading hypothesis with its confirmation and refutation named.
- **P2, H's cause not established.** "Not detection" reduced to "the signal is
  present; whether the weights use it was not tested"; the score change is the
  leading hypothesis, to be falsified first by Astra's two cheap checks.
- **P3, K and integration:** no change needed.

## Next recommendation (reconciled with Astra)

Astra's order, which Fable accepts: remove the two causal confounds first (one
is now done), gate both design decisions on a measurement, and keep the cube
untouched. The two decisions are Wrysk's; what Fable and Astra would tell him
is stated at each.

1. **J's counterbalance — done** (the swapped arm above). Diet is causal for
   the skimmer body's establishment; the body leg is still unmeasured.
2. **Two cheap falsification checks, then a score experiment, not a campaign.**
   (a) Sweep the frozen generation-9 controller over otherwise identical
   observations with the local and ring food scalars varied, with reset and
   with carried hidden state, to see whether its movement head responds to
   food at all. (b) Score scripted policies that differ only in dwell on food
   under the *current* `t_min + 0.25·stores`. If the present score already
   gives a strong monotone dwell gradient, the missing-gradient diagnosis is
   wrong; if the movement head already responds to food while residence still
   fails, cadence or recurrent dynamics come first. Only if both checks pass
   does the proposal go to Wrysk: Astra's fixed-horizon auxiliary
   `A = (1/T) Σ clip((E_credited − E_billed) / b_ref, −1, 1)`, with ticks after
   death scored −1, `S = t_min + λ·A`, `T`, `b_ref` and a small `λ` fixed
   before any outcome is seen, and a dwell-script ladder demonstrating
   monotonicity before any ES run. Credited usable energy, not served mass, so
   low-yield intake and depletion are not rewarded. **What Fable would tell
   Wrysk:** approve this as a bounded scoring experiment after the two checks;
   it is not a campaign and it changes nothing on the cube.
3. **Measure the plant budget before deciding what "depletion" or opening
   foliage means.** Run the same worlds without herbivores and record per
   cell: actual effective light and `N`, `P` and `Q` with reserve transfer,
   gross production and loss, exact consumer withdrawal (zero in that arm), and
   the crossing and recovery times; then the herbivore arm with exact
   withdrawal. Split the counter into "crossed with exact withdrawal since the
   last recovery" and "crossed without". If plant-only cells cross at the same
   places and times with a negative measured plant budget, the seeding reading
   is confirmed; if they stay above threshold, look at missed consumption and
   animal-mediated nutrient and light first. **What Fable would tell Wrysk:**
   approve the measurement and the counter split now; defer the §11 seeding
   change and do not put a universal critical `L·μ` in the contract (it is
   state-dependent; a documented analysis proxy at reference `N` is fine).
   The seeding change, if it comes, has a visible consequence — dim cells begin
   less lush and more heterogeneous instead of greening uniformly and fading —
   so it is an ecological and presentation decision, not a free correction.
4. **Instrument apex reach before touching eligibility or mating.** For every
   paid strike: separation at intent and at resolution, predator and prey
   displacement and heading, target identity continuity, whether the target
   crossed a cell or movement boundary; compare captures with `OutOfReach`.
   Failures that begin in range and resolve out implicate cadence or
   resolution; failures that begin out of range implicate target selection or
   pursuit. Age, reserve and radius stay unchanged until hunters can fund
   themselves.
5. **Then the remaining factorials in causal order:** the depth-only factorial
   with diet, placement and metabolic parameters fixed; I's nutrient probe
   folded into the herbivore-absent run rather than treated as its own study;
   and whether a detrital diet can fund an otherwise fixed body in `fast-leaf`,
   through the ledger's served → digestible → credited → billed chain, without
   inheriting arm C's body-effect wording.

Nothing in this list touches the cube.
