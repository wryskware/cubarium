---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The forager score: Astra's energy-margin auxiliary, **not proposed**

Deliverable 3 of workstream L
([brief](handoffs/ecology-v1-score-checks-opus-2026-09-16.md)), from Astra's
[round-2 review](7_Research/ecology-v1-round2-review-2026-09-16.md), next steps item 2.

This is the document that would have proposed replacing or supplementing the trainer's score. It
does not, because one of the two checks the brief required **falsified the hypothesis the change
rests on**. Everything Astra asked to see fixed is fixed here anyway — the constants, the
post-death rule, the pre-registered falsifiers, the bounded experiment — so that the proposal can
be picked up unchanged if the evidence ever turns round, and so that the falsifier that fired is
on the record next to the thing it refused.

**Status: not proposed as the next intervention.** Nothing in the trainer changed; nothing was
trained. `es::trainer::score` is still `t_min + 0.25·mean normalised terminal stores`, and the
auxiliary below exists only as a reported column in
[`es::scorecheck`](../crates/cubarium-search/src/es/scorecheck.rs).

## The proposal, stated in full

Astra's fixed-horizon auxiliary on usable **energy margin**, not served mass:

```text
A = (1/T) Σ_{t=1..T} clip( (E_credited,t − E_billed,t) / b_ref , −1, +1 )
S = t_min + λ·A
```

with the ticks after death contributing **−1** each, rather than the mean being truncated at the
death tick; otherwise dying early avoids future bills and a corpse outscores a body that kept
paying for itself.

### The constants, fixed before any outcome

They are compiled into `crates/cubarium-search/src/es/scorecheck.rs` at commit `14345d9`, which
**precedes** the commit that records any ladder run under them. That ordering is the whole point
of pre-registration and it is checkable from the history.

| constant | value | why this value |
| --- | --- | --- |
| `T` | **36,000 ticks** | the fixture's own episode horizon (`fixture::HORIZON_TICKS`). A surviving body's `A` then covers exactly its life and a dead one's covers exactly the ticks it did not live. Any other `T` would make `A` depend on a second, invented clock. |
| clip | **±1**, symmetric | one tick's margin cannot count for more than one `b_ref` either way, so a single very rich bite cannot buy a life of deficit and one catastrophic tick cannot erase a life of surplus. |
| ticks after death | **−1 each** | Astra's rule, verbatim. |
| `b_ref` | **the body's own upkeep price for one tick**, 3.100 × 10⁻⁴ e on this fixture | a property of the fixture's body and its config, not of any run's outcome — the twelve layouts found the same mature grazer, so the same number comes back on every one. It makes `A = +1` read as "earned at least one tick of upkeep more than it was billed" and `A = −1` as "fell at least one tick of upkeep short": the scale the body's own standing-still cost sets rather than one this document invents. |
| `λ` | **100 ticks** | `A ∈ [−1, 1]`, so `λ·A` moves a score by at most ±100 ticks and the whole term spans 200 ticks — 10 s of simulated time, 0.56 % of the horizon, 2.4 % of generation 9's current `t_min ≈ 8,400`. **A survival difference of more than 10 s is preserved strictly**; one of exactly 10 s can at worst be tied by the two extremes of `A`; nothing larger can be inverted. That is Astra's constraint, and it is swept over the whole reachable range of `A` by `the_auxiliary_cannot_reorder_a_material_survival_difference` rather than argued. It is also 800× the existing stores tiebreak, whose entire range is 0.25 ticks and which therefore cannot reorder two candidates differing by a single tick: an auxiliary meant to be a *gradient* rather than a tiebreak has to be larger than that by a wide margin. |

### What `E_credited` and `E_billed` are, exactly

Both are read from the world's own per-body ledger (`World::record_body_budgets`,
`cubarium_core::BodyBudget`), differenced tick by tick. Nothing is reconstructed.

- `E_credited,t` = `Δ battery_credit_total` + `η_ox · e_r · Δ reserve_credit_total` — the usable
  energy this tick's **food** delivered: the charge that went straight to the battery, plus what
  the material that entered the reserve is worth once oxidised at the config's own
  `organism.oxidation_efficiency` and `organism.reserve_energy_density`. This is credited usable
  energy, which is what Astra asked for; served mass would reward low-yield intake and depletion
  without paying the body's energetic cost.
- `E_billed,t` = `Δ bill_total` — `MotorBill::total_cost`, the one number the world books:
  upkeep plus translation plus turn. Oxidation is deliberately **not** income (it is a transfer
  between the body's own stores, already counted once), and growth and gestation energy are
  deliberately **not** a bill (they are investments, and penalising them would score against
  growing). Gut credits are apex-only and are zero for every ordinary body here.

## Why it is not proposed: the falsifier that fired

The brief's check (b) asked whether the **current** score is already strongly and monotonically
increasing in dwell. It is. From the
[result note](7_Research/ecology-v1-score-checks-2026-09-16.md):

| rung | `t_min`, training four | `t_min`, all twelve |
| --- | --- | --- |
| dwell-20 | 23,490 | 12,951 |
| dwell-100 | 24,269 | 16,658 |
| dwell-300 | 27,559 | 21,370 |
| dwell-1000 | **36,000** | **27,362** |
| dwell-3000 | 18,309 | 18,309 |
| until-threshold | 36,000 | 26,355 |

Moving a scripted forager from a 1-second dwell to a 50-second dwell is worth **+12,510 ticks of
score on the training four and +14,411 on all twelve** — a 53 % and a 111 % increase. The
proposed auxiliary's entire range is **200 ticks**. The gradient the current score already
carries for exactly the behaviour generation 9 is missing is **seventy times larger** than
anything this proposal would add.

The score is not flat at the other end of the range either, where generation 9 actually lives:
eating one cell and then starving (`stationary-grazing`, 9,090) outscores eating nothing at all
(`no-intake`, 7,420) by 1,670 ticks.

So the premise — "nothing the search scores pays for staying" — is refused by measurement, and
adding a term seventy times smaller than the one already there cannot be the next intervention.

## The ladder under the proposed `S`, as Astra asked

Computed from the same episodes, under the constants above. `A` and `S` are reported columns of
`runs/ecology-v1-score-checks/ladder.json`; no candidate was ever ranked by them.

| rung | set | `A` | `S = t_min + 100·A` | current score | same order? |
| --- | --- | --- | --- | --- | --- |
| dwell-20 | train-4 | −0.2596 | 23,464.04 | 23,490.140 | yes |
| dwell-100 | train-4 | −0.2153 | 24,247.47 | 24,269.141 | yes |
| dwell-300 | train-4 | −0.1634 | 27,542.66 | 27,559.141 | yes |
| dwell-1000 | train-4 | −0.0607 | 35,993.93 | 36,000.157 | yes |
| dwell-3000 | train-4 | −0.4896 | 18,260.04 | 18,309.021 | yes |
| until-threshold | train-4 | −0.0396 | **35,996.04** | **36,000.181** | yes |
| dwell-20 | all-12 | −0.4504 | 12,905.96 | 12,951.083 | yes |
| dwell-100 | all-12 | −0.3957 | 16,618.43 | 16,658.093 | yes |
| dwell-300 | all-12 | −0.3180 | 21,338.20 | 21,370.093 | yes |
| dwell-1000 | all-12 | −0.1230 | **27,349.70** | **27,362.133** | yes |
| dwell-3000 | all-12 | −0.4535 | 18,263.65 | 18,309.021 | yes |
| until-threshold | all-12 | −0.1010 | 26,344.90 | 26,355.155 | yes |

**`S` is monotone in dwell over `d` ∈ {20, 100, 300, 1000} on both sets**, exactly where the
current score is, and it turns down at `d = 3,000` for the same reason (a body that stands 150 s
on the weak opening crops it to nothing and starves on it). `S` **inverts nothing**: every
pairwise ordering is the current score's. That is the ladder Astra asked to see before any ES
campaign, and it passes — the auxiliary is well-behaved. It is simply not needed.

`A` does behave as designed where it can be seen at all. It uses its range sensibly —
`no-intake` −1.0000, `stationary-grazing` −0.9421, generation 9 −0.9628, dwell-20 −0.4504,
dwell-1000 −0.1230, the mobile script −0.1010 — and generation 9's −0.9628 decomposes into a
mean margin over its lived ticks of **−0.843 `b_ref`** (range −0.926 to −0.770 over the twelve
layouts): while it is alive, generation 9 is billed about 0.84 ticks of upkeep more than it earns,
every tick. The mobile script's is −0.087 and `dwell-1000`'s −0.110.

And there is one place the auxiliary is strictly better than what is there now. On the training
four, `dwell-1000` and the mobile script both hit the horizon, so `t_min` ties at 36,000 and the
ordering falls to the tiebreak. The current stores term separates them by **0.024 ticks**; the
auxiliary separates them by **2.11 ticks**, 88× more, in the same direction. If a future campaign
ever saturates `t_min`, this term is the right one to break the tie with. That is an argument for
keeping the proposal on file, not for adopting it now.

## Pre-registered falsifiers

Fixed before the ladder was computed, and recorded here whether they fired or not.

| # | falsifier | fired? |
| --- | --- | --- |
| 1 | the present score already gives a strong dwell gradient | **YES** — +12,510 / +14,411 ticks across the ladder |
| 2 | `S` ranks a short rich burst above sustained feasible residence | no — `dwell-20` (10.08 m served, 6/12 alive) scores 12,905.96 against `dwell-1000`'s 27,349.70 (11.74 m, 11/12) |
| 3 | `S` ranks stationary starvation above sustained feasible residence | no — `stationary-grazing` 8,995.79 against `dwell-1000` 27,349.70 |
| 4 | `S` ranks early death above sustained feasible residence | no — `no-intake` 7,320.00 and generation 9 6,817.72, both far below every ladder rung |
| 5 | a dwell-script ladder is not monotone under `S` before any ES run | no — monotone over the whole feasible range, and it inverts no pairwise ordering of the current score |

Falsifier 1 is the decisive one, and it fired. Four of the five would have passed, which is
exactly why the fifth-and-first had to be checked: a well-behaved term is not the same as a
needed one.

## The bounded experiment that would have followed

Recorded for completeness; **not authorised, not requested, not launched**.

One campaign, on the same twelve `fast-leaf` layouts and the same ecology hash
`09e244392ec91768`, scored by `S` with the constants above and nothing else changed:

- 16 antithetic pairs plus the centre, the existing `σ` and Adam settings, 20 generations.
- 4 training layouts optimised, 8 held out and reported only. Horizon 36,000.
- Budget: 20 × 33 × 4 = 2,640 episodes of ≤ 36,000 ticks. At the measured 108 episodes in 19.7 s
  on 8 workers, about **8 minutes** of simulation; ≤ 8 workers, ≤ 100 MiB of checkpoints.
- Success is not "the score went up": it is the **on-food fraction** of the selected centre
  moving off 0.08 toward the ladder's 0.65–0.94, measured by the same per-tick trace H and this
  workstream used. A campaign that improves `S` without moving residence has told us nothing.
- The matched control is the same campaign under the current score from the same seed, or the
  existing `es-eco-v1-fastleaf` run if its seeds match, because a 20-generation run under a new
  score and no control cannot attribute anything.

## What this does not touch

Nothing in this document proposes changing, and nothing in workstream L changed:

- the GRU (`neural/gru.rs`), the 70-scalar observation (`neural/obs.rs`), the seven-channel
  action adapter and its deadband (`neural/action.rs`), or the schema digest — no trained policy
  is invalidated by anything here;
- the motor contract, the intake law, oxidation, growth, the ecology v1 equations, or any
  config value;
- `es::trainer::score`, the optimiser, the fixture's layouts, hashes or horizon, or any
  `es-*` command's behaviour;
- the snapshot schema — the ledger and the trace this uses are transient, opt-in and unhashed;
- the cube, `state/`, port 7393, the shim, or the running `cubarium`.

The only behavioural addition is `Control::Dwell(d)`, a disclosed diagnostic control that no
candidate rollout can ever use (`Driver::Control` is never a candidate driver).

## If the evidence turns round

The one gap check (b) leaves open is named in the result note: the ladder measures the score's
gradient in residence **given route-following**, and generation 9 is not a route-follower. If a
later experiment shows the score is flat in generation 9's own behavioural neighbourhood — a
ladder interpolating between wandering with 93-tick dwells at 8 % on food and the route-follower
— then this proposal is the one to reach for, unchanged, constants and all. Until then the
evidence points at the search, not the score: see the result note's three candidates, of which
the first (the within-generation score spread from the existing checkpoints) needs no simulation
at all.
