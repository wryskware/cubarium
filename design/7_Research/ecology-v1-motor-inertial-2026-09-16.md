---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# An inertial motor model, paired against the shipped sweep model

Workstream T of ecology v1 round 4, to
[the brief](../handoffs/ecology-v1-motor-inertial-opus-2026-09-16.md), on Wrysk's
direction of 2026-09-16: *"make movement expense cost like actual physics would
require … if we're just doing an approximation, use a mean radius / sublinear
rotation … we don't need to model 'are claws outstretched' when turning; add
their mass in, model everything as balls or cylinders and use rough
heuristics."*

Evidence, not a decision. Nothing here is adopted; §6 states plainly what
adopting it would mean.

## 0. The short of it

`MotorModel::Inertial` gives every body a **second and a bit** more turning room
for the same budget, and the apex **2.33×** more. Paired against the shipped
`Sweep` model on P's eight-seed apex design it raises contacts 140 → 207,
captures 67 → 93 (2.09 → 2.91 per life), and — for the first time in this
workstream's history — makes the paid burst actually **close** the gap
(+0.21 px per burst under `Sweep`, −0.57 px under `Inertial`). Two apex members
crossed the 24,000-tick reproduction age gate that no member had ever reached.

On the whole world it is not an ecological lever the way the r0a rotation price
was. Over 36 paired `(candidate, seed, arm)` rows the motor's share of the body
bill rises from 12.0 % to 17.8 % on `fast-leaf` (18/18 pairs, spread
[+5.3, +6.1] pp), range rises 14 % (18/18), and feeding fraction rises 3 pp
(18/18) — while population moves −5.6 %, foliage retention −0.7 %, litter −1.5 %
and every one of the six gates is kept in every row.

## 1. The model

| | radius in the rotation term | envelope | bill |
| --- | --- | --- | --- |
| `Sweep` **(default, shipped)** | `max(lobes, grasp)` — the outermost *contacting* point | `\|v\| + r·\|ω\| ≤ speed_cap` | `move_cost·S·(\|v\| + k·r·\|ω\|)·dt`, `k` = 0.5 |
| `Inertial` | `lobes/√2` — a uniform disc's radius of gyration, grasp excluded | `√(v² + v_rot²) ≤ speed_cap` | `move_cost·S·(\|v\| + v_rot)·dt` |

Every organism under `Inertial` is a uniform disc of mass ∝ `structure` and
radius `phenotype.extent`. `v_rot = r·|ω|/√2` is the translation speed carrying
the same kinetic energy as spinning that disc at `ω`, so rotation is priced and
bounded as an **energy** — which is why it combines with `v` in quadrature
rather than by addition, and why `ROTATION_COST_SCALE` (the rod figure 0.5,
a stylized price) disappears into the radius. Angular-rate ceilings, wading and
the burst list are untouched; the capability that scales the cap is unchanged.

**A body that only translates is identical under both models** — the same bill
bit for bit, and the same delivered speed at every energy. That invariant is
what makes the pairing a measurement of rotation and nothing else, and it is
pinned by two tests.

**Not in this pass:** a cost of *acceleration*. Both models are memoryless and
price the motion held during the tick, not the change in it. Named as the next
step in §7.

### The visible effect, stated

Two numbers say what a viewer would see.

| body | rotation radius, `Sweep` → `Inertial` | pivot at the same budget | price per radian |
| --- | ---: | ---: | ---: |
| ordinary adult (`extent` 2.5 px) | 2.500 → 1.768 px | **×1.414** | ×1.414 |
| lanternjaw apex (`extent` 9 px, grasp 14.825 px) | 14.825 → 6.364 px | **×2.329** | **×0.859** |

An ordinary body turns √2 faster and pays √2 more per radian — the disc's 1/√2
at full price against half of the outer point's sweep. The **apex is the only
body whose radian gets cheaper**, because dropping the 14.8 px grasp more than
halves its radius and the full price only gives most of that back. What the apex
really gains is room, not a discount: 2.33× the pivot rate, on top of a
quadrature envelope that lets a body spending half its budget on rotation still
travel at √3/2 of the cap where `Sweep` leaves it exactly half.

One correction falls out of this. `world::view::neural_observation` and
`world::step::neural_decision` already read `turn_radius_px(o, None)` — the lobe
extent, with no grasp — while the envelope used the grasp. Under `Sweep` an apex
was therefore *told* one radius and *bounded* by another; under `Inertial` the
radius the envelope uses, the radius the bill uses, the radius the neural
adapter's `Envelope` uses and the radius the observation reports are one number
for every body in the world, apex included, for the first time.

### One design call worth naming

The two models bound different quantities, so the resolver's scaling factor is
found differently.

- `Sweep` collapses the capability and the purse onto one number,
  `u = min(speed_cap, motor_budget)`, and bounds `|v| + r·|ω|` by it.
  `affordable_motor` prices that whole magnitude at the dearer of its two
  halves, so a *turning* body is throttled slightly harder than its own bill
  requires. That conservatism is shipped behaviour and is kept arithmetic for
  arithmetic.
- `Inertial` keeps the two apart, because under it the capability bounds
  `√(v² + v_rot²)` and the purse bounds the billed `|v| + v_rot`. Both are
  positively homogeneous in the request, so each yields a ratio and the factor is
  the smaller. Nothing is approximated, the charge is exactly what the energy
  buys, and a purely translating body sees the two constraints coincide — which
  is what makes its behaviour identical under both models rather than merely its
  bill.

The alternative (one conservative collapsed budget at the worst-case √2) would
have throttled energy-bound *translation* by √2 under `Inertial`, confounding the
pairing with a change that has nothing to do with turning. Rejected for that
reason.

## 2. The switch

`cubarium_core::MotorModel::{Sweep, Inertial}`, a **`World`-level transient**:
`World::set_motor_model`, never persisted, never hashed, never a `WorldConfig`
field (which would move `calibrate::config_hash` for every retained row). A
resumed world runs `Sweep` until told otherwise. Selectable from
`apex-audit --motor`, `calibrate --motor`, `es-train --motor` and
`es-evaluate --motor`; an unrecognised name is refused, never defaulted.

Provenance. `es::trainer::Protocol` and `es::export::PolicyFile` both record the
contract by name, and `PolicyFile::check_motor` refuses a mismatch beside
`check_ecology`. `Sweep` is `skip_serializing_if`-skipped from the protocol's
JSON exactly as the R2a `min` aggregate is, so **every existing protocol hash is
unmoved** — `Protocol::default().hash()` is still `0x65c51e05060f0d5a`. An
`inertial` protocol is a different task with a different hash. A policy file that
records no motor reads as `sweep`: unlike the ecology, where `None` is genuinely
unknown and is refused, there was exactly one motor contract in this workspace
when those files were written.

**Not trained.** No policy was retrained for this note; both arms run the world's
own controllers and the ecologies' own configurations.

### The tests, written from the definitions before the implementation

19 tests: 14 in `crates/cubarium-core/tests/motor_inertial.rs`, 5 in
`crates/cubarium-search/tests/motor_provenance.rs`.

| test | what it fixes |
| --- | --- |
| `the_sweep_model_is_byte_identical_to_the_build_that_never_heard_of_the_switch` | six pinned state hashes at 1,500-tick boundaries of 9,000 ticks of a world with two apex adults and eight neural animals, **printed by commit 2eb8a9f before `MotorModel` existed**; both the untouched default and an explicitly-named `Sweep` must reproduce them |
| `the_inertial_model_moves_the_same_world` | the variant is not vacuous, and both worlds still close their books |
| `the_switch_is_a_transient_and_is_not_persisted` | a snapshot round trip resumes under `Sweep` |
| `pure_translation_bills_identically_under_both_models` | the bill, bit for bit, at six speeds |
| `an_energy_bound_translation_is_identical_under_both_models` | the *delivered speed* too, at six energies including zero — the two-constraint design call above |
| `a_pure_rotation_is_priced_by_the_radius_each_model_uses` | `r·ω/√2` at full price against `0.5·r·ω`: √2 dearer for an ordinary disc, 0.859 for the apex |
| `the_envelope_admits_the_diagonal_under_inertial_and_refuses_it_under_sweep` | `v = v_rot = cap/√2` sits exactly on the quadrature envelope and √2 outside the shipped one, which scales it by 1/√2 |
| `a_resting_disc_pivots_root_two_faster_than_the_outer_point_model_allows` | `u/r` against `√2·u/r` |
| `the_apex_grasp_is_a_turn_radius_only_under_the_sweep_model` | `turn_radius_px_in` drops the grasp under `Inertial`, and the observation's radius and the envelope's finally agree |
| `the_apex_gains_turning_room_rather_than_a_discount` | ×2.329 against an ordinary body's ×1.414, and the √3/2-against-1/2 travel left at half budget |
| `the_inertial_resolver_stays_inside_the_envelope_and_inside_the_purse` | 480 requests over degenerate radii, caps, energies and turns: finite, in-envelope, never outrunning the energy it was sized from |
| `the_neural_adapter_produces_in_envelope_motion_under_both_models` | `neural/action.rs` is **not edited**: handed the model's radius, its `omega_attain`, `requested_speed` and `requested_magnitude` produce finite, in-envelope, payable motion, and the delivered magnitude never exceeds the requested one so the feedback channel stays a fraction |
| `the_default_model_is_sweep_and_both_names_round_trip`, `the_inertial_rotation_term_is_the_discs_radius_of_gyration` | the names and the algebra |
| `the_sweep_protocol_keeps_the_hash_it_has_always_had` | `0x65c51e05060f0d5a`, and "motor" absent from the JSON |
| `an_inertial_protocol_is_a_different_task_with_a_different_hash` | different protocol hash, identical layout and config hashes |
| `a_policy_is_refused_by_name_under_the_other_motor_contract`, `a_policy_from_before_the_switch_reads_as_the_shipped_contract` | the refusal, and what `None` means |
| `a_layout_builds_its_world_under_the_contract_it_carries` | the protocol's record is not a label on a world that ignored it |

Green: `cargo test -p cubarium-core --release` 537 passed / 0 failed / 4 ignored,
`cargo test -p cubarium-search --release` 242 passed / 0 failed / 5 ignored,
`cargo test -p cubarium --release --test run_neural_seed` 7 passed, and
`cargo test --workspace --release` 1,635 passed / 0 failed / 29 ignored.

## 3. Paired arm A — the apex

P's eight-seed two-apex design, unchanged, with `--pursuit-stop reach-envelope`
under both motor contracts. Two declared screen candidates
(`baseline` `fc1aefa33ebd70a1`, `fast-leaf` `09e244392ec91768`), eight held-out
seeds, two adults introduced at tick 6,000, horizon 180,000, ledger and strike
record on, 8 workers. Search build `0629ac8motor`.

```bash
cubarium-search apex-audit \
  --config runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml \
  --seeds 8 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --pursuit-stop reach-envelope --motor sweep|inertial --workers 8 \
  --out runs/ecology-v1-motor-inertial/armA-{sweep,inertial}.json
```

Wall 54.7 s and 53.3 s: **108 s** against the brief's 3-minute cap.

**The `sweep` arm reproduces P's `reach-envelope-8` row for row.** 869 delivered
(89.7 %), 52 held (5.4 %), translation 4.57, sweep 7.96, motor 12.46, prey 2.61,
contacts 140 (14.4 %), captures 67 (2.094/life), contact→capture 47.9 %,
`OutOfReach` 829, `GraspUnmapped` 5, lifetime 13,164 / 12,282 / 23,201,
`Starvation` 32/32, predation 67, prey 745 → 847, and the gap bins
44/30/19, 238/89/39, 332/17/9, 221/4/0, 134/0/0 — all P's numbers exactly. The
pairing is therefore a measurement of the motor contract and of nothing else on
the apex side.

| | `sweep` | `inertial` |
| --- | ---: | ---: |
| recorded paid attempts | 969 | 1,020 |
| held at the burst's start | 52 (5.4 %) | 65 (6.4 %) |
| delivered | 869 (89.7 %) | 850 (83.3 %) |
| no strike frame | 48 | 105 |
| mean initial gap | 10.91 | 11.59 |
| **delivered:** separation at the burst's start | 11.44 | 12.50 |
| **delivered:** gap change over the burst (+ = grew) | **+0.16** | **−0.68** |
| **delivered:** hunter translation | 4.57 | **8.55** |
| **delivered:** hunter rotation term | 7.96 (at r = 14.83) | **15.61** (at r = 6.36) |
| **delivered:** whole motor `\|v\| + rot` | 12.46 | **24.00** |
| **delivered:** prey realised speed | 2.61 | 4.04 |
| **whole-arm gap change per burst** (+ = grew) | **+0.21** | **−0.57** |
| **contacts** (`resolved_in_reach`) | **140 (14.4 %)** | **207 (20.3 %)** |
| misses | 68 | 107 |
| **captures** | **67 (2.094 / life)** | **93 (2.906 / life)** |
| contact → capture | 47.9 % | 44.9 % |
| `OutOfReach` / `GraspUnmapped` | 829 / 5 | 811 / 7 |
| apex lifetime mean / median / max (ticks) | 13,164 / 12,282 / 23,201 | 14,541 / 11,178 / **46,449** |
| death cause | `Starvation` 32/32 | `Starvation` 32/32 |
| prey deaths by predation | 67 | 93 |
| prey population at introduction → end (Σ 16 runs) | 745 → 847 | 628 → 784 |

### The apex's ledger, over 32 lives

| line (e) | `sweep` | `inertial` | Δ |
| --- | ---: | ---: | ---: |
| upkeep | 155.87 | 172.16 | +10.5 % |
| strike, retreat and handling | 78.98 | 83.32 | +5.5 % |
| **motor — translation and turning** | **8.55** | **16.18** | **+89 %** |
| — translation | 3.872 | 7.621 | +97 % |
| — rotation | 4.677 | 8.562 | +83 % |
| **motor share of the whole bill** | **3.51 %** | **5.96 %** | +70 % |
| rotation's share of the motor charge | 54.7 % | 52.9 % | |
| total raised and spent | 243.39 | 271.67 | +11.6 % |
| credited from the gut | 33.20 | 54.12 | +63 % |
| gut material per life (m) | 0.648 | **1.057** | +63 % |
| break-even intake per life (m) | 4.754 | 5.306 | |
| **earned fraction of its own bill** | **13.6 %** | **19.9 %** | |

(`earned fraction` = energy credited from the gut ÷ total raised and spent. It
is within 0.7 pp of P's material-ratio definition of the same idea.)

### Captures by the gap the attempt began at

| initial gap | `sweep` n / contacts / captures | `inertial` n / contacts / captures | `sweep` mean `\|m\|` | `inertial` mean `\|m\|` |
| --- | ---: | ---: | ---: | ---: |
| 0–4 px | 44 / 30 / 19 | 44 / 27 / 12 | 5.30 | 7.73 |
| 4–8 px | 238 / 89 / 39 | 245 / **117** / **51** | 8.79 | 12.12 |
| 8–12 px | 332 / 17 / 9 | 282 / **42** / **18** | 11.25 | 21.07 |
| **12–16 px** | 221 / **0** / **0** | 272 / **17** / **10** | 15.03 | 28.23 |
| **16 px and beyond** | 134 / **0** / **0** | 177 / **4** / **2** | 15.46 | 30.81 |

This is the whole apex story in one table. Under `Sweep` an attempt that began
more than 12 px out **never once** ended in the claws, over 355 attempts. Under
`Inertial` the same band produces 21 contacts and 12 captures. The member's
motor magnitude in that band roughly doubles, and the extra is bought by turning
that no longer costs it 14.8 px of radius.

**And the reproduction gate finally opens.** Under `Sweep` no member ever became
eligible: the oldest reached 23,201 ticks against `may_reproduce`'s 24,000, so
every one of the 32 died at 97 % of its own minimum reproduction age. Under
`Inertial` members reach 46,449 ticks, the age gate stops being the refusing
term, and the first refusal becomes *reserve below the stock fraction* on 86.2 %
of 465,299 member-ticks watched. Still no mating — but the barrier has moved from
"never lived long enough" to "never stored enough", which is a different problem.

**A caveat the pairing cannot remove.** `--motor` changes the contract for *every*
body from tick 0, so the prey the apex is dropped into at tick 6,000 differ:
745 against 628 across the 16 runs. Part of the apex's improvement is hunting a
thinner, faster-turning prey population. Arm B measures that world directly.

## 4. Paired arm B — the whole world

A's screen control rows under both contracts: `baseline` and `fast-leaf`, the six
training seeds (1001–1006), apex arms 0/1/2, the shipped movement price
0.00036, horizon 180,000, `sample_every` 600, apex introduced at tick 6,000,
ledger on, 8 workers. 36 rows per contract.

```bash
cubarium-search calibrate --stage screen-{sweep,inertial} \
  --candidates baseline,fast-leaf --seed-set training --seeds 6 --arms 0,1,2 \
  --ledger --motor {sweep,inertial} --ticks 180000 --sample-every 600 \
  --introduce-tick 6000 --workers 8 --out runs/ecology-v1-motor-inertial
```

Wall 133.2 s and 132.5 s: **266 s** against the brief's 8-minute cap.

**The `sweep` rows reproduce the retained hashes.** All **36 of 36**
`final_state_hash` values match `runs/ecology-v1-calibration/screen/evals.jsonl`
row for row, and the 12 arm-0 rows also match a
`runs/ecology-v1-ladder/ladder/evals.jsonl` row at the same
`(candidate, seed, arm)`. The retained screen ran with the ledger *off* and this
run with it on, which is the ledger's inertness measured rather than asserted.
Every row completed, none collapsed, mass residual ≤ 5.9 × 10⁻¹⁰.

### Every gate is kept

`baseline` prints `SPVT.C` under both contracts in all three arms; `fast-leaf`
prints `SPVTGC PLAUSIBLE` under both in all three. **No gate flips in either
direction**, including the `G` gate (both prey guilds alive) that `baseline` has
always failed and `fast-leaf` has always passed.

### Paired per-seed differences (inertial − sweep), 18 pairs per candidate

| measure | `baseline` mean Δ [min, max] | pairs > 0 | `fast-leaf` mean Δ [min, max] | pairs > 0 |
| --- | ---: | ---: | ---: | ---: |
| population, late mean | −1.12 [−8.90, +15.77] | 7/18 | −3.41 [−12.87, +6.00] | 4/18 |
| forms alive, final | −0.44 [−1, +1] | 1/18 | −0.06 [−1, 0] | 0/18 |
| foliage retention (late/opening) | +0.23 [−1.59, +1.67] | 10/18 | −0.014 [−0.034, +0.048] | 2/18 |
| litter, late mean | +4.1 [−62.9, +57.0] | 8/18 | −2.5 [−9.7, +6.7] | 4/18 |
| **range, cells per body per window** | +6.9 [−342.7, +424.0] | 13/18 | **+58.3 [+19.3, +131.1]** | **18/18** |
| **motor share of the bill** | +0.040 [−0.034, +0.117] | 13/18 | **+0.058 [+0.053, +0.061]** | **18/18** |
| **motor billed (e)** | +91.5 [−176.4, +377.9] | 13/18 | **+192.3 [+144.0, +219.1]** | **18/18** |
| **feeding fraction** | +0.061 [−0.113, +0.233] | 15/18 | **+0.030 [+0.009, +0.048]** | **18/18** |

`baseline` is a noisy ecology — it fails the `G` gate, carries one or two forms
and its seed-to-seed spread swamps most of these deltas. `fast-leaf`, the
selected ecology, is where the signal is legible, and there four measures move in
the same direction on **every one of the 18 paired rows**.

### `fast-leaf`, arm 0, mean over six seeds [min, max]

| measure | `sweep` | `inertial` |
| --- | ---: | ---: |
| population, late mean | 60.8 [58.2, 67.1] | **56.0 [53.3, 57.8]** |
| population, final | 62.3 [54.0, 76.0] | 57.3 [50.0, 62.0] |
| forms alive, final | 3.00 [3, 3] | 3.00 [3, 3] |
| form evenness, late | 0.628 [0.591, 0.654] | 0.642 [0.613, 0.658] |
| browsers / grazers / scavengers, final | 48.8 / 13.3 / 0.2 | 44.5 / 12.2 / 0.7 |
| prey births / deaths | 194.0 / 155.7 | 192.0 / 158.7 |
| deaths: starvation / age / predation | 136.2 / 19.5 / 0.0 | 137.5 / 21.2 / 0.0 |
| foliage retention (late/opening) | 2.111 [2.064, 2.182] | 2.096 [2.057, 2.170] |
| wood / litter, late mean | 162.4 / 169.2 | 163.3 / 166.7 |
| alive cells, final | 1112.7 | 1112.0 |
| depletion events / recovery events | 0.8 / 0.0 | 0.8 / 0.0 |
| **range, cells per body per window** | 371.1 [348.3, 387.5] | **421.9 [393.4, 450.6]** |
| bill total / upkeep (e) | 3184.8 / 2802.4 | 3234.2 / 2657.8 |
| **motor share of the bill** | **12.01 %** | **17.82 %** |
| leaf / litter eaten (m) | 2379.7 / 1545.1 | 2375.6 / 1576.6 |
| **feeding / seeking fraction** | 0.432 / 0.561 | **0.461 / 0.531** |
| mean hunger | 0.804 | 0.807 |

Arms 1 and 2 (one and two apex adults) differ from arm 0 only in the predation
line: predation deaths 3.3 → 2.2 and 3.3 → 2.3, over 0–7 per run, which is far
too small a count to read as a trend — and note it points the *opposite* way to
arm A, where the apex captured more. The plausible reconciliation is arm A's
caveat: `Inertial` thins the prey stock the apex hunts, so the same fraction of a
smaller stock is more captures per apex life and fewer prey deaths per world.

### `baseline`, arm 0, mean over six seeds [min, max]

| measure | `sweep` | `inertial` |
| --- | ---: | ---: |
| population, late mean | 42.8 [26.7, 49.4] | 43.2 [38.0, 48.6] |
| forms alive, final | 2.33 [2, 3] | **1.83 [1, 2]** |
| form evenness, late | 0.471 [0.333, 0.663] | 0.300 [0.000, 0.400] |
| browsers / grazers / scavengers, final | 24.8 / 13.0 / 2.2 | 23.2 / 17.7 / **0.2** |
| foliage retention | 2.310 [2.00, 3.13] | 2.423 [2.01, 3.74] |
| litter, late mean | 161.2 | 158.0 |
| depletion / recovery events | 11.3 [3, 33] / 0.2 | 16.3 [3, 55] / 0.0 |
| range, cells per body per window | 347.9 [156.5, 424.2] | 400.2 [62.0, 500.4] |
| **motor share of the bill** | **11.37 %** | **16.27 %** |
| feeding / seeking fraction | 0.448 / 0.544 | 0.487 / 0.505 |

`baseline`'s variety loss is the one change worth flagging: forms alive fall in
8 of 18 paired rows and rise in 1, and the scavenger guild all but disappears in
arm 0 (2.2 → 0.2 final). It is a marginal ecology to begin with — `baseline`
fails the `G` gate under both contracts — and its seed spread is enormous, so
this is a signal to watch under `Inertial`, not a measured harm.

### Against r0a

`design/7_Research/r0a-motor-cost-ecology-2026-09-14.md` measured the rotation
price moving the legacy population 93 → 39 under the **pre-R0b** union-of-ceilings
envelope, where a unit adult could sweep 3.93 px/s while travelling 0.3 px/s and
`k` therefore priced thirteen times as much sweep as travel.

**That lever is gone and `Inertial` does not bring it back.** Under the shared
budget — either contract — rotation can never exceed the speed cap, so the most
the rotation term can cost is bounded by what travel already costs. The legacy
controller does turn appreciably more under `Inertial` (motor billed +50 % on
`fast-leaf`, 18/18 rows; range +14 %, 18/18) and the world absorbs it: population
−5.6 %, foliage retention −0.7 %, litter −1.5 %, every gate kept. The turning
habit changed; the ecology did not.

## 5. Where the energy goes, and why the bill barely moves

The `fast-leaf` arm-0 pair explains itself. Motor billed rises 382 e → 576 e
(+51 %), but upkeep *falls* 2802 e → 2658 e (−5.2 %) because the standing
population is 8 % smaller, so the whole bill moves only +1.6 %. The motor's share
of it rises 12.0 % → 17.8 %. The bodies are spending a visibly larger fraction of
their energy on movement, covering 14 % more ground per window, and finding food
sooner — feeding fraction 0.432 → 0.461, seeking 0.561 → 0.531 — while eating
essentially the same amount of leaf (2379.7 → 2375.6 m). The world is
food-limited, not search-limited, so turning more freely buys time rather than
calories, and the population settles slightly lower because each body costs a
little more to run.

## 6. Verdict

**(a) Does `Inertial` let the corrected apex close and capture more, and at what
earned fraction of its bill?** Yes, decisively.

| limb | reading |
| --- | --- |
| closes the gap | **yes — and for the first time.** The whole-arm gap change per burst goes from +0.21 px (growing) to −0.57 px (closing). P's corrected predicate stopped the gap growing; the corrected motor closes it. |
| reaches further | **yes.** Attempts beginning past 12 px produced 0 contacts and 0 captures in 355 attempts under `Sweep`; 21 contacts and 12 captures in 449 under `Inertial`. |
| raises contacts | **yes.** 140 → 207; 14.4 % → 20.3 % of paid attempts. |
| raises captures | **yes.** 67 → 93; 2.09 → 2.91 per life. |
| earned fraction of its bill | **13.6 % → 19.9 %.** Still starving — `Starvation` 32/32 under both — but half again as self-supporting. |
| lives long enough to breed | **the gate opens.** Max lifetime 23,201 → 46,449 ticks against a 24,000-tick `may_reproduce`; the refusing term moves from age to reserve. |

**(b) Does it change the whole world beyond seed noise?** Partly, and in a
bounded way.

- **Beyond noise, on the selected ecology:** motor share of the bill
  +5.8 pp (18/18 pairs, spread [+5.3, +6.1]), motor billed +50 % (18/18), range
  +14 % (18/18), feeding fraction +3 pp (18/18). These are not seed effects.
- **Inside noise, or nearly:** population −5.6 % (4/18 pairs positive, per-seed
  spread [−12.9, +6.0]; on arm 0 the seed ranges do not overlap, so the decline is
  real but small), foliage retention −0.7 %, litter −1.5 %, alive cells −0.1 %,
  births/deaths ≤ ±2 %.
- **Worth watching:** `baseline`'s variety. Forms alive fall in 8 of 18 pairs
  and its scavengers nearly vanish in arm 0. `fast-leaf` keeps all three forms in
  every row.

**(c) Does it keep every gate?** Yes. All six gates, all 36 rows, both
candidates, all three arms, unchanged in both directions.

### What Wrysk would be adopting

Adopting `Inertial` as **the** motor contract would mean, concretely:

1. **The rotation term and the envelope change for every body in the world, not
   just the apex.** Rotation becomes `r·ω/√2` over the body's own lobes at full
   `move_cost`, and translation and rotation combine in quadrature rather than by
   addition. `ROTATION_COST_SCALE` stops being a knob — it is absorbed into the
   radius — and `turn_radius_px` stops meaning "the outermost contacting point".
2. **On the cube it would look like bodies that turn more freely.** Every
   ordinary body pivots √2 faster for the same effort and can hold most of its
   travel through a turn; paths get curvier and less angular. Bodies cover 14 %
   more ground per window and spend 3 pp more of their time feeding. An apex
   pivots 2.33× faster and can actually charge: it reaches prey 12–16 px away,
   which under the shipped contract it never once did.
3. **A slightly leaner world.** About 6 % fewer animals on `fast-leaf`, with
   foliage, litter, stands and every gate essentially where they are. `baseline`
   may lose a form; it is already the marginal ecology.
4. **A new protocol hash for training, and every existing policy refused under
   it by name.** `PolicyFile::check_motor` will reject `runs/es-*/policy.json`
   and every exported centre the moment they are evaluated under `inertial`, and
   correctly so: the envelope and the price of a radian are not the same task.
   **Every trained forager would have to be retrained.** This is the single
   largest cost of adoption.
5. **Which calibration rows would need re-running.** The config hash does not
   move — the motor is a transient, not a `WorldConfig` field — so no retained
   row is *invalidated* as provenance. But every row whose *measurements* depend
   on movement would have to be re-measured under the new contract:
   - `runs/ecology-v1-calibration/screen/` — all 270 rows (15 candidates × 6
     seeds × 3 arms). Candidate selection compared populations, variety and
     foliage across candidates; those move by up to 8 % and `baseline`-like
     candidates may lose forms, so the selection must be re-run before
     `fast-leaf` can still be called the choice.
   - `runs/ecology-v1-calibration/holdout/` — the held-out validation of that
     selection, for the same reason.
   - `runs/ecology-v1-ladder/ladder/` — all 60 rows. The movement-price ladder
     is *directly* about the price of moving; a contract that raises the motor
     share of the bill from 12 % to 18 % changes what every rung means.
   - `runs/ecology-v1-ladder/f-movement-evals.jsonl` and workstream F's range,
     residence and revisit measures — range moves +14 % on 18/18 rows.
   - `runs/ecology-v1-apex-predicate/` — P's four artifacts, since the apex is
     where the contract bites hardest.
   - **Not** the plant-budget, diet-factorial or depth rows, except where they
     report a motor or range measure: their plant-side quantities move under 1 %.

   The two intake/budget notes that quote a motor share of the bill would need
   their numbers restated, not their conclusions revisited.

**A recommendation, since the brief asks for a verdict and not a decision.**
The apex evidence is strong and the whole-world cost is small and bounded, but
the retraining bill in (4) and the re-run list in (5) are the real price, and
they are not worth paying for the apex alone. The cheap, honest intermediate is
to keep `Sweep` as the contract and note that **most of the apex's gain comes
from one line** — the grasp in `turn_radius_px` — which could be dropped on its
own, under the shipped envelope and price, without touching any ordinary body,
any protocol hash or any trained policy. That variant is not measured here and
§7 names it.

## 7. Next steps, named not taken

- **A cost of acceleration.** The brief's explicit next term. Both models are
  memoryless and price the motion *held* during a tick; neither prices the
  *change* in it, so a body may reverse at full speed for free. This is the term
  that would make a charge feel like momentum rather than like drag.
- **The grasp alone.** Drop `capture_offset + capture_reach` from
  `turn_radius_px` under `Sweep`, changing nothing else. Cheap, ordinary bodies
  untouched, no protocol hash moves, and it isolates how much of arm A's result
  is the disc model and how much is just the 14.8 px radius. Worth one paired
  apex run before any wider adoption.
- **Retrain one forager under `inertial`** and evaluate it against the `sweep`
  forager on their own contracts, which is the only way to learn whether the extra
  turning room is *usable* by a policy rather than merely available. The adapter
  is conservative here in one place: `neural/action.rs` was not edited, so a
  policy's requests are shaped by the linear `v + r_g·|ω|` measure even though the
  envelope is quadratic — it can never leave the envelope, but it also never asks
  for the diagonal that `Inertial` would grant.

## 8. What this does not establish

- **Nothing about learned behaviour.** No policy was retrained. Both arms run
  the legacy controller and the apex's own pursuit; what a trained forager would
  do with the extra turning room is not measured anywhere here.
- **Nothing about the apex in isolation.** `--motor` changes the contract for
  every body from tick 0, so arm A's apex hunts a different prey population
  (745 → 628 at introduction). The apex's gain and the world's thinning are
  confounded by construction, and separating them needs the grasp-only variant
  in §7.
- **Nothing about longer horizons.** 180,000 ticks is 2.5 hours of world. A
  −5.6 % population and a −0.7 % foliage retention are small; whether they
  compound over a day is not measured.
- **Nothing about `baseline`'s variety loss** beyond "it happened in 8 of 18
  paired rows on an ecology that already fails the variety gate". Six seeds
  cannot separate that from drift.
- **Residence and revisit** (F's measures) are not in the `calibrate` record and
  are not reported. Range (`cells_per_body_window`) is, and it moves.
- **The disc is a heuristic, not a body.** `structure` is treated as a uniform
  disc of radius `phenotype.extent` for every form in the world, from a browser
  to a lanternjaw. That is what Wrysk asked for — "model everything as balls or
  cylinders and use rough heuristics" — and it is not a moment of inertia.

## 9. Reproduction

Search build `0629ac8motor` (`CUBARIUM_SEARCH_BUILD` pinned to the commit), one
release binary built once and copied out of the shared target directory.
Artifacts in `runs/ecology-v1-motor-inertial/` — `armA-sweep.json`,
`armA-inertial.json`, `screen-sweep/`, `screen-inertial/` — **7.8 MiB** against
the brief's 40 MiB cap. `runs/` is git-ignored by design; the four commands in
§3 and §4 reproduce all of it in 374 s of wall on 8 workers.
