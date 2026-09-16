---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 round 5, part 1 — independent review

**Latest status — after repair 1: not cleared; one P1 record repair and one P3 test repair remain.** P1-1 is cleared: `World::set_action_adapter` validates every attached policy before assignment, names both adapters on refusal, leaves the transient and snapshot state unchanged, and the sole search caller propagates the error. P1-2 is cleared: X and the consolidated note now distinguish the registered `MIXED` label from the substantive refutation of frozen-policy expression at this operating point, and treat the training arm and generation-wise p-values with the right exploratory/descriptive scope. P1-3 is repaired in the correction block, headline, channel discussion, approval section and consolidated note, but Y's body still asserts the withdrawn claims that founder and descendant margin rates are monotone and that the result “corrects R” (`ecology-v1-depth-ladder-2026-09-16.md:555-569`); its closing also still says “no height” and makes a wet-floor producer the remaining lever rather than one candidate (`:711-714`). Remove or rewrite those contradictory passages. Both P2 follow-up conditions are now recorded and were sent to XY2; their satisfaction remains for that campaign's result, not this repair. The production strict-majority formula is correct and changes no retained cell, but the new unit test only tests a second local copy of `of / 2 + 1`, not `agreement` itself (`crates/cubarium-search/src/census.rs:1281-1309`); P3 remains until a two-run tie is rejected through the production function. No default, roster or cube change is supported yet.

Disposition: retain both campaigns and their rows. Accept X's replay arithmetic, provenance, and resume refusal; accept Y's no-acceptable-rung verdict and arm split. Do not accept `cub-act-2` as a training default, “learnability” as identified, a wet-floor producer as forced, or Y's monotone channel/margin mechanism as written. Z remains out of scope for this part-1 review.

## Findings

### 1. P1 — the resume refusal is sound, but a live adapter change still bypasses the same contract

Fable's integration repair is correct at the persistence boundary. `World::from_state` validates every interned policy against the shipped `cub-act-1` before assembling the resumed world, and the test proves both the by-name refusal of a `cub-act-2` snapshot and an ordinary `cub-act-1` round trip (`crates/cubarium-core/src/world/lifecycle.rs:88-138`; `crates/cubarium-core/tests/action_adapter.rs:279-324`). That is the right consequence of keeping the adapter transient: bytes that cannot name the adapter must not silently reinterpret its policy.

The same mismatch remains legal in a live world. X widened `Policy::validate()` to accept a digest for either known adapter; only `validate_in()` checks the adapter actually in force (`crates/cubarium-core/src/neural/gru.rs:175-221`). `attach_neural_policy` and founding call that stronger check, but `World::set_action_adapter` is an infallible assignment. Its documentation explicitly says changing it after attachment is legal (`crates/cubarium-core/src/world/mod.rs:168-180`; attachment check at `world/view.rs:398-433`). An existing `cub-act-1` policy can therefore be attached, the world flipped to `cub-act-2`, and the same weights decoded under the other adapter with no refusal or invariant failure—the exact class of silent reinterpretation the resume repair prevents.

Make the setter fallible and atomic: before changing the transient, validate every interned policy against the requested adapter; on any mismatch, name both adapters and leave the world unchanged. The narrower alternative is to refuse any adapter change once a neural policy is interned. Pin both directions in a test. Search fixtures already set the adapter before founding, so this should not change X's rows.

### 2. P1 — X applied its registered rule honestly, but the rule's “mixed” branch is directionally wrong

The chronology and arithmetic check. The decision rule was committed at `3ad0a88` before the replay; the report carries the same rule; turn activity is 132/132 positive, on-food is 72/60 (`p = 0.3384`), `t_min` is 18/15 (`p = 0.7283`), and dwell is 53/78 with one tie (`p = 0.0356`, median −3.17 ticks) (`ecology-v1-turn-deadband-2026-09-16.md:26-49,147-173`). Calling that **mixed** is an honest application of the literal rule.

It is not an honest substantive reading of the bottleneck hypothesis. The rule defines falsification as `p > 0.1` on all three residence outcomes, so a significant change in the *wrong direction* prevents falsification and authorizes training. But the prediction under test was that releasing turn expression would improve on-food residence or worst-layout survival. It did neither and significantly shortened dwell. A detrimental residence result is evidence against that expression hypothesis, not evidence between support and falsification. The report should preserve “MIXED by the pre-registered rule” as history, then state the scientific result separately: **the frozen-policy expression hypothesis is refuted at this operating point; dwell worsened**. The bounded training run remains a permitted exploratory follow-up under the registered rule, not confirmation that the bottleneck moved to learnability.

X otherwise scopes the interpretation well. It explicitly calls learnability a hypothesis untested by the campaign and notes that every replay and held-out episode still dies (`ecology-v1-turn-deadband-2026-09-16.md:418-441`). The consolidated note should keep that modality. “The search can exploit the released channel” is plausible; “learnability, not expression” is not yet identified.

### 3. P1 — Y's no-rung verdict survives, but its monotonicity and common-mechanism headline do not

The registered acceptance result is sound. At arm 0, fast-leaf lineage establishment is 2/6, 3/6, 4/6, 3/6 and 2/6 over the five treatment rungs, below the binding 5/6 rule everywhere; baseline's only 6/6 rung, 0.55, reduces the grazer to 0.40× (`ecology-v1-depth-ladder-2026-09-16.md:396-445`). Therefore no tested arm-0 rung is acceptable. This is assignment evidence, not an ecosystem-health result, and Y correctly proposes no roster change.

The claimed monotone mechanism is arithmetically false:

- Fast-leaf descendant litter first **rises** `2.60 → 2.75` before falling, while descendant foliage peaks at `28.69` at 0.55 and falls to `26.34` at 0.75. Baseline descendant foliage goes `8.68 → 11.99 → 6.55 → 16.91 → 20.03 → 26.53`. These are strong overall shifts toward foliage, not monotone swaps at every rung (`ecology-v1-depth-ladder-2026-09-16.md:471-512`; independently reproduced by `runs/ecology-v1-depth-ladder/analyse.py`).
- Founder margin rate is not monotonically falling in baseline: `−0.003056` at 0.55 becomes **less negative**, `−0.002872`, at 0.75. Descendant margin is not monotone in either configuration: fast-leaf dips `+0.001015 → +0.000970`, and baseline falls `+0.000760 → −0.000296` before rising (`ecology-v1-depth-ladder-2026-09-16.md:514-550`). The stated correction to R is therefore unsupported. R had already disclosed that its pooled ledger could not separate founders and descendants and labelled its founder mechanism a reading, so Y refines that record rather than correcting an identified sign claim (`ecology-v1-depth-census-2026-09-16.md:452-500`).

The served-channel data establish increasing dietary overlap with the grazer's foliage and declining litter use overall. They do not establish that the skimmer removed material the grazer would otherwise have received, or that “the rescue and the cost are the same mechanism.” Field production, population composition and feedbacks all change with depth; at fast-leaf arm 0 the grazer horizon population is 0.90–1.24× control at every rung except the one seed lost at 0.55 (`ecology-v1-depth-ladder-2026-09-16.md:452-504`). Replace “takes the grazer's leaf,” “one move,” and “no height separates them” with the measured claim: **lineage success is associated with a shift from litter toward the same foliage channel used by the grazer; direct displacement was not isolated**. This repair leaves the no-rung verdict intact.

### 4. P2 — X's training arm is encouraging, but generations are not independent replicates; XY2's X rule should say so

The training table checks: mean population score and intake/tick favour `cub-act-2` in 15/16 generations, opening residence in 14/16, while centre and best-candidate columns do not separate; selected held-out minimum rises 6,914 → 7,870, intake/tick rises about 20%, residence is flat 0.0324 → 0.0327, and neither policy reaches a horizon (`ecology-v1-turn-deadband-2026-09-16.md:201-275`). X itself correctly warns that sixteen sequential generations are one autocorrelated trajectory, not sixteen experiments (`ecology-v1-turn-deadband-2026-09-16.md:423-428`). The exact sign-test arithmetic is fine; its p-values are descriptive, not an inferential replication count.

XY2 currently calls the effect “replicated” when the same paired-by-generation tests cross `p < 0.05` at one second seed and its selected held-out minimum is higher (`design/handoffs/ecology-v1-round5-followups-opus-2026-09-16.md:41-67`). Do not change a rule after its rows have begun. Keep that registered label if already committed, but qualify it as **the same directional pattern in a second pre-chosen training run**, not statistical replication over sixteen generations. In the two-seed synthesis, the seed/run is the replicate (`n = 2`), and no p-value over seeds is available.

The single most informative cheap addition is the diagnostic X already names: replay each seed's selected `cub-act-1` and `cub-act-2` centres under **both** adapters on the same layouts, with the intake/dwell trace. Report the 2 × 2 of weights × adapter for `t_min`, on-food fraction, dwell and intake/tick. If swapping only the adapter explains the gain, it is expression; if the `cub-act-2`-trained weights retain an advantage across adapters, search reached a different region; an interaction is co-adaptation. This will not prove optimiser adequacy, but it directly tests the “learnability rather than expression” story more sharply than a second pair alone.

X's split-half result should remain descriptive. A mean cosine of 0.0158 is only about 1.6 null standard deviations, the 6,435 complementary splits reuse the same sixteen vectors and full-population ranks, and X states both limitations (`ecology-v1-turn-deadband-2026-09-16.md:277-324`). It supports “unstable-looking at this generation,” not a conclusion about pair-count sufficiency.

### 5. P2 — Y's arm split is real; “the apex is load-bearing” is a treatment label, not a mechanism

The re-reduction of R's rows checks. At fast-leaf depth 0.55, arm 0 has grazer 15.2 versus its arm-0 control 16.8 (0.90×) and lineage in 3/6 seeds; arms 1 and 2 have grazers 10.8/23.0 (0.47×) and 11.2/24.2 (0.46×), with lineages in 5/6 each (`ecology-v1-depth-ladder-2026-09-16.md:552-582`). R's pooled statement that nothing turned on the apex was too strong. “Largely an apex-arm property” is supported for fast-leaf; baseline is comparatively uniform.

What is not identified is why. Arm number changes predator presence and count, predation, recycling, prey abundance and subsequent resource feedbacks. The retained census records do not isolate direct predation on skimmers, predation on grazers, or an indirect plant-mediated path. Call this the **apex-arm treatment**, not an apex mechanism or proof that the apex is biologically “load-bearing.”

XY2's Y rule is otherwise the right current-world test: reproduce R's 0.10/0.55 arm-2 rows under the historical half-space, then run every arm-2 ladder cell—including its own 0.10 control—under the shipped reach envelope, with unchanged `L ∧ ¬M ∧ ¬V` and 5/6 threshold (`design/handoffs/ecology-v1-round5-followups-opus-2026-09-16.md:69-91`). Report half-space versus reach-envelope differences at the two reproduction depths rather than silently treating R and XY2 as one predicate. If still cheap before rows close, add predation deaths cross-tabulated by prey form (or explicitly state that the mechanism remains unresolved); global prey deaths and separate deaths-by-form cannot say which form the apex removed.

### 6. P3 — Y's agreement rule is correct for these cell sizes but is not a general majority

Pre-registering the one-arm adaptation was honest and necessary. One run per seed makes seed agreement identical to the run, so 5/6 is the binding threshold; at R's three arms the function reproduces 2/3 (`ecology-v1-depth-ladder-2026-09-16.md:217-234`). Those are both odd cell sizes and Y/XY2 are unaffected.

The implementation and prose call `ceil(runs_of_seed / 2)` a “majority” (`crates/cubarium-search/src/census.rs:1270-1291`). For two runs it accepts one, which is a tie, not a majority. Either rename the rule “at least half” and justify that policy, or implement strict majority as `runs_of_seed / 2 + 1`; add the two-run boundary test. This is a latent generalisation defect, not a defect in the retained six-seed results.

## XY2 direction while it is in flight

1. Keep both registered rules and do not tune thresholds after observing rows.
2. For X, treat paired-generation p-values as descriptive and the second training seed as the independent repeat. Add the selected-centre weights × adapter crossed replay if it has not already become outcome-contingent.
3. For Y, keep the arm-2 reach-envelope ladder and historical half-space reproduction. Name the treatment precisely, show the predicate contrast at 0.10 and 0.55, retain raw non-monotone channel/margin sequences, and add per-form predation deaths if available without rerunning.
4. Neither follow-up should propose a default. A second favourable X seed still lacks whole-world evaluation because `es-population` deliberately refuses `cub-act-2` (`ecology-v1-turn-deadband-2026-09-16.md:128-139,326-348`). An acceptable Y rung would be a candidate for held-out confirmation, not a roster decision from six training seeds.

## What I did not check

I did not rerun X training, X's 264-episode replay, Y's 72 simulations, or any crate suite; Fable's field-for-field reproductions and five-suite totals are reported rather than independently reproduced. I did run Y's retained `analyse.py` against `runs/ecology-v1-depth-ladder/runs.jsonl`, inspect the retained JSON schemas, re-check the printed sign-test arithmetic, inspect the relevant git chronology and diffs, and trace the adapter, snapshot, attachment, census and agreement seams in source. I did not inspect Z, touch `state/`, port 7393, the shim, or the running cube. The pre-existing dirty worktree was left untouched. This review creates only this file and makes no commit.
