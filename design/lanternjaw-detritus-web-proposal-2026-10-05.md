---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The detritus web and the small-prey tier: roles that hold each other in balance

**Status: proposal.** This is not canon and decides nothing. It changes no code.

**Wrysk, 2026-10-05:**
- "i would hope that between glowcaps, capgnawers, littershredders, and lanterjaws, the general
  poulation of each and the litter itself would stay in some balance."
- "dunno if im 100% on lanternjaws being a ambush predator with slow metabolism. i think they can
  be a little more active and focus on the smaller prey which can easily have population booms.
  (i mean ideally the shredders and capgnawers would and would have fast reproduction along with
  the snails)."

**The rule this follows** (`WORKING_POLICY.md`, "Balance through roles"): an imbalance is first a
question about roles. For each stock that grows or shrinks without check, ask what would thrive
on it, whether that role exists here, and why it isn't answering. Rate tuning comes last.

## In short

- **Litter is already held**, but not by animals. Decay removes it at a fixed rate (e-fold about
  24 min), so the stock follows plant production. Animals take 1/2,000 to 1/20,000 of the
  plant matter shed. No affordable number of shredders could hold litter, so their job is to **follow** it.
- **Glowcap grows without check.** Stands rise 3–4× in six hours on the terrarium and are still
  rising. Capgnawers eat 1–2 % of the standing cap tissue a day.
- **The small detritivores never breed.** Littershredder, capgnawer and ripple-snail had
  0–2 births per tank in 4 h on both maps. They are well fed (B 2–6) but grow too slowly: at the
  rates measured, a hatchling would need 2–5 lifespans to reach laying size.
- **The cause is behaviour, not food.** They find food in only a fifth of their hungry awake
  time (access 0.18–0.27 on the terrarium). The blind forager never slows on food. The breeding
  rule then asks for a full-size adult and lays one egg.
- **The lanternjaw does not establish.** Every lanternjaw in the runs was a starving juvenile
  (reserve 2–4 %). Its heuristic waits at crowns, where only grazers too big for it feed. It
  never goes where the shredders are.
- **Proposal:** make the small detritivores r-strategists through behaviour first:
  - stay on food;
  - forage whenever their own face is dark;
  - breed at part size;
  - size each clutch to the surplus;
  - limit their own crowding (nest sites, egg cannibalism).

  Make the lanternjaw a **mobile sit-and-wait hunter of small prey**: it moves between
  prey-rich patches, waits still, and its search follows the prey it catches most (switching).
  Keep the egg bank, the footfall trigger, the parasitic hatchlings, wounds and the lure.

---

## 1. The web as it is

### 1.1 Who eats what

```
 plants ──leaf fall, dieback, dung, spills──► LITTER ──decay k_d (the "microbes")──► CO2 + mineral
                                                │  │
                       dead wood ◄── dieback    │  └──► littershredder (litter, carrion)
                            │                   │  └──► capgnawer (litter at half yield)
                            ▼                   ▼
                         GLOWCAP (wood + litter in its box) ──caps──► capgnawer
 glassfilm (wet faces) ──────────────────────────────────────────────► ripple-snail
 carcasses ──► littershredder, lanternjaw, Veilhound (carrion)
 lanternjaw ──kills──► littershredder, frondgrazer, capgnawer, grounded seedporter
 Veilhound ──kills──► frondgrazer, loftstrider, lanternjaw, grounded seedporter
 lanternjaw hatchling ──rides and drains──► any non-lanternjaw host
```

Code (main at fe300df0):

| who | diet | eaten by |
| --- | --- | --- |
| litter | stock per soil face; first-order decay `flora.decomposition` (terrarium 6.9e-4 /s), `crates/cubarium-voxel-flora/src/step.rs:3487-3546` | littershredder, capgnawer (yield 0.25), decay |
| dead wood | decay `wood_decomposition` (6.9e-5 /s) | glowcap, decay |
| glowcap | saprotroph: draws `substrate_uptake_per_s × wood × moisture` from litter and dead wood in its box (`flora/src/step.rs:2206`); establishes wherever box substrate ≥ 0.02, no light gate (`flora/src/step.rs:3760-3772`) | capgnawer only (`body.rs:1250`). The shredder does **not** eat caps (`body.rs:1050`, Litter + Carrion); `design/ecology-booklet.md` is stale on this |
| littershredder | litter, carrion (`crates/cubarium-voxel-fauna/src/body.rs:1050`) | lanternjaw (`body.rs:833`) |
| capgnawer | caps (yield 0.5), litter (yield 0.25) (`body.rs:1226-1260`) | lanternjaw |
| ripple-snail | glassfilm only (`body.rs:1491-1530`) | **nothing** (on no hunter's list) |
| lanternjaw | carrion only; it kills to make carrion (`body.rs:1196`, prey list `body.rs:826-844`) | Veilhound (`body.rs:1475`) |

### 1.2 Life histories

Values on the terrarium and patch maps: code defaults with `config/desktop/terrarium.toml`'s
overrides. Org-units are at the shipped mass scale ×30. A world day is 2 h of sim time
(`body.rs:124`).

| | littershredder | capgnawer | ripple-snail | lanternjaw | frondgrazer (contrast) |
| --- | --- | --- | --- | --- | --- |
| hatchling → laying size → adult (org) | 0.094 → 0.375 → 0.375 | 0.160 → 0.638 → 0.638 | 0.036 → 0.146 → 0.146 | 0.102 → 0.63 → 1.05 | 0.15 → 0.9 → 1.5 |
| laying size / adult | **1.0** | **1.0** | **1.0** | 0.6 | 0.6 |
| young per breeding | 1 egg of 0.12 | 1 egg of 0.20 | 1 egg of 0.047 | 3 dormant eggs of 0.15 | 1 live young |
| hold / interval / incubation (s) | 549 / 1098 / 158 | 549 / 1098 / 183 | 549 / 1098 / 183 | 549 / 1098 / dormant | 549 / 1098 / 84 gestation |
| daily need (body/day) | 0.03 | 0.03 | 0.02 | 0.04 | 0.06 |
| lifespan (world days) | 6 | 6 | 6 | 12 | 8 |
| active | nocturnal | nocturnal | nocturnal | nocturnal | diurnal |
| forager | `BlindForager` on `Chem(detritus)` | `BlindForager` on `Chem(fungal)` | `BlindForager` on `Chem(film)` | `LanternjawAmbush` | `BrowserForager` |
| prey choice | richest food at the mouth | caps or litter at the mouth | film | anything on its list in its size window (prey ≤ 1.5× own body), seen in its cone | — |

Sources:
- daily needs and lifespans: `body.rs:124-146`;
- the shredder, lanternjaw, capgnawer and snail tables: `body.rs:968`, `:1131`, `:1226`, `:1491`;
- the egg rules: `crates/cubarium-voxel-fauna/src/lib.rs:1480` (eggs), `:1501` (dormant eggs);
- breeding eligibility (`body ≥ birth_body` and reserve ≥ package + floor for 80 % of the hold):
  `crates/cubarium-voxel-fauna/src/step.rs:2781`, `lib.rs:1526-1545`;
- the preset overrides: `config/desktop/terrarium.toml:53-118, 264-303`;
- the heuristics: `crates/cubarium-voxel-fauna/src/controller.rs:558`.

The lanternjaw's recruitment adds three steps:
- **Dormant eggs** hear the footfall of prey on its list within 2 m. They hatch at dusk once the
  count reaches each egg's own threshold (2–10), unless an adult is within 4 m
  (`fauna/src/step.rs:3312`, tremor `predation.rs:214`).
- **A hatchling rides a host** within 0.75 m and takes 0.09 of a body a day from it.
- **It drops off** at 0.24 of adult body (`lib.rs:1767-1772`).

### 1.3 Feedbacks

**Present:**
- **Bite size and the gut** give each consumer a saturating intake. A bite is `want × E/(E+K)`,
  and the stomach K is 0.02 of body, emptied over 120 s.
- **Breeding needs a surplus.** In principle births follow food. In practice they are a step
  that is never reached (§2.3).
- **Starvation and age** remove animals.
- **The lanternjaw's egg bank counts distinct prey footfall.** Its recruitment rises with prey
  traffic. A nearby adult suppresses hatching, which spaces lanternjaws.
- **Parasitic hatchlings need hosts**, so recruitment also rises with host density.
- **The size window follows the hunter's own body.**
- **A sated hunter starts no strike.**
- **Glowcap's uptake scales with moisture and its own wood.** It dies back when its box is
  exhausted. Bites raise its induced defence.
- **Blind prey flee tremor.** A walking hunter warns them; a still one doesn't
  (`fauna/src/step.rs:1278-1283`).

**Missing:**
1. **Detritivore births don't respond to food.** Laying needs a full-size adult, and maturation
   takes longer than a life.
2. **The forager doesn't stay on food.** `BlindForager` keeps `forward` at 1.0 while it feeds
   (`controller.rs:674-684`), so it crosses a pile in about a second.
3. **The litter stock can't regulate detritivores.** Decay processes 10³–10⁴ times their
   intake, so food never runs short at any headcount the sim can carry. Nothing else limits a
   small-prey boom: no crowding rule, no egg loss, and in practice no predation.
4. **The lanternjaw's search ignores where prey are.** It walks to crowns and waits there
   (`controller.rs:1345-1350`). It has no switching, and no sense for blind prey (the lure only
   works on sighted animals).
5. **Nothing answers glowcap abundance numerically.** Capgnawers don't breed.
6. **The ripple-snail and the bellwing have no predator.**

---

## 2. What goes wrong

### 2.1 Data

All paths are under the scratchpad
`/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad/`,
which is disposable.
- **Terrarium, `soak/v1/runs/`:**
  - `preG` arm: 4 seeds (1, 3, 5, 6), branched from a staged founding at minute 120 and run to
    minute 360. It has the full fauna balance.
  - `pick_a63`: the same starts. It stopped at minutes 368–401 with plant stocks only.
- **Terrarium, `soak/runs/`:** `pick_a63` on the overlap build, **still running** (minutes
  459–552 of 1560 when read). Plant stocks only.
- **Patch, `seedling/collect/` and `spread2/collectH/`:**
  - 8 seeds (1, 3, 5, 6, 8, 9, 12, 14), minutes 120–360, with the fauna balance.
  - The two sets branch from the same founding saves, so their animals are not independent.
    They agree with each other; the tables use `seedling_a63`.
  - The patch founds **no lanternjaws and no Veilhounds** (`config/scenarios/patch.toml:34-40`).
  - Capgnawers founded on seeds 1 and 12 only.
- **Reference, `soak/s{1,3,5,6}.csv`:** a plants-only 16 h soak on the 2026-10-03 code. No
  animals.

The measures:
- **B** = assimilated / (upkeep + motor).
- **Access** = the share of hungry awake ticks spent eating.
- **Sated** = the share of awake ticks with the eating latch off.

All three are defined in `crates/cubarium-search/src/es/voxel/live/balance.rs`.

### 2.2 Stocks

| stock | terrarium | patch | verdict |
| --- | --- | --- | --- |
| litter | swings 3–10× within each 2 h day. Daily peaks rise from 1.3–2.5k (min 180) to 2.5–7.4k (min 270–390). The plants-only 16 h soak stays in 1.6–6.7k | peaks 0.3–1.1k at min 180 → 0.3–2.6k at min 360 | **held** by decay; tracks plant production |
| glowcap stands | 1.0–1.2k at min 120 → 2.5–4.4k at min 450–540, still rising (s6: 1031 → 4360). Plants-only soak: 1.0–1.2k → 1.65–2.9k at 16 h, slowing | 185–793 → 590–1,900 by min 360 (×1.6–4) | **unchecked**: only space and substrate stop it |
| glowcap caps (edible) | 119–150 → 220–580 | 15–74 → 60–215 | rising |
| dead wood | 11–35 → 26–375 | 4–11 → 4–65 | small, held by decay and glowcap |

**Litter flux** (the `flux` lines: `shed` against `litter_taken`). On the patch, plants shed
2,700–7,800 per 2 h window and animals took 0.1–1.4 of it. On the terrarium they shed
5,500–12,400 and animals took 1.5–3.3. On the terrarium at L ≈ 3,000, decay removes about 2 org/s,
roughly 15,000 per world day. The 16 shredders eat about 1 per world day.

### 2.3 Animals (minutes 120 → 360)

| | terrarium, 4 seeds | patch, 8 seeds |
| --- | --- | --- |
| littershredder | N 63 → 61, **0 births**; B 2.0–3.0; access 0.18–0.27; sated 0.02–0.07; body 0.26 (laying size 0.375) | N 44 → 45, 1 birth; B 2.0–5.9; access 0.05–0.64 |
| capgnawer | N 48 → 47, **0 births**; B 1.7–2.6; access 0.24–0.43; body 0.42–0.48 (laying size 0.638) | N 9 → 10, 2 births |
| ripple-snail | N 64 → 64, **0 births**; B 2.3–3.3; access 0.27–0.36; body 0.10–0.11 (laying size 0.146) | N 44 → 43, 0 births |
| lanternjaw | N 15 → 19; 11 hatched; body 0.17–0.20 (laying size 0.63), **reserve 2–4 %**; intake: 1,316 carrion bites in one seed, nothing elsewhere; 3 killed by Veilhounds | not founded |
| lanternjaw kills | 2 frondgrazers (seed 5); **no small prey in any seed** | — |
| frondgrazer | N 87 → 103, 50 births; B 3.0–6.1 | N 90 → 121, 57 births |
| bellwing | N 110 → 245, 197 births (seed 3: 50 → 136) | N 83 → 145, 113 births |

The **generation-time arithmetic** explains the zeros. The net specific growth rate is
g = (assimilated − spent) per body per world day, over mean body. From hatchling to laying size,
the body must grow by the factor in the table. At rate g that takes ln(factor)/g world days,
before the reserve is filled for the egg.

| | g measured (terrarium / patch) | hatch → lay factor | days to laying size | lifespan | gut ceiling on g |
| --- | --- | --- | --- | --- | --- |
| littershredder | 0.065 / 0.09 | 4.0 | 15–21 | 6 | ≈ 0.54 |
| capgnawer | 0.054 / 0.10 | 4.0 | 14–26 | 6 | ≈ 0.57 (caps) |
| ripple-snail | 0.048 / 0.059 | 4.0 | 23–29 | 6 | ≈ 0.34 |
| frondgrazer | 0.39 / 0.35 | 6.0 | 4.6–5.1 | 8 | — |
| bellwing | ≈ 0.85 | — | short | 6 | — |

The gut ceiling is yield × k × 7,200 / τ − daily need, eating whenever awake.

The small detritivores run at 10–20 % of what their own guts allow. The bellwing shows that the
same egg machinery booms once intake is high. **The founders are a cohort that will age out
without replacing itself** (median 12 h sim). The patch runs' near-zero deaths only mean 4 h is
short of a lifespan.

### 2.4 Why the lanternjaw starves

- **Its teacher looks for food in the wrong place.** It walks to a crown, the place grazers
  feed, and waits there with the lure lit (`controller.rs:1207-1384`).
- **A free juvenile can't take the prey at a crown.** It drops off its host at 0.25, so its
  window is prey ≤ 0.38. That rules out the adult frondgrazers (0.9–1.5) at the crown and most
  capgnawers (0.42–0.48). It allows only shredders, which live on litter at ground level, blind,
  away from crowns.
- **The lure attracts only sighted animals.** Shredders and capgnawers are blind.
- **The egg bank is working.** It hatched 11 in 4 h.

The chain breaks after drop-off, at "find prey I can kill". This matches the 2026-10-02
diagnosis (`design/ecology-timescales-2026-10-02.md` §1c).

### 2.5 Stocks growing or shrinking without check, and the role that should answer

| stock | trend | role that should answer | why it doesn't |
| --- | --- | --- | --- |
| glowcap stands and caps | growing | capgnawer (fungivore), plus plants retaking lit ground | capgnawers don't breed (§2.3); glowcap holds lit faces (`glowcap-yield` package, in flight) |
| littershredder, capgnawer, ripple-snail cohorts | flat now, **shrinking** past one lifespan | their own breeding | maturation exceeds lifespan |
| lanternjaw juveniles | starving | small prey | its search ignores small prey; no sense for blind prey |
| bellwing | **booming** (×2.2 in 4 h) | an aerial or night predator | on no hunter's list (`body.rs:830`) |
| ripple-snail | flat | a predator, or film limits | on no hunter's list; film likely binds later |
| litter | held | decay (exists) | — |

---

## 3. Proposal: roles and behaviours first, numbers last

### 3.0 What can hold what

- **Litter is the soil microbes' job, and they already do it.**
  - Decay is first-order, so litter can't run away. It tracks what plants shed.
  - In real soils microbes do most of the decomposing too. Macro-detritivores mostly fragment
    litter.
  - Asking shredders to hold litter would need thousands of bodies (§2.2), so they **follow**
    litter: more litter, more shredders, more lanternjaws.
  - One honest link from shredders back to litter: **comminution**. Shredder feeding speeds
    microbial decay where it feeds. This is optional (§3.3).
- **Glowcap is held by space, substrate and its grazer.** The `glowcap-yield` package handles
  space on lit ground. Capgnawers must breed to answer the rest.
- **A small-prey boom can't be capped by food.** Its caps must be behaviours: crowding limits
  and predation. Otherwise the boom runs to the sim's body budget and stops there, which is
  neither a balance nor cheap. This is the main design point of §3.1 and §3.2.

### 3.1 Small detritivores as fast breeders (littershredder, capgnawer, ripple-snail)

**A. Stay on food (area-restricted search).** A forager that tastes food above the feed gate
stops walking (forward 0) and feeds until its latch goes off or the stock under its mouth falls
below the gate. Then it turns back once toward where it last tasted food before it resumes the
search.
- Loop: more food → more time on food → access up → g up → breeding.
- This raises access, the factor measured at 0.2 that the gut allows up to 1.
- Risk: none for stability. Heavier local depletion, which is the point.

**B. Dark-active rather than strictly nocturnal.**
- **The rule.** A detritivore is awake while its own face is dark: the terrain's sky × daylight
  under the same 0.3 line, so under closed cover, in caves and at night.
- **Why.** Litter piles up under shade (the "shaded" cohort holds the most). Real woodlice and
  millipedes are photonegative rather than clock-bound.
- **What it touches.** This keeps Wrysk's circadian decision (2026-09-24) and changes only the
  starting pick "shredders nocturnal". It also gives shade a meaning: a refuge from the diurnal
  Veilhound and from sun.
- **Loop.** More cover → more litter and more foraging time → more breeding.
- **Risk:** a world with little shade keeps them nocturnal, as now.

**C. Breed at part size, small young, clutch sized by surplus.**
- Laying size drops from the full adult to a fraction of it. Insects and isopods breed well
  before their largest size and keep growing.
- Hatchlings are smaller.
- **The clutch is what the surplus pays for:** `floor(reserve above the floor / egg_organic)`,
  between 1 and a maximum. Fecundity then rises with condition **and** with body size, both real
  and both feedbacks.
- The existing surplus hold and interval stay as they are.
- Loop: fed → bigger clutches sooner → numbers follow food within about a world day.
- **Risk: boom overshoot.** D and §3.2 are the brakes. Each needs its own check (§4).

**D. Self-limitation that bites before food does.**
- **Nest-site limitation.**
  - An egg-layer lays only on a face with no other clutch of its lineage within a radius.
    Otherwise it walks on and tries again.
  - The density of breeding sites then caps recruitment. The cap scales with litter-covered
    ground, so it still follows the food.
- **Egg cannibalism.** A littershredder that tastes a clutch of its own or another detritivore
  lineage eats it as food (yield as carrion).
  - Real isopods and millipedes do this.
  - It makes recruitment fall with adult density: the classic stabiliser of a boom.
- **Loop.** Dense adults → fewer surviving eggs → recruitment falls before food is touched.
- **Risk:**
  - Too strong, and it holds numbers flat (today's picture).
  - Check that recruitment per adult falls with density but stays above replacement at low
    density.

**E. Capgnawer specifics.**
- Its eggs are laid at a glowcap (egg site "at fungal food"). Recruitment then sits where its
  food is.
- With A–D it grazes caps hard enough to matter. Caps already raise an induced defence per bite,
  which damps over-grazing.
- Loop: more glowcap → more capgnawers → caps grazed → glowcap stands thin (with B's dark
  activity, mostly in the dark grove where glowcap belongs).
- Risk: the capgnawer eats glowcap down to its refuge and crashes. The litter fallback (yield
  0.25) keeps a floor under it, and that link already exists.

**F. Ripple-snail.**
- A–C apply.
- Film is a small, logistic stock, so food will bind it, which is real self-limitation. Snails
  need D less. Without a predator, film alone caps them. Expect a snail-film cycle: watch it.

### 3.2 The lanternjaw as an active hunter of the booming small prey

**Role:** a mobile sit-and-wait hunter of small ground prey, like a scorpion or a centipede:
- it moves often between prey-rich patches;
- it waits still in each for a short spell;
- it strikes what comes into reach;
- it eats small meals often.

Adult grazers stay possible (wounds, three hits) but are not its living.

**Metabolism.** Daily need up from 0.04 to between the snail-shredder band and the grazer
(0.06–0.08). Reserve cap down from 1.0 (sized to hold a whole grazer) to the ordinary 0.5–0.6.
Satiated rest stays but ends sooner.
- Loop: it must eat most days, so its numbers follow prey within days instead of weeks.
- Risk: starvation between booms. The dormant egg bank already covers that gap.

**Foraging behaviour (replaces the crown-seeking teacher):**
1. **Patrol to prey cues.** It follows what the prey eat:
   - the detritus field (`Chem(detritus)`), which the shredders follow;
   - the fungal field (`Chem(fungal)`), which the capgnawers follow;
   - crowns for grazers.

   Which one it follows is its current **search image** (2).
2. **Switching.** It keeps a short memory, about a world day, of the prey it has caught or met,
   by lineage. Its search image is the lineage with the largest recent share, so it hunts where
   that prey lives.
   - This is a type III functional response. Common prey take most of the attacks, and rare
     prey get a refuge at low density. It is the textbook stabiliser of a predator with several
     prey, and it stops the lanternjaw from driving one prey to zero.
3. **Wait still, briefly.** On a patch with prey cues it stops and waits (makes no tremor). It
   leaves after a short fruitless spell (shorter than today's minute), choosing the next patch
   along the cue.
4. **Feel blind prey.** It gets the two-channel tremor sense the predation note already
   specifies for blind animals (`design/predation-and-flight-2026-09-28.md` §2.2). It turns
   toward the side with tremor and strikes when the strike capsule holds prey.
   - Scorpions hunt exactly this way.
   - Moving costs it surprise: walking prey hear a walking hunter, and blind prey already flee
     tremor. So stillness keeps its value, and "active" means relocating often, not chasing.
5. **The lure stays.** It is lit during waits and works on sighted prey (decision 3).

**How it fits the decided mechanics:**
- **Egg bank and footfall trigger** (Wrysk, 2026-09-28): unchanged.
  - A shredder boom is more footfall over the eggs, so more hatching. That is the numerical
    response, already built.
  - Adult suppression keeps hunters spaced, a second brake.
- **Parasitic hatchlings** (Wrysk, 2026-10-01): unchanged. A boom is more hosts, so more riders.
  The riders' drain is a small extra cost on dense prey.
- **Wounds** (decision 1): unchanged. Small prey still die in one hit.
- **Prey list** (§2.1, agreed): unchanged. "Its staple is shredders and young grazers" is already
  the agreed role; this makes the behaviour match it.
- **Nocturnal**: kept. Its prey are nocturnal or dark-active.

**Loops and risks:**
- Prey up → more footfall, hosts and catches → more lanternjaws hatched and laying → prey down
  → lanternjaws starve or age out → eggs wait → prey recover. That is a predator-prey cycle.
- **Damping:**
  - switching spreads attacks across shredder, capgnawer and grazer;
  - adult suppression and the hatch draw (1/3 per dusk) slow the predator's response;
  - prey self-limitation (§3.1 D) keeps the prey peak low.
- **Risk:** without D the cycle has large amplitude. Prey peaks hit the body budget, and
  lanternjaw crashes empty the map until the bank recovers.

### 3.3 Missing roles

- **An egg predator**, a role that cuts recruitment at density. §3.1 D gives it to the shredders
  themselves. A separate egg-digger stays a later option, as the predation note says.
- **A predator of the ripple-snail.** Options:
  - leave film to cap it;
  - put the snail on the lanternjaw's list at water margins;
  - add a shoreline hunter later.
- **A predator of the bellwing.** It is booming with nothing on it. This is outside this web and
  only named here. A night lanternjaw taking perched or grounded bellwings, through the existing
  "grounded" rule the seedporter uses, would be one way.
- **Comminution (optional).** Feeding detritivores raise decay where they feed. It ties shredder
  numbers back to litter and mineral turnover. It doesn't regulate them.

### 3.4 Numbers last: plausible starting values

These are given only so the behaviours have something to run on. Each is a knob with a registry
row, untuned.

| knob | now | proposed | why |
| --- | --- | --- | --- |
| detritivore `birth_body` / `body_max` | 1.0 | 0.4 | breed while still growing |
| detritivore `body_min` / `body_max` (hatchling) | 0.25 | 0.12 | small young; egg = 1.2 × body_min |
| clutch | fixed 1 | surplus-paid, max 3 (shredder, capgnawer), 4 (snail) | follows condition and size |
| nest-site radius (new) | — | 0.5 m | one clutch per patch of litter |
| egg cannibalism yield (new) | — | the carrion yield | eggs are meat |
| dark-active line (new) | — | sky × daylight < 0.3 at the face | the circadian line already used |
| lanternjaw daily need | 0.04 | 0.06–0.08 | active small predator |
| lanternjaw `reserve_cap` | 1.0 | 0.5–0.6 | small frequent meals |
| lanternjaw search-image memory (new) | — | 1 world day | switching |
| lanternjaw patch wait (new) | 60 s fruitless wait | 20–30 s | relocate often |
| lanternjaw tremor sense (new) | — | 1.5 m, the predation note's §2.2 | find blind prey |

**Check against the arithmetic.**
- With hatch → lay of 0.12 → 0.4 (a factor of 3.3), a shredder needs g ≈ 0.3–0.45 to lay in
  2.7–4 world days, inside its 6-day life.
- That g is 55–80 % of its gut ceiling. A and B have to deliver about a 4× rise in effective
  feeding time. If they don't, the gut (k, τ) is the next knob, and only then.

### 3.5 Risks in one place

- **Boom to the body budget.**
  - Each small body costs tick time. A thousand shredders is a compute question before it is an
    ecology one.
  - D is the first brake. A per-lineage body ceiling, if one is ever needed, is a performance
    guard, not ecology. Label it as one.
- **Boom-bust crash to extinction** of a prey lineage or of the lanternjaw.
  - Switching, D and the egg bank are the guards.
  - The measure is the minimum N across seeds (§4).
- **Shade dependence.** Dark activity favours maps with cover. On open maps the detritivores
  behave as now.
- **Capgnawer overgrazing glowcap.** The induced defence and the litter fallback are the guards.
- **Lanternjaw on grazers.** A hungrier lanternjaw may hit more sleeping grazers. That is within
  the agreed role (taken asleep, wounded or starving), but watch the grazer kill share.

---

## 4. What to measure

Run as staged runs per `WORKING_POLICY.md`:
- checkpoints each segment;
- a liveness check after the first few minutes (animals move and eat; each new mechanism's
  counter is nonzero: stops on food, dark-awake ticks, clutch sizes, nest refusals, eggs eaten,
  switches, tremor strikes);
- re-evaluate before continuing.

Tests check the mechanisms on hand-built inputs against the config, never a population or a
ratio.

**Readouts to add** (the balance block already has B, access, sated and births):
- awake share per lineage (unmeasured today; it splits access from sleep);
- **time from hatch to first clutch**, per body;
- clutch size distribution;
- eggs laid, eaten and hatched;
- nest-site refusals;
- lanternjaw catches **by prey lineage** beside each prey's share of the bodies (the switching
  index);
- lanternjaw adults established (free, at laying size);
- per-lineage N per 10 min;
- glowcap stands and cap stock per 10 min.

**Maps:**
- **Patch** (no predators by config): does the detritivore tier breed, and does **self-limitation
  alone** stop the boom? 8 seeds × 2 h from the minute-120 saves, then one 6–8 h soak.
- **Terrarium**, or `range` for predator space: does the lanternjaw establish on small prey, and
  does the system settle? Same staging, the soak overnight.

**It works when, across seeds:**
- **Breeding:**
  - shredder, capgnawer and snail lay within one lifespan;
  - hatch-to-first-clutch is under the median lifespan;
  - births per adult rise with intake (compare seeds and windows).
- **Stocks bounded:**
  - glowcap stands level off, or turn down, within the soak;
  - litter's daily peak follows plant production (no new trend);
  - each detritivore's N stays below the level where food binds, held by D and by predation.
- **Oscillations damped:**
  - successive prey peaks shrink, or the coefficient of variation of N over sliding 2 h windows
    falls over the soak;
  - lanternjaw numbers lag prey and follow them.
- **No extinction:** the minimum N of every lineage stays above zero on every seed. The
  lanternjaw may sit in its egg bank between cycles, and that counts as present.
- **The lanternjaw:**
  - free adults exist;
  - most catches are small prey;
  - the catch share tracks prey share (switching), not a fixed preference.
- **Cost:** ms/tick against total bodies, logged so a boom's price is visible.

---

## 5. Canon this would touch

The ledger (`design/0_Canon/DECISIONS.md`) has no entry on these species (D-0001–D-0003 only). The
binding choices are Wrysk's, recorded in `design/predation-and-flight-2026-09-28.md` §0 and
§2.13, in `design/handoffs/voxel-predation-parasite-2026-10-01.md`, and in the weather handoff
(circadian rhythms).

**Stays as decided:**
- decision 1 (wounds), decision 3 (lure through a glow sense), decision 4 as amended (egg bank,
  no spontaneous adults);
- §2.13 footfall trigger and adult suppression;
- the 2026-10-01 parasitic hatchlings;
- §2.1 prey list and the "staple is shredders and young grazers" role;
- circadian rhythms exist.

**Would change, and needs Wrysk's word:**
- **§2.8 "one big meal, then a long rest"**, his own earlier framing (2026-09-28), and the §2.12
  placeholders that encode it (upkeep, reserve cap). His 2026-10-05 remark leans away from it;
  this proposal takes it out.
- **The ambush teacher** (§2.11, a routine call): replaced by the patrol, wait and switch
  behaviour.
- **Starting circadian picks** (routine): shredder, capgnawer and snail become dark-active.
- **Detritivore breeding rule** (placeholders, `lib.rs:1480`): part-size laying, surplus-paid
  clutches, nest sites, egg cannibalism.
- **New sense:** the tremor module the predation note already specifies, given to the lanternjaw.

---

## 6. Open questions for Wrysk

1. **Litter's keeper.** Is it fine that the microbes (decay), not the shredders, hold litter, with
   shredders following it? The alternative needs thousands of bodies.
2. **The lanternjaw.** Mobile sit-and-wait on small prey with switching, the egg bank and lure
   kept: yes? Does "one big meal, then a long rest" go?
3. **Dark-active detritivores** in place of strictly nocturnal ones: yes?
4. **Crowding brakes.** Egg cannibalism and nest sites as the small-prey brakes: both, one, or
   leave predation to do it?
5. **The ripple-snail's check.** Film alone, the lanternjaw at water margins, or a new shoreline
   hunter later?
6. **The bellwing boom.** Flag it for its own pass?
7. **A body ceiling.** Is a per-lineage cap acceptable purely as a compute guard, labelled as such,
   if a boom outruns the brakes?
