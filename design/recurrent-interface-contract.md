---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# Recurrent sensory/action contract (proposed v1)

Fable's R0c deliverable under the [dispatch](handoffs/post-fable-dispatch-2026-09-14.md)
and the [R0c brief](handoffs/r0c-fable-contract-2026-09-14.md). It proposes one
implementation-ready interface between a recurrent policy and the world for the
first foraging learner, plus the versioned extensions that complete behavioural
ownership. It is a proposal: nothing here is canon, and no code implements it.

Context it builds on: the [recurrent plan](recurrent-organism-plan.md) §2–5 and §8–9,
the [review](7_Research/fable-recurrent-plan-review-2026-09-14.md) F5–F7, the
[R0a motor-cost](7_Research/r0a-motor-cost-ecology-2026-09-14.md) and
[food stock/flow](7_Research/r0a-food-stock-flow-2026-09-14.md) reports, and the
current source (spans cited inline; verify with Graft before editing).

**R0b incorporated.** The motor semantics below are the ones Opus shipped in
`99a2bfc` and documented in the
[R0b result](7_Research/r0b-motor-foraging-result-2026-09-14.md); the measured grazing
numbers in §9 come from that report's `mobile_grazing` diagnostic. §10 lists what
was resolved from it and what remains open.

## 0. Fixed frames, units and constants used below

| Symbol | Meaning | Source |
| --- | --- | --- |
| `dt` | 0.05 s, one world tick (20 Hz) | `lib.rs:82-84` |
| `Δt_c` | 0.1 s, one controller interval (every second tick) | plan §3 |
| px | surface pixel; a cell is 4 × 4 px, 16 × 16 cells per face | `cubarium-surface/src/field.rs:12-14` |
| m | material unit of `P`, `F`, `D`, reserve and structure | fields/organism |
| e | energy unit of `energy`, upkeep and motor bills | organism |
| `P_max` | producer carrying capacity per cell, `cfg.producer.max` (1.5 m default) | `config.rs:83-96` |
| `w_flood` | `cfg.water.flood` (1.5 default) | `config.rs:173-174` |
| `r_sense` | phenotype sensing radius, 2–12 px | `genome.rs` |
| `hops` | `clamp(ceil(r_sense / 4), 1, 3)` graph hops sensed | `world/lifecycle.rs:262-269` |
| `S, S_adult` | structure and adult structure (m) | phenotype |
| `R_max, E_max` | reserve and energy capacities | phenotype |
| `v_max` | `phenotype.speed_max`, px/s (0.3 for a unit adult) | phenotype |
| `ω_max` | `drives.turn_rate_max_deg` in rad/s (90°/s default) | genome drives |
| `r` | outer body radius that sweeps when pivoting, px: `max(lobe extent, grasp offset + reach)` at current scale. Native values: unit adult 2.5, size-0.6 founder ≈ 1.5, size-2.0 founder ≈ 5.0, adult lanternjaw ≈ 14.8 | `motor.rs:313` |
| `u` | `min(speed_cap, affordable_motor)`, px/s, with `speed_cap = effort · v_max / wading` (raised to a burst only on the world's boost list); `ω_max` is a separate clamp, never an addend | `motor.rs:161-173`, `world/step.rs:1089-1096` |
| `u_full` | `u` evaluated at `effort = 1`: what the body could spend this tick at full activation | derived |
| `ω_attain` | `min(ω_max, u_full / r)`: the fastest pivot the body can actually perform this tick | derived |
| body frame | +x along the organism's unit `heading` in its own chart, +y its clockwise side; the same basis `stamp_rig` and the apex capture offset use | `hunter/profile.rs` |

Every spatial input is expressed in the body frame of the *observer's own chart*,
using the positions the world already unfolds for it (`unfold_with`, neighbour
lists). Chart transport never enters the policy: a seam crossing changes the chart
of `heading` and of the held action together, so body-relative values are unchanged.

## 1. Observation vector v1: 70 scalars

Every value is `f64`, finite, clamped to the stated range. Fixed physical scales
only; never population statistics. "Absent" encodings are listed per family.

### 1.1 Layout

| Index | Count | Field | Range | Definition and scaling | Source | Absent |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 1 | `P_here` | [0, 1] | own-cell producer `P / P_max` | `fields.p` | — |
| 1 | 1 | `F_here` | [0, 1] | own-cell fruit `min(1, F / P_max)` | `fields.f` | — |
| 2 | 1 | `D_here` | [0, 1] | own-cell edible detritus `min(1, D_eff / P_max)`, `D_eff = D·min(1, ρ/e_r)` | `step.rs:376-379` | — |
| 3–20 | 18 | `food_near[k][c]` | [0, 1] | near ring (hop 1), sector `k` ∈ 0..6, channel `c` ∈ {P, F, D_eff}; index `3 + 3k + c` | §2 | 0 |
| 21–38 | 18 | `food_far[k][c]` | [0, 1] | far ring (hops 2–3 merged), same order; index `21 + 3k + c` | §2 | 0 |
| 39–50 | 12 | `body[k]` = (`presence`, `rel_size`) | [0, 1] each | per sector `k`: index `39 + 2k` presence, `40 + 2k` relative size of the body giving that presence | neighbour lists | (0, 0) |
| 51–52 | 2 | `crowd` = (fwd, side) | [−1, 1] | the existing repulsion sum rotated into the body frame, magnitude squashed `x/(1+x)` | `step.rs:416-427` | (0, 0) |
| 53 | 1 | `water` | [0, 1] | own-cell `min(1, w / w_flood)` | `fields.w` | — |
| 54 | 1 | `light` | [0, 1] | own-cell sampled light this tick | `habitat::sample` | — |
| 55 | 1 | `height` | [−1, 1] | embedded height (Top = 1, rim = −1) | `step.rs:380` | — |
| 56–57 | 2 | `up` = (fwd, side) | [−1, 1] | unit up-slope direction in the body frame; zero on the level top face | `up_direction` | (0, 0) |
| 58 | 1 | `reserve` | [0, 1] | `reserve / R_max` | organism | — |
| 59 | 1 | `energy` | [0, 1] | `energy / E_max` | organism | — |
| 60 | 1 | `development` | [0, 1] | `min(1, S / S_adult)`; below 1 is a juvenile | organism | — |
| 61 | 1 | `gestating` | {0, 1} | `escrow.is_some()` | organism | — |
| 62 | 1 | `age` | [0, 1] | `age_seconds / max_age_seconds` | organism, `cfg.organism.max_age_seconds` | — |
| 63 | 1 | `motor_avail` | [0, 1] | `min(1, u_full / v_max)`, `u_full = min(v_max / wading, affordable_motor)`: wading and the energy budget both show here, and both now throttle turning as well as travel | motor stage | — |
| 64–66 | 3 | `ate` = (graze, fruit, scavenge) | [0, 1] | material actually removed from fields by this mouth since the last controller update, each `/ (mouth_rate · Δt_c)` | settlement (`requests`) | 0 |
| 67 | 1 | `moved` | [0, 1] | mean resolved speed since last update `/ v_max`, clamped | `ResolvedMotion.speed` | 0 |
| 68 | 1 | `turned` | [−1, 1] | signed physical turn since last update `/ (v_max/r · Δt_c)`, clamped: the fixed per-body scale of a full-budget pivot, so the channel uses its range at native pace | `ResolvedMotion.turn` (transport excluded) | 0 |
| 69 | 1 | `delivered` | [0, 1] | Σ resolved motor magnitude ÷ Σ requested magnitude over the interval's ticks; exactly 1 when the requested sum is 0 (nothing asked, or an empty interval) | resolver | 1 |

**Total: 70.** Grouped: food 39, bodies 14, surroundings 5, internal 5, capability 1,
feedback 6. Parameter count with the §5 network: `3·32·(70 + 32 + 2) + 7·(32 + 1) = 10,215`.

### 1.2 Why these and not others

- **Own-cell food is the concentration at feeding reach.** Intake is settled per cell
  (`step.rs:1277-1330`); there is no sub-cell mouth position, so "at reach" is the cell.
- **Fruit gets its own sector channels.** The initial learner is a grazer with
  `diet ≥ 0.5` and may eat fruit, which settles first in the intake law; a policy that can
  only smell fruit underfoot could not seek it. The cost is 12 values. If the R1 throughput
  screen forces a cut, the 58-value variant drops indices 3+3k+1 and 21+3k+1 (the `F`
  channels) and nothing else changes.
- **Magnitude is preserved.** Sector values are weighted means of cell stocks in material
  units, scaled by one fixed constant. The current sensor unit-normalises its gradients
  (`step.rs:413-415`, `normalize_or_zero`); that is exactly what the plan forbids and it is
  not reused.
- **Bodies carry presence and observable relative size only.** No IDs, genome, energy,
  role or "is prey" bit. Relative motion is *not* implemented: neighbour lists
  (`pairs.rs:9-15`) hold position, distance and extent, no velocity. A policy can still
  infer approach from its own memory of sector changes. A velocity cue is a listed
  extension (§7), not a v1 promise.
- **Crowd pressure is a real signal.** It is the overlap term the world already computes
  from actual neighbour positions; it is not a contact solver and it is not labelled one.
  Blocked-motion feedback is *not* offered: nothing in the world blocks motion.
- **Dropped:** hunger memory (a controller artefact; the GRU has its own memory), the OU
  noise draw (variability comes from weights, bodies and experience; seeded policy noise is
  an extension), gut fullness (ordinary bodies have no gut; it enters with the apex
  extension), genome traits as inputs (constant per individual; the weights are per
  individual and inherit with the body), moisture and weather (not sensed by any organism
  today; `light` and `water` are the supported habitat cues).
- **Absent versus low.** An absent sector cell is inedible exactly as an empty one is, and
  the world's edge is already signalled by `height` and `up`, so food sectors use 0 for both.
  For bodies, `rel_size` is conditional on `presence > 0`, the standard occupancy-gated
  attribute. Feedback values are 0 when no interval has completed yet (birth tick).

## 2. Spatial sampler

**Sectors.** Six fixed sectors of 60°, sector 0 centred on the heading, numbered
clockwise in the body frame: centres at `θ_k = k·60°`. Each source at body-relative
angle `θ` contributes to sector `k` with the raised-cosine weight

```
w_k(θ) = ½ (1 + cos(π · Δ_k / 60°))   for |Δ_k| < 60°, else 0,   Δ_k = wrap(θ − θ_k)
```

Adjacent windows overlap by half, so `Σ_k w_k(θ) = 1` for every `θ`: a source is never
lost or double counted, and a small turn moves weight smoothly between two sectors
instead of swapping a sorted list.

**Food rings.** From the observer's cell, the BFS rings the world already precomputes
(`sense_rings`, `lifecycle.rs:273-297`: graph distance exactly 1, 2, 3; seams included;
never across the open rim; each cell appears once). The near ring is hop 1; the far
ring is hops 2 and 3 merged, truncated to the body's `hops`. Each cell `i` contributes
its channel stock `x_i` at the direction of its unfolded centre from the observer's
position (`unfold_with`, the same call the current sensor makes at
`step.rs:389-398`). Per sector and ring:

```
food[k][c] = min(1, (Σ_i w_k(θ_i) · x_i) / (P_max · Σ_i w_k(θ_i)))    if Σ_i w_k(θ_i) > 0, else 0
```

a weighted mean over the cells present, not a sum, so a sector's value does not grow
with how many cells happen to fall in it. Cells beyond `hops`, and cells that do not
exist past the rim, contribute nothing. Distance inside a ring is not weighted; ring
membership is the coarse range cue. Cells whose unfold fails or whose distance is
below `GRADIENT_EPS` are skipped, as now.

**Bodies.** From the observer's bounded neighbour list (nearest first, at most
`cfg.capacity.max_neighbors` = 16, positions already unfolded into the observer's
chart, each neighbour once):

```
presence[k] = max_n  w_k(θ_n) · max(0, 1 − d_n / r_sense)
rel_size[k] = extent_n* / (extent_self + extent_n*)   for the n* attaining that max, else 0
```

Ties for `n*` (equal weighted presence) resolve to the neighbour earlier in the list,
which is already ordered by `(distance, id)`, so the choice is deterministic across
runs and platforms.

`extent_n` is the neighbour's physical crowding extent, which for an apex member is the
profile's `body_extent_px`, not the artwork support. Disclosure: when more than 16
bodies are in range, the farthest are not sensed at all; that truncation is the world's
existing sensing budget, not a policy choice.

**Range and cost.** Sensing range is `r_sense`; the sensing bill `sense_cost · r_sense`
is already part of upkeep (`MotorBill::upkeep`) and does not change. The sampler adds
no new paid quantity.

**Occlusion.** None. Every cell within `hops` and every listed neighbour is sensed
regardless of bodies in between. This is an abstract local chemical/pressure sense
sampled from the fields, not a physical odour plume, a line of sight, or a hearing
model; diffusion or delay would be a separate environmental mechanism.

**Seams.** Directions and distances come from unfolded positions in the observer's
chart, then rotate into the body frame by the observer's heading in that chart. A
body straddling a seam therefore senses the same values it would on a flat unfolded
patch, and the contract test in §9 pins that.

**Implemented versus future.** Implemented now, by reuse: cell rings, unfold, neighbour
lists, extent, height, up, water, light. New but purely derived: sector weighting,
body-frame rotation, ring means. Future work, not v1: relative velocity of neighbours,
observable compatibility cues for mating (§7), any odour transport.

## 3. Actions v1: 7 channels

The head is linear (`y ∈ ℝ⁷`). Squashing, deadband and masks are actuator semantics,
identical for every controller that speaks this contract.

| Channel | `a_i` | Squash | Deadband | Mask (body capability) | World interpretation |
| --- | --- | --- | --- | --- | --- |
| 0 thrust | [0, 1] | `σ(y₀)` | `< 0.05 → 0` | always live | requested centre speed `v_req = a₀ · v_max / wading` px/s along the heading |
| 1 turn | [−1, 1] | `tanh(y₁)` | `|·| < 0.05 → 0` | always live | requested turn rate `ω_req = a₁ · ω_attain` rad/s, positive clockwise in the body frame; `ω_attain = min(ω_max, u_full / r)` |
| 2 graze | [0, 1] | `σ(y₂)` | `< 0.05 → 0` | `diet ≥ 0.05` | `Decision.graze_effort` |
| 3 fruit | [0, 1] | `σ(y₃)` | `< 0.05 → 0` | `diet ≥ 0.5` | `Decision.fruit_effort` |
| 4 scavenge | [0, 1] | `σ(y₄)` | `< 0.05 → 0` | `diet ≤ 0.95` (ordinary); `scavenge_fraction > 0` (apex member) | `Decision.scavenge_effort` |
| 5 attack | [0, 1] | `σ(y₅)` | `< 0.5 → 0` (level trigger) | apex member with `attacks_enabled` | request to enter the paid windup/strike; §7 |
| 6 reproduce | [0, 1] | `σ(y₆)` | `< 0.5 → 0` (level trigger) | body whose lifecycle reproduces (all ordinary bodies; adult apex) | ordinary: `Decision.bud`; apex: mating consent, §7 |

**Locomotion activation.** `active = (a₀ > 0 ∨ a₁ ≠ 0)` after deadband. `Decision.effort`
is 1 when active and 0 otherwise. Capability is a property of the body, not of how
much of it the policy uses: with `active`, the body's full translational capability
`v_max / wading` (before the energy cap) is available to be *split* between `v_req` and
`r·|ω_req|` by the resolver; with `¬active`, capability is 0 and the body is still. This
is the distinction the brief asks for: requested translation (`a₀`) is separate from
activation (`active`), and there is no second effort field that could add rotational
allowance. Confirmed against R0b: `Decision.effort` is the only policy-side term in
`speed_cap`, `capability()` is `speed_cap` alone, and no Mode label enters it.

**Why the turn request is scaled to `ω_attain`, not `ω_max`.** Under the shared budget
`r · ω_max` is 3.9 px/s for a unit adult against a 0.3 px/s budget, so a request in
genome units would saturate at `|a₁| ≈ 0.08` and the rest of the channel would be dead
range. Scaling to the attainable pivot rate makes `(a₀, a₁)` close to a direct split of the
budget. The requested magnitude, stated exactly, is

```
demand = a₀ · v_max / wading  +  r · |a₁| · min(ω_max, u_full / r)
```

and the resolver scales both terms by `u / demand` whenever `demand > u`. Two regimes
matter and both must be tested: with ample energy and a small body (current pace,
`ω_max` binds) the second term is `r·|a₁|·ω_max < u_full`, so demand stays inside the
budget even at `a₀ + |a₁| > 1` for moderate turns; with low energy `u_full` falls
below `v_max / wading`, so full thrust alone already exceeds the budget and any turn
is scaled with it. `a₀ + |a₁| > 1` is therefore not the universal scaling condition;
the resolver's own comparison is. Requested and resolved quantities stay distinct.
`ω_attain` is recomputed every tick from the body's current `r` and `u_full`, so growth,
wading and low energy all pass through it honestly; it grants nothing the resolver
would not.

**Per-tick request from a held action.** `MotorRequest` takes a *target orientation*,
not a signed rate (R0b note), so at every world tick the adapter builds the request
from the *current* heading: `heading_req = rotate(heading, ω_req · dt)`, `speed = v_req`. The resolver (`motor::resolve`) clamps the turn to `ω_max·dt`, the speed
to the ceiling, then scales both by one common factor so that
`|v| + r·|ω| ≤ u`. A held turn therefore keeps turning at the requested rate across the
two ticks rather than chasing a fixed bearing, and every tick pays for what was
actually resolved (`MotorBill::total_cost`). Nothing in the adapter assigns a heading.

**Supported directions.** Forward translation along the heading and signed rotation.
Reverse and lateral translation are **deferred**, not silently dead: the resolver moves
along the heading only (`MotorRequest` has no lateral term), the rigs are drawn along
the heading, and the pair pass and capture geometry assume forward-facing bodies.
Adding them means a resolver extension with its own morphological ceiling and art
support; that is a versioned action-schema change (§7), and until then the schema
simply has no such channel rather than a masked one.

**Consumption shares one mouth.** Before they reach the settlement, the three intake
efforts are normalised: `s = a₂ + a₃ + a₄; if s > 1 then a_i ← a_i / s`. With the
existing law `bite = rate · effort · dt · X/(X + K_P)` (`step.rs:1290-1300`), grazing and
fruit then together take at most `graze_rate · dt` and scavenging at most
`scavenge_rate · dt`, so total handling never exceeds one mouth-tick. The world's
fruit-first headroom order and the type-II term are unchanged. No `feed_min` gate
applies to a neural body: a poor cell is poor food through `K_P`, not through a
behavioural threshold (R0b arm 2 measures exactly this).

**Simultaneous spending.** Movement is paid from `energy` after upkeep, through the
resolver's budget; intake removes material from the cell and adds to reserve through
assimilation. The two draw on different pools and neither is free: a moving mouth
still pays its motion. Digestion, oxidation, maintenance, growth and ageing are
mandatory physiology with no channel.

**Cadence.** The policy updates at controller ticks (§5); actions are held between
them. Attack and reproduce are *levels*: the world samples them each tick under its own
cooldown/contact/funding rules and can never be retriggered faster than those rules
allow; a held high level is a standing request, not a stream of attempts.

## 4. Pure pivot and the R0b envelope

The resolver already treats pivot as legal at zero speed (`motor.rs:191-250`; test
`pure_pivot_is_legal_and_bounded_by_the_budget_over_the_radius`). What R0a got wrong was
`MotorLimits::capability = speed_cap + REFERENCE_RADIUS_PX · turn_rate_max`
(`motor.rs:137-141`): a union of two ceilings that R0b removes. Under this contract:

- **Physical availability** `u_full = min(v_max / wading, affordable_motor)` is what the
  body could spend this tick at full activation. It is reported to the policy as
  `motor_avail` (index 63).
- **Actual resolution** takes the policy's split `(v_req, ω_req)` and returns
  `(v, ω)` with `|v| + r·|ω| ≤ u`, `|ω| ≤ ω_max`, `v ≤ v_req`, same sign as requested.
- Availability is never derived from the translation that happened: a body that resolved
  `v = 0` while pivoting still had `u_full` available and spent it on sweep.

Worked examples for a unit adult with R0b's shipped semantics at the **historical
R0b pace** (`v_max = 0.3 px/s`; the current default after R0d is 5.0 px/s = 1 BL/s, see
the second table): `v_max = 0.3 px/s`,
`r = 2.5 px`, `ω_max = π/2 rad/s`, `wading = 1`, `move_cost = 0.006`, `S = 1`, rotation
price `k = 0.5`, `dt = 0.05`. With ample energy `u_full = 0.30` and
`ω_attain = min(1.571, 0.30/2.5) = 0.120 rad/s` (6.9°/s).

| Action `(a₀, a₁)` | `active` | `u` | Request `(v_req, ω_req)` | Demand `v + r·ω` | Resolved `(v, ω)` | Bill per tick (e) |
| --- | --- | --- | --- | --- | --- | --- |
| (0, 0) zero | no | 0 | (0, 0) | 0 | (0, 0): still | upkeep only |
| (1, 0) pure translation | yes | 0.30 | (0.30, 0) | 0.30 | (0.30, 0) | `0.006·0.30·0.05 = 9.0e-5` |
| (0, 1) pure pivot | yes | 0.30 | (0, 0.120) | 0.30 | (0, 0.120 rad/s = 6.9°/s) | `0.006·(0.5·0.30)·0.05 = 4.5e-5` |
| (0.5, 0.5) combined | yes | 0.30 | (0.15, 0.060) | 0.30 | (0.15, 0.060 rad/s = 3.4°/s), unscaled | `0.006·(0.15 + 0.5·0.15)·0.05 = 6.75e-5` |
| (1, 1) both full | yes | 0.30 | (0.30, 0.120) | 0.60 | scale 0.5 → (0.15, 0.060) | `6.75e-5` |
| (1, 0) with 0.10 px/s affordable | yes | 0.10 | (0.30, 0) | 0.30 | (0.10, 0) | `3.0e-5` |
| (0, 1) with 0.10 px/s affordable | yes | 0.10 | (0, 0.040) | 0.10 | (0, 0.040 rad/s = 2.3°/s) | `1.5e-5` |
| (0, 1) energy below upkeep | yes | 0 | (0, 0) | 0 | (0, 0): still, unpaid | upkeep to exhaustion |

At the **current default pace** (R0d, `v_max = 5.0 px/s`, `move_cost = 0.00036`, same
body): `u_full = 5.0`, `u_full / r = 2.0 rad/s`, so `ω_attain = ω_max = 1.571 rad/s`
(90°/s) and the genome ceiling binds.

| Action `(a₀, a₁)` | `u` | Request `(v_req, ω_req)` | Demand | Resolved `(v, ω)` | Bill per tick (e) |
| --- | --- | --- | --- | --- | --- |
| (1, 0) | 5.0 | (5.0, 0) | 5.0 | (5.0, 0) | `0.00036·5.0·0.05 = 9.0e-5` |
| (0, 1) pure pivot | 5.0 | (0, 1.571) | 3.93 | (0, 1.571 = 90°/s), unscaled | `0.00036·(0.5·3.93)·0.05 = 3.5e-5` |
| (1, 1) both full | 5.0 | (5.0, 1.571) | 8.93 | scale 0.56 → (2.80, 0.880 = 50°/s) | `0.00036·(2.80 + 0.5·2.20)·0.05 = 7.0e-5` |
| (1, 0.5) | 5.0 | (5.0, 0.785) | 6.96 | scale 0.72 → (3.59, 0.564) | |
| (1, 0) with 1.0 px/s affordable | 1.0 | (5.0, 0) | 5.0 | (1.0, 0) | `1.8e-5` |
| (0, 1) with 1.0 px/s affordable | 1.0 | (0, 0.4) | 1.0 | (0, 0.4 = 23°/s) | `0.9e-5` |

Read plainly: `(a₀, a₁)` is a split of one budget, a pure pivot at the full budget
costs half of full travel per px of magnitude, and every radian of turning gives up
`r` px of travel. Historical R0b consequences (6.88°/s unit-adult pivot, 13 s per cell)
were what prompted the R0d recalibration; at 1 BL/s a unit adult turns at its genome
ceiling and an adult lanternjaw at about 16°/s (65°/s during a paid strike).

## 5. Recurrence state, cadence and persistence

**Network (starting proposal, no sweep).** One GRU layer, `H = 32`, input `I = 70`,
linear head `O = 7`, `f64`. Reset-after convention with PyTorch gate order `(r, z, n)`
and two bias vectors per gate:

```
r = σ(W_ir x + b_ir + W_hr h + b_hr)
z = σ(W_iz x + b_iz + W_hz h + b_hz)
n = tanh(W_in x + b_in + r ⊙ (W_hn h + b_hn))
h' = z ⊙ h + (1 − z) ⊙ n
y  = W_o h' + b_o
```

Tensor layout: `W_i: [3H × I]`, `W_h: [3H × H]`, `b_i, b_h: [3H]`, rows in gate order,
row-major, little-endian `f64`. `W_o: [O × H]`, `b_o: [O]`. Loop order is fixed: for
each animal in slot order, gates in the order above, no reordering by the trainer.
Initialisation (a Cubarium choice, not an imported result): `b_hz` drawn so that
`σ(b_hz)` spans retention timescales of 1–100 updates, all weights small and
non-saturating; verified by the two-history check, not assumed.

**Cadence.** `phase ∈ {0, 1}` per animal. The controller runs on ticks where
`(tick + phase) % 2 == 0`, so the population's inference load is split across both
ticks and each animal sees `Δt_c = 0.1 s`. Feedback (indices 64–69) accumulates
every tick from that tick's resolved motion and settled intake, and is read then
zeroed at the animal's next controller tick, *before* that tick's motion is resolved.
The first interval after birth may hold fewer than two ticks (`feedback.ticks` says
how many); its means divide by the ticks actually accumulated, and an interval with
zero ticks reads as zero intake, zero motion and `delivered = 1`. Motor resolution,
costs, intake and physiology run every tick as now. `motor_avail` is sampled at the
controller tick from the same `affordable_motor` call the motor stage uses that tick,
in tick order, never from a duplicated approximate bill; a zero-capacity or
zero-rate case encodes as 0, never NaN.

**Per-animal mutable state** (exact values, all persisted):

| Field | Type | Reset | Notes |
| --- | --- | --- | --- |
| `hidden` | `[f64; 32]` | zero at birth | never reset at seams, meals, frames or save/load |
| `held` | `[f64; 7]` | zero at birth | the post-squash, post-mask action in force |
| `feedback` | `{ate: [f64;3], speed_sum, turn_sum, req_mag, res_mag, ticks: u32}` | zero at birth and after each controller tick | material and px/s sums, not normalised until read |
| `phase` | `u8` | birth tick parity | fixed for life |
| `policy` | index into the world's policy table | inherited | see below |

**Policy table.** `WorldState` gains one appended extension, `NeuralState`, in the same
style as care/hunter/quiet/apex (`world/state.rs:22-82`): a version, a `Vec<Policy>` of
exact weights (each with its `schema_digest`), and a sorted `Vec<(OrganismId, AnimalState)>`.
Identical weights are stored once and referenced; hidden state is never shared.
The snapshot schema advances by one from whatever is current when the extension is
added (15 if nothing else has moved), with the previous schema frozen as a mirror
decoder per repository practice; decoding the previous schema yields an empty
extension, so **every existing world loads with every organism legacy-controlled**,
and continues on the same trajectory (tests compare trajectories and state hashes,
not serialized bytes across formats). The digest is computed over exactly serialized
bytes of the canonical schema text and field lists, and `validate` checks every
weight and state value is finite and every policy reference resolves.
An animal is neural iff it has an entry; the world's step dispatches on that per
animal. Migration of a legacy world is an explicit development command that inserts
entries with chosen policies; loading never does it.

**Identity and slot reuse.** Entries are keyed by the full `OrganismId` (slot +
generation, `ids.rs:8-11`), so a reused slot cannot inherit hidden state. Death removes
the entry in the same boundary that removes the organism. Birth inserts a fresh entry
with zero `hidden`/`held`/`feedback`, `phase` from the birth tick, and the policy chosen
by the inheritance rule (§8).

**Seams.** Nothing to transport: `hidden`, `held` and `feedback` are chart-free. The
world transports `heading` and `ou` as now; the per-tick request is rebuilt from the
transported heading. Physical turn feedback uses `ResolvedMotion.turn`, which excludes
transport by construction.

**Dormancy.** A dormant apex (`dormancy.rs`) receives no observation and runs no
controller tick; `held` is forced to zero for the duration, `hidden` and `phase` are kept
untouched, and emergence resumes from that state. This is a physiological exception
owned by the world and is listed as such in §6.

**Save/resume.** Same-build uninterrupted versus save-then-resume must agree on
`state_hash` at every later tick (`snapshot.rs:201-203`); the existing determinism
suite pattern applies. No claim of cross-platform bitwise equality.

**Versioning.** One `schema_digest: u64` = FNV-1a over the canonical text
`"cub-obs-1|cub-act-1|gru32-reset-after-1|motor:r0b-99a2bfc|10hz"` plus the field
list of §1 and §3; the motor id names the commit whose `resolve` semantics the action
adapter targets. It is stored in every
`Policy` and in the extension header. On load, a policy whose digest differs from the
build's is refused by name (as `SUPPORTED_PROFILE_VERSIONS` does for hunters), never
reinterpreted. Boundaries: changing the observation layout, action set, GRU
convention, rate or motor contract requires retraining; changing world coefficients
(speed, costs, `K_P`) does not change the digest but does invalidate trained founders
as development artefacts, which the result document must say.

## 6. Path to complete behavioural ownership

Legend: **N** = the policy decides through this contract; **W** = a physical or
physiological rule the world keeps; **R** = a legacy behavioural rule that is removed
for neural animals (it must not silently steer them); **later** = needs a listed
extension (§7).

| Behaviour | Today | Under this contract |
| --- | --- | --- |
| Mode hysteresis `seek_on/seek_off`, hunger memory (`controller.rs:165-262`) | controller | **R**; N through `hidden` |
| Steering weights, depth drive, crowd term, OU noise | controller | **R**; N from sectors, `up`, `crowd` |
| Turn gate by mode (`rest/feed_turn_fraction`) | controller | **R** |
| Movement effort by mode | controller | N (`thrust`, `turn`, activation) |
| Envelope `|v| + r·|ω| ≤ u`, wading, energy budget, bills | world | **W** (R0b) |
| `feed_min` cell threshold | controller | **R** |
| Diet gates (0.05 / 0.5 / 0.95) | controller | **W** as capability masks |
| Type-II intake, fruit-first headroom, per-cell proportional shares | world | **W** |
| Patch departure and rest | emergent from hysteresis + noise | N |
| Budding thresholds `bud_reserve/energy/min_age` | controller drives | N (`reproduce`); **W** keeps escrow funding, capacity, gestation; maturity gate see §10 |
| Post-birth quiet pause (`quiet.rs`) | opt-in override | **R** for neural animals; the policy rests if it wants to |
| Apex perch/seek by reserve hysteresis (`HunterPhase::Perched`) | hunter FSM | **later**: N |
| Apex target choice (nearest eligible), pursuit heading override (`step.rs:929-960`) | hunter FSM | **later**: N steers; **W** resolves contact against whatever body is inside the capture region. R0b showed the legacy override deadlocks under the shared budget (it holds at `rest_effort` while needing budget to face the prey: ≈ 0.05°/s); a neural apex must pay to align, which is the R3 approach problem, not a reason to restore free rotation |
| Prey eligibility by size, windup/strike/recovery timing, strike cost, capture probability | world | **W** |
| Handling, digestion, gut capacity, meal recovery | world | **W** |
| Prey escape turn and dash (`step.rs:1000-1024`) | world override on sensed threat | **later**: N from `body` sectors and memory; no automatic dash |
| Apex mating pairing (nearest two perched adults, `step.rs:481-566`) | world rule | **later**: N consent, §7 |
| Gene averaging at mating (`encounter.rs:349-392`) | world rule | replaced by same-parent inheritance, §8 |
| Combat retreat / retaliation / injury | world rule on encounter | **later**: injury **W**; retreat is ordinary movement, N |
| Dormancy entry, sustain cost, emergence checks (`dormancy.rs`) | world rule | **W**, explicit physiological exception |
| Oxidation, maintenance, growth, ageing, death | world | **W** |

R3's ownership audit walks this table: every row marked **later** must by then be N
or an explicitly listed W; unlisted heuristics mean the claim of neural control is
false.

## 7. Extensions (versioned, not v1)

Each bumps the schema digest and retrains founders.

| Extension | Adds | Why not v1 |
| --- | --- | --- |
| **Apex internal state** | inputs: `gut = carried / gut_capacity`, `handling` {0,1}, `strike_ready` {0,1} (cooldown and affordability as one physiological bit) | dead for every non-apex body |
| **Attack action semantics** | `attack ≥ 0.5` while a size-eligible body lies inside the capture region enters the paid windup → strike → settlement sequence; the world picks the body inside the region nearest the capture offset as a deterministic handle. Steering into position is the policy's job, so the handle never chooses *for* it. | needs the apex input extension and R1 runtime |
| **Mate choice and refusal** | inputs per sector: `compat` (observable: same `form`, adult, not gestating, from the neighbour record; **not** energy or genome) and `rel_size`. Consent rule: A consents to B iff `reproduce ≥ 0.5` and B is A's forward-most listed neighbour within `MATING_RADIUS_PX` (smallest `|θ|`). Mating starts iff consent is mutual on the same tick pair. Refusal is expressed by facing away, moving away or a low `reproduce`; a global readiness bit of two nearby animals is insufficient by construction. | needs `compat` cue and apex extension |
| **Relative motion cue** | per sector `approach` = d(distance)/dt of the presence-giving body, from two consecutive neighbour records keyed by id inside the sampler (ids stay internal) | not present in neighbour lists today; memory can partly substitute |
| **Reverse / lateral thrust** | one signed fore/aft channel and one lateral channel with their own morphological ceilings inside the same `|v| + r·|ω| ≤ u` | resolver, rigs and capture geometry are forward-only |
| **Seeded policy noise** | one standard-normal input per controller tick from `Stream::OrganismTurn` | v1 variability from weights/bodies/experience first |
| **Neural dormancy** | would need a sensing/update/energy contract while concealed | plan: rule-driven exception stays |

## 8. Inheritance and evaluation

**Starting proposal: same-parent body and policy.** Ordinary births copy the parent's
genome with sparse mutation (`step.rs:2098`) and, under this contract, the parent's
policy with weight mutation. Two-parent apex births choose one parent from the recorded
birth RNG and take *both* body genome and policy from it; `encounter::recombine`'s gene
averaging is not used for neural lineages, because a policy co-adapted to one body
should not be paired with an average of two. Both biological parents remain in
`PairedParentage`. Hunter lineages, whose genome is copied exactly today, get
mutation enabled when policies become heritable, or there is nothing to select.

Remaining choices, with the preferred option first:

- Weight mutation: per-weight Gaussian with probability `p_w` and step `σ_w` scaled per
  tensor, recorded as a count and a seed in the birth event (preferred); versus
  whole-tensor perturbation.
- Compatibility: policies are compatible iff their digest matches; a mismatched pair
  cannot mate (preferred, simple); versus allowing it and taking the carrier's policy.
- Mutation of `phase`: never; it is not heritable.

**Evaluation is lineage-based and lifecycle-derived.** Reproducing descendants are the
required evidence later. The horizon for a diet is at least three generations of *that*
diet's lifecycle: ordinary bodies `gestation 30 s + bud_min_age 120 s + growth to adult`,
on the order of 1,000 s; apex bodies from the profile's `reproduce_min_age_seconds`,
`reproduce_interval_seconds` and `gestation_seconds`, which are longer and must be read
from the profile in force, not assumed equal to budding. Measures: descendants that
themselves reproduced, survival of the founder to its first reproduction, intake per
energy spent, distinct ground reached and turning sweep; never lineage size alone, and
never terminal stores.

## 9. Next implementation slice (R1a: sampler, action adapter, memory check)

No training, no optimizer selection, no live migration. Boundaries:

| Step | Where | Delivers | Depends on |
| --- | --- | --- | --- |
| 1 | `crates/cubarium-core/src/neural/obs.rs` (new) | `Observation70` builder from the existing per-tick inputs: cell rings, unfolds, neighbour list, fields, organism | R0b merged; `motor_avail` needs Opus's `u_full` |
| 2 | `crates/cubarium-core/src/neural/action.rs` (new) | `Action7` → `Decision` adapter: squash, deadband, masks, mouth normalisation, per-tick `MotorRequest` from held action | R0b's final `Decision`/`MotorLimits` shape |
| 3 | `crates/cubarium-core/src/neural/gru.rs` (new) | GRU32 forward, tensor layout, digest; independent numerical reference test with hand-computed values | none |
| 4 | `world/step.rs` observe/decide stage | per-animal dispatch: legacy controller or (`obs` → GRU at cadence → held `action` → adapter) | 1–3 |
| 5 | `world/state.rs`, `snapshot.rs`, `snapshot/v14.rs` (frozen mirror) | `NeuralState` extension, schema 15, empty-extension decode of 14, `validate` | 4 |
| 6 | `crates/cubarium-core/examples/` | one development probe that runs a fixed action tape (the §4 table) and a fixed hand-authored policy through the real step and prints resolved motion, bills and intake | 4 |

Fixtures that must exist before step 4 is called done:

- **Sampler geometry.** Partition of unity of `w_k`; a lone food cell dead ahead lands in
  sector 0 only; rotating the observer by 60° shifts every sector by one index exactly;
  a body straddling a seam senses the same *food and body* values as a physically
  equivalent flat layout with the same stocks and neighbours, and its body frame is the
  transported one (habitat inputs such as `light`, `height` and `up` legitimately differ
  across faces and are not asserted equal); a 1-hop body has an all-zero far ring;
  presence decays to 0 at `r_sense`; 17 neighbours disclose the truncation.
- **Action adapter.** The six §4 rows through `motor::resolve` with R0b's limits; mouth
  normalisation caps total handling at one mouth-tick; masked channels never reach the
  `Decision`; deadband gives exact stillness and a zero bill beyond upkeep; a held turn
  across a seam keeps turning by the same signed amount.
- **Two-history memory check.** One fixed hand-authored weight set; two input sequences
  that end in the identical observation vector but differ earlier; the held actions
  after the final step differ by more than a stated tolerance, and after a hidden reset
  they do not. This proves the interface can carry memory; it proves nothing about
  learning.
- **Persistence.** Uninterrupted versus resumed runs agree on `state_hash`; a schema-14
  world loads all-legacy and steps identically; slot reuse after a death starts from
  zero hidden state; a policy with a foreign digest is refused by name.

Completion criteria: every fixture above green; the development probe shows pivot,
travel, rest and continuous grazing through the real world; R1 throughput screen from
the plan (32 and 128 bodies × 2,000 ticks, 512 × 200, ≤ 60 s wall) reports sensor
construction, inference and total ticks/s separately. Only then are horizons and
compute chosen.

**What R0b measured** (`examples/mobile_grazing.rs`, one unit adult, matched 3×3
patch, 1,800 s arms, 5.1 s wall; details and limitations in the
[R0b result](7_Research/r0b-motor-foraging-result-2026-09-14.md)):

| Quantity | Measured | Bearing on this contract |
| --- | --- | --- |
| No-intake survival from standard stores | dies at 350 s | any R2 episode must run well past 350 s or it measures starting stores |
| Legacy-gated stationary grazer | dies at 460 s, ate 0.40 m | the behaviour the neural policy must beat |
| Mobile scripted grazer, first ring tour | 538 s, ate 2.79 m, reserve at ceiling | a profitable cycle exists at native pace; one tour alone exceeds the plan's provisional 600 s |
| Later tours | 133 s each, yield falling to 0.09 m by tour 6 | the patch, not the motor, is the limit: 3.39 → 0.60 m over 1,800 s against 7.05 m gross renewal |
| Motor share of the bill | 1.24 e of 12.4 e (10%) | at this pace standing still is the expensive act; `thrust` is cheap to learn |
| Travel between neighbouring cells | ≈ 13 s per 4 px cell at full budget | sets the minimum useful episode: depletion + several transitions ≈ 1,000 s |
| Ungated continuous request | strips cells to ~1e-3 m and below; the body goes broke but never dies because an infinitesimal bite lands before the death check | the starvation predicate / cropping floor must be settled before the R2 fixture, and until then fixtures must count `broke`, not `died` |

From the R1 screen: ticks/s at 32/128/512 bodies with and without inference. The R2
episode length must exceed depletion plus travel and outlast no-intake survival, so
the plan's provisional 600 s is too short by these numbers and at least 1,000–1,800 s
is the working assumption to confirm at the R2 checkpoint. The old M1 throughput is not
a benchmark for any of it.

## 10. Open decisions and resolved dependencies

Decisions for Wrysk, preferred option first.

1. **Pace.** Decided in principle by Wrysk on 2026-09-14: the display is real space at
   an unspecified scale, so pace is calibrated in **body lengths per second of the unit
   adult**, never in pixels, at about 1 BL/s cruise (today's 0.3 px/s is 0.06 BL/s, snail
   pace). The [R0d brief](handoffs/r0d-pace-opus-2026-09-14.md) applies it. At 1 BL/s the
   shared budget gives a unit adult ≈ 115°/s of pivot, so the genome ceiling binds again
   and `ω_attain` mostly equals `ω_max` for small bodies; the contract is unchanged
   because `ω_attain` and `motor_avail` follow whatever the world sets. Still open: the
   energy rebalance that the faster pace forces, deferred to the headless ecology search.
2. **Resolver scaling rule.** Opus kept the single common factor (`v = u²/demand` when
   over budget) and flagged that a translation-priority rule would be a redesign.
   Preferred: keep the common factor; this contract's `ω_attain` scaling already removes
   the dead range that made it bite, so the policy's split is honoured up to the budget.
3. **Starvation predicate and cropping floor.** R0b showed an ungated mouth keeps a
   broke body alive on infinitesimal bites. Preferred: settle this as one bounded
   world-rule change (a minimum harvestable stock, or evaluating death before intake)
   before the R2 fixture exists; it is a prerequisite, not part of this interface.
4. **Activation rule.** Preferred: binary activation (any motor channel above deadband
   gives the body its full capability; the request sets the split). Alternative:
   graded `max(a₀, |a₁|)` scaling of capability, which makes gentle turns slower still.
5. **Maturity gate for `reproduce`.** Preferred: the world requires `S ≥ S_adult` and
   funding; `bud_min_age` is dropped as a behavioural drive. Alternative: keep the age
   gate as physiology in v1 for continuity with the current lifecycle.
6. **Fruit sectors.** Preferred: keep (70 inputs). Fallback if throughput forces it: 58.
7. **Bursts for neural bodies.** Preferred: none in v1; the strike burst remains a paid
   world grant tied to the attack extension's strike phase. No escape dash.
8. **Quiet pause and care controls.** Preferred: `QuietState::validate` excludes worlds
   with neural animals until an explicit contract exists, as it already does for hunters.

**Resolved from R0b** (no longer conditional): `u = min(speed_cap, affordable_motor)`
with `effort` the only policy-side term (§0, §3); `MotorRequest` is a target heading, so
the adapter rotates the current heading by `ω_req·dt` each tick (§3); the radius table
(§0) and the worked examples (§4); the motor id in the digest (§5); the measured
survival, depletion, travel and cycle-yield bounds (§9); wading and bursts act on the
whole budget (§1 index 63, §3). Legacy fixture changes in R0b (widened windows,
re-anchored continuation oracles) do not constrain the adapter's tests.

**Readiness.** This contract is ready for the R1a implementation slice in §9, subject
to decisions 1–3 above being taken at the checkpoint. Nothing in it is canon, and it
does not authorise R1a, training or a world change.

## Usage

Measured token usage for this document: unavailable in this harness. It was drafted
from Graft skeletons and targeted spans and finalised against the R0b report; no
simulation was run for it.
