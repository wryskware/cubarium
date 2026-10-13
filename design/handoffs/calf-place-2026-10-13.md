---
status: open
date: 2026-10-13
owner: Opus (high), worktree `.claude/worktrees/calf-place`, branch `calf-place`
---

# CALF-PLACE: place loftstrider calves with their mothers, and read whether strays get caught

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only for what you run yourself
  (`quick = true`, `hosts = ["desktop"]`, `cwd` set, under 15 minutes). The batch below is
  submitted and handed back; never wait on it.
- Build in your own worktree's `target/` (reflink-seeded) at -j 8 or lower.
- The desktop is busy with brain training; eidolon is free for the batch.

## Why

Topic 8 (Wrysk: a rare feast, about one loftstrider a world-year, **mostly strayed calves**).
With the defence on, 16 eight-hour wide worlds (`runs/lab/1012-131142-balance-wide-8h-ae4a`)
had no loftstrider wounded or killed, and no calf was ever born: a loftstrider needs about
7 hours to build the reserve for a pregnancy. The calf half of the target is untested.
Wrysk (2026-10-13) chose placing calves over a 20-hour run: "extend the api if you have to
(its a dev api for now anyway). start from checkpointed worlds could be useful too."

## What exists

- Scenario files: `voxel_census … edit=FILE.toml`, `crates/cubarium-voxel-sim/src/edit.rs`
  (`Scenario`, ~line 620: `[[populate]]`, `[[place]]` with `stage = "juvenile"`), applied
  after founding or after `load=`.
- A placed juvenile has no mother and an age of 0. `Fauna::stage_identity(id, age_ticks,
  mother)` (`crates/cubarium-voxel-fauna/src/lib.rs` ~5514) sets both, but only at tick 0.
  The staged defence scenes (`defend.rs` `Scene.mother`) and the nurse/follow tests
  (`nurse_tests.rs` `young`, `follow_tests.rs` `young`) build calves with mothers: copy that.
- Young stray more with age (topic 7, `young.excursion_m`, doc §3.5); calf and mother bond
  both ways. A calf without a mother or with age 0 would neither bond nor stray like a born
  one, so the result would mislead.

## Do

1. **Extend the scenario file** so a placed calf is like a born one:
   - a way to give each adult of a lineage `n` calves beside it, at an age (`age_s`), with
     that adult as mother (a new `[[calve]]` entry, or `mother`/`age_s` on `[[place]]`/
     `[[populate]]`; your choice, say why);
   - whatever a born calf has that a placed one lacks: the mother's nursing/lactation state,
     the calf's stores at that age, the bond marks. Read the birth path (`step.rs`
     `newborn` ~5733 and where `mother` is set after it) and say what you set.
   - It must also work after `load=` (tick ≠ 0), so checkpointed worlds can be used.
   - Mechanism tests on small fixtures: a calved calf has its mother and age; the mother
     treats it as hers (nurses / guards as the code does for a born calf); it works on a
     loaded world.
2. **Checkpointed starts.** There are no wide-world saves on the current schema (the
   balance saves are schema 84; main is 88). Make the batch save hourly checkpoints and keep
   them, so later checks can start from settled worlds. If you find current-schema wide saves
   from a main run, say where; don't spend long looking.
3. **Quick-lane liveness** (one seed, ~10 world-minutes, wide world): calves exist with
   mothers, nurse, follow; nothing dies at once.
4. **Submit the batch** on main's config `config/desktop/terrarium-wide.toml`, your branch's
   binary (`binary = { tree = <worktree>, commit = <your commit> }`), seeds 1–8,
   3 world-hours, `--threads 2`, one calf per adult loftstrider at a young age where
   straying has started (pick from the knobs, say which). Use two `batch` jobs of 4 seeds on
   `hosts = ["eidolon"]` (one L3 group holds 10 threads; a 16-thread batch can't be placed).
   Make sure the scenario file reaches eidolon (check how `batch` stages `--config`; staging
   an arm's `edit=` may need a small labkit change, or pass it the way `--required` is).
   Example spec: the cancelled `1013-024623-ls-census-20h-a-50a2` (`runs/lab/…/` has it).
5. **A read script** for the finished batch, per world and total: calves placed, calf-hours
   alive, time calves spent beyond the excursion distance from their mother, standoffs,
   calves wounded/killed (by whom), adults killed, hounds wounded/killed, pack kills by
   prey. Use census columns and `scripts/studies/balance-model.py --census` where they
   cover it; add census columns if one is missing (a census column change is fine).

## Boundaries

- No change to behaviour, knob defaults or presets. The scenario API is dev-only; new
  fields default to today's behaviour.
- A save change bumps the fauna schema to the next free number; say which.
- Don't merge; the parent merges.
- EC2 is never yours.

## Return (at most 40 lines)

- The API you added (file:line) and what a calved calf gets, with the tests.
- The liveness check's job id and what it showed.
- The batch job ids, output paths, the scenario file, and the exact read command.
- Commits, schema number if any, and every departure from this brief, with why.
