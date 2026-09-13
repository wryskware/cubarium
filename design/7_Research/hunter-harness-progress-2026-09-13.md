---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Six-arm hunter measurement: executable foundation and open gates

Root implemented `crates/cubarium/examples/hunter_compare.rs`, its independent
`hunter_compare/audit.rs`, and the `hunter_observers` test target. Astra supplied
the bounded recovery module in `bc93a98` and `ad48e05`. This is experimental
measurement code, not selected balance or a live introduction. Fable's Lanternjaw
remains the chosen body; no art was shortened to fit unconfirmed core contact.

## Implemented and exercised

The command requires the completed unfiltered seed1–12 preparation archive. It
checks each exact schema9 two-hour snapshot SHA256, seed, tick, ecology projection,
population and complete-cohort stratum before starting. Every seed runs all six
arms in lockstep: untouched, budget control, specialist off/on and facultative
off/on. Placement is the predetermined Top grid; paired profiles differ only in
attacks_enabled. No care, seed filtering, rescue, live state, web or shim output.

An output directory must be new. It retains the preparation manifest, frozen
executable with SHA256, build ID, per-arm full config/profile and profile SHA256,
actual initializer receipt/heading, common pre-import inventory, post-initialization
and closing snapshots, streamed events/census, and per-seed/arm summaries. Hashes
are strings, not lossy JSON numbers. It accepts the complete72h bound5184000ticks;
shorter diagnostic runs are explicitly not the preregistered two-hour screen.
Resuming partial measurements is not implemented or implied.

The audit opens BEFORE imports and preserves that opening across all arms. It
counts gut material/energy separately from the ordinary body, gives adult
structure zero chemical energy, includes escrow, and compares fixed original
inventory-scaled limits for material/water, persisted compensated energy,
independent windowed energy and initializer identities. The old raw energy
diagnostic remains separate. Actual budget-control cap heat is included once
from its receipt, because current core does not add immediate initializer heat
to transient step telemetry. Source changes without another actual receipt fail;
unopened or previously failed audits cannot claim passing.

Each tick checks invariants and full generation-bearing birth/death membership.
Capture IDs must reconcile one-for-one with predation deaths. Duplicate offspring
events and wrong parents are rejected. Living ancestry is bounded; observer
escrows link real parent escrow to each reported child. Starts, closure without
birth, full-structure maturation and descendants reproducing are recorded, without
pretending post-step values isolate exact funding debits/heat or closure cause.
Whole-world total/form recovery and exact first-zero/decline tracking are wired.
Adult occupancy bins0/1/2/>2, maxima, longest>2 episode and zero-hunter intervals
are measured per tick; adult means full decoded structural size, not the older
presentation70% juvenile cue. That measurement definition is explicit.

Initializer failures retain error JSON and state snapshots where available.
Initialization/finalization failures become technical seed failures with evidence,
not ecological extinction. Failed steps can partially update observers, so their
summaries expose the last complete observer tick, actual closing world population,
observer population and an explicit untrusted-partial-statistics warning. The
command exits unsuccessfully for technical failure; existing artifacts are retained.

## Evidence and limitations

Root's first all-seed smoke was200ticks/10s per arm, then400ticks/20s per arm:
`/tmp/cubarium-hunter-smoke-20260913-{1,2,3,4}/`. All72 arms completed each run.
Smoke2 and3 used the SAME frozen executable with200-versus20tick observer windows;
root deep-checked all72 full-state hashes, closing snapshot SHA256s, whole-recovery
outputs, biological counts and occupancy denominators for exact agreement.
Root also checked all paired profile differences, equal founder/control budgets,
common pre-import inventories and identical off/on headings from opening artifacts.

Smoke4 includes the subsequent failure-reporting and offspring-reconciliation
fixes. It completed at tick144400 for every arm, with all strict numerical gates
passing and all complete observer boundaries at144400. Its independent energy
peak is4.645173135031655e-12. Frozen executable:
`/tmp/cubarium-hunter-smoke-20260913-4/hunter_compare.frozen`, build label
`0.1.0+59149ce`, SHA256
`a3548c4151f1345f1b5efa575cba22c048851c89cb9088c9c32eaca177498591`.
Core and art workers were still editing when compiled; the SHA pins that actual
binary, while its commit label alone does NOT identify all in-flight source.

The test command is:

```sh
cargo test -p cubarium --example hunter_compare --test hunter_observers --offline
```

The example includes the22 audit/recovery tests plus placement,72h bounds,
checksum, duplicate/wrong-parent offspring, failed-step census and initializer
failure-artifact tests (28 distinct tests). The separate observer target repeats
the22 observer tests; do not add the two counts as unique tests. An initial
failure-artifact fixture incorrectly reduced capacity below legacy founders.count
and therefore failed before initialization; correcting both fixture values made
the targeted test pass. This was a fixture issue, not a waived production failure.
Astra's independent reviews are `ca1a0b7` and
`astra-hunter-harness-review-2026-09-13.md` (with correction follow-up).

## Still required before the biological screen

- Local recovery is now wired and its paired short smoke completed; see below.
  Default-profile capture-window behavior still needs the longer biological run.
- Complete/review actual hunting, digestion, paid escape and offspring core tests;
  link exact funding identities to the observer evidence. Current smoke duration
  does not establish successful predation or paid lineage replacement.
- Align capture geometry, sensing/pursuit, phase-end settlement and juvenile
  scale with the chosen visible claws. The reflected mouth and6px placeholder
  findings are recorded in `lanternjaw-core-art-integration-gaps-2026-09-13.md`.
- Add same-build hunter-on restart and observer-cadence tests through real capture,
  handling and reproduction; no-hunter/short-smoke identity is narrower evidence.
- Run all twelve seeds through the actual2h screen, then candidate24/72h stages
  with all controls, raw outcomes and censoring. Summaries currently explicitly
  set complete_experiment_measurement=false; technical smoke success is not a
  completed experiment, balanced profile or approval for the cube.

The longer presentation backlog (canopy authored growth, quiet habits, biological
care responses, tunable ambient support/doses and higher-resolution detail) remains
open in the animation roadmap. No new package was deployed to the live cube here.

## Subsequent exact-position local recovery integration

Core `22d8dba` supplies the actual pre-removal post-movement prey/root pose,
scaled contact geometry and the same paid key on Attempt/Capture. Root added
`hunter_compare/spatial.rs` and paired orchestration. Each Capture must match a
same-tick paid successful Attempt in full identity, target, key and evidence.
The living-ID-bounded counter map rejects repeated/regressing settled keys; it
does not pretend that every paid entry settles (a hunter can die in flight).
Successful attempts without captures, captures without attempts, control-arm
captures, malformed positions/inventories and duplicated prey/keys fail.

The adapter also recomputes the reported contact measure and both physical centers
from recorded root/heading/geometry, checking the latter against the scaled saved
profile. This uses shared read-only geometry helpers: it verifies internal event
consistency, not an independent reimplementation of the topology algorithm.
Astra found that matching copies of corrupted evidence previously passed;
the regression changes both events' prey position while retaining their cached
measure and proves refusal without consuming the good event's key.

One LocalRecovery observer receives all six worlds at the same completed tick.
It maps the prey's capture position, not the hunter/previous view, into the existing
FieldGraph. Neighborhoods contain every cell at graph distance <=3, crossing seams
and respecting the open rim. Captures precede that tick's census. Six strictly
prior 200-tick samples, first capture per 12000-tick elapsed bin per on-arm,
bounded follow-up and explicit censoring remain Astra's implemented rules.

Per-seed `local-exposures.jsonl` streams every capture's full paid key, cells and
six-arm immediate end-of-step total/form counts. `local-recovery.jsonl` streams
selected follow-up windows and paired total-prey changes. The seed result records
seen/selected/unselected denominators, last complete paired tick and failure trust
flags. Per-form exposure counts are not per-form recovery windows. If an arm fails
mid-step, the paired observer closes at its last fully shared tick, not a mixture
of early/late arm positions. Failed writes/observations remain technical failures.

Targeted example suite: **36 passed, 0 failed** (the original28 plus eight bridge/
orchestration checks). New coverage includes seam/rim/vertex graph neighborhoods,
real certain-probability capture at the prey cell, missing/mismatched/duplicate
paid evidence, jointly corrupted poses, hunter-excluding form census, a saved
post-capture world continued for1200ticks with matching event streams and full
hashes under20/200tick extra read-only observations, and paired partial-step/
same-census-boundary streaming and censoring. The latter exposure is deliberately
synthetic to isolate file/cadence order; the separate contact fixture uses actual
controller payment and settlement. Neither is a default-profile balance trial.

The generic observer target still repeats22 of these tests. The new restart test
does not include reproduction. Exact funding transactions/escrow closure causes,
full biological stages and current core/art phase/scale review findings remain
open. No hunter was deployed by this observer package.

### Frozen spatial-bridge smoke evidence

Detached worktree `/tmp/cubarium-hunter-spatial-frozen-DIZmNc` is exactly `6bc725c`,
excluding concurrent source edits. Its release build and its own separate36-test
example suite both completed successfully. Two runs used its SAME frozen binary:
`/tmp/cubarium-hunter-spatial-smoke-20260913-{1,2}/`,400elapsed ticks per arm,
audit windows200and20 respectively. Executable SHA256:
`0a231a0c475eb6e23c11d27252133c0852954e1482183032bf8405a32758e485`;
build label `0.1.0+6bc725c` truthfully identifies this isolated source.

All12seeds ×6arms reached144400 with strict audits passing; peak independent
energy residual in the200tick run was4.7126746949288645e-12. All72 closing full
hashes and snapshot SHA256s, every non-audit arm-summary field, paired local
summaries and local JSONL files match across runs. Complete paired/arm observer
coverage is144400, active windows after close zero, and seen-capture denominators
match the arm capture counts. **There were zero captures in these20second smokes.**
Their empty local streams verify plumbing, not natural capture/recovery behavior;
the actual-capture and synthetic-window tests above cover those paths separately.

This frozen build predates the subsequent post-settlement boundary correction
`327862c` and measured profile-v3 effector `9eacb7e`. It is not a candidate balance
result for that later profile. The current two-hour experiment will use a separately
frozen corrected profile; missing funding details and art integration are still
explicitly excluded from complete-experiment claims.
