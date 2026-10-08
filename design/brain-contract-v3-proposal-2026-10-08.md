---
design_status: proposal
last_reviewed: 2026-10-08
decision_refs: []
---

# Brain contract v3: brains that can do what the animals now do

A proposal for Wrysk to decide from. It builds on the two diagnoses of 2026-10-08:
`design/7_Research/training-readiness-2026-10-08.md` (Opus, §Fact sheet, B-2, B-7) and
`design/7_Research/astra-training-readiness-2026-10-08.md` (Astra, §2 P1). Nothing here is
decided. Sizes, channel lists and counts are working estimates.

Abbreviations: `fauna/` = `crates/cubarium-voxel-fauna/src/`, `search/` =
`crates/cubarium-search/src/es/voxel/`. Line numbers are on `main` at 3eda1a9e.

## 0. Summary

- **Recommendation: a staged B.** Write one v3 contract for all nine lineages now, so
  centres are refused only once (predation §2.11). It would encode the percept into fixed
  slots and add the body's real actions. Brains keep only the body constraints.
- **Train in stages:** frondgrazer and littershredder first, then the other prey, then the
  seedporter, then the two predators.
- **One exception, which is Wrysk's call (Q2):** the Veilhound's pack tactic stays as
  callable options (architecture C) for its first round.
- **The safety layer (A) is not proposed as a training contract.** A brain trained under
  overriding reflexes never feels what happens when it fails to flee, so it can never take
  that job over.
- **The decisive design choice is Q3:** whether brains get bearings to places they cannot
  currently sense (shore, refuge tree, roost, den). Without them, cloning cannot carry over
  the reflexes that run path queries.

## 1. The nine lineages today

What applies to every lineage today:
- **Controller boundary.** A controller gets only the observation vector and its own memory
  (`fauna/controller.rs:1-10, 71-78`). It answers with `Response::Logits([f64; 3])` or with
  bounded actions (`:56-62`).
- **The fourth action.** The lure and the lift reach the body through the interim trait
  seams `lure()` and `lift()` (`:86-98`).
- **Reflex gating.** Each reflex runs only when the body's controller opts in, through
  `flees()`, `pack_tactic()`, `hunt_patrol()` or `rests_when_fed()` (`:104-140`). The GRU
  opts into none of them (`search/controller.rs:101-115`).
- **What every driven body gets, GRU included:**
  - no controller sampling while asleep (`fauna/step.rs:1164-1171`);
  - DROP (`:1540-1547`);
  - the sprint gate (`:2238`);
  - the eating-latch gate on bites and strikes (`:2350-2360`): a carnivore's `feed` is a
    strike when prey is in its capsule, refused unless `eating` is on.
- **What the observation lacks.** The vector carries none of these, though the body has
  them:
  - stomach (`fauna/lib.rs:1016`), latch (`:1022`) and wound (`:962`);
  - stamina (`FounderState::sprint_spent`/`winded`, `lib.rs:2043-2050`);
  - time of day (the WX4 note at `step.rs:1165`);
  - the percept record (`fauna/perception.rs:237-270`).

Senses come from `senses_of` (`fauna/body.rs:5972-6120`). Ground responses come from
`vigilance_of` and `hunker_of` (`fauna/vigil.rs:393-483`). The reflexes are in
`step.rs:1326-1547`. The founding counts on d72 are the base config's
(`config/desktop/terrarium-d72.toml:67-177`; code defaults `body.rs:1818, 1899, 2003`).

| Lineage | Heuristic perceives (range, file:line) | Heuristic + reflexes it uses (`step.rs`) | v2 brain today | Missing for a brain |
|---|---|---|---|---|
| **littershredder** (`Blind`, 8 founders) | Live table: tremor 2 m (`body.rs:5979`); `Chem(detritus)`, contact, wet, taste, light in the obs | `BlindForager` (`controller.rs:575-577`), `flees`=true (`:800`): freeze/withdraw on tremor (`vigil.rs:471-477`), shallows (1380), DROP | **Yes:** 23→GRU32→3 (`manifest.rs:539-546`), C1 centre (`cubarium-session/src/founders.rs:13-24`) | Tremor L/R, threat track, stomach/latch, daylight. Freeze is not an action: a GRU can only stop. No shallows |
| **capgnawer** (6) | Sight 1 m/0.5 m at night, 240°; tremor 1.5 m; cyan light 1.5 m (`body.rs:5984-5996`); `Chem(fungal)` | `BlindForager` (`controller.rs:586`), blind flee: turn away from tremor (`step.rs:1627-1633`), glow steering when hungry with no fungal scent (1517-1530), shallows | None: refused (`search/controller.rs:469-479`); interim 23-input manifest exists (`manifest.rs:1026-1056`) | Beacon (cyan) bearing/level, failed-visit mark, tremor, threat track |
| **ripple-snail** (8) | Sight 0.5 m, tremor 1 m (`body.rs:6000-6010`); `Chem(film)` | `BlindForager` (`controller.rs:591-593`), retract instead of flee (1343-1346), roam (1391), graze give-up (1402), shore laying (1412), water/thirst (1420), shallows | None; interim 23-input manifest (`manifest.rs:1097-1131`) | Dryness/thirst/ashore state, retract action, any bearing to water, shore egg face or richer film (all are path queries) |
| **frondgrazer** (`Browser`, 8) | Live: sight 8 m/4 m, 300°, a quarter while head-down; hearing 8 m; tremor 1 m; hunter smell 3.5 m; 3 kin tracks (`body.rs:6016-6031`); cone 2 m in the obs | `BrowserForager` (`controller.rs:578`), `flees`: herd (scan, orient, flee with hysteresis, alarm by hearing, bunch-and-clamp), **hunker** + shelter seek (`vigil.rs:393-422, 434-447, 956-1150`), shallows, DROP | **Yes:** 37→GRU32→3, `Cone(3, foliage/body)` (`manifest.rs:658-664, 510-537`) | Threat vs kin (the cone has one `body` class), threat count, kin alarm, hunter scent, hunker action, stomach/latch, daylight |
| **loftstrider** (2) | Live: sight 12 m/6 m, 360°; hearing 10 m; tremor 2 m; smell 2.5 m (`body.rs:6035-6048`) | `BrowserForager` banded to its mouth (`controller.rs:587-589`), herd without scan or alarm, group_min 2 (`vigil.rs:450-460`), shallows | None; interim 37-input manifest (`manifest.rs:1062-1092`) | As frondgrazer minus hunker. Only 2 bodies per world (ES problem, §3) |
| **bellwing** (6) | Live: no form vision; UV+cyan photoreceptor 4 m, loom 1 m, nectar smell 6 m, tremor 0.5 m (`body.rs:6053-6064`); `Fan(3)` 4 m, `Air`, altitude/ceiling in the obs | `HoverSip` (`controller.rs:595, 1075-1260`), `lift` (`:1251`), `rests_when_fed` (`:1255`): fed rest (`step.rs:1368-1377`) and flyer light steering (still beacon attracts, moving repels, loom, learned expectation; 1503-1515). No flee/shallows. Sampled every tick (`manifest.rs:1166-1170`) | None; a 43→4 manifest with `lift` exists (`manifest.rs:1137-1178, 710-720`) | Beacon kind/level/bearing, loom bearing, nectar gradient, stomach/latch ("rest when fed" reads the latch, which the obs lacks) |
| **seedporter** (4) | Live: sight 16 m/3 m, 180°; hearing 8 m; tremor 0.5 m; smell 2 m (`body.rs:6068-6081`); fruit-counting cone | `FruitForager` (`controller.rs:597, 954-1060`), `flees`: fruit walk (1334-1340), arboreal refuge (`vigil.rs:462-471`), roost at rest hour and in a downpour (1359-1364), insect catch by day (1436-1446), shallows. Ground dash 5.95 BL/s (`gait.rs:107-113`). The body climbs, perches and glides from forward/turn (`glider.rs:9-31`) and **sleeps only perched** (`glider.rs:45-47`) | None; interim 37-input manifest (`manifest.rs:1184-1217`) | Mode (ground/climb/perched/glide), height, threat/kin/prey/carcass tracks, carrion scent, sprint action, bearings to a climbable stand and a fruiting stand |
| **lanternjaw** (3 egg clutches) | **Not live:** interim fill (`perception.rs:48-54`). Prey within `hunt.sight_m` ahead or by tremor; table values exist (`body.rs:6086-6098`) | `LanternjawPatrol` (`controller.rs:1463-1505`): patrol patch to patch with the lure lit in the dark, search image, attack choice (1449-1474; `hunt.rs`), carcass yield (1477-1487). Strike = `feed` with prey in the capsule (2350) | None; refused with "until its contract v3 (predation P2)" (`search/controller.rs:463-467`); a 37→4 manifest with `lure` (`manifest.rs:969-1021`) | Prey/threat/carcass tracks, prey-food scent, face light (the lure only pays in the dark), stomach/latch. Riders and eggs have no controller (`step.rs:1167-1169`) |
| **Veilhound** (`chorister`, 1 pack of 4) | Live since S1: sight 16 m/9 m, 240°; hearing 10 m; tremor 1 m; smell 6 m (`body.rs:6105-6118`) | `PackHunter` (`controller.rs:1513-1540`), `pack_tactic`: drive/flank a shared target, give up, eat the kill, walk home to den rest, keep near the breeder, territory, pup stays at den (`pack.rs:1-22`; `step.rs:1488-1496`). Stalk gait (`vigil.rs:1377-1405`), rush 2.5 BL/s for 20 s (`gait.rs:109`) | None; interim 37→3 manifest (`manifest.rs:1224-1254`) | Prey/kin tracks, pack target, den bearing, breeder/pup state, sprint action, a team reward (§3) |

Two corrections to the brief's list:
1. **Strike is already expressible.** A carnivore's `feed` strikes when prey is in its
   capsule (`step.rs:2350-2360`). v3 needs no separate strike action, only the observation
   to aim it.
2. **Mate is not a slot.** No lineage searches for a mate. Conception is per body, and the
   pack breeder conceives alone (`step.rs:3950`).

## 2. Three architectures

### 2.1 Shared v3 blocks

**Bearing convention.** Body frame: positive to the right, encoded as `sin`/`cos`.
Proximity is `1 − d / range` of that sense. Every block carries validity.

| Block | Channels | Source |
|---|---|---|
| `SelfV3` (15) | The 8 v2 channels, plus: `stomach_fill` (S/K), `eating` latch, `wound`, `stamina` (1 − `sprint_spent`; 1 for a lineage with no sprint), `daylight`, `day_sin`, `day_cos` | `lib.rs:962, 1016-1022, 2043-2050`; the sky clock |
| `FlyerSelfV3` (17) | `SelfV3` + `altitude`, `ceiling` | `manifest.rs:566-577` |
| `Slot(c)` (7) | `valid`, `sin b`, `cos b`, `proximity`, `moving`, `still` (the FEAR posture REST/ASLEEP), `fresh` (1 − age/`give_up_s`) | `Percept.best[c]`, `Track` (`perception.rs:107-146, 237-245`) |
| `Threat` (8) | `Slot(threat)` + `n_threats` near (for the loftstrider's group_min 2) | `vigil.rs:605-635` |
| `Kin` (9) | `Slot(kin)` + `kin_alarm` (a kin heard bolting) + `n_kin` | `vigil.rs:833-844`, S2 alarm |
| `Prey` (7), `Carcass` (4) | `Slot(prey)`; nearest carcass: valid, sin, cos, proximity | `Percept.carcasses` (`:248-249`) |
| `Scent(ch)` (3 each) | level, gradient forward, gradient right | `ScentReading` (`scent.rs:92-96`) |
| `Tremor` (2) | left, right | `Percept.tremor` |
| `LightV3` (12 flyer / 6 capgnawer) | ambient; loom (level, sin, cos); still beacon (valid, level, sin, cos, kind UV/cyan); moving light (level, sin, cos) | `LightReading` (`perception.rs:191-214`) |
| `Places` (4 each, **Q3**) | bearing/proximity to the nearest: wadeable face, climbable stand, fruiting stand, den, water, shore egg face | Today's reflex queries (`step.rs:1334-1425`, `glider.rs`, `pack.rs`), cached at the remote-sense rate |

The local senses (`Contact`, `Wet`, `Taste`, `Chem`, `Light`, `Cone`, `Fan`, `Air`) stay
as v2 defines them. In B, the cone keeps its single `body` class. Threat and kin identity
come from the slots, so each fact is encoded once (this departs from §2.11's
`prey/threat/other` cone classes; the slots carry the same information, more cheaply).

**Cost of encoding.** The slots read a record the sense pass has already filled
(`step.rs:1155`), so encoding is negligible. The `Places` block is the one real cost:
today its path queries run only when a reflex triggers. It is unmeasured; measure it in
P6.

**Formulas used below.** Parameter count: `3H(I+H+2) + O(H+1)`. This reproduces v2's
5,571 and 6,915 (`manifest.rs:43-47`). MACs per forward pass ≈ `3H(I+H) + OH`.

### 2.2 A — safety layer

**What the brain controls.** The brain steers forward, turn and feed, plus lure/lift where
the body has them. A new `Controller::reflexes()` opts a brain into a **safety set**
chosen per lineage:
- flee or ground response: herd, hunker, refuge, freeze, retract;
- shallows;
- the snail's water/thirst reflex;
- the glider's roost;
- fed rest;
- DROP, which already applies.

**What the brain keeps.** The **behaviour reflexes** move to the brain: fruit walk, graze
give-up, roam, shore laying, light/glow steering, insect catch, patrol, pack tactic,
carcass yield.

**How the brain is told it was overridden.** Five channels are added:
- `overridden`: the share of the last interval's ticks whose held action came from a
  reflex, read from `Fauna::steer_of` (`lib.rs:4072`);
- a 4-way group of the overriding reflex: threat, water/terrain, rest/roost, other.

The delivered motion already comes back in `resolved_forward`/`resolved_turn`.

**Observation per lineage.** The lineage's existing v2 or interim layout, plus 7 Self
channels and the 5 override channels: v2 + 12.

| | shredder, capgnawer, snail | frondgrazer, loftstrider, seedporter, chorister | lanternjaw | bellwing |
|---|---|---|---|---|
| I → O | 35 → 3 | 49 → 3 | 49 → 4 | 55 → 4 |
| Parameters (H 32) | 6,723 | 8,067 | 8,100 | 8,676 |

**Seeding.** Clone the heuristic's **own** response, before overrides (today's
`RecordingController` is right for A, `search/controller.rs:249-259`). Mask the samples an
override replaced.

**Cost and training.** Per tick ≈ today's cost, about 8k MACs per sample. Training is as
today's live-train. **Size:** about 2–3 h for the gate and channels.

**Risks:**
- The brain never experiences the consequence its reflex prevents, so flight stays
  scripted forever. "Bellwings stop visiting glowcaps" can only happen if light steering
  stays out of the safety set and the beacon's kind reaches the observation.
- The two predators' brains are left with almost nothing: under the patrol and the pack
  tactic, only the search between engagements is theirs.
- It contradicts Wrysk's "heuristic behaviour only needs to be coarse enough to seed the
  brains" (2026-10-06).

### 2.3 B — learned (recommended)

**Observation:** the v3 blocks.

**Actions:** forward [0,1], turn [−1,1] and feed [0,1] (feed is the strike for hunters),
plus the body's own extra actions:
- `sprint` [0,1] for the chorister and the seedporter. It scales forward up to the
  lineage's top speed through the existing stamina gate (`gait.rs:56-70, 130-140`); a GRU
  is capped at 1 today (`manifest.rs:669-688`).
- `hunker` [0,1] for the frondgrazer. It enters or holds the vigil hunker; the shield and
  pry physics are unchanged (`vigil.rs:249-359, 1282-1294`).
- `retract` [0,1] for the snail, held for at least `retract_hold_s`.
- `lure` for the lanternjaw and `lift` for the bellwing, carried in the response. This
  retires the interim seams at `controller.rs:86-98`.
- No separate climb or glide action. The seedporter climbs by walking into a stem and
  glides by going forward off a perch (`glider.rs:9-31`). Its observation carries its mode
  so it can learn this.

**What stays (body constraints only):**
- sleep, with no controller while asleep;
- a glider sleeps only perched;
- DROP;
- the stamina gate;
- the eating latch gating bites and strikes;
- a pup's strike is refused (`step.rs:2354-2358`);
- the sunlit lure is unlit (`step.rs:1548-1561`);
- drowning, slope, climb and wound slowing are physics.

**Everything else** in `step.rs:1326-1535` is skipped for a brain. That is already true of
today's GRU. The difference is that the brain can now see what those reflexes read.

| Lineage | Observation blocks | I → O | H | Parameters |
|---|---|---|---|---|
| littershredder | SelfV3, Contact, Wet, Taste, Chem(detritus), Light, Tremor, Threat | 40 → 3 | 32 | 7,203 |
| capgnawer | as shredder + LightV3(cyan, 4) + `marked_here` | 45 → 3 | 32 | 7,683 |
| ripple-snail | as shredder + `dry`, `thirst`, `ashore`, `retracted` + Places(water, shore egg, film) | 56 → 4 | 32 | 8,772 |
| frondgrazer | SelfV3, Contact, Wet, Taste, Cone(3), Threat, Kin, Scent(hunter), Tremor, `hunkered`, Places(wade) | 71 → 4 | 32 | 10,212 |
| loftstrider | as frondgrazer minus hunker | 70 → 3 | 32 | 10,083 |
| bellwing | FlyerSelfV3, Air, Taste, Fan(3), LightV3(12), Scent(nectar), Tremor | 67 → 4 | 32 | 9,828 |
| seedporter | SelfV3, Contact, Wet, Taste, Cone(fruit), Threat, Kin, Prey, Carcass, Scent(hunter, carrion), mode ×4, height, Places(stand, fruit) | 95 → 4 | 64 (Q5) | 31,172 |
| lanternjaw | SelfV3, Contact, Wet, Taste, Cone(3), Prey, Threat, Carcass, Scent(small prey, grazer, carrion), Tremor, `face_light` | 75 → 4 | 32 | 10,596 |
| Veilhound | SelfV3, Contact, Wet, Taste, Cone(3), Prey, Kin, Carcass, Scent ×4, Tremor, Social (14: pup, breeder, pack-target slot shared by call, den, packmates near, pup begging) | 92 → 4 | 64 (Q5) | 30,596 |

**Cost.**
- MACs per sample: 7–10k at H 32, about 31k at H 64.
- Walkers are sampled every 0.25 s (`manifest.rs:771`). About 100 bodies give about 20
  forward passes per tick, about 0.2 M MACs. The bellwing runs every tick, which adds
  6–30 passes.
- Estimate: 0.1–0.5 ms per tick against d72's 7–12 ms, so 1–5 %. This is a scalar-f64
  estimate, not measured. The H 64 brains are few bodies.
- Plus the `Places` queries (unmeasured).

**Seeding.** Clone the **executed** action, after every override, together with its
`Steer` source (`lib.rs:3744-3779`). This is B-7: today's recorder takes the response
before the reflexes. The clone then flees, herds, wades out and roosts as the heuristic
did, **to the extent the observation shows why**. That is the reason for Q3.

- **Fitting:** the existing teacher-forced fit (`search/imitate.rs:20-25`) generalised to
  O ≤ 5.
- **Then ES**, warm from the clone at σ ≈ 0.05.

**Training cost.**
- **Speed.** Live-train on d72 runs at about 5.5× real time per world thread
  (training-readiness §Fact sheet). A 10-min window takes about 110 s wall, so 16 worlds
  on eidolon give about 30 updates an hour and 250–300 a night.
- **Pairs.** Pairs are bounded by bodies. Over 16 worlds:

  | Lineage group | Bodies | Pairs |
  |---|---|---|
  | shredder, frondgrazer, snail | about 128 | 16–32, with 2–4 bodies per variant |
  | capgnawer, bellwing | about 96 | 16–24 |
  | seedporter | 64 | 8–16 |
  | loftstrider | 32 | about 8 with 2 bodies each, or landscape-episode ES |
  | Veilhound | 16 packs | 8 (one variant per pack, §3) |
  | lanternjaw | adults only from phase-aligned starts | needs saved starts holding adults (egg to adult is hours, training-readiness B-6) |

- **From clone to fine-tune.** At about 10k parameters, a clone needs perhaps 100–300 ES
  updates (1 night per stage), against thousands from scratch.

**Risks:**
1. **A poor clone in a predator world means a founding massacre.** Mitigations: the
   predator-free stage first, and clone quality checked by the `Steer` agreement rate.
2. **Without Places**, the path-query behaviours (shallows, refuge, roost, fruit walk,
   shore) clone badly, and drowning and night exposure rise until ES finds a substitute.
3. **Sparse credit for rare events.** Death zeroes `E_end`
   (`search/live/window.rs:79-83`), but predation is rare per window, so the noise is
   high. Use longer windows for prey in predator stages.
4. **Few bodies per world** make the loftstrider, Veilhound and lanternjaw noisy.
5. **The pack** cannot learn coordination with 8 pairs in one night (hence Q2).

### 2.4 C — options

**What the brain outputs.** The brain's observation is B's. Its output is K option logits
plus the low-level actions. The chosen option runs its heuristic **skill**. Option 0 is
"own steering", which uses the low-level actions. An option holds for at least 1 s and
ends when its skill has nothing to do; the brain then falls back to own steering.

**Options per lineage (K):**

| Lineage | Options | K |
|---|---|---|
| shredder | forage, freeze, wade out | 3 |
| capgnawer | forage, tremor away, glow seek, wade out | 4 |
| snail | forage, retract, roam, graze move, shore lay, water | 6 |
| frondgrazer | forage, herd response, hunker/shelter, wade out | 4 |
| loftstrider | forage, herd response, wade out | 3 |
| seedporter | forage, fruit walk, refuge, roost, catch, wade out | 6 |
| bellwing | own, light steering, fed rest | 3 |
| lanternjaw | own ambush, patrol, carcass yield | 3 |
| Veilhound | own, drive/flank, give up, eat kill, home/den | 5, after splitting `pack::steer` |

**Parameters.** About B plus K·(H+1), at most 330 more.

**Seeding.** The `Steer` source is exactly the option label, so seed by **cross-entropy
cloning** on the recorded source and MSE on the heuristic's own action under option 0.
The heuristic is then both the default policy and the skill library.

**Cost.** Per tick: B's, without the `Places` cost, because each skill queries only when
chosen. Training: the cheapest ES of the three, since the clone already behaves fully.

**Risks:**
- Each reflex must become callable outside its own trigger. Today each returns `None`
  unless its condition holds (for example `flee`, `step.rs:1592-1648`). That is a refactor
  per reflex.
- The skills read privileged state (route graphs, the pack record), so the policy boundary
  (`controller.rs:4-10`) moves from the brain into the skills.
- The brain cannot improve a skill from inside it, for example a better flight angle.
- Dithering between options. Cloning needs a new categorical loss.

### 2.5 Recommendation and reasoning

**B for eight lineages; C for the Veilhound's first round (Q2); A not as a training
contract.**

1. **Wrysk's direction is B.** Heuristics are seeds (2026-10-06). Bellwings are to learn to
   skip glowcaps, which needs the beacon kind in the observation and no light reflex. The
   seedporter and the Veilhound need to choose among actions.
2. **B is closest to what a GRU body already is.** It runs without reflexes today, and B
   adds the information and actions those reflexes used. A, by contrast, adds reflexes
   that would later have to be removed, which brings a second behaviour change and a second
   retrain.
3. **The cost of B is the clone, not the runtime,** and cloning the executed action
   (package P0) is cheap.
4. **The pack is the exception.** Its tactic coordinates four bodies, a den and pups. ES
   with 8 pack-variants a night will not rediscover it soon. C keeps it intact while the
   brain learns when to hunt, rest and return. A later round can move the Veilhound to B
   when packs train against v3 prey.
5. **Opus's concern about brains learning without safety reflexes is real,** and it is
   answered by curriculum rather than by a permanent layer:
   - clone first, so brains start out fleeing;
   - predator-free stages first;
   - success gates on `killed_*`/`drowned_*` against the heuristic arm.

## 3. Order, maps, success measures and reward

**Maps.**
- `train-flat`: d96 with plain share 0.7. It is proposed in training-readiness §5 and does
  not exist yet; TOML only.
- d72: ecology and evaluation.
- d96: held-out seeds.
- `range` (`config/scenarios/range.toml`): multiple packs.
- **Predator-free** means d72 with `founding_count` 0 for the chorister and the
  lanternjaw. That is config only.

| Stage | Lineages | Map(s) | Success (paired against the heuristic arm, 8 seeds, from saved starts) |
|---|---|---|---|
| 0 | none (tooling) | none | Clones reproduce the `Steer` source on held-out recordings; the brain arm runs; v2 centres are refused |
| 1a | frondgrazer, littershredder | train-flat, then d72, predator-free | Net energy and births ≥ heuristic; awake moving share ≥ heuristic; drowned ≤ heuristic |
| 1b | same | d72 with heuristic predators | Above, plus `killed_*` ≤ heuristic; frondgrazers hunker when a pack closes (hunker events > 0) |
| 2 | capgnawer, ripple-snail, bellwing; loftstrider by landscape episodes | d72 with predators | Per lineage persists and breeds. Bellwing: the share of beacon visits to glowcaps falls over generations (Wrysk's prediction is the measure) |
| 3 | seedporter (H 64) | d72 with predators | Persists; roosts perched every night (it can only sleep perched); roost height and new-ground novelty rise with curiosity |
| 4 | lanternjaw, then Veilhound (C) | phase-aligned d72 starts; `range` for packs | Lanternjaw adults reach a meal and recruit. Pack hunts land in Wrysk's watchable band (training-readiness §3b `h_lo/h_hi`); pups reach independence |
| 5 | all nine | d72, held out on d96 | Astra's whole-world objective (Astra §3b); no lineage lost |

Two points about this order:
- **It differs from predation §2.11,** which trained the lanternjaw first. Prey go first
  here because two prey already have tooling and brains. The predators have the slowest
  clocks and the fewest bodies.
- **Lineages not yet trained keep their heuristics in every stage.** That is the
  per-lineage default today (`search/live/trainer.rs:17-20`).

**Reward per lineage.** The base stays `(E_end − E_start + E_given)/body_ref +
w·novelty`, with `E_end = 0` on death (`search/live/window.rs:79-83`).

- **Stomach (Q4a).** Add Astra's digestible-stomach potential `D` to both ends (Astra §3a,
  using the existing calculator `live/targets.rs:319-350`). v3 brains see the stomach fill
  and choose when to eat, so without `D` a late bite pays the next variant. Score awake
  time only (training-readiness §3a.1).
- **Curiosity** (`config/training/curiosity.toml`):
  - recalibrate frondgrazer 0.5 and shredder 0.3 on the v3 bodies;
  - seedporter share ≥ 2 (Wrysk, 2026-10-02);
  - lanternjaw 0 (its ambush character);
  - the others 0 until measured. Astra's table proposes the same.
- **Packs and parents (Q4b).** One variant per pack, scored by the pack's summed `E`
  including pups and escrow. Pack kills are made by drivers and eaten by others, so a
  per-body score punishes the drivers.
  - Before stage 4, audit that provisioning a pup moves energy without double counting.
    Brood already sits in reserve (`fauna/lib.rs:1452-1458`), so do not add it again
    (Astra §2 P1).
  - The egg-laying lineages keep `E_given` as it is.

## 4. Implementation packages

Every package ships mechanism tests on hand-built fixtures against whatever the config
says. No pinned numbers, digests or world hashes (WORKING_POLICY "Fast iteration").

| # | Package | Files | Size (worker-h) | Tests (mechanism) |
|---|---|---|---|---|
| P0 | **Executed-action recording.** A per-body tap after the write side records (v3 obs, held actions incl. lure/lift, `Steer`). The recorder offers both labels: pre-override for A/C option 0, executed for B | `fauna/step.rs` (after 1562), `fauna/lib.rs`, `search/controller.rs:207-269`, `search/imitate.rs` | 3–4 | A body forced into deep water records `Shallows` and the shallows action; one with no override records its own response; streams stay per body |
| P1 | **Contract v3 manifests and observation.** `SCHEMA_VERSION` 3; the blocks of §2.1; per-lineage module lists; hidden width folded into the canonical text (it varies per lineage now); variable action count (`Response::Logits` and `Actions` widened; `resolve_actions` per manifest action) | `fauna/manifest.rs`, `fauna/body.rs:4242-4563`, `fauna/perception.rs` (encoders), `fauna/controller.rs` | 10–14 (opus-high) | Each slot equals the record on a hand-built percept (bearing sign, proximity, validity 0 when empty, staleness); all finite; no coordinate reaches the vector; a changed module, action or H moves the digest |
| P2 | **Body actions.** `sprint` → stamina gate, `hunker` → vigil hunker entry/hold, `retract` → hold, lure/lift moved into the response (retire `Controller::lure/lift`) | `fauna/gait.rs`, `fauna/vigil.rs`, `fauna/step.rs`, `fauna/controller.rs` | 4–6 | Sprint above cruise only while stamina lasts and winded refuses; hunker 1 sets `hunkered` and the strike meets `Guard::Shield`; hunker 0 never hunkers; a pup's strike is still refused |
| P3 | **Runtime for nine shapes.** GRU generic over H (`cubarium-core/src/neural/gru.rs:24` fixes 32); `EpisodeGru` for every founder; live-train, ecology search and imitation take O ≤ 5 | `cubarium-core/src/neural/gru.rs`, `search/controller.rs:405-610`, `search/live/*`, `search/imitate.rs`, `search/store.rs` | 5–7 | The parameter count equals the manifest's for every founder; a v2 centre is refused against v3 by name; a wrong-H vector is refused; logits resolve through the shared adapter |
| P4 | **Reflex layer as a declared list.** Replace the four opt-in booleans with one `layer()`: heuristic or brain. The body-constraint list (§2.3) applies to all bodies; heuristics keep every reflex. An A-style per-lineage `brain_reflexes` list stays possible if Wrysk picks A | `fauna/controller.rs:100-140`, `fauna/step.rs:1326-1547` | 2–3 | A brain body in deep water is not steered out; a heuristic body is; DROP and sleep apply to both |
| P5 | **Reward.** `D` potential (if approved), awake-only scoring, pack-shared variants and pack score, curiosity recalibration run | `search/live/window.rs`, `live/trainer.rs`, `live/curiosity.rs`, `config/training/curiosity.toml` | 3–5 | `D` added to both ends and zero at death; a sleeping window scores nothing; pack members share one variant; the zero-weight score equals today's |
| P6 | **Places block (if Q3 = yes).** Bearings from the reflexes' own queries, cached at the remote-sense rate within the body's sense range; cost measured | `fauna/perception.rs`, `fauna/glider.rs`, `fauna/step.rs` reflex helpers, `fauna/pack.rs` | 4–8 | The bearing points at the face the reflex would walk to; it is empty beyond range; it is refreshed on schedule |
| P7 | **Stage runs.** Record, clone, check `Steer` agreement, ES on the stage map; detached and staged (`staged.rs`), with checkpoints | `config/training/*`, scripts | 1–2 set-up per stage, plus about 1 night of machine time | n/a (runs) |
| P8 | **Veilhound options (if Q2 = C).** Split `pack::steer` into callable skills; option head; cross-entropy clone | `fauna/pack.rs`, `fauna/step.rs:1488-1496`, `search/controller.rs`, `search/imitate.rs` | 6–10 (opus-high) | Each skill steers only when applicable; an option holds its minimum time; the clone's option matches the recorded `Steer` on a fixture |

**Order.** P0 and P1 first, in parallel. Then P2, P3 and P4. P5 can run alongside. P6 only
if Q3 = yes. P8 only if Q2 = C. Total core (P0–P5) is about 27–39 worker-hours.

**Version change and the C1 centres.**
- The bump: `SCHEMA_VERSION` 2 → 3 changes every founder's digest. The loader already
  refuses a mismatched centre (`cubarium-session/src/founders.rs:25-40`).
- In the same commit, remove the two C1 files and `BUILT_IN_POLICIES` entries
  (`founders.rs:13-24`). Mark them retired in `crates/cubarium/assets/policies/README.md`.
- Until stage 1 ships, all nine lineages run their heuristics. That also removes today's
  arm mismatch (B-2).
- Saved policy-driven worlds and E0/E1 starts are refused and regenerated ("always fresh,
  never migrate").
- **No weight transplant.** Zero-padding C1 into the v3 input matrix would be exactly the
  re-anchoring that rule rejects. v3 centres start from clones of the executed heuristic
  instead.

## 5. Questions for Wrysk

**Q1. Reflexes under brains.**
- **Options:**
  - (A) keep a safety reflex layer that overrides brains;
  - (B) brains learn everything except body constraints;
  - (C) brains choose among the heuristic behaviours as options.
- **Recommendation: B, staged as in §3,** with the exception in Q2.
- Interim, until v3 lands: run the two C1 brains' lineages on their heuristics in the
  display world, rather than C1 without reflexes.

**Q2. The Veilhound's pack.**
- **Options:**
  - (a) B: coordination is learned, one variant per pack;
  - (b) C: the pack tactic becomes options the brain chooses when to use;
  - (c) A: the tactic stays scripted over a foraging brain.
- **Recommendation: (b)** for its first round, then (a) once packs train against v3 prey.

**Q3. Do brains get bearings to places they cannot currently sense?** This means the
nearest wadeable shore, a climbable tree, a fruiting stand, the den, and water for the
snail.
- **Options:**
  - (a) none: senses only, and shore, refuge and roost are found by sight and search;
  - (b) within the body's sense range, plus the den always ("knows its home");
  - (c) unlimited, from the route graph.
- **Recommendation: (b).** It keeps the policy boundary honest. Without it, the clone loses
  wade-out, refuge and roost, and the seedporter (which sleeps only perched) and the snail
  pay first.

**Q4. Reward.**
- (a) Count the digestible stomach in net energy. **Recommended: yes.**
- (b) One shared variant and score per Veilhound pack, including pups. **Recommended:
  yes.**
- (c) Curiosity for lineages not yet set. **Recommended:** seedporter share ≥ 2 as you set,
  lanternjaw 0, others 0 until measured.

**Q5. Bigger brains for the seedporter and the Veilhound.**
- **Options:**
  - (a) H 64 (about 31k parameters, roughly 3× the ES dimension, about one more night per
    stage);
  - (b) H 32 for everyone first.
- **Recommendation: (a)**, given your note that they will need much more complex brains.

**Q6. Order.**
- **Options:**
  - (a) prey first (stages 1–3), predators last;
  - (b) the lanternjaw first, against heuristic prey, as predation §2.11 wrote.
- **Recommendation: (a).** The prey have tooling, brains and many bodies; the predators have
  the slowest clocks.
