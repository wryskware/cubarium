---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 next — independent finished-work review

**Disposition: keep the presentation work and the bounded experiment artifacts, but
correct the scientific interpretation and one host compatibility seam before treating
this package as a reliable basis for another training or ecology change.** The reports
are unusually explicit that assignment completion is not ecosystem health, and most of
their tables reproduce from the retained rows. The central ecological failures are not
hidden. However, the training report names the wrong body and compares material intake
directly with an energy bill; the host does not enforce the policy's new ecology hash;
the calibration gates are minimum-plausibility gates that do not test most of the
failures the prose says matter; and two causal diagnoses are stronger than the
experiment. `fast-leaf` is a reasonable **provisional diagnostic/development
configuration**, not a demonstrated improvement over the defaults. I would leave the
current non-neural display world alone; none of the findings below requires disturbing
it.

Scope: I read the four finished reports in the requested order, the dispatch handoff,
contract §§4, 6–7 and 9–15, the retained calibration/training JSON, the contact sheet,
the commit range `c8a30fa..e17e537`, and targeted source in the named calibration, ES,
population, presenter, lifecycle, motor and encounter paths. I did not run a calibration
campaign or training, did not touch the runner, state, shim or port 7393, and did not
inspect the physical cube.

## Findings

### 1. P1 — The training body is misidentified, and the “body budget, not controller” conclusion is not established

The training note calls the body a unit grazer with `diet = 0.85`, in the herbivore
guild (`ecology-v1-training-2026-09-15.md:137-141`; repeated at `:376-378`). That is not
the body the fixture creates. `World::found_training_animal` calls
`Genome::founder` (`crates/cubarium-core/src/world/lifecycle.rs:293-312`), whose default
diet is **0.7** (`crates/cubarium-core/src/genome.rs:56-58,207-223`). Ecology v1 decodes
that as `cap_foliage = 0.7`, `cap_detrital = 0.3` at `gamma = 1`, so it is a
**generalist**, not an herbivore (`crates/cubarium-core/src/genome.rs:408-433`; contract
§6.1, `ecology-v1-contract.md:324-350`). The held-out rows themselves expose the
mistake: every solitary episode reports positive detrital intake, for example h1 has
`0.265 m` (`ecology-v1-training-2026-09-15.md:188-197`).

The arithmetic quoted from the held-out table is numerically reproducible: mean served
material is `0.6436 m`, mean upkeep billed is `2.4740 e`, mean lifetime is 7,980.75
ticks, and every episode begins with `2.5 e` of usable stores. But `0.6436 m / 2.4740 e`
is not an energy ratio. A served bite must first pass through capability, food energy
density, material assimilation, battery headroom and later oxidation (contract §6.4,
`ecology-v1-contract.md:396-411`). The report's “takes in about a quarter of what it
burns” and “energy budget, not the search, is what ends the run” claims
(`ecology-v1-training-2026-09-15.md:22-29,208-215,532-540`) therefore do not follow
from the recorded columns.

What is established is narrower and still useful: this selected controller always
starves on these held-out patches, while extending this generalist body's whole-world
lifetime from about 5,574 to about 12,000 ticks. A plausible alternative remains that
the controller or short 16-update search leaves reachable intake unused. The report did
not run the disclosed mobile control on the same `fast-leaf` held-out layouts, and the
route stock itself falls from roughly 34 to 14 while the policy is present. Until a
feasible control is compared on the identical task, body infeasibility and controller
failure are confounded.

It was nevertheless right **not** to install this policy on the cube. Its lineage is
extinct in all six population trials, it changes no whole-world measure by more than
about 1%, and its body/provenance interpretation needs correction
(`ecology-v1-training-2026-09-15.md:270-325,374-398`).

### 2. P1 — `cubarium run --neural` does not enforce the policy ecology

The ES side records a configuration label/hash and correctly rejects a missing or
mismatched hash in `PolicyFile::check_ecology`
(`crates/cubarium-search/src/es/export.rs:19-40,66-95`). Evaluation and population
commands call that check (`crates/cubarium-search/src/es/commands.rs:843-846`;
`crates/cubarium-search/src/population.rs:945-949`). The host path does not. It
deserializes the policy and calls only `PolicyFile::policy()`, which checks the neural
schema digest and weights, then seeds the bodies
(`crates/cubarium/src/runner/mod.rs:238-279`). Thus `cubarium run --neural` can attach a
policy trained in `fast-leaf` to the defaults, or attach an older policy whose ecology
is unknown. This contradicts the consolidated claim that the policy is “refused by
name against any other” (`ecology-v1-next-results-2026-09-15.md:208-213`).

The inline `PolicyFile::new` repair fixed the host test's widened constructor, but not
this semantic seam. An exhaustive call-site check found no host `check_ecology` call;
the repaired test constructs a matching default hash and does not exercise a mismatch
(`crates/cubarium/tests/run_neural_seed.rs:14-24`). This is dormant on the live cube
because the policy was not installed, but it should be fixed before that ordinary
control is advertised as safe.

### 3. P1 — The six calibration gates do not measure the failures being discussed, and the winner criterion is post hoc

The gates are implemented as reported: completion, nonempty world, late foliage at
least 50% of opening, any births and deaths, herbivore plus detritivore present, and at
least 80% of opening living cells (`crates/cubarium-search/src/calibrate.rs:400-429,
431-502`). Censoring fails the late gates, which is sound. Guild assignment is fixed
from decoded capabilities at birth (`crates/cubarium-search/src/evaluate.rs:388-443`),
and the late window is the last fifth of the declared horizon, not of a shortened run
(`crates/cubarium-search/src/evaluate.rs:856-887`).

They are not health gates for the failures named in the handoff. They do not require:

- all four founder forms, or even three; the skimmer can always disappear;
- the generalist guild; `guilds_intact` checks only indices 0 and 1;
- any apex survival, mating, birth or emergence;
- a single depletion/recovery cycle;
- fruit, dead wood, or any other sign that those channels operate.

That is why the baseline can pass all six gates in all 12 held-out runs while retaining
only 2–3 forms, losing every skimmer, losing essentially all generalists in two arms,
and having no late predation (`ecology-v1-calibration-2026-09-15.md:488-522`). The
reports generally disclose this distinction, especially the consolidated disposition
(`ecology-v1-next-results-2026-09-15.md:9-16,262-275`). The stronger phrases
“plausible on every held-out seed” and “no drift toward collapse in any measure” should
not be allowed to undo that disclosure.

`fast-leaf` was legitimately shortlisted by the predeclared screen ordering
(`ecology-v1-calibration-2026-09-15.md:260-281,461-482`). Preferring it **over the
baseline**, however, was based after validation on founder forms retained, a reported
component that was neither a gate nor a predeclared baseline-selection rule
(`:500-516,550-560`). This is a transparent post-hoc exploratory choice, not a
held-out confirmation that it is better. “For free” is also too strong: the held-out
table shows roughly half the living wood (169–171 versus 334–345), a larger population,
different death balance and a different nutrient inventory (`:488-498`). Those may be
acceptable trade-offs, but they are trade-offs.

### 4. P1 — Spatial dilution is supported as a hypothesis, but its stated mechanism is wrong and causality was not tested

The observed association is real: qualifying bodies cover 259–451 distinct cells per
30-minute window, while the screen records only 0–9 depletion events per run and no
recoveries (`ecology-v1-calibration-2026-09-15.md:562-576`). The confined B1b/B6b
fixtures provide a useful contrast. The probe definition and 90% lifetime threshold are
implemented as described (`crates/cubarium-search/src/evaluate.rs:25-54,610-659`).

But the report says movement is charged per second rather than per distance, and hence
that travelling is free relative to staying (`ecology-v1-calibration-2026-09-15.md:
567-576`). The motor bill is explicitly
`move_cost * structure * (speed + k*r*abs(omega)) * dt`: translation is charged by
distance and turning by swept distance (`crates/cubarium-core/src/motor.rs:354-408`).
What is true is that the configured **per-pixel** price was deliberately reduced about
16.7-fold when speed rose (`crates/cubarium-core/src/config.rs:558-565`). Travel is
cheap, not free or time-only.

No screen arm manipulates movement cost, site fidelity, consumer density or available
area while holding the rest fixed. High range can spatially dilute pressure, but it can
also be a response to low local food; the per-window statistic omits short-lived bodies;
and the whole world supplies far more initial stock and renewal than the confined
fixtures. “Nothing holds an animal to a place” is therefore a good next hypothesis,
not an identified cause, and “every local plant feedback is dead code” is too absolute.

### 5. P2 — The apex is demonstrably transient; the mating-radius diagnosis is not isolated

The measurements do establish that one or two introduced apexes are a transient input:
0 matings, births, emergences or final survivors over 180 apex-bearing screen runs, with
small ecological effects (`ecology-v1-calibration-2026-09-15.md:427-459,603-610`).
They do **not** establish that the 10 px mating radius is the single deciding constant.
The actual mating predicate requires distance within the radius, both adults mature,
both in `Perched`, both passing stock/interval readiness, neither already committed,
and spare capacity (`crates/cubarium-core/src/world/step.rs:600-665`; radius at
`crates/cubarium-core/src/encounter.rs:14-16`). The report records no minimum pair
distance, simultaneous-ready time, or failure count by predicate. Early death or lack
of readiness could make any radius irrelevant. Radius should become an owner choice
only after that cheap opportunity audit.

### 6. P2 — Two quantitative calibration claims are stronger than the retained rows

- The accounting paragraph reports `max_abs_mass_residual <= 1.4e-10` in all 306
  screen/held-out runs and arm-0 energy residual `<= 1.2e-10`
  (`ecology-v1-calibration-2026-09-15.md:650-664`). Direct reduction of the retained
  JSONL gives screen mass `4.502e-10`, held-out mass `7.747e-10`, and screen arm-0
  energy `5.459e-10`. All remain inside the contract's `1e-9` acceptance tolerance,
  so this is a report arithmetic error, not an accounting failure. The apex residual
  minus imported energy does agree within `1.1e-11`.
- “The nutrient decline is the approach to a plateau, not a trend” and “no drift toward
  collapse in any measure” (`ecology-v1-calibration-2026-09-15.md:511-519`) are not
  established by one 300-minute horizon. In the retained `fast-leaf` rows, seeds 9002
  and 9004 still end below their own late-window nutrient means in every arm, while
  seeds 9001 and 9003 turn upward. That is mixed finite-horizon behavior, not a measured
  plateau. The consolidated report's later qualification—five hours, not indefinite—is
  the correct one (`ecology-v1-next-results-2026-09-15.md:262-268`).

### 7. P2 — Presentation stays on its side of the boundary, with two visual-policy exceptions worth vetoing

The implementation reads simulation stocks without changing ecological equations. Wood
drives structural stage through `(W/W_max)^(1/3)`; foliage fullness is `P/W`; dead wood
uses the same structural mapping; and tall height averages that wood read
(`crates/cubarium/src/art_present/habitat.rs:70-105,593-645`;
`crates/cubarium/src/art_present/tall.rs:106-148`). That is a defensible realization of
the contract's presentation boundary: persistent living structure, foliage travel on
it, distinct dead wood, and `D + C` for remains are all drawn from actual pools
(`ecology-v1-contract.md:604-619`). The ember/ash tones are distinct in the supplied
sheet; the five states are readable; and the recovery row visibly returns from rust to
green. The wood-based tall height also preserves, rather than conceals, a stripped
stand's structure.

Two items should not pass merely as aesthetic knobs:

1. **Veto the current claim that depletion is unmasked unless Wrysk accepts the
   shoulder after seeing it on the cube.** `FOLIAGE_FULL = 0.85` deliberately holds a
   bright canopy unchanged through its first 30% foliage loss
   (`ecology-v1-presentation-2026-09-15.md:120-148`; source
   `habitat.rs:85-93,633-645`). That is real masking under the handoff's “reflect the
   actual stocks” requirement. Test 0.95 or 1.0 beside 0.85 at native scale; retain the
   lowest value only if flicker is actually visible.
2. **Veto “dead wood is presented everywhere” while the soil band suppresses it.** The
   bottom five rows of each side face ignore `W` and `Wd` by design
   (`ecology-v1-presentation-2026-09-15.md:203-220`; `habitat.rs:719-728`). The soil
   scenery may correctly remain driven by `D + C`, but a separate dead-wood mark is
   needed if a real stand dies there. Do not fold `Wd` into litter; it should keep its
   distinct identity.

The taller columns are a genuine, disclosed visual change, but not an ecological
misrepresentation: height now follows the stock that actually represents structure. I
would withhold a `TALL_STEP` veto until Wrysk sees the physical cube. The contact sheet
does suggest the stripped canopy silhouette is less internally articulated than the
side-face plant, and the dead tall path still lacks a pixel-level test
(`ecology-v1-presentation-2026-09-15.md:435-451`); those are P3 follow-ups, not reasons
to reject the mapping.

### 8. P3 — Provenance is internally consistent, but the hash is misnamed

The retained policy/checkpoint values convert exactly to config
`09e244392ec91768`, protocol `8e51a1a9b1e2742b`, and policy digest
`8be01a3aa8e9f4a2`. Layout and protocol hashes include the selected ecology, and tests
exercise that they move with it (`crates/cubarium-search/src/es/fixture.rs:97-139,
720-751`; `crates/cubarium-search/src/es/trainer.rs:934-951`). The generation-9 policy
records the training build, configuration and protocol, and the held-out selection was
made from recorded training-center scores before evaluation
(`ecology-v1-training-2026-09-15.md:159-172`). That provenance is sound for this run.

`config_hash` is not FNV-1a despite its comments: it multiplies by
`0x1000000001b3`, not `0x100000001b3`
(`crates/cubarium-search/src/calibrate.rs:760-771`). Calling the one implementation
everywhere preserves identity; renaming the algorithm later is safer than silently
changing published hashes. Also, `check_ecology` enforces the hash, not the label, so
“refused by name” should be read as a named error, not label equality.

### 9. P3 — Test evidence finds no additional compiled seam, subject to sandbox limits

The relevant release tests pass on the finished tree: `cubarium-core` 467,
`cubarium-search` 86, `cubarium-render` 101, plus the host's 17 ecology-art tests and
four neural-seed tests. The latter confirms the widened constructor now compiles and
seeds a matching policy, but—as finding 2 explains—does not cover foreign ecology.

`cargo test --workspace --release` could not complete in this sandbox. The host library
reached 220 passed / 2 ignored; 27 web tests then failed only because binding
`127.0.0.1:0` is forbidden here. A second workspace run excluding the host completed
the core, search, render and surface suites, then the vendor `cube-proto` UDP test hit
the same socket permission (10 passed, 1 bind failure). I did not reproduce the web
suite in an unrestricted environment or run the four ignored renderer timing studies.
No failing assertion implicated this commit range.

## Next steps: Astra's opinion

I agree with Fable that measurement should precede another search, but I would change
the order and split the proposed coupled design task.

1. **First: close the exact per-body store budget and run one matched feasibility
   experiment.** Add the per-organism accumulator, but record the actual body first:
   either keep the current `diet = 0.7` generalist and say so, or deliberately construct
   a `diet = 0.85` grazer. For each food and body accumulate served `q`, digestible
   `q_d = cap*q`, reserve credit `eta_m'*q_d`, direct battery credit, later oxidation,
   upkeep, translation/turn cost, and terminal stores. With reproduction and growth
   disabled, check the store identity

   ```text
   usable_start + direct_energy_credit + oxidation_credit
     = body_bill_paid + usable_end + exported_on_death + residual
   ```

   on the same `fast-leaf` training and held-out layouts for stationary grazing, the
   disclosed mobile control, the initial center and generation 9. This is the single
   most informative cheap experiment. If the mobile control also dies and its best
   sustained credit/bill ratio stays below 1 despite reaching food, the body/fixture
   budget is binding. If it survives while generation 9 dies, the controller/search is
   binding. If stationary dies and mobile survives, relocation is necessary. The
   existing rows cannot distinguish those outcomes.

2. **In parallel with that small instrumentation repair: enforce ecology compatibility
   in the host.** Before `cubarium run --neural` seeds anything, compare the policy's
   required config hash with the fresh world's effective `WorldConfig`; refuse missing
   and mismatched hashes exactly as ES evaluation does. Add default/matching,
   mismatching and absent-hash host tests. This is not an ecology experiment, but it is
   required before trying any policy on the cube.

3. **Then test spatial coupling alone, before designing it together with diet
   curvature.** The current bill already charges distance. The cheapest intervention
   is a matched arm over `organism.move_cost`, for example `0.00036` (current),
   `0.0018`, and `0.006` e per structure-pixel, with all of A's other conditions held
   fixed. Measure cells/body/window, revisit interval, residence time per cell,
   depletion and recovery crossings, death cause, and net energy margin. Confirmation
   is a substantial fall in range plus repeated local depletion/recovery without a
   starvation collapse; refutation is unchanged range or deaths rising while depletion
   remains absent. Only then decide whether a new site-fidelity/memory mechanism is
   needed. I would not yet assert that a new equation is required.

4. **Do not couple `gamma > 1` to that experiment.** At `gamma = 1`, the current
   generalist already disappears. Raising `gamma` makes
   `diet^gamma + (1-diet)^gamma < 1` and penalizes breadth; without coexisting food
   niches or frequency dependence it is at least as likely to accelerate generalist and
   skimmer loss as to preserve variety. First report births, deaths and lifetime intake
   by `(founder form, diet bin, guild)` and establish whether the skimmer is lost because
   of its body/controller/habitat or because its generalist diet has lower realized
   yield. If a specialist-diversification experiment is then wanted, run `gamma = 1,
   1.5, 2` as its own matched matrix and look for stable occupation of **both** food
   channels, not merely disappearance of intermediates.

5. **Audit apex opportunity before asking Wrysk for a mating radius.** Add counts of
   two adults simultaneously alive, mature, `Perched`, stock-ready and interval-ready;
   the minimum ready-pair geodesic distance; and failures by predicate. Re-run only the
   two-apex baseline/`fast-leaf` arms. If ready overlap is zero, changing 10 px cannot
   help. If ready overlap exists but no ready pair enters 10 px, a radius or encounter
   policy becomes a real owner-facing choice. I therefore disagree with putting the
   radius decision third without this audit.

6. **Presentation follow-up:** compare shoulders 0.85/0.95/1.0 at native scale, add a
   separate soil-band dead-wood cue, and add the missing dead-column pixel test. This is
   cheap and independent of ecological tuning.

The open failures look like **several causes**, not one:

- no depletion/recovery and almost no dead wood are probably one spatial-pressure
  cluster, but movement price, behavior, density and total available area remain
  confounded;
- absent fruit is a separate threshold/allocation problem—mean `P` is below the 0.45 m
  ripening threshold, and more depletion would make fruit less likely, not fix it;
- skimmer-form loss and generalist-guild loss may overlap because the founder skimmer is
  a generalist, but form and diet mutate independently thereafter, so one cause is not
  yet shown;
- apex non-reproduction is separate again and presently confounds encounter opportunity,
  readiness and lifespan.

In short: repair the identity and host guard, run the exact matched energy/control
measurement, then isolate movement, diet and apex opportunity one at a time. Another
joint calibration or ES campaign before those checks would mostly fit ambiguity.

## What I did not check

I did not rerun any retained calibration row or the held-out policy command, regenerate
the contact sheet, inspect every changed presenter fixture, audit every one of the 2,008
new search-harness lines, run ignored timing tests, validate the temporary deployment
paths/logs, inspect the physical LEDs, or verify the live process/status. I did not
inspect artifacts that are no longer retained. Lore was unavailable during this review,
so design-history retrieval fell back to the named repository sources and Git history.
