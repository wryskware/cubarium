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
