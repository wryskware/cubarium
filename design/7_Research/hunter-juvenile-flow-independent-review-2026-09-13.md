---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review — juvenile flow diagnostic

## Scope and verdict

Reviewed `0d867f1` against frozen `512ee52` artifacts and the two ledgers
committed in `efdfd3b`. The diagnostic is trustworthy for **realized-path stock
accounting** in the two replayed arms. It is suitable to replay the remaining
nine birth-producing retained arms, provided each continues to require all four
identity/reconciliation gates. It does not establish that charging is not a
cause of juvenile non-growth, or that removing the raised threshold cannot alter
development.

This is exploration evidence, not a parameter recommendation or authorization
for a cohort, retune, migration, or live-world change.

## Evidence checked

- The branch adds a transient, write-only `FlowLedger`; `World::step` never
  reads it to select a branch, draw, clamp, or state transition. The two replay
  modes agree on closing bytes/state and all event records for seed-8 (997
  event-emitting ticks) and seed-6 (757).
- The example verifies each retained opening's SHA256 and state hash before
  replay, then verifies the full-horizon output bytes and retained summary hash.
  It uses a supplied `512ee52` build label solely for byte comparison, so the
  output gate concerns state rather than the diagnostic executable's identity.
- Mutation-site review finds recordings at the stock assignments for strike,
  upkeep, field feeding, handling, digestion, member oxidation, growth,
  funding, refund, descendant registration, death, and the end-of-tick probe.
  The growth predicate is read before its branch. This covers the stock-moving
  paths exercised by the two retained replays.
- Reconciliation is a strong executable completeness check for **nonzero flows
  exercised on a replay**. It cannot demonstrate a dormant branch is complete;
  the growth branch did not run in either child, and its cap-attribution counters
  therefore remain unit-test rather than artifact evidence.

## Identity and counting correction

The original four-gate table reported seed-8 identity values and its 62,279
growth-gate observations (39,649 founder plus 22,630 child) while heading the
section as if it aggregated both arms. Its reconciliation has 62,280 checks:
the child death adds one removal-site check beyond its 22,630 live-tick gate
observations. Seed-6 independently passes the same gates, but must be reported
separately: 32,887 founder and 8,660 child reconciliation checks, zero
violations, and worst residuals `1.55e-16` reserve, `1.37e-16` energy, zero
structure. The main report is corrected to name the seed-8 table and spell out
seed-6's own identity evidence.

## Causal boundary

Supported observations are narrow:

- Both observed children remained juvenile and never entered growth.
- On each realized path, reserve never cleared the 1.2 growth gate; the measured
  reserve outflow was oxidation, while energy outflow was dominated by paid
  upkeep demand plus strikes.
- The child oxidation classifications above the world reference were 12.0% and
  0.0%; their adult parents' classifications were 100% and 92.6%.

Those classifications do not answer the missing counterfactual. A threshold
change can affect adults, funding and birth timing, prey interactions and later
juvenile stocks. Therefore neither “the burn below reference would have happened
under background policy” nor “the raised threshold contributed literally
nothing” follows from these ledgers.

## Replay recommendation

Replay the remaining nine listed birth-producing arms only with the existing
isolated branch, retained inputs, and dedicated build cache. Do not start a new
cohort or change parameters. Treat any identity, event-neutrality, or
reconciliation failure as a diagnostic failure to preserve, not an artifact to
repair. Aggregate results as observed trajectories; retain the explicit
no-counterfactual limitation.
