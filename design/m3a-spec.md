---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# M3a specification — inheritance and visual representation

Implementation-level plan for M3a in the [implementation plan](implementation-plan.md):
the versioned genome with sparse mutation, lineage records, the small body
grammar and gait, and the E4/E5/E6 harnesses. It builds on the
[M2 world](m2-world-spec.md) and the [evolution](evolution.md) and
[appearance](appearance.md) proposals. Nothing here is canon; every number
is a hypothesis for the experiments named.

## Genome v2

Genome v1's fields stay (`size`, `metabolism`, `sense`, `reserve`, `mouth`,
`speed`, `hue`, the drive vector) with the same closed ranges. Three body
controls and two gait controls join them:

| Field | Range | Effect |
| --- | --- | --- |
| `lobes` | 1–4 (integer) | Body lobe count along the axis; capacity `S_adult` is unchanged, structure is redistributed |
| `aspect` | 0.6–1.8 | Length/width of the lobe chain; drag multiplier `aspect^0.5` on turning, speed multiplier `aspect^0.25` |
| `appendage` | none / head / tail (enum) | Head: mouth rate × 1.25, upkeep + 10 %; tail: `v_max` × 1.2, upkeep + 10 %; none: no change |
| `gait_period` | 0.4–3.0 s | Oscillator period; the renderer's step/glide cadence and, for tails, a paid thrust pulse |
| `gait_amplitude` | 0–1 | Visual excursion of lobes about the axis; no physiological effect (declared decorative) |

Functional consequences are those listed and nothing else. `gait_amplitude`
is the one declared cosmetic control besides `hue`.

## Mutation policy (fixed per run)

At every birth with `mechanisms.mutation` on: with probability `p_mut`
(default 0.5) the child genome differs from the parent's; otherwise it is an
exact copy. A mutated child changes `1–3` loci (uniform in that range) chosen
without replacement from the mutable set. Each locus changes by a Gaussian
step with standard deviation `0.08` of its range (clamped to the range), except
`lobes` (± 1 with equal probability, clamped) and `appendage` (uniform
re-draw among the three values). Drives mutate as loci like any other. Draws
come from `Stream::Birth` (key = parent slot, counters after the placement
draws). Mutation summaries (`locus`, `from`, `to`) are recorded in the birth
event and nowhere else in the world.

E4 restricts the mutable set to `size`, `metabolism`, `sense`, `reserve` via
`mechanisms.mutable_loci` (a list; empty = all).

## Lineage records

Each organism carries `lineage_depth` (founder 0, child = parent + 1) and
`founder_id` (the founding slot); both go into birth and death events. The
observer computes ancestry depth distributions, lineage survival, and
reproductive skew from the event log; the world keeps no genealogy.

## Body and gait

The renderer derives the body from the phenotype only:

- Lobe chain: `lobes` discs along body `+x`, centered so the chain's centroid
  is the anchor; total length `L = (1.4 + 0.9·(lobes − 1)) · size · aspect^0.5`,
  disc radius `r = (0.9 + 0.4·size) / aspect^0.25`, clamped so `L/2 + r ≤ 9`.
- Appendage: head → an extra disc of radius `0.6·r` at `+x` beyond the chain;
  tail → a disc of radius `0.5·r` at `−x` that oscillates across the axis by
  `gait_amplitude · r` with the gait phase.
- Gait: phase advances by `dt / gait_period`; lobes shift along the axis by
  `gait_amplitude · 0.4 · sin(2π·phase + k·π/2)` for lobe `k` (a crawl); tails
  swing as above. Moving effort scales the amplitude (`effort^0.5`); resting
  bodies are still.
- Brightness rules from M2 stay (mode, feeding fill). Hue accent stays.

Renderer-only phase state lives in the presenter keyed by organism ID; no
gait state is simulated or checkpointed.

## Experiments

### E4 — minimal-grazer attractor

Twelve seeds, 24 simulated hours, M2 defaults, `mechanisms.mutation` on with
`mutable_loci = [size, metabolism, sense, reserve]`, `p_mut = 0.5`; a matched
control with mutation off. Metrics from the event log and periodic genome
samples (`capacity.genome_sample_seconds = 300`, writing every living genome
to `genomes.jsonl`): per-locus distributions, fraction within 5 % of the lower
bound, parent–child differences, survival in lean patches by size quartile,
and intake/maintenance by size. Gate as in [experiments](experiments.md).

### E5 — mutant viability and readable habits

For the founder genotype: 1,000 single-step mutants (all loci mutable), each
run alone in a fixed habitat scene for 20 simulated minutes (patch present
for 10 minutes, then removed), offspring counted and removed. Measures:
feeds, survives, buds; pause durations, path curvature, directed feeding
response, patch residence, and hunger-memory persistence. A `cubarium probe`
command runs one genome in the scene headlessly and prints the measures; the
harness loops over mutants. Gate per the experiment plan.

### E6 — small visual grammar

Twelve genomes spanning `lobes` × `aspect` × `appendage` × gait, rendered in
a gallery scene (`cubarium demo --scene gallery`) that places them on Front,
Right, and Top including two seam crossings and one vertex crossing, moving
at their `v_max`. Wrysk judges on the cube: which are related, which way they
move, which feed, pause, bud. Recorded in `design/7_Research/`.

## Order of work

1. Genome v2 fields, decode, mutation policy, lineage fields, birth-event
   mutation summaries; schema bump. Tests: bounds, sparse-locus counts,
   determinism, exact copies when `p_mut = 0`.
2. Renderer body/gait from phenotype; gallery scene; E6 on the cube.
3. `cubarium probe` and the E5 harness; run E5.
4. E4 batch through the E2 harness with genome sampling; analyzer additions.
5. Decide M3b entry from E4/E5/E6 evidence.
