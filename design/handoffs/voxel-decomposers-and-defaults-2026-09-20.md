---
status: S1 and S2 landed; S3 moved
date: 2026-09-20
owner: Fable (orchestration); decisions recorded from Wrysk
---

# Trained founders by default, a litter diet for glowcap, and a soil pool study

Wrysk, 2026-09-20, after the closed water cycle landed (`voxel-water-cycle-
2026-09-20.md`) and Fable's test that founders driven by the P3-C wander-seed
GRUs survive on the generated closed world where the heuristics die (40 alive
vs 0 after 26 simulated minutes): (1) the seeder uses the trained models;
(2) glowcap gets a litter diet; (3) a brief, not yet dispatched, on how a soil
organic-matter pool would integrate with the whole ecology.

Shared rules: explicit-path commits ending with `Co-Authored-By: Claude Fable
5.1 <noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`; always fresh (a changed schema refuses old files);
tests ≤ 200 ticks by conservation arithmetic, no bit-identical pins; no other
constant changes than the ones named; runs may use all cores; `runs/` is
disposable. Two workers run at once on disjoint files: S1 in
`crates/cubarium` (+ a new assets directory), S2 in `cubarium-voxel-flora`. Return ≤ 40 lines each.

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

Moved to its own brief, **not dispatched** by Wrysk's instruction:
`voxel-soil-pool-brief-2026-09-20.md`.

## Integration note (Fable, 2026-09-20, at 864f926)

**S1 landed** (c510382): the two P3-C centres are committed under
`crates/cubarium/assets/policies/` with a provenance README, embedded with
`include_str!`, validated against this build's manifest digest, and installed
on the ambient run's founders by default; `--founder-policy` overrides,
`--founder-heuristic <lineage|all>` selects the observation-only control.
One simulated hour on the generated closed world: littershredder 24 alive /
95,668 bites on the trained default against 0 alive / 6,798 on the
heuristics; frondgrazer 0 alive in both arms (bites stop at about minute
26). The browser die-out is a new question, handed to Wrysk's thread in
`voxel-browser-autopsy-2026-09-20.md`. Known limit: the fauna snapshot holds
one bool per lineage, so loading a policy-driven world under a *different*
trained centre is not refused; refusing it needs a policy digest beside the
flag (a fauna-crate change, not made).

**S2 landed** (3d1841b, 864f926): a saprotroph's substrate is dead wood plus
litter, drawn pro rata by stock through the existing per-pool draw, yield
applied per withdrawal (litter's retained energy is capped, a log's is not).
No constants moved, no schema change, photo species untouched. The five
tests specified in the brief are in `tests/round5b.rs` under the names
given there. Six-hour census on the generated closed world, glowcap at
1 / 3 / 6 h: 12 / 10 / 9 (never below 7) against the baseline 8 / 8 / 0
(zero at minute 307). Fable confirmed the two CSVs. Litter is 4–8 % lower
at every checkpoint (the fungi now drain it); plant stands at 6 h are 273
against 286, consistent with fungal tissue holding mineral that litter decay
used to release, a side effect for Wrysk to weigh with the soil-pool brief.
The shredder line is unchanged because `voxel_census` still runs the
heuristics: the example does not go through the ambient run's built-in
default. Fable pointed `voxel_plant_autopsy` at the summed substrate.

**S3** was run by Wrysk directly; its note is `design/soil-organic-matter-
exploration-2026-09-20.md` (3f44c37).

Follow-ups landed (Sonnet, 2026-09-20): 0abd911 — `voxel_census` and
`voxel_founder_autopsy` install the built-in trained default through
`install_default_founders` and print the driver per lineage; a trailing
`heuristic` argument keeps the control. 40e4c18 — fauna snapshot schema 8
records a per-lineage policy digest (`EpisodeDriver::digest`, FNV-1a over
the weights); a loaded world whose lineage ran a different centre is refused
by name and both digests, the same centre is accepted; tests in
`crates/cubarium/src/voxel/mod.rs`. Workspace suite 1,971 green. Fable
confirmed the census now announces the built-in centres.
