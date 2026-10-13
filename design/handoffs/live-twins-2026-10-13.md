---
status: open
date: 2026-10-13
owner: Opus (high), worktree `.claude/worktrees/live-twins`, branch `live-twins`
---

# LIVE-TWINS: train a lineage on matched all-sign twin windows (a switch, off by default)

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only for what you run (`quick = true`,
  `hosts = ["desktop"]`, `cwd` set, under 15 minutes). Hand back anything longer as a spec. Never
  wait on a job.
- Build in your own worktree's `target/` (reflink-seeded) at -j 8 or lower. Job binaries go to
  the main checkout's `runs/bin/<name>-<commit>`.
- This is concurrency-touching trainer code: copies run in parallel. Keep determinism: a null
  table (σ 0) must give bit-identical twins and a zero update.

## Why

Brain training on live 10-minute windows (`comparison = per_body`) can't see a top-layer
variant's effect. Findings:
- **LJ-SIGNAL** (merged 0750c24c): the variant's share of a within-world pair difference
  (ICC) is about 0 at every σ from 0 to 0.1.
- **LJ-TWIN** (merged 97f87b41; `examples/lj_twin.rs`, `kin::matched::run_assigned`,
  `scripts/brain-pilot/twin_signal.py`; runs `runs/lab/1013-010026-ljtwin-*`): a variant moves
  ambush/patrol share consistently across starts (ICC up to 0.69). The energy difference
  between twins is mostly which twin caught prey.
  - **All-sign twins** (every lanternjaw of the copy on `+ε_p`, against the same start on
    `−ε_p`): score ICC 0.104 [0.011, 0.252] at σ 0.05 over 10 minutes; 0.118 over 2 minutes.
  - **Focal twins** (one body on the variant) sit at about 0, because the focal's flip
    reshuffles its neighbours' luck.
  - **Price:** all-sign twins over 2-minute windows reach a cosine of 0.2 with the true
    gradient at ~0.8 thread-hours per update (N ≈ 118/ICC measurements). Longer windows don't
    help. The staged scenes can't move ambush/patrol at all.

The design (`design/behaviour-ai-2026-10-11.md` §10.3, §11.7, topic 10) keeps the comparison a
switch, "kept open". Wrysk decides whether training uses all-sign twins. Your job is to make it
ready behind a switch that is off by default, so the decision is a flag, not a build.

## Do

1. **A new comparison value, `twins`** (kin config `comparison`, `kin/config.rs` `Comparison`),
   distinct from the focal `matched`. Doc comment: every body of the lineage in one copy on
   `+ε_p`, every one in the other copy on `−ε_p`, from one saved start; the score is the
   difference of the lineage's summed (or mean; say which and why) window scores, relatives
   included as the kin shares say.
   - The live loop (`live/trainer.rs` `train_inner`, ~line 685) refuses `matched` today. Accept
     `twins` there.
2. **The generation loop for a `twins` lineage.** Follow LJ-TWIN's outline (its handback
   names the pieces):
   - capture a start at each world's window boundary;
   - a scene that installs two-layer centres (`lj_twin`'s `LiveStart` is the model;
     `SavedScene`'s centres are GRU-only);
   - run the all-sign copies through `run_assigned`, in parallel;
   - `pair_scores` → `ascend_pairs` for the update;
   - then advance each world on the centre, as today, so the worlds live on.
   - Knobs (serde, with doc comments and registry rows): the twin window's length; pairs per
     generation; starts per pair. Choose the layout (pairs × starts) from LJ-TWIN's measured
     ICCs, and say why.
   - Lineages on `per_body` or `family` in the same run train exactly as today.
3. **Tests** (mechanism, on small fixtures, nothing past a few hundred ticks):
   - a null table gives bit-identical twins and a zero update;
   - the update's sign follows a variant that helps the lineage in a scene where `+` helps by
     construction;
   - a `twins` lineage leaves a `per_body` lineage's update in the same run byte-identical;
   - plain training stays byte-identical (`a_plain_lineage_trains_exactly_as_today`).
4. **A quick-lane smoke:** a lanternjaw `twins` run for 2 generations at σ 0.05, small enough
   for 15 minutes. Report the per-generation wall time and the twins' divergence (null twins
   identical).
5. **Hand back specs, not submitted:**
   - a lanternjaw stage-1 on `twins` (σ 0.05, 2–3 minute twin windows, top-up
     `lanternjaw=8:0.3`, checks every 10 generations), sized to about 3 hours on 16 desktop
     threads;
   - the same for the seedporter (`--top-up seedporter=8:0.3`; full seedporters never eat and
     trip the liveness check).
   - Price both.

## Boundaries

- Default off: no lineage uses `twins` unless a kin file or flag says so. The menus, labels,
  reward terms, heuristics and the `per_body`/`family`/`matched` paths stay unchanged.
- Don't submit a training run; Wrysk decides that.
- EC2 is never yours.

## Return (at most 40 lines)

- What you built, where (file:line), the knobs and their registry rows.
- The layout you chose and why, with the cost per generation it implies.
- Tests and their result; the smoke's job id, wall time per generation, and its null-twin check.
- Spec paths and prices.
- Commits, and every departure from this brief, with why.
