---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Size-aware growth gate: implementation, reference parity and a seed-1 pilot

The single candidate in
[Astra's proposal](astra-hunter-size-aware-growth-proposal-2026-09-13.md) is
implemented, tested and piloted on seed 1. **Paid juvenile growth happened for the
first time**, and in this one seed it came with a worse outcome for the lineage.
Both facts are recorded exactly as observed.

This is an experiment, not an accepted biological default. `main`'s core is
untouched, nothing is live, and no seed beyond 1 has been started.

## Provenance

| | |
| --- | --- |
| Base | `0d867f1` (the reviewed flow-ledger diagnostic) on frozen `512ee52` |
| Branch | `experiment/hunter-size-gate-2026-09-13` |
| Implementation commit | `8390cdd` |
| Warning cleanup commit | `9f4cf7d` (branch head) |
| Worktree | `captures/experimental-source/hunter-size-gate-2026-09-13` (gitignored) |
| Build cache | `captures/build-cache/size-gate` |
| Pinned executable | `captures/build-cache/size-gate/release/examples/hunter_compare` |
| Executable SHA256 | `90d30b8b1b7f2f7e34a8bcc25d5785144b51f84ed24b80b317d4234c15e82a4f` |
| Build id | `0.1.0+9f4cf7d` |
| Patch on `main` | [`assets/hunter-size-gate-instrumentation-2026-09-13.patch`](assets/hunter-size-gate-instrumentation-2026-09-13.patch), verified to reverse-apply cleanly onto `9f4cf7d` |

The diagnostic worktree, the retained charging artifacts, the ordinary-fauna
worker's tree and `main`'s core were not touched. Nothing was written to `/tmp`.

## What was implemented

**`FixedHunterProfile::growth_gate` is the whole production change.** At the
existing post-oxidation, pre-growth site:

```
legacy_gate = growth_reserve_min · reserve_max
version 5:  gate = legacy_gate · clamp(structure / structure_adult, 0, 1)
everything else: gate = legacy_gate, the original expression
enter only when S < S_adult AND R > gate
```

For this frozen profile that is **0.6·S**: 0.48 at birth, 0.72 at S=1.2, 0.96 at
S=1.6 and 1.2 at adulthood, where it equals the legacy gate exactly.

- **Semantic profile version 5** (`PROFILE_VERSION_SIZE_GATE`), previously unused.
  It explicitly keeps charge80's fixed 0.80 `E_max` oxidation activation — the trap
  the proposal named. Matching only version 4 in `oxidation_policy` would have
  dropped charging and made this two changes. `lanternjaw_trial` still writes
  version 3, and unsupported versions are still refused by name.
- **The gate is a precondition only.** The increment keeps all four caps, both
  debits, the construction heat and the physiological ordering. Nothing is capped
  to `reserve − gate`, no reserve floor is protected, nothing is replenished.
- **A degenerate adult denominator returns the legacy gate** rather than
  propagating a NaN, because a NaN gate would silently read as a very strict
  threshold instead of the mechanism under test.
- **The flow ledger adapts to a moving gate.** The overwritten `gate_reserve`
  scalar is renamed `gate_reserve_last` and joined by min, max, the legacy gate
  beside it, and the threshold *and size* at the first actual growth, plus
  first-growth and first-adult boundaries and max structure. A member that never
  grew reports `None`, not a fabricated threshold.
- **`hunter_compare` gains the recipe** `reserve-targets-charge80-size-gate-v1`
  and a `--seeds` filter. A restricted run records `ran_seeds` and
  `seed_subset_pilot: true` in its manifest, so a partial collection cannot be
  read as a cohort.

## Tests

`cargo test -p cubarium-core`: **152 lib tests plus every integration suite, 0
failures.** `cargo test -p cubarium --example hunter_compare`: **78, 0 failures.**

**16 new tests** in `crates/cubarium-core/tests/hunter_size_gate.rs`:

- version 5 validates, keeps `OxidationPolicy::Fixed(0.80)`, and its serialized
  diff from charge80 is the version field alone;
- versions 3 and 4 and ordinary organisms return the legacy gate bit for bit;
- gate values at S = 0.8, 1.2, 1.6, 2.0, and clamping at both ends;
- degenerate adult denominators (0, negative, NaN, ±∞) fall back, never compare
  against NaN;
- the same juvenile at R=0.6 **grows under version 5 and is refused under version
  4** — the contrast the experiment exists to create;
- strict equality refuses and the next representable reserve above the gate
  permits;
- an adult never grows under either policy;
- **all four increment caps with real positive growth**, each binding in turn —
  rate, remaining structure, reserve, battery — with the reserve and battery
  debits closing at the mutation site, plus zero build cost omitting the battery
  term;
- a mid-growth snapshot restart reproduces state and both event streams against an
  uninterrupted run for 120 further ticks, and keeps the selector;
- an unsupported version is refused rather than reinterpreted;
- the observer records the growth and moves nothing — identical closing bytes,
  state hash and event stream.

Two existing expectations were updated because their subject genuinely changed:
`SUPPORTED_PROFILE_VERSIONS` is now `[3, 4, 5]`, and the unsupported-version
example moved from 5 to 6.

**One proposal item could not be tested as written.** A member's
`juvenile_growth_rate` is in the profile's *positive* validation set, so a zero
member growth rate is refused at the door and the zero-rate branch is unreachable
for a member. That is pinned as a contract refusal, and the limiting arithmetic is
covered at a rate of 1e-9 instead.

## Reference parity: 6 of 6 arms reproduced

The experiment build running the **unchanged** charge80 recipe reproduces the
retained `512ee52` artifacts for every seed-1 arm.

| Arm | payload identical | events identical | census identical | closing state hash |
| --- | --- | --- | --- | --- |
| untouched | yes | yes | yes | equal |
| budget_control | yes | yes | yes | equal |
| specialist_off | yes | yes | yes | equal |
| specialist_on | yes | yes | yes | equal |
| facultative_off | yes | yes | yes | equal |
| facultative_on | yes | yes | yes | equal |

**The projection is stated, not assumed.** The CUBW header embeds the build id, so
the file SHA256 *must* differ between `0.1.0+512ee52` and `0.1.0+9f4cf7d`, and it
is recorded rather than asserted equal. Parity is the payload: schema 12, CRC32,
FNV state hash and payload length, recomputed from both sets of bytes, plus the
recorded event and census streams compared line for line.

## Candidate divergence, under a named projection

The semantic selector differs from tick zero by construction, so the census rows
are compared after removing exactly one named field,
`hunter_state.profile.version`. Nothing is wildcarded, and no raw state hash is
claimed equal while the selector differs.

| Arm | identical under projection | event streams identical | first divergence |
| --- | --- | --- | --- |
| untouched | **yes** | yes | — |
| budget_control | **yes** | yes | — |
| specialist_off | **yes** | yes | — |
| facultative_off | **yes** | yes | — |
| specialist_on | no | no | census tick **170600**, member `17:3` |
| facultative_on | no | no | census tick **170600**, member `17:3` |

The four arms without a juvenile are identical, including their event streams —
the control the proposal asked for. The two hunting arms diverge at the first
census boundary after the first altered growth transaction. `17:3` is the same
child the retained cohort recorded, born at tick 170401.

## The pilot result: growth happened, and this seed got worse

**Structure moved for the first time.** Reference: every child sat at S=0.8 for
its whole life, as the whole retained cohort did. Candidate:

| Arm | child | first census | structure | first increase | reached adult |
| --- | --- | ---: | --- | ---: | --- |
| specialist_on | `17:3` | 170600 | 0.8199 → **0.8485** | 170800 | no |
| facultative_on | `17:3` | 170600 | 0.8199 → **0.8485** | 170800 | no |
| facultative_on | `90:4` | 231400 | 0.8078 → **0.8485** | 231600 | no |

Two points are worth noting. The child was already at 0.8199 by its first census
boundary, 199 ticks after birth — exactly 199 rate-limited steps of 0.0001, so it
grew at the cap from its first tick. And all three stopped at **0.8485**, which is
where the proposal's own arithmetic illustration said a juvenile converting its
birth endowment under continuous maximum oxidation would meet the rising gate
("approximately S=.8485"). The mechanism behaved as its arithmetic predicted, and
the stopping point is set by the endowment, not by the gate being shut.

**Every other outcome in this seed moved the wrong way.**

| Seed-1 hunting arm | captures | offspring | closing hunters | founder extinction | adult occupancy 0 / 1 |
| --- | ---: | ---: | ---: | ---: | --- |
| specialist_on reference | 114 | 3 | 1 | **survived** | 0 / 144000 |
| specialist_on candidate | **49** | **1** | **0** | **212551** | 75450 / 68550 |
| facultative_on reference | 114 | 3 | 1 | **survived** | 0 / 144000 |
| facultative_on candidate | **90** | **2** | **0** | **267524** | 20477 / 123523 |

In the reference both founders survived the horizon; under the candidate both
died, the lineage went extinct in each arm, and captures and paid births fell.
Prey did correspondingly better: closing prey 93→99 and 92→99, minimum 66→70.
All twelve arms passed their conservation audits with no failure.

**This is an adverse result and it is retained exactly.** It is also one seed, and
the founder deaths are downstream of a divergence that began with the child, not a
direct effect of the increment: after tick 170600 the two worlds are legitimately
different and their prey, encounters and timing all differ. Eleven seeds are unrun.
No tuning was done in response, and none should be.

## The named gap is closed: the mutation-site record

Root authorised closing the ledger gap before the twelve-seed collection. The
runner now enables the bounded flow ledger per arm and writes one `flow.json` at
close — one record per member for the whole run plus fixed-width bins, never a
per-tick world dump. A member that died is retained with its removal-site stocks;
one alive at the horizon is right-censored. A **new build was frozen and a new
paired seed-1 pilot run in new exclusive directories**; the earlier pilot and its
`90d30b8b…` binary are untouched and its census is not reinterpreted.

| | |
| --- | --- |
| Instrumentation commit | `ae866a5` (branch head) |
| Pinned executable SHA256 | `27c138c5843eb58ab78355e1440e57c600380c53ac64f3a99731cf8c554b5846` |
| Build id | `0.1.0+ae866a5` |
| Outputs | `captures/hunter-size-gate-pilot-flow-2026-09-13/` |

### Three identity gates, all 6 of 6

| Comparison | Result | What it establishes |
| --- | --- | --- |
| New reference vs retained `512ee52` | **6/6** | the instrumented build still reproduces the original charge80 artifacts |
| New reference vs the earlier pilot's reference | **6/6** | — |
| New candidate vs the earlier pilot's candidate | **6/6** | **the observer changes biology by zero at full horizon**: the earlier run had no ledger in the loop at all, and 144000 ticks × 6 arms come out with identical payloads, state hashes and event streams |

Each arm additionally ran a bounded in-run probe — its own opening stepped 2000
ticks twice, with and without recording — and all six report identical state and
event records. The run fails if any of that does not hold.

The runner and the reducer now both fail on a member the arm held that the ledger
did not record, a record for an unregistered member, a non-finite or negative
amount, a source total not actually recorded per source, an incomplete horizon, a
payment that does not reconcile within the ledger's own frozen `1e-9` tolerance,
structure built without matching reserve spent, or a cap attribution that does not
account for the steps taken. Original tolerances are unchanged and the raw legacy
energy diagnostic is still a diagnostic, not an audit.

### What actually happened, measured at the mutation site

The census bounded the first divergence at tick **170600**. The actual first
altered growth transaction was at tick **170401** — the child's own birth tick,
199 ticks earlier. The census could only bound it; this is the transaction.

| | candidate `17:3` | candidate `90:4` | reference children |
| --- | ---: | ---: | ---: |
| Growth ticks | **485** | **485** | **0** |
| Structure gained | 0.048500 | 0.048500 | 0 |
| Reserve spent | 0.048500 | 0.048500 | 0 |
| Battery spent | 0.024250 | 0.024250 | 0 |
| Construction heat | 0.121250 | 0.121250 | 0 |
| Bound by rate / remaining / reserve / battery | **485 / 0 / 0 / 0** | **485 / 0 / 0 / 0** | — |
| Gate at first growth (structure) | 0.48 (S=0.8) | 0.48 (S=0.8) | — |
| Gate range observed | 0.4800 → 0.5091 | 0.4800 → 0.5091 | 1.2 constant |
| Max structure | 0.848500 | 0.848500 | 0.800000 |
| Reached adult | no | no | no |
| **Total reserve intake, all four sources** | **0.000000** | 0.142414 (scavenging only) | 0.5165 / 0 / 0 |

Three facts follow from the records rather than from arithmetic about them.

**Every one of the 485 steps was rate-limited.** Not one was bound by reserve, by
the battery or by remaining structure. The plateau at 0.8485 is therefore not a
cap biting: growth simply stopped being permitted, as the rising gate (0.48 →
0.5091) met the falling reserve. The earlier report's "the proposal's arithmetic
predicted 0.8485" is now replaced by the measurement — 485 × 0.0001 from 0.8, with
the gate at the last permitted step recorded.

**`17:3` grew entirely on its birth escrow and never ate at all.** Its recorded
intake is 0.000000 across digestion, frugivory, grazing and scavenging. In the
reference the same child digested 0.5165. So the growth did not come out of food.

**The child that grew died sooner.** Reference `17:3` lived 9582 ticks to 179982;
candidate `17:3` lived 4463 ticks to 174863 — less than half — having spent 0.0485
reserve and 0.02425 battery on structure. That is an individual-level adverse
outcome measured directly, alongside the seed-level ones already retained.

Reference arms confirm the other side: all four members per hunting arm, gate
constant at 1.2, zero growth ticks, max structure exactly 0.800000.

## Limitations and the remaining gap

- **One seed. Not a cohort, not a viability result.** Maturation was not reached
  in any arm; no descendant became adult; adult occupancy never exceeded one.
- ~~The mutation-site ledger is not wired into `hunter_compare`.~~ **Closed** at
  `ae866a5`; see the section above. The first pilot's census remains a census and
  is not reinterpreted as per-tick evidence.
- Census structure values are boundary samples, not every intra-tick assignment.
  Where the two disagree, the flow record is the transaction and the census is the
  bound.
- The divergence projection removes one named field; it is not a proof that no
  other selector-derived value exists, only that none was found in these rows.

## Commands

```
# tests, in the experiment worktree
cd captures/experimental-source/hunter-size-gate-2026-09-13
CARGO_TARGET_DIR=<repo>/captures/build-cache/size-gate cargo test -p cubarium-core
CARGO_TARGET_DIR=<repo>/captures/build-cache/size-gate cargo test --release -p cubarium --example hunter_compare

# the pilot, with the pinned executable (both runs, ~2.5 min each)
BIN=captures/build-cache/size-gate/release/examples/hunter_compare
$BIN captures/hunter-openings-2026-09-13 \
  captures/hunter-size-gate-pilot-2026-09-13/reference-charge80-seed1 \
  --profile reserve-targets-charge80-v1 --seeds 1 --ticks 144000
$BIN captures/hunter-openings-2026-09-13 \
  captures/hunter-size-gate-pilot-2026-09-13/candidate-size-gate-seed1 \
  --profile reserve-targets-charge80-size-gate-v1 --seeds 1 --ticks 144000

# parity and divergence
node scripts/hunter-size-gate-parity.mjs \
  --reference captures/hunter-size-gate-pilot-2026-09-13/reference-charge80-seed1 \
  --candidate captures/hunter-size-gate-pilot-2026-09-13/candidate-size-gate-seed1 \
  --retained  captures/hunter-charge-candidate-two-hour-512ee52 \
  --seeds 1 --binary-sha256 90d30b8b1b7f2f7e34a8bcc25d5785144b51f84ed24b80b317d4234c15e82a4f
```

### The instrumented pilot, at `ae866a5`

```
BIN=captures/build-cache/size-gate/release/examples/hunter_compare   # 27c138c5…
N=captures/hunter-size-gate-pilot-flow-2026-09-13
$BIN captures/hunter-openings-2026-09-13 $N/reference-charge80-seed1 \
  --profile reserve-targets-charge80-v1 --seeds 1 --ticks 144000
$BIN captures/hunter-openings-2026-09-13 $N/candidate-size-gate-seed1 \
  --profile reserve-targets-charge80-size-gate-v1 --seeds 1 --ticks 144000

node scripts/hunter-size-gate-parity.mjs \
  --reference $N/reference-charge80-seed1 --candidate $N/candidate-size-gate-seed1 \
  --retained captures/hunter-charge-candidate-two-hour-512ee52 \
  --prior-reference captures/hunter-size-gate-pilot-2026-09-13/reference-charge80-seed1 \
  --prior-candidate captures/hunter-size-gate-pilot-2026-09-13/candidate-size-gate-seed1 \
  --seeds 1 --binary-sha256 27c138c5843eb58ab78355e1440e57c600380c53ac64f3a99731cf8c554b5846
```

Artifacts, retained unchanged: `captures/hunter-size-gate-pilot-2026-09-13/` (the
first pilot, with its own `90d30b8b…` frozen binary) and
`captures/hunter-size-gate-pilot-flow-2026-09-13/` (the instrumented one, 77 MB).
Committed copies:
[`assets/hunter-size-gate-pilot-parity-2026-09-13.json`](assets/hunter-size-gate-pilot-parity-2026-09-13.json)
and [`assets/hunter-size-gate-flow-pilot-2026-09-13.json`](assets/hunter-size-gate-flow-pilot-2026-09-13.json).

`node --test scripts/hunter-size-gate-parity.test.mjs` — **14 passed, 0 failed**
(9 before; the 5 new cover the flow-record checks, per-source identity, the actual
first growth, and two error paths an
[independent review](astra-hunter-size-gate-review-2026-09-13.md) reported: two
unreadable snapshots certifying each other as identical, and a matching census
prefix passing as full stream identity. Both were real defects in this reducer and
are fixed; that review's two probes now pass.)

Branch tests at `ae866a5`: `cargo test -p cubarium-core` — **153 lib** (16 in
`hunter_size_gate`, one new for `FlowLedger::validate`) plus every integration
suite, 0 failures. `cargo test --release -p cubarium --example hunter_compare` —
**78**, 0 failures.

### Launch-ready for all twelve

The same two commands with `--seeds` omitted collect all twelve seeds under both
recipes. That is **not** started and needs root's review of this package first.

## What I did not do

No seed beyond 1 was started. No 24- or 72-hour run. No parameter was tuned in
response to the pilot. The pilot outputs are retained as produced. The retained
charging artifacts, the diagnostic branch and `main`'s core are unchanged, and
nothing was added to the cube's startup configuration. This is not a completion
or live-introduction claim, and the user's goal remains much broader than this
screen.
