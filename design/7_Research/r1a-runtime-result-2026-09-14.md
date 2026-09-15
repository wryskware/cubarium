---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R1a: the recurrent runtime, as built — 2026-09-14

Evidence for Fable against [the R1a brief](../handoffs/r1a-fable-runtime-2026-09-14.md) and
[the interface contract](../recurrent-interface-contract.md) §9, plus Wrysk's step-zero
override. This decides nothing. It records what was implemented, what was measured, which
contract text the implementation contradicts, and what is still missing.

## 0. Step zero: the starvation predicate

Wrysk's override, taken before any neural work and shipped on its own commit (`6b9e255`).

**Old rule.** `energy <= 0 && reserve <= 0`, evaluated in the physiology pass, *after* the
feeding settlement. Reserve decays geometrically and oxidation never empties it exactly, so a
broke body standing on a stripped cell took an infinitesimal bite every tick and never died.
R0b found live bodies at `energy = 5.5e-57`.

**New rule.** In the motor stage, **before** intake settles, a body dies of `Starvation` when

```
Organism::raisable_energy(cfg, dt) < MotorBill::upkeep(dt)
```

where `raisable_energy` is `energy + min(e_r · min(oxidation_rate·dt, reserve) · η_ox,
E_max − energy)`: what it holds plus everything one tick of oxidation could convert, under the
world's own rate, density, efficiency and headroom. The exact-zero test is gone. A body that
fails the test also has `affordable_motor == 0`, so it holds still on its last tick through the
existing envelope, with no special case. The predicate is recorded per slot in the motor stage
and applied in the physiology pass, so no food arriving later in the tick can reverse a body
that already failed to pay for being alive.

**One deliberate choice.** The predicate does **not** apply the oxidation *threshold*
(`org_cfg.oxidation_threshold`, or a member profile's). The threshold decides when a healthy
body tops itself up; gating on it would kill bodies the physiology would have refuelled on the
next tick. Rate, density, efficiency and headroom are applied exactly.

**Measured** with the unchanged `examples/mobile_grazing` fixture (four arms, 36,000 ticks):

| arm | before (R0d) | after | change |
| --- | --- | --- | --- |
| `gated-still` | died 460 s (tick 9190) | died 460 s (tick 9190) | none |
| `open-still` | **broke at 9278, alive at 1,800 s** | **died 464 s (tick 9279)** | the trickle phase is gone |
| `mobile` | funded at 1,800 s | funded at 1,800 s | none |
| `no-intake` | died 369 s (tick 7384) | died 369 s (tick 7384) | none |

Only the arm that the defect was about moved, and it moved by the whole trickle phase: one
tick after it first could not pay, instead of never. The three arms whose stores reach zero on
the same tick they become unpayable are unchanged, which is why **no legacy starvation window
needed re-recording** anywhere in the suite.

**Two new tests** (`crates/cubarium-core/tests/starvation.rs`): a body on an epsilon-stock cell
(`P = 1e-9`, reserve 0, energy half of upkeep) dies with cause `Starvation` on the next tick;
a body with **zero** energy and enough reserve for one tick of oxidation survives and pays its
upkeep out of the reserve.

**Two changed assertions.** `tests/apex_encounters.rs`'s carrier-death and partner-death
fixtures zeroed both stores in a world configured with `maintenance = move_cost = sense_cost =
0`. With living free, an empty body owes nothing and honestly does not starve. Each fixture now
gives its own doomed body a nonzero `phenotype.maintenance` — the organism's own field, not the
world config — so the world config and every accounting assertion in those tests are untouched.

## 1. What was implemented

| Contract §9 step | Where | State |
| --- | --- | --- |
| 1 sampler | `crates/cubarium-core/src/neural/obs.rs` | done |
| 2 action adapter | `crates/cubarium-core/src/neural/action.rs` | done |
| 3 GRU32 | `crates/cubarium-core/src/neural/gru.rs` | done |
| 4 per-animal dispatch | `world/step.rs` stages 5, 5c, 6, 7, 9 | done |
| 5 persistence | `world/state.rs`, `snapshot.rs`, `snapshot/v14.rs` | done, schema 15 |
| 6 probe | `crates/cubarium-core/examples/neural_probe.rs` | done, with the throughput screen |

Plus `neural/state.rs` (`Feedback`, `AnimalState`, `NeuralState`) and
`World::attach_neural_policy`, the single explicit door into the extension.

**All 70 inputs are present**, fruit sectors included. Nothing was cut; the throughput screen
(§4) did not force the question.

**The sampler is pure.** It takes already-unfolded offsets, the bounded neighbour list and the
organism's scalars, and returns the vector. It never reads the world, draws from the RNG or
allocates per animal (the world reuses two scratch vectors across the whole stage). That is
what makes the geometry testable without a world, and it is also why a seam is invisible to it:
the only chart operation in the module is the rotation out of the observer's own chart into the
body frame.

**Dispatch is by absence, not by suppression.** A neural animal never reaches
`decide_quiet`, so mode hysteresis, hunger memory, steering weights, the OU draw, the turn gate
and `feed_min` are not executed rather than overridden. The two downstream paths that could
still have steered an ordinary body were inspected and handled explicitly:

- **Prey escape/dash** (`step.rs` stage 5c) skips neural prey by id. Its own sensing still
  reports the pursuer through the body sectors; what is removed is the automatic turn and the
  free speed boost.
- **Post-birth quiet pause** (`quiet.rs`) is refused in combination: `WorldState::validate` and
  `attach_neural_policy` both reject a world with neural animals and an enabled quiet policy,
  by name. Ordinary care controls are untouched.
- **Apex membership** is refused the same way: the §7 apex sensory and action extensions do not
  exist, so a member cannot speak this interface.

Everything the world does physically still applies to a neural body: the `|v| + r·|ω| ≤ u`
envelope, wading, the bills, capacity, contact and damage, the type-II intake law with its
fruit-first headroom and per-cell proportional shares, oxidation, growth, gestation funding and
escrow, ageing and death.

**Lifecycle rules are retained.** Age gates, funding and gestation are unchanged; a
`reproduce ≥ 0.5` level is a standing request the world funds under its own rules. A neural
offspring copies the parent's policy index with **fresh** private state. No weight mutation,
no two-parent neural mating, no change to biological inheritance.

## 2. Deviations from the brief and the contract

1. **`Decision` gained one field.** The contract separates requested translation (`a₀`) from
   activation, but `Decision` had no way to say so: `world/step.rs` built its `MotorRequest`
   with `speed: speed_cap`, i.e. always the whole capability. `Decision.speed_request:
   Option<f64>` is the seam. `None` is the legacy value and the only one the ordinary
   controller produces, so every legacy trajectory is bit-identical. `resolve` still clamps it
   to `speed_cap`, so it can only ask for *less* than the capability.
2. **Turn sign.** See the contract corrections below. The contract asks for a clockwise-positive
   `turn` channel and then reads feedback from `ResolvedMotion.turn`, which is
   counter-clockwise-positive. The adapter converts once.
3. **Capability masks also honour the world's mechanism switches.** A world with
   `mechanisms.grazing = false` masks graze and fruit; `scavenging = false` masks scavenge. The
   contract lists only the diet gates. This matches what the legacy controller does and what
   the settlement would otherwise refuse anyway.
4. **`motor_avail` and `ω_attain` share one calculation** taken at the observe stage from the
   energy the body holds before this tick's payment — which is the state stage 6 will price the
   move against, because nothing between the two stages touches an ordinary body's energy.
   Zero radius gives the genome ceiling alone; zero budget gives zero; a non-finite ceiling
   gives zero. No approximate energy bill is duplicated in the sampler.

## 3. Tests

| Command | Result |
| --- | --- |
| `cargo test -p cubarium-core --release` | 450 passed, 0 failed, 3 ignored |
| `cargo test --workspace --release` | 1,286 passed, 0 failed, 20 ignored |

New: `src/neural/**` unit tests (33), `tests/neural_runtime.rs` (13), `tests/starvation.rs` (7),
`tests/neural_geometry.rs` (3), `tests/neural_predation.rs` (1).

**Sampler.** Partition of unity over 720 bearings; a lone cell dead ahead lands in sector 0 and
nowhere else; rotating the observer 60° clockwise shifts every sector by exactly one index; a
sector is a mean and not a sum (one cell and three identical cells give the same value); a
1-hop body has an all-zero far ring; presence decays linearly to 0 at `r_sense`; `rel_size` is
the extent share and is gated on presence; and a hostile input (NaN stocks, infinite extents,
a 1e9 crowd vector, out-of-range feedback) still yields 70 finite values inside their ranges.

**Adapter.** The six §4 rows through the real `motor::resolve` at R0b's persisted pace,
including the two energy-capped rows and the below-upkeep row; a pure pivot with zero thrust
resolving to nonzero paid rotation; the deadband giving exact stillness and no bill beyond
upkeep; the true requested magnitude in both regimes (genome-bound at the R0d pace,
budget-bound at low energy); wading throttling the requested speed; zero radius, zero budget
and a non-finite ceiling all giving finite requests; a held turn turning by the same signed
amount on four consecutive ticks; mouth normalisation capping three saturated mouths at one
mouth-tick; masks for a pure grazer, a pure scavenger and a mechanisms-off world; attack masked
for every ordinary body; and the level triggers on either side of 0.5.

**GRU.** A hand-computed two-unit reset-after reference, written from the equations rather than
from the code, exercising the input path, a nonzero previous hidden state through both the
update gate and the reset-gated recurrent term, and a cross-unit recurrent weight (so reading
`W_h` down its columns would fail). A second fixture separates reset-after from reset-before
via a nonzero `b_hn`. Shape and finiteness validation, and the 10,215-parameter count.

**Memory.** Two four-step sequences ending on the *identical* observation vector and differing
earlier give held-action outputs 0.5 apart on the thrust channel; replaying only the shared
final step — the hidden-reset control — gives bit-identical outputs. This proves the interface
carries memory. It proves nothing about learning.

**Persistence.** A six-animal world with animals on **both** cadence phases and one body parked
on a seam: uninterrupted and save-then-resume agree on `state_hash` at every one of 120 ticks
after the save, and the snapshot round trip is lossless. A schema-14 payload, built by hand
from the frozen projection, decodes to an **empty** extension — every organism
legacy-controlled — hashes equal to the reference world, and then tracks it for 200 ticks. A
legacy-only world runs 300 ticks with an empty extension and still shows the legacy hunger
memory the contract removes for neural bodies. Death removes the entry at the boundary that
removes the body, and a reused slot is a different key with zero hidden state. A funded
ordinary birth by a neural parent produces a neural child with the parent's policy and fresh
private state. Four refusals: a foreign digest (by name, saying it cannot be reinterpreted), a
body that is not alive, an apex member, an enabled quiet policy.

## 4. The probe and the throughput screen

`cargo run -p cubarium-core --release --example neural_probe`. One unit adult at the R0d pace
(5 px/s, `r` 2.5 px) on a patch held at `P_max`, reserve deliberately at 20% so intake has
headroom. Each segment is 200 ticks (10 s) with a bias-only head, so the tape is fixed and only
the world's response varies.

| segment | px travelled | net turn (deg) | paid upkeep (e) | paid motion (e) | eaten P (m) | Δreserve (m) | Δenergy (e) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rest | 0.00 | 0.0 | 0.06200 | 0.00000 | 0.0000 | +0.0000 | −0.06200 |
| travel (thrust 1) | 49.98 | 0.0 | 0.06200 | 0.01799 | 0.0000 | +0.0000 | −0.07999 |
| pure pivot (turn +1) | 0.00 | 900.0 | 0.06200 | 0.00707 | 0.0000 | +0.0000 | −0.06907 |
| split (≈0.5, ≈0.5) | 25.00 | 450.5 | 0.06200 | 0.01254 | 0.0000 | +0.0000 | −0.07454 |
| graze in place | 0.00 | 0.0 | 0.06200 | 0.00000 | 0.2588 | +0.1553 | +0.00000 |
| travel and graze | 49.98 | 0.0 | 0.06200 | 0.01799 | 0.2649 | +0.1590 | +0.00000 |

**These numbers are the repaired ones; see "Repair cycle 1" below for what the first version
got wrong.** Intake is the world's own `IntakeDiagnostics` — material that actually left a
field through a mouth — and the motion column is the resolved motion priced through
`MotorBill::motor_cost`. Read plainly: a pure pivot turns 90°/s (the genome ceiling, which
binds at the new pace) with zero travel and pays 0.00707 e for the sweep; the split halves both
and costs between the two; grazing works standing still and while travelling, out of one mouth.
The probe asserts that a closed mouth records exactly zero intake and that travel-and-graze
records both a motor debit and an intake.

**Throughput.** Each arm run twice, all-legacy and all-neural. Births off.

| bodies | ticks | legacy ticks/s | neural ticks/s | world difference µs/body-tick |
| ---: | ---: | ---: | ---: | ---: |
| 32 | 2,000 | 13,899 | 8,203 | 1.561 |
| 128 | 2,000 | 6,578 | 2,832 | 1.571 |
| 512 | 200 | 1,032 | 547 | 1.680 |

**Component cost, measured at the calls** (added in repair cycle 1; the world difference above
is *not* an isolated measurement, because the two arms also differ in controller and
trajectory, so it subtracts the legacy controller work that was removed as well as adding the
neural work):

| bodies | sampler µs/call | inference µs/call | adapter µs/call | all three, µs/body-tick |
| ---: | ---: | ---: | ---: | ---: |
| 32 | 0.999 | 2.423 | 0.052 | 1.763 |
| 128 | 1.067 | 2.438 | 0.052 | 1.804 |
| 512 | 1.407 | 2.439 | 0.052 | 1.975 |

The sampler and the GRU run on the animal's controller tick (half the body-ticks); the adapter
runs on every one, because a held action is rebuilt from the current transported heading each
tick. Each region is bracketed by two `Instant::now()` calls, so a sub-microsecond column
carries a few percent of its own instrument. The summed component cost slightly *exceeds* the
world difference, which is the expected sign: the neural arm does not pay for the legacy
controller.

Whole screen: **2.0 s of wall time** against the 60 s budget. At 512 bodies an all-neural world
still runs 547 ticks/s, 27× the 20 Hz the display needs. **No channel was cut, and none needs
to be on this evidence.**

## 5. Contract corrections

Proposed replacement text, for Fable to apply. The implementation follows these, not the
current document.

### §1.1, index 68 (`turned`) — source column and a note

Replace the `Source` cell for index 68 with:

> `ResolvedMotion.turn`, **negated** (transport excluded)

and add, immediately after the §1.1 table:

> **Sign.** The `turn` action channel, the sector numbering and the body frame's `+y` all run
> **clockwise**. `ResolvedMotion.turn` is measured by `Vec2::screen_angle`, which increases
> **counter-clockwise** on screen. The two differ by a sign, and the adapter converts once
> (`neural::action::resolved_turn`) so the mismatch never reaches the policy: a positive
> `turned` means the body physically turned the way a positive `a₁` asks it to.

### §3, after "Per-tick request from a held action"

Add:

> **How the split reaches the resolver.** `Decision` carries the requested heading and an
> effort that the world turns into *both* the capability and the requested speed
> (`MotorRequest { heading: d.heading, speed: effort · v_max / wading }`). A policy that
> separates activation from requested translation therefore needs one more field:
> `Decision.speed_request: Option<f64>`, `None` for every legacy caller and
> `Some(a₀ · v_max / wading)` for a neural one. `resolve` still clamps it to `speed_cap`, so
> the field can only ask for less than the capability, never for more, and a legacy trajectory
> is bit-identical.

### §3, capability mask column

In the mask cells for channels 2, 3 and 4, replace the diet conditions with:

> `cfg.mechanisms.grazing ∧ diet ≥ 0.05` / `cfg.mechanisms.grazing ∧ diet ≥ 0.5` /
> `cfg.mechanisms.scavenging ∧ diet ≤ 0.95` (ordinary)

with the note:

> A world switch is a capability of the body's world exactly as the diet gate is a capability
> of the body: the legacy controller applies both, and the settlement would refuse the bite
> anyway. Masking here keeps the `Decision` honest.

### §2, "Bodies" — deterministic ties

Add after the `rel_size` formula:

> **Ties.** The neighbour list arrives sorted by `(distance, id)` and the sampler keeps the
> *first* contributor that attains the maximum (a strict `>`), so two neighbours with an
> identical weighted presence are separated by distance and then by their full `OrganismId`.
> The result does not depend on slot reuse order.

### §5, "Policy table" — what the digest is taken over

Replace the digest sentence with:

> One `schema_digest: u64` = FNV-1a over the canonical text
> `"cub-obs-1|cub-act-1|gru32-reset-after-1|motor:r0b-99a2bfc|10hz"` followed by `|obs:` and the
> §1 field list and `|act:` and the §3 channel list, as one ASCII string
> (`crate::neural::PROFILE_TEXT`). The exact bytes are the constant in the source; a document
> and a build that disagree about them would produce two digests, which is precisely the
> failure the digest exists to catch, so the source is authoritative and this section names it.

### §5, "Snapshot schema" — inspected, not assumed

The current build's schema was 14 (`snapshot.rs`), so the extension is **15** and the frozen
mirror is `snapshot/v14.rs`, as §5 and §9 already say. No correction needed; recorded because
the brief asked for the number to be inspected rather than taken from the plan.

## 6. What is left, and what is not claimed

- **No trained policy exists.** Every weight set in this slice is hand-authored or zero. The
  memory check is a statement about the interface; the probe is a statement about the world's
  response to a fixed tape. Neither is evidence about learning, foraging competence or
  sustainability.
- **No apex extension, no weight mutation, no optimizer, no migration command, no subsidy.**
  Neural+apex and neural+quiet are refused; a legacy world is never migrated by loading.
- **The starvation correction is a world-physiology change, not an ecology rebalance.** The
  cropping floor R0b and R0d both named is still open, and so is the energy rebalance the R0d
  pace forces.
- **The far-ring seam residual is real, if tiny.** Own cell, near ring and body channels match a
  physically equivalent flat layout exactly; the far ring (hops 2–3) matches to 5.1e-6, which is
  floating point in the unfold moving a hair of weight between two overlapping sector windows.
  Nothing at that scale is sensory information, and nothing here claims bit-identity.
- **The component timings carry their own instrument.** Two `Instant::now()` calls bracket each
  region, so the 0.052 µs adapter column in particular is close to the measurement floor.
- **The display world was not observed over a long horizon.** It was reset, confirmed stepping,
  and left alone; no ecological claim follows from that.

Closed in repair cycle 1, and therefore no longer listed: the seam-equivalent sampling fixture,
the 17-neighbour truncation fixture, the held turn through a seam, and the sensor/inference
timing split.

## 6a. Repair cycle 1 (2026-09-15)

Against [Astra's review](r1a-runtime-review-2026-09-14.md) of `693a300`. All three of the
review's reproductions were confirmed failing on a copy of its
[source](assets/r1a-review-regressions.rs) before anything was changed, then promoted into the
permanent suites and the temporary file deleted.

### P1 — Predation left dangling private state

**Root cause.** The capture settlement (`world/step.rs`, the `caught` branch) is the *second*
boundary in the world that removes an organism, and it removed only the body. The physiology
pass's death commit had been given `neural.remove`; this one had not. An ordinary neural body is
legitimate prey for a legacy hunter — refusing a neural *apex* says nothing about that
combination — so after an ordinary interaction the world failed its own `validate` with
`neural animal 1:1 has no organism`, which breaks continuation loading.

**Fix.** `neural.remove(prey_id)` at the capture boundary, beside the body's removal.

**Regression.** `tests/neural_predation.rs::a_captured_neural_prey_leaves_no_private_state_behind`
— a certain capture of a neural prey under a legacy hunter, then `validate`, `check_invariants`,
a snapshot round trip that resumes and steps 40 ticks, and slot reuse starting from zero hidden
state. Before: `Err("neural animal 1:1 has no organism")`. After: valid, lossless, resumes.

### P1 — Solvency admitted what settlement never collected

**Root cause.** `raisable_energy` admits a body that can raise this tick's upkeep from
`energy + one tick of oxidation`, but the payment was `min(cost, energy)`. A body with no stored
energy had the whole bill forgiven, and the physiology pass then credited it the full oxidation
gain. The review measured a body owing 1.85e-4 e ending the tick holding its entire 8.0e-4 e
credit, having paid 7.59e-19 e. The original test asserted survival and non-negative energy —
not payment — which is exactly the gap.

**Fix.** Settlement now collects from the same resources solvency counted: stored energy first,
then exactly the shortfall oxidised out of the reserve, at the world's own rate, density and
efficiency, with the burned material going to `fields.n` and the inefficiency to heat — the
physiology pass's own transaction. The burn is recorded per slot and subtracted from that body's
allowance in the physiology pass, so `oxidation_rate · dt` bounds the **tick**, not each pass.
Only the *mandatory* half can ever reach the reserve: `motor_budget` is sized from stored energy
alone, so a body short of upkeep has `u = 0`, stands still, and its shortfall is never motion.
Movement is still paid from the battery.

**Regressions** (`tests/starvation.rs`), all asserting the actual payment:

| case | assertion | result |
| --- | --- | --- |
| zero stored energy, adequate reserve | payment == upkeep to 1e-15 | was ~0, now exact |
| partial stored energy | battery first, reserve gives up exactly the shortfall | burn is the shortfall's 1.45e-4, not the whole 5.0e-4 allowance |
| inadequate reserve | pays what it has, dies of Starvation | dies |
| intake later in the tick | fed and bare arms burn identical reserve | both 1.9375e-4 |
| shared allowance | total burn ≤ `oxidation_rate · dt` | holds |

The accounting suite stays green: material, energy and heat are all conserved.

### P2 — Neural budding skipped the retained minimum-age gate

**Root cause.** `decide_quiet` refuses a budding request on five conditions: the quiet hold,
`escrow.is_none()`, the two drive *thresholds* `bud_reserve` and `bud_energy`, and
`bud_min_age_seconds`. A neural body skips that function entirely, and no equivalent world
admission check existed.

**The exact conditions retained**, and why:

| condition | owner | reason |
| --- | --- | --- |
| `escrow.is_none()` | world (already) | one gestation at a time is a lifecycle rule |
| funding (`reserve ≥ structure+reserve`, `energy ≥ build+energy`) | world (already) | the child must be paid for |
| population cap | world (already) | capacity |
| `bud_min_age_seconds` | **world, added here** | physical maturity; the brief keeps present maturity, funding and gestation constraints in this slice |
| `bud_reserve`, `bud_energy` | **policy** | these are drive *preferences* — "am I comfortable enough to breed" — which is exactly the behaviour the policy is meant to own |
| quiet hold | n/a | neural+quiet is refused outright |

**Fix.** The ordinary gestation admission in the physiology pass now also requires
`age_ticks · dt ≥ bud_min_age_seconds`. It is a strict no-op for a legacy body: `d.bud` already
carries the same test, and a scripted diagnostic intent can only suppress `bud`
(`d.bud = d.bud && b`), never set it.

**Regressions** (`tests/neural_runtime.rs`): below the gate (age 0, and one tick short of 120 s)
opens no escrow; at the gate a funded adult does; mature but unfunded does not. The existing
birth test's precondition is repaired — it now winds the world clock past the gate so the parent
is genuinely old enough, instead of asserting it in a comment; its horizon drops 4,200 → 1,200
ticks as a result.

### P2 — The probe reported field loss as intake and net energy as motor cost

**Root cause.** `P eaten` was accumulated from own-cell stock deltas, which contain growth,
mortality and decomposition — and which read the wrong cell: the settlement runs *after*
movement, so the cell that is grazed is not always the one the body started the tick on.
`motor` was `start.energy − end.energy − assumed_upkeep`, mixing expenditure with assimilation
and oxidation credits.

**Fix.** Intake from `World::intake_diagnostics()`; the bill rebuilt from the resolved motion
through `MotorBill::motor_cost`; paid upkeep, paid motion, net reserve and net energy reported
separately so credits are visible instead of netted away.

**Before and after**, same tape:

| row | old "P eaten" | new eaten P | old "motor" | new paid motion |
| --- | ---: | ---: | ---: | ---: |
| rest | 0.1796 | **0.0000** | 0.00000 | 0.00000 |
| travel | 0.1692 | **0.0000** | 0.01799 | 0.01799 |
| graze in place | 0.4044 | **0.2588** | 0.00000 | 0.00000 |
| travel and graze | 0.4150 | **0.2649** | **0.00000** | **0.01799** |

The probe now asserts both checks: a closed mouth records exactly zero intake, and
travel-and-graze records both a motor debit and an intake. The same confounder was found in a
starvation fixture while writing it; that one reads the diagnostic too.

### The three owed checks

`tests/neural_geometry.rs`, through a new read-only `World::neural_observation` accessor. The
sampling block is factored out of `neural_decision` into `sample_observation`, so the accessor
runs the same geometry the controller does rather than a second copy of it.

- **Seam-equivalent sampling.** A body in the last cell row before the Top/Front seam and one
  deep in the interior, both at their cell's exact centre (sub-cell position decides which cells
  a body-relative offset lands in, so this has to match or the fixture measures itself), painted
  with the same stocks at the same body-relative offsets via `travel`, with a companion body at
  the same body-relative position. Habitat channels excluded per contract §9.
  **Measured: own cell and near ring max |Δ| = 0, body channels max |Δ| = 0, far ring 5.1e-6.**
- **17 neighbours.** Sixteen crowded behind the observer and a seventeenth alone dead ahead, all
  inside `r_sense`: sector 0 reports no presence while the others do, and the fixture asserts the
  seventeenth really was in range.
- **Held turn through a seam.** A neural body under a steady clockwise turn crosses the seam; the
  five ticks after turn by the same signed amount as the five before, to 1e-9. The crossing tick
  itself is excluded as a change of chart, not a turn.

### Component timings

`NeuralTiming` (transient, never persisted, never hashed, written only inside the neural branch)
counts wall nanoseconds and calls at the sampler, at `Gru32::forward` and at the action adapter.
The numbers are in §4. The headline: the summed component cost (1.76–1.98 µs/body-tick) is
*larger* than the legacy-versus-neural world difference (1.56–1.68), because that difference also
subtracts the legacy controller work the neural arm does not do.

## 7. Build and development status

Wrysk resets `state/` at every major change, and did so at the end of both passes.

- **After R1a.** pid 1848090, build `0.1.0+a0cb6bf`, fresh, ran to tick 195355.
- **After repair cycle 1.** Stopped 1848090, emptied `state/` including `.lock`, relaunched
  `nohup ./scripts/run-cube.sh --fresh`. pid **2062111**, build **`0.1.0+a106cee`**, `state_dir`
  `/home/wrysk/wryskware/cubarium/state`, `sink` `shim`, **`resumed_from: null`**,
  `start_tick: 0`, stepping. The shim was not touched.

The display world is **legacy-controlled** in both passes. No policy is attached to it, and
nothing attaches one by itself: `World::attach_neural_policy` is the only door and it is called
nowhere outside the probe and the fixtures. `World::neural_observation`, added in the repair
pass, is read-only — it takes no step, consumes no draw and changes nothing.

Commits, oldest first:

| commit | what |
| --- | --- |
| `6b9e255` | step zero: the starvation predicate, its two tests, the two changed apex fixtures |
| `ac5d8ef` | `neural/{obs,action,gru,state}.rs`, 33 unit tests |
| `ea07e5b` | per-animal dispatch, `Decision.speed_request`, schema 15 + `v14` mirror, 10 runtime fixtures |
| `a0cb6bf` | `examples/neural_probe`: the action tape and the throughput screen |
| `7614bef` | this result document |
| `2055a9b` | repair 1: the three runtime defects (predation boundary, solvency settlement, maturity gate) |
| `a106cee` | repair 1: honest probe accounting, component timings, the three owed fixtures |

## Usage

Measured token usage: unavailable in this harness. Context-counter deltas are not a billed-usage
measurement and are not reported as one.
