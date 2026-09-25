---
status: open
date: 2026-09-24
owner: Fable (organism line); Wrysk said go 2026-09-24
follows: retrain round 2 (runs/r2 in worktree retrain2, not shipped)
---

# Live-world training: brains learn inside the running ecology

## Why

Wrysk, 2026-09-24: "whenever we balance ecology, the brains are frozen, and whenever we
train brains the ecology is frozen. can we come up with a way to do all this together?"

Retrain round 2 shows the cost of the split:
- Its brains survive far better than the heuristic (shredders alive at 60 min 2 → 15 on
  default, zero drownings).
- But they park: browsers walk about 5 m an hour (the heuristic about 500).
- In 6 h censuses the population booms to 650–800 animals.

Two causes compound:
- **Training** asked the wrong question. Browser episodes are 4 minutes in a frozen world,
  too short to strip a crown, and walking costs energy.
- **The live world** doesn't punish parking. A crown regrows fast enough to feed a parked
  browser for over an hour.

Wrysk's direction:
- Start with a small world from the natural (staged) generator, without flowing water.
- Consider a "fast water" that is good enough without full fidelity.
- Keep more animals in mind (predation and flight are only designed, not built; the
  seedporter will climb latticevine).
- Ecology factors are too many to list by hand; tune them ES-style too.
- Spend the design effort on which large-scale factors to measure.

This package is the first step: the brains half, with the ecology readout built in and
a seam for the ecology half.

## What to build

Worktree `.claude/worktrees/live-train`, branch `live-train` off main.

### 1. The training world

**The world:**
- The `small` staged preset (160×72×24 at 0.125 m), with its flowing water off:
  - no river re-entry (`reentry_m3_per_s = 0`);
  - no tier pools required (`min_tier_pools = 0`);
  - anything else the staged generator uses to make water run.
- Keep the lake, the soil water and the showers. Rising water after rain is one of the
  things the brains must now see: frozen water is why round 1's brains drowned.
- Set this through config. The shipped presets stay unchanged.
- Everything live: plants grow, regrow, seed and die; litter falls; animals breed and
  die; water moves.

**Fast water:**
- **Profile first.** Measure this world's tick by phase at `threads=1`, with the founders
  and at a boomed population (a few hundred animals).
- If water is under ~25% of the tick, stop there and report.
- Otherwise add a **training-only** cheaper water mode and prove it is good enough.
  Candidates, in order:
  1. the water step every k ticks at the same per-second rates;
  2. still water as a level per basin with the soil field below it;
  3. your own better idea.

  "Good enough" means, over seeds 1–4 and 1 h of simulated time:
  - lake level, soil moisture and plant cover within the seed spread of full water;
  - the animals' water readings (Wet, standing depth, drownings) statistically alike.
- Never the live app's default.

### 2. Every animal carries its own brain variant

- Today a lineage shares one driver. Give each body its own weight vector, a handle into a
  per-generation table so it isn't a copy per body.
- **ES over many persistent worlds, both lineages at once:**
  - One world per worker, `threads=1`. Worlds are **not reset** each generation, so the
    ecology carries on while the brains change.
  - A generation is a window of W simulated minutes. At its start, every living body of
    a lineage gets a variant from that lineage's antithetic pairs; newborns during the
    window get one too.
  - Twins of a pair go to nearby bodies of the same lineage where possible, so luck of
    place cancels.
- **The score** per body per window keeps today's form so numbers compare:
  (intake − motor)/body_ref + 0.25·survived.
  - Normalise within the world before pooling, so a rich world doesn't outvote a poor one.
  - Each lineage gets its own ES update, as now.
  - **No new reward terms.** Whether parking survives live training is the question this
    package answers. If it does, the lever is the ecology, not the reward.
- **Population bounds, training only:** a world that loses a lineage, or passes a cap you
  choose for cost, is replaced by a fresh world or a saved snapshot of another. Report
  how often.
- **Start** from round 2's centres:
  - `retrain2/runs/r2/es-browser/centers/upd160-heldout.json`;
  - `retrain2/runs/r2/es-blind/centers/upd512-heldout.json`.

  They match this build's manifest.
- **Choose W, pairs per lineage and the world count by measurement,** and report why.

### 3. The ecology readout, every generation

The same worlds report, per generation and pooled over worlds:
- Population per lineage, births and deaths by cause (starved, drowned, other) per hour,
  and median age.
- Moving share: body-minutes moving over 0.1 BL/s. Mean pace in BL/s. Unique area per
  body-hour.
- Plant stands per species, cover, standing foliage, woody share.
- Litter and carrion stocks, lake level, and soil moisture.
- Tick cost (ms per tick per world).

This is the draft list of large-scale factors. Wrysk will choose targets from it later, so
make adding a factor a one-line change.

### 4. The seam for the ecology half (do not build the search)

- Each world holds an **ecology parameter vector**: a named, ranged list of continuous
  settings, e.g. regrowth rate, bite yield, breeding reserve threshold, litterfall rate.
- For now every world gets the defaults. Route at most a handful to prove the plumbing.
- Later each world gets its own perturbation, and a world-level ES scores it against the
  targets.
- Take the names from `design/backlog.md`'s user-configurable parameter list where one
  exists.
- New lineages (lanternjaw, bellwing, seedporter) must join by adding a manifest and a
  centre. Nothing in the loop may be hard-wired to two lineages.

## Tests (their own first commit, ≤ 200 ticks, no pinned hashes)

- Two bodies of one lineage with different variants act differently on the same
  observation; the same variant acts identically.
- A newborn gets a variant; a body's score counts only its own window.
- Antithetic twins: pooled over a toy world with a known best direction, the update points
  the right way (sign test).
- The flowing-water switches: the configured training world has no re-entry stream, and its
  water inventory is conserved over 200 ticks.
- If a fast water mode is built: conservation over 200 ticks, and it refuses to be the live
  default.

## Measure and try

- **Throughput** against frozen ES on the same CPUs: scored body-windows per second,
  simulated ticks per second, and ms per tick.
- **A trial run** of at most 2 h wall on CCD1: both lineages, with the readout per
  generation.
- **Then a gate-style check** on held-out live worlds (small, still water, seeds 1–8,
  60 min, plus two 6 h runs): the trial's centres against round 2's.
  - Walked m per hour, moving share, alive at 60 min, births, drownings, and population
    at 6 h.
  - The question: does parking go away when depletion and crowding are in training?

## Constraints

- **CPUs:** CCD1 only (`taskset -c 8-15,24-31`, cargo `-j 16`). CCD0 is Wrysk's and the
  live app's. **eidolon is busy with another run: don't use it.**
- **Parity is statistical, never bitwise.** FMA and znver5 builds are welcome for training.
- No changes to the fauna or flora rules, rates or thresholds in the live defaults. Use
  FxHash and dense arrays on sim paths.
- **Git:** your own worktree with `CARGO_TARGET_DIR` inside it. Explicit-path commits
  ending with the `Co-Authored-By` and `Claude-Session` lines you were given. Never bare
  `git stash`. Do not merge to main; if main moves, merge it into your branch before
  reporting.
- **Do not edit** `design/handoffs/README.md`, `design/README.md`, `config/tachyon/*`,
  `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or the untracked
  `design/organism-anatomy-2026-09-21.md`.
- No windows (headless only).
- Before reporting, run the full workspace suite (release, CCD1).

## Return (≤ 40 lines)

- commits;
- the tick profile, and the fast-water verdict with its fidelity numbers;
- the ES design as built: W, pairs, worlds, twin placement, normalisation, population
  bounds, and why;
- throughput against frozen ES;
- the trial's readout (first and last generation) and the gate-style check;
- what you would change before the ecology half goes in.
