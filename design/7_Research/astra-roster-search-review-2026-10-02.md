Verdict: **no-go** for an overnight full-roster search at `31a147de`.

# Astra — full-roster search groundwork review

Reviewed branch `rsearch`, head `31a147dea90bae9e45b3c34ceab1116db62e6deb`, in
`.claude/worktrees/rsearch`. All code/config/brief citations below are relative to
**that worktree**, not main. This is review evidence, not a design decision.
The reported exhibit screen ratios and earlier two-seed outcomes are worker evidence
supplied in the review request; I did not repeat those simulations. I inspected the
implementation and tests and independently checked the scoring/cost arithmetic.
Graft fell back to main's index, so branch-specific source was read directly.

The groundwork is useful, but the current objective can spend the night optimizing
survival on founding reserves and host attachment while known food-access failures
truncate everyone else's evidence. A **smaller, explicitly partial pilot** is reasonable
after findings 1–3 are addressed and its objective is checked end to end. Do not label
that pilot a full-roster sustainability result. Findings 4–6 must be addressed before
using the advertised per-lineage or finalist verdicts as an acceptance gate.

1. **Blocking — Known broken feeders can terminate every candidate's useful evaluation; downweighting their birth targets alone is insufficient.**

   **Evidence:** `config/training/search/roster.toml:82–106`;
   `crates/cubarium-search/src/es/voxel/live/search.rs:347–365,742–781,865–879`;
   `targets.rs:328–334`. `EvalSpec::new` sets `stop_on_loss=true`; search stops at
   the first loss of **any** founded lineage. Exhibit/finalist runs set it false
   (`search.rs:2165–2167`), so their two-hour observations are not the trajectory
   duration CMA will necessarily see. If seedporters disappear before one cycle,
   all eight replenishment targets get maximal shortfall: **40 penalty points**,
   independent of their otherwise successful feeding. Early death can also erase
   other lineages' opportunity for a late birth or the first dusk. The user-reported
   near-zero bellwing intake, extinct seedporter and unfed chorister make this an
   immediate risk, not an edge case.

   **Change:** for tonight, keep these three lineages in the simulated food web
   and in diagnostic output, but make them **readout-only for optimization**:
   omit their stage/replenishment/loss penalties and exclude their loss from the
   stop condition. Specify how their counts affect the population-band objective
   so the search does not gain fitness merely by preserving or killing their
   endowments. Freeze their six genes each and the founding pack-size gene for
   this pilot. Continue early stopping on failures that actually settle the
   pilot's scored question. This needs an explicit scoring/stop mask; the current
   `persist_only` option does not implement it for v3. Merely reducing all targets'
   `missing_cost`, or removing names from `birth_required`, will not fix the
   global stop and replenishment penalties. Restore full-roster objectives after
   the feeder mechanics are repaired and recheck the candidate then.

2. **Blocking — The lanternjaw growth ladder is a readout, not a fitness gradient, and dead riders can satisfy it.**

   **Evidence:** `targets.rs:262–274,313–316`;
   `roster.toml:90–101`; `search.rs:866–879,980–999`;
   `roster_tests.rs:335–373`. With target 0.5, attachment, half-grown rider,
   detached juvenile and successful hunter score 0.5, 0.625, 0.75 and 1, but
   **all four have zero shortfall**. CMA consumes shortfall, not ladder score.
   The added drain/detach genes therefore have no direct stage reward for improving
   growth after attachment. Attachment is cumulative: one rider can attach, die,
   and leave viable dormant eggs; the lineage is not `lost`, so it still meets
   the target. Host-death release is also promoted to the same 0.75 rung as
   growth-triggered dropoff, potentially rewarding damage to hosts. A free body
   switches the share discount off even if it is only a freed juvenile.

   **Change:** separate the attainable two-hour **pass threshold** from a graded
   objective that continues to reward survival and growth above attachment.
   Require a live rider or demonstrably surviving descendant for attachment
   credit, and distinguish healthy dropoff from host-death release. Use the
   existing rider-growth readout in fitness, with actual size gain alongside
   progress toward a gene-controlled detach threshold. Keep adult/hunting credit
   separate from merely living free. Do not solve this by demanding a free adult
   in every two-hour seed: the supplied 1–3-hour growth observations and missing
   first-dusk hosts show why that is not an attainable universal starting gate.

3. **Blocking — The food screens are not proven upper bounds, so they can discard viable candidates.**

   **Evidence:** `screen.rs:516–541,564–579,614–730`;
   `roster.toml:67–80`; `roster_tests.rs:562–584`.
   The code explicitly claims a refused lineage “cannot be fed,” but it fixes
   plant abundance to founding caps and vine faces to **one at founding**.
   Neither limits abundance over the following two hours or in a sustainable
   mature world. Measuring 12 roots/12 faces establishes the initial state, not
   a maximum productive surface. Current reserve and food stocks can bridge
   low initial production while plants or vines grow. Several other terms are
   estimates, not conservative impossibility bounds:

   - Foliage income assumes `light_moisture=0.67`, below a possible value of 1,
     and filters on an adult crown's band, which need not bound juvenile geometry.
   - Nectar/fruit production uses horizon-to-horizon sunlight duration, whereas
     implementation produces whenever `daylight>0`, including twilight
     (`crates/cubarium-voxel-flora/src/step.rs:2697–2718`).
   - Film uses the optimum of an averaged day/night logistic rate. A fixed-stock
     optimum under averaged light is not automatically an upper bound on a
     time-varying stock/harvest trajectory; it also uses floor area for habitat
     and a harvest-fraction movement estimate (`screen.rs:684–695`).
   - Carnivore “food” still rejects against assumed carrion from founding prey
     and their nominal breeding/lifespan clocks, even though predator balance is
     advertised as readout-only. Predator need ignores satiated torpor savings
     and, for juvenile lanternjaws, income from hosts. An imaginary full adult
     per founding clutch is not the actual two-hour consumer population.

   **Change:** tonight, demote these stock-flow/access estimates to readouts or
   soft penalties. Retain a hard refusal only where all relevant supply paths
   are absent or a genuine, documented upper bound is below unavoidable demand.
   Keep `vine_faces=1` as an accurately named **founding measurement**, not a
   rejection bound. Passing ratios of 3.3–659× are useful diagnostics; seedporter
   starvation despite passing is expected because these calculations do not
   model actual navigation, contact or feeding opportunities. The tests currently
   exercise rich/zero-supply examples; they do not establish absence of false
   rejection near the boundary.

4. **Important — Replenishment fixes the brief-meal-rise trap, but “met” ignores it and the finalist checks only the first cycle.**

   **Evidence:** `targets.rs:253–258,302–334`;
   `search.rs:485–512,768–770,1019–1027,2204–2216`;
   `roster_tests.rs:417–435`. For a lineage with intake 1, starting biomass 2
   and end biomass 1.75, replenishment correctly gives **0.8** and shortfall
   **0.2**. A late birth can nevertheless leave `met=true`; incomplete cycles
   also do not clear that flag. The printed `met/worlds` therefore is **stage
   met**, not the combined target outcome. In an eight-hour finalist, `snap_cycle`
   is captured once at hour two and never refreshed: hours 2–8 cannot invalidate
   the replenishment result.

   `Snap` counts living bodies, reserves and gestation escrow but not laid eggs.
   Moving organic matter into eggs looks like expenditure and later hatching
   looks like a gain. That is not a closed lineage inventory, and can bias
   comparisons between egg layers and gestators. Within a closed lineage total,
   one-cycle net retention is a useful recruitment-inclusive balance metric;
   it should not be described as a pure metabolic spending measurement.

   **Change:** distinguish `stage_met`, `replenishment_met` and combined `met`
   in both JSON and the printed summary. Include viable egg organic stocks or
   explicitly account for laying/hatching transfers. Track consecutive complete
   cycles and judge later/post-founding cycles in finalists. Preserve the current
   readiness cap: readiness without a required birth correctly cannot meet the
   birth target (`targets.rs:303–310`); the missing tests are the combined verdict,
   egg transfers and a good first cycle followed by a losing later cycle.

5. **Important — The chorister's “pack intake” quarter accepts a hit without a meal; its finalist kill counter is not strict kill attribution.**

   **Evidence:** `targets.rs:279–285`; `roster_tests.rs:378–411`;
   `crates/cubarium-voxel-fauna/src/pack.rs:411–423`.
   A litter, any provisioning, sufficient surviving pups and **one hit** can
   score all four quarters with zero carrion intake. The test checks a zero-hit,
   zero-intake example but misses this case. Even a kill alone does not prove
   the pack ate. `pack_chase_kills` increments when a target disappears and
   carrion exists at its target site, without checking whether the pack caused
   that death; starvation during a chase can qualify.

   **Change:** require actual positive assimilated food intake for the intake
   quarter, independently of hit/kill credit; combine it with cycle replenishment
   for sustainability rather than treating one tiny bite as sufficient. At
   finalist stage require an attributed chorister kill **and** consumption.
   Retain any-time litter credit and pup survival/provisioning—the six-hour
   interval should not punish an early litter. These checks do not need to block
   a pilot with choristers excluded from scoring under finding 1, but they must
   be corrected before enabling its advertised target.

6. **Important — `--finalist` is a long fresh-world run, not yet the promised slow-clock validation.**

   **Evidence:** `search.rs:2101–2185,2204–2216`; `targets.rs:262–274`;
   `design/handoffs/roster-search-groundwork-2026-10-02.md:59–67`.
   Refounding is explicitly documented and deterministic continuation from
   time zero can be acceptable; the problem is the verdict. The default
   horizon is two hours plus the maximum configured wait (normally six hours),
   not event-relative validation of every litter or late hatch. One first litter
   plus surviving pups still satisfies the chorister quarters; independence or
   a subsequent litter is not itself required. A lanternjaw can reach the top
   rung through eggs laid, or a cumulative hit and any free survivor, without
   demonstrating successful feeding by an established hunter. Finding 4's
   first-cycle-only replenishment compounds this.

   **Change:** inspect actual event times and require the intended later-stage
   outcomes: growth-triggered release/continued survival, free-living feeding,
   independent pups, and replacement opportunity after the litter interval.
   Prefer continuing saved finalist worlds to paying the founding two hours
   again, but do not turn save support into tonight's critical path. A long run
   with these events merely possible is a diagnostic soak, not a pass certificate.

7. **Important — Night bisection is sound for this clock; R demotion is sensible, but S/F remain design heuristics rather than impossibility screens.**

   **Evidence:** `screen.rs:147–160,273–329,336–410`;
   `roster_tests.rs:67–124,280–294`;
   `crates/cubarium-voxel/src/weather.rs:404–424`.
   The night calculation uses the simulation's own smooth daylight function and
   `Activity::awake`, refines crossings and wraps bouts correctly. The checked
   normal-clock tests give about **2,304 s diurnal** and **4,896 s nocturnal**,
   summing to 7,200 s; usable-reserve arithmetic gives the expected 700 and
   1,166.67 seconds. I found no bisection defect for the shipped fixed thresholds.
   A 14,400-cell crossing grid could miss extremely narrow windows if clock
   thresholds were evolved later; local mode also substitutes a 12-hour day.
   Neither is the present cycle-mode search's main risk.

   Night rejection enforces a chosen 80%-onset, 1.2-margin uninterrupted-sleep
   design envelope, not biological impossibility: an animal that occasionally
   wakes to feed could live outside it. That deliberate envelope is reasonable
   if named honestly. Removing the R parking constraint is reasonable with an
   actual browser movement target; frozen curiosity-trained brains are not a
   guarantee of motion under changed costs. S's upper bound still rejects rich
   standing food even if the trained browser moves, while F uses a fixed harvest
   estimate and demands much more than break-even (`F_min=10`, hard refusal
   below 5 with margin 2). These can also reject good candidates.

   **Change:** keep the requested night envelope, label its purpose, and retain
   observed movement as the acceptance signal. Demote S/F to soft/readout criteria
   for the initial pilot unless Wrysk specifically wants those additional hard
   behavioral constraints. Do not claim all pre-screen rejections are impossible
   ecologies. No rewrite of the normal-clock bisection is needed before launch.

8. **Important — The 73-gene set is coherent, but its extremes and founding endowments need an explicit pilot scope.**

   **Evidence:** `roster.toml:108–160,165–288`;
   `roster_tests.rs:526–559`; `crates/cubarium-voxel/src/genome.rs:438–445,513–533`;
   `config/desktop/terrarium.toml:56–141`.
   The count is **6×9 + 15 plant + 1 pack + 3 rider = 73**. Frozen growth,
   lifespan and reproduction fields mean frozen **live terrarium** values, not
   old code defaults—appropriate given the explicit base. Upkeep u multiplies
   motor by u and bite by **u×intake**. Intake up to ×10 can restore bite after
   upkeep ×0.12, but not after the allowed ×0.05 extreme: maximum bite there is
   only half its base. That is a real tradeoff, not a decoder bug. Reserve up
   to ×5 combined with full founders supplies much more initial biomass/energy;
   it can pay early births, pup provisioning and later carrion without renewable
   food success. Fixed reproduction costs then become easier relative to stores.

   The three rider knobs address the measured recruitment bottleneck, but reduced
   host upkeep also reduces rider income. Freeing drain/detach without fixing
   finding 2 gives CMA little reason to solve healthy growth and some reason to
   exploit host release. Pack size also changes both founding biomass and the
   population band, rather than representing a fixed resource-budget comparison.

   **Change:** keep the life-history freeze and explicit base. For the partial
   pilot, freezing the failed-feeder 18 genes plus pack size leaves **54 genes**.
   Keep rider knobs only with a meaningful survival/growth objective. Report
   initial biomass and recurring replenishment alongside population outcomes;
   do not call a founder-funded result sustainable. Do not widen intake solely
   to preserve the maximum bite everywhere without checking the intended tradeoff.

9. **Important — λ16 × three seeds is too expensive for the first diagnostic night unless throughput and objective signal justify it.**

   **Evidence:** `search.rs:1681–1743,1772–1785,1805–1821`;
   `crates/cubarium-search/src/main.rs:775–804`; the supplied **26 minutes per
   world-thread** measurement. Every generation runs an extra reference on the
   same seeds: λ16 means **51 worlds**, not 48, or **1,326 thread-minutes**.
   At 16 workers that is at least 82.9 minutes, roughly **104 minutes** for
   equal-duration jobs in four waves. At eight workers it is at least 165.8,
   roughly **182 minutes** in seven waves. An eight-hour night buys only around
   four or two complete generations respectively before overhead/variation.

   **Change:** after fixing the objective, start with **λ8, three common seeds
   per generation**, a fixed wall budget and a measured passing-draw fraction.
   Including reference, 27 worlds cost 702 thread-minutes: about **52 minutes at
   16 workers**, or **104 at eight**, under the same equal-job approximation.
   This is exploration, not convergence of a 54/73-dimensional ecology. Use
   the allowed worker ceiling and current load; retain `CUBARIUM_THREADS=1`,
   niceness and the 64-GiB limit. Do not infer throughput from runs shortened by
   broken feeders. Begin with a small explicit proposal step (e.g. σ0 0.05–0.1)
   and cheap draw/screen checks: CLI σ0=0.5 acts on value-mode wake hunger too,
   starting near 0.9 with upper bound 0.95, and can spend many draws on clipping
   or rejected sleep budgets. Increase λ only after useful ranking is visible.

10. **Important — Launch defaults can quietly change the experiment, and best-ever fitness compares different seeds.**

    **Evidence:** `roster.toml:21–36,73–77`;
    `crates/cubarium-search/src/main.rs:777–804,872–885`;
    `search.rs:1687–1705,1805–1821`.
    The example command still requests **eight** seeds per generation, while the
    proposed run uses three. Search defaults water updates to every **four** ticks;
    the exhibit/finalist check defaults to **one**. Search defaults are also one
    simulated hour and a three-hour wall budget unless overridden. Relative base/
    brain/world paths make launching from the correct checkout important. The
    night-mechanics hook is still unmerged here (`night_bloom_scale=0`), so tonight
    is an explicitly pre-mechanics experiment. Finally, seeds change each generation
    and “best” is chosen by lowest raw fitness across those different seed sets;
    a favorable placement set can win that contest.

    **Change:** write one concrete pilot launch with explicit worktree, search,
    start, world, λ, seeds=3, hours=2, σ, water cadence (use **1** to match the
    reported checks unless deliberately benchmarking an approximation), workers
    and wall budget. Compare candidate and incumbent on common/held-out seeds
    before selecting a finalist; treat `best.toml` as a nominee. Tomorrow's feeder
    and night changes require a new evaluation, not automatic promotion of this
    night's best.

11. **Important — Close the review with small counterexample checks, not another long exhibit.**

    **Evidence:** `roster_tests.rs:315–435,448–559` and the scoring paths cited
    above. The arithmetic and decode tests are useful, but mostly validate intended
    happy paths. Their current assertions let growth change a readout without
    changing fitness, accept stage success despite failed replenishment, and omit
    a hit-only chorister. The earlier exhibit belongs to an earlier genome; its
    results should not be relabeled as a current search-mode smoke.

    **Change:** before the reduced launch, run focused checks for (a) excluded
    feeder loss not truncating scored lineages, (b) growing/surviving riders scoring
    better than attach-and-die with eggs remaining, and (c) the actual launch
    configuration reaching the intended score/serialization path. Before enabling
    full-roster/finalist approval also check hit-without-intake, birth with negative
    cycle balance, egg-stock transfer and later-cycle decline. These are small
    deterministic accounting/target cases, not multi-hour unit tests. A bounded
    pilot generation can establish real throughput and ranking; no new overnight
    is authorized by this review alone.
