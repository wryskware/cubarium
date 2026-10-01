---
status: open
date: 2026-10-01
owner: main thread; Wrysk said go 2026-10-01
plan: design/cotraining-plan-2026-10-01.md
---

# Co-training stage 0: search genome, targets v2, net-energy reward

This package makes the ecology search and unified mode ready to co-train brains and
ecology on the terrarium. Read these before starting:
- the plan;
- `design/ecology-parameter-registry-2026-09-28.md` §3–6 and its decisions;
- `design/7_Research/ecology-parameter-audit-2026-09-28.md` §1.4, §6 and §8.

Don't run any search. Stage 1 launches after this merges.

**Worktree:** `.claude/worktrees/cotrain`, branch `cotrain` off main, with
`CARGO_TARGET_DIR` inside it. Commit in this order:
1. this brief, if it is not already on main;
2. the tests listed below;
3. the code.

## 1. Brain reward: net energy, including the loss at death (Wrysk 2026-10-01)

- **The score per body per window:** the change in the body's energy,
  (E_end − E_start + E_given) / body_ref.
  - E is the body's tissue plus reserve, in the units intake is counted in.
  - E_given is the energy passed to offspring during the window: gestation, eggs.
    Breeding is neither a cost nor a loss.
  - **A body that dies in the window has E_end = 0,** whatever the cause. This is the
    death loss: starvation loses the body, not only an empty reserve.
- **Where it applies:** it replaces (intake − motor)/ref + 0.25·survived in
  `voxel-live-train` and in unified mode. Leave the frozen trainer alone.
  - Keep the within-world normalisation.
  - No other terms, and no flag for the old score.
- **Tests:**
  - For every trained lineage's default body, at the window lengths the CLIs default to,
    a body that dies in a window scores below an idle survivor of the same window. If
    this fails for some lineage, report it rather than adding a term.
  - With the same intake and motor, breeding does not lower the parent's score.
  - More intake raises the score, and more motor lowers it.

## 2. Search genome (R3)

Build the registry proposal's §3–5 with these choices:

- **Search files and the generic decode.**
  - Search files live in `config/training/search/`.
  - `legacy.toml` reproduces today's routed vector. Test: decoding it at ×1 gives
    today's configs.
  - `cotrain.toml` is stage 1's search.
  - The decode replaces `PARAMS`, `MASS_SCALED`, `LINEAGE_PARAMS`, `fauna_config` and
    `flora_config` in `es/voxel/live/ecology.rs`.
  - `--training-ecology` and `voxel-ecology-check --arm` take a genome or a resolved
    parameter file.
- **Genes:**
  - Registry §4's main knobs: per lineage for littershredder, frondgrazer and
    lanternjaw, and per plant group. Aim for about 20–40.
  - `derive()` per crate. For example, motor = `motor_factor` × maintenance, so an
    upkeep gene no longer silently reprices walking (audit §8.1).
  - Each gene is a multiplier in log space, and ×1 = today.
  - **Ranges** reach each knob's real-world baseline from audit §6 /
    `parameters.tsv`, so the search can move there itself.
  - List the genes and their ranges in the commit message.
- **Not genes:** `digest` rows. These are mass scale, body size, the eye and mouth
  anchors, cruise, cone and taste. The brains' sense contract hashes them. Mass scale
  stays at the shipped ×30.
- **`K_bio`:** a neutral clock gene, per the registry decision 2.
- **The R/S/F screen:**
  - Port `design/7_Research/ecology-parameter-audit-2026-09-28/scripts/invariants.py`
    to Rust. It runs over the resolved parameters and the founding plan.
  - A candidate outside R 0.02–0.3, S 0.02–0.2, or with F < 10 (audit §1.4: F ≫ 1), takes a failure
    penalty without a run. Record its R, S and F.
  - Measure where the defaults sit on the terrarium at ×30. If they fall outside the
    bands, report it. Don't widen the bands silently.
- **Tests:**
  - Decoding at ×1 gives today's configs, in every crate.
  - A gene moves exactly the paths it matches.
  - A derived field follows its base gene, and stays put when the search frees it.
  - An unknown path in a search file is an error.
  - A digest row named as a gene is refused.
  - The screen rejects a hand-built world where parking wins by arithmetic.

## 3. Targets v2 (Wrysk 2026-10-01: persist, breed, bounded)

- **The world.** `voxel-ecology-search` gets `--world <file>`, as the check already has.
  Stage 1 uses `config/desktop/terrarium.toml`.
- **The penalty.** It is zero inside the band and grows with the normalised distance
  outside it. It has these parts:
  - **Persistence:** every lineage, the lanternjaw included, and every plant species
    that the default world keeps on that seed.
  - **Breeding:** every lineage has births in the second half.
  - **Bounded:** animals per world stay in [lo, hi] over the second half, and never go
    above peak.
    - The defaults are 1×, 3× and 5× the world's founding animal count. The founding
      count is used because today's starving defaults give no capacity to measure
      against.
    - The three are flags.
  - **Moving:** the browser moving share is at least 0.30.
  - **Lanternjaw share:** 5–15 % of the animals over the second half.
    - The lanternjaw founds as a dormant egg bank. If it can't reach the band within
      1 h on any seed, report that.
- **Untrained lineages.** A lineage given no `--centre` runs its heuristic and takes no ES
  step. Today that is the lanternjaw. Unified mode's brain pairs apply only to lineages
  that have a centre.
- **Tests:**
  - The penalty is zero inside the band and monotonic outside it, for each new part.
  - Unified mode with no lanternjaw centre runs one window.

## 4. Measure; don't search

- **Throughput:** terrarium ticks per second per world, single-threaded, on idle cores.
- **The defaults' R, S and F.**
- **The stage-1 baseline:**
  - `voxel-ecology-check --world config/desktop/terrarium.toml`, arm `default`.
  - Seeds 1–8 × 1 h, with round 2's centres by transfer, as in
    `.claude/worktrees/live-train/scripts/live-train/terrarium-gate.sh`.
  - Report, per lineage: alive, births, deaths by cause, and moving share.
- **Smoke:**
  - Two generations of `voxel-ecology-search` on the terrarium: λ = 4, 2 seeds, 10 min.
  - Then the same in unified mode with `--brain-pairs 2`.
- **Stage-1 sizing:** from the throughput, propose λ, seeds, hours and generations for a
  3 h wall budget.

## Constraints

- **Every number is a knob** (WORKING_POLICY). New harness numbers are flags or
  search-file fields. Model numbers get registry rows.
- **CPU:** Wrysk allows the desktop 90–100 % (2026-10-01).
  - Check the load first.
  - If a live `cubarium voxel` is running, stay on CCD1 (`taskset -c 8-15,24-31`).
  - Use `nice -n 10`.
- **Tests:** each ≤ 200 ticks, no pinned hashes. Run only the crates you touched; the
  integrator runs the full suite.
- **Git:**
  - Explicit-path commits, never `commit -a`, ending with the `Co-Authored-By` and
    `Claude-Session` lines you were given.
  - No bare `git stash`.
  - Don't merge to main. If main moves, merge it into the branch.
- **Other:**
  - No windows.
  - Stop every background loop before you report.
  - Don't edit `config/tachyon/*`, `scripts/tachyon-*` or `art/gen/*`.

## Return (≤ 40 lines)

- commits;
- the gene count, with any knob you could not tie to a gene and why;
- the defaults' R, S and F;
- throughput;
- the baseline table;
- the smoke result;
- the proposed stage-1 sizing;
- anything that makes the targets unreachable by construction.
