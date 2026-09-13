---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Local care observer: bounded source review

Reviewed commit `4d10351`, specifically `examples/care_compare.rs` and
`examples/care_compare/local.rs`, against `local-care-observation-2026-09-13.md`.
The reviewed files still matched that commit when tested. Current AGENTS/Graft
instructions and Lore retrieval were consulted; canon remains unchanged. This is
permission to use the first measurement slice as scoped, not biological acceptance.

**Disposition: no significant correctness defect found for the planned fixed
openings × three targets × three isolated Standard actions, 600 ticks before and
2400 ticks after the input, period 72000, local cadence 20.**

- `run_with_options` observes every completed tick. The opening instant is a
  snapshot only, not counted member-time. `LocalObserver::observe/sample` reject
  skipped, repeated or mismatched tick boundaries. Counts are post-tick living
  organisms, not every transaction that might occur before a same-tick death.
- At elapsed 600 the existing cadence sample is pre-input. The next loop freezes
  full slot+generation IDs at that boundary, then applies care, then observes tick
  601. Fixed-cohort cumulative counts therefore exclude pre-pulse time; regional
  totals include it. Use regional counter differences from the elapsed-600 sample
  for post-input rates. The sample at that boundary has not yet attached the cohort
  object; separately published cohort metadata correctly names the same boundary.
- Fixed IDs are followed anywhere, while dynamic counts remain inside the frozen
  radius-three graph region. Death/reused slots cannot supply replacement members;
  newcomers/descendants are excluded. Empty cohorts remain `Some(empty)` and
  serialize with opening count zero. No percentage calculation divides by them.
  Regions overlap, and the report warns against summing them as disjoint groups.
- Feed/Rain/Clean each schedule at relative zero, independently of cycle offsets.
  The requested integer dose reaches the core unchanged. Period 72000 exceeds
  this horizon, so exactly one attempt occurs. A refused/partial cleanup remains a
  recorded outcome, not proof of a beneficial response. For longer runs, kinds
  repeat and targets rotate as documented; “isolated” does not itself mean one-shot.
- CLI/options bound elapsed duration, periods, targets, dose and local output to
  at most 2000 intervals plus the opening. The final non-cadence boundary is
  included without duplication. Observer memory is fixed regions/cohorts and
  bounded samples, not a per-tick event archive. Arithmetic fits u64 under the
  existing CLI duration and u32 organism-capacity bounds.
- Openings with a hunter profile are refused before stepping; constructing the
  World additionally validates state. This preserves the tool's no-gut inventory
  scope. Main also refuses an active shower and tick overflow. Local methods hold
  only immutable WorldState references and do not reset telemetry or draw RNG.

No path-length, sustained quiet-bout, edible-D, per-diet intake or flood-time claim
is made by this first package. Those are explicit limitations, not hidden missing
acceptance gates. `fed_this_tick` counts any actual field intake among the selected
living animals, not its amount, source diet or manual-crumb provenance. Fixed
opening traits do not convert the cohort into an eligible-scavenger denominator.
The forthcoming reducer should preserve these distinctions and retain zeros.

## Verification

Independent foreground `cargo test -p cubarium --example care_compare --quiet`:
**9 passed, 0 failed**. Existing tests exercise tick continuity, seam/rim regions,
actual intake versus mode, fixed selection after movement, all three isolated
actions/dose, delayed pre-pulse equality, report limits and full arm equality with
the local observer disabled after removing only its added output.

Independently compared the retained `/tmp/cubarium-care-new-default.json` and
`/tmp/cubarium-care-old-default.json`: both complete `baseline` and `cared` arm
objects are exactly equal. This verifies the retained comparison artifacts; I did
not rerun the historical executable or claim that one short seed proves long-run
behavior. No full response cohort, live action or production edit was performed.
