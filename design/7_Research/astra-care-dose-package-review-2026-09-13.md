---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Adjustable dose package: independent review

**No concrete rollout blocker found** in the bounded backend review of core
`2f92076`, host `1065dba` and report `e1aa426`. Root owns actual browser/runtime
acceptance and deployment. This review did not touch the live world or Fable's
in-progress presentation work, and does not establish ecological balance.

Lore and the handoff were checked against actual source. The two earlier contract
gates are implemented: legacy `accepted` rejects any dose field, while
`accepted_dose_v1` requires a valid integer; status advertises the exact version,
bounds and standard value, and retained rows preserve their requested dose.
Unknown complete journal records fail before recovery; only an unterminated
suffix is truncatable. Standard writes keep the old record shape. Amount passes
unchanged through prepared/planned commands, durable serialization and runner
conversion, and participates in duplicate identity. Existing stale/retired and
cooldown semantics remain unchanged.

Core validates both admitted and persisted doses. The standard branch returns
the nominal value directly; feed's per-cell multiply then energy multiply,
rain's depth/weight/envelope ordering, and cleanup's half-stock/per-cell cap and
proportional energy export remain in their old order. Nonstandard amounts scale
only nominal totals. Allowance exhaustion refuses rather than reducing a feed.
An active shower keeps its own amount across another refused shower and restart.
The no-input water path remains `None`, without new ecological operations or RNG.

Schemas 8/9/10 now refer to frozen pre-dose care/shower shapes; schema 11 has its
own mirror with that same care shape. Migration preserves old progress and adds
standard dose, and schema 10's non-empty hunter-history refusal remains. Legacy
projections refuse nonstandard in-flight showers; schema-7 ecology hashing still
drops extensions and is not a full-state replay identity. Schema-11 HunterState
still borrows the current shape, a documented future migration obligation rather
than a defect introduced by this package.

## Executed evidence

Focused foreground suites passed, **140 tests total, zero failed on the completed
permission-appropriate runs**:

- Core `care`, `care_dose`, `care_dose_migration`, `energy_correction`,
  `hunter_migration`, `snapshot_hardening`: 13 + 15 + 6 + 7 + 6 + 8 = **55**.
- Host library `care::`: **40**, including journal bounds, uncertain writes,
  directory durability, mixed discriminators, malformed tails and duplicate dose.
- Runner `care_replay`: **10**, including mixed-dose replay, nonstandard shower
  resume, held boundary and uncertain-write recovery.
- Host library `sink::web::tests`: **35**, including actual HTTP amount parsing,
  capability and existing security checks.

The initial sandboxed runner suite had six passes and four failures caused by
`EPERM` when binding temporary localhost ports. The approved rerun passed all ten;
those first failures were not hidden or treated as ecology defects. Test ports
and state were temporary, not the live owner or state directory. No new regression
fixture was necessary because no production defect was reproduced.

Independently verified the retained old core care/world/snapshot source against
`e55501d`, the generator's source SHA and its executable SHA. Ran that **old**
generator into `/tmp/cubarium-astra-dose-fixtures-O2UWyi`: all four newly written
files match the committed schema-11 fixtures byte for byte. The current migration
tests then pass both 600-tick old-payload continuations: unfinished rain and a
real hunter extension, alongside existing genuine schema-7/8/9 continuations.
These continuations do not alone test a *new* post-load standard feed/clean
admission; those paths additionally have unchanged source arithmetic and direct
per-cell tests. Do not broaden the fixture claim beyond what it executes.

Small provenance correction, **not a rollout blocker**: the retained generator is
`/tmp/cubarium-pre-dose-lvTTlU/target/debug/examples/dose_fixtures`, and its source
is `examples/dose_fixtures.rs`, not the release/pre_dose_fixtures paths named in
the provenance report. Actual executable SHA is exactly the published
`405b02abe11247e1e9c3917941ba2f058abd2d527e5830fe15d7ba49fe92dcd6`;
source SHA is also the published value. Reproduction of the bytes confirms this
is a path/profile documentation error, not evidence of fabricated old snapshots.

A Git tag is a suitable lightweight code checkpoint. It does not make an old
binary understand schema 12 or dose-bearing journals: preserve a matched state
and journal when crossing that compatibility boundary. This is an upgrade safety
constraint, not a request for another review or approval ritual.
