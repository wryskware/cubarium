---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ordinary-fauna development flow: the skimmer grows, then is not replaced

Retention update, 2026-09-13: these are historical pilot findings. Raw captures
were deleted at Wrysk's request; the remaining ten replays finished without a
completed interpretation before cleanup. Source and committed compact reports
remain. Use the [current handoff](../handoffs/01-ecosystem-health.md).

Two full two-hour replays of the retained pre-hunter cohort, instrumented at the
mutation sites, examine a question the
[census follow-up](fauna-development-followup-2026-09-13.md) could only bound:
whether late skimmer losses reflect failure to develop. **In these two pilots**
the skimmer's growth branch runs, some descendants reach their adult target,
and every death is starvation at exactly zero
reserve and zero energy. What ends the seed-2 population is that budding stops
and the standing adults are not replaced. Seed2 is only one of the eight known
losses; the other seven loss histories are not certified by these two pilots.

Nothing was retuned, seeded, migrated or fed. No ecological configuration,
default, rate, threshold, gate, cost or geometry moved. This is exploration
evidence from two seeds. It contains **no counterfactual** and attributes
nothing.

## Provenance and isolation

| | |
| --- | --- |
| Base | `1d7b386` — the revision that produced the retained cohort |
| Branch | `diagnostic/fauna-development-flow-2026-09-13`, head `cdeeb55` |
| Worktree | `captures/diagnostic-source/fauna-development-flow-2026-09-13` (gitignored) |
| Build cache | `captures/build-cache/fauna-development-flow` |
| Patch on `main` | [`assets/fauna-development-flow-instrumentation-2026-09-13.patch`](assets/fauna-development-flow-instrumentation-2026-09-13.patch), refreshed from `cdeeb55` |

### Which binary produced which artifact

Three executables exist. They are **not** byte-identical to one another, and no
claim here says they are: the release profile carries debug info, whose line
tables embed a per-source-file checksum, so editing any source file — even a
`#[cfg(test)]` block that the release build excludes — changes the executable's
hash while changing no compiled behaviour.

| Source commit | Binary SHA256 | Produced |
| --- | --- | --- |
| `3868fdc` | `3e5891a7…664512` | the retained smoke `smoke-seed-1-20000.json` **and both full pilots** `seed-1.json`, `seed-2.json` |
| `4f00121` (adds test-only corrections) | `dba771ee…27596` | a seed-2 regeneration used only for comparison |
| `cdeeb55` (adds the output reservation) | `7289398d…b89e2` | a smoke regeneration used only for comparison, and the I/O-guard evidence |

The comparison scope is exact, and it is the artifacts rather than the
executables:

- `4f00121`'s seed-2 artifact equals the retained `seed-2.json` after removing
  **only** the `diagnostic_binary_sha256` field. Every other key, including all
  455/428 member records and all five gate blocks, compares equal. Astra
  reproduced this independently.
- `cdeeb55`'s 20000-tick smoke artifact equals the retained
  `smoke-seed-1-20000.json` after removing **only** that same field.

So the two edits after `3868fdc` are behaviour-preserving as measured, and the
retained pilots continue to stand for the committed head. They were not rerun,
because neither edit touches the step, the ledger or the gates; an I/O-only
change is established by a short smoke, not by two more full replays.

**Why `1d7b386` and not `512ee52`.** The retained cohort
`captures/hunter-openings-2026-09-13` is schema 9, build label `0.1.0+1d7b386`.
`1d7b386` is *pre-hunter*: there is no `hunter.rs` and `world.rs` names no
hunter, so the four ordinary forms are the entire fauna and there is no
hunter-only membership assumption available to import. `512ee52` is roughly 120
commits later and carries a schema 12 migration; the retained closings cannot be
reproduced there at all. Closing identity below is therefore a **same-schema 9 →
9 comparison**, and the cross-schema caveat that constrained the
[hunter juvenile diagnostic](hunter-juvenile-flow-diagnostic-2026-09-13.md) does
not apply here. This is a baseline choice, not a schema 12 claim.

Reproduction was established before any instrumentation was written. With the
retained frozen `pre-hunter-runner` (SHA256 `725305bc…21d0f6`), both a
fresh-from-seed run and a resume from the retained tick-zero snapshot reproduce
the retained closing byte for byte — seed 1 `07ad2da5…4831f5`, seed 2
`03ccf13e…0c39f1`. A binary built *now* from unmodified `1d7b386` with the
current toolchain reproduces seed 1's closing identically, so the current
toolchain has not drifted from the frozen build and the identity gates are
sound.

The diff is four files: a new `crates/cubarium-core/src/devflow.rs`, one line in
`lib.rs`, an `Option` field plus the recording sites and the ledger controls in
`world.rs`, and a new `crates/cubarium/examples/fauna_development_flow.rs`. Only
**six** pre-existing lines are touched, and all six are renames rather than
rewrites: the growth step's four caps and the budding affordability predicate's
two sides are bound to names so the binding cap and the blocking side can be
identified. Same operands, same order, same `min` chain; the one removed short
circuit is on a side-effect-free `f64` comparison. Gate 3 proves the result.

`cargo fmt --check` is **not** a usable gate at this revision: the current
rustfmt wants to reformat 26 files in `cubarium-core`, including many this work
never touched. The claim made here is the narrower one the diff supports.

## Five gates, both pilots

Each replay refuses to report unless all five pass, so a silent failure is not
available. A short `--ticks` prefix reports closing identity as *not asserted*
rather than as passed.

| Gate | Seed 1 | Seed 2 |
| --- | --- | --- |
| 1. Retained input identity | PASS — sha256 `238238a9…68d41e`, state hash `17068701073499880296` | PASS — sha256 `76309e37…39aa9a19`, state hash `2069952022073752900` |
| 2. Closing ecology identity (9 → 9) | PASS — sha256 `07ad2da5…4831f5`, state hash `8611632631396418712`, forms `[38,27,32,4,0,0,0,0]` | PASS — sha256 `03ccf13e…0c39f1`, state hash `443919231205506543`, forms `[48,17,23,0,0,0,0,0]` |
| 3. Observer on/off neutrality | PASS — identical closing bytes, 785 events record for record, all 144000 per-tick counters, both residuals bit-identical | PASS — same, 744 events |
| 4. Flow/stock reconciliation @ 1e-12 | PASS — 13,513,740 checks, **0 violations**, 0 unregistered records, worst residual 4.06e-16 energy / 1.66e-16 reserve / 5.59e-17 structure | PASS — 12,326,932 checks, **0 violations**, worst 4.09e-16 / 1.64e-16 / 5.59e-17 |
| 5. Census reconstructed from members alone | PASS — 1440 samples, no disagreement | PASS — 1440 samples, no disagreement |

Gate 2 asserts the *state*: the build label is supplied to the encoder, not
derived from the diagnostic binary, so it concerns the world rather than the
executable's identity. Gate 5 is the one that matters for this question — the
per-individual birth and death boundaries alone reproduce the retained
`telemetry.jsonl` at every one of its 1440 hundred-tick samples, so the records
*explain* the observed decline rather than merely coexisting with it.

`cargo test -p cubarium-core`: 137 lib tests (13 in `devflow::tests`) and every
integration suite, 0 failures; 195 tests in the full workspace run.

## The skimmer develops

This rules out a universal developmental stall in these pilots, not a resource
constraint on juvenile development. Counts are
individuals over the whole two hours, both pilots, from the per-member records.

| Seed 1 | grazer | glider | burrower | skimmer |
| --- | ---: | ---: | ---: | ---: |
| Members ever (founders + born) | 156 (10+146) | 166 (5+161) | 95 (4+91) | **38 (5+33)** |
| Reached the adult target | 117 | 127 | 59 | **27** |
| Died (all starvation) | 118 | 139 | 63 | **34** |
| Censored at 144000 | 38 | 27 | 32 | **4** |

| Seed 2 | grazer | glider | burrower | skimmer |
| --- | ---: | ---: | ---: | ---: |
| Members ever | 202 (10+192) | 135 (5+130) | 74 (4+70) | **17 (5+12)** |
| Reached the adult target | 146 | 100 | 48 | **11** |
| Died (all starvation) | 154 | 118 | 51 | **17** |
| Censored at 144000 | 48 | 17 | 23 | **0** |

The adult totals include five already-adult skimmer founders in each seed.
Actual descendants reaching the target are **22/33 in seed1 and6/12 in seed2**;
11/17 is not the seed2 juvenile maturation fraction. The independent review also
reconstructs the reserve prerequisite closed on571704/600586 juvenile ticks
(95.191%) in seed1 and213785/223348 (95.718%) in seed2. Resource permission still
limits when development can happen, even though admitted increments are paid.

**Entered growth steps are rate-limited, not resource-cap-limited.** This does
not show that the resource prerequisite never withholds entry on other juvenile
ticks. Across
both pilots the branch was entered on every single tick on which both gate sides
were open — 28,882 of 1,148,136 skimmer member-ticks in seed 1, 9,563 of 474,615
in seed 2 — and every entry gained structure. Of those entries, the binding cap
was the intrinsic `growth_rate · dt` on 28,860 and 9,557 respectively, the
remaining structure headroom on 22 and 6, and the reserve or energy cap on
**zero**. This exercises two of the four cap counters that the hunter
diagnostic left dormant; the reserve and energy caps remain unit-test evidence
only.

That is the opposite of the hunter juvenile case, where the reserve side of the
gate was shut for every tick of both observed children's lives.

## What the deaths and the boundaries say

Every death in both pilots, all four forms, is `starvation`, with reserve and
energy both at exactly 0. The death boundary is one tick after the decision for
every member (the core stamps `now + 1`), and that **final interval is recorded
explicitly rather than assumed** — it is 1 for all 354 and 340 deaths.

Same-boundary slot reuse occurs once, in seed 1: member `39:4` dies at boundary
116978 and `39:5` is placed into the freed slot at the same boundary. The
generation-bearing ID keeps them distinct, and the pair is recorded on both the
death and the birth. Seed 2 has no such case.

**Seed2's extinction follows a recruitment gap, not a universal failure to mature.** The
last skimmer birth is at tick 57135 (47.6 min). No skimmer is born afterwards.
The five remaining animals die over the next 26.3 minutes, the last at tick
**88708** (73.9 min) — inside the retained telemetry's first-zero bracket
(88700, 88800]. Seed 1 keeps budding until tick 136666 (113.9 min) and censors
four survivors.

## Every requested bud was funded

Across both pilots and all four forms, `bud_decisions == funded` **exactly**:
839 budding decisions, 839 fundings, zero blocked by reserve, zero blocked by
energy, zero blocked by the population cap, zero blocked by an escrow already
held, zero refunds, zero miscarriages. 835 of those became placed children; the
other four were still gestating at the horizon. When the controller decides to
bud in these pilots, the animal can pay at the funding site. The recorded
difference is the *rate* of budding decisions, not rejected funding requests.
This does not rule out reserve/energy-dependent eligibility in the controller
before a request exists, nor explain why those requests become less frequent:

| per million member-ticks | grazer | glider | burrower | skimmer |
| --- | ---: | ---: | ---: | ---: |
| Seed 1 budding decisions | 36.7 | 36.3 | 23.6 | **28.7** |
| Seed 2 budding decisions | 37.0 | 38.3 | 21.6 | **25.3** |

## Lower assimilated intake without downstream contention

The own-cell measurements are **potential access**, recorded at the cell the
member occupies. They are not intake, and an occupancy count identifies no
cause.

| Seed 1 | grazer | burrower | skimmer |
| --- | ---: | ---: | ---: |
| Mean own-cell producer | 0.2291 | 0.0959 | **0.2226** |
| Ticks with own-cell producer at zero | 0.11% | 50.28% | **0.61%** |
| Access class `both` | 99.89% | — | **99.39%** |
| Contested ticks | 0.00% | 0.00% | **0.00%** |
| Actual ÷ requested intake | 1.000 | 1.000 | **1.000** |
| Assimilated reserve per 1000 member-ticks | 0.169 | 0.116 | **0.118** |

The skimmer stands in a cell holding producer on 99.4% of its ticks, at
essentially the grazer's density, and loses **nothing** to contention: its share
multiplier never falls below 1 and it always receives exactly what it requested.
Its assimilated reserve intake is nonetheless about30% below the grazer's per
member-tick. Seed2 records0.110 assimilated reserve per1000 member-ticks.
Skimmer raw actual intake is0.19977/0.18636 per1000 member-ticks in seeds1/2;
do not conflate that with credited reserve. A sharing multiplier of1 excludes
downstream contention loss, not supply effects on the request: the bite formula
already multiplies by `food/(food+K)`. These observations therefore do not
separate intrinsic request rate, effort and density-dependent saturation.

Frugivory is exactly zero for every skimmer in both pilots; its intake is
grazing with a small scavenging tail.

## The sensing bill does not scale, and the skimmer is smaller

Upkeep demand is carried as three terms. The payment is a single clamped amount
and is deliberately **not** split across them; shortfall is reported separately.
Skimmers were underpaid on 36 ticks of 1,148,136 in seed 1 and 17 of 474,615 in
seed 2 — upkeep is essentially always paid in full. (Burrowers, by contrast, ran
short on 6,789 and 10,083 ticks.)

| per 1000 member-ticks | grazer | glider | burrower | skimmer |
| --- | ---: | ---: | ---: | ---: |
| Sensing demand, seed 1 | 0.0595 | 0.0596 | 0.0587 | **0.0600** |
| Sensing demand, seed 2 | 0.0595 | 0.0601 | 0.0594 | **0.0600** |
| Sensing share of upkeep demand, seed 1 | 19.4% | 18.9% | 27.3% | **26.6%** |

**Sensing is a flat bill.** Every form pays about 0.06 per 1000 member-ticks
regardless of its size, because the term is `sense_cost · sense_radius · dt` and
carries no structure factor. Maintenance does scale — its per-structure rate
tracks the genome's metabolism exactly (0.13756 / 0.19676 = 0.699 against the
skimmer's 0.7). The skimmer's adult target is 0.903 against the grazer's 1.008
and its reserve ceiling 0.852 against 1.025, so the same flat sensing bill is a
larger fraction of a smaller animal's budget: 26.6% against 19.4%.

This replicates, for an *ordinary* form and with no hunter anywhere in the
world, the structural observation the hunter juvenile diagnostic made about a
juvenile. It is **not** authorization to change `sense_cost`, its scaling, or
anything else.

Reserve outflow composition is, by contrast, unremarkable: oxidation takes
77.7% of the skimmer's reserve outflow in seed 1 against the grazer's 76.9%,
with growth ~10% and reproduction funding ~12% for both.

## What this does not establish

- **There is no counterfactual.** Nothing was changed, so nothing here
  attributes the decline to sensing cost, to intake rate, to budding rate, or to
  anything else. These are two realized paths, described.
- **Two seeds are not a cohort.** Seed 1 retains skimmers and seed 2 loses them;
  ten seeds remain unrun, including seven of the eight known losses.
- **A small population is a weak statistic.** The skimmer starts from 5 founders
  against the grazer's 10, and 17 members over two hours is a thin basis for any
  rate comparison. The difference between the two pilots' budding rates (28.7 vs
  25.3 per million member-ticks) is not shown to be outside seed noise.
- **Own-cell availability is potential access.** It is not intake, and neither a
  bare encounter nor an occupancy count identifies a cause.
- **Upkeep payment is not split** across its three demand terms; the shares
  above are of demand.
- **Two growth caps remain dormant.** The reserve and energy caps never bound a
  step in either replay; only their unit test covers that path.
- **Survivors are censored, not successes.** Forms 4–7 have zero exposure here:
  a null ratio, not a successful zero-percent outcome.

## Reduction and reproduction

The read-only reducer checks an artifact for complete expected coverage — every
gate, every form slot including the zero-exposure ones, and every member's
lineage, paid birth, boundary, reconciliation, potential-vs-actual intake, and
upkeep closure. It refuses a prefix run presented as two-hour coverage, a pooled
zero that hides a per-member violation, a slot reuse recorded on one side only,
a cross-schema payload comparison, and a u64 hash carried as a JS number. It is
an **artifact reduction, not a replay**: the runtime gates were asserted inside
the harness against the retained snapshots, and the reducer restates rather than
re-derives them.

Astra's independent review found four malformed artifacts that the first version
accepted. Each is now a refusal with its own regression, and Astra's probe runs
unchanged against the fixed reducer:

| Accepted before | Now refused because |
| --- | --- |
| `forms[3].reached_adult_target` inflated 27 → 38 | every form-row count is rebuilt from the member records — members, founders, descendants, recruitments, survivors and deaths by cause — not just the member total |
| `samples_compared = 1440` with an empty `reconstructed_series` | the series is the evidence behind the count: it must be present, match the count, rise strictly, keep all eight form slots, sum to its own population, and end at the horizon |
| a child with `parent_energy_debit = -1` | every birth-payment amount must be finite and non-negative, and the energy debit must equal `build_heat + escrow.energy` exactly, as the funding site computes it |
| a member missing `worst_residual.energy` | a residual must carry exactly the three stocks; a dropped key previously read as a clean reconciliation for a stock nothing checked |

The general lesson is that a summary is only evidence if it is derivable from
the records it summarises. The first version checked several summaries against
themselves.

```sh
node --test scripts/fauna-development-flow.test.mjs
node --test scripts/astra-fauna-flow-pilot-review.test.mjs
node scripts/fauna-development-flow.mjs \
  captures/fauna-development-flow-pilots-2026-09-13/seed-1.json \
  captures/fauna-development-flow-pilots-2026-09-13/seed-2.json
```

Root's follow-up found one remaining coverage gap: a nonempty, final-only census
could still pass. The reducer now requires exactly1440 samples at the retained
100-tick cadence, with nonnegative safe-integer totals and all eight form counts.
The synthetic valid fixture now carries every scheduled sample, reconstructed
from its members' birth/death boundaries, rather than only the final census.
25 reducer tests, Astra's4 independent probes run unchanged, and the reduction
of both real pilots all passed on2026-09-13. This changes only artifact validation,
not biology or the executable used for the replays.

## Outputs are reserved before anything is read

`cdeeb55` makes the harness reserve its output path with `create_new` **before**
it opens the cohort or replays anything, and write through that retained handle.
An existing file, a directory, a symlink, or the empty reservation left by an
interrupted run are all refused. Its seven example tests pass.

The ordering is what protects retained evidence, so it is verified rather than
assumed. Pointing the harness at an occupied output *and* a nonexistent cohort
fails on the reservation, never reaching the cohort:

```
Error: output must be a NEW file: …/occupied.json
Caused by: File exists (os error 17)
```

The control run — the same nonexistent cohort with a free output path — fails
instead on the cohort read (`initial-manifest.json`, os error 2), which is what
shows the cohort read would genuinely have happened and that the refusal above
really is ordered ahead of it. The occupied file's bytes were unchanged
throughout. Evidence is retained under
`captures/fauna-development-flow-io-guard-2026-09-13/`.

One operational consequence: a run interrupted after reservation leaves a
zero-byte output that blocks a retry at the same path, by design. Choose a new
path rather than deleting the reservation, so an interrupted run stays visible.

To reproduce a pilot:

```sh
cd captures/diagnostic-source/fauna-development-flow-2026-09-13
export CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/captures/build-cache/fauna-development-flow
cargo build --release -p cubarium --example fauna_development_flow
O=/home/wrysk/wryskware/cubarium/captures/fauna-development-flow-pilots-2026-09-13
"$CARGO_TARGET_DIR/release/examples/fauna_development_flow" --seed 1 --out "$O/seed-1.json"
```

Each full pilot is two 144000-tick replays and takes about 60 s. `--ticks N`
runs a prefix and says in its own output that closing identity was not asserted.
`--from-seed` constructs the world from the seed instead of loading the retained
tick-zero snapshot, as an investigation lever.

The ten remaining seeds are **not launched**. Seven
of the eight known losses are among them, and the replacement-failure reading
above rests on one extinction. Running them is the obvious next step and needs
root's review of this pilot and harness first. No parameter change is authorized
by this diagnostic, and none is proposed here.
