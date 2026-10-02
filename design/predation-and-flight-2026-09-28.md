---
design_status: proposal
last_reviewed: 2026-09-28
decision_refs: []
---

# Predation and flight: two substrates

Answers `design/handoffs/predation-and-flight-2026-09-23.md`. Model mechanics, not
species tuning: every number here is a placeholder for `design/backlog.md`. Body plans
are not needed (the lanternjaw's is still open); sizes come from the agreed ladder
(`design/organism-large-animals-2026-09-23.md` §7). Package briefs:
`design/handoffs/voxel-predation-2026-09-28.md` (P1, P2) and
`design/handoffs/voxel-flight-2026-09-28.md` (F1–F3).

## 0. Wrysk's decisions

**Wrysk, 2026-09-28: "decisions sound good"**: all six accepted, with decision 4
reshaped by his lanternjaw life cycle (§2.13) and the chorister notes in §2.14.
Later the same day he chose the footfall hatching trigger and agreed the proposals in
§2.1, §2.3 (success targets), §2.9, §2.13 and §2.14, including choristers pushing
lanternjaws off carcasses.

1. **Damage: wounds, not one-hit kills.** A hit adds to a wound stock scaled by the
   size ratio; small prey die in one hit, an adult grazer takes three (a hard target
   for a lanternjaw, Wrysk 2026-09-28), a loftstrider needs a pack. Wounds heal slowly and the healing costs reserve. *Alternative:* every
   successful strike kills (simpler, but a pack and a big animal can't exist).
2. **Carcasses last hours.** Carrion decay goes from an e-fold of 200 s to about 1 h, so
   one kill feeds the killer, other lanternjaws and shredders over time. This also
   lengthens every other corpse and feeds shredders more; it is a live-rule change.
3. **The lure works through a glow sense.** Every sighted animal gets a glow channel in
   its cone; glowing food (bloomcrown cores, glowcap caps, latticevine bells) makes
   glow worth approaching, and the lure exploits that. Whether prey fall for it is
   learned, not scripted. *Alternative:* the lure is paid decoration only.
4. **Predators enter by introduction, never spontaneously.** A viewer/CLI control adds
   1–2 lanternjaws on prey-rich ground (Working policy, 2026-09-14), and a world config
   may introduce them once at a set minute (default 60) after prey establish.
   *Alternative:* seed them with the founders at minute 0.
   **Amended by Wrysk 2026-09-28:** lanternjaws arrive from **dormant hidden eggs** that
   hatch when conditions are right (§2.13); a fresh world is founded with a buried egg
   bank, not adults. The controls stay as development tools.
5. **Downpours ground flyers.** Showers make flight dearer; a downpour forces landing,
   so bellwings shelter under crowns and overhangs. *Alternative:* cost only.
6. **Nectar is a flower product, not the seed parcel.** Latticevine flowers already
   hold nectar in code; bloomcrown gets a small nectar stock of its own. Reading the
   parcel would make the pollinator a seed predator.

Everything else below is a routine call, stated with its visible effect, vetoable.

## 1. What exists (main at 225bafbb)

- **Fauna:** two lineages, `Founder::{Blind, Browser}` (littershredder 0.375 m,
  frondgrazer 0.75 m). State = `Cropping | Walking | Resting | Climbing`, a record of
  the last act. No health, no animal–animal contact, no airborne position: every path
  forces a support face (`body.rs` `advance_candidate`, `step.rs` `terrain`).
  Reproduction is single-parent. Deaths: `Starved`, `Drowned`, `Removed`; a corpse goes
  whole to the carrion pool on the death face (`step.rs` ~1946).
- **Controller:** GRU, 3 actions `forward`, `turn`, `feed`, every 0.25 s. Manifest v2
  is per lineage; any input change moves its digest and refuses trained centres.
- **Cone:** 45 rays, range 2 m. An animal occupies **one cell** above its standing face
  for sight; the policy sees only `body` and `body_proximity` per sector: no kind, no
  size.
- **Carrion:** a ground pool per support face, `take_carrion`, decay 0.005 /s (e-fold
  200 s), respired, mineral to soil. Only shredders eat it.
- **Latticevine** (`flora/src/cover.rs`): face cover with spur phases; `CoveredFace`
  holds `nectar` and `fruit` with `take_nectar` / `take_fruit`. Nothing calls them.
- **Lanternberry** has no fruit product (fruit = the parcel). Bloomcrown has no nectar.
- **In flight on branches:** WX4 circadian (`State::Asleep`, `sleep_upkeep` 0.5,
  `wake_hunger`; branch `worktree-agent-af081de32b2b849ee`, fauna schema 18); animal
  lifespan `Departure::Aged` and plant grazing defence (branch `live-train`, also fauna
  schema 18); live-world training (same branch). None is on main.
- **Legacy** `cubarium-core` lanternjaw: Stalking → Windup → Strike → Handling →
  Recovering, `capture = clamp(0.65·S_h/(S_h+S_p), 0.10, 0.75)`, strike paid up front,
  prey moved whole into the hunter's gut (no carcass). Worth borrowing the phases and
  the up-front cost; not the gut transfer.

## 2. Predation

### 2.1 Who eats whom

A carnivore lineage carries a **prey list** (founder lineages) and a **size window**:
prey body at most `prey_max_ratio` × its own (lanternjaw 1.5). The list for the
lanternjaw: frondgrazer, littershredder; later capgnawer, grounded seedporter, newborn
loftstrider. As a mesopredator its staple is shredders and young grazers; an adult
grazer is inside the window but hard (three hits, §2.4), taken mostly asleep, wounded
or starving (Wrysk, 2026-09-28).
Carrion is always on a carnivore's menu (the roster's fallback). A juvenile predator's
window follows its own body, so young lanternjaws take shredders and juvenile grazers.

### 2.2 Seeing and being seen

- **Bodies have volume.** For sight and contact an animal is its footprint disc × its
  current height, filled into the cone's occupancy, not one cell. A 0.45 m lanternjaw is
  about 4 cells tall at 0.125 m. (Encounter contract §1 already names this as the
  target.) Movement still ignores other animals: no collision solver.
- **Kinds per observer.** A ray that hits a body reports it as **prey** (in my prey
  list and window), **threat** (I am in its list and window) or **other**. One lookup per
  hit on the founder pair. Manifest v3 replaces `body` with these three.
- **Still threats hide.** A threat reads as *threat* only if it moved faster than
  0.1 BL/s within the last 2 s, or is within 0.5 m. Otherwise it reads as *other*.
  Ambush then pays: a still lanternjaw is a harmless shape until it is too late.
- **The blind shredder feels tremor.** A new two-channel module (left, right): the sum
  over bodies within 1.5 m of `mass × speed / distance²`. A still predator makes none;
  a walking one warns. This is how blind prey escape, and why stillness matters for them
  too.
- **Concealment is free.** Foliage, pools and terrain already occlude rays. A
  lanternjaw in a frond stand or behind a ledge is hidden by the existing march.
- **Glow (decision 3).** Rays report emission of what they hit: bloomcrown core,
  glowcap caps, latticevine bells, the lure, later the chorister's flash. The values are
  the per-species emissive parts the renderer already has (package L checkpoint 3),
  moved into model data so the simulation, not the picture, owns them. The lure sits
  at the lanternjaw's tail tip (above the body, ahead of the jaw), so a lanternjaw in
  cover can show its lure while its body stays hidden. A lit lure costs reserve per
  second (`lure_power`). The trained lanternjaw lights it with a fourth action, `lure`;
  its heuristic lights it while waiting.

### 2.3 The strike

The strike is the carnivore's **`feed`**: the action layout stays `forward, turn, feed`
(plus `lure` for the lanternjaw). On a due tick with `feed` held:

1. If a prey body's cylinder intersects the **strike capsule** (from the jaw, forward
   `strike_reach` = 0.6 L, over the jaw band `[0, 0.6 H]`), a strike begins.
2. Otherwise, if carrion is under the mouth, it eats carrion (the existing detritus
   mouth, carrion only).

A strike has three phases, all visible poses:
- **Windup** 0.15 s: crouch; no movement.
- **Lunge** 0.25 s: the body moves forward up to `strike_reach` under the ordinary step
  rule (it cannot lunge off a cliff or through rock), then the contact test runs against
  the prey's cylinder **where the prey is now** (prey that moved away is missed by
  geometry).
- **Recovery** 3 s: no strike, half speed.

The strike cost (100 s of the striker's awake upkeep) is paid at the lunge, hit or miss,
as in the legacy hunter. A hungry lanternjaw can't afford to strike at everything.

**Contact then dodge.** Contact is geometry. On contact, the prey dodges with
`agility × awareness × aspect × condition`:

| Factor | Values (placeholders) |
| --- | --- |
| agility (species) | frondgrazer 0.5, littershredder 0.3 |
| awareness (prey state) | walking 1.0, awake resting 0.7, cropping 0.4, climbing 0.4, asleep 0 |
| aspect | struck from its rear half (outside its cone) 0.5, else 1 |
| condition | its reserve fraction, clamped to [0.5, 1] (a starving animal is slow) |

So a sleeping grazer is always hit, a cropping one struck from behind almost always, a
walking one watching the lanternjaw half the time.

**Success targets** (calibration, not rules; Wrysk: not a high rate, as in real
predators): a lanternjaw strike on its staple prey succeeds about 30–50 % of the time
(an ambusher at close range); a chorister hunt, a whole chase, about 15–25 %, as for
wolves and lions. Agility and awareness are tuned against these once P1 measures them.

**Concurrency.** Strikes are resolved as events after the act phase (`Hit { prey,
damage }`), applied in one serial pass. No animal writes another during the act.

### 2.4 Damage: the wound stock (decision 1)

`Animal::wound` in [0, 1]. A hit adds `strike_power × (S_pred / S_prey) × (1 − armour)`;
at 1 the prey dies (`Departure::Killed`).

| lanternjaw (body 0.035), power 0.7; frondgrazer armour 0.25 | |
| --- | --- |
| on an adult grazer (0.05, ratio 0.7) | 0.37 per hit: three hits |
| on a juvenile grazer (0.015, ratio 2.3) | kill |
| on an adult shredder (0.0125, ratio 2.8) | kill |

A wounded animal is slower (speed cap × (1 − 0.5·wound)). Wounds heal at 0.5 per 30 min,
paying reserve (`heal_cost` per unit wound per unit body); a starving animal doesn't
heal. The alternatives were one-hit kills (no big prey, no pack) and a drain (the
predator eats a living animal, which reads badly and bypasses the carcass). Mass is
untouched by a wound: an animal that escapes keeps all of itself.

### 2.5 Death and the carcass

`Killed { by: Founder }` joins the departures, counted by the ledger and census. The whole
body (structure, reserve, gut, any gestation settled as today) goes to the carrion pool
on the prey's face, exactly as other deaths do.

**Decay (decision 2):** `carrion_decomposition` 0.005 → 0.00028 /s (e-fold 1 h, half a
day on the 2 h clock). The 200 s placeholder was never measured; it spends a grazer
before a lanternjaw has finished its first mouthful.

**The carcass is drawn.** The ground pool gains a small carcass record (lineage code,
heading, organic at death) so the presenter can draw the dead animal (interim: its model
lying flat and darkened), then remains below 30 % (dossier D2's "collapsed body, then
bone"). The record carries no mass; the pool does.

### 2.6 Feeding from a carcass

No handling state. The killer eats from the pool with its mouth like any carrion eater
(bite, yield, reserve room). Handling time is how long filling its reserve takes. It can
be joined at the edge by another lanternjaw or by shredders, and one carcass feeds them
all over the hour. The lanternjaw's `reserve_cap` is 1.0 (an adult grazer fills it), so
it gorges once and then rests.

### 2.7 Defence

- **Flee** is behaviour: prey brains get threat and tremor channels (v3); the prey
  heuristics get a flee rule (turn away from a seen threat within 3 m, or from tremor).
  The lanternjaw cruises at 0.6 BL/s (0.6 m/s), slower than a fleeing grazer
  (0.75 m/s): it wins by the lunge, not the chase. The chorister will be the runner.
- **Awareness and condition** (the table above) make the vulnerable states count:
  asleep (WX4), cropping, climbing, starving, wounded, young.
- **Armour** per lineage: a damage multiplier (the grazer's saddle shield 0.25,
  shredder 0).
- **Counter-strike seam:** an optional `Counter { reach, damage, cost }` per lineage,
  fired when struck, wounding the attacker. Unused until the loftstrider's kick.

### 2.8 The long, cheap rest

Build on WX4's `State::Asleep`, kept general as its brief asks: a rest with a reason and
an upkeep factor.

- **Sleep** (WX4): out of phase, `sleep_upkeep` 0.5.
- **Satiated rest** (new): a carnivore whose reserve is at least 0.9 of its cap rests in
  any phase at `torpor_upkeep` 0.25 and wakes at 0.5 of its cap (or by WX4's hunger rule).
- Both skip the controller. A resting animal has awareness 0 (it is prey too, later).

With the lanternjaw's own upkeep a fifth of the grazer's (the biosphere's "low resting
demand"): an adult grazer carcass fills its reserve (0.035); resting it burns 1.75e-6 /s,
so that meal carries it about **5.5 h, nearly three days on the clock**, and a shredder
about 1.4 h. Awake it burns four times faster. Wrysk's "one big meal, then a long rest" is these two numbers; a hibernation
arm is a lower `torpor_upkeep`, no new rule.

### 2.9 Entering a world (decision 4, amended)

No spontaneous appearance. Lanternjaws arrive from their **egg bank** (§2.13): a fresh
world is founded with a few buried clutches in prey-rich components and no adults, so
the first lanternjaws hatch when the prey have established. Development controls stay:
`--introduce lanternjaw:N` (adults, full stores) and a viewer key that buries a clutch.
A world with no prey component founds no clutches and says so.

### 2.10 The pack seam (chorister later, not built)

Wounds from several attackers add, and any hit can be the kill. That is the whole seam
for a pack taking a loftstrider. Reserved, not built: a `Call` module (a broadcast cue
and a call output) in the manifest's module list.

### 2.11 Sensing contract v3 and training

One retrain round takes every input change at once, so centres are refused once:

- all lineages: bodies with volume; `prey / threat / other` in place of `body`; glow
  (decision 3); `wound` and `daylight` in Self (the WX4 note);
- Blind: tremor;
- lanternjaw: its own manifest (Browser's cone plus a carrion chemical field, `lure`
  action).

Training: the lanternjaw's heuristic (wait in cover near prey food, lure lit; strike when
prey in reach; eat carrion; rest when full) is the teacher; clone, then ES. Episodes are
**live worlds with both sides** (the live-train loop: per-animal variants, persistent
worlds, every lineage's own update). Round one trains the lanternjaw against heuristic
prey (they flee by rule); round two trains all lineages together on v3. If the prey collapse or the predator starves in every world, the
loop's population bounds already reseed; the readout says how often.

### 2.12 The lanternjaw in numbers (placeholders)

| | |
| --- | --- |
| adult | 1.0 × 0.25 × 0.45 m, a slim scorpid build; `body_max` 0.035 (0.7 of the grazer's) |
| upkeep | `maintenance_per_s` 0.0002 (a fifth of the grazer's) |
| reserve | `reserve_cap` 1.0 |
| pace | cruise 0.6 BL/s; lunge 0.6 L in 0.25 s |
| activity | Nocturnal (WX4 pick) |
| strike | windup 0.15 s, recovery 3 s, cost 100 s of awake upkeep, power 0.7 |
| lure | `lure_power` = its awake upkeep while lit (doubles its burn) |
| rest | satiated at 0.9 of cap, torpor 0.25, wakes at 0.5 |
| young | buried eggs (§2.13); lifespan 3× the grazers' if lifespan lands |

### 2.13 The lanternjaw's life cycle: a dormant egg bank (Wrysk, 2026-09-28)

Wrysk: adults lay **hidden eggs** that stay **dormant until conditions are right**,
then hatch. That gives the cadence of predators arriving without tuning them as
constant tenants. How an egg senses the moment is open; the proposal is below.

- **Laying.** A lanternjaw with sustained surplus (the ordinary reproduction gate) lays
  a clutch of 3 buried in soil or litter on the face it stands on, paying each egg's
  organic matter plus a small reserve. The shredder's clutch machinery is the base.
  Buried eggs are invisible to every cone and edible by nothing (an egg-digging
  predator is a later option).
- **Dormancy.** An egg pays a tiny upkeep from its own reserve and loses viability
  slowly (median about 12 h, six days on the clock), so a bank can outlast a prey
  crash but not forever. A dead egg goes to the litter of its face.
- **Hatching trigger (Wrysk chose this, 2026-09-28):** an egg counts **prey footfall**: distinct prey bodies
  that pass within 2 m, heard through the same tremor rule the shredder uses (§2.2). It
  hatches at dusk once the count over the last day reaches its own threshold (drawn per
  egg, mean 6, spread wide, so a clutch hatches over several good spells rather than at
  once), **and** no adult lanternjaw is within 4 m (adults suppress hatching, so a
  living hunter's patch isn't flooded). Rejected: a rain pulse (blind to whether prey
  exist), the smell of carrion, age alone.
- **Hatchlings** are 0.46 L, eat shredders and carrion, and disperse from the clutch.
- **The cycle this makes:** hunters eat down the prey, starve or age out; their eggs
  wait; the prey recover and walk over the bank; a new generation hatches. The founding
  bank (§2.9) is the first turn of it.

### 2.14 The chorister's life cycle (notes for its own package, not built)

Wrysk, 2026-09-28: live birth; the young are costly and raised by the parents, like
wolf pups or lion cubs, and can't hunt until they reach an age; the pack keeps a den
and is a long-term tenant; hunts succeed rarely; metabolism tuned so they don't
massacre the prey; larger than the lanternjaw (1.4 m + tail against 1.0 m, per the
ladder).

Agreed by Wrysk, 2026-09-28 (numbers stay placeholders):
- **Den:** a covered site (under an overhang, in a hollow or grotto) that the breeding
  adult picks; pups stay within a few metres of it until independent.
- **Provisioning:** an adult that returns to the den with a full reserve passes some of
  it to the pups (a paid transfer, efficiency about 0.7; visible as the adult standing
  over the pups). Pups also eat at carcasses the pack is feeding on.
- **Independence age:** pups can't strike before it (about one clock-day); after it they
  follow the adults and hunt with them.
- **One breeding animal per pack**, one litter of 2–4 at most every few clock-days and
  only on sustained surplus. That, not hunger, is what bounds the pack: its size is set
  by how many pups its hunting can raise, like wolves.
- **Territory:** a pack avoids another pack's den radius, so the world's area bounds the
  number of packs (a 20 m ring likely holds one).
- **Cost of a hunt:** chasing is dear (the motor's cost rises steeply above cruise), so
  choristers abandon most chases; together with dodge that gives the 15–25 % target.
  They gorge and rest like the lanternjaw, at a higher upkeep.
- **Risk:** long-term tenants with no egg bank can die out in a small world. The
  development control re-introduces a pack; bigger worlds (the desktop terrarium) are
  where they belong first.
- **Choristers push lanternjaws off carcasses** and may kill them (wolves and coyotes):
  the lanternjaw is in the chorister's prey list and the chorister is a threat to it.
  Cheap once kinds exist; it pushes lanternjaws toward the edges and the night.

## 3. Flight

### 3.1 Locomotion modes

`Mobility` gains a mode: `Ground` (today), `Wall` (the shredder, today), `Climb` (plant
stems and latticevine faces, new), `Fly` (powered), `Glide` (unpowered), `Perched` (at
rest off the ground). A lineage enables the modes it has: bellwing `Fly + Perched`,
seedporter `Climb + Glide + Perched`, shredder `Wall`.

### 3.2 Position and air

- The pose gains `y_m`, the body's base height in metres. Walkers keep it on their
  support face; flyers move it freely. `site` stays the support face directly below (for
  pools, ledgers, census).
- **Air is any cell that is not solid and not water.** A flyer is a sphere of radius
  half its span for collision against terrain and water only.
- **Foliage is passable but slow:** inside a crown's foliage layers a flyer's speed cap
  halves. Tiny bellwings slip through leaves; nothing is walled by a crown.
- **Ceiling:** at most `band_max` (bellwing 2.5 m, the 20 voxels) above the support face
  below. The 4–20-voxel band in the roster is behaviour, not a rule.
- Flying bodies count in the cone occupancy like walkers.

### 3.3 Powered flight (bellwing)

- A fourth action, `lift` [−1, 1] (vertical effort), beside `forward`, `turn`, `feed`.
  Forward 0 and lift 0 is a hover.
- **Cost per second airborne:** `flight_factor` (4) × basal, hover 5× basal, plus lift
  work on the way up (`climb_work` per metre × body). Descent is free. Takeoff costs
  1 s of flight.
- Cruise 8 BL/s (1 m/s at 0.125 m).

### 3.4 Landing and perching

A flyer lands when its body touches a perch while descending or while sipping. Perches:

- the top of any support face (ground, ledges);
- the top of a stand's crown slab (a bloomcrown core), from the crown cache;
- a latticevine-covered face (clinging to the wall);
- the underside of a crown slab or an overhang (hanging, the frond-underside roost).

A perched animal pays basal (or sleep upkeep at night). If its perch goes (a crown eaten
or dead, a face uncovered) it takes off, or drops to the ground if it can't pay.

### 3.5 Climb, then glide (seedporter)

- **Climb** extends the shredder's wall machinery to plant surfaces: a stand's stem (a
  vertical line from its site to the crown's base) for stands at least 1 m tall
  (vaulttree, lanternberry, umbrellafrond), and latticevine-covered faces. Half walking
  pace, the shredder's climbing cost factor. At the crown the climber may perch in it.
- **Glide** from any height at least 1 m above the landing surface: no thrust, descent
  at `glide_ratio` 2.5 (horizontal : vertical), speed 3 BL/s, steered by `turn`. It lands
  on the first support face, crown or covered face it meets. Cost 1.2 × basal.
- So a seedporter climbs one grove's tree and glides to the next; the SP-B pattern.
- Models for these modes exist (2026-10-01; the reimagining doc's §Seedporter): specimens
  `seedporter` (climbing), `seedporter-stand` (perched), `seedporter-ground` (walking) and
  `seedporter-glide`. F3's presenter picks one by mode; nothing does yet.

### 3.6 Senses in 3D

A flyer's fan looks down: pitches −80, −50, −20, +10, +40, range 4 m, the same three
sectors. Self gains `altitude` (height over the support face below, / `band_max`) and
`ceiling` (clear height above, capped). A **bloom** class in the cone marks nectar that
can be taken (latticevine faces in flower, bloomcrown cores with nectar), beside foliage,
the body kinds and glow.

### 3.7 Food (decision 6)

- **Nectar** is a product: latticevine faces in flower already hold it
  (`take_nectar`). Bloomcrown gains a `nectar` stock on adult stands, paid from reserve
  in daylight up to a cap; unvisited nectar stays. It is energy-rich and mineral-poor
  (`nectar_mineral_fraction` small but not zero: pollen lumped in until pollen exists;
  the assimilation rule `min(yield·organic, mineral/n_tissue)` would starve a pure
  sugar eater).
- **Fruit:** latticevine fruit exists (`take_fruit`); lanternberry gets a fruit
  product in the seedporter's package.
- **Later seams:** a sip records the last bloom visited (pollen carriage); a fruit meal
  records seeds in the gut (the vine's only long dispersal).

### 3.8 Weather and night (decision 5)

- Shower: flight cost × (1 + rain rate / R₀).
- Downpour: no takeoff; a flyer aloft descends to the nearest perch, preferring one with
  something overhead. Visible: bellwings under fronds and overhangs through a storm.
- Night: bellwing Diurnal (WX4 pick), perched asleep. Wind drift is later.

### 3.9 Bellwing and seedporter in numbers (placeholders)

| | bellwing | seedporter |
| --- | --- | --- |
| body | 0.125 m, span 0.375 m, `body_max` 0.001 | 0.5 × 0.25 × 0.2 m, `body_max` 0.012 |
| modes | Fly, Perched | Climb, Glide, Perched |
| pace | cruise 8 BL/s | walk 1 BL/s, climb 0.5, glide 3 at ratio 2.5 |
| activity | Diurnal | Diurnal |
| food | nectar | latticevine fruit, lanternberry fruit |
| predators | none yet | lanternjaw, when grounded |

## 4. Order of work

See the two briefs; each package is tests first (a separate high-effort pass), then the
implementer, in its own worktree, measured over seeds 1–8 on three presets.

1. **P1 predation substrate**, with the lanternjaw's body and heuristic so it can be
   seen and measured. After WX4 is on main. Heuristic controllers only.
2. **P2 contract v3 and training**: every manifest change at once, the lanternjaw's
   brain, prey retrained with threats. Needs live-train on main (Wrysk's call).
3. **F1 flight substrate**, with the bellwing's body, heuristic and bloomcrown nectar.
   After P1 merges (same files).
4. **F2 bellwing brain**: its own manifest; joins the next live-train round.
5. **F3 seedporter**: plant climbing, glide, lanternberry fruit, gut seeds.
6. Later: capgnawer, loftstrider (its own package), chorister on the pack seam.

## 5. Not decided here

Population viability (deferred by Wrysk; §2.8 keeps the long rest open); the
lanternjaw's body plan and every look (art direction; all poses here are interim);
every number above (backlog); the chorister's pack controller; pollen and gut seeds
beyond their seams.
