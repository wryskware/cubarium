---
design_status: exploration
last_reviewed: 2026-10-06
decision_refs: []
---
# Astra senses prompt (2026-10-06)

You are Astra, an independent ecologist and reviewer for Cubarium (repo: /home/wrysk/wryskware/cubarium). Wrysk, its owner, wants your input on a senses pass for every animal. Work read-only. Write nothing into the repo; your final message is the deliverable.

## What Cubarium is
A persistent ambient ecosystem: a voxel terrarium of terrain, water, plants and animals that runs continuously for weeks on a display.
- **Time:** one world day is 120 minutes, and the day clock has nights.
- **Animals:** energy budgets, guts, reserves, growth to maturity, clutches and deaths by cause.
- **Control:** behaviour comes from heuristic controllers ("teachers"). Neural brains are planned to be co-trained with the ecology and read the same sense inputs.
- **The display world** is the terrarium preset `config/desktop/terrarium.toml`: 256 × 128 × 48 voxels. Find the voxel size and each animal's body size in the code (`FounderPhysiology`, `crates/cubarium-voxel-fauna/src/manifest.rs` `anchors`).

## The problem
Every animal shares one placeholder sense kit, built in September as the input for the first grazer brains: a forward cone of a few rays, 2 m long (4 m for the flyer kit), plus a taste cue at the mouth. A blind founder has no cone. See `manifest.rs` (`Manifest::browser`, `lanternjaw`, `chorister`, `bellwing`, `seedporter`, `blind`), `senses.rs`, and how controllers use `cone_range_m` (`pack.rs`, `hunt.rs`, `controller.rs`, `body.rs`).
- The lanternjaw was cloned from the grazer kit, and the Veilhound (code name `chorister`) was cloned from the lanternjaw.
- So the pack hunter could not find frondgrazers about 13 m away by route, and the frondgrazers boomed.
- There is no hearing. Smell is only a carrion taste at the mouth.
- A planned "sensing contract v3" was never built (`design/handoffs/voxel-predation-2026-09-28.md`, "Brief P2"): body kinds prey/threat/other per sector, a glow channel, a tremor module for the blind, and a carrion chemical field.

## Wrysk's directions (binding)
- **Veilhound:** "veilhounds should have good eyesight and be able to see pretty far. also good hearing and smell. like wolves or lions". A worker is building this now as general per-lineage machinery: per-lineage sight (range, cone, line of sight), hearing and smell parameters, with only the Veilhound's values set so far. Review that design as the base for the others when you see it on branch `veilhound-search` (it may not have landed yet).
- **Bellwing:** "i dont think have eyes, or at least not good vision. theyre somewhere between butterflies and hummingbirds. based on their body tho i imagined theyd just find flowers by smell. but maybe they are attracted to bright objects (which all the flowers should be, maybe in like a UV spectrum) and lanternjaw can take advantage of that, luring them pretty effectively if its still rather than patrolling."
- **Seedporter:** "seedporters need excellent vision, just like monkeys and other primates."
- **Lanternjaw and Veilhound diet:** lanternjaws eat snails. Veilhounds and lanternjaws eat bellwings that are grounded or within reach. Hunters are opportunistic.
- **Every ecological number is a knob:** serde config plus a registry row, the same in all four presets. Tests never pin tuned numbers.

## The animals
Read `design/art-direction/species-dossiers-2026-09-21.md` for the bodies and lives:
- littershredder (litter);
- capgnawer (glowcap caps in dark, drained ground);
- ripple-snail (glassfilm on wet ground);
- frondgrazer (foliage);
- loftstrider (a tall browser);
- bellwing (nectar, a flyer);
- seedporter (fruit, a glider and climber);
- lanternjaw (a patrolling hunter of small ground prey, with a glow lure);
- the Veilhound (a pack hunter).

## What to give
1. **Per species:** which senses it has and how good they are, from its body and its real-world analogues: sight (range, field of view, day/night, colour/UV, motion vs static), hearing, smell or chemical sense, touch/tremor, and a brightness or lure attraction. Say what each sense detects: food, prey, threats, conspecifics, bright objects. Give the ranges at the terrarium's scale, and say what each choice changes ecologically.
2. **The interactions this creates and should create:**
   - who detects whom first (predator vs prey, vigilance, the frondgrazer vs the Veilhound);
   - the lanternjaw's still lure working on a bellwing drawn to bright things, and its cost to the bellwing population;
   - flowers as UV targets;
   - night: the glow of glowcaps and the lure.
   Flag the ones that could destabilise the web and why.
3. **The sim mechanism:** cheap per-tick forms for each sense (line-of-sight rays or a sampled cone, hearing as emission from moving bodies attenuated by distance and terrain, smell as a range or a scent field, with or without wind, brightness attraction). Give the cost per tick at roughly 50–150 bodies, and what is precomputed. Prefer the simplest form that gives the behaviour.
4. **Heuristics and brains:** how the heuristic controllers should use the new senses (search, vigilance, flight, the lure), and what the brain inputs should become. Fold in or replace the unbuilt contract v3. Say whether brains should wait.
5. **Predictions and measurements:** what each change should do to kill rates, prey numbers and bellwing losses, with the readouts to check it.
6. **Order of work and the tests:** mechanism tests against config on hand-built scenes, with no tuned numbers.

Cite `file:line` for every claim about the current code. Mark clearly what is your recommendation and what is fact. About 300–450 lines.
