---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ordinary-fauna flow diagnostic: interrupted handoff review

**The original delegated run stopped; neither full pilot completed.** The
surviving source and successful prefix are useful, but the parent's exit 0 and
“Now running” final message are not evidence of a completed or continuing pilot.
Root has subsequently started foreground recovery session **58216**, with no
background delegation and fresh output paths. The observations below precede
that recovery and do not certify its later edits or results.

## Exact stopped-work evidence

- `/tmp/cubarium-opus-fauna-flow-resume-2026-09-13.jsonl` reports background
  task `b8zg7cgl4` as `stopped`; its subagent statistics report completed=0,
  system-killed=1. The stream also contains the 600-second background wait-ceiling
  termination message and a child interruption at `2026-09-13T13:36:53Z`.
- Its task output,
  `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/21d211ee-113f-4cbd-85d5-c4b431a8aba1/tasks/b8zg7cgl4.output`,
  contains `[killed]`. A host-level process inspection found no matching
  fauna replay, Cargo build or delegated noninteractive Claude worker; only four
  unrelated, long-lived interactive Claude processes. The sandbox's private
  PID namespace alone would not establish that absence.
- `captures/fauna-development-flow-2026-09-13/run-log.txt` records
  `smoke exit: 0`, then starts the seed-1 144000-tick instrumented replay.
  It has no completed instrumented replay, seed-1 exit or seed-2 start.
  The directory contains the binary hash, log and `smoke-seed-1-20000.json`;
  **neither `seed-1.json` nor `seed-2.json` exists** at this review boundary.

## Implemented versus verified

Isolated source:
`captures/diagnostic-source/fauna-development-flow-2026-09-13`, base `1d7b386`,
schema 9. Commit **7bf816545e01e272886432f1e91444898887a337** contains
`devflow.rs`, its export and mutation-site hooks. The public ledger-control
methods in `world.rs` are uncommitted; `fauna_development_flow.rs` is untracked.
This is not a fully committed diagnostic package.

The inspected hook diff keeps the ledger transient and write-only relative to
simulation decisions. Feeding records distinguish request from actual settlement;
growth permission is observed before the branch; paid growth/funding/refund/death
records are adjacent to the mutations. The original growth caps and payment
operands/order remain on inspection. The harness runs observer-on and observer-off
from the same opening and compares closing state, life events, tick-counter
digest and telemetry. This is a source assessment plus prefix evidence, **not a
completed two-hour observer-neutrality certificate**.

The recorded executable and current on-disk executable agree at SHA256
`3e5891a77869acd3174b2b158bdd6b6d0344bcf4aeccbc46fb34e7df21664512`.
The 20000-tick seed-1 smoke records 141 members, 200 census samples and 1,483,997
stock-reconciliation checks. Input identity, observer neutrality, reconciliation
and reconstructed-census checks report passing; full retained-closing identity
is explicitly **not asserted**. Worst recorded stock residuals are
S=5.594483210025203e-17, R=1.5677563414140394e-16,
E=4.0383820419653826e-16, against the ledger's fixed absolute 1e-12.

Independently compared existing baseline resume copies to the retained original
closing files, byte-for-byte:

| Seed | Baseline resume equals retained | Closing SHA256 |
| --- | --- | --- |
| 1 | yes | `07ad2da5b1978f246b197aa1bab0a27681b7dc78fb67c4db1d1665d2f74831f5` |
| 2 | yes | `03ccf13e4225212c828fcd74124804496572d0df6993d78abd9533bde50c39f1` |

Copies are under `captures/fauna-development-flow-baseline-2026-09-13/resume-seed-{1,2}`;
originals under `captures/hunter-openings-2026-09-13/seed-{1,2}`.
This supports baseline same-schema replay identity; it is not an instrumented
pilot result or an independently reconstructed build provenance.

## Focused tests actually executed

Against the surviving source above, using only workspace build output:

```sh
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/captures/build-cache/astra-fauna-flow-review cargo test --manifest-path captures/diagnostic-source/fauna-development-flow-2026-09-13/Cargo.toml -p cubarium-core --lib devflow::tests
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/captures/build-cache/astra-fauna-flow-review cargo test --manifest-path captures/diagnostic-source/fauna-development-flow-2026-09-13/Cargo.toml -p cubarium --example fauna_development_flow
```

Core ledger: **8 passed, 2 failed, exit 101**. Harness: **5 passed, exit 0**.
The two existing ledger failures appear to be fixture mistakes, not demonstrated
recording defects:

1. `devflow.rs:1019`, `a_tick_whose_flows_match_the_stocks_leaves_no_residual`:
   expected checks/violations `(1,0)`, actual `(1,1)`. Its energy arithmetic is
   `1 - .25 + .125 + .375 = 1.25`; the fixture supplies closing E=1.5.
2. `devflow.rs:1033`, `a_missing_mutation_site_shows_up_as_a_violation`:
   the test expects a signed -.25 worst reserve residual, whereas totals expose
   its absolute magnitude +.25. The violation itself is correctly detected.

Correct the fixture expectations without weakening the fixed tolerance or hiding
real residuals, then rerun. Harness tests cover hash primitives, removal-boundary
census semantics and tolerance identity, not the entire four-gate failure matrix.

## Recovery gates

The current harness writes output with `std::fs::write` at line 1327, which
overwrites an occupied path. Foreground recovery must preserve old artifacts;
root's fresh-path instruction handles the immediate run, and an explicit
no-overwrite refusal should protect subsequent use.

Still required: passing ledger tests; completed seed-1 and seed-2 144000-tick
artifacts with full input/closing/neutrality/reconciliation gates and the retained
census cross-check; scoped harness commit; the promised read-only reducer/tests,
research report and compact patch/provenance handoff. The proposed main files
`scripts/fauna-development-flow.mjs`, `scripts/fauna-development-flow.test.mjs`
and `design/7_Research/fauna-development-flow-diagnostic-2026-09-13.md` were absent.
Do not start the remaining ten seeds or infer a biological explanation from the
short prefix. No diagnostic/core code was changed and no replay was launched by
this independent review. Canon, Lore and Graft were consulted; this report is
evidence, not a new ecological decision.
