---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 round 2 — independent review

**Latest status — after repair 1: both original P1 findings are cleared; two P2 wording residuals and one P3 test residual remain.** `Arm::ASwap` is the exact implementation requested—same planned cells and skimmer body, with 0.85 in slots 0/1/4/5 and 0.60 in 2/3/6/7, opt-in and absent from `Arm::ALL` (`crates/cubarium-search/src/factorial.rs:147-195,480-505`). I reproduced the retained campaign in scratch: all eight A/As final-state hashes matched, the original A matched its earlier rows on 4/4 seeds, and direct reduction of the retained rows gives 29/32 within-slot lifetime wins and 22/32 versus 2/32 establishments, with low-diet median life higher in all four worlds. J now correctly withdraws the clone-level Fisher inference and calls the world the replicate; arm C is correctly a diet-locus yield difference, with the body leg unmeasured (`ecology-v1-diet-factorial-2026-09-16.md:405-442,468-501`). I and H are correctly scoped in their repaired findings, and the reconciled five-step recommendation accurately states my opinion (`ecology-v1-round2-results-2026-09-16.md:276-294,323-376`). The residual P2s are in stronger surrounding prose: the consolidated disposition still says the controller “ignores” the scalar and that the score, not observation use, is next, and still calls the depletion events “almost never grazed cells” before immediately conceding that only one-second occupancy and attributed bites were measured (`ecology-v1-round2-results-2026-09-16.md:21-40`); J and the consolidated result still present F's 84% as proven survivorship and a surviving lineage as necessarily foliage-diet, although the counterbalanced cold-founding experiment makes that explanation plausible rather than identifying the causal process in F's reproductive descendant census (`ecology-v1-diet-factorial-2026-09-16.md:444-464`; `ecology-v1-round2-results-2026-09-16.md:246-256`). P3: the new unit test passes and checks parsing/default exclusion, but it recomputes two Boolean slot formulas rather than comparing `plan(Arm::A)` with `plan(Arm::ASwap)`, so it does not itself pin the claimed same-cell/same-body/complement output against a future plan regression (`crates/cubarium-search/src/factorial.rs:1181-1192`). The current code and retained run are sound; this is only a regression-test gap. Earlier findings remain below as history.

Disposition: retain H's intake trace, I's ladder result, and K's apex ledger as useful evidence, but do not accept the package's full causal reading yet. H establishes that generation 9 fails by leaving food, not that the training score is the unique cause. I correctly refutes the preregistered price hypothesis, but its sampled visits and derived static \(L\mu\) threshold do not establish that nearly every depletion is an ungrazed local plant-budget failure. Most importantly, J did not counterbalance diet across founding cells, so its 13/16 versus 1/16 result is confounded by position and its Fisher test uses clones sharing four worlds as independent observations. Arm C measures the yield difference between two roster diet loci, not a body effect. I would let Wrysk authorize a tightly bounded score experiment after two cheap falsification checks; I would not yet change the foliage-seeding contract or canonize a critical \(L\mu\).

## Findings

### P1 — J's arm A is not a matched causal comparison of diet

The descriptive counts and the reported Fisher calculation check: the retained arm-A rows contain 13/16 established animals at diet 0.60 and 1/16 at diet 0.85, and a 2×2 Fisher test that treats the 32 clone lives as independent produces the reported order of magnitude, \(p\approx4\times10^{-5}\) (`ecology-v1-diet-factorial-2026-09-16.md:267-305`). The experimental assignment does not support that inference:

- The code always assigns diet 0.60 to slots 0, 1, 4, and 5, and diet 0.85 to slots 2, 3, 6, and 7 (`crates/cubarium-search/src/factorial.rs:467-499`). Diet is therefore confounded with founding cell; no cell is observed under both diets.
- The confound points in the observed direction. In the retained arm-A rows, the mean starting foliage for low-diet versus high-diet slots is 0.188/0.113, 0.170/0.116, 0.188/0.161, and 0.172/0.106 for seeds 1001–1004. Mean starting litter is likewise higher for the low-diet slots in every seed: 0.091/0.056, 0.092/0.071, 0.080/0.070, and 0.050/0.043 (`runs/ecology-v1-diet-factorial/runs.jsonl`, arm-A rows grouped by seed and slot).
- Four clones of each treatment share each seeded world, so 16 clone lives are not 16 independent experimental replicates. The tiny Fisher value is descriptive conditional on that independence assumption, not a valid experiment-level probability. At the independent seed level, all four directions agree, which is suggestive, but four paired worlds are not the claimed strength of evidence.

The scripted seam does successfully suppress clone reproduction (`factorial.rs:659-666`), and the same tick and same set of cells are used. Those controls do not cancel fixed treatment-by-cell assignment. The sound conclusion is “the low-diet slots survived better in all four worlds,” not “moving only diet reversed the result.” F's 84% foliage-diet association may be consistent with descendant survivorship near a parent's viable patch, but J has not proved that explanation.

The consolidated result inherits this overstatement when it calls the cells matched and says moving only diet caused the reversal (`ecology-v1-round2-results-2026-09-16.md:37-43,267-270`).

### P1 — J arm C measures a diet-locus yield difference, not a body cost

The arithmetic is sound: digestible/served is about 0.413 for the skimmer-roster treatment and 0.900 for the burrower-roster treatment (`ecology-v1-diet-factorial-2026-09-16.md:351-381`). But arm C changes both form and diet. The two fractions follow directly from the treatments' diet breadth/capacity, so they establish a real yield cost for those two roster pairings; they do not isolate a “body leg” or a body-level penalty. The later statement that the body leg has been measured is stronger than the design (`ecology-v1-diet-factorial-2026-09-16.md:400-426`).

J was right to decline arm B as a finding: 0/8 versus 2/8 is weak and the stated Fisher result is uninformative. “Body and habitat are one statement because of depth” should likewise be softened to “not separable here.” Depth is a plausible mechanism, but roster form also bundles size, speed, swimming, and metabolic properties. A depth-only factorial is still needed.

### P2 — I refutes the registered price hypothesis, but overstates the depleted-cell mechanism

The preregistration is timestamped before the implementation, its gates were applied as written, and `REFUTED` is correct. No non-control price simultaneously gives founder-grazer broods in at least 4/6 seeds and range ratio below 0.60: baseline 0.0006 narrowly misses range at 0.604 and has no brood, while fast-leaf 0.0006 broods in 6/6 but has range 1.027; fast-leaf 0.0018 has range 0.390 but no brood (`ecology-v1-ladder-2026-09-16.md:242-264,424-439`). The nutrient fall from roughly 790 to 530 is also fairly labelled a correlate rather than a cause (`ecology-v1-ladder-2026-09-16.md:633-647`).

The retained classifications reproduce as 1,355 depleted cells: 971 with no sampled visit, six with attributed post-depletion bites, one pressure-limited classification, nine recovered, and 1,345 called marginal. Three qualifications matter:

1. “Never visited” means never occupied at a one-second probe. The tracker samples positions every 20 ticks (`ecology-v1-ladder-2026-09-16.md:64-67`; `crates/cubarium-search/src/depletion.rs:290-328`), so an animal can cross or feed between probes. The report's claim that a visit cannot be missed is not supported (`ecology-v1-ladder-2026-09-16.md:494-506`).
2. Bites are attributed to the current or last sampled cell rather than recorded at the exact withdrawal site (`crates/cubarium-search/src/depletion.rs:495-521`). This is useful screening telemetry, not an exact per-cell consumption ledger.
3. The derived \((L\mu)_\mathrm{crit}\) is a diagnostic proxy, not a constant in the contract. It uses static base light and a reference nutrient value and omits the actual time-varying light, nutrient, water stress, reserve flow, and \(Q\). At the depletion threshold, the contract's emergency reserve reflush can be active, so \(L\mu\) alone does not determine short-run recovery (§4.4 and §11; `design/ecology-v1-contract.md:177-205,524-533`; `ecology-v1-ladder-2026-09-16.md:92-130,674-682`).

Thus “most counter crossings were not accompanied by observed grazing at the probe resolution” follows. “A depletion event is almost never a grazed-out cell” and “the mechanism is uniform over-seeding into cells that cannot hold it” remain leading hypotheses. A matched herbivore-absent run with actual per-cell \(N\), effective light, \(P\), \(Q\), plant income/loss, and exact withdrawal would confirm the mechanism if the same cells still cross while their plant budget is negative. It would refute it if the crossings disappear or their measured plant budget is positive.

The consolidated result therefore goes too far in calling the counted depletions established habitat-limited declines and prescribing local break-even seeding from this evidence (`ecology-v1-round2-results-2026-09-16.md:262-276,294-300`).

### P2 — H diagnoses the phenotype cleanly, not the cause in selection

The three-way accounting is sound for the branches measured. Across the 12 generation-9 layouts, the mouth is open on every on-food tick (8,365/8,365), and requested and served material agree to the reported tolerance. The large difference is occupancy: median on-food fraction is about 8.3% for generation 9 and 95.2% for the mobile script (`ecology-v1-intake-2026-09-16.md:143-182`; `runs/ecology-v1-intake/intake.json`).

The dwell rows support “failure to remain on food”: generation 9 visits most food cells but has median dwell of 93 ticks versus 2,299 for the script, and every recorded departure occurs above the feeding threshold (`ecology-v1-intake-2026-09-16.md:243-275`). They do not by themselves prove “not detection.” The observation contains both local foliage and ring food signals (`crates/cubarium-core/src/neural/obs.rs:60-96,130-180`), so the signal is available; the work did not test whether the frozen weights, recurrent state, update cadence, or nearly constant one-third mouth allocation actually use it. H acknowledges those exclusions (`ecology-v1-intake-2026-09-16.md:294-304`).

A dense ledger term is a reasonable hypothesis, but not yet the uniquely implied next move. Before a campaign, two cheap checks should try to falsify it: sweep the frozen controller over otherwise identical observations with local/ring food varied, both with reset and carried hidden state; and score scripted policies that differ only in food dwell. If the current score already changes strongly and monotonically with dwell, the missing-gradient diagnosis is wrong. If the policy's movement head already responds strongly to food while residence still fails, cadence or recurrent dynamics deserve priority.

Accordingly, the consolidated statement that detection has been excluded and the controller ignores food “because” of its score is not established (`ecology-v1-round2-results-2026-09-16.md:262-264,286-293`).

### P3 — K's death accounting is complete at the budget level; “reach next” is properly scoped

K accounts for all 16 deaths as starvation and closes the energy story: food is about 6% of the billed energy, strikes about 33%, travel about 1.7%, upkeep about 65%; only 15 of 449 paid strikes capture, while 402 (89.5%) fail `OutOfReach` (`ecology-v1-apex-eligibility-2026-09-16.md:129-226`). That is a complete proximate budget diagnosis, not yet a complete kinematic explanation of why pursuit fails—which K correctly leaves open.

Introducing at tick 24,000 is a valid way to make 1,200 seconds of represented age possible without changing ordinary introduction semantics, and the age-zero control at the same tick is the right comparison (`ecology-v1-apex-eligibility-2026-09-16.md:72-111`). The reserve term is first in the logged refusal ordering on about 80% of member-ticks and every life peaks at reserve fraction 0.5; that supports “reserve is the binding readiness term under this ordering,” not a claim that age or radius would otherwise pass on every tick (`ecology-v1-apex-eligibility-2026-09-16.md:245-278`).

K's correction to E is fair as a scope correction: at tick 24,000 there are 1,180 mutually sensed candidate pairs in both aged and control worlds, so “never sensed a candidate” cannot be generalized from E's earlier tick-6,000 audit. It does not invalidate E's actual early-window observation. Measuring strike separation, closing speed, target motion, and chase phase is the right next apex question; neither mating radius nor readiness thresholds should be changed first.

### P3 — The new doors are narrow and the reviewed integration seams are consistent

`World::found_animal_with_genome` validates the supplied genome through the normal clamp/validation path, refuses invalid placement/capacity, and shares the private `found_body` construction with the existing training door (`crates/cubarium-core/src/world/lifecycle.rs:295-387`; `crates/cubarium-core/src/genome.rs:246-290`). `introduce_hunters_with_age` changes represented birth time while the ordinary introduction delegates with age zero. The H trace and I ledger are separately opt-in consumers of `BudgetRecorder`; the command paths do not silently enable either in an ordinary world. I found no hash/protocol mismatch from I's `--ledger`, and no viewer seam caused by the audit-only founder-age option.

I ran the focused release tests for the three core doors and the three search measurement surfaces: 76 passed and two diagnostic entry points remained intentionally ignored. This supports inertness and API integration, though it is not a substitute for the reported full core/search/host suites.

## Next steps: Astra's opinion

### 1. Repair J before treating diet or form as causal

The single most informative cheap experiment now is an arm-A swap: on the same four seeds and eight placements, put diet 0.85 in slots 0, 1, 4, and 5 and diet 0.60 in slots 2, 3, 6, and 7. Analyze within-slot paired outcomes across the original and swapped assignments, with seed/world as the independent replicate. This should take only one arm of J. The diet interpretation is confirmed if the 0.60 advantage follows diet within the same slots in all or nearly all worlds; it is refuted if the advantage stays with the originally richer slots or materially collapses. Do this before citing 13/16 versus 1/16 as a diet effect or designing around F's 84% association.

### 2. Approve a score experiment, not a training campaign yet

I agree that survival-only \(t_{\min}\) is an impoverished learning signal. I would propose a fixed-horizon auxiliary based on usable energy margin, not served material:

\[
A=\frac{1}{T}\sum_{t=1}^{T}\operatorname{clip}\!\left(\frac{E_{\mathrm{credited},t}-E_{\mathrm{billed},t}}{b_{\mathrm{ref}}},-1,1\right),
\qquad S=t_{\min}+\lambda A.
\]

After death, score the remaining ticks as \(-1\), rather than truncating the mean; otherwise dying early can avoid future bills. Fix \(T\), \(b_\mathrm{ref}\), clipping, and \(\lambda\) before looking at outcomes, with \(\lambda\) small enough that the auxiliary cannot erase a material survival difference. Credited usable energy is preferable to served mass because the latter rewards low-yield intake and depletion without paying the body's energetic cost.

Wrysk should approve that as a bounded scoring proposal only after the two H checks above. It is falsified as the next intervention if the present score already gives a strong dwell gradient, or if the proposed score ranks short rich bursts, stationary starvation, or early death above sustained feasible residence. A short dwell-script ladder should demonstrate monotonicity before any ES campaign.

### 3. Measure the plant budget before deciding what “depletion” or opening foliage means

I would not put a universal critical \(L\mu\) into the contract. The useful break-even quantity is state-dependent: effective light, actual \(N\), water stress, senescence, reserve \(Q\), reflush, and consumer withdrawal all contribute. A named scalar computed at reference \(N=0.4\) can remain a documented analysis proxy, but it should not define ecological depletion.

First run the 12 fast-leaf layouts without herbivores and record, per cell and tick or sufficiently fine interval:

- actual effective light and \(N\);
- \(P\) and \(Q\), including reserve transfer;
- gross plant production, senescence/loss, and exact consumer withdrawal (zero in this arm);
- the first threshold crossing and subsequent recovery.

Then repeat or compare with herbivores using exact per-cell withdrawal. Split the counter into “crossed with any exact consumer withdrawal since the prior recovery” and “crossed without withdrawal”; do not call a one-second occupancy sample “ever visited.” If plant-only cells cross at the same places/times with a measured negative net plant budget, the seeding hypothesis is confirmed. If they remain above threshold, look first at missed consumption or animal-mediated nutrient/light effects.

Only after that result would I choose between a plant-only warm-up and seeding each cell below its measured local equilibrium. The latter has a visible consequence: dim or nutrient-poor cells begin less lush and more heterogeneous, rather than greening uniformly and fading. That is an ecological and presentation decision, not a free correction. My advice to Wrysk is: approve the measurement and exact counter split now; defer the §11 seeding change.

### 4. Instrument apex reach before touching eligibility or mating

For every paid strike, record separation at intent, separation at resolution, predator and prey displacement/heading, target identity continuity, and whether the target crossed a cell or movement boundary. Compare captures with `OutOfReach` failures. If failures begin in range but resolve out of range, cadence/resolution is implicated; if they begin out of range, target selection or pursuit policy is implicated. Keep age, reserve, and mating radius unchanged until hunters can fund themselves.

### 5. Run the remaining factorials in causal order

After the arm-A swap, run the depth-only form factorial with diet, placement, habitat, and metabolic parameters held fixed. Fold I's nutrient probe into the herbivore-absent plant-budget experiment rather than treating endpoint \(N\) as a separate causal study. Finally test whether a detrital diet can fund an otherwise fixed body in fast-leaf using the ledger's served → digestible → credited → billed chain. That calibration question is real, but it should not inherit arm C's body-effect wording.

In short, I agree with Fable that score design, plant seeding/depletion, and apex reach are the important fronts. I would add the J counterbalance repair at the top, turn the proposed foliage change into a measurement-gated owner decision, and postpone the broader depth and detrital work until the two causal confounds are removed.

## What I did not check

I did not rerun H, I, J, or K; inspect every per-tick CSV; replay all 60 ladder rows individually; audit every line in the full merge range; run the full workspace suites; inspect the live cube; or touch `state/`, port 7393, the shim, or the running process. I checked the retained JSON/JSONL aggregates and selected rows, the named report tables and briefs, the relevant contract equations, the targeted implementation seams, git history/statistics, and focused release tests. Lore's local daemon was unavailable, so repository retrieval used Graft followed by exact source reads.
