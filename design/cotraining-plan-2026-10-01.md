---
design_status: exploration
last_reviewed: 2026-10-01
decision_refs: []
---

# Co-training: brains and ecology in lock step

A proposal. Wrysk asked on 2026-10-01 to start the large balance pass, where the brains
train and the ecology parameters are tuned together. Wrysk's answers to the open questions
are at the end.

## What already exists

- **Live-world brain training,** `voxel-live-train`
  (`design/handoffs/voxel-live-world-training-2026-09-24.md`).
  - Worlds persist, and every body carries its own variant.
  - Each lineage gets one ES update per window.
  - The score is (intake − motor)/ref + 0.25·survived.
- **Ecology search,** `voxel-ecology-search`
  (`design/handoffs/voxel-ecology-search-2026-09-24.md`).
  - CMA-ES over the hand-coded vector in `es/voxel/live/ecology.rs`: 9 shared multipliers
    plus per-lineage ones.
  - Each candidate runs on freshly founded worlds, with common seeds across a generation.
  - The score is a band penalty: 30–80 animals per small world, a peak of 120, browser
    moving share ≥ 0.30, every lineage and plant species persists, and every lineage
    breeds in the second half.
- **Unified mode** (`--brain-pairs > 0`). The same worlds also carry antithetic brain
  variants. Each generation takes one CMA step for the ecology and one ES step per
  lineage. This is the lock step. It is built but has never been run.
- **The registry, R1 and R2** (2026-10-01): 851 registered knobs. The search cannot
  reach them yet, because it still decodes the old hand-coded vector.

## What the sequential rounds showed

- **es3** tuned a vector that the audit then discarded: walking cost 4× basal
  (audit §1.1).
- **The x30 terrarium gate** (2026-09-29) showed that training ≈ round 2. Everyone
  starves, there are about 0 births, and the heuristic dies out too.
- So the brains get almost no signal in today's world. A brain cannot learn to forage
  well in a world where nobody lives out the first hour.

## What is missing

1. **R3.** It is specified in `design/ecology-parameter-registry-2026-09-28.md` §3–5 and
   adds:
   - search files and the generic decode;
   - 20–40 master genes with `derive()`;
   - the R/S/F pre-screen, so worlds that fail by arithmetic never cost a run.
2. **Targets for today's world.** The 30–80 band was set for the small two-lineage
   world. Today's world has three lineages (the lanternjaw, a heuristic ambusher that
   trains from P2), the terrarium and today's plant set.
3. **The brain reward.** This has been open since 2026-09-28 (audit §8.3). The score
   cannot reward a sated walker.
4. **A viable starting vector.** Lock step from a starving world gives the brains noise.

## Proposed loop

**Stage 0: R3 plus targets v2.** One package: an opus-high pass writes the tests, then an
opus-medium pass builds it.
- **Genes:** registry §4's list, per lineage and per plant group. These are mass scale,
  upkeep, motor factor, intake factor, yield, the breeding reserve, lifespan, plant
  growth, leaf life, foliage per wood, defence onset and relaxation, decomposition, and
  `K_bio` (neutral).
- **Ranges** wide enough to reach the audit's real-world baselines, so the search can move
  ×1 there itself instead of in a separate manual step.

**Stage 1: viability.**
- **Search:** ecology-only CMA. The brains are held: round 2's centres by transfer, and
  the lanternjaw's heuristic.
- **Screen:** R/S/F skips the arithmetic failures.
- **Each candidate:** 8 common seeds × 1 h on the terrarium.
- **Exit:** a vector under which every lineage breeds in the second half hour and none
  starves out.
- **Bound:** at most 3 h of wall time.

**Stage 2: lock step (unified mode).** It starts from the stage-1 vector.
- **Each generation:** N ecology candidates × the common seeds. Every world also carries
  brain pairs. Then one CMA step, and one ES step per trained lineage.
- **Once at the start:** a short σ = 0 brain control, to show the signal exceeds the
  placement noise.
- **Every 10 generations:** a held-out check on seeds 1–8 × 1 h. The current centres and
  mean vector are compared with the stage-1 result.
- **Bound:** an overnight run, with the wall time fixed when it launches.

Why unified rather than alternating blocks:
- It is already built.
- Brains learn across a spread of ecologies, so they don't overfit one vector.
- There is no ping-pong where each half undoes the other's last block.

The noise risk is handled by normalising within each world, which the code already does,
and by a small σ.

**Stage 3: gate.**
- 8 seeds × 2 h: the tuned vector with the trained brains, against the defaults.
- Then Wrysk watches it live. It runs from a training-only config, and the live defaults
  stay unchanged until Wrysk says otherwise.

**Compute.** Measure ticks per second on the terrarium first, then pick the host.
- The x30 training ran about 200 s per generation on the desktop.
- Eidolon is free for training.
- AWS Graviton only on Wrysk's go.

## Wrysk's answers (2026-10-01)

1. **Targets: persist, breed, bounded.**
   - Every lineage and every plant species persists.
   - Every lineage breeds in the second half.
   - Animals stay in a band.
   - Browsers have a moving share of at least 0.30.
   - Lanternjaws are 5–15 % of the animals.
2. **Brain reward: net energy, including the loss at death.** This replaces
   (intake − motor)/ref + 0.25·survived.
3. **Compute: eidolon at 100 %.** The desktop may also use 90–100 %: there are no other
   agents on it, and these processes don't saturate the cores.

Stage 0's brief is `design/handoffs/cotrain-stage0-2026-10-01.md`.
