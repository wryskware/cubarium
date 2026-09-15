---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 — implementation result

Evidence, not a decision. This records what was built against
[the ecology v1 contract](../ecology-v1-contract.md) §3–§10 under the
[Opus handoff](../handoffs/ecology-v1-opus-2026-09-15.md), what the suite says about it, and
what the §13.2 scenarios measured. Nothing here promotes anything to canon, and nothing here
was tuned: every §11 value is the contract's, and every number below is what the simulator
produced at those values.

## Build and commit

- Baseline: `4283040` on `main`. The code is commit `b1dd394`; this note is the commit that follows it.
- `graft build` refreshed after the change (6,296 nodes, 13,102 edges).
- Schema 16, config version 8. No migration path exists anywhere in the tree.

## Verification

```bash
cargo test -p cubarium-core      # 460 passed, 0 failed, 2 ignored
cargo test -p cubarium-search    #  66 passed, 0 failed
cargo test -p cubarium           # 569 passed, 0 failed, 16 ignored
cargo run -p cubarium-core --release --example ecology_v1_scenarios -- all   # 63 s wall
```

All three suites are green. The two ignored core tests and the sixteen ignored host tests are
pre-existing (`#[ignore]` fixture regenerators and capture-writing art studies), untouched here.

The accounting tests A1–A9 are `crates/cubarium-core/tests/ecology_v1.rs`. A green suite is
necessary and not sufficient; the review re-derives A1, A2a, A3b, A6 and A9 independently.

## What the scenarios measured

Every row is from one `--release` run of
`crates/cubarium-core/examples/ecology_v1_scenarios.rs`, censored at the 36,000-tick horizon
and never extended.

### B0 — stand baseline (the anchor)

| class | P | W | Q | F | dP/dt | dW/dt | §11 hand P | §11 hand W |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| average | 0.1768 | 0.1091 | 0.0000 | 0.0000 | 4.2e-5 | 1.7e-5 | 0.50 | 0.33 |
| bright | 0.5178 | 0.4875 | 0.0435 | 0.0544 | 8.8e-5 | 2.1e-4 | 0.56 | 0.60 |

Both classes are **still climbing** at the horizon and neither has reached the §11 table. The
bright stand is within 8 % of the hand `P` and 19 % below the hand `W`; the average stand is
65 % below its hand `P`. Neither is equilibrium and this is not claimed to be.

**Finding B0-1, and the largest one in this note: the plant reserve `Q` never refills.** The
bright stand ends at 0.0435 — essentially the seed value 0.045 — against the hand table's 0.30,
and the average stand's reserve reaches exactly zero and stays there. §4.4 spends income on
foliage first, then wood, and only the remainder refills `Q`; with `D_P = r_p·W·dt` and
`D_W = r_w·W·dt` both proportional to `W`, a growing stand's income is fully absorbed before
the reserve is reached. Nothing is broken — the arithmetic is §4.4 exactly — but the
consequence is that a stand grown by the simulator never becomes what §4.8 calls a donor
(`Q > q_prop·Q_max`), and never holds the "finite budget for recovery" §4.4 describes. B3, B4b
and B7 below all turn on this.

### B1a — one pinned grazer, one bright mature stand, 12,000 ticks

| measure | value |
| --- | --- |
| leaf bitten | 0.5672 m |
| fruit bitten | 0.0504 m |
| feces | 0.3026 m |
| mean bite rate over the 254 s it lived | 2.43e-3 m/s |
| ratio to §11's bright sustainable yield (6.4e-4) | 3.8× |
| reserve-saturated ticks | 0 of 2,108 |
| stand at horizon | P 0.0006, W 0.4414, Q 0.0000, Wd 0.0445 |
| stand death | censored at 36,000 |
| grazer starved | tick 5,082 (254 s) |

Expected direction met in kind: the stand is stripped to `P ≈ 0`, its reserve is spent to zero,
dieback opens, and the grazer starves. Two departures from §11's arithmetic:

- **B1a-1.** The realised bite is 3.8× the sustainable yield, not ~40×. §11's 40× is the bite
  at `P = 0.5` with a full mouth; the realised average includes the collapse, during which the
  type-II term `X/(X + K_P)` throttles the mouth hard (`K_P = 0.45` against a stand that is
  under 0.01 for most of the run). The direction is the same; the multiple is not.
- **B1a-2.** The reserve **never** saturates (0 of 2,108 requesting ticks), where §13.2 expected
  it to saturate first. `R_max` is 1.0 m and the grazer took 0.62 m in total.
- **B1a-3.** The stand does **not** die inside the horizon. Dieback is `κ · unpaid` with
  `unpaid = m_w·W·dt ≈ 3.5e-6` per tick, so carrying `W` from 0.44 down to `W_min = 0.02` needs
  on the order of 10⁵ ticks. "The stand dies" is true of the mechanism and false of the horizon.

### B1b — one mobile legacy grazer on a 5 × 5 mature region

| class | region production (§11) | cells visited | Σ P painted → final | min stand P | stand deaths | grazer |
| --- | --- | --- | --- | --- | --- | --- |
| bright | 25 × 6.4e-4 = 1.6e-2 m/s | 66 | 27.578 → 0.0099 | 0.0003 | 0 | starved at 18,252 (913 s) |
| average | 25 × 1.5e-4 = 3.8e-3 m/s | 120 | 7.148 → 5.377 | 0.0748 | 0 | starved at 5,260 (263 s) |

**Finding B1b-1 — a bright failure, which §13.2 names as a finding.** Bright coexistence with
retained foliage was expected: the region's production is 1.6e-2 m/s against a cruising need of
6.9e-3. It did not happen. The single grazer ran the 25-cell region from 27.6 m of standing
foliage down to 0.01 m and then starved. The region's *production* exceeds the animal's *need*,
but production is not what the mouth meets: a stand only regrows at `r_p·W` and the bite is
`mouth_rate · X/(X + K_P)`, so the grazer strips faster than the patch renews and then cannot
recover its own upkeep from what is left. Whether this is the animal's foraging, the
plant-side rates or the type-II half-saturation is for the whole-ecosystem tuning, not this
slice.

The average arm behaved as expected: the grazer ran the region down (Σ P 7.15 → 5.38 with a
minimum of 0.075) and starved.

### B2 — depletion and relocation, three bright stands three cells apart

| stand | ticks on | min P | Q at departure | P at horizon | Q at horizon |
| --- | --- | --- | --- | --- | --- |
| (5,8) | 461 | 0.2816 | 0.0369 | 0.5578 | 0.0267 |
| (8,8) | 283 | 0.3836 | 0.0426 | 0.5579 | 0.0736 |
| (11,8) | 226 | 0.4133 | 0.0434 | 0.5579 | 0.0857 |

The grazer visited 97 cells and starved at tick 4,469 (223 s). It left each stand at
`P ≈ 0.28–0.41`, above the type-II floor `K_P = 0.45` rather than at it — the mouth is throttled
long before the stand is empty. Every stand recovered fully (to `P ≈ 0.558`) by the horizon,
which is reported rather than assumed; the grazer was dead by then, so recovery-before-return
was never tested.

### B3 — plant recovery after defoliation

| arm | reached 0.5·P* | reached 0.9·P* | reserve dip → end |
| --- | --- | --- | --- |
| bright, B0-measured stand | censored | censored | 0.0435 → 0.0000 |
| bright, §11 hand stand | 8,626 (431 s) | 19,129 (956 s) | 0.3000 → 0.0486 |
| average, B0-measured stand | censored | censored | 0.0000 → 0.0000 |
| average, §11 hand stand | censored | censored | 0.1600 → 0.0000 |

**Finding B3-1 — recovery is decided entirely by the reserve, and the B0-measured stand has
none.** A stripped stand's income is `c·P` with `P = 0`, so the only foliage it can make is
reflush from `Q`. The B0 bright stand's 0.0435 m of reserve buys `0.0435/(1+c_g) = 0.036` m of
leaf, below the §11 breakeven of `P ≈ 0.07`, so income never covers maintenance again and the
stand decays for the rest of the run. The §11 hand stand's 0.30 m of reserve buys 0.25 m, well
above breakeven, and recovers on almost exactly §11's predicted schedule (0.9·P* at ~16 min
against the predicted ~10 min). Average is censored in both arms, as §13.2 expected.

Because the contract is ambiguous about what "mature stand" means once B0 has run (§13 says
both "the §11 steady-state values" and "the measured values become the mature stand"), B3, B4b
and B7 are each run **twice**, once from each, and both are reported. See "contract
interpretations" below.

### B4a — three pinned grazers on one isolated bright mature stand

| measure | value |
| --- | --- |
| `Q` reached zero | tick 914 (46 s) |
| dieback opened | tick 915 (46 s) |
| stand death | censored at 36,000 |
| dead wood at horizon | 0.1189 m (living W 0.3455, P 0.0002) |
| grazers starved | 8,200 / 8,200 / 8,200 (410 s) |

`Q → 0` and `W → Wd` as expected, with no regrowth (there is no donor). The stand's death is
censored for the same reason as B1a-3: dieback at `κ · m_w · W · dt` is far slower than the
horizon.

### B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | censored at 36,000 | 0.0000 (0 % of B0's W) | 0.0000 |
| the §11 hand table's bright stand | tick 1,500 (75 s) | 0.0588 (9.8 % of 0.60) | 0.0696 (12.4 % of 0.56) |

With §11's stand the ring establishes in 75 s — §13.2 expected "within a minute" — and the
rebuild is censored at 30 min, also as expected. With B0's measured stand **nothing is ever
sent**: no ring cell clears `q_prop · Q_max`, so §4.8 never fires (finding B0-1).

### B5 — dietary exclusion

Conversions frozen (`m_p = ripen = drop = k_d = k_c = k_w = fall = 0`), bodies pinned, each
animal built from its full default founder kind (diet, size, metabolism, speed, depth, swim).

| kind | cap_h | cap_d | (a) foliage | (b) charged litter | (c) placed carcass |
| --- | --- | --- | --- | --- | --- |
| burrower | 0.00 | 0.90 | died 9,788, ate **0.000** | died 13,900, ate 5.421 | died 7,774, ate 1.999 |
| grazer | 0.85 | 0.00 | died 6,435, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| glider | 0.90 | 0.00 | died 6,655, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| skimmer | 0.60 | 0.40 | died 7,224, ate 0.999 | died 8,333, ate 5.845 | died 8,476, ate 1.999 |

The exclusions are exactly §6.1's: the burrower takes nothing from foliage, the grazer and
glider take nothing from litter or remains, and the skimmer eats all three. The `7,420` rows
are the pure-starvation baseline for a grazer-sized body with no food at all, so a kind that
"died 7,420" starved as if the cell were empty — which, to its machinery, it was.

**B5-1.** Every body eventually died, including the skimmer, because each fixture holds a
finite, non-renewing food (1 m of foliage, 2 m of litter, 2 m of carcass) and B5 freezes every
conversion. §13.2's "the skimmer lives on all three" is met in the sense that matters —
it is the only kind that draws on all three — but it is not immortal on them, and the test
measures dependence rather than viability, as §13.2 says.

### B6a — reproduction on a finite input (3 × 3 bright, renewal off)

| measure | value |
| --- | --- |
| births | 2 |
| deaths | 4 starvation, 0 age, 0 collapse |
| peak population | 4 |
| extinct | tick 12,621 (631 s) |
| intake | leaf 4.952 m, fruit 0.319 m |
| plant income over the run | **0.0000 m** |
| cumulative `light_in − heat_out` | **−16.97 e** |

Exactly the expected direction: births while the stock lasts, then starvation to zero, with
plant income identically zero and `U` strictly falling. Every birth was paid — A2b and A1 cover
the escrow accounting at machine precision, and the world's own debug audit ran on every tick
of this run.

### B6b — reproduction on a renewing patch (7 × 7 bright)

| measure | value |
| --- | --- |
| births | 8 |
| deaths | 10 starvation |
| peak population | 10 |
| extinct | tick 22,199 (1,110 s) |
| intake | leaf 35.367 m, fruit 2.556 m |
| plant income over the run | 32.331 m |
| cumulative `light_in − heat_out` | −65.50 e |
| §11 ratio | 49 × 6.4e-4 = 3.14e-2 m/s against 1.4e-2 for two cruising grazers = **2.24** |

Births happened, as expected. They were followed by starvation to extinction, which §13.2 says
is measured rather than assumed — so this is the measurement, and it is the same shape as
B1b-1: a production surplus of 2.24× did not keep a population alive, because the mouths meet
stocks rather than production.

### B7 — establishment

| donor | recipient | established | recipient at horizon |
| --- | --- | --- | --- |
| B0 measured | bright (L·μ 0.60) | censored | W 0.0000 |
| B0 measured | dim (L·μ 0.20) | censored | W 0.0000 |
| §11 hand table | bright (L·μ 0.60) | 22,912 (1,146 s) | W 0.0290 P 0.0337 Q 0.0100 |
| §11 hand table | dim (L·μ 0.20) | censored | W 0.0056 P 0.0056 Q 0.0028 |

**B7-1.** With a §11 donor the bright cell establishes at 1,146 s, not the ~5 min §11 predicted.
§11's estimate assumes the donor spends `k_est · dt` every tick; the measured donor spends only
what its reserve holds above `q_prop · Q_max`, which recovers slowly, so the transfer is
intermittent. With a B0 donor nothing is sent at all (finding B0-1).

**B7-2.** Neither dim recipient died back inside the horizon, so §13.2's "the dim cell
establishes and is expected to die back" is **not observed**: the dim cell did not finish
establishing either. Both are reported as censored rather than resolved.

## Contract clauses that had to be interpreted

Each of these was ambiguous or under-determined in the contract; none of them is a change to a
§4–§10 equation, a §4.0 read/write, a §11 value or a §15 rule.

1. **"Mature stand" after B0 (§13 vs §13.2).** §13 defines a mature stand as "the §11
   steady-state values of that class painted at tick 0" and, two sentences later, says B0's
   "measured values become the 'mature stand' every later scenario is judged against". Under
   §11's provisional numbers those are different stands in the one stock §4.8 reads. **Taken
   both ways**: B3, B4b and B7 run one arm from each, clearly labelled, and both are reported.
   B1a, B1b, B2, B4a, B6a and B6b paint B0's measured values, as the handoff directs.
2. **Whether §4.8 runs during B0.** §13.2's B0 is "one lone stand … no animals", and §13 says
   the world is "stripped to the named cells" — but a bare cell is still a legal propagule
   recipient. Propagules were left **on**: disabling a §4 mechanism for a baseline would make
   B0 measure something other than the model. In the event the question was moot — a B0 stand
   never clears the donor reserve floor, so nothing was ever sent.
3. **The cell class boundary at `W_min = 0`.** §3.1 defines alive as `W⁻ ≥ W_min`, bare as
   `W⁻ = 0`, and establishing as `0 < W⁻ < W_min`; at `W_min = 0` the first and third overlap.
   Read as: **bare** is `W⁻ = 0`, **alive** is `W⁻ > 0 and W⁻ ≥ W_min`, **establishing** is the
   rest. At the §11 value `W_min = 0.02` this is the contract's own partition exactly.
4. **3c on a frozen cell.** §4.0 marks ripening and drop "all" cells, while §3.1 freezes an
   establishing cell against "income, maintenance, growth, senescence, dieback, death" —
   ripening is not in that list. Implemented as the table says: 3c runs on every cell.
   Unreachable in practice, since a propagule's foliage is orders of magnitude below
   `fruit_min · P_max`.
5. **Two sequential `e_d_max` clamps in 3b and 3c.** §4.5 clamps litter energy at `e_d_max·D²`
   and §4.3's drop clamps again at `e_d_max·D³`. Because `D³ ≥ D²`, clamping twice dumps
   slightly more heat than one combined clamp would. Implemented **as written** (sequential),
   which is the conservative reading.
6. **`e_v` and `e_p`.** §11 makes `e_v` "one density for all plant tissue", equal to today's
   `e_p`. Two config knobs for one quantity is a defect in an accounting identity, so
   `ProducerConfig.energy_density` was **removed** and `PlantConfig.energy_density` is the one
   `e_v` for `P`, `W`, `Q` and `Wd`. `params.rs`'s exclusion list names `plant.energy_density`
   now; §15.3's promise that "`params.rs` names stay valid" holds for every *searched*
   parameter, which this never was.
7. **`Q_0`.** §11 gives the initial reserve as `0.5 · Q_max0`, and §14 fixes `PlantConfig` to
   fifteen named fields with no room for that fraction. It is a documented module constant,
   `fields::INITIAL_RESERVE_FRACTION`, not a new knob.
8. **Where `EcologyV1State` is defined.** §14 puts it in `world/state.rs`. The **field** on
   `WorldState` is there; the **type** is in `fields.rs` beside the subphases that operate on
   it, and re-exported as `cubarium_core::world::EcologyV1State`. Same wire shape, same
   trailing position.
9. **"Feeding threshold per stock" (§14, controller).** Read as the existing `feed_min` gate
   applied per channel, with the detrital channel reading `D_eff + C_eff` — which is what §6.2
   and §9 describe. No new threshold was added.
10. **A hunter's carried carcass on its death.** §8 says "a hunter member's death goes to `C`"
    and §5 lists "a hunter member's death" among `C`'s sources, without separating the body
    from the gut. Both go to `C`: a carcass a hunter was carrying is a body, not plant litter.
    Its digestion *rejects* go to `D`, as §8 says explicitly.
11. **`ecology_hash`.** §15.1 says to keep it "only if the care/no-care comparison still uses
    it". It does (`tests/care_replay.rs`, `examples/care_compare.rs`, `quiet_compare`,
    `hunter_compare`, `Telemetry`), so it is kept — with the consequence, named here, that the
    schema 7 projection cannot cover the ecology v1 pools, so two worlds differing only in wood
    now hash alike under `ecology_hash`. `state_hash` covers everything and is what every
    replay check in the suite uses.

## §11 values `validate` forced to change

**None.** Every §11 provisional value is the shipped default and every one of them is admitted
by `WorldConfig::validate`. The new validations (`rate · dt ≤ 1` by name for `m_w`, `m_p`,
`ripen`, `drop`, `k_d`, `k_c`, `k_w`, `fall`; `alive_min < donor_min ≤ wood_max`;
`propagule_split` summing to 1; `capability_gate ∈ [0, 0.5]`; `capability_exponent > 0`) all
pass at the §11 table with room to spare.

## Old-schema refusal tests, and the retired continuations

Schema 16 refuses every schema from 7 to 15 by name (`SnapshotError::UnsupportedSchema`). The
`From<WorldStateVn> for WorldState` conversions and `v10::migrate` are **deleted**; the frozen
mirror shapes, their `SCHEMA_Vn` constants and the `project` functions remain, because the
refusal tests and `ecology_hash` still use them.

Refusal tests, each asserting the named version on the real recorded bytes (and, where the
provenance recorded one, each file's own payload hash first):

| test | fixtures |
| --- | --- |
| `ecology_v1::a7_schema_sixteen_round_trips_and_every_older_schema_is_refused_by_name` | the rule itself, schemas 7–15 |
| `snapshot::tests::every_older_schema_is_refused_by_name` | the rule, in-crate |
| `snapshot::tests::a_relabelled_schema_sixteen_payload_is_refused` | trailing-byte relabel |
| `snapshot_hardening::the_schema_version_is_sixteen_and_every_predecessor_is_refused` | every version 0–16 |
| `continuation_fixtures::every_retired_fixture_is_present_and_refused_by_name` | all 25 `.cubw` files |
| `care::the_live_schema_seven_fixtures_are_refused_by_name` | `live-v7-55200*` |
| `energy_correction::the_pre_correction_fixtures_are_refused_by_name` | `live-v7-55200`, `live-v8-172800*` |
| `hunter_migration::the_pre_hunter_schema_nine_fixtures_are_refused_by_name` | `pre-hunter-v9-173400*` |
| `hunter_migration::every_schema_ten_payload_is_refused_by_name_whatever_it_carries` | hand-framed schema 10 |
| `care_dose_migration::the_pre_dose_schema_eleven_fixtures_are_refused_by_name` | `care-v11-*` |
| `quiet_migration::the_pre_quiet_schema_twelve_fixtures_are_refused_by_name` | `quiet-v12-*` |
| `astra_quiet_policy::the_schema12_quiet_fixtures_are_refused_by_name` | `quiet-v12-*` |
| `hunter_charging::the_pre_change_charge_fixtures_are_refused_by_name` | `hunter-v3-charge-*` |
| `astra_target_cleanup::the_retained_seed6_artifacts_are_refused_by_name` | the seed-6 capture |
| `apex_dormancy::dormant_state_round_trips_and_schema_thirteen_is_refused` | hand-framed schema 13 |
| `neural_runtime::a_hand_framed_schema_fourteen_payload_is_refused_by_name` | hand-framed schema 14 |

Retired continuation comparisons — the load-and-step-600 checks and the `*-plus600-r0b` /
`-r0d` recordings that anchored them. **No fixture file was deleted or regenerated**; each
provenance note records the retirement.

| retired comparison | fixtures | provenance note |
| --- | --- | --- |
| schema 7 care migration + continuation | `live-v7-55200*` | `live-v7-v8-provenance.md` (new) |
| schema 8 correction migration + continuation | `live-v8-172800*` | `live-v7-v8-provenance.md` (new) |
| schema 9 empty-hunter continuation | `pre-hunter-v9-173400*` | `pre-hunter-v9-provenance.md` |
| schema 11 pre-dose shower and hunter continuations | `care-v11-*` | `care-v11-provenance.md` |
| schema 12 pre-quiet plain and care continuations | `quiet-v12-*` | `quiet-v12-provenance.md` |
| schema 12 profile-3 charging continuation | `hunter-v3-charge-*` | `hunter-v3-charge-provenance.md` |
| the schema 12 seed-6 exact artifact replay | `captures/…/seed-6/` | retirement recorded in the test |
| the whole regenerator (`continuation_fixtures::regenerate`) | all of the above | the file's own header |

Three claims those tests carried were **rebuilt on worlds this build makes** rather than
retired, because they were never about a particular recording: the schema 7/8/9 projections are
still checked as strict prefixes of the schema 16 payload
(`hunter_migration::the_older_projections_still_drop_only_what_they_are_named_for`); the
charging policy's candidate band is still separated by a hand-built member
(`hunter_charging::a_member_inside_the_band_separates_the_two_policies`); and the quiet pause
suite now warms its own mature world for 6,000 ticks instead of loading the schema 12 fixture.

## The runtime edits, and nothing else

Exactly the four §15.2 edits were made in `neural/`:

1. `PROFILE_TEXT` gains `|eco:v1`, so the schema digest changes and every existing policy file,
   every `runs/es-*` centre and the R3a display seed is refused by name. No `runs/es-*` campaign
   is resumable and the R2b/R2c/R2d results are not evidence about this ecology.
2. `Capability` carries `cap_foliage` and `cap_detrital` instead of `diet`; the graze and fruit
   masks open on `cap_foliage > 0`, scavenge on `cap_detrital > 0`.
3. `Feedback::channels` normalises all three `ate` channels by the one `mouth_rate`.
4. The observation sampler's `d_here` and `food_*` detrital channels read `D_eff + C_eff`.

The observation layout, the action layout, `Gru32`, the optimizer, the trainer loop, the export
format and the motor contract are untouched. `GRU_PARAMETERS` is still 10,215 and
`tests/neural_runtime.rs` still passes unchanged apart from the retired schema 14 migration.

## Other consequences worth naming

- **A fresh world starts with far less foliage.** `P_0` is now `initial_fraction · min(P_max,
  α · W_0)` rather than `initial_fraction · P_max · L₀ · μ₀`; the brightest cell opens near
  `P = 0.24` instead of `0.60`, which is below the `fruit_min · P_max = 0.45` ripening
  threshold. A default world therefore holds **no fruit at all** after 100 s, where the
  pre-ecology-v1 world did. Two tests that asserted "a default world ripens some fruit in
  100 s" now assert zero and exercise ripening on a world rich enough to do it.
- **`mass_residual` tolerances moved from 1e-12 to 1e-9.** The identity now sums eight 1,280-term
  vectors instead of four, and the extra summation rounding is ~1e-12 absolute on a world
  holding ~660 m. 1e-9 is the contract's own A1 bound (§13.1).
- **Telemetry consumers** that summed `detritus` as "all dead matter" now need
  `detritus + carrion + dead_wood` (§15.3). `Telemetry` gains `wood`, `plant_reserve`,
  `dead_wood`, `carrion`, `carrion_energy`, `bare_cells`, `establishing_cells`, `plant_deaths`
  and `recolonisations`.
- **`RenderView` and `FieldDump` carry every new pool** and nothing draws them. The presentation
  task of §12 needs no further core change.
- **`Layout::build` in `cubarium-search`** paints `W = P/α` and `Q = q_cap·W` with its foliage,
  and `painted_material` counts all three. The layout and protocol hashes change by
  construction; the optimizer, trainer loop and export format are untouched.

## Open findings, in the order a reviewer should weigh them

1. **B0-1 — the plant reserve never refills** under §11's values, so no simulator-grown stand is
   ever a donor and §4.8 never fires outside a hand-painted fixture. This is §4.4's allocation
   order meeting §11's rates; it is not a coding defect and was not tuned away.
2. **B1b-1 — bright coexistence failed**, which §13.2 explicitly names as a finding. One grazer
   emptied a 25-cell bright region whose production exceeds its cruising need by 2.3×.
3. **B6b — a 2.24× production surplus still ended in extinction**, the same shape as B1b-1.
4. **B1a-3 / B4a — stand death is far slower than the 30-minute horizon** at `κ = 1` and
   `m_w = 2e-4`: a stripped stand's dieback needs ~10⁵ ticks to carry `W` to `W_min`.
5. **B7-1 — establishment beside one §11 donor takes 1,146 s, not §11's ~300 s**, because the
   donor's spend is limited by its reserve above the floor rather than by `k_est`.
6. **B7-2 — the dim recipient neither established nor died back** inside the horizon; both are
   censored, so §13.2's expectation there is untested rather than contradicted.
7. **`ecology_hash` no longer covers the ecology.** Kept because the care/no-care comparison
   uses it (§15.1), but a reader should not treat it as an ecological identity any more.

None of these was tuned. Every one is a measurement at the contract's own §11 values, which
WORKING_POLICY 2026-09-14 says are to be tuned together later, not one at a time here.

## Stop

Held-out checks, training in the revised ecology, display deployment and the §12 presentation
task are separate assignments after the review.
