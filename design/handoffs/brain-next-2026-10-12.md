---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/brain-next`, branch `brain-next`
---

# BRAIN-NEXT: train two-layer brains in the live loop, and teach every mode

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.
- Never wrap a `scripts/lab/lab` command in `timeout`. Killing it mid-git leaves a stale
  `.git/index.lock` in the main checkout.
- The desktop's slots are reserved by long A/B batches. Put jobs under 15 min on the quick
  lane (`quick = true`). Longer jobs queue: submit them and hand back with the ids.

## Why

This finishes packages 7 and 8 of `design/behaviour-ai-2026-10-11.md` (read §0 rows 2–4 and
10, §10, §11.3 and §11.7). What's merged on main:
- the two-layer core: `crates/cubarium-search/src/es/voxel/two_layer/`;
- its fauna wiring (TWO-LAYER-WIRE, 8d574cd8);
- the kin estimator: `kin/`, the `--kin` flag, `kin/stage.rs`, `VariantTable::antithetic_two_layer`.

Read the commit messages of 8d574cd8 and 0d465846, and the handbacks quoted in
`design/handoffs/two-layer-wire-2026-10-12.md` and `kin-train-2026-10-12.md`.

**The first clones** (labd jobs `1012-081648-two-layer-clone-{seedport,lanternj}`, outputs in
`runs/lab/<job>/run/run/summary.json`):
- **Lanternjaw:** the two-layer clone is far better than plain (patrol action error 0.00014
  vs 0.025).
- **Seedporter:** intent is right while foraging (0.99) and weak on keeping up with the
  troop (0.59) and rest (0.42). Its actions are a little worse than plain.
- **Coverage:** in 30 recorded minutes, most modes never occurred: roost, groom,
  follow_mother, flee_to_trees for the seedporter; feed, give_way, flee, rest for the
  lanternjaw.

**Known gaps:**
1. The live trainer loads only plain centres.
2. Shadow samples label actions only, not intents.
3. `force_own` forces only base modes.
4. No closed-loop refit.
5. The clone tool panics on terrain seeds the training world refuses (21, 22).

## Do

Commit after each numbered step.

1. **Two-layer centres in the live trainer.**
   - `live/trainer.rs`'s `Lineage` takes a `BrainFile` template.
   - Variants are built with `EpisodeDriver::two_layer`, with stage 1 perturbing only the top
     (`kin/stage.rs`).
   - Write-back goes through `TwoLayerPolicyFile`.
   - Plain lineages stay byte-identical. Add a test that a plain run's scores are unchanged.
2. **Intent labels for the shadow.** Run the heuristic intent layer dry beside a
   brain-driven body: compute its proposal before the hold without acting, so closed-loop
   samples carry the teacher's intent and subject. Show that the dry run changes nothing
   about the body.
3. **Mode coverage by staged scenes.** For each seedporter and lanternjaw mode that didn't
   occur, build a small staged scene that triggers it naturally:
   - night for roost;
   - a larva on a troopmate for groom;
   - a young seedporter with its mother for follow_mother;
   - a threat for flee_to_trees and flee;
   - a carcass for feed;
   - a larger animal for give_way;
   - a rest hour for rest.

   Follow the engagement harness's pattern (`cubarium_voxel_sim::engagement`). Record
   teacher streams from them into the clone tool. List any mode you couldn't stage, and why.
4. **Closed-loop refit.**
   - One correction round: the learner picks intents, the dry-run teacher labels the states
     it reaches, and the tool refits.
   - The clone tool skips refused terrain seeds with a note instead of panicking.
5. **Per-mode competence checks** (§11.3: "Competence before freezing").
   - For each mode, a closed-loop check on its staged scene: does the cloned brain, in that
     scene, do what the mode is for (reach the roost, groom the troopmate, keep near the
     mother, reach the trees, eat the carcass)?
   - Two-layer beside plain beside the heuristic, on matched seeds through `kin::matched`
     (null-pair noise is zero).
   - Average action error over mostly-foraging samples doesn't count.
6. **Rerun the seedporter and lanternjaw clones** with steps 2–4, then step 5's checks.
   - Report per-mode intent agreement, action error and competence, against plain and the
     heuristic.
   - This is the evidence for whether the two-layer brain is worth training. Wrysk:
     "otherwise it wont be worth using either".

## Boundaries

- LS-DEFEND (branch `ls-defend`) and the Veilhound packages (branches `vh-tactics`,
  `vh-fix`) edit loftstrider and Veilhound behaviour in the fauna crate. Keep your fauna
  edits to the dry-run hook and staged-scene setup.
- No loftstrider or Veilhound cloning yet.
- No long training runs. This package makes training possible and shows the clones' per-mode
  competence.

## Checks (written into tests as you go)

- A plain lineage's live scores are unchanged.
- A two-layer variant's stage-1 perturbation leaves the bottom alone end to end, through the
  trainer and the write-back.
- The dry-run teacher doesn't change the body's behaviour or the RNG stream.
- Each staged scene triggers its mode in the heuristic. Assert the mode, not timings.
- The clone tool skips a refused seed.

## Return

The rules file's format, plus the per-mode table (agreement, action error, competence for
two-layer, plain and heuristic), job ids, the modes that couldn't be staged, and every
departure from §10–§11.3, with why.
