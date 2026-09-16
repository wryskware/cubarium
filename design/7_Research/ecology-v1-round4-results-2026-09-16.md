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
| P apex predicate pair | Opus 5 high | worktree, merged | 190 s / 4 min | 8.0 MiB / 20 | core 523, search 224 |
| R skimmer depth census | Opus 5 high | worktree, merged | 4.2 min / 6 min | 0.6 MiB / 30 | search +20 |
| S preconditioned opening | Opus 5 high | worktree | *(pending)* | | |

Fable's verification so far: Q's two ignored experiments re-run and the
retained files rewritten with identical generation-9 figures (span
6,459–8,915, sd 643, concordance 15 and 12 of 16, 2 masked); its integration
tests pass. P's four-seed reach-envelope arm re-run and compared field for
field with its retained rows (identical, timing keys excluded); its 13 new
tests pass on the branch and the suites after merge.

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

## P — the apex pursuit predicate, paired: confirmed, and the next term is the turn radius

Full note: [ecology-v1-apex-predicate-2026-09-16.md](ecology-v1-apex-predicate-2026-09-16.md).
Commits `d19150e` (the switch, tests, audit aggregations, CLI), `8d1639a`
(note). The switch is a `World`-level transient on the strike recorder (no
`WorldConfig` field, no new `World` field, since `lifecycle.rs` belonged to
another worker); `ContactMeasure::pursuit_holds(stop, …)` is the single place
either rule is written, so the record cannot transcribe a rule the world did
not run; `--pursuit-stop` refuses an unknown name rather than defaulting.
Tests first and red: before `step.rs` was touched, the behavioural tests
failed with "the burst was requested but not delivered: 0.0019 px/s against a
held cap of 0.21". The shipped arm reproduces K's and N's retained rows to
the attempt (449 paid, the same class histogram, 15 captures).

| | half-space (shipped) | reach envelope |
| --- | --- | --- |
| paid attempts (8 seeds, 32 lives) | 894 | 969 |
| **held at the burst's start** | **89.4 %** | **5.4 %** |
| gap change per burst | −1.07 px | −0.21 px |
| delivered burst: translation / turn sweep / whole motor | 2.7 / 1.7 / 4.4 px/s | 4.6 / **8.0** / 12.5 px/s |
| contacts | 88 | 140 |
| **captures per life** | **1.19** | **2.09** |
| fraction of its own bill earned | 7.7 % | 12.9 % |
| lifetime mean / max | 12,440 / 16,585 | 13,164 / 23,201 |
| death cause | starvation 32 / 32 | starvation 32 / 32 |
| prey population at the end | 864 | 847 |

Captures by initial gap is the clearest row: under the shipped rule every
capture began inside 8 px and 186 attempts that began at 8–12 px produced no
contact; corrected, 4–8 px nearly doubles and 8–12 px becomes productive
(17 contacts, 9 captures); past 12 px nothing changes in either arm.

**Verdict by Astra's rule: confirmed.** The held fraction falls sixteenfold,
closure improves, contacts rise 59 % and captures 76 %, lifetime rises 6 %
and the earned fraction of the bill goes from 7.7 to 12.9 %. **Disclosed
caveat:** the brief's four-seed pair ran first with contacts flat (46 against
46) and captures 15 → 21; P widened to eight seeds because sixteen lives
cannot separate a 10 % contact rate from 14 %, and that widening was not
pre-registered. What rescues it: the four added seeds are out of sample for
the decision to widen, and on them alone the effect is larger in every
direction (contacts 42 → 94, captures 23 → 46, the gap actually closes).

**Strike constants: not adequate once delivered, and raising them is not the
repair.** The delivered burst translates at 4.6 px/s against a nominal 16.7,
because 64 % of the boosted budget is turn sweep priced at
`motor::turn_radius_px` = |capture offset| + capture reach = 14.8 px; relative
closure is about 2 px/s, so the mean 11.4 px gap needs about 6 s, not 1.25.
The escape multiple is exonerated a second time on new evidence: the prey the
corrected hunter chases realises 2.6 px/s, a quarter of its cap. **Named next
term, untouched:** `motor::turn_radius_px` — the same arithmetic N found
consuming the resting envelope now consumes the boosted one; it is not
apex-specific, so an arm on it must be read against the whole world.

**What Wrysk would be approving:** the one-line change of the predicate to
`in_contact()`. Visible whenever an apex is spawned from the viewer: it
charges (0.3 → 4.6 px/s during a burst), stalks half as much, handles twice
as much, eats about twice as often, ends the prey population about 2 % lower,
one apex reached 97 % of its minimum reproduction age, and a new
`GraspUnmapped` outcome appears in 5 of 969 attempts where a charging body's
grasp lands off the surface at the open rim. It does **not** make the apex
viable: all 32 still starve at 12.9 % of their bill and readiness overlap
stays zero.

## R — the skimmer at depth 0.55 in a reproducing world: refuted, a trade not an addition

Full note: [ecology-v1-depth-census-2026-09-16.md](ecology-v1-depth-census-2026-09-16.md).
Commits `6431156` (pre-registration before any row), `d08884e` (16
definition tests, 12 red against a stub), `080bee6` (the override, run loop,
driver, rule), `c38b5a6` (brood counter repair), `6b91316` (note). Search-only
(`census.rs`); no core file, no `WorldConfig` field. The override rewrites the
five roster skimmers' `depth` between `World::new` and the first step and
re-decodes their phenotypes; a control run at 0.10 reproduces the ordinary
harness world field for field at 2,000 ticks and, at full length, all 60
retained control rows by `final_state_hash` (A's screen, I's ladder, M's
present-off) and F's published variety census row for row.

| config | skimmer `depth` | kinds at end | grazer at horizon | glider | burrower | **skimmer** | seeds with any skimmer at the horizon |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 2.11 | 8.1 | 14.3 | 16.1 | 0.9 | 1 of 6 |
| baseline | **0.55** | 2.72 | **2.8** | 12.4 | 10.4 | **18.1** | **6 of 6** |
| `fast-leaf` | 0.10 | 3.00 | 21.3 | 27.9 | 12.7 | 0.0 | 0 of 6 |
| `fast-leaf` | **0.55** | 3.50 | **12.4** | 30.3 | 12.6 | **5.6** | **6 of 6** |

(18 runs per cell: 6 seeds × 3 apex arms; no world lost.) The skimmer's
ledger margin changes sign in both configurations; the grazer's falls in
both (to 0.35× and 0.58× its horizon population, margin rate to a fifth and a
forty-fifth); glider and burrower move 1–3 %. Water depth under a skimmer
falls 0.19 → 0.05 d and 0.25 → 0.08 d; the binary wet flag moves the wrong
way, as O warned. O's predicted diet drift appears: skimmer entrants in the
foliage bin 2 → 27 % and 0 → 14 %, and those bodies survive best in the world
(still an association; every one is a descendant).

**Verdict by Astra's rule: refuted in both configurations, on the variety
clause.** The lineage persists (baseline 16 of 18 worlds, `fast-leaf` 13 of
18 against 2 and 0 for the control) and no monoculture forms, but the grazer
pays: in baseline the loss is decisive on its face (0.35×) and thin
underneath (the baseline grazer reaches the horizon in only 2 of 6 control
seeds, so the loss is one world); in `fast-leaf` it is marginal on its face
(0.58× against a 0.60× line) and solid underneath (the grazer falls in 6 of 6
seeds and is gone from all three arms of one seed). R disclosed two defects in
its own pre-registration (a clause measuring a breeding founder's lifetime
where O's quantity was a sterile clone's; a per-seed agreement clause for a
lineage-founding rate that is concentrated in a few worlds) and reported both
as written. **The most consequential finding the brief did not anticipate:**
0 of 360 founder skimmers reach any horizon at either depth in either
configuration — the rescue is a lineage effect, not founder survival — and the
founder's own life moves in opposite directions (`fast-leaf` 609 → 1,352 s,
baseline 580 → 373 s), read as a 0.55 body paying 4.4× the motor bill for 3.5×
the range, which `fast-leaf`'s foliage funds and baseline's does not.

**What Wrysk would be approving, corrected:** not "skimmers that stop dying on
the rim" — they all still die. One genome value buys a fourth lineage that
persists, paid for out of the grazer: a trade of one kind for another, exactly
what the refutation clause was written to catch. And 0.55 is the grazer's own
`depth`, so "off the wet floor" and "onto the grazer's height" are the same
move here and nothing separates them. **R's recommendation, which Fable
accepts: do not take the roster change as tested.** Named next: a depth
ladder {0.10, 0.20, 0.30, 0.40, 0.55, 0.75} at arm 0, 60 runs, about 3.5
minutes — is there a depth that rescues the skimmer's lineage without taking
the grazer's horizon population? — plus splitting the ledger's margin bins by
generation and recording form-3 served material by channel.

## S — a plant-only preconditioned opening

*(pending)*

## What this does and does not establish

*(after P, R, S)*

## Next recommendation

*(after P, R, S, reconciled with Astra)*
