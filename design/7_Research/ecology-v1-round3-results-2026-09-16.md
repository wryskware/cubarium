---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 3: results of workstreams L, M, N and O (2026-09-16)

Fable's consolidated result for the reconciled next steps of
[the round-2 result](ecology-v1-round2-results-2026-09-16.md) ("Next
recommendation (reconciled with Astra)"), after Astra's
[round-2 review](ecology-v1-round2-review-2026-09-16.md) cleared round 2.
Briefs: [L](../handoffs/ecology-v1-score-checks-opus-2026-09-16.md),
[M](../handoffs/ecology-v1-plant-budget-opus-2026-09-16.md),
[N](../handoffs/ecology-v1-apex-reach-opus-2026-09-16.md),
[O](../handoffs/ecology-v1-depth-factorial-opus-2026-09-16.md), committed at
`1525604`. Four Opus workers in parallel: L on `main`, M, N and O in worktrees.
Wrysk's standing instruction: keep going, tune nothing, backlog operator
controls and artwork ([backlog](../backlog.md)); the one decision reserved for
him this round is whether the bounded score experiment runs if L's checks leave
the hypothesis standing.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| O depth × diet factorial | Opus 5 medium | worktree, merged `10d3e2a` | 35 s / 3 min | 180 KiB / 10 MiB | search 199 |
| L score falsification checks | Opus 5 high | `main` | *(pending)* | | |
| M plant budget | Opus 5 high | worktree | *(pending)* | | |
| N apex strike reach | Opus 5 high | worktree | *(pending)* | | |

Fable's verification so far: O's whole design re-run and matched by
`final_state_hash` on 16 of 16 rows; its search suite 188 on the branch, 199
after merge.

## O — the depth × diet factorial

Full note: [ecology-v1-depth-factorial-2026-09-16.md](ecology-v1-depth-factorial-2026-09-16.md).
Commits `94d148b` (harness), `3c384be` (note); merged `10d3e2a`. Search-only;
no core change. Tests first, 29 in the factorial suite (+12) and two new unit
tests, run red before the arms existed.

**Design decision, disclosed.** The brief asked for "two counterbalanced arms"
and a "4 × 8 Latin-style assignment" in one sentence; two arms cannot give
every cell every one of four treatments, and a slot-by-slot complement on both
factors would have put the depth contrast *between* cells — the confound Astra
caught in J. O used four rows (`Arm::D1..D4`), treatment `(slot mod 4) XOR row`
(bit 0 diet, bit 1 depth): every cell holds every treatment once, every row
holds each treatment twice on two faces, and each pair of rows is a
one-factor exchange in every slot. 16 runs, 35 s. `depth` decodes only to the
preferred height in the steering term (no capacity, rate or bill; asserted by
test); 0.55 is the roster grazer's value (`h_pref` +0.1, just above the
equator) against the skimmer's roster 0.10 (`h_pref` −0.8, the wet rim).

| treatment (depth / diet) | reached horizon | established | median life | served foliage / litter | median net margin | algae-band probes | distinct cells |
| --- | --- | --- | --- | --- | --- | --- | --- |
| T0 0.10 / 0.60 (roster skimmer) | **0 / 32** | 27 | 863 s | 0.81 / **4.66** m | **−0.0020** e/s | 34 % | 122 |
| T1 0.10 / 0.85 | 6 | 7 | 386 s | 4.29 / 0 | −0.0053 | 9 % | 79 |
| T2 0.55 / 0.60 | **26** | 30 | 4,500 s (horizon) | **22.5** / 1.95 | **+0.0006** | 7 % | 426 |
| T3 0.55 / 0.85 | **27** | 27 | 4,500 s | 16.4 / 0 | **+0.0008** | 5 % | 406 |

**Verdict: depth couples the body to litter — supported, more strongly than
anything J measured.** Within-cell, depth 0.55 beats 0.10 on net margin in
30 of 32 pairs at the founder diet and 29 of 32 at the foliage diet, in 4 of 4
worlds, and the median margin changes sign in every world (per-seed medians
never overlap). The roster skimmer starves 32 of 32 and never reaches the
horizon; moved off the rim it reaches the horizon 26 of 32 while eating thirty
times the foliage over three and a half times the cells. Depth barely moves
the binary wet-probe fraction (67 → 65 %) but moves the water depth under the
body (0.31 → 0.05 d) and the algae-band fraction (34 → 7 %); future notes
should report water depth, not the wet flag. **Interaction, a sign reversal:**
on uncensored net margin the founder's 0.60 wins 25 of 32 pairs at depth 0.10
and loses 26 of 32 at depth 0.55, in 4 of 4 worlds. That reconciles J with F:
F's association (foliage-diet skimmers do better) is real, and real only for a
body not parked in a pool; J saw the opposite sign because every clone it
compared had `depth` 0.10. The T0-versus-T1 contrast is J's counterbalanced
pair on the same cells and seeds and agrees qualitatively (low diet wins 25 of
32 pairs against J's 29; established 27 against 7; 4 of 4 worlds), with hashes
necessarily different because each run now holds all four treatments.

Corrections O made against its own rows before publishing: the six depth ties
at the foliage diet are cells where both clones reached the horizon (T1's
lottery winners), not double starvations; fruit and carrion are near zero,
not zero; the Top-face slot did not work as a null (n = 4, described only).

**Named, not launched:** re-run F's 150-minute reproduction census with the
roster skimmer at `depth` 0.55 and nothing else changed — the test of whether
a founding-time effect this large survives a lineage, and the only way to
decide whether `depth` 0.10 is a bug in the roster or a bug in the world. The
wet floor has no producer (`water.algae_light` raises a *living* cell's light
floor; foliage grows only where wood already is), so the skimmer's designed
larder does not exist; that is an ecology decision and Fable's. And the
detrital-funding question is now close to answered in the negative: across
128 lives litter funded nothing (T0 served 4.66 m, credited 1.15 m against a
4.67 e bill, starved 32 of 32) and every positive margin came from foliage.
Until a calibration shows some setting at which litter pays, ecology v1 has
one working guild, not four.

## L — the score falsification checks

*(pending)*

## M — the plant budget of a depleted cell

*(pending)*

## N — why an apex strike ends out of reach

*(pending)*

## What this does and does not establish

*(after L, M, N)*

## Next recommendation

*(after L, M, N, reconciled with Astra)*
