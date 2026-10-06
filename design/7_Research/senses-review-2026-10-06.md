---
design_status: exploration
last_reviewed: 2026-10-06
decision_refs: []
---
# Senses pass: review of Astra's design, and my suggestions

Reviewer: Opus worker, 2026-10-06. Read: `astra-senses.last.md`, `astra-senses.txt`, the species
dossiers, the `veilhound-search` branch tip `8a4908c6` (`perception.rs`). I did not audit Astra's
citations. I checked four facts in the code because the review rests on them; they are marked
**fact** with a path.

**Bottom line.** Astra's per-species senses are sound, and most of its ranges are good starting knobs.
I disagree in four places:

1. **Activity times.** The still lure can never meet a bellwing today, because the two animals are
   never awake at the same time. Astra noticed the mismatch but didn't follow it through.
2. **Smell.** It should be a field that animals leave behind, not a per-pair distance test. A field
   gives density-dependent hunting, the "boom answering" Wrysk wants, without extra machinery.
3. **Simpler machinery.** Use one ray per pair for all three senses, and replace "confidence"
   with "last-known position and age".
4. **Vigilance.** It should feed the strike's existing awareness term, not add a second layer of
   protection on top of it.

---

## 1. Per species

| Species | Astra | Mine | Why |
|---|---|---|---|
| Littershredder | eyeless; litter chemistry over 1–2 m; tremor about 1 m (2 m for heavy steps); freeze or withdraw | **agree** | Matches the dossier ("blind shredder", downward feelers). Freezing instead of fleeing keeps it on the litter it is there to clear. |
| Capgnawer | fungal chemistry 2–3 m; coarse sight 1 m; cyan glow attraction that needs the fungal scent to agree | **agree, one change**: let weak cyan attraction act *without* scent when the animal is hungry (see §2, lure on capgnawers) | Astra calls lure attraction a risk. I think it is the lanternjaw's best tool against a capgnawer boom. |
| Ripple-snail | wet-connected film chemistry 0.5–1 m; shadow and looming sight 0.5 m; retracts | **agree** | A snail that runs would look wrong. Retracting should make it slower to eat, not immune: Wrysk said lanternjaws eat snails. |
| Frondgrazer | moving bodies at 8 m by day and 4 m at night; static bodies at 5 m and 2 m; 300° field; hears walkers at 6 m and runners at 8 m; smells at 3–4 m | **agree on ranges**. **Change:** alarm comes out of hearing (a bolting neighbour is loud), not from a separate alarm cue. Add a visible scan pause. | The eye stalks support a wide field. Alarm-by-hearing costs nothing extra and gives herd bolts the viewer can see. |
| Loftstrider | 360° from an eye ring at 1.5 m; 12 m sight; 8–10 m hearing | **agree** | Six eyes means panoramic vision. Make sure adults standing their ground is not mistaken for sensory failure. |
| Bellwing | weak form vision 1.5 m; UV/blue brightness (flowers 4 m, lure 2–3 m); floral smell 4–6 m; no hearing | **simpler: no form vision at all.** One photoreceptor gives directional brightness, plus a "looming" reading when a shadow falls on it within about 1 m. Smell leads the search; brightness steers the approach. **Rule: a still light attracts, a moving light repels.** | This is closer to "no eyes" than a 1.5 m form vision. The still/moving rule is Wrysk's lure in one line. |
| Seedporter | 12 m by day and 3 m at night; 220° field with a sharp 90° centre; strong colour; hearing 6 m | **raise to 16 m by day**, the Veilhound's range, with the best recognition of still objects in the community. **Narrow to about 180°** with a forward binocular centre. | Primates have forward eyes: less field of view, more detail. The seedporter sits high, so 16 m is reachable over terrain, and "excellent vision" should not lose to the Veilhound's. |
| Lanternjaw | moving prey at 4 m by day and 3 m at night; still prey at 2 m; tremor 2–3 m; carrion smell 4–6 m; live prey 2–3 m; lure strongest when still | **agree on ranges. Change how it finds prey:** follow the small-prey scent field (§3). **Activity: see §2 and decision D1.** | Following a field drawn to dense prey makes the lanternjaw answer bug booms with no switching rule. That is Wrysk's opportunism. |
| Veilhound | keep the branch's values: 16 m, 240° field, 10 m hearing, 6 m smell; night 8–10 m; imperfect evidence | **agree. Add a stalk gait:** below the prey's hearing threshold while approaching, then a rush at commit distance. Share pack evidence only within hearing range of the animal that detected it. | Lions stalk, then rush. That makes the senses visible. The branch currently pools every awake adult's detections, which is telepathy (it matches Astra's finding). |

Notes:
- **Wolves and lions hunt at night**, but the frondgrazer is diurnal. A night-hunting Veilhound
  would kill sleeping grazers, which have no vigilance at all, so I recommend it stays diurnal for this
  pass (decision D4).
- **Where I'd go below Astra.** Astra's night ranges matter only for animals awake at night. Today
  that is the lanternjaw and the dark-active detritivores. Night values for diurnal species are
  real knobs, but they have nothing to act on yet, so don't spend test effort on them.

## 2. Interactions

**Fact: the lure and the bellwing never meet today.**
- Lanternjaw is `Nocturnal` and bellwing `Diurnal` (`crates/cubarium-voxel-fauna/src/body.rs:1225,1412`).
  The two rules are exact complements around `day_line` (`lib.rs:593-594`), so there is zero
  overlap.
- The preset gives the lanternjaw no `active` override. The detritivores (capgnawer, snail,
  shredder) are `DarkActive` (`config/desktop/terrarium.toml:65,117,333`).
- **Nothing in the sim reads the lure yet.** `founder_state.lure` costs energy (`step.rs:1499-1506`)
  and is drawn (`crates/cubarium/src/voxel/animal.rs:1279`), and that is all.
- Astra suggests "activity overlap, shaded daytime hunger, or twilight". Under the current rules,
  none of those happens.

**My recommendation: make the lanternjaw `DarkActive`**, like the prey it mostly eats.
- By day it hunts in caves, under overhangs and in deep shade, with its lure lit against the dark.
- A bellwing flying past by day sees a high-contrast still light, while sunlit flowers have to
  compete with daylight.
- At night bellwings roost, and a lanternjaw that finds one perched within reach eats it. That is
  Wrysk's "grounded or within reach".
- This makes cave mouths dangerous ground for bellwings, which the viewer can read. And it costs
  nothing new: the `DarkActive` machinery already exists (decision D1).

**Lure on capgnawers (new; Astra listed it as a risk).**
- The lure is cyan, and so are the glowcaps. A hungry capgnawer that sees a cyan light with no fungal
  scent makes a short visit.
- How many it catches rises with capgnawer density, which rises with glowcap stock. So the lure
  helps answer a fungal boom through the lanternjaw, and the lure becomes useful during the
  lanternjaw's own active hours.
- Bound it with the same failed-visit memory as the bellwing (decision D2).

**UV flowers.**
- I agree with Astra's rules: reflectance needs incident light; emission doesn't; a stand counts as
  a beacon only while it holds nectar; whole plants are never beacons; display colour stays out of it.
- Beacons are the nectar flowers (bloomcrown, latticevine), glowcaps (emissive) and lit lures
  (emissive).

**Bellwing attraction rule (simpler than Astra's scent × brightness weighting):**

| Step | What decides it |
|---|---|
| search heading | floral scent gradient |
| approach heading, within brightness range | the brightest *still* beacon |
| landing and sipping | scent present at the mouth, and taste |

- A lure or glowcap gets the approach and a hover, but never a landing. The bellwing then aborts and
  remembers the spot for a configured time.
- The lanternjaw strikes during that hover if the bellwing is in reach.
- **Fact:** the current eligibility rule excludes flyers that aren't perched or grounded
  (`predation.rs:108-115`). I agree with Astra: let the strike capsule's geometry decide for a low
  hover, not the mobility mode.

**Vigilance must feed the existing dodge, not stack on it.**
- **Fact:** the dodge chance is already `agility × awareness(state) × aspect × condition`
  (`predation.rs:149-168`). Strike success is partly a dice roll that knows nothing about senses.
- If vigilance also makes prey flee earlier, the Veilhound pays twice, and the frondgrazer boom it
  is meant to answer comes back.
- **Mine:** awareness = full if the prey has a fresh percept of the striker, otherwise the asleep/unaware
  value. Senses then *are* the awareness, and the dodge stops guessing (decision D5).

**Destabilisation risks.** I agree with all of Astra's: omniscience, permanent alarm, lure traps,
plant over-consumption, alternative prey crashing. I'd add:
- **Alarm cascades.** Hearing-driven bolts could run as a chain reaction. Prey should flee only from
  a loud *approaching* source or a seen threat. A bolting neighbour causes orienting and a pause,
  not flight. Flight needs hysteresis.
- **Changing tremor changes egg hatching.** The lanternjaw egg bank hatches on a prey footfall
  count (the P1 decisions). Keep that counter on its own emission rule, or check it explicitly after
  the tremor change.
- **The stalk asymmetry is structural.** If stalk speed is below the grazer's hearing threshold and
  Veilhound sight is longer than grazer sight, every stalk succeeds until sight range. Both
  thresholds must stay knobs, and the gap is what decides hunt success. Measure it with the
  first-detection funnel before touching kill rates.
- **Glowcaps as a trap.** Glowcaps in daytime shade draw bellwings to the same caves as `DarkActive`
  lanternjaws. That is the intended edge, but count failed-visit flight cost as Astra says.
- **Refuges.** With smell through rock, 6 m covers the whole ring depth (6 m), so terraces stop being
  refuges. Terrain has to attenuate every sense, not only sight (§3).

**Interactions Astra didn't raise, for later:**
- **Seedporter sentinel calls** that frondgrazers overhear. This creates a "landscape of fear"
  around groves, which stabilises predator–prey cycles (decision D6, later).
- **Veilhounds pushing lanternjaws off carcasses** (already decided). Lanternjaw smell of carrion
  plus Veilhound hearing makes kill sites a visible gathering point.

## 3. Mechanism

Astra's shared evidence sampler is the right idea, but it is over-specified for a first pass:
confidence, class confidence, foliage attenuation, an air-connectivity grid, per-source odour
kernels. I'd split by how each sense scales.

| Sense | Form | Cost at 150 bodies, staggered at 4 Hz |
|---|---|---|
| sight, hearing, tremor | **pairwise** from spatial bins rebuilt each tick, with at most N candidates per observer (nearest first, prey/threat/kin only) | ≤150×8 rays × ~128 cells ÷ 5 ticks ≈ 30k cell visits per tick; bounded, unlike Astra's "up to ~1M" |
| smell (animals, carrion) | **surface scent field**, a few channels (small-prey, grazer, hunter, carrion) on the existing surface-field machinery (detritus/fungal/film); deposit ∝ body mass, decay is a knob | one grid update per channel per tick; the receptor samples level and gradient |
| smell (food) | the existing detritus/fungal/film fields, plus a nectar field | already paid for |
| brightness | **beacon list** (nectar stands, glowcaps, lit lures), indexed, rebuilt on the plant clock | small n × observers with brightness receptors |

Key simplifications:

1. **One terrain ray per pair serves all three pairwise senses.**
   - Clear: sight allowed, hearing and tremor at full strength.
   - Blocked: no sight; hearing and tremor scaled by an occlusion knob.
   - No foliage attenuation yet. The current failure is hunters not finding prey, not prey lacking
     cover. Foliage comes later, if the funnel shows grassland is a death zone.
2. **Scent field, not pairwise smell.** Smell gives a patch and a gradient, never an individual,
   which is Astra's own requirement and comes for free here. Concentration grows with density, so a
   predator following it is drawn to booms: opportunism with no switching rule.
   - It can't cross solid terrain, if deposits sit on surfaces and spread only between connected
     surface cells.
   - It costs O(cells), not O(pairs).
   - No wind, agreeing with Astra.
3. **Hearing.** Emission is speed × size (knob exponents), plus explicit calls. It reads as class
   "unknown, large/small" unless sight confirms the class. Classification comes only from sight or
   from the scent channel.
4. **Memory: last-known position plus age, no confidence float.**
   - A target's position updates only on a sensing tick where it was sensed.
   - Pursuit walks to the last-known point and gives up after a timeout knob.
   - This is the anti-omniscience rule, and it is deterministic and testable.
5. **3D distance for range** (agreeing with Astra; the branch gates on horizontal `d`). All senses
   that detect report as a bitmask, not the first one that succeeds.
6. **The percept record** per observer, fixed size:
   - per class (prey/threat/kin/food/beacon/unknown), the strongest item: bearing, elevation,
     estimated distance, modality mask, age;
   - scent levels and gradients;
   - tremor left/right;
   - light: ambient level, looming, brightest still beacon.

   Heuristics read only this. Later, the neural encoding is a pure function of it.
7. **Route cost.** Agree with Astra that the approach's whole-component distance field, rerun on
   every retarget, will cost more than sensing. Cache it per pack per target cell.

On staggering: 4 Hz by `id mod 5` for remote senses, with contact, tremor and flight at tick rate.
I agree with Astra.

On brains: I agree they wait. **Disagreement on timing:** Astra brings in the full neural layout in
its step 2. I'd define the percept record there, but write the brain encoding only after the teachers
behave well on it (step 5 below). That avoids churning the schema while the record's channels are
still moving.

## 4. My suggestions beyond Astra

1. **`DarkActive` lanternjaw** (D1): the only way the lure meets bellwings, and it matches its prey.
2. **Senses feed the dodge's awareness** (D5): one source of truth for prey being "aware".
3. **Scent field with density-proportional deposit.** Predators drift to booms by construction, so
   it serves the roles-before-numbers rule directly.
4. **Stalk-then-rush Veilhound gait,** plus a frondgrazer **scan pause** while cropping (its blind
   window). Together they are the visible duel of the pass.
5. **Emergent herd alarm** from hearing a bolting neighbour, with no alarm-call channel for grazers.
6. **Failed-visit memory** as a general percept feature (a spatial "no reward here" mark with decay),
   shared by bellwings at lures and glowcaps and capgnawers at lures. One mechanism bounds every trap.
7. **Pack sharing gated by hearing range** of the animal that detected the prey. That is honest and
   costs almost nothing, since packs stay close.
8. **Roost height as bellwing safety:** a perched bellwing within strike reach is fair game at night.
   That puts selection on roost choice, with no new rule.
9. **Funnel counters first:** detection → approach → chase → strike → dodge → kill, each by modality
   and by lineage pair. They belong in logs and census, never on screen. Astra asks for the same
   readouts; I'd build them in package S1 so every later package is measured on them.

## 5. Order of work and packages

0. **Veilhound branch lands** (owned by its worker). Before merging, fix:
   - range on 3D distance;
   - modality reporting as a mask;
   - replace the test pinning other lineages to 90° / no hearing / no smell (`pack_search_tests.rs:331`).
     The new test checks only the configured table.
1. **S1: perception core** (opus-high: crosses pack, hunt, step and senses).
   - Contents:
     - `SenseTable` rows for all nine species with the §1 values;
     - spatial bins and candidate cap;
     - one ray per pair shared by the three senses;
     - hearing emission;
     - scent field channels;
     - the percept record with last-known position and age;
     - pack sharing by hearing range;
     - funnel counters;
     - migrate pack.rs, hunt.rs patrol/carcass and the external flee to read the percept.
   - No new behaviours; the values for the other species are inert until S2/S3 read them.
2. **S2: ground responses** (opus-high, after S1).
   - Contents:
     - frondgrazer and loftstrider scan, orient, flee with hysteresis, and alarm by hearing;
     - Veilhound stalk gait;
     - awareness → dodge (D5);
     - freeze/withdraw for shredder and snail;
     - capgnawer fungal-plus-glow seeking;
     - seedporter fruit and threat use of the percept.
   - Files: ground controllers, pack.rs, predation.rs `dodge_chance`.
3. **S3: light and flight** (opus-high, after S1, **in parallel with S2**).
   - Contents:
     - beacon list from flora (nectar stands, glowcaps) and lit lures;
     - bellwing photoreceptor (brightness and looming) and the still-attracts / moving-repels rule;
     - failed-visit memory;
     - lanternjaw `DarkActive` (if D1) and lure-wait behaviour;
     - strike-capsule eligibility for low-hovering bellwings (predation.rs eligibility only).
   - Files: flight controller, hunt.rs ambush, the flora beacon export, `is_prey_of`.
   - S2 owns `dodge_chance` and S3 owns `is_prey_of`. Both touch predation.rs, so the parent merges S2
     first and S3 rebases onto it; those are disjoint functions.
4. **Parent check:** short staged paired runs on the funnel counters. These are Astra's §5
   readouts, measured against Wrysk's targets (persist, breed, bounded) and not against tuned numbers.
5. **S4: brain encoding** of the percept record, then teacher cloning. Wait for steps 1–4.

Tests: Astra's mechanism list is good and pins no tuned numbers. Add to it:
- a still lit lure vs a moving lit lure vs an unlit hunter, with the same bellwing;
- a scent field that does not cross a solid wall, and does cross a connected ledge;
- a prey's percept of the striker toggles the awareness term;
- the egg-bank footfall counter is unchanged by the tremor refactor.

Tests come from a separate opus-high authoring pass, per the global rules.

## 6. Decisions for Wrysk

| # | Question | Astra | My recommendation |
|---|---|---|---|
| D1 | Lanternjaw active hours. Nocturnal today, so it never meets a waking bellwing. | twilight, shade or hunger overlap (doesn't happen under the current rules) | **`DarkActive`**: hunts in caves and shade by day with its lure lit, everywhere at night. Cave mouths become dangerous for bellwings. |
| D2 | Does the lure also draw capgnawers (the cyan of the glowcaps)? | flagged as a risk | **Yes, weakly**, only when hungry and with no fungal scent near. The lanternjaw then answers capgnawer booms. |
| D3 | Bellwing vision | weak form vision, 1.5 m | **No form vision**: one light sense (brightness and looming shadow) plus smell. Closest to "no eyes". |
| D4 | Veilhound at night (wolves and lions hunt at night) | night range 8–10 m | **Stay diurnal this pass.** Night hunting of sleeping grazers would bypass vigilance. Revisit after the funnel. |
| D5 | The strike's "awareness" becomes "has the prey sensed the striker" | not raised | **Yes.** Visible escapes replace part of the dice roll. Keep the earlier 15–25 % hunt success as a target to measure, not a dice setting. |
| D6 | Seedporter sentinel calls that grazers overhear | not raised | **Later, after S2 is measured.** It adds a visible fear zone around groves. |

## Wrysk's choices (2026-10-06)

Wrysk, on the senses pass: "yeah go with your picks on all three". The three were:
- **D1. Lanternjaw hours:** active wherever it is dark. That means at night, and in caves and deep shade by day, with its lure lit. It meets bellwings at dusk and in shade.
- **D2. The lure draws capgnawers,** weakly: a hungry capgnawer with no fungal scent nearby (the lure is the glowcaps' cyan).
- **D4. The Veilhound stays a day hunter** for this pass.

Also from Wrysk, 2026-10-06, in his own words:
- **Veilhound:** "good eyesight and be able to see pretty far. also good hearing and smell. like wolves or lions".
- **Bellwing:** it has no eyes, "or at least not good vision… find flowers by smell… attracted to bright objects (which all the flowers should be, maybe in like a UV spectrum) and lanternjaw can take advantage of that, luring them pretty effectively if its still rather than patrolling". So D3, no form vision, follows from his own description.
- **Seedporter:** "excellent vision, just like monkeys and other primates".
