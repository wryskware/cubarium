---
status: open — SHADE-REAL built on branch shade-real (unmerged; effect check written, not launched); VAULT-GUIDE open
date: 2026-10-10
owner: the ecology thread (Wrysk briefs it; it spawns the workers when it fits)
packages: VAULT-GUIDE (small, independent), SHADE-REAL (ecology, opus-high)
---

# VAULT-SPACING: real shade, seeds that travel, and a field guide that counts by stage

Rules for every worker: `design/handoffs/worker-rules-2026-10-07.md`.
- No windows.
- No tool call over 5 minutes.
- Long jobs go through labd (`scripts/lab/lab submit`), and the parent keeps one waiter.
- Explicit-path commits.
- Tests check mechanisms, never tuned numbers.
- Every new number is a serde knob with a registry row.

Eidolon is read-only: never touch the running app or its saves; copy with scp.

## Why

Wrysk (2026-10-10): "the world i have running on eidolon says there are 440+ vault trees,
but i dont think i see nearly that many displayed. definitely not unless they are all
practically on top of each other. might create a bit of an ecology issue with stands being
too close".

The diagnosis was VAULT-COUNT (`design/handoffs/vault-count-2026-10-10.md`). Its evidence:
- a copy of his save and the counts, in the main checkout's `runs/vault-count-2026-10-10/`
  (`count.txt`, `vaulttrees.csv`, `spacing.txt`, `densest-grove-topdown.png`);
- the tool `crates/cubarium/examples/vault_count.rs`, on branch `vault-count`, built on
  1270d05c (eidolon's build).

What it found, in the save at 1270d05c:
- **442 stands: 342 seedlings, 53 saplings, 47 adults.**
  - Seedlings draw 0–5 cells (median 1). Almost all of them are hidden.
  - Saplings draw a median of 13 cells.
  - The adults draw 138–1107 cells each. These are what he sees.
- **The stands sit almost on top of each other.**
  - Nearest neighbour: median 0.12 m (adjacent cells) at every stage. The nearest adult
    to another adult is at a median of 0.35 m.
  - Clumps at 1 m: 24 clumps, and the largest holds 333 stands.
  - The dense part is one 10 m arc of the ring (x 70–80 m), which holds 33 of the 47
    adults.
  - 291 of the 342 seedlings sit under a bigger vaulttree's crown.
- **Nothing thins them.** Their light is a median of 0.95 for adults and 0.54 for
  seedlings. Three things cause that:
  1. **Shade is weak.** `FloraConfig::shade_k_per_m2` is 0.09375
     (`crates/cubarium-voxel-flora/src/lib.rs` ~3130–3137, default ~3360). The law is
     `exp(-shade_k · P / crown_area)`, so a full vaulttree crown passes about 94 % of the
     light. Main's wider crown (1.85 m radius, 16684755) spreads the same stock over
     3.4× the area, so it passes even more.
  2. **Crowns of equal height never shade each other.** `step.rs` ~956 skips every
     neighbour with `c.top <= layer.top`. A grove of same-height adults therefore stands
     in full sun.
  3. **The seedling gate only asks for break-even.** `establish_light_margin` is 1 (the
     gate is near `step.rs` ~4388). The ledger shows 2,535 seeds refused under canopy and
     424 germinated anyway.
- **Seeds land close to the parent.** Vaulttree is `Dispersal::Wind` with `hop` 4
  (1 m) (`SpeciesConfig::vaulttree()`, lib.rs ~2807–2833). That puts 79 % within 3 m and
  4 % past 10 m (`Dispersal` docs, lib.rs ~905).
- **The field guide counts every stand.** `game/ui/field_guide/field_guide.gd` `_live()`
  ~309–323 shows `stands`. That number comes from `crates/cubarium-observatory/src/
  query.rs` ~474–479, which counts all stands with no stage.

## Wrysk's picks (2026-10-10)

- "A for the fieldguide": count by stage.
- "B+C sound good": real crown shade, and crowns of equal height shading each other.
- "seeds probably could travel more. idk what dispersal mechanisms it currently has
  otherwise."
- "for shade generally, it probably should be realistic numbers, which would fix other and
  future groves as well". **The shade law is global**, so this is not a vaulttree-only
  knob.

## VAULT-GUIDE (small; any lane, opus-medium or Codex high)

**Taken by the art thread (Wrysk, 2026-10-11: "you can take up the VAULT-GUIDE thread"):**
Codex high, worktree `.claude/worktrees/vault-guide`, branch `vault-guide`. The ecology thread
doesn't need to do it.

The field guide's live line for a plant shows its stands by stage, in the same form the
animals use (`"%s · %s"` with `_plural`). For example "47 mature · 53 saplings · 342
seedlings", with the head number still the total.

- The observatory's plant population gains per-stage counts. Use the sim's own stage
  definition (find it; VAULT-COUNT's tool used stages 0/1/2). Don't invent a threshold.
- Do this for every plant species that has stages; a species with none keeps "N stands".
- The words are a UI choice. Show Wrysk a headless capture of the guide before merging.
  Don't build a new layout.

## SHADE-REAL (ecology; opus-high)

Make crown shade physically real for every plant. Let a same-height grove shade itself.
Let vaulttree seeds travel as far as a winged tree seed really does. Follow
`budget-before-tuning`: put the predicted numbers in the plan before any run.

1. **Realistic attenuation.** Light under a crown follows Beer–Lambert:
   `exp(-k_ext · LAI)`.
   - Reference values:
     - broadleaf `k_ext` ≈ 0.5–0.7;
     - a closed temperate crown's leaf area index is 3–6, so it passes about 5–20 %;
     - a dense forest floor gets 1–5 % of open light.
   - Derive the law from leaf area instead of a fitted constant. Leaf area is
     `specific leaf area × foliage stock / crown area`, times `(1 − porosity)` where the
     law already uses it.
   - That needs each species' leaf area per unit of foliage stock, so map the sim's stock
     units to mass first. Cite the sources for every number.
   - The constant stays a knob, renamed if its meaning changes.
2. **Crowns at the same height shade each other.**
   - Replace the `c.top <= layer.top` skip with a physical rule: overlapping crowns share
     the light by how much of each lies above the other.
   - Equal tops should share symmetrically, not let each other off. Don't let a taller
     crown take less than it does today.
   - Use one rule for every species pair. The fungus exception and `same_face_shades`
     stay as they are.
3. **Vaulttree dispersal.**
   - List every way a vaulttree seed moves today: the wind kernel, and whether any animal
     carries or passes it (seedporter fruit carrying, guts).
   - Give it a realistic winged-seed kernel. `Dispersal::Wald` exists (springturf uses it):
     mean ≈ release height × wind / fall speed. A samara falls at about 1 m/s and is
     released from the crown, 2.25–6.5 m. Expect a mean of metres to tens of metres, with
     a tail reaching other groves on the 80 m ring.
   - Say whether `establish_light_margin` and `establish_light_min` still make sense once
     shade is real.
4. **Budget first.**
   - Before any run, state the predicted numbers:
     - light under an adult crown and in a gap;
     - seedling establishment under a crown versus in a gap;
     - adult spacing (nearest adult to adult, from a median of 0.35 m now to what?);
     - the vaulttree stand count by stage at steady state;
     - what the change does to the other species' groves (turf under trees, lanternberry,
       bloomcrown, glowcap's dark habitat).
   - Use the whole-web model for the budget (`webmodel-line`). Use short probes on the real
     sim for what the model can't place in space.
   - Food matters too: vaulttrees are the loftstrider's food (`population-targets`: one
     loftstrider family on the wide world).
5. **Check.**
   - A short run: 60 min of world time × seeds 1–8 via labd, on main's world, with an early
     stop.
   - Preflight first (`effect-check-preflight`: prove the arms differ).
   - Report the counts by stage, adult spacing and light against the prediction. Include a
     headless picture of the densest grove before and after, at desktop scale.
   - A changed save schema means fresh worlds only (`always-fresh-never-migrate`). Say if it
     does.

Return (≤ 40 lines):
- the law and every number with its source;
- the predictions, then the measured values;
- what moved for the other species;
- commits.

Merge to main by default (`merge-and-iterate`). Deploying is Wrysk's call.

## SHADE-REAL built (2026-10-10, branch `shade-real`, unmerged)

Commits: `cdaa72d4` (the law, dispersal, tests), `1b87422c` and `e31cbf74` (the `shade_probe`
example), on `c8b1ac84` (VAULT-COUNT's `vault_count`, cherry-picked).

- **Law, every foliage layer of every species:** `T = exp(−share · k_ext · (1 − porosity) · LAI)`,
  `LAI = SLA · foliage_kg_per_unit · stock / disc area`.
  - `flora.light_extinction_k` 0.6 replaces `shade_k_per_m2`.
  - Per species, `specific_leaf_area_m2_per_kg` and `foliage_kg_per_unit`.
  - The mass map is read off each analog's closed-stand LAI: over its crown disc, or over one
    face for the three ground covers.
  - Sources are in the code docs and registry rows.
- **Level crowns:** `shade_share` gives the part of an occluder's band above the receiver's
  middle. Equal bands get half each, symmetrically. A lower top gives nothing, so a taller crown
  is unchanged. The fungus exception and `same_face_shades` are as before.
- **Vaulttree seed:** the wind is its only carrier. It now uses the mechanistic WALD from the
  crown: release 0.775 × crown height, wind 2 m/s, fall 1 m/s, eddy ratio 0.1. A full tree's
  mean is 10 m, with 7.6 % past 20 m.
- **No save schema bump.** An old save takes the new knobs' defaults but keeps its own vaulttree
  dispersal. So a check must found fresh worlds, not load main's library saves.
- **Static probe on the VAULT-COUNT save** (`shade_probe --build-config`; numbers in
  `e31cbf74`):
  - under adult crowns the ground gets a median 0.34 of open light, and gaps 0.93;
  - a vaulttree newborn passes the gate in 46–69 % of under-crown columns and 96–99 % of gaps;
  - static thinning: vaulttree 442 → 147–160, umbrellafrond 1171 → 183–238, lanternberry
    1518 → 667–1036.
- **Effect check specs, not launched:**
  - `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/f216c350-6b0e-45f5-8e01-9b322c2e7f2c/scratchpad/shade-real-ab.toml`:
    8 seeds × 60 min.
  - `.../scratchpad/shade-real-ab-6h.toml`: 4 seeds × 6 h, for stage counts and spacing.

## VAULT-GUIDE worker notes (2026-10-11)

- **Where it lives today:**
  - The field guide's live line is `game/ui/field_guide/field_guide.gd` `_live()` (~309):
    plants show `_plural(stands, "stand")`, and animals already show
    `"%s · %s" % [adults, juveniles]`.
  - The count comes from `crates/cubarium-observatory/src/query.rs` `PlantPopulation`
    (~380) and the loop at ~474. Add the per-stage counts there.
- **The sim's stage:**
  - Find the definition the flora sim uses. VAULT-COUNT's tool (`vault_count.rs`, on branch
    `vault-count`) read stages 0/1/2.
  - Name the stages with the sim's own terms. Order the line largest stage first, with the
    head number still the total.
  - A species without stages keeps "N stands".
- **The Godot extension:**
  - Build it only in your worktree (`scripts/build-godot-ext.sh` writes that tree's
    `game/bin/`).
  - **Never in the main checkout**: Wrysk's app runs from it, and rebuilding the library
    under a running app crashed it (SIGBUS).
- **The picture:**
  - Take it headless or through `scripts/hidden.sh`, never in a window.
  - `game/tests/guide_test.gd` opens a save. Add a `--shots DIR` option there, or use the
    pattern in `game/tests/*_shots.gd`. Show the Vaulttree page and one plant without stages.
  - `/home/wrysk/wryskware/cubarium/runs/vault-count-2026-10-10/` holds a copy of his save.
    Use it if it still loads; otherwise found a fresh world.
- Run the observatory's tests and `guide_test.gd`.
- Sign commits `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>`. Don't merge.

**VAULT-GUIDE MERGED** main bb6b1295 (2026-10-11): stage counts from `profile_index`; Wrysk: "im sure its fine".
