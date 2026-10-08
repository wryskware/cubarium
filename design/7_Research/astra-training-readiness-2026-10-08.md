---
design_status: exploration
last_reviewed: 2026-10-08
decision_refs: []
---

# Astra: training readiness, 2026-10-08

## 1. Verdict in five lines

1. **Ready for bounded diagnosis and a two-lineage pilot; not ready for a large full-roster training campaign.**
2. The missing ingredient is trustworthy task definition and measurement, not another optimizer or more CPUs.
3. Only littershredder and frondgrazer have supported GRU drivers; their contract omits several newer capabilities.
4. Ecology search exists, but its shipped exclusions, persistence accounting and staging can mislabel success.
5. **First: make one current-build, paired baseline evaluation truthful, resumable and fast, then train against it.**

Scope and authority: independent static diagnosis, not implementation or a new accepted design.
Read the common brief, WORKING_POLICY, canon README and ledger; owner requirements in the brief govern this proposal.
Inspected main starting at `e5f214f5`; another session advanced it to `dcfceb85` during review.
That intervening commit changes figure presentation and documentation, not the training paths cited here.
No compilation, tests, simulations, windows, AWS/account operations or eidolon access were performed.
Only this report was authored; the pre-existing untracked `d2.log` was left alone.
Consequently **current build/test success and the reported October 7 three failures remain unverified**, not confirmed failures.
Graft provided source locations; Lore's broad retrieval sometimes returned old material, which was not treated as current truth.

## 2. Ranked blockers and fixes

### P0 — The scored objective is not the owner's requested full ecosystem

`config/training/search/roster.toml:77–115` sets `readout_only = [chorister, lanternjaw, loftstrider]` and `counts_readout = true`.
These are functional exclusions: loss, stage, replenishment and balance penalties omit those lineages; population/peak/share have zero contribution.
Evidence: `live/targets.rs:401–455`, `live/search.rs:576–579,281–333` under `crates/cubarium-search/src/es/voxel/`.
The pilot file excludes bellwing and seedporter too; neither file should be presented as full-roster success.
Moreover `search.rs:1816–1841` forgives plant loss when the default loses that plant on the same seed.
It also ignores plants absent at the candidate start; an unsuccessful founding cannot establish “every species persists.”
**Fix:** keep the historical pilot available, but evaluate an explicit required roster independently of it.
Report absolute persistence beside paired improvement; score a failed founding/required-species absence as failure, not a smaller task.
Only deliberate habitat-specific omissions in a named diagnostic map are exempt; the display-world evaluation requires the whole roster.

### P0 — Long training does not yet satisfy the staged-run policy

`live/search.rs:943–1042` steps a whole window with count/loss stops but no staged Runner or world checkpoint.
Its checkpoint contains CMA, centres, optimizer state and best genome after a generation, not in-flight worlds (`1587–1616`).
`live/grid.rs:369–408` uses the same EvalRun; completed-cell resume does not resume an interrupted cell.
`live/trainer.rs:463–477` saves centres, not its persistent worlds or familiarity; `--save-every` is not whole-training recovery.
E1's four-hour plants-only growth also runs straight through, saving its grown world at the end (`live/assembly.rs:340–408`).
Reusable staging exists: `crates/cubarium-voxel-sim/src/staged.rs:510–629` checks drivers/ledgers, samples liveness, saves segments and obeys STOP.
Its movement/eating check distinguishes sleeping and deliberately resting animals (`344–434`); reuse that distinction.
**Fix:** integrate segment checkpoint/liveness handling into the selected long-run path before launching it unattended.
Checkpoint failure currently only prints and returns (`staged.rs:631–640`): make the orchestrator stop if the expected checkpoint is absent.
A timeout alone is insufficient: `search.rs:2008–2012` only estimates the budget between generations, including no first-generation deadline.
Until repaired, use existing staged censuses and short, supervised trainer smokes; do not advertise those as an overnight training run.

### P1 — The neural contract is behind the animals, and seven lineages are unsupported

The relevant runtime is **voxel** `EpisodeGru`, not the older core `Observation70/Action7` subsystem.
`es/voxel/controller.rs:426–478` supports 23→32-GRU→3 for littershredder and 37→32-GRU→3 for frondgrazer, explicitly refusing the other seven.
`crates/cubarium-session/src/founders.rs:13–24` embeds their C1 generation-14 centres dated October 2.
Capgnawer, loftstrider, ripple-snail, bellwing, seedporter, lanternjaw and chorister/Veilhound therefore use heuristics.
The current observation still contains energy/reserve/readiness/motion feedback and old contact/taste/chem/cone channels.
It contains no stomach fullness/eating latch, time-of-day channel, stamina, or S1/S2 detected-target records (`fauna/body.rs:4242–4423`).
Here and below `fauna/` abbreviates `crates/cubarium-voxel-fauna/src/`.
The new sense pass runs, but flee, fruit search, roost and other reflexes are mostly heuristic-only (`fauna/step.rs:1153–1170,1326–1532`).
The GRU implements only drive/reset (`es/voxel/controller.rs:101–115`); it does not opt into those reflexes.
Forward remains bounded by 1 in the neural action manifest (`fauna/manifest.rs:669–688`), whereas heuristic sprint requests can exceed cruise (`fauna/gait.rs:118–126`).
The live config changes cruise/yaw references, not action channels (`fauna/lib.rs:2382–2394`).
**Fix:** tonight can test/retrain the two supported local foragers as an explicitly limited experiment.
Before broad training, settle the observation/action boundary: either expose the newer sensed information and actions, or deliberately retain shared reflexes.
This is a scope/behaviour decision, not evidence that old weights are corrupt. A digest-compatible centre can still be ecologically obsolete.
The built-in loader validates then loudly falls back (`session/founders.rs:33–79`); log actual driver selection rather than infer it from a filename.

### P1 — Reward credit crosses meal and variant boundaries

`live/window.rs:79–83,113–149` uses body + reserve + gestation escrow, with zero end energy at death, plus offspring given and curiosity.
Digestible stomach is missing from that energy stock, despite an existing mineral-aware calculator (`live/targets.rs:319–350`).
Thus digesting yesterday's food can score as today's gain; a bite late in a ten-minute window can benefit the next variant.
This is a credit-assignment concern, not proof that a particular policy has exploited it; measure the ranking change.
Brood is already a label inside reserve (`fauna/lib.rs:1452–1458`); **do not add brood again**.
The shipped curiosity weights 81/27 were calibrated before the gut/succession/senses changes (`config/training/curiosity.toml:18–38`).
**Fix:** instrument both energy definitions; propose the stomach-aware potential in §3 for approval, recalibrate curiosity on current bodies.
Retain the death loss and actual offspring transfer. Do not restore a survival bonus or pay for raw bites.

### P1 — Starts and readouts can hide the failure being measured

Saved starts correctly reject schema, build, policy and world mismatches (`live/start.rs:295–353`); old E1 data must be regenerated, not migrated.
E1 means four hours of plant growth plus constructed consumers/juveniles/pack/egg bank, not an ecology that established itself (`assembly.rs:38–74,154–260,340–435`).
On library starts only M/P rate paths change; other candidate changes are listed as frozen (`start.rs:443–469`; `grid.rs:40–55`).
A 112-block roster-pilot search therefore cannot simply be applied in full to E1 and assumed effective.
Library evaluation clears readout-only exclusions (`search.rs:761–764`), an important exception to the E0 exclusions above.
But it rebuilds its founded/species baseline **after burn-in** (`search.rs:810–832`).
`burn_in_lost` is recorded, but not included in the persistence calculation; a vanished plant or lineage can disappear from that denominator.
Late births are detected from surviving new IDs once a minute (`search.rs:984–999`); born-and-died-between-samples young are missed.
`balance.rs:137–180` calls R-hat a replacement estimate; it is surplus extrapolated over lifespan, not observed recruitment.
Its surplus also omits rider losses although B's spending includes them (`balance.rs:71–72,152–159`), overstating availability in parasitized lineages.
**Fix:** preserve the required pre-load/pre-burn-in roster; count birth/maturation events, and keep R-hat diagnostic.
Emit candidate versus installed parameters, frozen paths, actual accepted seed, imported stores and driver identities for every arm.

### P1 — Two-hour windows cannot certify the slow life histories

d72/d96 days last two hours and their season/year lasts five days: ten simulated hours (`config/desktop/terrarium-d72.toml:27–59`).
Veilhound conception is seasonal and once per pack-year; gestation is 6,000 s and independence 21,600 s (`:178–192`; `fauna/step.rs:3736–3744`).
`search.rs:2444–2462` chooses finalist duration from dusk, independence and litter interval; it does not explicitly cover the next breeding-season opportunity.
The brief's lanternjaw egg-to-adult estimate of 5–7 h is plausible but **not a fixed guaranteed clock**; hosts, meals and growth determine it.
**Fix:** two-hour screens judge access, energy, stage transitions and eligible breeding; phase-aligned mature starts test slow mechanisms quickly.
Use longer staged finalists for complete recruitment and season recovery, not fresh-world 6 h waits after every edit.
Changing lifespan or speeding maturation solely to make a short fitness window happy is outside a routine tuning decision.

### P2 — Screens, genes and teacher labels need a bounded audit

The roster file still uses old S/F bands and a 0.67 light/moisture factor; R alone is readout-only (`roster.toml:40–75`).
R/S/F use optimistic adult foliage and static regrowth, not current accessibility or succession (`live/screen.rs:281–335`).
Night endurance derives clock-wide sleep bouts from Activity (`screen.rs:421–451`), not measured den, shade, weather and meal-rest trajectories.
These calculations are useful warnings; old empirical bands are not physical impossibility proofs.
The roster has 40 gene blocks, many expanded per lineage; the pilot has 112 explicit blocks. The counts are not equivalent to equal search dimension.
Its coverage emphasizes upkeep/gut/yield/growth, not the newer habitat contest, rest, senses or seasonal mechanisms.
**Fix:** resolve every path and tie, exhibit x1 and two nearby viable candidates, report refusal causes; do not blindly widen bands until something runs.
Teacher recording wraps `Controller::drive` before final reflex overrides (`es/voxel/controller.rs:248–258`; `fauna/step.rs:1301–1574`).
It cannot teach the final newer reflex behaviours from those labels; record the executed action with matching observable inputs if that becomes the chosen task.
No current whole-world “stuck” defect was established here. Audit intended effort versus delivered movement, food access and steering source before rate tuning.
The three reported failing search tests must be rerun by the implementation owner; fix mechanisms or hand-built fixtures, never pin new ecological defaults.

### P2 — The helper launchers are behind resource policy

`scripts/scenario-starts.sh:38–39,55–61` launches all map/seed jobs together, without run-study affinity or a bounded queue.
It can link a checkpoint from a failed arm and writes OUT/DONE after wait; inspect per-arm state, not that marker alone.
`scripts/aws-graviton.sh:196` launches with worker reserve zero, no run-study wrapper and no 64 GiB hard limit.
**Fix:** bounded host-local queues, explicit pool sizes and per-arm acceptance; preserve existing data when choosing output paths.
Desktop observed during review: two L3 groups of 16 logical CPUs, load averages approximately 117/51/56. It was not spare capacity at that instant.

## 3. Proposed fitness score

### 3a. Brain reward per lineage

Separate **implemented reward** from **proposed accounting correction**; neither the scalar nor a trained result is a canon decision.
Implemented: `r = (E1 - E0 + G)/Mref + w_i N`, with death E1=0, E=body+reserve+escrow, G=positive change of reproduction.given.
All energy quantities here are assimilable-organic units, **not joules**; Mref is the fixed lineage manifest body reference.
Readout: `NetEnergy`/`BodyScore` (`live/window.rs:88–107,638–652`), including died, delta, given, novelty and curiosity separately.
Proposed: `D = Σ_food min(y_food * stomach_organic, stomach_mineral / n_tissue)`; omit the mineral bound when n_tissue=0.
`E* = body + reserve + escrow + D`, and `r* = (E*1 - E*0 + G)/Mref + w_i N`; set **all E*1=0 on death**.
Use the existing `digestible_stomach` implementation; add it to both endpoints, never gross stomach organic or assimilation a second time.
For the two current learners, G is the actual offspring transfer. Before training provisioning species, audit giving/receiving attribution per body.
No separate kill, bite, birth-count, speed or survival bonus: the ecological score measures population outcomes independently.
N is familiar-cell novelty divided by cells traversable at adult cruise in a fixed window (`live/curiosity.rs:119–205`).
Use current defaults of one adult-body-length cells and a 30 min familiarity half-life; novelty is dimensionless and not a strict probability.
Use ten-minute windows initially; preserve memory across them. Changing window duration requires weight rescaling/recalibration.
Calibrate `w_i = α_i * SD(r_energy) / SD(N)` on within-world current-build samples, then freeze it for the comparison.
If novelty variance is negligible, report no measurable curiosity signal and defer calibration; do not divide by noise to manufacture a huge weight.

| Lineage | α_i proposal / existing intent | Tonight's treatment |
|---|---:|---|
| Frondgrazer | 0.5 | Supported; calibrate against current C1 and heuristic baselines. |
| Littershredder | 0.3 | Supported; calibrate after meal-accounting measurement. |
| Seedporter | 2.0 minimum | Owner's high-curiosity intent; no supported GRU yet. |
| Lanternjaw | 0 | Preserve ambush character; no supported GRU yet. |
| Capgnawer, loftstrider, ripple-snail, bellwing, chorister | 0 provisionally | Heuristic controls; measure novelty, do not invent personalities. |

Use per-world/per-lineage centering and scaling, then antithetic ES; these mechanisms already exist (`live/trainer.rs:129–175`).
Match body age, stores, local food and season where possible; neighbouring twins are not identical environmental counterfactuals.
Run σ=0 placement-noise controls; log pair coverage, energy/novelty spreads and held-out performance, not just update norm.
Keep newborns separately reported; do not normalize a dead body's loss by its shortened lifetime.
Require both signs of each used perturbation to have adequate samples; zero for an unplaced candidate is not measured fitness (`trainer.rs:129–154`).
Validate curiosity against death-risk and depleted-food cases before accepting a centre; recalibration is not permission to erase the death cost.
**Wrysk decides:** stomach-aware potential versus strictly assimilated tissue reward; new sensor/action/reflex scope; curiosity character of additional lineages.
Routine: correcting readouts, preserving transfer/death accounting, sampling noise and calibrating already specified α values for an experimental arm.

### 3b. Ecology: whole-world objective and success conditions

Current minimized fitness is mean seed penalty + screen soft penalty + out-of-box penalty (`search.rs:2085–2127`).
The raw per-world penalty is count-band + peak + discounted lanternjaw-share + 10×movement deficit + 10×missing/silent/stage + weighted replenishment/balance.
`counts_readout` suppresses the first three; v3 replaces silence with stage shortfall; “stage” is already part of persist, not another additive term.
Current balance adds `5 Σ[(1-B)_+ +(1-Rhat)_+]`, with NaN treated as missed; replenishment adds a further five-weight shortfall.
There is **no meadow/grove/hunt-watchability term** in `search.rs:281–333`.

Proposal: record a vector of conditions first, then use the following transparent bounded penalty to rank experimental candidates.
Let `h(x)=clip(1-x,0,1)` and `d(x;l,u)=clip(max(0,(l-x)/l,(x-u)/u),0,1)` for positive l,u.
Let `L` count absent required animal lineages and plant species; include viable eggs/dormant plant states, but separately require active renewal.
Keep L for the declared starting roster, including failures during founding/burn-in. No exemption because the reference also failed.
For each required lineage i, define reproductive deficit J_i on an **eligible** interval and cycle replenishment Q_i below.
For one world, minimize:
`P = 100 L + 10 mean_i(J_i) + 5 mean_i[h(Q_i)] + 2 A + 2 K + 10 V + 2 Z + 3 M + G_b + G_v + H`.
The non-extinction terms total at most 37; no amount of aesthetic improvement compensates for one lost required species.
Invalid numerics, broken ledgers/controllers or a missing required checkpoint are invalid evaluations, not ecological success with a finite bonus.

| Term | Definition, unit and initial target | Readout / missing work |
|---|---|---|
| L | Count of lost required lineages/species. | `Evaluation` end counts/eggs and flora state; add dormant viability and pre-burn-in roster accounting. |
| J_i | 0 for a late actual birth on an eligible short-cycle test; otherwise `1 - 0.5 min(readiness/0.5,1)`. | `LineageOutcome` exists; replace minute-ID birth sampling with exact events. |
| Slow J_i | On stage trials use `1-progress`, explicitly tagged proxy; complete-cycle trial requires actual young and surviving recruitment. | Lanternjaw ladder and pup/provision counters exist (`targets.rs:363–455`); cohort-to-reproduction summary missing. |
| Q_i | `I/(I-ΔS)` over one complete day, I=assimilation+Δdigestible-gut, S=body+reserve+escrow+digestible-gut. | `cycle_books`/`replenishment` already implement it (`targets.rs:334–360`); no-income→0, insufficient interval→not evaluated. |
| A | Mean late `d(N;Nlo,Nhi)`, dimensionless. | Trace counts. Pilot Nlo=N0, Nhi=3N0, tied to a fixed reference map/start, not candidate founding. |
| K | `clip((Npeak-Ncap)/Ncap,0,1)`; Ncap=5N0 initially. | Trace peak. Include every lineage, record eggs separately. |
| V | `h(moving_share/0.30)`, second half. | Existing delivered movement >0.1 BL/s per body-minute (`window.rs:32–33`); also report awake-only share. |
| Z | Mean late `d(lanternjaw_bodies/N;0.05,0.15)`. | Existing Trace; restore scoring only on established/full-world evaluations, not egg-bank setup. |
| M | `h(largest_meadow_area/A_meadow_target)`, late median; propose 1 m² and require at least one qualifying patch throughout finalist late interval. | Census meadow_patch_max × voxel_m²; threshold is an aesthetic choice. |
| G_b | `h(largest_lanternberry_patch_area/A_grove_target)`, late median; propose 0.5 m² initially. | Census grove_patch_max exists; add age/recruitment context so founders alone are not success. |
| G_v | `h(new_established_vaulttree_sites / target_sites)`, target initially 1 on a renewal trial. | Forest patch area exists; event/cohort attribution of new persistent sites needs a reducer. |
| H | `d(hunt_bouts_per_world_hour;Hlo,Hhi)`; propose 0.5–2 bouts per simulated world-hour, averaged over ≥10 h. | Strikes/hits/killer-prey deaths exist; bounded hunt-bout aggregation and visibility/duration are missing. |

Do not treat an unobserved seasonal opportunity as a failed animal; mark the short result “provisional, lifecycle untested.”
For ranking a short pilot, slow J uses its declared stage proxy; for final certification, replace it with observed recruitment and show both scores.
Keep B, R-hat, days-to-empty, access and sated share alongside Q; do not pay separately for several correlated optimistic surplus ratios.
Census already emits meadow/grove/forest patches (`voxel_census.rs:1167–1173,1421–1429`): integrate them rather than claim the metric is absent.
Its patch numerator is top-layer occupied faces and its denominator drained ground; this is not automatically visible green area or forage connectivity.
`crates/cubarium/src/voxel/patches.rs:40–78,82–130` defines that geometry; dormant turf survival must not be mistaken for a lush meadow.
Kills alone cannot encode rare, watchable hunts; preserve event timing and one short headless sequence for eventual owner assessment.
**Provisional choices needing Wrysk:** numerical population/density bands, whether lanternjaw share includes riders, meadow/grove scale, acceptable hunt frequency.
The numerical visual targets above are starting proposals for d72/d96, not measured preferences; show their component results and sensitivity before adoption.
Until those are answered, output a Pareto comparison as well as this explicitly provisional composite; do not label its winner an accepted display design.
Weights above are a concrete proposed default, not tuned biological constants; publish them in a training config if adopted.

Noise: common accepted seeds and identical starts for all arms; compare paired ΔP as well as absolute conditions.
Use eight seeds for the normal check; report all losses, median/range and a paired bootstrap interval, acknowledging its weakness at n=8.
For search rank `F = mean_seed(P) + 0.5 SD_seed(P)` as a proposed risk penalty; missing runs are not silently dropped.
Evaluate maps separately, then weight the chosen repertoire explicitly; a flat training map must not overwhelm display-world failures by having more replicates.
Use a separate held-out set for selecting finalists, and new seeds for final comparison; do not choose “best ever” from scores on changing seed sets.

## 4. Run protocol

### Inner loop: one change answered in under 30 wall minutes

1. State a role hypothesis, a budget and predicted number before running: e.g. capgnawer access doubles while fungal production is held fixed.
2. Inspect t=0: required roster, edible accessible stock, root water, gut/reserve/brood, readiness, controller sources and reachable components.
3. Run a tiny mechanism check only if arithmetic cannot answer it; use constructed fixtures and current config values, not pinned tuned defaults.
4. Pair baseline/change on eight accepted seeds, explicit 2 h horizons, from identical saved starts at identical phase.
5. First segment 3–5 sim-minutes: controllers present, movement **or legitimate rest**, eating when hungry, mechanism counters and conservation.
6. Save/review at 10–30 min segments; keep a bounded latest checkpoint set. Stop failed setups immediately; record why each ecological arm stopped.
7. Stop hopeless candidates on required extinctions or a predeclared deficit rule; retain their failure contribution rather than average only survivors.
8. At 30/60/120 sim-minutes, compare access, input/output budgets, cohort transitions and absolute persistence, then the scalar.
9. Report a paired effect and uncertainty, and “uncertain” if the mechanism clock did not occur. A soak can continue off the edit's critical path.

For 16 worlds (two arms × eight seeds), two hours each is **32 simulated world-hours**.
At 20 Hz (`cubarium-voxel/src/lib.rs:97–99`), each world needs 144,000 ticks.
Eight concurrent worlds give two waves: 5 ms/tick measured under that concurrency costs about 24 wall minutes, excluding setup/reduction.
At 10 ms/tick it costs 48 minutes: that configuration cannot meet the target, irrespective of how optimistic its launch banner is.
The equivalent aggregate requirement is >64 simulated world-hours per wall-hour, plus setup margin.
Measure founder and mature segments at 1/2/4 simulation threads and chosen concurrency; do not extrapolate an empty arena benchmark.
Use a smaller connected map or a mature phase-specific 1 h branch if necessary; disclose what question the shorter run can answer.

### Overnight: bounded sequence, conditional on measurement and repairs

Hours 0–2: current-build search-crate verification, resolved config/screen audit, score extraction, first-segment baseline and throughput measurement.
Hours 2–4: regenerate accepted E0/E1/mature starts; inspect them before admitting them. E1 is a constructed treatment, not proof of establishment.
Hours 4–8: fixed-brain, narrow ecology comparisons and a separate two-lineage brain pilot; avoid changing both at once initially.
Start ecology with roughly 6–12 biologically related genes and λ=4–8, two common seeds for cheap proposal ranking.
Promote candidates to the normal eight-seed ×2 h check before spending a long horizon; keep the baseline in every generation.
Brain pilot: 4–8 matched worlds, 4–8 antithetic pairs initially, ten-minute windows, fixed ecology and an explicit wall/update limit.
Ensure enough animals/pair-sign coverage before increasing pairs; 16 pairs with only a few bodies per world is mostly placement noise.
Try old C1, current heuristic and sense-ablated C1 as controls; σ=0 measures placement noise before interpreting a gradient.
Only attempt a few unified lockstep generations after each isolated loop gives repeatable signal; evaluate the resulting centre/genome combinations separately.
Hours 8–12: held-out short evaluations and staged slow-clock soaks of the strongest arms; preserve summaries/centres and latest disposable checkpoints.
Use requested seeds 1–8 only if accepted as distinct; otherwise record deterministic accepted seeds. Hold out 9–16 or another disjoint accepted set.
Existing search uses separated seed bases by generation (`search.rs:2015–2022`); preserve actual accepted identities across comparisons.
For finalists use phase-aware 10–20 h intervals or saved starts covering conception→birth→independence and lanternjaw recruitment.
For long-term plant renewal use multiple season cycles and eventual lifespan-spanning soaks; two hours never establishes weeks of sustainability.
An 8 h soak is useful evidence, but does not automatically span the ten-hour season or a complete Veilhound cohort.

Resource protocol: check existing jobs/load first; per current WORKING_POLICY ceiling, leave ≥10% and one logical CPU free and apply a 64 GiB address-space limit.
Desktop ceiling is 28/32, eidolon 21/24 absent a new explicit override; shared usage can reduce these substantially.
Launch every multithreaded study with host-local `scripts/run-study.py --slot N -- ...`; distribute N across L3 groups and budget pools within each group.
On desktop the observed groups are 0–7,16–23 and 8–15,24–31; choose by current occupancy, not an assumed empty V-cache group.
Explicitly bound `--tail-cores` as well as `--threads`; defaults can enlarge per-world pools as queues empty (`main.rs:789–793,954–961`).
A starting idle-host arrangement is several two-thread worlds per group; choose the count from measured throughput and RSS, not maximum thread count.
Independent episode training may use the policy's exception, but retains its ≤16-worker cap; the live ecosystem studies are not that exception.
This reviewer did not inspect or alter eidolon. The orchestrator should consume its already-running census first and schedule around it.
No live restarts or AWS launches are included in the autonomous work; no display windows are needed.

## 5. Maps: use d72 normally, d96 for validation, flat zones for diagnosis

d72 is 256×128×72 at 0.125 m: 32 m circumference ×9 m depth; d96 is ×96: 32×12 m footprint.
Their raw voxel counts are 2,359,296 and 3,145,728: d96 has 33% more cells, not a guaranteed 33% runtime increase.
Both keep the terrarium preset and two-hour day; changing depth does not double the room around the ring for pack territories.
The DESK generator reserves a 6 m rear massif and adds foreground lowland (`terrarium.rs:67–79,442–452`).
Its front plain still retains the drainage fall around the ring (`terrarium.rs:1006–1043`): “plain” does not mean perfectly level or species-passable.
Existing knobs cover ground relief/rise/fall, lowland plain share, shelves, arches, spires, ramps, lake flats and erosion.
Generic Recipe also supports reducing relief/ridges/tilt and disabling structural passes (`recipe.rs:1044–1148`; `generate.rs:976–1064`).
I found no finished named training preset guaranteeing several flat, connected, food-complete zones for every animal size.
Do not promise that zeroing terrain relief alone does so: water, crown obstruction, steps and body clearance also decide accessibility.
Build a small authored/generated fixture and validate components using the existing fauna geometry/route rules; do not invent a parallel notion of passability.

| Map/start | Purpose | Status / proposal |
|---|---|---|
| Flat connected patch, width sufficient for the tested body | Isolate sensing, food finding and unobstructed grazing. | New small fixture/preset; shallow water edge, ground forage and a reachable crown zone; no predators initially. |
| Same zones with one ramp/obstacle/seam crossing | Separate movement failure from feeding failure. | New controlled variant; assert reachability at t=0, measure delivered motion. |
| d72 terrarium, E0 + mature saved starts | Main mixed ecology/brain evaluation. | Existing config; regenerate starts and audit full founding roster. |
| d96 terrarium, held-out seeds | Transfer to deeper lowland and different habitat mixture. | Existing config; keep physical units and count-density normalization explicit. |
| `patch`, `dry-hills`, `wet-valley` | Fast plant/grazer mechanism checks. | Existing scenarios; patch is 160×72×24, deliberately omits the two predators. |
| `range` | Multi-pack spacing/hunting hypothesis. | Existing 512-column, 64 m ring; use when circumference, rather than depth, is the limiting factor. |

Source for existing scenarios: `config/scenarios/{patch,range,dry-hills,wet-valley}.toml`, including their documented exclusions.
Report reachable food area and viable founder counts per lineage, not total floor area alone; normalize bands to the fixed reference map.
Cross-check selected finalists at display water cadence 1 after fast experiments at cadence 4; dynamics are not assumed identical (`main.rs:963–964`).

## 6. Work split

### A — Autonomous next 8–12 h, ranked by value per hour

These are recommendations to the parent orchestrator, not actions performed by this read-only reviewer. Choose a bounded subset as capacity permits.

| Rank | Work and concrete output | Rough effort | Machine |
|---|---|---|---|
| 1 ★ | Verify current search crate; classify the three reported failures; resolve presets/genes/centres; print the exact active objective and exclusions. | 0.5–1.5 h | Desktop, low-priority bounded build/test once. |
| 2 ★ | Repair required-roster/burn-in loss accounting and exact birth readouts; retain paired plant comparison as a diagnostic. Small mechanism tests. | 1–2 h | Desktop development; tiny local checks. |
| 3 ★ | Make selected study/training path segment-resumable, fail on checkpoint/liveness failure; bound scenario queue and host-local affinity. | 2–3 h | Desktop development; short local resume check. |
| 4 ★ | Reuse census patch/killer-prey/energy counters in one comparison reducer; add installed-versus-requested knob and driver reporting. | 1–2 h | Desktop; existing census output when delivered. |
| 5 ★ | Benchmark cold/mature d72 segments and build one connected control map plus a few fresh valid starts; show t=0 accessibility. | 1–2 h setup + growth | Desktop small checks; eidolon after its census frees budget. |
| 6 | Paired heuristic/C1/sense-ablation check and σ=0 noise calibration; measure old/new energy potential and curiosity scales without adopting either. | 1 h setup + 1–3 h bounded compute | Available desktop slot; otherwise eidolon. |
| 7 | One role-driven ecology contrast and two-lineage pilot if prerequisites pass; held-out comparison, no live deployment. | Remaining 2–4 h compute | eidolon; desktop only if measured spare capacity exists. |
| 8 | Prepare EC2 command/budget plan and aarch64 compatibility checklist; correct stale launch assumptions. No launch. | 0.5–1 h | Desktop, read-only preparation. |

★ Low-hanging fruit means high decision value, not guaranteed trivial code. Items 1–5 are more valuable than an uninterpretable night of optimization.
If staging repair overruns, finish the paired staged census and current-state report; do not spend the remaining night on an unsafe/unrecoverable trainer.
No tuned-number tests, schema migration, new unsafe code, archival build copies or changes to accepted design are needed.
Each ecology contrast names the stock and its answering role: litter→shredders/caps; cap biomass→capgnawers; prey abundance→hunters; accessible foliage→browsers.
Separate fungal cap versus structural/mineral stock, and carrion production versus actual meals; abundant inaccessible food is a behaviour problem first.
Meadow decline calls for inspecting woody recruitment/competition and the wet–dry niche before simply increasing turf growth.
These are hypotheses to check against the incoming census, **not newly measured claims that those stocks currently diverge**.

### B — Wrysk decisions, with options and recommendation

1. **Which brain task first?** (a) two current local foragers; (b) extend their senses/actions first; (c) all nine now.
   Recommend a bounded (a) benchmark tonight, then (b) for the next substantive training round; defer (c).
   Within (b), choose fully learned newer behaviours versus shared reflexes; I recommend explicit sensed inputs/actions, with unavoidable body constraints shared.
2. **Should acquired digestible food count in net energy before assimilation?** (a) tissue/reserve only; (b) add the digestible gut potential.
   Recommend (b), measured side by side first; this preserves death/offspring accounting and reduces delayed meal credit between variants.
3. **What constitutes ecological success across slow clocks?** (a) every short run breeds; (b) short stage tests plus complete-cycle recruitment.
   Recommend (b), retaining every required lineage/species on full-world checks; agree the population density bands and whether the 5–15% lanternjaw share includes riders.
4. **What should the display look like quantitatively?** Absolute meadow/grove patch area or area fraction; rare hunts measured per world-hour or per predator-hour?
   Recommend at least one visible continuous meadow and several persistent grove sites, scored in m², and hunts per world-hour; choose thresholds after viewing representative cases.
5. **What cloud budget is worth spending after a useful pilot?** (a) owned machines only; (b) one capped Spot trial; (c) two hosts overnight.
   Recommend (b) first, then (c) only after mature-world cost and recovery are measured; an initial $30 Spot planning cap covers the example below with margin.

The first four decisions affect scope or the meaning of success; they do not block routine diagnosis, metric repairs, starts or throughput work.

## 7. EC2-scale plan

Run independent complete jobs per host (existing mode A), not a WAN-coupled lockstep trainer initially.
`scripts/aws-graviton.sh:5–23,84–115` supports c7g/c6g fallback, Spot default, self-termination and delete-on-termination storage.
AWS lists c7g.16xlarge as 64 vCPUs/128 GiB: [C7g specifications](https://aws.amazon.com/ec2/instance-types/c7g/).
The repository's September 28 runbook measured good **frozen landscape** scaling; that is not current d72 ecology throughput.
More relevant historical evidence: the October 1 cotrain report says two c7g hosts gave only ~15% speedup because of the slowest-world barrier (`design/cotraining-plan-2026-10-01.md:127–131`).
Use independent seeds/candidate evaluations or replicate searches to exploit extra hosts; measure each host's L3 topology and mature-world memory first.
Example large campaign: 2 hosts ×8–12 h; 2 independent searches, λ=8, four ranking seeds, 8–16 generations if measured cost permits.
Eight generations require about 576 world evaluations including references: 1,152 simulated world-hours at 2 h each, before holds/soaks.
Reserve at least a quarter of the budget for held-out eight-seed checks and complete-cycle finalists; reduce generations if the measured budget does not fit.
Compute `cost = host_hours × actual regional hourly price + storage/transfer`; do not budget from ticks/s on an obsolete arena.
Planning assumptions **not live price quotes**: $1/host-h Spot or $2.50/host-h On-Demand, conservatively rounding the dated runbook's ~$0.64–0.75/$2.32 c7g rates.
That gives $16–24 Spot or $40–60 On-Demand for two hosts ×8–12 h; 25% reserve yields approximately $20–30 / $50–75, plus small storage/transfer charges.
AWS's current pricing page says Spot prices vary by pool/time; its dynamic table did not supply a verified c7g quote in this review: [Spot pricing](https://aws.amazon.com/ec2/spot/pricing/).
Before launch: price/availability/quota check, x86/aarch64 qualitative result check, current-build tests, representative throughput and a recovery rehearsal.
Fix the launcher to enforce the current reserve, 64 GiB limit and study affinity; do not copy the old reserve-zero command unchanged.
Use periodic off-host checkpoint/result retrieval, not just final fetch: Spot can interrupt with a two-minute warning ([AWS interruption documentation](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/spot-instance-termination-notices.html)).
Keep a bounded rolling checkpoint set and versioned summaries/centres; the optimizer and world's recovery requirements must both be covered.
Do not call `up --dry-run` as a harmless account probe: the script calls key/security-group setup before its launch dry-run (`aws-graviton.sh:84–100`).
Wrysk's launch approval is required; nothing in this diagnosis starts or reserves a host.

## 8. Uncertainty and the measurement that resolves it

**The highest-value single measurement:** a current-build paired C1-versus-heuristic evaluation from identical d72 starts across eight seeds, staged at 3/30/60/120 sim-minutes.
Record drivers, requested/delivered motion, bites and assimilation, digestible gut, energy stocks, births/maturation/deaths, food accessibility and concurrent ms/tick.
Include σ=0 only if measuring ES placement noise; the basic comparison must remain understandable without training underway.
This separates a poor brain from an unreachable food web and quantifies whether the under-30-minute protocol is physically achievable.
Consume the existing eidolon census first; its outcomes may eliminate arms or supply current starts, but only if build/schema/controllers match.
Remaining uncertainties: actual current test failures, accepted seed availability, current centre digest acceptance, mature-world throughput, and the severity of reward-ranking changes.
Also unmeasured: current predator meal success, actual lanternjaw completion time, dormant-plant recovery, multi-pack space and full seasonal recruitment.
One cohort trace spanning each slow transition resolves its timing; neither an old report nor a two-hour survivor count does.
No present claim of ecosystem stability, brain superiority, or a cloud speedup follows from this static inspection.
