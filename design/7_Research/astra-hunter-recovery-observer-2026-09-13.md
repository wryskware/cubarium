---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Hunter recovery observer implementation

Implemented only `crates/cubarium/examples/hunter_compare/recovery.rs`, following
`hunter-experiment-contract-2026-09-13.md`. Root owns the surrounding harness and
added the host's direct serde dependency. This module contains no world ownership,
simulation mutations, RNG, surface traversal, files or clocks. Its inputs are
counts, capture identities/locations, absolute ticks and precomputed graph-cell
neighbourhoods. Public output records derive `serde::Serialize`.

## Interface to root

```rust
type Counts = [Vec<u32>; 6]; // arm -> field cell -> non-hunter population

CaptureId { hunter_slot: u32, hunter_generation: u32, attempt: u64 }
Capture { arm: usize, cell: usize, id: CaptureId }

LocalRecovery::new(opening_tick: u64,
                   neighborhoods: Vec<Vec<usize>>, // indexed by center cell
                   on_arms: Vec<usize>) -> Result<LocalRecovery, String>
local.record_captures(tick: u64, captures: &[Capture], post: &Counts)
    -> Result<Vec<LocalRecord>, String>
local.observe_sample(tick: u64, counts: &Counts)
    -> Result<Vec<LocalRecord>, String>
local.finish(tick: u64, reason: &str) -> Result<Vec<LocalRecord>, String>

WholeRecovery::new(opening_tick: u64, counts: &[u32])
    -> Result<WholeRecovery, String>
whole.observe_tick(tick: u64, counts: &[u32]) -> Result<(), String>
whole.summary(reason: &str) -> Vec<WholeRecord>
```

Create one local observer across all six arms, normally on-arm indices `[3,5]`,
and one whole-world observer per arm. Root labels whole-world channels, normally
total prey followed by eight form counts. Build graph-distance-three neighbourhoods
with the existing surface helpers. Local constructor validates unique in-range
cells and inclusion of each center, but does not claim to verify graph distance.

At the opening tick, provide the initial local sample. Thereafter step paired
worlds in lockstep. Call `record_captures` once with **all** captures at a tick,
before `observe_sample` at that same tick. Empty capture batches are optional.
The module sorts ties by full hunter/attempt ID and checks duplicate IDs within
a batch. Cross-tick capture/death deduplication remains the harness/core audit's
responsibility, not an unbounded retained-ID set here.

Samples must be contiguous at opening + n*200 ticks. Capture batches cannot skip
an intervening census boundary; whole-world input must be contiguous every tick.
Malformed shapes, invalid arm/cell/batch IDs, ordering, tick overflow and skipped
samples return errors before mutation. Output vectors must be streamed by root.

## Local behavior and bounded retention

Six strictly prior samples are retained. A full reference additionally requires
at least 1,200 elapsed ticks, so six samples spanning only the first 50 seconds
do not masquerade as a completed minute. Integer count sums are divided once by
six to avoid rounding a whole-number mean slightly upward before `ceil`.

The first capture in each 12,000-tick elapsed bin per on-arm is selected regardless
of whether its subsequent data are convenient. Insufficient-pre and no-deficit
statuses are explicit and never become recoveries. For a deficit, count must
reach `ceil(pre_mean)` for 1,200 ticks observed at 200-tick cadence; a sampled dip
resets the hold. Crossing and confirmation ticks are separate. Within-cadence
dips cannot be detected by this local sampling scheme and are not claimed absent.

All selected windows remain active until 72,000 ticks after capture, retaining later
milestones and recurrent exposures. All other captures in the same neighbourhood
are counted by arm, including same-tick nonselected captures and post-recovery
exposures; the index capture is excluded. Each control uses its own pre-mean for
the paired difference-in-change, not the treated arm's reference. When no complete
pre-reference exists, raw milestone counts still appear and the serialized
`differences_in_change: Option<[f64;6]>` is null. Root identified the loss of
follow-up in the initial immediate-output handling; this refinement keeps all
selected windows without changing the memory bound or relabeling their statuses.

Milestones request offsets 1,200 / 6,000 / 18,000 / 72,000 ticks. Because captures
rarely align to the census grid, the reported count is the **last cadence sample
at or before** the endpoint, with both requested and observed ticks explicit.
No future sample is pulled into a window or used to extend its one-hour horizon.
An unconfirmed crossing remains censored; timeout and run-end censoring are named
separately, with observed coverage and closure tick. Recovery status can be known
even when later follow-up ends early; confirmation and closure remain separate.

Retention is six six-arm cell arrays, fixed supplied neighbourhoods, and at most
seven windows per on-arm, each with no more than four milestones. For two on-arms
this is at most fourteen windows. The dense-capture test actually reaches seven
for one arm. Counts of seen/selected captures remain available for denominators;
the module never stores all capture or census history.

## Whole-population behavior

The first strict drop below 50% of opening count starts one six-hour window per
channel. A return to at least 90% must persist 36,000 ticks; every-tick counts
detect even between-census dips, and confirmation occurs on the 200-tick reporting
cadence. First decline, zero and crossing ticks are exact simulation ticks, not
sample-time estimates. A confirmation beyond the six-hour deadline is refused.

Absent-at-opening, no-decline, recovered, not-recovered-by-six-hours and
right-censored are separate statuses. Minimum population, first zero and time
below half continue over the entire census even after the first recovery window
closes: `census_until_tick` distinguishes their coverage from recovery's
`observed_until_tick`. `summary` is non-consuming, so intermediate 2h/24h reports
do not reset an ongoing window or discard its later recovery. Local `finish`, in
contrast, closes the local observer; do not use it at an intermediate checkpoint
if the same local windows will continue.

## Verification and remaining integration

Formatted only the owned module. Independently compiled it against the existing
serde artifact, without compiling the concurrently changing core:

```text
rustc --edition=2024 --test crates/cubarium/examples/hunter_compare/recovery.rs \
  --extern serde=target/debug/deps/libserde-e3bb7a4a6dc9e3b3.rlib \
  -L dependency=target/debug/deps -o /tmp/cubarium-recovery-tests
/tmp/cubarium-recovery-tests
```

Result: **16 passed, 0 failed**. Tests cover strict-prior ordering, stable ties,
partial prehistory, no deficit, hold reset, censored crossings, per-control means,
graph-cell aggregation, exact and off-grid deadlines, post-recovery follow-up,
full follow-up for non-deficit/insufficient-pre windows, bounded dense-capture
memory, invalid inputs, absent forms, exact decline/zero
ticks, between-sample whole-world dips and late unconfirmed recovery.

Root still must include the module, stream records, reconcile events, map real
capture positions/IDs and supply genuine graph neighbourhoods/counts. If core
capture location is not yet available, local recovery is unavailable, not inferred
from previous-tick positions. This is tested measurement logic, not completed
paired experiments, predator balance evidence or a live change.
