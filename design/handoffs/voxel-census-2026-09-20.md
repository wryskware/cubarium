---
status: open
date: 2026-09-20
owner: Fable (orchestration); a local-model worker takes it
---

# Voxel census: does the seeded habitat last unattended?

Backlog question 1 (`voxel-era-backlog-2026-09-18.md`), narrowed to one
read-only measurement. Deliverable: one new example binary and one CSV.

## The program

Add `crates/cubarium/examples/voxel_census.rs`. It must:

1. Build the default world: `let cfg = cubarium::voxel::VoxelConfig::default();`
   then `let mut world = cubarium::voxel::scene::authored(cfg.world.clone());`.
2. `let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());`
   `let mut fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::default());`
   `let seeded = cubarium::voxel::habitat::seed(&mut world, &mut flora, &mut fauna);`
   Print `seeded` (`stands`, `logs`, `founders`, `litter_tiles`, and
   `seeded.animals()`) to stderr.
3. Settle the litter cue field the live founders sense, then build the live sim:
   `let mut senses = cubarium_voxel_fauna::Senses::new();`
   `senses.settle(&world.view(), &flora.view());`
   `let mut sim = cubarium_voxel_sim::Sim::new(world, flora, fauna, cubarium_voxel_sim::SimConfig::default(), Some(senses));`
   (five arguments; the last is the senses field, since commit 815de70).
   and call `sim.step()` in a loop. Ticks are 20 per simulated second
   (`cubarium_voxel_fauna::TICK_HZ`).
4. Every simulated minute (1,200 ticks) print one CSV row to stdout with:
   `sim_min`, then for each plant species in `cubarium_voxel_flora::Species::ALL`
   the count of stands of that species (`sim.flora().view().stands`, field
   `species`), then for each animal species in
   `cubarium_voxel_fauna::Species::ALL` the count of animals
   (`sim.fauna().view().animals`, field `species`) and their mean `body`,
   then `flora_births`, `flora_deaths`, `fauna_births`, `fauna_deaths`
   from `sim.flora().view().ledger` and `sim.fauna().view().ledger`
   (fields `births`, `deaths`), and the total litter organic
   (sum of `g.litter` over `sim.flora().view().ground`).
   Print a header row first. Column names must be readable, e.g.
   `stands_bloomcrown`, `animals_frondgrazer`, `body_frondgrazer`.
5. Take one argument: simulated hours to run (default 6). Stop after that.

Check the exact enum variant names and field names in
`crates/cubarium-voxel-flora/src/lib.rs` and `crates/cubarium-voxel-fauna/src/lib.rs`
before writing them; use `Species::ALL` and a `match` or `format!("{s:?}")`
lower-cased for the column names. If a field named here does not exist,
find the real one with `graft grep "<name>"` and use that; do not invent
fields.

## The run and the report

- `cargo run --release -p cubarium --example voxel_census -- 6 > runs/voxel-census-6h.csv`
  (create `runs/` if needed; it is git-ignored). Expect a few minutes.
- Then write `runs/voxel-census-6h-summary.md`, at most 30 lines: for each
  plant and animal species, the count at 0, 1, 3 and 6 hours; the hour any
  species first reaches zero; whether animal births ever exceed zero; whether
  litter accumulates or drains. Numbers only, from the CSV. No advice.

## Rules

- Commit only `crates/cubarium/examples/voxel_census.rs` with
  `git add crates/cubarium/examples/voxel_census.rs && git commit -m "..."`.
  Never `git add -A`, never `git commit -a`. End the commit message with the
  line `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Do not modify any other file. Do not change any simulation rule or constant.
- `cargo build --release -p cubarium --example voxel_census` must succeed
  with no warnings from the new file.
- Report back in at most 20 lines: the commit hash, the summary file path,
  and the four-row species table.
