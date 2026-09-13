---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Juvenile flow diagnostic: growth never ran, and charging is not why

A read-only mutation-site ledger now measures where a paid juvenile's reserve and
battery actually go. Run on two arms so far — the richest case in the corrected cohort (the seed-8
`specialist_on` child that made 13 captures) and the seed-6 `facultative_on`
child born at the old failure boundary — it settles two things the
[census-based reduction](hunter-charge-rerun-results-2026-09-13.md) could only
bound, and it **contradicts the reading that the raised charging threshold is
what starves the juvenile**.

Nothing was retuned, migrated or seeded. No configuration, threshold, cost,
geometry, gate or default moved. Nine arms remain unrun; two children are not a
cohort. Lore/Graft/canon were consulted; canon carries no hunter or growth entry,
so this is unclassified exploration evidence.

## Provenance and isolation

The instrumentation lives on its own branch, never on `main`:

| | |
| --- | --- |
| Base | `512ee52a54207ff3d6b59b6fee753c9603480af6` (the frozen charging-rerun source) |
| Branch | `diagnostic/hunter-juvenile-flow-2026-09-13`, commit `0d867f1` |
| Worktree | `captures/diagnostic-source/hunter-juvenile-flow-2026-09-13` (gitignored, workspace disk) |
| Build cache | `captures/build-cache/juvenile-flow` |
| Diagnostic binary SHA256 | `35943316416a947b6fedc42eaa973f787d2140f6fc53b95ca5360d6c52032977` |
| Patch on `main` | [`assets/hunter-juvenile-flow-instrumentation-2026-09-13.patch`](assets/hunter-juvenile-flow-instrumentation-2026-09-13.patch) |

`/tmp/cubarium-charge-rerun-512ee52` and its retained executable were not touched
and are not the build used here. The artifacts are schema 12 and were read as
schema 12; nothing was migrated to `main`'s schema 13, and no expected hash was
relabelled. `main`'s core is unchanged by this work.

The diff is four files: a new `crates/cubarium-core/src/flow.rs`, one line in
`lib.rs`, one `Option` field plus twelve recording sites in `world.rs`, and a new
`crates/cubarium/examples/juvenile_flow.rs`. The only pre-existing lines modified
are the five that name the growth step's four caps so the binding one can be
recorded; the arithmetic and its order are unchanged, and gate 2 below proves it.
No formatter churn: `world.rs` is untouched by `cargo fmt`.

## Why a ledger rather than more census

The retained census samples stocks every 200 ticks. A stock difference cannot
separate oxidation from growth — both move reserve in the same tick — so every
"oxidation drained it" statement drawn from that census was a budget inference.
The ledger reads each amount at the assignment that moves it, in the core's own
expression, and reads the growth predicate *before* the branch it gates, so a
closed gate is an observation rather than the absence of a growth record. Upkeep
is the one place the core pays a lumped, clamped debit; its three terms are
recorded as *demanded* beside the single amount paid, and the payment is
deliberately **not** split across them.

## Four gates, all passed

| Gate | Result |
| --- | --- |
| 1. Input identity | Opening `81cc5909aded672bc4271021c94fdddf649149b88c67f85463f02d75afc078e5`, state hash `13859516448854398358` — both equal to `opening.json` |
| 2. Output identity | Replayed closing re-encodes to `e567279199bf9c246113373c2e37f8c7ffa1277eb63f81f65ba0cd1fe5c546bd`, state hash `6145446113726102027` — both equal to the retained `closing.cubw` and `summary.json` |
| 3. Observer neutrality | Two full 144000-tick replays differing only in `enable_flow_ledger()`: identical closing bytes, identical state hash, all 997 event-emitting ticks identical record for record |
| 4. Flow/stock reconciliation | **0 violations** over 62279 member-ticks; worst residual 8.70e-17 reserve, 7.23e-16 energy, exactly 0 structure |

Gate 2 asserts the *state*: the build label is supplied to the encoder, not
derived from the diagnostic binary. Gate 4 is what makes the ledger evidence
rather than a plausible tally — a mutation site the ledger missed would move a
stock by ~1e-4 or more and would show up immediately.

`cargo test -p cubarium-core` on the branch: **149 lib tests** (8 of them new for
the ledger) plus every integration suite, 0 failures — including `hunter` (28),
`hunter_charging` (17) and `astra_target_cleanup` (2) with the instrumentation
compiled in and inactive.

## The child's whole life, measured

Child `41:5`, parent `6:6`, born tick 173805 with its escrow (structure 0.8,
reserve 0.8, energy 0.6), starved at 196434. 22630 ticks = 1131.5 s, **juvenile
for every one of them**.

**Growth ran zero times.** The branch was entered on 0 of 22630 ticks and gained
0 structure. This now covers the whole life, including the 35-tick
last-census-to-death suffix the census-based report explicitly could not reach —
that open question is closed for this arm, by mutation-site observation rather
than by a rate bound.

**It is always the reserve side of the gate that is shut, and it is not close.**

| Growth predicate, per tick | |
| --- | ---: |
| Structure below adult | 22630 / 22630 |
| Reserve above `0.3 × reserve_max` = 1.2 | **0 / 22630** |
| Closest approach from below (reserve 0.8545) | deficit 0.3455 |
| Mean pre-growth reserve | 0.0706 |
| Reserve at exactly zero | 15590 / 22630 = 68.9% |

**Every unit of reserve the child ever lost went to oxidation.**

| Reserve | amount | share |
| --- | ---: | ---: |
| Opening escrow | 0.8000 | |
| In — digestion | 2.7227 | 100% of inflow |
| In — frugivory / grazing / scavenging | 0.0000 / 0.0000 / 0.0000 | 0% |
| Out — oxidation | 3.5227 | **100% of outflow** |
| Out — growth | 0.0000 | 0% |
| Out — reproduction funding | 0.0000 | 0% |
| At death | 0.0000 | closes to −4.3e-13 |

**Charging accounts for one eighth of that burn.** The ledger applies the same
`above_reference` test the charging diagnostics use, per member instead of per
process:

| Oxidation | child `41:5` | founder `6:6` |
| --- | ---: | ---: |
| Ticks active | 7050 | 21883 |
| Ticks above the world's configured 0.5 reference | **846 (12.0%)** | **21883 (100%)** |
| Reserve burned | 3.5227 | 10.9410 |
| Burned above reference | **0.4225 (12.0%)** | **10.9410 (100%)** |

88% of the juvenile's burn happened at battery levels below the *background*
recipe's own activation point, so it would have happened under the background
policy too. The intervention lands almost entirely on the adult founder, whose
burn is 100% above reference. The reading in the previous report — that the
battery mechanism converts precisely the stock the growth gate needs — is
correct as a description of the pathway and **wrong as an attribution to the
raised threshold** for the juvenile.

**What actually empties the battery is upkeep.**

| Child energy | amount | share |
| --- | ---: | ---: |
| In — oxidation | 5.6364 | 72.4% |
| In — digestion | 2.1494 | 27.6% |
| Out — upkeep paid | 5.8669 | **70.0%** |
| Out — strikes (30 × 0.08) | 2.4000 | 28.6% |
| Out — handling | 0.1189 | 1.4% |
| Out — growth | 0.0000 | 0% |

Of the child's upkeep *demand*, sensing is 2.7156 (46.3%), maintenance 2.2630
(38.6%) and movement 0.8884 (15.1%). The per-tick figures separate cleanly:

| Per tick | child (structure 0.8) | founder (structure 2.0) |
| --- | ---: | ---: |
| Maintenance | 1.000e-4 | 2.500e-4 |
| Sensing | **1.200e-4** | **1.200e-4** |

Maintenance scales with structure exactly as expected (ratio 2.5 = 2.0/0.8).
**Sensing does not scale at all** — a juvenile at 40% of adult structure pays the
identical 12-radius sensing bill, and for it that bill is the single largest
upkeep term, where for the adult it is 28.5%. The chain the ledger measures is:
a fixed sensing bill plus strikes keeps the battery low → oxidation fires to
refill it → oxidation is the only consumer of reserve → reserve never reaches
1.2 → the growth branch never runs. That is a measured description of this
child's path. It is **not** authorization to change `sense_cost`, its scaling, or
anything else.

For contrast, founder `6:6` (adult, 39649 ticks, starved 183648) took in 10.5410
reserve and spent 12.5410: oxidation 10.9410 (87.2%) and one funded gestation
1.6000 (12.8%). Its growth branch also never ran, correctly — its structure side
was shut for all 39649 ticks because it was already adult. It funded exactly one
child at 1.6 reserve + 1.0 energy with escrow 0.8/0.8/0.6 and 0.4 build heat,
matching the transaction records exactly.

## The second arm replicates it, and harder

Seed-6 `facultative_on` — the arm whose invalid closing state
`9140998897574537509` was the original study's failure — replays through all four
gates too: closing state hash `11174133049431148431` equal to the retained one,
observer-neutral across all 757 event-emitting ticks, 0 reconciliation violations.

Child `74:5`, born 170739, starved 179397, juvenile for all 8659 ticks:

| | child `74:5` | child `41:5` (seed 8) |
| --- | ---: | ---: |
| Growth branch entered | **0 / 8659** | **0 / 22630** |
| Reserve above the 1.2 gate | 0 ticks | 0 ticks |
| Closest approach (deficit) | 0.1728 | 0.3455 |
| Reserve at exactly zero | 74.5% | 68.9% |
| Reserve outflow that is oxidation | **100%** | **100%** |
| Oxidation burn above the 0.5 reference | **0.0%** | 12.0% |
| Sensing share of upkeep demand | 43.4% | 46.3% |

For this child the raised threshold contributed **literally nothing**: not one of
its 2208 oxidation ticks was above the world's configured reference. Its parent
`29:6`, by contrast, burned 92.6% of its reserve above reference. The pattern
across both arms is the same and it is not marginal — charging lands on the
adult (100%, 92.6%) and essentially misses the juvenile (12.0%, 0.0%).

The facultative profile's scavenging is also now measured rather than assumed:
founder `29:6` scavenged on 140 ticks for 0.0297 reserve — 0.36% of its intake —
and child `74:5` never scavenged at all. Frugivory and grazing are exactly zero
for every member on both arms, as a diet-0.0 genome implies.

## What is still not attributed

- **No counterfactual exists.** The 12% figure is what the recorded
  `above_reference` test says about this realized path. It is not "removing the
  raise returns 12% of the burn": removing it changes the trajectory. No juvenile
  in this cohort has ever run under the world's configured threshold.
- **Upkeep's payment is not split.** The shares above are of *demand*. On this
  arm demand and payment differ by 0.0001 over one tick, so the distinction
  barely bites — it will matter for a member that cannot pay.
- **Encounter versus approach is still unresolved.** The ledger counts 30 strikes
  and 13 captures; it does not measure how often prey was reachable and not
  struck. The acquisition-is-not-intercept question stands exactly where the
  corrected report left it.
- **The growth-cap instrument is unexercised.** All four `bound_by_*` counters
  are zero because the branch never ran. Only its unit test covers that path.
- **Two arms, two children, two seeds.** Nine arms remain. The two measured
  children are the longest-lived and one mid-length case; the seven children that
  captured nothing are unmeasured, and a child that never eats may fail for a
  different reason than one that eats and cannot bank it.

## Running the other ten arms

Same binary, same gates, one arm per invocation (~50 s each, two full replays):

```
cd captures/diagnostic-source/hunter-juvenile-flow-2026-09-13
export CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/captures/build-cache/juvenile-flow
R=/home/wrysk/wryskware/cubarium/captures/hunter-charge-candidate-two-hour-512ee52
O=/home/wrysk/wryskware/cubarium/captures/juvenile-flow-2026-09-13
for a in 1/specialist_on 5/specialist_on 7/specialist_on \
         1/facultative_on 2/facultative_on 5/facultative_on \
         7/facultative_on 8/facultative_on 12/facultative_on; do
  cargo run --release -p cubarium --example juvenile_flow -- \
    "$R/seed-${a%/*}/${a#*/}" --out "$O/seed-${a%/*}-${a#*/}.json"
done
```

The loop form above was executed for `6/facultative_on` and is what produced
that arm's ledger. Each run refuses unless all four gates pass, so a silent
failure is not available. `--ticks N` runs a prefix for a smoke check and says in its own output
that closing identity was not asserted.

**No parameter change is authorized by this diagnostic**, and none is proposed
here. An adult-scoped charging threshold remains a future candidate only, and
this arm's 12% figure weakens rather than strengthens the case for it. No 24- or
72-hour hunter study follows from a passing diagnostic.

## Verification

- `cargo test -p cubarium-core` on `diagnostic/hunter-juvenile-flow-2026-09-13`:
  149 lib tests (8 new in `flow::tests`) and every integration suite, 0 failures.
- `juvenile_flow … seed-8/specialist_on --ticks 32000`: gates 1, 3, 4 passed;
  gate 2 correctly reported as not asserted for a prefix.
- `juvenile_flow … seed-8/specialist_on` at the full 144000-tick horizon: all
  four gates passed. Ledger:
  [`assets/hunter-juvenile-flow-seed8-specialist-2026-09-13.json`](assets/hunter-juvenile-flow-seed8-specialist-2026-09-13.json).
- `juvenile_flow … seed-6/facultative_on` at the full horizon: all four gates
  passed. Ledger:
  [`assets/hunter-juvenile-flow-seed6-facultative-2026-09-13.json`](assets/hunter-juvenile-flow-seed6-facultative-2026-09-13.json).
- The retained charging artifacts, the earlier reductions and every original
  failure record are unchanged.
