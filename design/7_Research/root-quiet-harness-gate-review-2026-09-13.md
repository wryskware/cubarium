---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Root review: quiet-harness certification is not ready yet

Reviewed the first harness, `6d4ac7b`, against the complete ordinary-quiet
proposal and its handoff. This review does not change the candidate policy,
autonomous default, or live world. The live cube remains the isolated schema-12
sail release `9cf0e1d`. Lore and Graft located the relevant contracts and source;
research is evidence, not a design decision.

## Reproduced acceptance gaps

Root loaded the real seed-1 `off_nocare`/`off_feed` smoke summaries from
`captures/quiet-smoke-probe`, changed **only copies in memory**, then called
`verifyArm`. The unmodified summary passed its low-level gate. Each of these
inconsistent copies also passed when it should have been rejected:

| Mutation | Observed result |
| --- | --- |
| Material limit changed to `1e20`, material drift to `1e10` | Accepted |
| No-care arm given Feed ledger totals without a receipt | Accepted |
| Feed's absolute application tick changed to 1 | Accepted |
| Feed receipt's declared outcome replaced with a rejection | Accepted |

The reducer's SHA-256 at reproduction was
`e3ea62335b6fff5bc8b10aa62824bc52e81a35b41a767d0be840bb2eae7eb1c2`.
Executable reproducer: [root-quiet-gate-probe.mjs](assets/root-quiet-gate-probe.mjs).
Original result: [root-quiet-gate-probe.json](assets/root-quiet-gate-probe.json).
These probe a low-level validator using the smoke's real 2400-tick duration;
they do **not** relabel a smoke as a ten-minute experiment or mutate its files.

Required corrections: independently derive the fixed limit from verified opening
inventories; reconcile complete care ledgers, sequence, absolute and elapsed
ticks, target, dose, outcome and actual material/energy amounts. Merely copying a
self-reported baseline into four arms does not make it independent evidence.

## Other source-level findings

- `Arm::verify_resume` reconstructs **both** compared worlds from the same encoded
  snapshot. Their agreement does not test uninterrupted versus resumed execution.
  Use the real ongoing timeline against a decoded shadow, including identical care
  when crossing its scheduled boundary; record which actual intervals were checked.
  The fallback at elapsed 400 can preempt the first actual pause and must not be
  described as a mid-pause check when that happens.
- `loadRun` accepts any integer duration at least 12000, rather than checking the
  exact horizon name/duration map. It never reads the recorded closing snapshot.
  Census length and last tick are checked, but interior coverage and flows are not.
  Factor, configuration, source-opening and snapshot identities need full checks.
- Same-face endpoint displacement is not exact transported path length at a
  reflecting rim. Tiny drift can cross a seam when an organism starts near it.
  Use real per-tick segments; the proposal's requested measurement should not be
  replaced by an unbenchmarked claim that cloning a render view is too expensive.
- Quiet record totals need identity-linked lifecycle reconciliation, not just
  global counts. Life events and opening identities must also be retained so
  parentage, deaths, recruitment and surviving ancestry can actually be inspected.
  A final count of surviving cohorts is not that history.

Astra owns separate independent observer regressions for release into ordinary
Resting, death on the fortieth held interval, and missing/duplicate quiet records.
Those are separate from the runner/provenance findings here; neither set of tests
should be weakened to certify the first harness.

## Disposition

**Do not launch the prescribed comparison yet.** Native Opus is correcting the
harness, validator and tests in a separate scoped follow-up. Keep the original
failed checks and use new output paths for subsequent smokes. Re-review the
committed corrections before the all-twelve-seed four-arm ten-minute screen.
No biological acceptance, deployment, or complete-goal claim follows from the
first harness's green unit tests or its smoke's self-reported audit flags.
