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
95 %, walks off stands it has barely touched, and ignores the "food here" scalar
it is given; the next move is the score, not the observation. The introduced
apex starves: it captures on 3 % of paid strikes and earns 6 % of its bill, and
even when introduced already past the age gate it never becomes ready because
its reserve only falls; the next question is its reach, not its eligibility.
The finer movement-price ladder is refuted under its pre-registered rules, and
in refuting it the campaign found that ecology v1's "depletion events" are almost
never grazed cells: 971 of 1,355 depleted cells never held a prey body, the
depleted set is the dimmest band of the habitat at every price, and the nine
recoveries are plant-side events in three cells. The depletion/recovery cycle
§4.4 was written for has not yet been observed, and the display shows a world
seeded above its own equilibrium in its dim cells. J's factorial (below) closes
the skimmer question. None of this is "ecosystem healthy"; all of it narrows
what the next design change must be.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| H intake diagnostic | Opus 5 high | `main` (`52fe697`…`e9c64fa`) | 5 s / 6 min | 23 MiB / 40 | core 487, search 120 |
| K apex death + eligibility | Opus 5 high | worktree, merged `9de1bf2` | 84 s / 8 min | 132 KiB / 20 | core 491, search 123 |
| I grazer-gated ladder | Opus 5 high | worktree, merged `4065012` | 3.3 min / 8 min | 4.3 MiB / 40 | search 152 |
| J form × diet factorial | Opus 5 high | worktree | *(pending)* | | |

Fable's verification: H's whole experiment re-run from its ignored test and
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
prices: plant-side events. **Reading:** a depletion event in ecology v1 is
almost never a grazed-out cell; it is a cell seeded at `0.4·P_cap` (§11,
`producer.initial_fraction`) in a habitat that cannot hold that foliage, losing
it slowly with nothing eating it. A's and F's counters are correct and the name
on them is not; the depletion/recovery cycle §4.4 was written for has not been
observed, not because it is rare but because nothing measured so far would be
one. Stated limits: "plant-limited" is empty by threshold (no depleted cell was
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

*(pending: J's result lands here)*

## What this does and does not establish

- Established by measurement: the policy's failure is residence on food, not
  effort, detection or clamping; the apex dies of a 3 % capture rate and never
  fills its reserve, whatever its age; the movement price cannot concentrate
  grazing without starving the grazer first; the counted depletions are
  habitat-limited declines in unvisited dim cells, and the counted recoveries
  are plant-side.
- Open: why the policy ignores the food-here scalar (a score question); why a
  strike ends out of reach (kinematics, escape or pursuit); whether the
  contract should name a critical `L·μ` and seed foliage by it; the skimmer
  (J).
- The cube is untouched by this round: build `77c42e8`, `fast-leaf`, shipped
  movement price, shoulder 0.95 by override. Nothing here changes a number the
  display produces. What the display's dim cells are showing, on I's reading,
  is a slow decline from an over-seeded start, not grazing.

## Next recommendation

*(to be reconciled with Astra after J lands)*
