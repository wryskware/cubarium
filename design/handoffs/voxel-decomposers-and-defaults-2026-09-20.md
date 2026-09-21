---
status: open
date: 2026-09-20
owner: Fable (orchestration); decisions recorded from Wrysk
---

# Trained founders by default, a litter diet for glowcap, and a soil pool study

Wrysk, 2026-09-20, after the closed water cycle landed (`voxel-water-cycle-
2026-09-20.md`) and Fable's test that founders driven by the P3-C wander-seed
GRUs survive on the generated closed world where the heuristics die (40 alive
vs 0 after 26 simulated minutes): (1) the seeder uses the trained models;
(2) glowcap gets a litter diet; (3) explore how a soil organic-matter pool
would integrate with the whole ecology, not as a glowcap-only resource.

Shared rules: explicit-path commits ending with `Co-Authored-By: Claude Fable
5.1 <noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`; always fresh (a changed schema refuses old files);
tests ≤ 200 ticks by conservation arithmetic, no bit-identical pins; no other
constant changes than the ones named; runs may use all cores; `runs/` is
disposable. Three workers run at once on disjoint files: S1 in
`crates/cubarium` (+ a new assets directory), S2 in `cubarium-voxel-flora`,
S3 writes one design note only. Return ≤ 40 lines each.

## S1 — the live founders are driven by the trained centres by default

Owner: Opus 5, medium. Files: `crates/cubarium/src/voxel/{habitat,mod}.rs`,
`crates/cubarium/src/cli.rs`, new `crates/cubarium/assets/policies/`.

1. Copy the two centres into the crate as committed assets:
   `runs/voxel-es-p3c-blind-wander/centers/gen441-center.json` →
   `assets/policies/littershredder-p3c-wander-gen441.json` and
   `runs/voxel-es-p3c-browser-wander/centers/gen322-center.json` →
   `assets/policies/frondgrazer-p3c-wander-gen322.json` (90 and 111 KB).
   Add a short `assets/policies/README.md` naming their provenance (P3-C,
   `design/handoffs/voxel-senses-phase2-briefs-2026-09-19.md` integration
   note 6) and the rule that a centre is replaced, never edited.
2. The ambient run (`cubarium voxel`, no `--arena`) installs these two by
   default through the existing `install_founder_controllers` path, embedded
   with `include_str!` so the binary carries them. `--founder-policy` still
   overrides per lineage; a new `--founder-heuristic <lineage|all>` selects
   the observation-only heuristic as the disclosed control. Print which
   driver each lineage runs, as today.
3. The snapshot refusal rule stays: a saved world whose lineage ran a policy
   and is loaded with a different default is refused by lineage digest, not
   demoted. Adjust the existing test.
4. Verify: `cargo nextest run -p cubarium` green; then the generated closed
   world (`--scene generated --config runs/closed-generated.toml`, png sink)
   for 60 simulated minutes at default, and once with `--founder-heuristic
   all`: report the founder census lines for both (alive, bites,
   assimilated per lineage) and the flora stands. That is the evidence the
   default changed the outcome.

## S2 — glowcap eats litter as well as dead wood

Owner: Opus 5, high (a model rule in the flora crate). Files:
`crates/cubarium-voxel-flora/src/{lib,step}.rs`, its tests. Read
`design/ecology-v1-contract.md` §4, the saprotroph income rule in `step.rs`
("A saprotroph's income", step 5b) and `take_pool`.

Tests are specified here before the implementation and must be written as
stated: (a) a glowcap on a site with litter and no dead wood gains tissue over
100 ticks and the litter pool falls by exactly what the ledger booked
consumed, organic, mineral and energy each; (b) with both pools present, the
draw splits pro rata to the pools' stocks (or by the declared preference —
state which) and the sum equals the uptake bound `substrate_uptake_per_s ·
W · μ · dt`; (c) a littershredder and a glowcap on the same litter site over
100 ticks together take no more than the pool held, and the flora and fauna
ledgers still close; (d) `establishment_gates` for glowcap passes on a site
whose box holds litter ≥ the substrate gate but no dead wood; (e) the five
photo species are unchanged: their step on a litter site takes nothing.

Implementation: the saprotroph's substrate is the **sum of dead wood and
litter** in its mycelium box, both for income (step 5b draws from both pools
through `take_pool`, the same accounting the shredder's `take_litter` uses)
and for the establishment gate (`dead_wood_in_box` becomes a substrate-in-box
that sums both, named accordingly). No new constants: the existing
`substrate_uptake_per_s`, `substrate_yield` and `establish_substrate_min`
apply to the sum. If the pools' mineral or energy fractions differ, the yield
rule applies pro rata to what was taken from each; state it in the doc
comment. Carrion stays out of scope. Then: `cargo nextest run -p
cubarium-voxel-flora -p cubarium-voxel-sim -p cubarium` green, and the
six-hour `voxel_census 6 generated closed`: report glowcap's count at 1, 3,
6 h against the recorded 8 → 1 → 0 (first zero at minute 307), and the
shredder line, since the two now share a pool.

## S3 — how a soil organic-matter pool would integrate

Owner: Opus 5, high. Read-only: no crate edits. Deliverable: one design note
`design/soil-organic-matter-exploration-2026-09-20.md` with front matter
`design_status: exploration`, at most 250 lines. Read `design/ecology-v1-
contract.md`, `design/terrain-and-ecosystem-proposal-2026-09-16.md`,
`design/voxel-ecology-sketch-2026-09-16.md`, the flora crate's ground pools
(`litter`, `dead_wood`, `carrion`, their mineral and energy companions, the
`carrion_decomposition` rule) and `design/0_Canon/DECISIONS.md` for anything
that already binds soil or nutrients.

Answer, with pointers into the code for each claim:
1. What the world's organic and mineral flows are today, as a diagram in
   text: sources, pools, consumers, sinks, and what leaves the world.
2. Where a soil organic-matter pool would sit: per soil voxel, per column,
   or per support face beside the existing ground pools; what feeds it
   (litter decay, carrion decomposition, dead wood decay, root turnover),
   what draws on it (saprotrophs, mineralisation into the plants' mineral
   pool, nothing else), and at what rates the contract already implies.
3. Its effect on the whole ecology: plant mineral nutrition (does the
   mineral pool today have any source but the seeded stock?), the
   shredder–glowcap competition for litter, the decomposer's floor when
   nothing dies, and whether it closes a loop the world currently leaks.
4. Simulation cost: state per voxel or per column, which tick phase, whether
   it needs diffusion or only local decay, and an estimate against the
   measured tick (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
5. What it means for presentation (soil colour, mushrooms where the soil is
   rich) and for the game (a player enriching soil), one paragraph each.
6. A recommendation with two or three bounded packages, each with its test
   in one sentence. Make no decision; say what Wrysk would be deciding.
