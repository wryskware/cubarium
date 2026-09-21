---
status: open
date: 2026-09-20
owner: Fable (orchestration); Wrysk decides
---

# Live founders: the sensed bodies enter the seeded habitat

Wrysk, 2026-09-20: the first live ambient instance needs the two sensed founder
bodies (littershredder = `Founder::Blind`, frondgrazer = `Founder::Browser`)
living in the seeded example habitat, not only in the frozen arena. Today the
live schedule (`cubarium_voxel_sim::Sim::new`, `sys_fauna`) is senses-free and
the habitat seeder introduces the legacy `Species::Frondgrazer` animal. This
package brings the founders into the live world with their observation-only
heuristics by default and a saved GRU centre as an option. No appendage/genome
work (that is Wrysk's senses thread), no art, no reward or score.

Owner: one Opus 5 worker, high effort (cross-crate: sim schedule, fauna step,
habitat seeder, CLI, snapshot). Files: `crates/cubarium-voxel-sim/src/lib.rs`,
`crates/cubarium-voxel-fauna/src/{lib,step,snapshot}.rs`,
`crates/cubarium/src/voxel/{habitat,mod}.rs`, `crates/cubarium/src/cli.rs`.
Read `design/voxel-senses-phase1-plan.md` ("Frozen arena contract"),
`crates/cubarium-voxel-sim/src/arena.rs` (how the arena introduces founders and
installs controllers) and `crates/cubarium/src/voxel/mod.rs` (`sensing_driver`)
first. Related evidence: `design/handoffs/voxel-senses-phase2-briefs-2026-09-19.md`
integration notes 4–5 (cue reach 1.5 m, cone 2 m).

Steps, each committed by explicit path:

1. **A live senses field.** `Sim::new` (the live schedule) accepts an optional
   settled `Senses`; when present, the live fauna system steps with
   `Fauna::step_with_senses` so the litter field updates at its own cadence
   (`UPDATE_TICKS`) from the *live* flora, which now grows, drops litter and is
   eaten. Without one, behaviour is exactly today's. Measure and report the
   per-tick cost of the field on the seeded habitat (sources = every litter
   site; the field is sparse and active-set based) at 1 and 16 threads; if it
   is more than 10 % of the tick, say so and propose, do not implement, a cap.
2. **Founders in the seeder.** `habitat::seed` introduces littershredders on
   litter-bearing soil and frondgrazer founders on open soil through
   `Command::IntroduceFounder` (hungry: the P2-C `StartingStores`), replacing
   the legacy `Species::Frondgrazer` introductions (keep that species in the
   crate; only the seeder stops placing it). Counts are the seeder's existing
   constants or new ones beside them, stated in the doc comment. Install the
   founder's own heuristic (`BlindForager` / `BrowserForager`) on each through
   `Fauna::set_controller`.
3. **Births and controllers.** Check what `step::births` does for a founder
   body. If founders reproduce, a newborn must get a fresh controller of the
   parent's kind (a controller factory per founder, not a shared instance).
   If founders cannot yet reproduce, say so plainly and leave it; do not
   invent a body model. Founder offspring are paid; never free.
4. **CLI.** The ambient run (`cubarium voxel`, no `--arena`) gains
   `--founder-policy <founder>=<centre.json>` (repeatable) to install a saved
   GRU on that founder kind instead of its heuristic; the file's lineage must
   match. Default stays the heuristics. `./scripts/run-voxel.sh --background`
   must still work.
5. **Snapshot.** Save/load must carry the founder bodies. Bump the fauna
   snapshot schema (always fresh: the new schema refuses old worlds).
   Controllers are re-installed on load by kind (heuristic) or from the same
   `--founder-policy` flags; a saved world whose founders were GRU-driven and
   is loaded without the flags is refused with a clear message, not silently
   downgraded to a heuristic.
6. **Verification.** Short tests: the seeded habitat introduces both founder
   kinds; each takes a bite within 200 ticks; the closed-ledger test still
   holds. Then one local run of the seeded habitat for 5 simulated minutes
   (`--sink png --seconds` or the web sink) reporting founders alive, intake
   per kind, and the field cost from step 1. Run `cargo nextest run -p
   cubarium-voxel-sim -p cubarium-voxel-fauna -p cubarium` green.

Constraints: explicit-path commits with the `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>` line; never `git add -A`; do not edit
`design/handoffs/README.md` or the senses briefs file; no bit-identical pins;
tests ≤ 200 ticks; nothing fixture-side enters an observation; the arena path
and its tests must be unchanged in behaviour. The `--background` rule for
windows applies. Return ≤ 40 lines: commits, the run's census line, field
cost, and what step 3 found.
