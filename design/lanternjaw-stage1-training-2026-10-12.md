---
design_status: proposal
date: 2026-10-12
package: BRAIN-FIX step 4 (design/handoffs/brain-fix-2026-10-12.md)
---

# The lanternjaw's first training run: stage 1, top only

This is a run spec, not a decision. It follows `design/behaviour-ai-2026-10-11.md` §10.4 and
§11.7. The lanternjaw goes first because its menu (ambush, patrol, feed, give_way, flee,
rest) won't change in the coming merges. The seedporter's and loftstrider's menus will
change (YOUNG-ROAM adds `explore`), so they wait.

## What trains

- **Starting centre (BRAIN-FIX-2):** the two-layer lanternjaw re-cloned on hungry teacher
  streams (job `1012-132335-bf2-lj-clone-then-smoke-5960`, `run/clone/lanternjaw-two-layer.json`;
  `two_layer_clone fullness=0.3`, the same fullness as the top-up below). BRAIN-FIX's clone
  (job `…-e4ac`) recorded no patrol or ambush: its teacher worlds placed lanternjaws at a
  placed body's default stores, which is sated (traced: the default body matches a
  fullness-1.0 body sample for sample), so they digested through the whole recording.
  BRAIN-FIX-2 also labels a sated lanternjaw standing at a carcass `rest`, not `feed` (the
  latch is off and it takes nothing).
- **Stage 1:** the kin stage defaults to `top`, so the variant builder perturbs only the top
  (intent head and pointer). The bottom stays frozen at its copy.
- **Comparison:** `per_body` in the live windows, as today. The lanternjaw is solitary, so
  every kin share is 0 (the default `KinConfig`). No `--kin` file is needed.
- **Worlds:** the training worlds as today (`small`, still water, shipped ecology, no egg
  bank). Each world is topped up to 3 grown lanternjaws on their heuristic
  (`--top-up lanternjaw=3:0.3`, 0.3 of their stores, so the first windows have hunger to
  score). The trainer then puts them on variants like any other body. Three is the egg bank's
  founding count.
- **Staged scenes as extra episodes: not in this run.** A scene variant's score has to be
  compared through the matched-copies runner (`kin::matched`). The live loop refuses
  `matched` by design (§11.7: matched live windows are a priced trial). Joining the two means
  a second scoring path with its own normalisation, which isn't cheap. So the scenes are
  readouts here, not training episodes.

## Readouts every check (`--check-every 10`, `checks.jsonl`)

Each check runs `two_layer::check` on the current centre. It also runs once on the starting
centre (generation 0).

- **Energy against the heuristic on matched seeds.** Six live seeds (21, 31, 32, 41, 42, 51;
  a seed the founding refuses is redrawn deterministically from it, noted in the run log, so
  both arms still found the same world; BRAIN-FIX-2). Each seed is founded twice, with the
  same top-up: once with the lanternjaws on their heuristic, once on the centre. Each arm runs
  600 s. The check records the lanternjaws' energy per starting body (bodies and stores),
  deaths and bodies. It also records the share of body-seconds with the eating latch on, which
  shows whether the window gave them anything to win.
- **Per-mode competence** on the staged scenes (feed, give_way), seeds 101–104, both arms.
- **Hold durations and switch rates** for each arm, from its own intent records: the share of
  decision samples per intent, the holds that ended and their mean length, and switches per
  body-minute.
- The trainer's own `generations.jsonl` line per generation: mean raw score, gradient norm,
  update RMS, bodies and the replacements.

## The runs

- **Smoke** (BRAIN-FIX's job `1012-124416-brain-fix-lj-stage1-smok-200d` panicked on a
  refused training-world terrain; BRAIN-FIX-2 redraws it and reruns the smoke chained after
  the new clone, job `1012-132335-bf2-lj-clone-then-smoke-5960`, `run/smoke/`): 2 worlds, 2 pairs,
  1-minute windows, 2 generations, with a check (2 seeds × 60 s, scene seed 101) on the start
  and after each generation. It should show that the run trains (one update per generation),
  writes back (`centres/final-lanternjaw.json`) and runs the checks.
- **Real run:** 16 worlds, 16 pairs, 10-minute windows, up to 60 generations (10 simulated
  hours per world), a 5-hour wall cap, and a check every 10 generations. Its spec is in the
  BRAIN-FIX handback.

## Prediction, before it runs

- **Generation 0** (the clone against its rules): the paired energy gap (brain − heuristic)
  is near 0, inside the seed-to-seed spread. Competence should be close to the rules' on feed
  (both near 1). On give_way it will be at or below the rules' (the first clone scored 0.06
  against 0.63).
- **Revision (BRAIN-FIX-2):** the starting clone now has patrol and ambush in its teacher
  data, so generation 0's paired gap should still sit near 0, but the windows' hunger
  (top-up 0.3, which the trace shows turns the eating latch on and the patrol to ambush and
  patrol) should give most early windows signal, not few. The rest of the prediction stands.
- **Early generations** carry little signal. A sated lanternjaw stands still and digests, so
  every arm's window is identical. The first clone's live arms matched to 1e-9 in 300 s,
  because their bodies were full. The top-up's hunger and the persistent worlds' own hunger
  cycle decide how many windows carry signal; the check's `hungry_share` shows it.
- **What "beats the heuristic" should look like after about 30 generations** (5 simulated
  hours per world):
  - the paired gain per body (brain − heuristic) is positive on at least 4 of the 6 seeds;
  - its mean is above the spread between the generation-0 seeds;
  - deaths are no higher than the rules';
  - feed competence holds and give_way competence is not lower than at generation 0;
  - switches per body-minute stay within a factor of 2 of the rules', with no intent taking
    over the holds.

  Stage 1 can only re-time and re-choose among moves the clone already makes: when to wait in
  ambush or move on, when to yield, when to stop and digest. So the gain should be small, a
  few percent of the rules' energy change, not a new behaviour.
- **How I'd know it's working:**
  - the lineage's mean raw score in `generations.jsonl` trends up across the generations where
    `hungry_share` is high, and the gradient norm stays above zero;
  - the paired gap rises from check to check, without the holds degenerating.
- **How I'd know it isn't:** after about 30 generations the paired gap is still inside the
  generation-0 spread. Next steps would then be:
  - run the frozen-brain noise check of §10.3, which is still owed;
  - lengthen the windows or raise the hunger;
  - move to stage 2.
