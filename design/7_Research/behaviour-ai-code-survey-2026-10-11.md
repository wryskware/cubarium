---
design_status: exploration
date: 2026-10-11
---

# Behaviour-AI code survey: group defence and pack tactics (2026-10-11)

A read-only map of what `crates/cubarium-voxel-fauna` does today for coordinated
behaviour: loftstrider family defence, Veilhound pack tactics, and how both meet the
trained brains. Evidence, not decisions. Paths are relative to
`crates/cubarium-voxel-fauna/src/` unless they start with `crates/`, `config/` or `design/`.
The Veilhound's code identifier is `chorister`; it appears below only where code is quoted.

Numbers are the shipped code defaults plus the wide-world preset
(`config/desktop/terrarium-wide.toml`). Derived figures (speeds in m/s, hits to kill,
window sizes) are computed from those constants by hand, not measured in a run.

---

## 1. The per-tick pipeline

**Clock.** 20 ticks/s (`crates/cubarium-voxel/src/lib.rs:98`). The controller period is
0.25 s, 5 ticks (`manifest.rs:1157`, `cadence_ticks` at `manifest.rs:1871`). Remote
sensing runs at 4 Hz, staggered by id: body `id` senses on ticks where `tick ≡ id mod 5`
(`knobs.rs:943`, `perception.rs:713-723`). A body is sampled when `age_ticks % 5 == 0`
(`step.rs:1557-1559`). Sensing and sampling therefore run at the same rate, at different
phases per body.

**Order** (`step.rs:148-160`, three parts):

1. `begin` (`step.rs:177-262`), **serial**. Dens are renewed, then circadian state (sleep,
   rest bouts, alarm wakes, the eating latch), rain, upkeep, digestion, `heal`, growth,
   dispersal at adulthood, guts. Then `pack::upkeep`: calls count down, pups come of age,
   succession, provisioning (`pack.rs:112-134`). Independent pups disperse after that.
2. `read` (`step.rs:265-297`). The litter and scent fields update every `UPDATE_TICKS`,
   then `controllers()` runs (`step.rs:1531-2200`). (The brief's "1326–1547" is stale.)
   - `perception::sense_pass` (`step.rs:1543`; `perception.rs:696-771`) is **parallel**:
     each due live body builds its tracks on the compute pool, and the results are folded
     into `fauna.percepts` serially, in body order.
   - `places::refresh` (`step.rs:1544`).
   - The **read side** (`step.rs:1648-1758`) is **parallel**: each due body's observation
     (`body::observation_read`) and its controller's `drive`, split into jobs by
     `par::ranges`. Nothing shared is written; sense writes come back as values.
   - The **write side** (`step.rs:1764-2200`) is **serial in body order**: every reflex,
     including flee/vigil, herding and the pack tactic, then the held actions.
3. `finish` (`step.rs:301-383`). `act()` is **serial** over all bodies (`step.rs:332`,
   `step.rs:2796+`). Then `vigil::pry` (`step.rs:335`). Then the strike `Hit`s, applied
   serially in the order the strikers acted (`step.rs:336-371`), then births, deaths and
   burrs.

**Parallelism** is `bevy_tasks::ComputeTaskPool`, not rayon (`par.rs:1-30`). Results come
back in job order, so a parallel run equals a serial one. Only four places use it:
`sense_pass`, the observation read side, the `LiveGrid` builds and `encounter` route
columns (`grep par::run`). Every reflex and every cross-animal write is serial.

**Cross-animal effects.**
- Strikes are events. `strike_tick` pushes a `Hit` (`step.rs:3771-3932`); the serial pass
  applies it (`apply_hit` `step.rs:3958`, `wound` `step.rs:3987-4010`). See the
  "Concurrency" note in `predation.rs:6-10`.
- A counter-strike is a `Hit` with `counter: true`. It is pushed **before** the strike's own
  hit in the same vector (`step.rs:3836-3841`), so it lands first.
- Pry progress is written once a tick by `vigil::pry` (`vigil.rs:1450-1513`).

**Shared group state today.**
- **`PackRecord`** (`lib.rs:1241-1311`): den, breeder, `target`, chase clocks
  (`chase_since_tick`, `chase_progress_tick`, `chase_best_m`, `chase_work`), `target_site`,
  `feast`, `feast_eaten`, `abandoned`, `shunned[4]`, `drawn`, `drawn_scent`, `season`.
  It is saved state. Pack members write it in the serial reflex pass (`commit`
  `pack.rs:1325-1368`, the give-up block `pack.rs:1690-1800`). This is race-free but
  order-dependent: the lowest-index due member that finds no target commits first in a
  tick.
- **`PackMember`** (`lib.rs:1201-1213`): `pack`, `pup`, `calling_ticks`.
- **There is no herd or family record.** A loftstrider "herd" exists only as
  - `herd_labels` (`lib.rs:2990-2997`): connected components chained within
    `herding.comfort_m`, computed only when a calf reaches adulthood (`step.rs:1398-1416`);
  - each body's own percept kin tracks.
- `Animal` has **no parent/mother id** (field list `lib.rs:962-1100`). No code can tell a
  calf's mother from any other adult.

**`Steer` sources** (`lib.rs:4244-4281`): `Rest, Heuristic, Fruit, Flee, Roost, FedRest,
Shallows, Roam, Patrol, Yield, Pack, Light, Drop, Leave, Search, Nest, Wade, Host`. There
is **no Herd source**: the herd pull keeps its caller's source (`step.rs:1872-1884`).

**Reflex priority.** In the write side every later layer overwrites `actions` and `src`:
**the last one to fire wins** (`step.rs:1783`, "the last override winning"). In order:

1. `rove::search` → Search (`step.rs:1812-1826`).
2. Body actions: the sprint multiplier, and `hunker_action` (a brain's hunker,
   `step.rs:1830-1840`).
3. Glider fruit walk → Fruit.
4. FORAGE-LEAVE → Leave.
5. **HERD-PULL** `herding::steer` (`step.rs:1877`). Only on Heuristic, Search or Leave
   sources.
6. Snail retract.
7. Nest-seek → Nest.
8. **Flee / vigil** → Flee (`step.rs:1901-1910`). Only for `prey` bodies: heuristic, not a
   flyer, not lanternjaw or Veilhound (`step.rs:1778`).
9. Roost, fed rest, shallows, roam, graze, shore and water reflexes.
10. Seedporter `catch_step`.
11. Lanternjaw patrol → Patrol.
12. Carcass yield → Yield.
13. Host-seek → Host.
14. **Pack tactic** → Pack (`step.rs:2050-2058`). Only on heuristic Veilhounds
    (`step.rs:1781`).
15. Vine nest, light steering.
16. DROP / WADE-OUT, last of all (`step.rs:2114-2127`).

Stop-and-go `pace` then applies to unfled prey and pack bodies (`step.rs:2147-2152`). Note
that flight (8) beats herding (5), and that DROP beats everything.

**Reflexes are heuristic-only.** `prey`, `pack` and `patrol` all require
`controllers.layer(id) == Heuristic` (`step.rs:1776-1781`). The contract states it: "Only
`Heuristic` adds flee/refuge/hunker choice, herding, … pack tactics" (`controller.rs:173-178`).
A brain-driven loftstrider or Veilhound therefore has **no vigil, no herd pull and no pack
tactic**: only body constraints.

---

## 2. Vigilance and fear (`vigil.rs`)

**Threat detection.**
- `threats()` (`vigil.rs:661-694`) reads the body's own record: `class::THREAT` tracks
  younger than 2 × stagger (10 ticks).
- `class_of` makes a body a THREAT when its lineage lists the observer as prey, with **no
  size-window test** (`perception.rs:345-357`). A lone Veilhound is a threat to an adult
  loftstrider it could never take.
- A threat is **pressing** when it is seen moving, seen within `still_threat_near_m` (0.5 m,
  `knobs.rs:118`), heard or felt, and is not "calm" by FEAR (`vigil.rs:671-679`).
- Each track carries a gait guess, `cruise_bl × size_m`, while it moves (`vigil.rs:683-685`).

**The herd response for the loftstrider.**
- Its table (`vigil.rs:500-509`): `Herd` mode, `flee_start_m` 6, `flee_stop_m` 12,
  `calm_s` 10, **`group_min` 2**, `run_noise` 3, `orient_s` 3. Then `..WATCH`: FEAR on,
  `calm_margin_m` 3, `calm_facing_deg` 30, `lost_calm_s` 30 (`vigil.rs:246`).
- **No scan pause** (`scan_every_s` 0), **no herd alarm** (`alarm_emission_factor` 0) and
  **no hunker** (`hunker_of` enables only `Founder::Browser`, the frondgrazer,
  `vigil.rs:424-455`).
- `sleep_hearing` 0.5 is added to every lineage (`vigil.rs:457-462`).
- With the hunker off it takes the `else` branch of `herd()` (`vigil.rs:1107-1112`):
  ```
  need = if adult { group_min } else { 1 }   // vigil.rs:1041
  flight(&mut v, &vg, &a, &th, need, …)      // vigil.rs:708-752
  ```
  "Adult" means `body >= core.birth_body`, which is 0.6 of `body_max` (`vigil.rs:1040`).

**What an adult loftstrider does, by case** (test `an_adult_stands_its_ground_below_its_group`,
`ground_response_tests.rs:213-234`, asserts `fleeing == (n >= 2)`):

- **One pressing Veilhound within 6 m.** It does not flee. It reaches the `orient_s > 0`
  branch (`vigil.rs:1175-1195`), takes `standing` (the nearest pressing threat within
  `flee_start_m`) and returns `head(..., forward 0)`, `feed 0`. It **stands facing the
  hunter and does not eat**, for as long as that hunter keeps pressing within 6 m. There is
  no timer on standing.
- **Two or more pressing within 6 m.** `flight` starts. `away()` runs straight away from
  the nearest pressing threat's point at forward 1.0, with no sprint (`vigil.rs:754-805`).
  The flight ends beyond 12 m after a 10 s calm, when the hunter is seen still, or after
  30 s with it lost.
- **Hunter asleep, or resting and facing away** (FEAR, `calm_of` `vigil.rs:639-659`). Not
  pressing. Inside 3 m (3D) it walks off at forward 1 (`calm_walk` `vigil.rs:861-879`).
  The hunter becomes a full threat again when it moves, is heard or felt, faces within 30°,
  or comes within 0.5 m.
- **Calf** (`body < birth_body`). `need` = 1, so **a calf flees one Veilhound alone, while
  its adults stand**. It does not seek shelter: `seek_shelter` needs `hunker.enabled &&
  herd_enabled` (`vigil.rs:1125-1131`, `955-962`), which the loftstrider lacks. This is the
  most important current constraint for family defence (section 4).

**The alarm.** It is a sound model of a kind, not an instant flag.
- A running body (fleeing or bolting) emits `run_noise` × its walking emission
  (`perception.rs:637-645`). Emission is `speed^1 × length^1` (`perception.rs:388-396`).
- The alarm is read from **kin tracks heard at the last remote sensing**: loudness × distance
  must be at least `alarm_emission_factor` × a walking kin's emission (`vigil.rs:1148-1170`).
- Only kin hear it, only lineages with `alarm_emission_factor > 0` (the frondgrazer only),
  and only within the hearer's `hearing_m` and `kin_tracks` budget.
- Latency is up to one stagger (≤5 ticks) plus the hearer's next sample (≤5 ticks): about
  0.25–0.5 s per hop. The bolt lasts `alarm_bolt_s` 3 s, then `alarm_refractory_s` 20 s of
  deafness to alarms.
- The loftstrider neither alarms on hearing nor reads alarms awake. Its Kin block's alarm
  channel is always 0 (`observe_v3.rs` `kin_alarm` returns 0 when the factor is 0).

**Alarm wakes sleepers** (`rhythm.rs:371-408`, called at `step.rs:906-914`).
- An asleep body of a lineage that hears wakes when **any running body of any lineage**
  within `hearing_m` (3D) reaches `hearing_min / sleep_hearing`.
- "Running" means `vigil.running()`: a prey fleeing or bolting (`rhythm.rs:399-408`). **A
  rushing Veilhound is not "running"**, so a rush does not wake a sleeping herd. A fleeing
  neighbour, or a hit, does.
- A woken body stays up `alert_s` 60 s (`step.rs:861`, `knobs.rs:137`).

**Asleep and resting bodies.**
- Asleep bodies **do not sense** (`perception.rs:718`) and **are not sampled**
  (`step.rs:1557`). A sleeper cannot detect a Veilhound.
- It wakes only by: an alarm as above; a hit (`wound()` sets `Resting` and 60 s alert,
  `step.rs:3997-4004`); hunger reaching emergency (`step.rs:915-920`); or its rest bout
  ending.
- A leaning lineage's rest bout is sleep: off-phase → `Rest::Sleep` (`step.rs:922-952`).
  Bouts are drawn per body from `rhythm.chance`, keyed by id (`step.rs:892-901`). The
  loftstrider's table is `day 0.85, night 0.25, twilight 0.6, rest_mean_h 1.0,
  rest_min_h 0.25` (`rhythm.rs:302-311`). One day-clock hour is 5 sim-minutes
  (`rhythm.rs:263`).
- So **herd members sleep independently**, with no synchronised rest and no sentinel.
- The loftstrider has no rest-site reflex: no roost, no den, no `Place(…)` for rest
  (`places.rs:17-27` kinds). It sleeps where it stands.

**Awareness and the dodge.**
- `dodge_chance_with = agility × awareness × aspect × condition` (`predation.rs:248-262`).
- Awareness is `awareness_sensed` 1.0 if the prey's record holds the striker within
  `aware_within_s` 1 s, or it is fleeing that striker; else `awareness_unaware` 0; asleep 0
  (`vigil.rs:1364-1385`, `knobs.rs:144-146`).
- Rear aspect ×0.5; condition floor 0.5 (`knobs.rs:128-129`).
- **Loftstrider `agility` is 0** (`body.rs:2499`): it never dodges. The frondgrazer's is 0.5
  (`body.rs:2216`).

**Still gain.** Every lineage senses 1.25× farther (and with 1/1.25 floors) after 0.5 s
standing still (`body.rs:6614`, `SenseTable::sensing` `body.rs:6571-6590`). A loftstrider
standing its ground therefore sees 15 m by day.

---

## 3. HUNKER and HERD (frondgrazer only)

**HUNKER** (`vigil.rs:88-116`, knobs `vigil.rs:280-347`, values `vigil.rs:424-455`):
- It clamps down at 2.5 m and comes out after none has been within 5 m for 20 s × condition.
- Shield 0.9, growing with body², opened by wounds.
- **Pry**: each awake, hungry hunter in contact pries at 1/90 s⁻¹. The body resists at 1/40
  × body² × (1 + 0.5 per hunkered kin within 1.5 m, up to 3). Opened, it lies bared for
  20 s.
- Worked numbers are in the doc comment (`vigil.rs:405-423`): three hunters open a lone
  adult in 120 s, four in 51 s.
- `pry` runs once a tick, serially (`vigil.rs:1450-1513`). Under HERD, only the pack whose
  `target` is this body pries it (`vigil.rs:1486-1490`).

**HERD** (`herd_enabled`, the Browser only):
- It runs at a pressing hunter within `run_start_m` 8 m.
- It is caught, and hunkers, when the hunter is within `catch_m` 3 m and its gait is faster
  than its own cruise, or when a 0.5 m escape probe is blocked (`vigil.rs:1057-1061`).
- Before contact, an adult runs to bunch with kin farther than `ring_m` (`vigil.rs:1062-1074`).
- A juvenile seeks a closed adult within `shelter_seek_m` 6 m and tucks against its far
  side (`seek_shelter` `vigil.rs:999-1027`).
- A strike crossing a closed adult's footprint reaches the calf at `shelter_exposure` 0.25
  (`vigil.rs:980-997`, used at `step.rs:3870-3871`).
- Tests: `herd_tests.rs:159-653` and `testpass4_herd_tests.rs:152-494`. Examples:
  `a_herd_runs_from_a_pack_beyond_hunker_range_then_hunkers_when_it_closes`,
  `a_juvenile_in_reach_moves_toward_a_hunkered_adult_without_hunkering`,
  `a_hungry_pack_only_pries_the_grazer_it_is_actually_targeting`.

**Many eyes.** Frondgrazer only: `scan_kin_m` 4, `scan_kin_gain` 0.25, `scan_kin_max` 4
(`vigil.rs:483-489`), applied in `herd()` at `vigil.rs:1201-1225`.

**HERD-PULL** (`herding.rs`, values `herding.rs:87-93`):

| lineage | comfort_m | pull | herd_max | memory_s | young_only |
|---|---|---|---|---|---|
| frondgrazer | 3 | 0.5 | 4 | 60 | no |
| loftstrider | 6 | 0.5 | 0 | 60 | **yes** (calves only) |
| seedporter | 4 | 0.5 | 0 | 60 | no |

- The pull leans the walking turn toward the **mean point of the fresh kin tracks**
  (`group_pull` `herding.rs:118-145`).
- It is suppressed while the body crops, is blocked, is off the ground or stands still
  (`herding.rs:212-214`).
- With no kin in sense it walks toward the remembered best kin track for `memory_s`.
- **Loftstrider `kin_tracks` is 1**, the default. Its table never overrides it
  (`body.rs:6675-6688`; default `body.rs:6556`). So a calf's "family" is **its single
  nearest sensed loftstrider of any age**, and an adult's Kin count channel is at most
  0.25. The frondgrazer weighs 6 (`body.rs:6658-6660`).
- `stay_with_group` (`herding.rs:159-176`) is the same pull as a whole action, written as a
  future hybrid skill. It is not wired to anything yet.

**Dispersal.** Configured in the wide preset (`terrarium-wide.toml:154-157`, `group_max`
3, `at = "adult"`).
- When a calf reaches `birth_body`, it **leaves the world**: removed, its matter deposited
  as litter (`step.rs:1363-1462`). It does not walk away.
- It leaves if the adults of its herd (chain within 6 m) plus itself exceed 3
  (`step.rs:1398-1420`).

---

## 4. The loftstrider today

**Body** (`body.rs:2460-2517`).
- Adult 2.2 m long × 2.2 m wide × 3.4 m tall. Eye at 0.44 H, mouth band 0.235–1.03 H, steps
  1.15 m up and down, wades 0.25 H, drowns at 0.44 H.
- Built from the frondgrazer by volume (r 3.52, `body.rs:222`). Geometry scales by
  `(body/body_max)^(1/3)` (`body.rs:2944-2973`).
- Wide preset: `body_max` 1584, `birth_body` 950.4 (0.6), `body_min` 158.4 (0.1),
  `mass_scale` 9000 (`terrarium-wide.toml:161-167`).
- A newborn is cbrt(0.1) = 0.46 of the adult: about 1.0 m long and 1.6 m tall. At
  `birth_body` it is 0.84: about 1.86 m long and 2.9 m tall.

**Gait.**
- It inherits cruise 1 BL/s from the frondgrazer (`body.rs:2168`): 2.2 m/s full-grown,
  1.86 m/s at `birth_body`, about 1.0 m/s newborn.
- **No sprint.** `sprint_of` gives sprints only to the Veilhound and the seedporter
  (`gait.rs:107-114`).
- Flight is forward 1.0, which is cruise. Stop-and-go pace is 10 s cycles with 1–3 s pauses
  (`rhythm.rs:361`), not applied while fleeing (`step.rs:2147`).
- Wounds slow it by ×(1 − 0.5·wound) (`step.rs:2986-2991`, `knobs.rs:138`).

**Defence.**
- `armour` 0.0, `agility` 0.0, `counter: None`, with the comment "On no prey list until
  the [Veilhound] (R3); no defence counter" (`body.rs:2497-2500`).
- It has no hunker, no shield and no kick. Its only defence today is **standing its ground
  facing a lone hunter**, which has **no mechanical effect**. It does not lower the pack's
  prospect except through the 0.5 "aware" factor (section 5).
- The `Counter` seam exists and works (section 6). It is used only by the lanternjaw
  (`body.rs:2351`).

**Family and founding.**
- `founding_together` with `founding_min_region_faces` 500, AtFood, per world
  (`body.rs:2501-2510`). Wide preset: `founding_count` 2 (`terrarium-wide.toml:141-145`).
- One calf per gestation. Wide preset: `gestation_s` 14400, one litter a year,
  `breeding_phase` 0.8 (`terrarium-wide.toml:146-153`).
- No lactation or provisioning: only the Veilhound has `provision`.
- A calf forages for itself from birth and is held near kin only by the herd pull above.
- Nothing ties a calf to its mother. There is no parent id, and the "nearest kin" pull
  applies to any loftstrider.

**Rest.** As section 2: independent rest bouts, slept where it stands, blind while asleep,
woken by an alarm, a hit or hunger.

**Near a Veilhound while resting.**
- If it is **asleep**, nothing happens until one of those wake events.
- If it is **awake and still** (posture REST), it senses as usual and responds as above.

**Pack prey list.** The Veilhound's hunter table lists the loftstrider (`body.rs:2771-2772`).
The lanternjaw's does not (`body.rs:1778`). The Veilhound is the loftstrider's only
threat.

---

## 5. The Veilhound pack (`pack.rs`)

**Body and gait** (`body.rs:2724-2805`, `gait.rs:110`).
- 1.4 m long × 0.35 m wide × 0.6 m tall, cruise 1 BL/s (1.4 m/s), steps 0.6 m.
- **Rush** 2.5 × cruise = 3.5 m/s, endurance 20 s, recovery 120 s.
- Wide preset: `body_max` 411.6, `birth_body` 246.96 (0.6), `mass_scale` 4500
  (`terrarium-wide.toml:336-346`).
- Rhythm (`rhythm.rs:337-345`): any hour (0.35 day, night and twilight), `rest_mean_h` 2.0.
- Pace: 12 s cycles (`rhythm.rs:363`).

**Speed against loftstriders.**

| Pursuer | Full adult (2.2 m/s) | Young adult (1.86 m/s) | Newborn calf (~1.0 m/s) |
|---|---|---|---|
| Veilhound cruise, 1.4 m/s | slower | slower | **faster** |
| Veilhound rush, 3.5 m/s for 20 s | faster: closes 1.3 m/s | faster | faster |

The rush triggers once the prey runs (`vigil::stalk` `vigil.rs:1530-1534`), so a fleeing
loftstrider draws the rush at once.

**Pack table** (`Pack::CHORISTER`, `body.rs:1008-1088`; struct `body.rs:673-1000`).
- `pack_size` 4 (one breeder), `pack_radius_m` 6, `den_radius_m` 4, `rest_radius_m` 0.5,
  `territory_radius_m` 10.
- `flank_offset_m` 1.5, `chase_give_up_s` 20 (progress 0.5 m or any work done),
  `switch_margin` 1.5 after a 10 s stall, `close_in_m` 2.5, `commit_m` 2.5,
  `stalk_margin` 0.7, `stalk_freeze` on.
- `follow_radius_m` 3, `chase_reserve_floor` 0.3, `call_s` 1.
- Selection knobs (below), `shun_s` 120.

**Who decides, and when** (`tactic` `pack.rs:1487-2157`).
- **Hunt start.** At least half the adults have their eating latch on, or any adult is in
  an emergency (`pack_wants` `pack.rs:362-366`), and no adult is spent.
- The first due member in body order that wants food and has a packmate within 6 m runs
  `choose` and `commit`s (`pack.rs:1908-1940`).
- The **leader**, for the draw only, is the breeder when it is hungry, else the
  lowest-id hungry adult (`pack.rs:969-979`).
- **Pooled knowledge.** `known()` (`pack.rs:796-826`) merges the PREY tracks of every
  awake adult packmate within the member's **hearing radius (10 m)**. It reads their
  percept records directly. No sound is emitted and nothing is called; this is a
  privileged read (test `a_packmate_knows_a_sighting_only_within_hearing_range`,
  `pack_search_tests.rs:358`).

**Target choice** (PACK-TUNE and HERD-PULL).
- `score = prospect × size^0.5 / (1 + d/6) × straggler` (`pack.rs:1178-1188`).
- `prospect` (`pack.rs:862-914`) is the product of:
  - takeable here; for a hunkering lineage, `1 − R/(n·p)` when the members at hand can open
    it, else 0;
  - × 0.8 per same-lineage awake body within 3 m (up to 4);
  - × 0.5 when the prey's record is aware of **any** pack adult;
  - × (1 + 2·wound);
  - × 0.25 for a flyer.
- `straggler = 1 + 1.0 × min(d_nearest_kin, 6)/6`, using **true positions** of every awake
  same-lineage body (`pack.rs:1190-1208`).
- Commit needs prospect ≥ 0.1 (`body.rs:1085`). The best candidate must also have a
  strike node in route reach (`pack.rs:1307-1316`).
- No term weighs juveniles directly. Juveniles score higher through the hunker term, which
  does not apply to the loftstrider, and through the size window. Size enters as
  `size^0.5`, so meat favours big prey.
- Tests: `of_an_isolated_grazer_and_one_in_a_herd_at_equal_reach_the_pack_takes_the_isolated_one`
  (`herding_pack_tests.rs:109`),
  `a_pack_picks_a_juvenile_or_an_isolated_grazer_over_a_hunkered_adult_in_a_ring`
  (`pack_search_tests.rs:526`).

**Roles** (`pack.rs:2001-2075`).
- Hunters are participants (awake, eating or emergency) within 12 m of the target.
- The **driver** is the hunter nearest the target, ties to the lowest id. It walks straight
  at the target, or by route.
- Every other hunter takes a **flank point** 1.5 m either side of the target, across the
  driver's line, alternating by rank (`pack.rs:2046-2066`). It closes in once within 0.5 m
  of that point.
- When a straight walk is blocked, everyone follows the route instead, so flanking happens
  only on open ground.
- There is **no encirclement, no ambush line, no cutting off the escape, no
  drive-toward-a-partner and no turn-taking**.
- The stalk then applies (section 2). Feed is held throughout, so any prey in a member's
  capsule is struck.

**Chase end** (`pack.rs:1690-1800`).
- Ends are `Killed`, `Gone`, `Late` (20 s with no progress), `Spent`, `Trespass`, `Lost`
  (no adult has sensed it within `give_up_s` 10 s), `BrokeOff` (prospect under 0.05 with
  the members at hand) and `Switched`.
- A target dropped alive is shunned 120 s. An armoured herd is shunned within 3 m
  (`herd_shun_m`).

**Strike** (`founder_act` `step.rs:3108-3139`; `prey_in_capsule` `step.rs:3623-3688`).
- It fires automatically when the member's eating latch is on, `feed` is held, it is not a
  pup, and some prey is in its capsule.
- It strikes the **nearest prey in the capsule, whatever the pack target**.
- Capsule: the jaw sits 0.5 L ahead of centre and reaches 0.6 L, about 0.84 m for an adult.
  The height band is `max(2.25 m, 0.6 H)` (`body.rs:1757-1759`, `2777`), so it reaches up a
  loftstrider's flank.
- Timing: windup 0.15 s, lunge 0.25 s, recovery 3 s (`body.rs:1786-1788`), so **one strike
  per member per ~3.4 s** at most. Each lunge costs 100 s of awake upkeep (`step.rs:3790-3793`).

**Size window** (`predation.rs:97-160`; `window_body` `pack.rs:80-98`).
- Prey `window_size ≤ 1.5 × Σ size` of participating adults within 6 m, where `size =
  body/mass_scale`.
- A full-grown Veilhound is 0.0915; a full-grown loftstrider 0.176; one at `birth_body`
  0.106.
- **One Veilhound** (window 0.137) can take a loftstrider up to about 0.78 of `body_max`,
  which includes **young adults**. **Two** (0.274) can take any adult. This is computed;
  the test `a_lone_chorister_does_not_take_a_loftstrider_and_a_pack_does`
  (`chorister_tests.rs:410`) checks only a full-grown one against 1 and 3.

**Damage on big prey** (`step.rs:3880-3893`).
- Per hit: `strike_power 0.7 × size_pred/size_prey × (1 − armour)`.
- Full-grown Veilhound on full-grown loftstrider: 0.364, so **3 hits kill** (wound ≥ 1,
  `step.rs:4006`).
- On a loftstrider at `birth_body`: 0.61, 2 hits.
- On a newborn calf: about 3.6, **1 hit**.
- Hits from several attackers add (`two_choristers_wounds_on_a_loftstrider_add`,
  `chorister_tests.rs:447`).

**Counter-wound precedent** (`counter_answer` `step.rs:3933-3950`, `Counter`
`body.rs:1898-1918`).
- A grown body (≥ `birth_body`) with a `Counter` answers a strike that *contacted* it.
- The answer is a hit on the striker:
  `damage × size_self/size_striker × (1 − striker armour)`, applied only if the striker's
  jaw ends within `footprint_radius + reach` of it. The answerer pays
  `cost × maintenance × body`.
- The lanternjaw's values: reach 0.5, damage 0.7, cost 100.
- Tests: `hunt_escape_tests.rs:312-338`. The strike on the answering body is unchanged.
- **It is purely reactive.** There is no proactive kick, no target choice and no range
  beyond the striker's own jaw. Contact is computed before the dodge roll, so a dodged
  strike is still answered (`step.rs:3838` comes before the dodge at `step.rs:3892`).

**After a kill.**
- The pack's `feast` holds while the carcass record stands. Hungry members eat; no new
  target is taken (`pack.rs:1856-1906`).
- Out of phase or fed, a member walks home to rest within `rest_radius_m` 0.5 of the den,
  if the trip fits its reserve (`must_walk_home` `pack.rs:368-424`, called from
  circadian `step.rs:942`). Otherwise it rests in place.
- Rest bouts are **independent per member**.
- An idle adult walks back within 3 m of the breeder (`pack.rs:2088-2105`).

**Den and pups.**
- The den is a covered, dark, dry site (`den.rs`, `renew_dens` `pack.rs:442`).
- Pups stay within the 4 m den radius, never strike (`step.rs:3115-3118`), and are
  provisioned by adults at the den: scramble, 0.7 efficiency, regurgitation only above
  `regurgitate_reserve_fraction` (`provision` `pack.rs:198-335`).
- Pups become independent at 21600 s (wide preset). Then they disperse once there are more
  than 4 adults, and leave the world (`terrarium-wide.toml:304-309`).

**The "call".**
- `calling_ticks` is set on a target change or a strike (`pack.rs:68-75`, `step.rs:6819`)
  and counts down (`pack.rs:121`).
- **No rule, sense or renderer reads it.** `grep calling()/calling_ticks` outside tests
  finds only the setters. It is saved state with no effect.

---

## 6. Wounds

- **Stock.** `Animal::wound`, f64, saved; schema 20 (`snapshot.rs:377`).
- **Formula.** Strike damage as in section 5; armour multiplies `(1 − armour)`. A hunkered
  shield adds a glance chance; bared removes armour (`step.rs:3848-3866`). A retracted snail
  is ×(1 − retract_armour).
- **Several attackers add.** `a.wound += damage` (`step.rs:3993-3995`).
- **Death.** `wound ≥ 1` kills (`step.rs:4006-4008`). The whole body becomes carrion with a
  carcass record (`kill` `step.rs:4012`).
- **Slowing.** Speed ×(1 − `wound_slow` 0.5 × wound) for walkers and flyers
  (`step.rs:2986-2991`, `3203`; glider `glider.rs:405`).
- **Healing** (`heal` `step.rs:1259-1287`). `wound_heal_per_s`, wide preset 4.55e-4/s
  (`terrarium-wide.toml:672`), paid from the reserve at `heal_cost` 1 × body per unit healed.
  A 0.36 wound heals in about 800 s.
- **Other effects.** A hit wakes the prey and keeps it alert 60 s (`step.rs:3997-4004`).
  The pack's prospect rises ×(1 + 2·wound). A wound opens a hunker shield
  (`wound_opens`, `vigil.rs:316`).
- The v3 SelfV3 block exposes the body's own wound (`observe_v3.rs:203`). No track exposes
  **another** body's wound, so prey and hunters cannot see who is wounded.

---

## 7. Senses relevant to coordination

**Pipeline** (`perception.rs:1-50`, `sense_one` `perception.rs:575-635`).
- Bodies are binned on a 2 m grid.
- Each observer takes candidates within its reach and sorts them nearest-first. It weighs at
  most `max_candidates` 8 prey and threats plus `kin_tracks` of its own lineage
  (`knobs.rs:944`).
- One terrain ray per pair is shared by sight, hearing and tremor. Blocked, it stops sight
  and multiplies hearing and tremor by 0.25.
- The record holds at most 32 tracks (`perception.rs:72`).

**Ranges** (`senses_of`, `body.rs:6609-6760`):

| | sight day/night (moving) | still fraction | half-angle | hearing (floor) | tremor | smell | kin_tracks |
|---|---|---|---|---|---|---|---|
| loftstrider | 12 / 6 m | 0.667 | 180° | 10 m (0.245) | 2 m | 2.5 m-level | **1** |
| Veilhound | 16 / 9 m | 1.0 | 120° | 10 m (0.00375) | 1 m | 6 m-level | **1** |
| frondgrazer | 8 / 4 m | 0.625 | 150° | 8 m (0.327) | 1 m | 3.5 m-level | 6 |
| seedporter | 16 / 3 m | 1.0 | 90° | 8 m | 0.5 m | 2 m-level | 1 |

All of these gain 1.25 when still. The frondgrazer sees 0.25 of its range head-down.

**A track** (`Track`, `perception.rs:107-146`) holds:
- id, lineage, class, modality mask, last-known x/z/y, site, lift, grounded, climbing;
- `moving`, distance, bearing, elevation, loudness, footfall;
- `size_m` (length, so age can be inferred), tick, `heading`;
- `posture` (ACTIVE, REST or ASLEEP, by sight only, `perception.rs:166-174`).

It does **not** hold wound, vigil state (fleeing, hunkered, standing), pack role, pack
target, or whether the body is a parent or calf beyond size.

**Counts.**
- A loftstrider perceives up to 8 threats with bearing, distance, moving and posture, but
  **only one kin**.
- A Veilhound perceives up to 8 prey, **one packmate**, and through `known()` (privileged)
  the prey of packmates within 10 m.

**Hearing.**
- There is no propagating sound field.
- Hearing is pairwise, per remote sensing: emission over distance against a floor, with the
  same ray occlusion. It applies only to bodies already among the candidates.
- **There is no "call" channel.** Running prey are louder (`run_noise`). Nothing else emits.

**Scent** (`scent.rs`).
- A diffusing surface field with channels `Grazer`, `Hunter`, `SmallPrey`, `Carrion`
  (nectar handled separately).
- Bodies lay their lineage's channel. The frondgrazer, loftstrider and seedporter lay
  `Grazer`; the lanternjaw and Veilhound lay `Hunter` (`body.rs` `scent:` fields).
- The pack follows prey scent up its gradient when it senses nothing (`scent_step`
  `pack.rs:948-967`).
- The loftstrider reads `Scent(hunter)` only through a brain's v3 block. No heuristic rule
  of its own reads hunter scent.

**Tremor.** Footfall within `tremor_m`, used by freeze and the snail.

**`Places`** (`places.rs:1-27`). Kinds: shore, water, shore_egg, film, roost, fruit, den,
nest. Saved homes: roost, water, den, nest. There is no herd, rest or rally place.

---

## 8. Brain contract v3 as built

**Manifests** (`manifest.rs:860-1020`). Loftstrider modules (`manifest.rs:927-938`): Self,
Contact4, Wet, Taste1, Cone3, SelfV3, **Threat**, **Kin**, Scent(hunter), Tremor,
Place(shore). Actions are the plain three, forward/turn/feed (`manifest.rs:1499`): **no
sprint, no hunker**.

Veilhound modules (`manifest.rs:1000-1018`): Self, Contact4, Wet, Taste1, Cone3, SelfV3,
**Prey**, **Kin**, Carcass, Scent ×4, Tremor, **Social**, Place(den), Place(shore). Actions
are `SPRINT_ACTIONS`: the three plus `sprint_effort` (`manifest.rs:1670`). It has no Threat
block.

**Block contents** (`observe_v3.rs:176-260`):
- **Threat**: the record's **single best (nearest) threat** as 7 slots (valid, sin, cos,
  proximity, moving, still, fresh), plus a count `min(n,4)/4`.
- **Kin**: the best kin's 7 slots, the alarm flag (0 for the loftstrider) and the count.
- **Prey**: the best prey's 7 slots.
- **Social** (`observe_v3.rs:369-386`): `breeder, pup, target{valid, sin, cos, prox},
  feast{valid, sin, cos, prox}`. The target bearing is computed from the target's **true
  current pose** (`t.pose.x, t.pose.z`), not from any percept.
- **SelfV3**: stomach, latch, own wound, stamina, daylight, day phase.

**Option head.**
- `Manifest::option_head` is **0 for every lineage** (`manifest.rs:453-457`, set at
  `1302/1361/1423`).
- `resolve` computes `Resolved::option` from logits after the actions
  (`controller.rs:258-270`), but **nothing in the step reads `.option`**. The only reads
  are in tests (`controller.rs:1802`).
- No option menu exists in code. The plan (`design/training-plan-2026-10-08.md` §6 item 23,
  lines 740-757):
  - option 0 is own steering;
  - the seedporter, loftstrider and Veilhound get "stay with the troop or family" and the
    pack's "drive, flank, den, rest or play" skills, which are "to be written";
  - P8 splits `pack::steer` into callable skills
    (`design/brain-contract-v3-proposal-2026-10-08.md:377`).
- `herding::stay_with_group` is the only skill written.
- `Steer` is recorded per body (`fauna.steer`, `step.rs:2153`) and in the tap. It is the
  natural label for cloning an option, but there is no mapping from `Steer` to option
  today.

**What a brain body cannot see that the heuristics use:**
- **pack**:
  - packmates' prey tracks within 10 m (`known()`);
  - the shared target choice, its score and the prospect terms: true positions of all prey
    kin, the target's pry and opened state, and whether its record is aware of any pack
    member;
  - the chase clocks, `shunned` and `abandoned`;
  - the driver/flank roles, computed from the true positions of all participants;
  - `window_body` (the true sum of nearby participants' sizes);
  - the prey's scan state and running state, which `stalk` reads from the prey's record.
- **vigil**:
  - more than one threat (the brain gets the best one and a count, while `group_min`
    counts pressing threats within 6 m);
  - `pressing` and `calm`, which the brain must infer from moving, still and posture;
  - the flight point and clocks;
  - `shelter_adults`, which iterates `fauna.animals`, gated by a fresh kin track.
- **herd**: only the nearest kin. The heuristic averages fresh kin tracks, but for the
  loftstrider that is also one.
- **Brain bodies get none of these reflexes** (section 1). A brain loftstrider that should
  "stand its ground" or "run with the family" must learn it from the Threat, Kin and Self
  slots alone. A brain Veilhound sets no pack target: only heuristic members `commit`. It
  still reads Social, which shows the target its heuristic packmates chose.

---

## 9. Cost

**Body counts on the wide world** (`terrarium-wide.toml`).
- The ring is 640 × 96 voxels at 0.125 m, 80 m × 12 m (`terrarium-wide.toml:21-25`).
- Megafauna are per world: **2 loftstriders**, **1 pack of 4 Veilhounds**
  (`terrarium-wide.toml:141-145, 304-309`). With dispersal on, the population targets are
  one loftstrider herd of 3–6 and one pack of up to 4 adults plus pups.
- Other lineages scale with dry area over `reference_land_m2` 346 (`terrarium-wide.toml:663-667`).
  On a 2.5× wider ring, the per-desk counts (frondgrazer 8, capgnawer 6, snail 8, bellwing
  6, seedporter 4, plus shredders and lanternjaws) multiply by roughly 2.5. That is an
  estimate; no run was made. Expect on the order of 100–200 founded bodies before
  breeding.

**Per-tick structure.**
- **Remote senses.** One fifth of live bodies per tick, parallel. Each costs a bin gather,
  a sort, and ≤ (8 + kin_tracks) rays. The ledger counts pairs and ray cells
  (`perception.rs:755-759`: `percept_pairs`, `percept_ray_cells`).
- **Observations.** One fifth of founders per tick, parallel.
- **Reflexes.** Serial, per due body. Most read only their own record (O(≤32 tracks)).
- **Whole-world scans in serial code** (each is O(N) over `fauna.animals`):
  - `pack::known` per call (`pack.rs:807-824`);
  - `prospect` twice per candidate: `near()` (`pack.rs:883-890`) and the awareness scan
    (`pack.rs:902-906`);
  - `straggler` per candidate (`pack.rs:1196-1202`);
  - `at_hand` and `nearest_member`;
  - `prey_in_capsule` per due carnivore holding feed (`step.rs:3652`);
  - `window_body`;
  - `shelter_adults` per juvenile frondgrazer sample and per strike (`vigil.rs:955-962`);
  - `vigil::pry`, O(hunkered × N) every tick (`vigil.rs:1474-1495`);
  - `alarm_wakes`, O(sleepers × running), only when someone sleeps and someone runs.
- **Route fields.** `choose` builds a route distance field per call
  (`routes.distances(from, reach_steps)`, `pack.rs:1305`). Graphs are cached per lineage
  (`fauna.pack_routes`) and refreshed every 60 s.

**The spatial index.**
- `Bins` (2 m) are rebuilt per sense pass (`perception.rs:524-573`) and used only there.
- `LiveGrid` (`live_grid.rs:1-21`) is the voxel occupancy for rays and cones, patched
  between stages. It is not a neighbour index.
- No pack or vigil rule uses a spatial index. They scan `fauna.animals`.

**`hot_path_tests.rs`** are **equivalence oracles**, not timing guards. They check the
mouth-reach cache, the dense detritus field, the light memo and the motion sweep against
verbatim old code (`hot_path_tests.rs:1-19`). No test pins a time budget. Profiling is the
`profile` feature's phases (`FaunaRemote`, `FaunaWindows`, `FaunaObserve`, `FaunaReflex`,
`step.rs:1539-1765`).

**Implications** (structural reasoning, not measured):
- A rule reading only the body's own percept record (kin and threat tracks) is O(32) per
  sample. That is negligible next to the rays already cast.
- One extra O(N) world scan per due body is O(N²/5) per tick. At N ≈ 200 that is about
  8000 cheap distance checks a tick, still small against ray marching. Today only the pack
  (4 bodies) pays it.
- A per-group pass once per tick (label herds, as `herd_labels` already does) costs
  O(n_lineage²) for a 3–6-body herd: trivial. It must run serially, like `pry`, between
  `act` and the hits, or in `begin`, to keep the serial-write discipline.

---

## 10. Conventions a new mechanism must follow

**Knobs.**
- Per-lineage behaviour tables are `Copy` serde structs with `#[serde(default)]`, and a
  `*_of(founder)` constructor carrying the placeholder rationale: `vigilance_of`,
  `hunker_of` in `vigil.rs`; `herding_of`; `sprint_of`; `rhythm_of`. They hang off
  `FounderPhysiology` and are wired in `FounderPhysiology::placeholder`
  (`body.rs:2011-2024`).
- World tables live in `knobs.rs` (`PredationConfig` `knobs.rs:113-149`,
  `PerceptionConfig` `knobs.rs:909-965`).
- The `Pack` table uses per-field `serde(default = "pack_…")` functions that read
  `Pack::CHORISTER` (`body.rs:1090+`).

**Registry.**
- Every numeric leaf needs a row in `registry.rs`. Switches (bools, enums) have no row; the
  code says so in comments (`registry.rs:336-375`, e.g. "`hunker.enabled` is a switch: no
  row").
- Coverage is enforced by `crates/cubarium-voxel-fauna/tests/registry.rs`, using
  `crates/cubarium-voxel/tests/support/registry_spec.rs:46` ("Test 1, coverage: every
  numeric leaf …"). A missing row fails with "`{leaf}` has no row in the parameter
  registry" (`crates/cubarium-voxel/src/registry.rs:589`).

**Save schema** (`snapshot.rs:17-25, 150-173`, currently `SCHEMA = 78`).
- A new knob is **not** a bump: the config is a named-field document beside the state.
- New saved state **is** a bump: an `Animal` field, a `Percept`/`Vigil` field, a ledger
  line, or a `PackRecord` field.
- Old worlds are refused, never migrated. Per-tick session state can stay unsaved
  (FxHashMaps on `Fauna`, as `fauna.steer`, `route_flee` and `pressed` are), pruned every
  64 ticks (`perception.rs:700-711`).

**Pose and renderer.**
- `pose.rs` is geometry only. The renderer reads `Animal` fields directly: `hunkered`,
  `state`, `strike`, mobility (`crates/cubarium/src/voxel/animal.rs:1025`,
  `crates/cubarium/src/voxel/figure.rs:1395-1418`).
- Rig clips are chosen by state name: `cropping`, `asleep`, `idle`, `hunker`, each with an
  eased weight (`figure.rs:1405-1411`).
- The frondgrazer's hunker is drawn as its standing model posed toward a baked hunker
  model, or as the `hunker` rig clip (`crates/cubarium/src/voxel/hunker.rs:1-35`).
- A new visible behaviour (stand-off, rear, kick, a calf tucked in) therefore needs:
  - a saved, readable `Animal` field or a derivable state;
  - a renderer weight in `figure.rs` with the never-pop easing;
  - a rig clip in the lineage's Blender rig.
- `calling_ticks` shows the risk: a state can be saved and never drawn.

**Diagnostics and census.** Counters only; no rule reads them.
- `VigilLedger` (`vigil.rs:1554-1594`): scans, flees, alarm_bolts, orients, aware and
  unaware strikes, stalk and rush samples, hunkers, bunches, runs_to_hunker,
  `grazer_kills_by_posture`, glances, `kills_juvenile` / `kills_adult`, opens.
- `FaunaLedger` pack lines (`lib.rs:3314-3379`): `pack_chases`, `pack_chase_kills`,
  break-offs, switches, declined, out_of_reach, `pack_chases_by_sense`.
- Hunt figures: `funnel[hunter][prey][sensed…eaten]`, `hits_by_founder`,
  `dodges_by_founder`, `counters_by_founder`, `counter_wound_by_founder`.
- Sleep: `alarm_wakes_by_founder`, `rest_bouts_by_founder`.
- `PackEvent` log (`lib.rs:4416-4450`): ChaseStart (participants, taker, score), ChaseEnd
  with `ChaseEnd` reasons, FeastEnd.
- `census_readout.predation` (`step.rs:6789-6791`); `herd_tally` and `mean_nearest_kin_m`
  (`herding.rs:36-37`).
- The existing study harness is `crates/cubarium/examples/hunter_study.rs`. It already
  tallies hunker, run and kill postures.

---

## 11. Not on the list but relevant

- **The pack tactic runs only on heuristic Veilhounds, and so does the flee/vigil rule for
  prey.** In a mixed world (some brains, some heuristics), heuristic members carry the
  whole pack record.
- **The strike ignores the pack target.** Any prey in the capsule is struck, so a pack can
  wound a bystander calf on its way to a chosen adult. Pry is target-gated under HERD;
  strikes are not.
- **The flank geometry is static** (target ± 1.5 m across the driver's line, by rank). It
  does not track the prey's escape heading. Prey flight is straight away from the nearest
  threat. Nothing tests flankers turning a fleeing animal.
- **FEAR versus the pack's rest.** A pack resting at its den (posture REST or ASLEEP,
  facing away) is watched, not feared, inside the 3 m margin. A loftstrider family can
  graze beside a sleeping pack today.
- **`sleep_hearing` makes the alarm cross-lineage for sleepers only.** A fleeing
  frondgrazer wakes a sleeping loftstrider, but an awake loftstrider ignores both a fleeing
  frondgrazer and a fleeing kin.
- **`window_body` and the strike's damage both use `size = body / mass_scale`.** The wide
  preset gives the two lineages different `mass_scale` (9000 against 4500), consistent with
  the code ratios. A preset that rescales one without the other silently changes the
  window and the hits-to-kill.
