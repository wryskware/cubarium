---
status: Wrysk said go 2026-10-01; R0 dispatched
date: 2026-10-01
owner: Fable (organism line)
worktree: .claude/worktrees/roster (branch roster, off main 28a4ab65)
---

# Roster: the six model-only animals become live founders

Six animals are baked models drawn only by `voxel_specimens`: capgnawer, loftstrider,
ripple-snail, bellwing, seedporter, chorister. This line makes each one a simulated
founder, one package at a time, in one worktree (one fauna worker at a time).

## Decisions

- **Wrysk, 2026-10-01:** the shredder goes back to litter, and the capgnawer owns the
  glowcap caps. Choristers are founded only in the bigger worlds (desktop and voidrunner
  terrarium, not the Tachyon panel) for now. Groundwork comes first.
- **Fable:** the shredder keeps carrion; only the caps move. A new founder's
  **code-default founding count is 0**, so co-training and training worlds are unchanged.
  The live presets turn it on. Each animal starts on a heuristic; brains come later
  (P2 contract v3, live-train).
- **Standing rules:** every ecological number is a knob (WORKING_POLICY). Plausible
  defaults, no tuning to today's ecology. Short function tests only. The commit
  message is the report. Plain coloured voxels are the draw.

## Order

| Pkg | What | Brief |
|---|---|---|
| R0 | Groundwork: species as data | below |
| R1 | Capgnawer + loftstrider (walkers, existing food), shredder loses caps | written when R0 lands |
| R2b | Ripple-snail (glassfilm R2a already merged) | below |
| F1 | Flight substrate + bellwing | `voxel-flight-2026-09-28.md` F1 (worktree overridden to this one) |
| F3 | Seedporter: climb, glide, lanternberry fruit, gut seeds | below |
| R3 | Chorister: pack, den, live birth, provisioning, territory | below |

---

## Brief R0: species as data

**Objective.** After R0, adding a founder takes these steps: an enum arm with
name/role/manifest, a physiology default (body, diet, mouth band, founding mode), a
heuristic constructor, and a model and coat under its name. No hand-written per-species
branch in the step, the founding, the search or the presenter. Today the lanternjaw's
arm touches about 70 non-test sites in 24 files
(`grep -rn 'Founder::Lanternjaw' crates/ --include='*.rs'`).

**R0 is behaviour-preserving.** It changes no ecological number and no outcome.

**Crates:** `cubarium-voxel-fauna`, `cubarium-voxel-sim`, `cubarium-search`, `cubarium`.

### What to build

1. **Diet is a list.** `FounderPhysiology` gets `diet: Vec<Food>` (a serde knob), and
   the defaults are today's: shredder `[Litter, CapTissue, Carrion]`, browser
   `[Foliage]`, lanternjaw `[Carrion]`.
   - The mouth (`step.rs` ~1710–1750, `body::mouth_detritus`, `mouth_foliage_stand`,
     `mouth_pool_site`) offers each food from its source:
     - `Litter` and `Carrion` come from the ground pool at the standing face;
     - `CapTissue` comes from a stand with `Trophic::Saprotroph`;
     - `Foliage` comes from a stand with `Trophic::Photo`.
   - When several foods are present, keep today's shredder choice rule, generalised over
     the list.
   - The stand's food class comes from its trophic, not from whose mouth took it.
   - `Diet::of(founder)` and the permission filter are derived from the list, or
     replaced by it. The detritus cue field and the founding food filter
     (`habitat.rs` `lineage_food`, `food_on`) read the same list.
2. **The mouth band is two knobs.** Add `mouth_floor_fraction` (default 0) beside
   `mouth_ceiling_fraction`. Both are multiples of body height above the standing
   surface. R1's loftstrider feeds 0.8–3.5 m up.
3. **Founding counts are config.** `FOUNDER_COUNTS` / `SHREDDERS` / `BROWSERS` /
   `FOUNDING_CLUTCHES` become a per-lineage table keyed by lineage name, with today's
   numbers as defaults. A preset TOML can set it (`[fauna.founders]` or wherever the
   sim config already lives; your call). An unnamed lineage takes the code default.
   - The founding loop runs over `Founder::ALL`.
   - A physiology field `founding: Bodies | Clutches` carries the lanternjaw's egg bank.
   - The no-egg-bank control stays expressible: a count of 0.
4. **One heuristic constructor per founder** in one place (`controller.rs` ~925 already
   is that place). Every other `match founder` that encodes ecology becomes a
   physiology field.
5. **Search and host loop over founders.** `cubarium-search` es/voxel and the
   `cubarium` host enumerate `Founder::ALL` or the lineages a config names, not three
   hand-written arms.
   - The search covers the lineages that its config names and that have a nonzero
     founding count. A founder with count 0 has no gene, target or score term.
   - `config/training/search/cotrain.toml` must still load and mean the same thing.
6. **Presenter by name.** A live animal's model is looked up by lineage name among the
   founder models, falling back to `specimens` (`crates/cubarium/src/voxel/model.rs`).
   - The coat (`colours.rs`) also falls back to the specimen coat.
   - The point: a new founder draws with its baked specimen model and needs no rebake.
7. **The checklist.** A short doc comment on `Founder` listing the steps above.
8. **Registry.** Each new field gets a registry row. Regenerate `design/parameters.md`
   (`cargo run -p cubarium-voxel-sim --example params -- --write design/parameters.md`).

### Rules

- **No outcome change.** On a short run (seed 1, desktop terrarium, about 2,000 ticks,
  heuristics), the bites, births and deaths per founder are equal before and after.
  Report both. If they differ, find out why before committing; don't pin a hash.
- **Snapshot schema:** bump it if the format changes. Always fresh, never migrate.
- **Out of scope:**
  - new species and the shredder's diet change (R1);
  - manifests and senses (P2);
  - flight (F1);
  - every number.

### Tests (short function tests, no world past a few hundred ticks)

- A founder whose diet lacks `CapTissue` never takes a cap that stands in its band.
- A lineage with count 0 founds nothing; a count read from a preset TOML is honoured.
- A founder with no founder model draws with its specimen model.
- The existing suites of the four crates pass.

### Return (≤ 40 lines)

- Commits, as hashes and subjects.
- The before/after short-run numbers.
- Any `match founder` you left in place, and why.
- Anything this brief did not decide.

Commit your own paths only (`git commit -- <paths>`). Stop every background shell
before you return.

---

## Brief R1: capgnawer and loftstrider, and the shredder loses caps

**Objective.** Two new walking founders on R0's tables, each driven by a heuristic, each
drawn with its baked model, founded in the live presets.

**R1 is a model-rule change.** A separate test pass comes first, then the implementer.

### Decisions (Fable; implement, don't revisit)

**Shredder**

- Diet becomes `[Litter, Carrion]`, with no caps.
- Nothing else about it changes.

**Capgnawer** (looks: `animal-body-reimagining-2026-09-24.md` §Capgnawer; ecology:
`animal-body-candidates-2026-09-23.md` §Capgnawer)

- **Adult size:** 0.25 × 0.21 × 0.19 m.
- **Diet:** `[CapTissue, Litter]`.
  - Cap yield equals the shredder's cap yield today.
  - Litter yield is half the shredder's litter yield (conditioned litter, a weak
    fallback).
- **Senses:** the shredder's blind layout, 23 inputs, with one change. Its `Chem` reads a
  **fungal** field whose sources are saprotroph cap stock, where the shredder's reads the
  detritus field.
  - That makes it a manifest of its own (`capgnawer`), the same width.
  - The fungal field is built the way the detritus field is.
- **Heuristic:** the shredder's forager reading its own `Chem`. Write a new one only if
  the shredder's is hard-wired to detritus.
- **Life history:** the shredder's reproduction mode. Rates come from the shredder's,
  scaled by the body-volume ratio, the way P1 derived the lanternjaw from the grazer
  (`predation-and-flight-2026-09-28.md` §2.12).
- **Predation:** on the lanternjaw's prey list; the size window decides.
- **Founding:** near glowcap stands (from R0's diet-driven food filter).

**Loftstrider** (`organism-large-animals-2026-09-23.md` §3, selected appearance and
niche)

- **Adult size:** 2.2 × 2.2 m across the legs and fronds, 3.4 m tall.
  - The eye ring is at 1.5 m, so `eye_height_fraction` is about 0.44.
  - The mouth band is 0.8–3.5 m: floor 0.235, ceiling 1.03 of body height.
  - Fractions scale with growth, so a newborn (0.46 L) reaches about 1.6 m.
- **Diet:** `[Foliage]`. The mouth floor keeps it off turf, rosettes and ground cover.
- **Sloppy feeder:** `bite_spill_fraction` (a knob, default 0.25) of every bite's
  withdrawn tissue goes to the litter pool at its standing face. This is a mass-balance
  flow; the ledger closes.
- **Senses:** the browser's 37-input layout. Its cone counts only foliage whose height
  above its standing surface lies inside its mouth band. That makes it a manifest of its
  own (`loftstrider`), the same width. Interim until P2.
- **Heuristic:** the browser's forager on its own cone and mouth. A new one only if
  needed.
- **Life history:** long-lived and slow, with one young per birth and a long gestation.
  Rates come from the browser's scaled by body volume, the same way. The young following
  its parent is out of scope; note it.
- **Body:** long legs. `step_up_m` is 0.5 and the wade/drown fractions are scaled so it
  wades where the grazer drowns. Knobs, plausible.
- **Predation:** on no prey list yet (the chorister comes in R3). No defence counter.
- **Founding:** near the tallest vascular stands.

**Both**

- **Code-default founding count is 0.**
- **Live presets:**
  - `config/desktop/terrarium.toml`, `config/voidrunner/terrarium.toml` and the desktop
    `voxel.toml` found 6 capgnawers and 2 loftstriders;
  - `config/tachyon/voxel.toml` founds 4 and 1.
- **Presenter:** draw with the baked specimen model through R0's fallback; no rebake.
  The coat is the specimen coat. Quarter turns as today; free rotation is not in R1.

### After R0 (032e775b, 50e54b83): what R1 builds on

- **Diet:** a food set, written in TOML as `diet = ["litter","carrion"]`.
- **Founding:** `[fauna.founders.<lineage>] founding_count = N` in a preset, with
  `founding = "bodies"` and `founding_site = "at_food"` for both new animals.
- **Prey list:** `Hunter::LANTERNJAW.prey` is a positional array; add the capgnawer's
  column.
- **Checklist:** follow the one on `Founder`.
- **Glassfilm** is merged (ff7ad6dc). The R2b ripple-snail builds on it later.
- **Measure:** use `founder_tally` (R0); `voxel_census config=` reads the preset's
  `[fauna]` tables.

### Check t = 0 first

On the desktop terrarium at t = 0, how much foliage stands in a loftstrider's 0.8–3.5 m
band, and how many glowcap caps are there? If either is zero, found the animal anyway,
report the number and don't tune plants. Starving is evidence, not a blocker.

### Tests first (separate worker; short function tests, ≤ 200 ticks)

- The shredder never takes a cap and still takes litter and carrion.
- A capgnawer takes a cap in its band, and takes litter at the lower yield.
- A capgnawer's `Chem` rises toward a cap and doesn't respond to litter alone.
- A loftstrider takes foliage at 2 m and none at 0.3 m. A bite's spill lands as litter at
  its face, and the ledger closes.
- A loftstrider's cone reads foliage in its band and none below its floor.
- A preset with capgnawer = 6 founds up to 6 near glowcaps. The code default founds
  none.
- A live capgnawer and a live loftstrider each draw with their specimen model.

### Measure (after the implementer, 8 seeds × 60 min, desktop terrarium, heuristics)

Per founder, report:
- alive at 15, 30 and 60 min;
- bites by food;
- births;
- deaths by cause.

Then one headless GPU capture of each new animal in the world
(`--gpu-target headless --gpu-capture DIR`), with absolute paths.

### Return (≤ 40 lines)

- Commits.
- The t = 0 numbers.
- The table.
- Capture paths.
- Anything undecided.

Commit your own paths only. Stop every background shell before you return.

---

## Brief R2a: glassfilm, the ripple-snail's food (flora only; runs beside R0/R1)

**Objective.** A thin living film on shallow submerged ground that grows from light,
dies when it dries, spreads, and can be grazed. It is the "Glassfilm" row of
`theoretical-biosphere-2026-09-16.md` (an aquatic mat with a funded nutrient route).

**Worktree:** `.claude/worktrees/glassfilm`, branch `glassfilm`, off `roster`. It merges
back into `roster`.

**Crates:** `cubarium-voxel-flora` only, plus a read-only water query from
`cubarium-voxel` if one is missing. No fauna, sim, search or presenter edits; R2b does
those.

### Decisions (Fable; implement, don't revisit)

- **It is a fourth pool on `Ground`**, not a species or stand: `film`, `film_mineral` and
  `film_energy` at the site, beside litter, dead wood and carrion.
- **Where it lives:** a site is wet-eligible while its standing water
  (`VoxelView::standing_depth_m` over the support face) is above 0 and at most
  `film_max_depth_m` (default 0.6).
- **Light:** the site's sky light as the plants read it (`step.rs` `sky_at`, with canopy
  shade if the plants apply it), times `exp(−film_extinction_per_m × depth)` (default
  0.8).
- **Growth:** logistic, `film_growth_per_s × light × film × (1 − film / capacity)`, with
  capacity `film_capacity_per_m2 × cell area`.
  - Each unit of new tissue draws mineral from the site's `mineral` at the plants'
    tissue ratio. That is the nutrient route: no mineral, no growth.
  - Energy is booked as plant production is.
- **Upkeep:** `film_maintenance_per_s × film`, respired.
- **Spread:** an eligible site with less film gains fragments from each eligible
  4-neighbour at `film_spread_per_s × neighbour film`. The neighbour pays, so mass moves
  and none is created.
- **Death:** a site that is dry, or deeper than the maximum, loses film at
  `film_dry_death_per_s` into its own litter pool (aquatic residue becomes litter).
- **Founding:** a `seed_film` call puts `film_seed_per_m2` on every eligible site,
  booked as introduced, the way `Seed` is. R2b calls it from the founding.
- **Grazing:** `take_film(site, want) -> Option<Taken>`, pro rata, with
  `take_litter`'s rule.
- **Readers:** the film stock per site and an iterator over film sites, for R2b's chem
  field and the presenter's tint.
- **Cadence:** the film may step less often than every tick (for example once per
  simulated second, with dt scaled). Budget: under 0.2 ms per tick, averaged, on the
  desktop terrarium.
- **Ledger:** the flora ledger closes with film terms (introduced, produced, respired,
  to litter, consumed).
- **Knobs:** every number above is a serde field on `FloraConfig` with a registry row,
  and gets plausible defaults. A film should fill its capacity in an hour or so of good
  light and die within about ten minutes of drying. Don't tune further.
- **Snapshot:** the pool is saved. Always fresh: bump the flora schema.

### Tests first (separate worker; short function tests, ≤ 200 ticks)

- Film grows on a lit shallow submerged site.
- It doesn't grow at depth 0, below the maximum depth, in the dark, or with no mineral.
- Drying sends film to litter.
- Spread moves film to an empty eligible neighbour and conserves it.
- `take_film` takes pro rata.
- The ledger closes over a short run with all of these.

### Measure

- On the desktop terrarium at t = 0: how many eligible sites there are, with their
  area.
- After `seed_film` and 30 simulated minutes, plants only: the total film and its
  spread.
- The film step's ms per tick.

### Return (≤ 40 lines)

- Commits.
- The numbers.
- Anything undecided.

Commit your own paths only. Stop every background shell before you return.

## F1 overrides (the flight brief stands except for these)

- **Worktree:** `roster`, after R1. Build on R0's tables: diet `[Nectar]`, founding count
  in config.
- **Founding:** the code default is 0. The live presets found 6 (desktop and voidrunner
  terrarium) and 4 (Tachyon). `--introduce bellwing:N` stays.
- **Presenter (item 10):** the bellwing is already baked as a specimen; draw it with that
  model through R0's fallback, at `y_m`. One pose, as for every animal. The vane beat
  and the folded perch pose are owed, and they need more baked poses. Note it; don't
  invent them.
- **Measure:** 8 seeds × 60 min (WORKING_POLICY "Verification runs"), not 4 h. Presets:
  the desktop terrarium and the Tachyon voxel config. The MP4 stays.

## Brief R2b: the ripple-snail (runs after R1, before F1)

**Objective.** A slow, water-bound film grazer on glassfilm (R2a, merged ff7ad6dc),
driven by a heuristic, drawn with its baked specimen model. The film is visible, so a
grazed trail shows.

**R2b is a model-rule change.** A separate test pass comes first, then the implementer.

### Decisions (Fable; implement, don't revisit)

Looks are in `animal-body-reimagining-2026-09-24.md` §Ripple-snail; ecology in
`animal-body-candidates-2026-09-23.md` §Ripple snail.

**Lineage**

- `Founder::RippleSnail`, named `ripple-snail` (the specimen key, so the presenter
  fallback finds it), with role `film_grazer`.

**Food**

- A new `Food::Film`, taken with `Flora::take_film` from the ground pool at the
  standing face.
- Diet is `[Film]`.
- Film yield equals the browser's foliage yield: fresh tissue, not detritus.

**Body**

- **Adult size:** 0.19 × 0.19 × 0.063 m (3:1 long to tall).
- **Pace:** cruise 0.3 BL/s.
- **Template:** the shredder. Rates scale by body volume exactly as R1's capgnawer does,
  with the shredder's reproduction mode and timings.

**Senses**

- The shredder's blind layout, the same width. Its `Chem` reads a **film** field whose
  sources are the film stock per site (`FloraView::film_sites`), built as R1's fungal
  field is. That makes it a manifest of its own, `ripple-snail`.
- `Wet` is already in the layout.

**Heuristic**

- The blind forager on its own `Chem`.

**Aquatic**

- A physiology flag `aquatic` (true for the snail only), plus the knob `water_margin_m`
  (0.25).
- An aquatic body never drowns and pays no wade penalty.
- It refuses a step whose destination support face has no standing water within
  `water_margin_m`, the way a wall refuses one.

**Eggs**

- Laid on a face holding film. This is a physiology field for the egg site:
  `litter | film`, where the shredder and capgnawer stay `litter`.

**Prey**

- On no hunter's list.

**Film in the world**

- A flora knob `seed_film_at_founding` (default **false**, so co-training worlds don't
  change). When it is true, the founding calls `seed_film`.
- The live presets set it true.

**Founding**

- `founding_site = "at_food"` on film sites.
- The code default is 0. The live presets found 8 (desktop and voidrunner terrarium,
  desktop `voxel.toml`) and 6 (Tachyon) when the world has film sites. A shortfall is
  reported, not fixed.

**Presenter**

- Draw film as floor marks. `PART_FLOOR_MARK` exists in `cubarium-gpu/src/voxel.rs`,
  but nothing emits it yet.
- Each film site gets a mark on its support face. Its colour is an interim green-gold,
  the "thin green-gold coating" of `theoretical-biosphere-2026-09-16.md`, named
  `FILM_INTERIM` in `colours.rs`. Its strength follows `film / capacity`, so a grazed
  trail shows as less tint and nothing pops (fade on the frame clock).
- If floor marks don't render without shader work, stop and report; R2b does no shader
  work.

### Tests first (separate worker; short function tests, ≤ 200 ticks)

- A snail on a submerged film site takes film, the books close, and the film pool
  falls.
- It never takes litter or foliage.
- It doesn't drown after `drown_after_s` under water, where a shredder in the same
  place does.
- It refuses a step onto dry ground beyond `water_margin_m`, and accepts one within the
  margin.
- Its `Chem` rises toward film and reads 0 where there is none.
- Its eggs go only on a film face.
- With `seed_film_at_founding` false, a founded world has no film. With it true, the
  world has film and a preset count of 8 founds snails on film sites.
- A live snail draws with its specimen model, and a film site emits a floor mark whose
  strength follows its stock.

### Measure (after the implementer)

Run 8 seeds × 60 min on the desktop terrarium with heuristics. Report:

- **Snails:** alive at 15, 30 and 60 min, bites, births and deaths by cause.
- **Film:** total film with snails against a no-snail control.

Then one headless GPU capture showing snails and film, as absolute paths.

### Return (≤ 40 lines)

- Commits.
- The table.
- Capture paths.
- Anything undecided.

Commit your own paths only. Stop every background shell before you return.

---

## Brief F3: the seedporter (after F1)

**Objective.** Seedporters climb fruiting stands and vine faces, eat fruit, glide from
grove to grove, and carry seeds in the gut. They run on a heuristic and are drawn with
their specimen model. The spec is `predation-and-flight-2026-09-28.md` §3.5, §3.7 and
§3.9 and the F3 outline in `voxel-flight-2026-09-28.md`. It builds on F1's `y_m`,
modes and perches.

**F3 is a model-rule change.** A separate test pass comes first, then the implementer.
Commit before the measure.

### Decisions (Fable; implement, don't revisit)

**Lineage and body**

- `Founder::Seedporter`, named `seedporter`, with role `glider`.
- **Body:** 0.5 × 0.25 × 0.2 m.
- **Template:** the browser's table scaled by body volume as in R1
  (r = 0.025 / 0.105 ≈ 0.24, so `body_max` ≈ 0.012, matching §3.9).
- **Activity:** diurnal; it sleeps perched in a crown.

**Movement**

- Modes `Ground`, `Climb`, `Glide` and `Perched`. Fill in F1's `Climb` and `Glide`
  stubs.
- **Pace:** walk 1 BL/s, climb 0.5, glide 3 BL/s.
- **Climbing:** on the stem of any stand at least 1 m tall (vaulttree, lanternberry,
  umbrellafrond) and on latticevine-covered faces. It uses the shredder's wall-climb
  cost factor and may perch in the crown at the top.
- **Gliding:** only from at least 1 m above the landing surface, at `glide_ratio` 2.5,
  steered by `turn`, costing 1.2 × basal. It lands on the first support face, crown or
  covered face it meets.
- All of these numbers are knobs.

**Food**

- **New `Food::Fruit`**, with diet `[Fruit]`.
  - Latticevine fruit comes through `take_fruit`.
  - Lanternberry gets a `fruit` stock on adult stands. It is paid from reserve in
    daylight up to `fruit_cap`.
  - Ripe fruit left too long falls to litter after `fruit_drop_s`.
  - Fruit is energy-rich, with `fruit_mineral_fraction` small but not zero.
  - These are flora knobs.

**Gut seeds**

- A fruit meal records the source species' seeds in the gut: `seeds_per_fruit_organic`.
- They are voided at the body's current face after `gut_transit_s`.
  - Lanternberry seeds go into the site's seed bank (`SeedCohort`).
  - Latticevine seeds go into the vine's seeding path if one exists (`VineSeed`).
    Otherwise they are dropped and noted as owed.
- Seeds are counted, not invented: they come out of the fruit's organic matter, and
  the books close.

**Senses**

- The browser's 37-input layout. Its cone counts fruit stock in view (stands and vine
  faces), where the browser's counts foliage, as the loftstrider's banded cone did.
  That makes it a manifest of its own, `seedporter`. Interim until P2.

**Heuristic**

1. Walk or climb to the nearest fruit it can see, then eat until the source is empty
   or its reserve is full.
2. Then glide toward another visible fruit source within glide reach
   (height × ratio), or climb down.
3. With nothing seen, wander on the ground.
4. Sleep perched in a crown at night, and shelter perched under a crown in a downpour.

**Prey and founding**

- **Predation:** a grounded seedporter (mode `Ground`) is on the lanternjaw's and the
  chorister's prey lists. A climbing, perched or gliding one isn't.
- **Founding:** `at_food`, near lanternberry and latticevine. The code default is 0.
  - Desktop and voidrunner terrarium, and desktop `voxel.toml`: 4.
  - Tachyon: 3.
- **Presenter:** the specimen model at `y_m`, one pose.

### Tests first (separate worker; short function tests, ≤ 200 ticks)

- It climbs a 1 m lanternberry stem at half walking pace and perches in the crown. It
  won't climb a stand under 1 m.
- A glide from 2 m covers about 5 m before landing, at 1.2 × basal. No glide starts
  below 1 m.
- A lanternberry grows fruit in daylight up to its cap, from its reserve, and none at
  night. Uneaten fruit falls to litter.
- A seedporter takes latticevine fruit and lanternberry fruit, and both books close.
- Gut seeds are voided after the transit time at the current face, into the seed bank.
- The cone reads fruit and doesn't read plain foliage.
- A grounded seedporter is lanternjaw prey; a climbing one isn't.
- Snapshot round trip for a climbing, a gliding and a perched seedporter.
- It draws with its specimen model at `y_m`.

### Measure

Run 8 seeds × 60 min on the desktop terrarium with heuristics. Report:

- seedporters alive at 15, 30 and 60 min;
- fruit eaten by source;
- glides started;
- seeds voided, and for lanternberry seeds landing in a grove other than their source:
  how many and how far;
- the time shares on the ground, climbing, gliding and perched.

Then one headless capture of a seedporter in a crown.

### Return (≤ 40 lines)

- Commits.
- The table.
- Capture paths.
- Anything undecided.

Commit your own paths only. Stop every background shell before you return.

---

## Brief R3: the chorister (after F3)

**Objective.** Choristers live as packs in the bigger worlds. Each pack has a den, one
breeder, provisioned pups and a territory. They hunt alone or as a pack and push
lanternjaws off carcasses. Everything runs on a scripted pack heuristic (D17 option 1).
The spec is `predation-and-flight-2026-09-28.md` §2.10 and §2.14 (agreed by Wrysk) and
`organism-large-animals-2026-09-23.md` §5.

**R3 is a model-rule change.** A separate test pass comes first, then the implementer.
Commit before the measure.

### Decisions (Fable; implement, don't revisit)

**Lineage and body**

- `Founder::Chorister`, named `chorister`, with role `pack_hunter`.
- **Body:** 1.4 × 0.35 × 0.6 m. The 0.8 m tail is geometry only.
- **Template:** the lanternjaw's hunter table and strike machinery, with no lure.
  - Stocks and rates scale by body volume (r = 1.4·0.35·0.6 / 1.0·0.25·0.45 ≈ 2.61), as
    in R1.
  - Specific upkeep is 2× the lanternjaw's (§2.14: "a higher upkeep"), as a knob.
- **Activity:** diurnal, so the lanternjaw keeps the night.
- **Diet:** `[Carrion]`, eaten from its own kills as the lanternjaw does.

**Prey and threats**

- The chorister hunts the frondgrazer, the loftstrider and the lanternjaw (§2.14:
  "may kill them").
- It is therefore a threat to each of them, and the lanternjaw's existing threat
  reading makes it leave a carcass.
- **The pack seam:** the size window measures prey against the **summed body of
  packmates within `pack_radius_m`** (knob, 6 m), so only a pack can take a
  loftstrider. Wounds from several attackers already add (§2.10).

**Pack**

- An animal carries a `pack` id.
- **Founding mode `pack`:** `founding_count` counts packs, and `pack_size` (knob, 4)
  counts adults per pack. The first adult is the breeder.
- **Succession:** when the breeder dies, the oldest adult in the pack breeds.

**Den**

- At founding, the breeder picks a covered site (solid within `den_cover_m` overhead,
  knob 3 m) near the founding point.
- Adults rest and sleep within `den_radius_m` (knob, 4 m).

**Breeding**

- Only the breeder conceives, with live birth (gestation).
- A litter holds 2 to `litter_size_max` young (knob, 4), only on the ordinary sustained
  surplus gate, and at most once per `litter_interval_s` (knob, 3 clock-days).

**Pups**

- Pups stay within `den_radius_m` and can't strike until `independence_age_s` (knob,
  1 clock-day). After that they join the pack.
- **Provisioning:** an adult at the den whose reserve is above
  `provision_reserve_fraction` (knob, 0.6) passes reserve to the hungriest pup there.
  The transfer runs at `provision_rate_per_s` with efficiency `provision_efficiency`
  (0.7); the loss is respired, and the books close.
- Pups also eat at the pack's carcasses.

**Territory**

- A pack's heuristic never paths into another pack's `territory_radius_m` (knob, 10 m)
  around its den. A 32 m world founds one pack.

**Heuristic, a scripted pack tactic**

- Any member that sees prey inside the pack's window makes it the pack target.
- The nearest member drives; the others flank to points `flank_offset_m` either side of
  the target.
- Each member strikes from inside its capsule.
- A chase is dropped after `chase_give_up_s` (knob, 20 s), or when a member's reserve
  falls under `chase_reserve_floor` (knob). The steep motor cost above cruise does the
  rest.
- Alone, it hunts like the lanternjaw, without a lure.
- After a kill the pack eats, then returns to the den.

**Call**

- A `calling` flag is set on a member for `call_s` (knob, 1 s) when the pack target
  changes or a member strikes. It is saved and readable by the presenter.
- The cyan flash is owed to art: no glow is drawn in R3. The manifest's `Call` module
  stays P2's.

**Founding counts**

- The code default is 0.
- Desktop and voidrunner terrarium, and desktop `voxel.toml`: 1 pack.
- Tachyon: 0 (Wrysk: bigger worlds only).
- `--introduce chorister:N` introduces N packs.

**Presenter**

- The specimen model through R0's fallback, one pose. Pups are drawn at their body
  scale.

### Tests first (separate worker; short function tests, ≤ 200 ticks)

- **Founding:** a preset with 1 pack founds `pack_size` choristers with one pack id and
  a breeder, the den covered. The code default and Tachyon found none.
- **Size window:** a lone chorister doesn't take a loftstrider; with packmates inside
  `pack_radius_m` it does. Two choristers' wounds add.
- **Threat:** a lanternjaw sees a chorister as a threat and leaves a carcass it
  approaches. A chorister can kill a lanternjaw.
- **Breeding:** only the breeder conceives. A litter is 2 to `litter_size_max`, and
  none comes before the interval.
- **Provisioning:** an adult at the den with reserve above the fraction feeds a pup at
  0.7 efficiency, and both ledgers close.
- **Pups:** a pup inside its independence age makes no strike and stays within
  `den_radius_m`.
- **Territory:** with two packs, neither pack's members enter the other's territory.
- **Succession:** the breeder dies and the oldest adult breeds.
- **Snapshot:** a round trip keeps the pack, den, breeder and calling state.
- **Presenter:** a live chorister draws with its specimen model.

### Measure

Run 8 seeds × 60 min on the desktop terrarium with heuristics. Report:

- choristers alive at 15, 30 and 60 min;
- hunts started and kills by prey, with the success rate (§2.14 aims for 15–25 %; report
  it, don't tune);
- pups born;
- lanternjaws displaced from carcasses.

Then one headless capture of a pack.

### Return (≤ 40 lines)

- Commits.
- The table.
- Capture paths.
- Anything undecided.

Commit your own paths only. Stop every background shell before you return.
