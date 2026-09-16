---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 4: results of workstreams P, Q, R and S (2026-09-16)

Fable's consolidated result for the reconciled next steps of
[the round-3 result](ecology-v1-round3-results-2026-09-16.md), in the order
Astra set in [its round-3 review](ecology-v1-round3-review-2026-09-16.md).
Briefs: [P](../handoffs/ecology-v1-apex-predicate-opus-2026-09-16.md),
[Q](../handoffs/ecology-v1-es-antithetic-opus-2026-09-16.md),
[R](../handoffs/ecology-v1-depth-census-opus-2026-09-16.md),
[S](../handoffs/ecology-v1-precondition-opus-2026-09-16.md), committed at
`15e13c8`. Four Opus workers in parallel: Q on `main`, P, R and S in worktrees.
These are the measurements that precede three owner decisions (the apex
predicate, the skimmer's roster depth, a preconditioned opening), each of
which would be visible on the cube; none of this round touches the cube.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| Q antithetic ES analysis | Opus 5 high | `main` (`da2bdc8`…`a281162`) | 1.4 s / 5 min | 0.24 MiB / 20 | search 230, core 517 |
| P apex predicate pair | Opus 5 high | worktree | *(pending)* | | |
| R skimmer depth census | Opus 5 high | worktree | *(pending)* | | |
| S preconditioned opening | Opus 5 high | worktree | *(pending)* | | |

Fable's verification so far: Q's two ignored experiments re-run and the
retained files rewritten with identical generation-9 figures (span
6,459–8,915, sd 643, concordance 15 and 12 of 16, 2 masked); its integration
tests pass.

## Q — where the ES search loses candidate variation: it does not; there is too little at source

Full note: [ecology-v1-es-antithetic-2026-09-16.md](ecology-v1-es-antithetic-2026-09-16.md).
Commits `da2bdc8` (definitions and 11 tests against `todo!()` bodies, red
first), `214df60` (the reduction, the step geometry, the deadband occupancy,
two integration tests that run a real generation through the trainer and
replay it), `a281162` (note). No change to the trainer's score, update,
sampler or protocol; no core change.

**Integrity checks first.** Replaying the sixteen recorded updates from the
initial centre reproduces the checkpoint with max |Δθ| = 0; every recorded
gradient norm is recomputed exactly; all 512 candidate-layout scores
re-derive from the episodes; Astra's generation-9 figures reproduce (span
6,459–8,915, sd 643, r = 0.81 with intake, r = 0.54 with opening residence).

| generation | candidate span | sd | centre | best | pairs concordant on intake per tick | masked by the minimum | retained ÷ orthogonal reference |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6,478–7,997 | 398 | 6,521 | 7,997 | 11 / 16 | 6 | 1.003 |
| 3 | 6,377–7,864 | 459 | 7,852 | 7,864 | 14 / 16 | 5 | 1.000 |
| 7 | 6,456–8,681 | 577 | 8,227 | 8,681 | 16 / 16 | 0 | 1.003 |
| 9 | 6,459–8,915 | 643 | 8,703 | 8,915 | 15 / 16 | 2 | 1.014 |
| 12 | 6,400–10,276 | 834 | 7,596 | 10,276 | 11 / 16 | 5 | 0.992 |
| 15 | 6,473–9,045 | 757 | 7,110 | 9,045 | 9 / 16 | 9 | 0.999 |

Pooled over 255 informative pairs: the preferred member has more intake per
lived tick in 76 %, more opening residence in 62 %; 29 % of pairs are ranked
by a layout the estimator's mean would not choose.

**Verdict by Astra's rule: refuted as an optimiser or update problem.**
Nothing cancels (the summed pair contribution retains 1.00× the orthogonal
reference in every generation; opposed pairs are not a thing in 10,215
dimensions with 16 draws) and nothing is erased (the replay is exact; the
centre sits in the top quartile of its own population for nine consecutive
generations; the population mean climbs 6,896 → 7,655). But the refutation
branch's premise also fails: candidates plainly do feed and reside more when
they score more. The honest statement is neither branch verbatim: **the
reduction and the update are faithful, and the residence variation they have
to work with is real and about two orders of magnitude too small** — the
whole candidate range of opening residence in generation 9 is 0.8–5.5 % of
life against a route-follower's 97 %, and the search gains about 50 ticks per
generation against a 29,000-tick gap. Attention moves, as the refutation
branch directs, to parameterisation and the adapter.

**Deadband occupancy** (generation-9 centre ± σε, 768 samples, 12 layouts):
thrust is never inside its deadband; the turn head is clipped to exactly zero
on 100 % of ticks from a reset state (29 of 32 candidates) and on 42 % of the
carried trajectory (candidates 12–77 %), with the centre's mean |turn head|
0.060 against an edge of 0.050 — the channel straddles the clip. **Named next
change, not implemented, the score untouched:** the adapter's turn deadband
operating point — residence requires stopping and turning, and this is the
one measured place where weights → behaviour throws away most of what the
search puts in; it changes what a policy can express, so it is a fresh run
under a new protocol hash, never a migration. Ranked behind it: the
four-layout minimum as the aggregation (it inverts the estimator's preference
on 29 % of pairs; `Aggregate::Mean` exists and hashes as a different task, so
a paired A/B costs no code; the minimum is what forces generality, and that
risk is stated), then σ after the adapter so the two are not confounded. The
rank reduction and the pair count are ruled out by measurement, not deferred.
Deliverable 3 (reconstructing the 32 candidates for H's exact on-food column)
was not needed: intake is material that left the stand through the mouth, so
it already requires on-food residence, and it separates the preferred member
in 76 % of pairs.

## P — the apex pursuit predicate, paired

*(pending)*

## R — the skimmer at depth 0.55 in a reproducing world

*(pending)*

## S — a plant-only preconditioned opening

*(pending)*

## What this does and does not establish

*(after P, R, S)*

## Next recommendation

*(after P, R, S, reconciled with Astra)*
