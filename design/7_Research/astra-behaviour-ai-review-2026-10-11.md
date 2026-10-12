---
design_status: exploration
date: 2026-10-12
---

# Astra review: behaviour AI, family defence and pack tactics

## Summary

The direction is sound, but the proposal needs contract changes before implementation.
The largest defect is mathematical: today's within-world normalization cancels the proposed pack reward's shared benefit.
Second, the observation and recurrent timing contracts are incomplete; a faithful clone can lack the information, subjects or transition experience needed to work.
Third, adding today's reactive counter alongside aimed kicks defeats the proposed facing and cooldown limits.
Sparse encounters and interacting antithetic variants also invalidate the promise of quick top-only learning.
Signals-only coordination, energy-only reward and staged learned layers remain reasonable choices.
None of the evidence below requires reversing Wrysk's eight calls; the shared-family fallback is already authorized by topic 2.
This is a code-and-design review only: no builds, tests or simulations were run.

References below use **proposal** for `design/behaviour-ai-2026-10-11.md`, **survey** for `design/7_Research/behaviour-ai-code-survey-2026-10-11.md`, and **ethology** for `design/7_Research/behaviour-ai-ethology-2026-10-11.md`. The proposal was committed as `36a8e451` during this review. Code references are to the working tree read for this review. Proposed checks below are mechanisms to add, not tests performed here.

## Findings, most important first

### 1. The current score normalization removes the cooperative pack incentive

**Claim under review.** Proposal §10.3 and §9 say a hound that cuts off prey instead of eating first benefits from its packmates' meals.

**Evidence.** `crates/cubarium-search/src/es/voxel/live/trainer.rs:140–161` z-scores bodies within each world and lineage before pooling by candidate; `trainer.rs:493–525` feeds those values to ES. For one pack with equal packmate weight, write each member's own energy score as `e_i`, and the pack total as `T`. Its proposed score is exactly:

`S_i = e_i + r × sum(other members' e) = (1 − r) × e_i + r × T`.

Every member receives the same `r × T`. Subtracting the within-world mean removes that term. For `0 ≤ r < 1`, division by the within-world standard deviation also removes `1 − r`: the normalized scores are **identical to scoring own energy alone**. At `r = 1`, all scores are equal and normalize to zero. This applies exactly to the four-adult, one-pack case without extra relationship terms; pups or unequal relationship weights change the algebra but do not repair the underlying comparison. With only two mutually related bodies at equal weights, the same cancellation applies.

**Concrete failure.** A cutoff hound that increases everybody's food does not gain the intended extra training credit. A sigma-zero noise run cannot reveal a useful cooperative gradient that the scorer deterministically deletes. Merely assigning one shared variant and one shared score per family also fails if that score is repeated over its bodies and normalized within a world containing only that family.

**Proposed change.** Specify the unit of comparison before implementing KIN-TRAIN. Preserve the energy formula, but compare kin-weighted returns across matched evaluations of the same focal body or family. Do not center away the sole group's total inside its world. In the shared-variant fallback, produce one group return per episode and compare plus/minus group variants across matched world copies. Keep raw own, relative and combined scores in the readout. Add a tiny algebraic check in which a perturbation improves only a relative's energy and verify that its training advantage survives the complete scorer, including normalization and ranking.

**Owner calls.** Spec correction supporting topic 2, **kin-weighted reward**. No change to its energy-only basis. The recorded fallback remains usable only with the corresponding estimator change.

### 2. “Living relatives” must not discard the very death defence is meant to prevent

**Claim under review.** Proposal §10.3 adds each *living* relative's score over the window; §9 says defence pays because calves survive.

**Evidence.** `crates/cubarium-search/src/es/voxel/live/window.rs:118–140` deliberately sets end energy to zero at death. `window.rs:539–545` retains a disappeared body's meter and marks it dead, and `window.rs:657–698` emits its score. `crates/cubarium-voxel-fauna/src/lib.rs:1103–1108` records maternal identity for life, but founders and introduced bodies have `mother = None`.

**Concrete failure.** If “living” means present at scoring time, a calf killed during the encounter disappears from its mother's return instead of contributing its negative energy change. A surviving calf spending energy can then be worse for her score than an omitted dead calf. The same problem applies to dead packmates. Looking up relations only among surviving animals also loses a dead mother's links. A naive equality check on maternal IDs makes all `None` founders siblings. Separately, founded adults need not be relatives: kin reward does not automatically make an unrelated adult value every family calf, even though the heuristic helps them all.

**Proposed change.** Define a reward roster from relations at window start, with explicit birth and membership-change rules; retain its meters after death. Include each relative's **base own-energy score**, not its already kin-augmented score. Define how overlapping sibling/packmate relations combine, and require a real shared maternal ID for siblings. Meter heuristic relatives too: today's final `BodyScore` list skips bodies with no candidate (`window.rs:670–672`), although their energy was tracked. A staged family must explicitly establish its mother–calf relation instead of assuming introduction does so. Check calf death, mother death, a newborn during the window, and a heuristic relative with hand-built ledgers.

Keep the limitation explicit: this rewards retained calf energy and future gains represented inside the window, not a calf's entire reproductive future. A rescue costing more energy than its weighted benefit can correctly lose under the agreed objective. Do not silently add a survival or defence bonus to force it to win.

**Owner calls.** Spec clarification preserving topics 2 and 7, **kin reward and the two-way bond**. Any demand that unrelated helpers invariably defend, regardless of energy consequences, would need a separate owner decision; the present calls do not require that guarantee from trained brains.

### 3. Nearby antithetic bodies are interacting opponents in an experiment, not matched counterfactuals

**Claim under review.** Proposal §10.3 treats other variants mainly as score noise; §10.4 expects fast signal from the small top head.

**Evidence.** `crates/cubarium-search/src/es/voxel/live/window.rs:228–275` assigns adjacent bodies opposite signs of the **same** perturbation, including a nearby newborn's assignment. `live/variants.rs:39–52` creates those shared perturbation directions. They interact in one world, rather than replaying the same animal from the same start. `live/trainer.rs:474–480` allocates pairs by population, and `trainer.rs:156–179` gives unrepresented candidates zero scores before forming the update. The earlier 8-world, 8-pair pilot already had training differences overlapping placement noise (`design/training-plan-2026-10-08.md` §1c).

**Concrete failure.** A mother's plus perturbation is commonly paired with a nearby relative's minus perturbation. Its relative's return depends on both policies. The relative's perturbation is correlated with the mother's search direction, so its contribution is not simply independent zero-mean noise. Cooperation, interference and competition can enter with the wrong sign for the desired focal-policy improvement. For two equal-weight relatives, even the raw same-world difference is `(1 − r) × (e_plus − e_minus)`; their common benefit cancels before any z-scoring. Different ages, reserves and roles further confound plus/minus comparisons.

The counts are also restrictive. With the pilot's eight worlds and eight pairs, two loftstriders per world give only one body per candidate; four hounds give two. Forty to ninety frondgrazers give roughly twenty to forty-five bodies per candidate, but correlated neighbours are not that many independent worlds. At the proposed family encounter rate, eight ten-minute windows contain only about 0.7–1.3 family engagements in total; at one kill per ten hours, about 0.13 loftstrider kills. A few thousand top weights do not make those observations sufficient. Small ES perturbations can also leave every argmax unchanged, especially behind hold/margin gates.

**Proposed change.** For the first decisive family/pack learning experiment, use matched episode copies: perturb a focal body plus/minus while holding partners, opponents and starting state fixed, then rotate the focal role and sign. Other bodies can retain their own variants, but must have the same ones in both copies. This preserves topic 2's per-body choice and measures its social consequences causally. The already-approved shared-family fallback perturbs the whole family together, with plus/minus families in separate matched copies. Begin with frozen opposing policies, then broaden opponents and co-training after signal is demonstrated.

Retain the sigma-zero check as a placement diagnostic, but add nonzero-sigma checks: repeated directional differences on matched starts, decision-change frequency, held-out energy improvement, and candidate coverage. Never treat a missing sign as a measured zero return. Report independent families/episodes and event counts. Replace “the signal comes quickly” with a hypothesis, and make the staged encounters the primary early evidence for these scarce lineages. The frondgrazer trial can test the menu machinery sooner; it cannot validate pack credit assignment.

**Owner calls.** Estimator and evidence changes within topics 2–4. Invoking the documented shared-family fallback does not reopen topic 2. Neither reward shaping nor a larger population in the live artwork is required.

### 4. The subject pointer and observation schema cannot yet express the proposed decisions

**Claim under review.** Proposal §10.1 gives the top head a pointer over **kin and threat** slots; §2.2 promises subjects for calves, quarry and places.

**Evidence.** `crates/cubarium-voxel-fauna/src/perception.rs:345–355` classifies a hound's quarry as `PREY`, not `THREAT`. `observe_v3.rs:210–219` emits only the best kin, best threat and best prey; increasing `kin_tracks` does not by itself add four brain slots. The current slot encoder (`observe_v3.rs:113–130`) omits size, lineage, heading, elevation and modality, and merges resting and asleep into one bit. The raw track has many of those fields (`perception.rs:107–145`). Proposal §2.1 adds stance and lame bits but does not fully specify the missing multi-slot contract.

**Concrete failure.** A hound cannot point at its quarry through the specified head. A brain cannot choose a calf rather than an adult or estimate an interception trajectory from the present encoded slots. The top can select neither a rest site nor an exploration place. Distance-ranked slots reorder as animals cross, so “hold slot 1” can silently change which calf is guarded. Four nearest kin can also exclude the mother in a six-member family. A call reduced to one bearing per kind cannot identify “my calf,” distinguish two callers, or mark a subject missing from the ordinary slot set.

**Proposed change.** Define a typed, bounded subject contract: `none`, sensed/remembered body, and sensed/remembered place. Include prey candidates for hunters. Give each selectable body the same usable information the teacher reads: identity association, lineage/size, geometry, observed heading or velocity estimate, posture/stance, modality and freshness, and recognized family relation. Raw global IDs need not enter the network; retain the selected identity internally and re-mark its current slot each sample. Specify eviction, missing-subject expiry and a null pointer, with masks for invalid intent/subject combinations. A subject must not change merely because slot order changed.

Reserve or prioritize recognized mother/calf tracks within the bounded budget where justified, and define how a heard caller is represented without granting its current unseen position. Use subject-conditioned intent selection or masked subject logits so the two heads cannot choose a valid intent paired with an impossible subject. Verify slot permutations, crossing targets, death/loss of subject, and two simultaneous distress callers. This is workable with small menus; it requires more than adding one marker bit.

**Owner calls.** Spec completion for topics 1, 3, 4 and 7; no changed call.

### 5. Teacher forcing needs a causal recurrent schedule and evidence that each frozen skill actually works

**Claim under review.** Proposal §10.1–10.2 puts the top head over the bottom GRU state while feeding the held intent into that GRU; the bottom is then frozen for stage 1.

**Evidence.** Proposal §2.2 says the bottom runs every tick, whereas §10.1 says every controller sample. Current sampling is at the manifest cadence (`crates/cubarium-voxel-fauna/src/step.rs:1552–1567`), described as 4 Hz in survey §1. The existing imitation implementation has one recurrent computation followed by output heads (`crates/cubarium-search/src/es/voxel/imitate.rs:862–907`); its tap currently writes option label zero everywhere (`imitate.rs:384–419`). It is useful infrastructure, not an existing two-stage learner.

**Concrete failure.** If the teacher's newly selected intent is inserted into the bottom input before predicting that same intent at the top, the label leaks through the hidden state. Excellent cross-entropy then means “copy the answer already supplied,” not “choose when to switch.” If runtime instead supplies the previous intent, training and execution disagree. Updating the GRU twice on decision samples to resolve the circularity silently changes the recurrent clock.

There is a second, independent risk: cloning only the teacher's chosen mode in each situation does not show how another mode behaves there. A frozen bottom can ignore the intent and reconstruct the teacher's actions from the senses, or fail when the top selects a legitimate mode earlier or later. Freezing it prevents weight damage, not execution failure. The two spare intents have no teacher data at all. A hidden state trained only for local steering may also discard information needed for strategic choice.

**Proposed change.** Write the temporal recurrence explicitly. One sound simple version uses the previously held intent for the single GRU update, predicts the next intent from that state, and applies its conditioning at the next defined action boundary; another separates current observation encoding from the conditioned motor computation. Choose and clone the exact same ordering, including subject updates, hold age and reset behaviour. Keep the actual controller cadence unless a deliberate faster motor loop is specified and measured.

Train the strategic loss into the shared representation during cloning, and consider a direct observation path to the top if its required information is otherwise lost. Collect permitted alternative-mode and transition examples by invoking teacher skills in staged situations, then perform short closed-loop checks with the learner choosing intents and the teacher supplying labels on states it actually reaches. Assess rescue/guard/retreat/intercept separately; average action error on mostly grazing samples is insufficient. Mask unnamed spare intents until the later self-invented-mode study. Stage 1 should freeze all bottom parameters and optimizer updates explicitly; today's variant builder perturbs the entire vector (`live/variants.rs:39–52`).

**Owner calls.** Spec correction preserving topic 3's **two learned layers, named modes and staged training**. No recommendation to return to scripted runtime skills.

### 6. The reactive counter bypasses the proposed kick's facing and cooldown budget

**Claim under review.** Proposal §3.3 adds both an aimed guard kick and the existing reactive `Counter`, then argues that one adult cannot kick four attackers because it can face and kick only one at a time.

**Evidence.** `crates/cubarium-voxel-fauna/src/step.rs:3867–3871` creates a counter on contact, before dodge resolution at `step.rs:3919–3924`. `counter_answer` (`step.rs:3964–3979`) checks adulthood and distance to the incoming jaw, but not facing, sleep, windup or cooldown. It can answer each of several incoming bites. Moving it after dodge fixes only one of these issues.

**Concrete failure.** Four hounds biting different sides can each receive a counter in the same tick, including during an aimed kick's cooldown. An adult can become an omnidirectional damage source while supposedly exposing its flanks. Alternatively, an implementation that arbitrarily adds cooldown only to the aimed path leaves the reactive path dominant. Hound injury frequency and apparent encirclement value then measure the wrong mechanism.

**Proposed change.** Give the loftstrider one physical kick state machine and one shared recovery budget, whether the request originated from a deliberate action or a reactive response. Specify whether a landed bite requests a later kick or consumes an immediately available response, and apply the same eligible state, arc, cost, recovery and target rules. Do not require changing the lanternjaw's distinct counter unless separately intended. Resolve kick eligibility consistently when multiple bites arrive, and define whether attacks already in flight still land after a lethal kick. Test rear approaches, two simultaneous contacts, contact during recovery, dodge, and a target that moves during windup.

The damage arithmetic itself is reasonable: the wide preset's size ratio is approximately 1.92, so damage 0.18 gives about 0.346 wound on a full-grown hound; size means `body / mass_scale`, not body length (`body.rs:2944–2948`). Wounds accumulate and kill at one (`step.rs:4018–4036`), with healing between hits (`step.rs:1266–1288`). “Three kicks” is therefore an approximate grown-body tuning target, not a universal law test. Retain topic 6's intended lethality rather than weakening damage to hide duplicate counters.

**Owner calls.** Spec correction supporting topic 6, **kicks wound**, and topic 8's rare hound deaths. No changed call.

### 7. The brain cannot currently choose physiological rest, and normal sensing cannot wake a deep sleeper

**Claim under review.** Proposal §0 topic 5 and §3.4 assign when/where to rest to the brain and light/deep sleep to the body; §2.2 adds only kick and call actions.

**Evidence.** `crates/cubarium-voxel-fauna/src/controller.rs:37–67` has movement/feed and body actions but no rest request. Zero movement is not a physiological sleep command. `step.rs:899–995` independently chooses circadian bouts and sets `State::Asleep` before controller execution; asleep bodies are then excluded from sampling (`step.rs:1560–1566`) and sensing (`perception.rs:712–720`). The existing wake path is separate (`rhythm.rs:371–408`); it currently has no terrain-ray occlusion in its calculation.

**Concrete failure.** A brain can select “guard” yet be put to sleep by the old circadian code, or select “rest” without obtaining the new physiology. Calls delivered only in the normal remote sense pass never reach deep sleepers. Checking whether a relative is awake from stale tracks can admit deep sleep after that relative has already fallen asleep. Independent random bouts do not guarantee a sentinel: several animals can all see another awake and start deep sleep before observing the change. With light-rest relatives counting as awake, everyone can be lying down even when the rule works exactly as written.

**Proposed change.** Specify a rest request through the shared action/body interface, with the body enforcing its physiological eligibility and light/deep transitions. If intent itself is that request, say so explicitly and acknowledge that it has a direct body effect beyond conditioning the learned motor output. Apply it identically to brain and heuristic bodies. Retain a sleep-compatible hearing path for footfalls and calls, with the intended occlusion semantics. Use recent **visual** evidence of awake kin, not an unseen animal's true state or a generic refreshed track timestamp. Require defined light-rest intervals between deep bouts and report the joint all-deep fraction; describe sentinel presence probabilistically, not as a guarantee.

The subject and rest-state contracts must land before package 2 is described as complete. A dedicated wake check should include a snort/bleat as well as a rushing hound. The unchanged frondgrazer input shape also does not prove unchanged alarm behaviour when a motion-based signal becomes a buffered event.

**Owner calls.** Spec completion of topic 5, **body fixes plus sleeps lighter**. No shared watch rota is required. Local signals could in principle support a rota, so §3.4's claim that any rota requires a group mind is too strong, but there is no reason to reopen Wrysk's selected approach.

### 8. Guarding a calf and directing a strike need physical targeting contracts, not just steering

**Claim under review.** Proposal §3 puts an adult between calf and hound; §3.3 compares 1.3 m kick reach with 0.84 m bite reach; §4.2 makes strikes go to the selected target by deciding when to hold feed.

**Evidence.** `crates/cubarium-voxel-fauna/src/step.rs:3654–3715` selects the nearest eligible prey in a jaw capsule. Opening the feed gate when the intended target is present does not change that selection. Contact includes jaw offset, a sweep and **both bodies' footprint radii** (`step.rs:3528–3532`, `3552–3571`), so the two bare reach numbers do not establish which attack can land first. The specific interposed-adult protection is confined to hunkered HERD prey (`vigil.rs:955–995`), which does not include loftstriders.

**Concrete failure.** The selected calf and a closer adult can both occupy the capsule; feed is allowed because the calf is there, yet the nearest adult is struck. Conversely, calf damage on this path is not blocked merely because a guard point lies in front of it. An adult may look protective while the attack geometry still reaches through its footprint. A centre-based kick range and surface-based bite range can even let a hound bite from outside the advertised kick zone.

**Proposed change.** Pass the selected attack identity to strike creation and retain it through windup and contact. Define the opportunistic-juvenile exception as an explicit sensed target substitution, including how the brain can express it. Specify kick origin, body-size scaling, front arc, height band, terrain obstruction, tracking during windup and the hit shape in the same geometric convention as bites. Decide how an interposed adult obstructs a lunge to a calf: physical interception or an explicit footprint-cover mechanism, not automatic invulnerability. Check two prey in one capsule, a calf behind an adult, opposite sides of a ledge, and ring-seam geometry.

Also decide what replaces `window_body`: it currently gates **physical strike eligibility** as well as target choice (`step.rs:3679–3688`; `pack.rs:80–99`). Replacing it with sensed pack size everywhere would make a bite's legality depend on whether the attacker noticed a helper. Separate sensed risk estimates from the actual damage/contact mechanism.

**Owner calls.** Spec completion of topics 1, 6 and 7, preserving local decisions and wounded defence. No owner call demands a particular new cover formula.

### 9. Calls and retained pack bookkeeping can still smuggle in a shared tactical mind

**Claim under review.** Proposal §2.1 lets a hound adopt a call's subject if it senses either subject or caller; §2.4 retains the feast record as nontactical bookkeeping; §7 tests that some pack member senses the prey.

**Evidence.** Hearing a caller establishes its approximate source, not which of several unseen prey it is chasing. The proposed call contains an exact subject ID. Today's Social block reveals a target's true position **and** the shared feast's location (`crates/cubarium-voxel-fauna/src/observe_v3.rs:370–383`). The heuristic actively travels to `pack.feast` (`pack.rs:1880–1894`), and shared hunt start/end processing creates that feast (`pack.rs:1662–1701`). Removing only the target pose leak leaves another behavioural use of globally shared location.

**Concrete failure.** A hound hearing a cue through rock can acquire an exact unseen target, or learn where to feed from a bookkeeping record it never sensed. The proposed test passes as long as a distant packmate senses the quarry, even though the focal hound has no sensory route to that information. Two bleats of the same kind can collapse to one bearing and summon a mother to the wrong calf. If calls are handled only through the current nearest-animal candidate cap, nearby clutter can exclude a louder, more distant caller (`perception.rs:601–628`).

**Proposed change.** Separate event payloads used internally for diagnostics from information delivered to the receiver. An unidentified hunt cue attracts attention to the caller; adopting an exact quarry requires an existing track or a locally observable, unambiguous association such as visible pursuit direction. Define voice recognition for family identity if used. Calls need bounded source records, age, deduplication/refractory handling and receiver eligibility; the call's own range must govern candidate collection.

Keep global feast/kill facts for accounting, but use each animal's sensed carcasses, remembered visits and received signals to choose where it goes. Audit remaining global reads of membership, hunger, target progress, shunning and roles. Replace the pack-wide sensory assertion with a focal-animal check: changing unseen bodies while preserving its full percept/call history must not change its decision. Include stale information and occluded-caller cases. Physical contact and reward accounting can still read true world state.

**Owner calls.** Enforces topic 1, **signals only**; no changed call.

### 10. Local rules can encircle, but isolation, role stability and interception are still underspecified

**Claim under review.** Proposal §2.4 and §4 infer a broad tactical repertoire from two-rule encirclement, local option scores and suppression of already occupied roles.

**Evidence.** Ethology §5.2 explicitly says the Escobedo model starts with **one prey already isolated**. Its `da` is distance from prey at which wolf–wolf repulsion is activated, not a required minimum distance between wolves. Proposal §7 nevertheless proposes checking the inter-hound gap against `da`. Proposal §4.2 uses “a packmate already driving” and “kick cooling” without specifying how either is inferred from the listed stance bits.

**Concrete failure.** Four hounds with similar views can all choose cut-off, then all abandon it when they see one another there. Adults can all choose the same guard point and face the nearest hound, leaving the opposite approach open. A calf whose mother-first route crosses a hunter may ignore a nearer safe adult. Constant-velocity lead points can be behind rock, beyond a ledge or obsolete after a turn. No packmate visible does not mean no packmate exists, and an unseen kick is not evidence of a fresh cooldown. None of this is resolved by replacing a shared record with four similar score functions.

**Proposed change.** Keep the local architecture, but specify local memory and geometric skills: target commitment and expiry; a consistent tie-break or persistent approach-side preference; occupied-sector avoidance; safe and reachable calf/defender points; bounded lead prediction from observed motion; and replanning when blocked. Treat mother's identity as a preference subject to immediate reachability, not an unconditional route. Infer another animal's role from its recent movement relative to the target, and infer kick recovery only from an observed kick with a timestamp; otherwise retain uncertainty. Test feasible interception and failed interception separately.

Use the two-rule model to justify an encirclement prototype, not a promise of all tactics. Replace the `da` gap assertion with checks of the actual forces, non-overlap and bounded approach under a suitable fixture. Add asymmetric, occluded and confined scenarios, not only symmetric open-ground circles. The heuristic may explicitly implement plausible cut-off and rescue skills; their coordination must still use local evidence.

**Owner calls.** Spec completion for topics 1 and 7. No need for a shared planner or behavioural reward.

### 11. A fixed raw-logit margin is not equivalent to a margin on heuristic utility

**Claim under review.** Proposal §2.2 and §10.1 apply the same hold and switch-margin mechanism to heuristic scores and learned logits; §7 requires an option always to hold for at least `hold_s`.

**Evidence.** The imitation loss is categorical cross-entropy (`crates/cubarium-search/src/es/voxel/imitate.rs:904–906`). Multiplying all logits by a positive factor leaves their argmax unchanged but changes whether their differences cross a fixed margin. Cross-entropy and ES can alter that scale without improving strategic discrimination. The held intent and time are themselves inputs (§10.1).

**Concrete failure.** Early underconfident logits can trap an animal in rest or search. Later high-confidence logits can defeat the intended margin. ES can appear to improve intent choice merely by changing output scale. Applying a second hold to labels that already embody the teacher's hysteresis can delay a switch twice. A strict minimum hold can keep a calf walking toward a now-invalid subject or keep an adult grazing through a lethal approach.

**Proposed change.** Keep persistence but specify its semantics independently of score units. Options include a calibrated, fixed-temperature probability margin or a bounded normalized utility output, with a separately tuned brain margin. Define whether imitation targets the pre-arbitration proposal or the post-arbitration held choice and train the matching runtime process. Subject death, loss of eligibility and physical emergencies need explicit termination/reconsideration rules; reconsideration can still let the learned top choose what to do. Read out hold durations, reasons for termination, switch rates and saturation. Test invariance properties and configured mechanism boundaries, not today's margin values.

**Owner calls.** Spec correction within topic 3's **pick-and-hold named intents**. It changes no recorded owner choice about neural logit calibration.

### 12. The harness can show mechanisms, but its proposed exposure does not establish the annual balance claims

**Claim under review.** Proposal §6 budgets a stable family and rare feast; §7 says 8 × 6 hours is enough to read a fall from eleven deaths to about two; §10.4 scores encounters of at most five minutes.

**Evidence.** At one kill per ten-hour world-year, 48 world-hours imply **4.8 expected kills**, assuming the family and pack remain present, not two. Adult and hound deaths once in several years are rarer still. The birth settings allow one seasonal conception per adult (`config/desktop/terrarium-wide.toml:149–157`), but conception also requires condition/availability and gestation can fail when reserve cannot pay (`crates/cubarium-voxel-fauna/src/step.rs:4904–4955`). Surplus young disperse (`step.rs:1370–1431`); this is another population loss, not free extra replacement capacity. Proposal §6 mixes per-chase success with per-engagement success, including standoffs that need never become chases.

**Concrete failure.** Fewer kills after early family extinction can look like successful defence. A flat forced encounter estimates conditional success, not encounter frequency or annual replacement. A short episode ending at kill gives hunters no meal credit; one ending at give-up can spare a failing policy the rest of the common evaluation horizon. Surviving wounded animals can carry unmeasured later healing/death costs. Kill-count diet share also does not establish food dependence: a small number of large carcasses can dominate energy. A family-only training arena may teach perpetual guard or repeated testing that is costly in a normal world.

**Proposed change.** Keep the rare-feast target as a target, and report a feasible bracket rather than declaring it reachable from the current arithmetic. Separate encounters, tests, chases, captures and kills; record exposure while both populations exist, calf availability, births, ageing, dispersal and unmet feeding needs. Couple attack rate to prey density and alternate food availability. Report consumed/assimilated energy by prey as well as kills. Preserve prey trajectories and lineage losses beside total fitness; this is essential for the prey-switching worry.

Use matched scenario starts and signs, terrain/occlusion/night variants, multiple calf ages and separations, wounds/stamina variation, and mixed brain/heuristic compositions. Keep a common evaluation duration including a feasible post-engagement feeding/recovery portion, and label unresolved encounters as censored rather than give-ups. Include ordinary non-encounter feeding/rest situations in training or held-out checks. Healing matters on the stated short horizon (`step.rs:1266–1288`), so report remaining wounds as a diagnostic without inventing a wound penalty in reward.

Stage the wide check with an early liveness/mechanism checkpoint, then continue only for a stated event or slow-clock question. Use saved starts at relevant seasonal/age stages for reproduction evidence. Forty-eight hours can identify a large failure and provide a rough kill-rate estimate; it cannot settle all rare-event targets. The proposed knobs are relevant, but whether their ranges jointly meet topic 8 remains an empirical question. Literature supports the mechanisms, not these rates: ethology marks kick lethality unmeasured, the moose comparison observational, and the giraffe sleep site free of roaming lions.

**Owner calls.** Measurement/spec changes preserving topics 6 and 8, including prey switching as a watch item rather than a reward term or a gate.

### 13. Package interfaces must precede parallel lineage work; the current coupling list is incomplete

**Claim under review.** Proposal §8 says packages 3, 4 and 6 touch different files and can proceed independently after package 2; it treats intent timers as disposable session state and expects negligible added tick cost.

**Evidence.** All three touch the serial controller/write path, action representation, observation encoding and labels. `crates/cubarium-voxel-fauna/src/step.rs:1783–1789` distinguishes heuristic reflexes and records the last override; `step.rs:2171–2194` records that final `Steer` and executed action. It is not a persistent high-level intent. Removing pack targets also affects grazer pry gating (`vigil.rs:1486–1489`), kill/feast bookkeeping (`pack.rs:1662–1701`) and feeding travel (`pack.rs:1880–1894`). The proposed loftstrider all-age herding has another trap: dispersal currently requires `phys.herding.young_only` (`step.rs:1408–1412`).

The survey predates current changes: `Animal::mother` exists (`lib.rs:1103–1108`), schema is now 80 (`snapshot.rs:173–178`), and `nurse::mother_pull` only applies to nursing-dependent young (`nurse.rs:115–142`, `228–243`). It cannot be reused unchanged for loftstriders or post-weaning seedporters. Proposal §1 repeats the old “no parent id” claim, contradicting its own §3.5; label that account as historical. Proposal §5 postpones a seedporter alarm call, while §3.5/package 5 already require a post-weaning youngster's distress bleat and maternal response.

**Concrete failure.** A shared `Steer` field loses intent when a physical override wins, corrupting teacher labels and hold age. A new pack controller can disable prying on the frondgrazer and alter prey switching before any intended balance tuning. Turning off `young_only` to make adults herd can disable dispersal and invalidate the family budget. The calf-follow helper can silently do nothing for the intended lineage/age. Independent packages can disagree about the same action and save schemas. A training checkpoint that discards intent state and then cold-starts the recurrent controller is not equivalent to continuing the engagement.

**Proposed change.** After the baseline harness, define the common contracts before splitting lineage work: observations/call records; selected intent and subject distinct from executed steering source; body actions and overrides; local tactical memory; reward cohort/evaluation unit; and save/reset semantics. Record chosen intent, nominated subject, executed actions and override reason separately in the tap. Place physiological constraints after the requested actions consistently, while keeping heuristic-only decisions out of the brain path. Port pry participation and attack accounting to controller-independent physical events or local attack state. Decouple dispersal eligibility from the old young-only herding switch. Extract a sensed mother-follow primitive independent of nursing, and include the required seedporter distress behaviour explicitly.

Package 6 can build architecture against provisional menus, but full loftstrider/Veilhound cloning depends on 3–4, and exploration labels depend on 5. Package 7 needs those labels **and** the revised estimator/episode contract; §10 currently underspecifies work called “specified.” Package 2 alone does not yet ship bunching from a call unless it also includes the reaction skill owned by package 3.

For saves, distinguish harmless disappearance of an old call from resetting a kick recovery or training memory. State changes, including serialized tracks, need the applicable schema change; pure knob additions do not. Define fresh-controller burn-in or restored recurrent/intent state for matched evaluation starts. Verify controller attachment after load, with a small mechanism check rather than a migration suite. Preserve plain-lineage manifests explicitly if their ongoing training must remain compatible. Measure full mixed-world cost after both expanded sensing and both brain layers land: four kin rays, call queries, routing and any faster motor rate dominate a cheap top head, and measuring package 2 cannot bound all of that.

**Owner calls.** Package and integration corrections only; topics 1–7 are retained. The normal display stays free of diagnostics, all ecological numbers remain serde knobs with registry rows, and verification stays mechanism-based.

## Checked and found sound

- A learned named-intent layer conditioning a learned recurrent motor policy is a reasonable starting architecture; the issues are its temporal/data contracts, not the two-layer choice itself.
- Cloning before ES and freezing the motor weights for an initial strategic experiment are sensible staging choices once closed-loop skill competence is established.
- Reward bookkeeping may use true maternal/pack relationships without giving those facts or unseen positions to the controller (proposal §10.3 versus §2.1).
- The existing energy meter already preserves death losses and counts digestible stomach when enabled; kin reward can build on it without defence, kill or coordination bonuses (`live/window.rs:118–140`, `184–196`).
- Local sensory coordination can produce encirclement without a shared tactical record; the cited model supports that narrower claim (ethology §5.2).
- The current loftstrider flight asymmetry, blind sleep and rushing-hound wake gap are real problems, consistent with the survey and inspected controller/sleep paths.
- The mother record is now real, and the existing follow primitive uses the calf's own fresh kin track rather than the mother's true position (`nurse.rs:237–246`).
- Kick damage near one-third of a full-grown hound is consistent with the current size and wound formulas; it remains a tunable design choice, not a biological measurement.
- Range-limited calls and identical physical action semantics can support mixed brain/heuristic worlds once the residual shared-state and prying dependencies are removed.
- Rest-site scent avoidance, bounded sleep bouts, calf rescue and predation risk are relevant mechanisms to investigate before numerical balance tuning.
- The family-only engagement harness is a useful first mechanism probe; the wide-world prey-switching check is necessary because short energy returns cannot certify long-run ecology.
- Keeping invented modes, look controls and player commands for later avoids expanding the first learning experiment unnecessarily.
- The proposal correctly supersedes training-plan item 23 only within Wrysk's newer calls; the remaining proposal and this review are not canonical decisions.

## Open questions

- Which exact scalar and accompanying health vector will decide “beats the heuristic”? Own/kin training energy and whole-world ecology fitness are different readouts; the proposal should identify the comparison configuration and common starts.
- What body references and starting stores will the social-lineage experiments use? These determine whether rescuing a small calf is worth its immediate energy cost under the agreed reward, without any behavioural bonus.
- Are overlapping maternal and packmate weights additive or exclusive, and how are birth, dispersal and a change of pack membership treated? This needs a single explicit accounting rule.
- Does the intended family mean a maternal pedigree, nearby conspecifics, or both? The heuristic can help unrelated calves, but the proposed kin objective does not promise to retain that choice after training.
- What information does a hunter actually observe when a target kicks, tires, changes heading or calls? The visible event/track contract is not yet detailed enough to settle tactic feasibility or neural input size.
- Should a body be able to move while winding up a kick, and can it turn its arc during windup? Together with actual attack geometry and turn rate, this determines whether flank attacks have a useful opening.
- Do realistic wide-world starts provide enough groves, alternate prey, rest sites and calf separations for the annual target? Code and arithmetic alone cannot establish this; no simulation was authorized for this review.
- How long must aftermath evaluation run for meals, wounds and calf survival to affect energy reliably? A fixed five-minute ceiling may suffice for some starts and fail for others; that remains to be measured.
