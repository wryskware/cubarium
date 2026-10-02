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

## Result, 2026-10-01 (stages 0–3 run overnight)

**What ran**
- **Stage 1** (ecology only, 1 h worlds) was stopped early. Two harness problems surfaced:
  - Breeding was a binary cliff, so the search had nothing to climb. Fixed with graded
    readiness.
  - The x30 bite K was mass-scaled. That is a units bug: each bite got 17 % of its want.
    Fixed at 6526dc36.
- **Stage 2** (2 h worlds, 41 genes, soft screen margin, movement weight ×10 from g10/g8):
  - **A, the lock step:** 15 generations, 189 brain steps.
  - **B, the ecology-only control:** 12 generations.
  - Both ran on two c7g.16xlarge spot hosts from g5/g4 on. The speedup was only about 15 %,
    because the window barrier waits for the slowest world.

**Gate:** seeds 9–16 × 2 h. Columns are summed over 8 worlds; "late" counts worlds with a
late birth.

| arm | shredders alive / births / late | browsers alive / births / late | browser moving |
|---|---|---|---|
| defaults + round-2 brains | 18 / 10 / 6 | 18 / 6 / 1 | 0.40 |
| A ecology + A brains | 41 / 31 / 8 | 51 / 32 / 8 | 0.20 |
| A ecology + round-2 brains | 36 / 21 / 8 | 40 / 22 / 8 | 0.18 |
| B ecology + round-2 brains | 36 / 19 / 8 | 58 / 39 / 8 | 0.14 |
| B ecology + A brains | 33 / 34 / 8 | 53 / 34 / 8 | 0.15 |

**Reading the gate**
- **Breeding is solved:** every lineage breeds late in every world.
- **Survivors are about 2.5× the defaults.** No plant species was lost in any world.
- **The trained brains add +10 births per lineage** on the same ecology.
- **Movement is not solved.** Browsers move 0.14–0.20, below the 0.30 target and below the
  defaults' 0.40. A's brains also park the shredders, at 0.22 against 0.51.
- **Lanternjaws hatch but never establish,** in every arm.
- **Wrysk saw browsers stuck spinning in crags:** `design/handoffs/browser-stuck-in-crags-2026-10-01.md`.

The files are in `config/training/cotrain-2026-10-01/` (training only; the live defaults
are unchanged):
- `lockstep-g15-mean.toml` and its two centres;
- `control-g12-mean.toml`.

Watch it with `./scripts/run-voxel.sh --config config/desktop/terrarium.toml --params
config/training/cotrain-2026-10-01/lockstep-g15-mean.toml --founder-policy
browser=config/training/cotrain-2026-10-01/lockstep-g15-centre-frondgrazer.json
--founder-policy littershredder=config/training/cotrain-2026-10-01/lockstep-g15-centre-littershredder.json`.

## Result, 2026-10-02: curiosity lock step

**What ran.** Both runs use curiosity (`config/training/curiosity.toml`, brief
`design/handoffs/brain-curiosity-2026-10-02.md`) on the build with the crag fix and wall slide
(6e69e7b2). Both warm-start from the g15 mean and centres, with σ0 0.10, 16 brain pairs, and a
wall at 10:00.
- **C1** ran on the desktop with the calibrated weights, for 14 generations.
- **C2** ran on eidolon with 2× weights (`curiosity-x2.toml`), for 10 generations.

**Gate:** seeds 9–16 × 2 h. Each cell is summed over the 8 worlds and shows
alive / births / worlds with a late birth / moving share.

| arm | ecology | brains | shredders | browsers |
|---|---|---|---|---|
| defaults | defaults | round 2 | 61 / 16 / 7 / 0.74 | 24 / 6 / 1 / 0.73 |
| A15 | g15 | g15 | 94 / 60 / 8 / 0.32 | 33 / 13 / 3 / 0.38 |
| C1 | C1 | C1 | 34 / 41 / 8 / 0.28 | 100 / 78 / 8 / 0.33 |
| X12 | C1 | C2 | 50 / 53 / 8 / 0.43 | 57 / 29 / 7 / 0.90 |
| X21 | C2 | C1 | 103 / 80 / 8 / 0.41 | 51 / 41 / 6 / 0.38 |
| C2 | C2 | C2 | 110 / 81 / 8 / 0.51 | 41 / 17 / 8 / 0.62 |

**Reading the gate**
- **The crag fix alone** (A15 against last night's A) lifts movement from 0.20 to 0.35. It
  costs browser breeding, which drops to 3 of 8 worlds.
- **The ecology sets the shredder–browser balance.** C2's ecology carries about 105 shredders
  with either set of brains. C1's ecology favours browsers.
- **The curiosity weight sets browser movement.** The 2× brains move 0.62–0.90, and the
  calibrated brains move 0.33–0.38. That movement costs the 2× brains browser births.
- **Still failing:**
  - Springturf is lost in 1–2 of 8 worlds in every trained arm.
  - Lanternjaws never establish, with at most 5 alive and no late births.
- **Most balanced:** X21, C2's ecology with C1's brains. Every trained lineage moves above
  0.38 and has more than 50 alive, but browsers miss a late birth in 2 worlds.

The files are in `config/training/curiosity-2026-10-02/` (training only; the live defaults are
unchanged). Raw CSVs are under `runs/gate-curio/`.
