---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 implementation review

*Astra (GPT via the Codex plugin), 2026-09-15. Saved verbatim by Fable because
the Codex sandbox was read-only; Astra's material unit "kg" below means the
contract's `m`.*

**Disposition — repair required before acceptance.** The material and energy equations are substantially faithful, but subphase 3h uses the wrong recipient snapshot, `ecology_hash` cannot see ecology-v1 state, and several acceptance scenarios and result-note pass claims do not measure what the contract specifies.

Reviewed `design/ecology-v1-contract.md`, the Opus handoff and repair-cycle brief, the implementation result note, commits `b1dd394`, `6d304f1`, `3316980`, and `26c382e`, and the named implementation, test, example, compatibility, and fixture files. The owner rules are retained: worlds restart fresh, old policies need not reload, no parameter changes belong to this milestone, and presentation is separate. The four requested Cargo commands were attempted, but the execution environment remained read-only; finding 9 records the exact failures. No training or live-display action was performed.

## 1. P1 — Subphase 3h tests recipient eligibility against the pre-tick class, not post-3d wood

The §4.0 table requires 3h to read recipient `W⁴`, after mortality in 3d (`design/ecology-v1-contract.md:120-140`). The implementation computes `work.class` once from `w0` before the subphases (`crates/cubarium-core/src/fields.rs:362-383`) and uses that class for both propagule recipient filters (`crates/cubarium-core/src/fields.rs:633-638`, `crates/cubarium-core/src/fields.rs:654-660`). A cell with `W⁰ >= W_min` that dies in 3d has `W⁴ = 0` (`crates/cubarium-core/src/fields.rs:514-532`) but remains ineligible for establishment until the following tick. That differs from the specified named state and can change the result even though iteration remains deterministic.

The rest of the ordering is faithful: 3a-3d use the pre-tick plant snapshots, 3e and 3f compute withdrawals from their pre-subphase stocks and use current-stock energy densities (`crates/cubarium-core/src/fields.rs:396-605`), 3g transports a snapshot (`crates/cubarium-core/src/fields.rs:609-613`), and 3h snapshots donor reserve after 3d and commits the accumulated transfers together (`crates/cubarium-core/src/fields.rs:615-689`). The identified stale recipient class is the iteration-sensitive exception.

**Repair:** derive 3h recipient eligibility from the post-3d `W⁴` snapshot and add a regression in which a same-tick plant death becomes an eligible recipient. Preserve snapshot allocation and joint commit so donor/recipient iteration order cannot affect the result.

## 2. P1 — `ecology_hash` omits every ecology-v1 stock and counter

`WorldState` appends foliage, wood, reserve, fruit, litter, carrion, and ecology counters (`crates/cubarium-core/src/world/state.rs:83-98`), but `ecology_hash` hashes the schema-7 projection only (`crates/cubarium-core/src/snapshot.rs:180-185`). Consequently two schema-16 worlds can have the same hash while differing in every new field. The care-replay check relies on this hash (`crates/cubarium/tests/care_replay.rs:112-154`), so a replay can falsely pass after ecology state diverges. This is a contract defect as well as an implementation gap: §15 retained the old care-neutral hash without specifying a schema-16 projection (`design/ecology-v1-contract.md:708-752`).

**Repair:** define and implement a current-schema, care-neutral deterministic hash that includes all ecology-v1 stocks and counters, then make replay tests prove that perturbing each included ecology field changes the hash. This does not require a migration path or old-policy compatibility.

## 3. P2 — B0, B1b, B6b, and B7 do not measure their stated acceptance claims

- B0 is described as a lone-stand drift comparison, but the scenario leaves propagules enabled (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:347-380`). The recorded run reports `0.166 kg` sent by the bright stand (`design/7_Research/ecology-v1-implementation-2026-09-15.md:58-66`), whereas the §11 hand table has no propagule sink (`design/ecology-v1-contract.md:536-556`). R2-1 is therefore a **scenario-design defect**, not evidence of reaction arithmetic drift. The result note also reports average reserve exactly full and then says neither class filled its reserve (`design/7_Research/ecology-v1-implementation-2026-09-15.md:63-75`).

- B1b records `25 * stand.material()` as “painted” (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:598-610`), and `material()` is `P+W+Q+F`, not foliage (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:192-194`). The note labels those values `ΣP painted` (`design/7_Research/ecology-v1-implementation-2026-09-15.md:94-99`). Recomputed opening foliage is `25*0.4695 = 11.7375 kg` bright and `25*0.0977 = 2.4425 kg` average, not `23.037` and `6.381`. The reporting error is a **scenario-design defect**; the observed bright coexistence failure and 18 dead stands, once measured correctly, are a **later whole-ecosystem tuning question**, not a demonstrated implementation defect.

- B6b accumulates births, deaths, population, foliage, intake, income, and a static two-body ratio, but never records actual total upkeep or the doubling-time/recovery comparison required by §13 (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:1214-1308`; `design/ecology-v1-contract.md:677-679`). From the recorded figures, `34.319/1800 = 0.01907 kg/s`, only `1.36x` the stated `0.014 kg/s` initial two-body need, not `2.24x`; population also peaks at 8 (`design/7_Research/ecology-v1-implementation-2026-09-15.md:185-197`), invalidating a fixed two-body denominator. B6b extinction “despite a 2.24x surplus” is a **scenario-design defect**, not a verified tuning or implementation result.

- B7 paints a nominally bright donor into a world whose light is uniformly `0.4` (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:1332-1344`). B7-2's dim-recipient outcome cannot separate donor illumination from recipient illumination and is a **scenario-design defect**.

**Repair:** isolate B0 from propagule loss and regenerate every B0-derived comparison; report B1b foliage separately from total plant material; instrument B6b with time-varying total upkeep, surplus, doubling time, and recovery time; and give B7 independently controlled bright-donor and dim-recipient environments. Do not tune parameters in this repair.

## 4. P2 — Several A1-A9 tests can pass materially incorrect implementations

A1 checks aggregate residuals and invariants only (`crates/cubarium-core/tests/ecology_v1.rs:287-315`), so paired omissions or the same wrong stock included on both sides can pass. A3b tests only the coincident endpoint `k_d*dt = fall*dt = 1` (`crates/cubarium-core/tests/ecology_v1.rs:739-777`), where correct joint withdrawal makes fall zero; an implementation that always suppresses fall whenever decomposition is nonzero could pass. Its senescence/ripening endpoint checks only final foliage and aggregate invariants (`crates/cubarium-core/tests/ecology_v1.rs:779-793`). A4 exercises a leaf bite only and asserts a heat lower bound (`crates/cubarium-core/tests/ecology_v1.rs:856-928`), so fruit, litter, or carrion energy mistakes can pass. A6's hunter-death assertion only requires carrion to be at least body mass and does not assert the post-death residual (`crates/cubarium-core/tests/ecology_v1.rs:1198-1220`), so discarding gut contents can pass. A8 is determinism-only (`crates/cubarium-core/tests/ecology_v1.rs:1397-1409`). A9 uses one donor and recipients that begin bare (`crates/cubarium-core/tests/ecology_v1.rs:1447-1568`), so it misses both same-tick death eligibility and multi-donor proportional allocation.

Within their narrower setups, A2a (`crates/cubarium-core/tests/ecology_v1.rs:487-527`), A5 (`crates/cubarium-core/tests/ecology_v1.rs:938-1071`), A7 (`crates/cubarium-core/tests/ecology_v1.rs:1277-1323`), and A9's establishment equation do directly test contract claims. The suite as a whole nevertheless tests important parts of the implementation's own reading rather than all §13 claims.

**Repair:** add stock-by-stock A1 assertions around every subphase; an interior joint-withdrawal case where both decomposition and fall are nonzero; exact material-and-energy assertions for all four foods; exact ordinary death, miscarriage, hunter body/gut, reject, and feces destinations; and A9 cases for same-tick death plus multiple donors and recipients.

## 5. P2 — The result note presents unmet expectations as resolved passes

B1a is described as directionally unchanged even though the zero-reserve stand dies and the reserve-saturated stand is censored, which does not establish the required ordering “reserve saturates first” before death (`design/7_Research/ecology-v1-implementation-2026-09-15.md:78-92`; `design/ecology-v1-contract.md:669-671`). B3 is called resolved even though the B0 `0.9` arm is censored and the hand-start `0.9` arm takes 1288 seconds against the contract's approximately 600-second expectation (`design/7_Research/ecology-v1-implementation-2026-09-15.md:123-136`; `design/ecology-v1-contract.md:672-674`). B4b calls a 132-second result close to “within a minute,” but 132 seconds exceeds 60 (`design/7_Research/ecology-v1-implementation-2026-09-15.md:155-164`; `design/ecology-v1-contract.md:674-675`). The retained B5 evidence says every skimmer died while reframing “lives” as drawing on carrion (`design/7_Research/ecology-v1-implementation-2026-09-15.md:353-374`; `design/ecology-v1-contract.md:676`). B6a records cumulative net energy, not per-tick monotonicity, and does not demonstrate every escrow debit; the note nevertheless claims both (`design/7_Research/ecology-v1-implementation-2026-09-15.md:172-183`, `design/7_Research/ecology-v1-implementation-2026-09-15.md:376-391`). A2b does not assert births, and `plant_deaths_total > 0 || tick > 0` becomes tautological after the first tick (`crates/cubarium-core/tests/ecology_v1.rs:616`).

These are expectation-versus-measurement and reporting defects. The raw measurements may remain useful, but they do not support the pass/resolved wording.

**Repair:** after repairing the scenarios and tests, restate each result against the literal §13 criterion; mark censored, missing, or contrary measurements as unresolved rather than passes.

## 6. P2 — Search parameter metadata still advertises removed `producer.energy_density`

Moving energy density from `ProducerConfig` to per-stock `PlantConfig` is acceptable under the fresh-world and old-policy owner rules. The types and validation implement that model (`crates/cubarium-core/src/config.rs:90-103`, `crates/cubarium-core/src/config.rs:140-154`, `crates/cubarium-core/src/config.rs:870-878`), and no configuration TOML reference to the removed key was found. However, the supposedly exhaustive compatibility claim in the result note (`design/7_Research/ecology-v1-implementation-2026-09-15.md:467-472`) is contradicted by `crates/cubarium-search/src/params.rs:155-160`, which still lists `producer.energy_density` as a search parameter. A tool can therefore emit or present an invalid schema-16 key.

Schema refusal itself is correct: schema 16 is the only accepted version and 7-15 have no migration path (`crates/cubarium-core/src/snapshot.rs:127-173`). The four permitted neural changes are confined to the profile tag (`crates/cubarium-core/src/neural/mod.rs:33-49`), capability masks (`crates/cubarium-core/src/neural/action.rs:32-65`, `crates/cubarium-core/src/neural/action.rs:98-105`), mouth-rate-normalized ate feedback (`crates/cubarium-core/src/neural/state.rs:38-77`), and `D_eff + C_eff` observation sampling (`crates/cubarium-core/src/world/step.rs:33-45`, `crates/cubarium-core/src/world/step.rs:2663-2671`, `crates/cubarium-core/src/world/step.rs:2801-2812`, `crates/cubarium-core/src/world/step.rs:2834-2838`). The reviewed commit diff contains no optimizer, trainer, or exporter edit. The ES fixture paints wood and reserve as required (`crates/cubarium-search/src/es/fixture.rs:108-117`, `crates/cubarium-search/src/es/fixture.rs:150-166`, `crates/cubarium-search/src/es/fixture.rs:213-269`).

**Repair:** remove or replace the stale search parameter metadata and add a schema-16 parameter-list/config-validation check. No snapshot migration or old-policy loader is required.

## 7. P3 — The implemented §3-§10 ledgers otherwise balance, including all four bite types

Independent recomputation supports the local identities. In 3a, each reserve-funded tissue increment withdraws `(1+c_g)Δ`, creates `Δ` tissue, and returns `c_gΔ` to mineral nutrient; maintenance similarly transfers paid reserve to nutrient (`crates/cubarium-core/src/fields.rs:396-480`). The 3b-3d transfers are one-for-one (`crates/cubarium-core/src/fields.rs:482-532`). For joint 3e/3f withdrawal, the total fraction is `k_d dt + fall dt(1-k_d dt) <= 1`; at `k_d dt = fall dt = 1`, decomposition takes all and fall takes zero, including carrion (`crates/cubarium-core/src/fields.rs:535-605`). In 3h a donor debit `S` produces `S/(1+c_g)` tissue plus `c_g S/(1+c_g)` nutrient, and the code splits tissue by the configured fractions while incrementing establishment once (`crates/cubarium-core/src/fields.rs:615-689`). This verifies A1 and A3b algebraically apart from finding 1's recipient-state error; A9 arithmetic is correct for the one-donor case tested.

For each fruit, leaf, litter, and carrion bite, let `q_d = cap*q`, `a = eta'_m*q_d`, and feces be `(1-eta'_m)q_d + (1-cap)q`. Then `a+feces=q`. With `spare = rho*q_d - e_r*a`, the energy outputs are `e_r*a + G + (spare-G) + rho(1-cap)q = rho*q`. The four branches implement those same terms (`crates/cubarium-core/src/world/step.rs:1591-1697`): undigested material goes to litter and its energy goes to heat. Shared mouth-rate normalization and D:C effective-density sampling are explicit (`crates/cubarium-core/src/world/step.rs:1490-1567`). Ordinary death and miscarriage route plant/body/reserve material to carrion (`crates/cubarium-core/src/world/step.rs:2104-2138`), hunter body and gut contents route to carrion (`crates/cubarium-core/src/world/step.rs:2188-2200`), hunter rejects route to litter (`crates/cubarium-core/src/world/step.rs:1743-1762`), and all bite feces route to litter. Separate D and C withdrawals prevent cross-stock decomposition leakage (`crates/cubarium-core/src/fields.rs:535-605`). The invariant ledger includes the new material and stored-energy stocks (`crates/cubarium-core/src/world/invariants.rs:18-40`, `crates/cubarium-core/src/world/invariants.rs:121-161`).

For A2a, the only field light-to-energy paths are growth and fruit ripening (`crates/cubarium-core/src/fields.rs:401-414`, `crates/cubarium-core/src/fields.rs:496-502`); with `g=0` and `ripen=0`, both are disabled. No contrary path was found in the reviewed reaction code.

**Repair:** none for these equations; retain them while applying findings 1-6 and strengthening their direct tests.

## 8. P3 — The remaining open items are scenario defects or later tuning questions, not demonstrated implementation defects

The eight result-note open items classify as follows, based on the cited measurements and code:

1. B1b-1 bright coexistence failure and 18 stand deaths: **later whole-ecosystem tuning question**, after correcting B1b reporting (`design/7_Research/ecology-v1-implementation-2026-09-15.md:94-105`).

2. B6b extinction despite a claimed 2.24x surplus: **scenario-design defect** because actual upkeep and recovery time were not measured (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:1214-1308`).

3. R2-1 B0 drift: **scenario-design defect** because propagule export is enabled while absent from the hand table (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:347-380`).

4. R2-2 B2 no recovery: **scenario-design defect** because the grazer dies before the return phase, so recovery-before-return is not exercised (`design/7_Research/ecology-v1-implementation-2026-09-15.md:109-121`).

5. B3 average censored: **later whole-ecosystem tuning question**; censoring is an ecological outcome, not evidence of code nonconformance (`design/7_Research/ecology-v1-implementation-2026-09-15.md:123-136`).

6. B7-2 dim recipient: **scenario-design defect** because donor and recipient light are not independently controlled (`crates/cubarium-core/examples/ecology_v1_scenarios.rs:1332-1344`).

7. `ecology_hash`: **contract defect**, with a corresponding implementation/test repair, because the retained schema-7 projection excludes schema-16 state (`crates/cubarium-core/src/snapshot.rs:180-185`).

8. R2-4 `p_reflush*alpha = q_cap`: **later tuning question**. The equality is real (`0.25*2 = 0.5`) and full reserve buys only `Q/(1+c_g)` tissue, but no implementation deviation follows from that coincidence (`design/ecology-v1-contract.md:177-206`).

No parameter value is proposed for any tuning item.

**Repair:** repair only the scenario and hash defects identified above; carry the three tuning questions forward to the later whole-ecosystem search without changing parameters in this milestone.

## 9. P2 — Required executable verification was blocked

All four permitted commands were attempted on 2026-09-15 and exited 101 before compilation or execution:

```text
cargo test -p cubarium-core
error: failed to open: /home/wrysk/wryskware/cubarium/target/debug/.cargo-build-lock
Caused by: Read-only file system (os error 30)

cargo test -p cubarium-search
error: failed to open: /home/wrysk/wryskware/cubarium/target/debug/.cargo-build-lock
Caused by: Read-only file system (os error 30)

cargo test -p cubarium
error: failed to open: /home/wrysk/wryskware/cubarium/target/debug/.cargo-build-lock
Caused by: Read-only file system (os error 30)

cargo run -p cubarium-core --release --example ecology_v1_scenarios -- all
error: failed to open: /home/wrysk/wryskware/cubarium/target/release/.cargo-build-lock
Caused by: Read-only file system (os error 30)
```

**Not verified: cargo blocked.** Current-build test and scenario behavior was not reproduced in this review. Source inspection and independent recomputation support findings 1-8, but the retained run tables were not independently verified.

**Repair:** rerun these exact four commands in a genuinely write-capable workspace and attach their summaries to the repaired result note; address any new failure before acceptance.

## Fable repair handoff

1. Fix 3h recipient eligibility to use post-3d `W⁴`; add same-tick-death and multi-donor/recipient coverage while preserving snapshot allocation and joint commit.

2. Define and implement a current-schema care-neutral `ecology_hash` covering every ecology-v1 stock and counter, with perturbation tests.

3. Isolate B0 from propagule export and regenerate B0-derived comparisons; correct B1b foliage reporting.

4. Make B6 measure actual time-varying upkeep, surplus, doubling time, and recovery; make B7 control donor and recipient illumination independently.

5. Strengthen A1/A3b/A4/A6/A9 with the direct stock, intermediate-endpoint, four-food energy, remains-routing, and multi-donor/new-death assertions identified above.

6. Remove or replace the stale `producer.energy_density` search parameter metadata.

7. Rerun the four required Cargo commands in a write-capable workspace and correct the result note so every pass/resolved claim matches the literal measurement and expectation.

*Fable's note on finding 9: the four commands were run by Fable in this
workspace after both worker commits (core 461 / search 66 / host 569 passed,
0 failed) and are re-run again in repair cycle 2; the sandbox limitation was
Codex's, not the repository's.*
