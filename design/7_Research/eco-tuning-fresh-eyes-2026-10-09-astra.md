Fresh-eyes proposal — ecology tuning, 2026-10-09

Replace the first four blind CMA generations with a budget-informed sensitivity experiment and staged candidate selection. First check whether the nine genes can reach the requested outcomes. Spend long runs on independent validation and unresolved slow mechanisms. This is a proposed revision of protocol A, not a change to accepted policy.

Review scope: repository and existing outputs read only; no simulations, builds, process changes, cloud calls or repository edits. Numerical observations below use completed seeds s1, s4, s7 and s8; the other four were unfinished when inspected. No other review was consulted.

1. What limits effectiveness per compute

The principal problem is incomplete observability and possibly insufficient controls, before optimizer choice.

• The score does not express the bellwing target. `fitness-v3.toml:51–58` records nominal 90 per d72 but penalizes only above 900: the wide ceiling is 3,000, not about 300. Seasonal swings should remain permitted, but an agreed seasonal abundance envelope must distinguish sustained overshoot. Also, `score.py:762–791` calls an arm feasible when it introduces no losses beyond the baseline. Final acceptance needs absolute persistence of every required lineage, not merely this relative flag.

• Survival is concealing lifecycle failure. In all four completed seeds, lanternjaws have zero adults and zero eggs laid at 10 h. Their surviving bodies hatched from the starting supply. Example: s7 `err.log:1896` reports 14 hatchlings, 10 detached, zero adults and zero eggs laid. The year scorer's J uses surviving recruits, including hatchlings, rather than completed replacement (`score.py:265–306,460–471`). Therefore even a year check can give reassuring reproduction credit without a self-renewing lineage. Diagnose host-to-independent-to-adult transitions with censored ages; do not interpret a short window with no birth as failure.

• Mass rescaling solves bookkeeping, not experimental comparability. `Fauna::replace_config`, `lib.rs:4530–4585`, scales bodies, reserves, guts and embryos and books introduced matter, while preserving counts and clocks. Loading hundreds of grazers at ×150 is a large biomass intervention. A healthy equilibrium at 27–80 animals can look bad while that inherited population adjusts; a damaging intervention can look good while reserves buffer it. Slow headcounts and the screen's peak penalty also inherit yesterday's ecology. Keep shock recovery distinct from founding success.

• The controls may conflict. `eco.toml:67–72` ties bellwing, seedporter, capgnawer and lanternjaw mass despite opposite population problems. In completed seeds, Veilhounds killed 18–24 seedporters each; s7 seedporters encountered low offers 92.3% of the time (`err.log:1906`; deduplicated `vitals/life.csv`). Investigate accessible fruit and predation alongside mass. Lanternjaw maturation needs a functioning nutritional pathway, not a larger headcount weight. Surplus litter needs its decomposer/shredder role examined; increasing mass cannot repair an absent response to abundance.

• The landscape gap is enormous and invisible to the screen. Completed seeds have only 0.42–1.13% wooded floor, versus 20–30%. Their hour-8-to-10 leaf offtake is 0.46–0.92%, versus 10–30% (census rows at minutes 480 and 600; scorer formulas at `score.py:663–709`). Litter ends at 16,725–24,333 stock units from about 2.6–2.8 initially. Browsers should regulate foliage, decomposers should answer litter, and tree recruitment/refuges must answer canopy failure. These are mechanistic questions before numerical optimization; foliage removal alone need not create woods.

• The launcher actually evaluates seven candidates plus an unchanged baseline, not the CMA mean (`eco_search.py:354–417`). Four generations cost 256 × 1.3–1.7 = 333–435 core-hours. The same eight baseline continuations are repeated each generation: caching three repetitions saves 31–41 core-hours alone. The optimizer is diagonal SepCMA, so four updates provide little evidence about interactions. Its best selection is a difference of aggregate ranks, not a paired uncertainty estimate (`:215–221`). The launcher has no holdout enabled.

• Young d72 timing is not a mature-wide performance forecast. At s7 minute 600, total tick time is 41.15 ms, reflex processing 20.25 ms, remote sensing 0.55 ms (`err.log:1876–1878`). Further reducing remote sensing has little upside there. This single endpoint identifies a profiling lead, not a proved whole-run speedup.

2. Proposed bounded campaign

All runs below are future work. Keep heuristics fixed, the plant year unchanged and scores restricted to outcomes. Roles precede rate tuning. New ecological controls require serde fields and registry rows; no new unsafe code. Reuse existing host-local `run-study.py`, one thread per world, staged checkpoints, load/early-segment liveness and conservation checks. Respect CPU reserves, memory ceilings and other jobs. No cloud launch without Wrysk's go.

A. Reuse evidence and make predictions first; reserve at most six core-hours.

Recompute the target vector from finished baseline logs, including species persistence, adult replacement, stocks, leaf flux, hunting and landscape. Separate native cohorts from founders and starting eggs. Check one-pack/one-family structure as well as counts: counts alone do not establish social groups. Cache baseline results by binary, input checkpoint, genes, world seed, horizon and telemetry settings; do not treat cache reuse as extra replication.

Construct whole-web supply/demand budgets at target headcounts. For each food channel use measured production, reachable fraction, assimilation, upkeep, growth and reproduction. Solve approximately for mass demand and gut headroom instead of independently guessing nine numbers. As a transparent first prediction, fixed-production leaf intake must rise roughly 11–65× from the observed late windows; reducing grazer counts simultaneously requires an even larger per-animal change. This explains why the previous ×300–900 mass estimate needs testing, not treating as a fitted result.

Use a few thousand algebraic combinations, then a bounded subset of web-model trajectories, capped at one core-hour. The existing model supports logged-supply animal replay and matched starts (`web_model.rs:184–256,750–788`). Updating its inputs/fits is work still needed. Use it for hypotheses and brackets; reject candidates without simulation only on valid physical impossibility bounds, not an old fit's extinction forecast.

B. Identify directions cheaply: 20 arms × two contrasting saved wide worlds × 15 sim-minutes.

The arms are the shipped control, a budget-predicted centre, and ± perturbations for each of nine genes in log coordinates. Use two baseline worlds spanning food/abundance conditions, paired within every comparison. Short-window outputs are intake/upkeep, food drawdown, leaf flux, condition and cohort progress; these diagnose mechanisms, not final population success. Record introduced stocks separately. Fit regularized per-outcome local responses, retaining seed differences and uncertainty. Do not fit dozens of interactions to 20 settings.

C. Resolve interactions: six arms × four worlds × 30 sim-minutes.

Include both controls, two promising settings and two combinations testing the strongest suspected coupling, particularly mass × gut and grazer mass × predator ratio. Select these after B. Check whether short-window signs replicate on new worlds and different resource states. When a target remains beyond a boundary, replace an uninformative point with a budget-predicted extended bracket; do not call the nearest box edge success. If model/short-run directions disagree, retain uncertainty and promote a diagnostic challenger rather than eliminate it automatically.

D. Confirm: two candidates plus shipped control × all eight saved worlds × two sim-hours.

This retains protocol A's full screen for finalists. Reuse exact matching continuations where possible; the budget below conservatively assumes rerunning whole stages. Early stopping requires conservation/liveness failure, irreversible lineage loss, or clear sustained resource collapse. An inherited excess count, an empty short breeding window, or a poor partial P is insufficient. All promoted candidates receive identical complete horizons and seed sets; never feed unequal censored scores directly into SepCMA.

Add six targeted continuations: these three arms on two pre-event saved worlds for one sim-hour each. Existing checkpoints include m000183 and m000243 around the first pack births near minute 217. Choose the actual checkpoint from observed life events. For lanternjaw, select the oldest independent cohort and inspect growth toward adulthood; if no relevant transition can occur within the hour, say the test is inconclusive. These are diagnostic branches, not substitutes for fresh years.

E. Validate once: freeze both candidates and the assessment, then run them and the unchanged control on eight fresh, previously unused world seeds × 10 sim-hours: 24 years.

Stage the runs and inspect early outcomes, but do not retune on these holdouts and still call them independent. A setup defect invalidates its comparison; candidate-specific collapse is a result. Assess all target components and whole-window trajectories, with landscape at year end and seasonal bellwing exposure rather than a punitive single peak. Keep paired per-seed differences, spread and worst cases. Eight seeds are useful replication, not a guarantee against rare extinction.

Stopping: no more than approximately 265 active core-hours in this campaign. Stop earlier if the budgets and replicated probes establish incompatible constraints or a missing role. Return the limiting mechanism, the target bracket and a narrowly scoped proposed remedy. Stop successfully when absolute persistence, lifecycle evidence and target outcomes agree across validation worlds; disclose every exception. A lower scalar score alone is not success. One year without observable slow replacement remains unresolved and merits a targeted later extension, not automatic certification.

3. Compute, wall time and expected outcome

Use the brief's measured 1.3–1.7 core-hours per mature two-hour screen and 7–8 per fresh year. Short-window estimates scale these rates provisionally; setup, checkpoints and candidate population changes must be measured. At 60 ms/tick, a two-hour screen actually costs 2.4 hours: the central estimates are not a worst-case guarantee.

| Stage | Runs | Active core-hours |
|---|---:|---:|
| Cheap probes, reduction, model allowance | bounded | ≤6 |
| B: 20 × 2 × 15 min | 40 | 6.5–8.5 |
| C: 6 × 4 × 30 min | 24 | 7.8–10.2 |
| D: 3 × 8 × 2 h | 24 | 31.2–40.8 |
| Event branches: 3 × 2 × 1 h | 6 | 3.9–5.1 |
| E: 3 × 8 × 10 h | 24 | 168–192 |
| Total, rounded allowance | | **225–265** |

The current four-generation screen followed by the same 24-year validation costs approximately 501–627 core-hours. The proposed campaign roughly halves that, without assuming faster simulation or early-stop savings.

| Execution | Approximate elapsed campaign time, after capacity is available |
|---|---|
| Desktop, 14–16 physical-core workers | 20–24 h |
| Desktop + eidolon, 26–28 workers, equal per-core speed assumed | 11–14 h |
| One c7g.16xlarge, enough memory for 24–40 workers, equal speed assumed | 10–13 h plus provisioning/transfers |

These are wave/critical-path estimates, not total work divided by every available core. Eidolon and Graviton mature-wide throughput are unmeasured here. Scale their simulation time by measured seconds-per-tick ratios before committing a budget; reduce concurrency for measured RSS. At the brief's $0.011/core-hour, a whole 64-core host costs $0.704/hour: about $7–9 for 10–13 hours, excluding storage/transfers/retries. Active useful work would cost only $2.5–2.9 at perfect packing, which is not the invoice. Two hosts usually add idle capacity for this narrow campaign; use a second only if memory or a deliberately broader batch warrants it. Year trajectories remain serial within each world.

Expected deliverable: a ranked causal sensitivity map, tested interactions, realistic resource brackets and two independently tested candidates. A fully balanced world is conditional: zero lanternjaw maturation and the wooded-floor gap make promising that outcome unjustified. A useful failure is an identified missing ecological control before hundreds of additional core-hours disappear. Future brains and multi-year persistence remain unvalidated.

4. Novel ideas ranked by expected effectiveness per compute

| Rank | Idea, evidence status and expected benefit | Build effort |
|---|---|---|
| 1 | Paired adaptive allocation plus cached controls. **Checked:** baseline repeats and staging already exist. **Hypothesis:** early flux responses can discard many weak settings. Spend additional worlds/time where uncertainty could change promotion; preserve full finalist checks. | 0.5–1.5 days; scheduling/cache keys and a separate diagnostic reducer |
| 2 | Invert budgets before searching; learn simulator residuals per outcome. **Checked:** mass/gut ties and open-loop web-model interfaces. **Hypothesis:** viable settings lie near a small set of resource-balance relationships. Perturb along those relationships, then learn only mismatches and a few interactions. | 1–2 days, including current-data adapter; cap model fitting |
| 3 | Event-aligned starts plus a cohort transition model. **Checked:** life events, stage vitals and pre-birth checkpoints exist; lanternjaw bottleneck is visible now. **Hypothesis:** conditional growth/maturation estimates expose bad slow-lineage settings earlier than headcounts. Preserve right-censoring and validate feedback in whole worlds. | 0.5–1 day |
| 4 | Optimize the measured reflex hot path, with safe batching/reuse/spatial indexing where profiling supports it. **Checked:** endpoint timing. **Hypothesis:** substantial repeated work is removable; halving a path occupying half a tick would yield about 1.33× overall speedup there. No speedup booked in the budget. | 1–3 days after a short profile |
| 5 | Small-map mechanistic probes as auxiliary fidelity. **Checked:** d72 is cheaper when young. **Hypothesis:** intake/gut/defence response transfers; predator demographics and grove geometry may not. Calibrate paired response signs before using it to reject wide candidates. | 0.5–1 day |

Start with ranks 1–3, using existing scripts and modest analysis code. These effort estimates overlap; do not first build a general optimization framework or port the simulator to GPU.

5. First cheap probes, each below one core-hour

• **Score replay, <0.1 core-hour:** reduce completed logs under the stated target vector; expose the bellwing ceiling, absolute losses, founder/egg carryover and component saturation. Prediction: existing “feasible” or reproductive labels are weaker than the requested outcomes.

• **Cohort/role audit, <0.1:** deduplicate life records by body ID, reconstruct transitions and competing death causes. Prediction: lanternjaw failure localizes after independence and before adulthood; seedporter losses implicate predator exposure as well as food access. Check, rather than assume, causal attribution.

• **Budget bracketing, ≤0.8:** current production/uptake arithmetic plus a capped web-model batch. Predict masses and gut headroom before launching any changed world. Report bounds that remain unbracketed.

• **Rescale shock, ≤0.5:** control and predicted centre on two completed saves, one sim-minute each, preceded by t=0 stock accounting. Prediction: relative fullness survives rescaling, while absolute demand and introduced matter jump. Use this only to qualify the loaded-start experiment.

• **Timing, ≤0.5:** repeated 1,200-tick continuations from the same mature save with phase timing; compare matched conditions. Prediction: reflex costs dominate remote sensing. Do not change live jobs or infer speed from a young map.

• **Fidelity check, ≤0.8:** a few one-minute wide/d72 paired mass/gut perturbations, measuring normalized flux response. A sign reversal disqualifies small-map pruning; agreement only earns further calibration.

For these microprobes, each invocation is headless, wrapped in `timeout 270`, and uses the execution host's study wrapper. Stop at the compute cap; no probe needs a window or an unattended long run. None was executed during this review.
