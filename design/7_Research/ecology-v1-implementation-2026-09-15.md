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

**Two runs are recorded.** Run 1 implemented §4.4 as it stood and measured finding B0-1 — the
plant reserve could never persist. Fable fixed that in the contract (`3316980`: a `q_share` of
every surplus to the reserve first, reflush only below `p_reflush · P_cap`), and **repair cycle
1** implements the revision and re-measures. Run 2's tables are the current ones; run 1's are
kept below so the difference is visible rather than described.

## Build and commit

- Baseline `4283040` on `main`. Run 1 is the code commit `b1dd394` and the note `6d304f1`.
- **Repair cycle 1** follows contract commit `3316980`, which rewrote §4.4 after run 1's
  finding B0-1, added the §11 values `q_share` 0.2 and `p_reflush` 0.25, added the config
  fields `reserve_share` and `reflush_below` to §14, corrected B4a's expectation, and accepted
  all eleven of run 1's interpretations (§18, third round).
- `graft build` refreshed after each change.
- Schema 16, config version 8. No migration path exists anywhere in the tree.

## Verification (after repair cycle 1)

```bash
cargo test -p cubarium-core      # 461 passed, 0 failed, 2 ignored
cargo test -p cubarium-search    #  66 passed, 0 failed
cargo test -p cubarium           # 569 passed, 0 failed, 16 ignored
cargo run -p cubarium-core --release --example ecology_v1_scenarios -- all   # 63 s wall
```

All three suites are green. The two ignored core tests and the sixteen ignored host tests are
pre-existing (`#[ignore]` fixture regenerators and capture-writing art studies), untouched here.

The accounting tests A1–A9 are `crates/cubarium-core/tests/ecology_v1.rs`, now with three more
A1 arms and a three-band A2b fixture covering the repaired §4.4 branches (a share taken, a full
reserve taking none, a reflush gated and capped), plus one dedicated unit test,
`the_reflush_threshold_and_the_reserve_share_are_the_repaired_allocation`, which proves that a
stand at or above `p_reflush · P_cap` never draws its reserve for foliage. A green suite is
necessary and not sufficient; the review re-derives A1, A2a, A3b, A6 and A9 independently.

## Run 2 — what the scenarios measured after the repaired §4.4

This is the current measurement. Run 1's tables are kept below, under their own heading, so the
change the repair made is visible rather than described.

### B0 — stand baseline (the anchor)

| class | P | W | Q | F | Q_max | dP/dt | dW/dt | propagules sent | §11 hand P/W/Q |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| average | 0.0977 | 0.1050 | **0.0525** | 0.0000 | 0.0525 | 2.5e-5 | 0.0 | 0.0000 | 0.50 / 0.33 / 0.16 |
| bright | 0.4695 | 0.3578 | **0.0894** | 0.0047 | 0.1789 | 1.7e-4 | 1.5e-4 | **0.1660 m** | 0.56 / 0.60 / 0.30 |

**The repair works.** The average stand's reserve is now **exactly full** (`Q = q_cap · W`) and
the bright one is half full and still rising, against run 1's 0.0000 and 0.0435. The bright
stand crossed the §4.8 donor threshold and **sent 0.166 m of reserve to its bare neighbours** —
in run 1 nothing was ever sent by any simulator-grown stand. B0-1 is resolved.

**Finding R2-1, the cost of the share: B0 is now *further* from the §11 hand table, not
closer.** Bright `ΔP` went from −0.0422 to −0.0905 and `ΔW` from −0.1125 to −0.2422; average
`ΔP` went from −0.3232 to −0.4023 and its wood did not grow at all (`W` is still exactly the
seed `W_0 = 0.105`, `dW/dt = 0`). That is what `q_share = 0.2` costs at a 30-minute horizon: a
fifth of every surplus now goes to a stock the hand table assumed was already full, so foliage
and wood accumulate more slowly. §11 predicts this in words — "`P*` sits a few hundredths
lower until the reserve is full" — and the measured gap is larger than "a few hundredths"
because neither class finished filling inside the horizon. Both classes are still climbing;
this is a finite-horizon baseline, not equilibrium.

### B1a — one pinned grazer, one bright mature stand, 12,000 ticks

| measure | run 2 | run 1 |
| --- | --- | --- |
| leaf / fruit bitten | 0.5473 / 0.0035 m | 0.5672 / 0.0504 m |
| feces | 0.2699 m | 0.3026 m |
| mean bite rate over its life | 2.31e-3 m/s (3.6×) | 2.43e-3 m/s (3.8×) |
| reserve-saturated ticks | 0 of 2,980 | 0 of 2,108 |
| stand at horizon | P 0.0006, W 0.3275, Q 0.0000, Wd 0.0294 | P 0.0006, W 0.4414, Q 0, Wd 0.0445 |
| stand death | censored | censored |
| grazer starved | 4,769 (238 s) | 5,082 (254 s) |

Unchanged in direction. The fruit intake fell tenfold because the B0 bright stand now carries
`F = 0.0047` instead of 0.0544 — foliage reaches the `fruit_min · P_max` ripening threshold
later when a fifth of the surplus is going to the reserve.

### B1b — one mobile legacy grazer on a 5 × 5 mature region

| class | cells visited | Σ P painted → final | min stand P | stand deaths | grazer |
| --- | --- | --- | --- | --- | --- |
| bright | 66 | 23.037 → **0.0140** | 0.0004 | **18** | starved 21,877 (1,094 s) |
| average | 44 | 6.381 → **0.0460** | 0.0004 | 0 | starved 9,313 (466 s) |

**Finding R2-2: the repair did not fix B1b-1, and the bright arm got worse.** The grazer still
emptied the region — and now killed 18 of its 25 stands, where run 1 killed none. The reason is
visible in B0: a stand that spends a fifth of its surplus on reserve carries less wood
(`W` 0.358 against 0.487), so the same grazing pressure carries `W` below `W_min` inside the
horizon. The grazer itself lived 20 % longer (1,094 s against 913 s) because the reserve it was
indirectly eating had been stocked. Coexistence with retained foliage is still **not** observed
in the bright class.

### B2 — depletion and relocation, three bright stands three cells apart

| stand | ticks on | min P | Q at departure | P at horizon | Q at horizon |
| --- | --- | --- | --- | --- | --- |
| (5,8) | 1,060 | 0.0003 | 0.0112 | 0.0003 | 0.0000 |
| (8,8) | 969 | 0.0003 | 0.0000 | 0.0003 | 0.0000 |
| (11,8) | 925 | 0.0003 | 0.0000 | 0.0003 | 0.0000 |

41 cells visited; grazer starved at 6,466 (323 s). **Changed direction from run 1.** In run 1
the grazer left each stand at `P ≈ 0.28–0.41` and all three recovered to `P ≈ 0.558`; in run 2
it stripped all three to `P = 0.0003` and **none recovered** by the horizon. The stands are
smaller (B0 bright W 0.358 against 0.487) so each holds less, and the grazer stayed ~1,000 ticks
on each instead of ~300. Recovery-before-return is still untested — the grazer was dead first.

### B3 — plant recovery after defoliation

| arm | 0.5·P* | 0.9·P* | reserve dip → end |
| --- | --- | --- | --- |
| bright, B0-measured | **28,919 (1,446 s)** | censored | 0.0000 → **0.0894** |
| bright, §11 hand | 8,626 (431 s) | 25,755 (1,288 s) | 0.0000 → **0.1500** |
| average, B0-measured | censored | censored | 0.0000 → 0.0000 |
| average, §11 hand | censored | censored | 0.1600 → 0.0000 |

**B3-1 is resolved for bright.** In run 1 the B0-measured bright stand never reached even
0.5·P* and decayed for the whole run; now it recovers to half its foliage in 24 min **and
refills its reserve to 0.0894** on the way. The §11-hand arm also now refills (0.30 → 0 → 0.15)
where run 1 left it at 0.0486. Average is still censored in both arms — see the residual
findings.

### B4a — three pinned grazers on one isolated bright mature stand

| measure | value |
| --- | --- |
| `Q` reached zero | 2,166 (108 s) — run 1: 914 (46 s) |
| dieback opened | 2,167 (108 s) |
| stand death | **censored, as §13.2 now expects** |
| `W` decline since dieback | 0.3580 → 0.2572 m over 1,692 s |
| mean `dW/dt` | 5.955e-5 m/s |
| measured e-folding time | **5,119 s (85 min)** |
| §11 prediction `1/(κ·m_w)` | 5,000 s (83 min) |
| dead wood at horizon | 0.0848 m |
| grazers starved | 8,098 (405 s) ×3 |

The corrected expectation is met precisely: the measured e-fold is within 2.4 % of `κ · m_w`.
The stand also held its reserve 2.4× longer than in run 1 before the grazers emptied it.

### B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | **2,642 (132 s)** | 0.0466 (13.0 % of B0's W) | 0.0588 (12.5 %) |
| the §11 hand table's bright stand | 1,500 (75 s) | 0.0495 (8.3 % of 0.60) | 0.0618 (11.0 %) |

**Resolved.** In run 1 the B0-measured ring never sent anything at all; it now establishes in
132 s, close to §13.2's "within a minute" and to the §11-hand ring's 75 s. Rebuilding is
censored at 30 min in both arms, as expected.

### B5 — dietary exclusion

Unchanged from run 1 to the digit — B5 freezes every conversion and paints its food directly,
so §4.4 never runs in it. The §6.1 exclusions hold exactly: burrower 0.000 on foliage; grazer
and glider 0.000 on litter and on carrion; skimmer eats all three.

### B6a — reproduction on a finite input (3 × 3 bright, renewal off)

| measure | run 2 | run 1 |
| --- | --- | --- |
| births / deaths | 2 / 4 starvation | 2 / 4 |
| peak population | 4 | 4 |
| extinct | 15,232 (762 s) | 12,621 (631 s) |
| plant income over the run | **0.0000 m** | 0.0000 m |
| cumulative `light_in − heat_out` | **−15.42 e** | −16.97 e |

Expected direction met, unchanged: births while the stock lasts, then starvation to zero, with
plant income identically zero and `U` strictly falling.

### B6b — reproduction on a renewing patch (7 × 7 bright)

| measure | run 2 | run 1 |
| --- | --- | --- |
| births / deaths | 6 / 8 | 8 / 10 |
| peak population | 8 | 10 |
| extinct | 28,571 (1,429 s) | 22,199 (1,110 s) |
| plant income | 34.319 m | 32.331 m |
| §11 surplus ratio | 2.24 | 2.24 |

**Finding R2-3: the repair did not fix B6b.** The population still goes extinct, 29 % later
than in run 1 and with two fewer births. A 2.24× production surplus still does not sustain two
grazers.

### B7 — establishment

| donor | recipient | established | recipient at horizon |
| --- | --- | --- | --- |
| B0 measured | bright | **19,003 (950 s)** | W 0.0301 P 0.0370 Q 0.0151 |
| B0 measured | dim (L·μ 0.20) | censored | W 0.0071 P 0.0071 Q 0.0036 |
| §11 hand | bright | **6,139 (307 s)** | W 0.0445 P 0.0551 Q 0.0223 |
| §11 hand | dim (L·μ 0.20) | censored | W 0.0173 P 0.0173 Q 0.0086 |

**B7-1 is resolved.** The §11-donor bright arm now establishes in 307 s against §11's ~300 s
prediction, where run 1 took 1,146 s; the donor's reserve is no longer drained into its own
foliage, so the spend is continuous rather than intermittent. The B0-donor arm, which sent
nothing at all in run 1, now establishes in 950 s. **B7-2 is not resolved**: neither dim
recipient established or died back inside the horizon, in either arm.

## Run 1 — what the scenarios measured before the §4.4 repair

Kept unchanged, so the repair's effect is visible rather than asserted. Everything below this
heading is the first implementation run (`b1dd394`), against contract §4.4 as it stood before
commit `3316980`.

Every row is from one `--release` run of
`crates/cubarium-core/examples/ecology_v1_scenarios.rs`, censored at the 36,000-tick horizon
and never extended.

### Run 1: B0 — stand baseline (the anchor)

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

### Run 1: B1a — one pinned grazer, one bright mature stand, 12,000 ticks

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

### Run 1: B1b — one mobile legacy grazer on a 5 × 5 mature region

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

### Run 1: B2 — depletion and relocation, three bright stands three cells apart

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

### Run 1: B3 — plant recovery after defoliation

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

### Run 1: B4a — three pinned grazers on one isolated bright mature stand

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

### Run 1: B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | censored at 36,000 | 0.0000 (0 % of B0's W) | 0.0000 |
| the §11 hand table's bright stand | tick 1,500 (75 s) | 0.0588 (9.8 % of 0.60) | 0.0696 (12.4 % of 0.56) |

With §11's stand the ring establishes in 75 s — §13.2 expected "within a minute" — and the
rebuild is censored at 30 min, also as expected. With B0's measured stand **nothing is ever
sent**: no ring cell clears `q_prop · Q_max`, so §4.8 never fires (finding B0-1).

### Run 1: B5 — dietary exclusion

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

### Run 1: B6a — reproduction on a finite input (3 × 3 bright, renewal off)

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

### Run 1: B6b — reproduction on a renewing patch (7 × 7 bright)

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

### Run 1: B7 — establishment

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

All eleven below were **accepted** in contract §18's third round; interpretation 6
(`producer.energy_density` removed in favour of the one `e_v`) is now recorded in §14. They
are kept here as the record of what was decided and why.

Repair cycle 1 added no new interpretation: the revised §4.4 is unambiguous, and
`reserve_share` / `reflush_below` are the two §14 fields with their stated `[0, 1]` validation.
One arithmetic coincidence in §11 is worth naming, and it is a measurement rather than a
reading — see finding R2-4.

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

**None, in either run.** Every §11 provisional value — including repair cycle 1's `q_share` 0.2
and `p_reflush` 0.25 — is the shipped default and every one is admitted by
`WorldConfig::validate`. The validations (`rate · dt ≤ 1` by name for `m_w`, `m_p`, `ripen`,
`drop`, `k_d`, `k_c`, `k_w`, `fall`; `alive_min < donor_min ≤ wood_max`; `propagule_split`
summing to 1; `capability_gate ∈ [0, 0.5]`; `capability_exponent > 0`; and now
`reserve_share ∈ [0, 1]`, `reflush_below ∈ [0, 1]`) all pass at the §11 table with room to
spare.

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
- **`hunter.rs::observations_do_not_move_a_profile_three_hunter_world` was re-recorded twice**,
  once for ecology v1 and once for repair cycle 1. Its schema 12 projection covers `config` and
  `fields`, and both moved each time — the config gained the `plant` block and then its two new
  fields, and the plant arithmetic changed. The claim the test makes, that *observing* a world
  never moves it, is unchanged; only its build-specific anchor is.
- **Repair cycle 1 touched exactly three source files**: `config.rs` (the two fields and their
  validation), `fields.rs` (the revised §4.4 block), and the B4a reporting in the scenario
  binary. Nothing in `neural/`, `snapshot.rs`, `step.rs` or `cubarium-search` moved.

## Open findings after repair cycle 1

### Resolved by the repaired §4.4

| run-1 finding | evidence it is resolved |
| --- | --- |
| **B0-1** — the reserve never refills, so no simulator-grown stand is a §4.8 donor | B0 average `Q` is now **exactly full** (0.0525 = `q_cap · W`), bright is half full and rising (0.0894 of 0.1789), and the bright stand **sent 0.1660 m** of propagules where run 1 sent none. |
| **B3-1** — a stripped stand cannot recover, because it has no reserve to reflush from | B3 bright, B0-measured arm: 0.5·P* at 1,446 s where run 1 was censored; the reserve dips to 0 and **refills to 0.0894** during the run. The §11-hand arm refills to 0.1500 where run 1 left it at 0.0486. |
| **B4b, B0-donor arm** — nothing ever sent | establishes at 2,642 (132 s), against the §11-donor ring's 75 s. |
| **B7-1** — establishment took 1,146 s, not §11's ~300 s | §11-donor bright arm now establishes at **307 s**. The B0-donor arm, which sent nothing in run 1, establishes at 950 s. |
| **B4a death censored** | not a defect; §13.2 now expects censoring and asks for the decline rate. Measured e-fold **5,119 s (85 min)** against `1/(κ·m_w)` = 5,000 s (83 min), within 2.4 %. |

### Not resolved by the reserve fix

| finding | evidence |
| --- | --- |
| **B1b-1 — bright coexistence still fails, and got worse.** | One grazer took a 25-cell bright region from Σ P 23.037 to **0.0140** and **killed 18 of its 25 stands** (run 1: 0 deaths). It lived 1,094 s against 913 s. The stands carry less wood after the repair (B0 bright `W` 0.358 against 0.487), so the same pressure carries them below `W_min` inside the horizon. §13.2 names a bright failure as a finding, and it is still a failure. |
| **B6b — a 2.24× production surplus still ends in extinction.** | 6 births, 8 deaths, peak 8, extinct at 1,429 s (run 1: 8 / 10 / 10, 1,110 s). Later and smaller, not avoided. Same shape as B1b-1: mouths meet stocks, not production. |
| **R2-1 — B0 is now *further* from the §11 hand table, not closer.** | Bright `ΔP` −0.0422 → −0.0905, `ΔW` −0.1125 → −0.2422; average `ΔP` −0.3232 → −0.4023 and its wood did not grow at all (`W` still exactly the seed 0.105, `dW/dt = 0`). `q_share = 0.2` is paid out of growth, and neither class finished filling its reserve inside the 30-minute horizon, so the diversion is still in force at the measurement point. §11 predicts the sign of this ("`P*` sits a few hundredths lower until the reserve is full") but not the size. |
| **R2-2 — B2 changed direction.** | Run 1: the grazer left each stand at `P ≈ 0.28–0.41` and all three recovered to `P ≈ 0.558`. Run 2: it strips all three to `P = 0.0003` and **none recovers** by the horizon, staying ~1,000 ticks per stand instead of ~300. Same cause as B1b-1: the B0 stands are smaller. Recovery-before-return remains untested, because the grazer dies first in both runs. |
| **B3 average — censored in both arms, unchanged.** | B0-measured: 0.5·P* censored; §11-hand: 0.5·P* censored, reserve 0.16 → 0. §13.2 expects the average class to sit near breakeven and be reported censored, so this is the expected direction — but it means the repair bought recovery for bright only. |
| **B7-2 — the dim recipient neither establishes nor dies back.** | Censored in all four arms across both runs (run 2: W 0.0071 with a B0 donor, 0.0173 with a §11 donor). §13.2's expectation there is untested rather than contradicted. |
| **`ecology_hash` no longer covers the ecology.** | Structural, untouched by the repair. Kept because the care/no-care comparison uses it (§15.1); `state_hash` covers everything. |

### New, from run 2

**R2-4 — at §11's own values the reflush ceiling and `Q_max` are the same number.**
`p_reflush · α = 0.25 · 2 = 0.5 = q_cap`, so `p_reflush · P_cap = q_cap · W = Q_max` exactly
whenever `α · W < P_max`. A stand with a *full* reserve therefore cannot quite reach the reflush
ceiling: paying `1 + c_g` per unit of leaf, `Q_max` of reserve buys `Q_max / 1.2` of foliage and
the reserve binds first, by that factor. The mechanism is correct either way — the unit test
demonstrates the ceiling binding by over-provisioning the reserve — but "reflush up to
`p_reflush · P_cap`" is, at this table, always "reflush until the reserve runs out". Worth a
look in the whole-ecosystem tuning; not touched here.

None of these was tuned. Every number is a measurement at the contract's own §11 values.

## Stop

Held-out checks, training in the revised ecology, display deployment and the §12 presentation
task are separate assignments after the review.
