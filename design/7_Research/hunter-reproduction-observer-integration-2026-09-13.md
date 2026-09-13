---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reproduction observer integration

Native Opus committed the read-only transaction observer in `d7d190a`. Root
connected it to each comparison arm immediately after draining both event queues.
It replaces inference from disappearing post-step escrows with the core's actual
Funded/Born/Refunded/Miscarried/NotFunded records. Actual funding quantities come
from core mutation sites; the observer independently recomputes their identities
and reconciles held escrows, offspring, birth and death records. It never claims
to isolate a transaction by subtracting post-step parental stocks.

Each summary now includes `reproduction_audit`, `open_gestations`, and
`last_complete_reproduction_tick`. These replace the old ambiguous
`observed_escrow_starts` and `escrows_closed_without_birth` fields; historical
frozen outputs keep their original format and limitations. Raw reproduction
records use the existing streamed hunter event channel. No new world, snapshot
schema, profile tuning, care action, or live output is introduced by this wiring.

Verification on the shared working tree after `d7d190a`:

```text
cargo test -p cubarium --example hunter_compare
56 passed, 0 failed
```

That is 36 prior audit/recovery/orchestration tests, three opportunity diagnostics
tests, and 17 reproduction tests. The latter include real core funding, birth,
refund, ordinary miscarriage, same-tick funding/death, stock/cap refusals, and
rejected corrupted transaction batches. Unit/module coverage does not yet prove
that every possible malformed batch is refused. Independent adversarial review
is in progress; suspected gaps around unrecognized refusal identities, repeated
same-tick transactions, and overflow remain to be resolved from concrete evidence.

`complete_experiment_measurement` deliberately remains false pending that review
and frozen cohort verification. The currently running profile-3 two-hour screen
uses its earlier frozen executable, not this new observer. It will not acquire
new evidence retroactively. The [opportunity replay](hunter-opportunity-probe-2026-09-13.md)
independently matches one arm's closing full-state hash but is not a substitute
for the full paired measurement. No claim of balance or live-readiness is made.
