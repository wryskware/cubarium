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
| F1 | Flight substrate + bellwing | `voxel-flight-2026-09-28.md` F1 (worktree overridden to this one) |
| R2 | Ripple-snail + its wet-surface film producer | written when R1 lands |
| F3 | Seedporter: climb, glide, lanternberry fruit, gut seeds | `voxel-flight-2026-09-28.md` F3 outline |
| R3 | Chorister: pack, den, live birth, provisioning, territory | `predation-and-flight-2026-09-28.md` §2.10, §2.14 |

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

## Brief R2b (outline; written in full when R1 lands): the ripple-snail

- **Food:** `Food::Film` from `take_film`. Diet `[Film]`.
- **Body:** 0.19 m, 3:1 long to tall, slow (cruise about 0.3 BL/s).
- **Senses:** the blind layout with `Chem(film)`; `Wet` already exists.
- **Water-bound:** never drowns. It refuses a step whose destination has no standing
  water within `water_margin_m`.
- **Eggs:** laid on a face with film.
- **Prey list:** on none yet.
- **Founding:** on film sites.
- **Presenter:** an interim film tint on submerged support faces, named interim until
  Wrysk picks a colour. A grazed trail shows as less tint.
