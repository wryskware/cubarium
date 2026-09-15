---
design_status: exploration
last_reviewed: 2026-09-14
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
| `cargo test -p cubarium-core --release` | 438 passed, 0 failed, 2 ignored |
| `cargo test --workspace --release` | 1,274 passed, 0 failed, 20 ignored |

New: `src/neural/**` unit tests (33), `tests/neural_runtime.rs` (10), `tests/starvation.rs` (2).

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

| segment | px travelled | net turn (deg) | upkeep (e) | motor (e) | P eaten (m) | reserve gained (m) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rest | 0.00 | 0.0 | 0.06200 | 0.00000 | 0.1796 | 0.0000 |
| travel (thrust 1) | 49.98 | 0.0 | 0.06200 | 0.01799 | 0.1692 | 0.0000 |
| pure pivot (turn +1) | 0.00 | 900.0 | 0.06200 | 0.00707 | 0.1796 | 0.0000 |
| split (≈0.5, ≈0.5) | 25.00 | 450.5 | 0.06200 | 0.01254 | 0.1790 | 0.0000 |
| graze in place | 0.00 | 0.0 | 0.06200 | 0.00000 | 0.4044 | 0.1553 |
| travel and graze | 49.98 | 0.0 | 0.06200 | 0.01799 | 0.4150 | 0.1590 |

Read plainly: a pure pivot turns 90°/s (the genome ceiling, which binds at the new pace) with
zero travel and pays for the sweep; the split halves both and costs between the two; grazing
works standing still and while travelling, out of one mouth. The `P eaten` in the non-grazing
rows is the cell's own background flux, not the mouth. The two grazing rows show zero *net*
energy spent because assimilation refills a battery that is already at its ceiling; the upkeep
column is still the bill that was charged.

**Throughput.** Each arm run twice, all-legacy and all-neural, so the sampler-and-inference
cost is a difference and not an estimate. Births off.

| bodies | ticks | legacy ticks/s | neural ticks/s | sampler+inference µs per body-tick |
| ---: | ---: | ---: | ---: | ---: |
| 32 | 2,000 | 14,027 | 8,568 | 1.420 |
| 128 | 2,000 | 6,716 | 2,980 | 1.459 |
| 512 | 200 | 1,046 | 569 | 1.566 |

Whole screen: **1.9 s of wall time** against the 60 s budget. The per-body-tick cost is
essentially flat in population, as it should be: the sampler is bounded by `hops` and by
`max_neighbors`, and inference runs on every second tick. At 512 bodies an all-neural world
still runs 569 ticks/s, 28× the 20 Hz the display needs. **No channel was cut, and none needs
to be on this evidence.** Sensor and inference are not separated further than this difference:
the screen measures the whole neural path against the whole legacy path, which is the quantity
that decides whether the runtime is affordable.

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

- **Two named §9 sampler fixtures were not built.** "A body straddling a seam senses the same
  70 values as the equivalent flat layout" and "17 neighbours disclose the truncation" have no
  test. The first is structurally true — the sampler receives offsets that the world has
  already unfolded into the observer's chart, and does nothing chart-dependent — but
  *structurally true* is not *tested*, and the seam behaviour that **is** tested is the weaker
  save/resume-across-a-seam agreement. The second is the world's existing `max_neighbors`
  budget, unchanged by this slice and undisclosed to the policy by design.
- **The held turn is tested across ticks, not across a seam.** The mechanism is the same — the
  request is rebuilt from the current transported heading every tick — and a seam-parked body
  is in the persistence fixture, but there is no fixture that measures the signed turn on
  either side of a crossing.
- **No trained policy exists.** Every weight set in this slice is hand-authored or zero. The
  memory check is a statement about the interface; the probe is a statement about the world's
  response to a fixed tape. Neither is evidence about learning, foraging competence or
  sustainability.
- **No sensor/inference split.** The throughput screen reports the whole neural path against
  the whole legacy path. Separating the sampler from the GRU would need instrumentation this
  slice did not add.
- **No apex extension, no weight mutation, no optimizer, no migration command, no subsidy.**
  Neural+apex and neural+quiet are refused; a legacy world is never migrated by loading.
- **The starvation correction is a world-physiology change, not an ecology rebalance.** The
  cropping floor R0b and R0d both named is still open, and so is the energy rebalance the R0d
  pace forces.

## 7. Build and development status

Wrysk authorized a world reset at ship time ("we can just reset it every major change").

- **Before.** pid 1738804, build `0.1.0+3e21c57`, `state/`, world tick 51409, `resumed_from:
  null`. Stopped.
- **`state/`** emptied, `.lock` included.
- **After.** `nohup ./scripts/run-cube.sh --fresh`, which rebuilds this checkout and launches
  the normal `assets/atelier`. pid **1848090**, build **`0.1.0+a0cb6bf`**, `state_dir`
  `/home/wrysk/wryskware/cubarium/state`, `sink` `shim`, **`resumed_from: null`**,
  `start_tick: 0`, stepping (tick 174 at the check). The shim was not touched.

The display world is **legacy-controlled**. No policy is attached to it, and nothing attaches
one by itself: `World::attach_neural_policy` is the only door and it is called nowhere outside
the probe and the fixtures. The probe runs on its own isolated world.

Commits, oldest first:

| commit | what |
| --- | --- |
| `6b9e255` | step zero: the starvation predicate, its two tests, the two changed apex fixtures |
| `ac5d8ef` | `neural/{obs,action,gru,state}.rs`, 33 unit tests |
| `ea07e5b` | per-animal dispatch, `Decision.speed_request`, schema 15 + `v14` mirror, 10 runtime fixtures |
| `a0cb6bf` | `examples/neural_probe`: the action tape and the throughput screen |

## Usage

Measured token usage: unavailable in this harness. Context-counter deltas are not a billed-usage
measurement and are not reported as one.
