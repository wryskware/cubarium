---
status: open
date: 2026-10-11
---

# Group defence and pack tactics: the behaviour-AI design thread

You are a **design thread** (Opus, xhigh effort), talking directly with Wrysk. Design first;
implementation comes later, through workers, once he approves the design.

## His words (2026-10-11)
> giraffe or elephant defense. it takes a pack coordinated effort to take down an adult. multiple
> adults together can generally defend themselves. lone juveniles are the most defenseless. they
> should generally run to the adults (but are probably slower than adult veilhounds) and adults
> alert and run to defend the babies. should balance defense with, sometimes veilhounds do win and
> inflict enough wounds to take down an adult, or are able to overcome the juvenile, or are able to
> isolate one individual from the pack or corner it or intercept it from joining the others. tbh,
> its fairly advanced behaviour, so work really thinking about how we go about implementing a more
> advanced behavioural AI for the species'. give this to an opus xhigh thread to design and have it
> ask astra to review.

So the loftstrider's defence is the first case. The real deliverable is **how Cubarium does
coordinated, multi-animal behaviour**: herd and family defence on the prey side, pack tactics on
the predator side, and how both live in the heuristics and later in the trained brains.

## Read first
- `AGENTS.md`, `WORKING_POLICY.md`, `design/0_Canon/README.md` and the relevant entries in
  `design/0_Canon/DECISIONS.md`.
- Memory: `MEMORY.md` loads by itself. The entries that bind this work most are:
  - `balance-through-roles`, `shape-situations-not-behaviour`, `heuristics-seed-brains`;
  - `frondgrazer-armour-defence` (HUNKER + HERD, the closest precedent), `drop-and-calm-prey`
    (FEAR), `veilhound-senses`, `predation-flight-decisions`;
  - `population-targets`, `rhythms-heights-direction`, `loftstrider-delayed-entry`;
  - `every-number-a-knob`, `tests-never-pin-numbers`, `one-question-at-a-time`,
    `his-mechanism-not-the-workers`, `art-direction-v01` (new poses are his call).
- Design notes:
  - `design/predation-and-flight-2026-09-28.md` (wounds, not one-hit kills; Veilhound hunt
    success target 15–25 %);
  - `design/brain-contract-v3-proposal-2026-10-08.md` and `design/training-plan-2026-10-08.md`;
  - `design/ecology-roles-2026-10-09.md`.
- Brain contract, decided 2026-10-09:
  - Six lineages get v3 B: the brain runs everything but physics and is cloned from the heuristic.
  - **Seedporter, loftstrider and Veilhound get the B/C hybrid:** option logits plus their own
    steering, with **social skills in the option menus**. One net with two heads for now.
  - Brains get bearings to sensed places and the den, and a fading memory of home-range places.
- Code: start with `graft ask "<question>" --source`. Entry points:
  - `crates/cubarium-voxel-fauna/src/ground_response_tests.rs`
    (`an_adult_stands_its_ground_below_its_group`) and the registry row `vigilance.group_min`
    (`registry.rs` about L340): an adult flees only from at least this many threats; a juvenile
    flees from one.
  - The HUNKER and HERD knobs (`hunker.*`) and the Veilhound pack hunt and den code
    (`graft ask "Veilhound pack hunt target choice"`).
  - The counter-wound a grown lanternjaw deals a striking Veilhound
    (`hunt_escape_tests.rs` about L312). It's a precedent for kicks.

## What we know about loftstrider deaths (probe, 8 seeds × 6 h, wide world, 2026-10-11)
- Every loftstrider death was a Veilhound kill; none starved.
- **10 of 11 victims were killed on open low ground**, 10–31 m from the den, while **Resting or
  Asleep**. They had been still for 400–4,500 s. None was cornered. At 20-minute checkpoints, we
  can't tell whether they fled at the end.
- Founders on small high regions (658–2,350 faces) died once in 14 animal-runs. Founders on the
  main area died in 7 of 9 cases (in at least one arm).
- **Close pairs lost both animals in one pack visit:** 6.7 m apart, killed at minutes 173 and 179.
- **Fixed since (main af0afefc):**
  - LS-TOGETHER founds a family in one walkable region of at least 500 faces, so the pair can meet
    and breed.
  - Seedporters now bear twins on a calendar pregnancy (SP-TWINS).
- Loftstriders are expected to need established vaulttree groves (`loftstrider-delayed-entry`).
  Extinction in a young world isn't a failure in itself. **A world where a family can never
  defend itself is.**

## Questions the design has to answer (a starting list, not decisions)
1. **Architecture.** Where coordination lives:
   - each animal reacting to what it senses (positions, calls);
   - a shared group state (herd or pack blackboard);
   - utility scoring or behaviour trees over a per-species option menu.

   Then: how it maps onto the B/C hybrid's option menus, and how a brain cloned from the heuristic
   could learn it (what the observations and options must carry). Keep it cheap per tick: wide
   runs already take about 1.8× real time on Graviton.
2. **Signals.** Alarm calls, juvenile distress calls and pack cues as sense channels (hearing
   range, who reacts and how). Real animals coordinate this way; a shared blackboard might be a
   shortcut brains can't learn from.
3. **Prey defence (loftstrider, elephant/giraffe):**
   - adults converge on a threatened calf;
   - a formation that keeps juveniles inside;
   - kicks that wound Veilhounds;
   - deterrence by the number of adults facing the pack;
   - sleep and rest exposure (giraffes sleep in short bouts; sentinels; staggered sleep).

   How it differs from the frondgrazer's hunker, and the seedporter's retreat to the trees.
4. **Predator tactics (Veilhound, wolf/lion):**
   - target choice: juvenile, straggler, wounded, lone animal;
   - isolating one from the group;
   - cornering it against terrain;
   - intercepting a juvenile running to the adults;
   - wearing an adult down with wounds over a long engagement;
   - when to give up.
5. **Balance.** Veilhounds should sometimes win, as above. Targets: hunt success 15–25 % (D5); on
   the wide world, one Veilhound pack and one loftstrider family. Say how the design's knobs reach
   that, with a budget estimate (`budget-before-tuning`), before any run.
6. **Which species get what.** At least loftstrider and Veilhound; say whether frondgrazer HERD
   and the seedporter should move to the same machinery or stay as they are.
7. **Verification.** Law tests; short scenario probes (a staged attack on a family on a flat map,
   `train-flat`); a short 8-seed check through labd; and an MP4 of an engagement for Wrysk
   (headless; `animations-as-gifs` for the format).
8. **Staging.** Packages with explicit interfaces, ready to brief, in an order where each one
   ships a visible, testable piece.

## Rules that bind the design
- **Fix with roles and behaviour before numbers. Shape the world; never score or reward a
  behaviour.** Fitness and brain rewards never get a "defended" or "coordinated" term. Defence
  pays because it keeps calves alive.
- Every ecological number is a serde knob with a registry row and a real-world note. Realistic
  numbers.
- Heuristics are teachers: coarse is fine, silly bugs are not.
- New poses or animations (kick, ring stance) are art; Wrysk decides the look. Ask him.
- Write "Veilhound", never "chorister", except where code needs the identifier.
- Don't turn this into canon. The doc carries `design_status: proposal`, and you record his
  choices with his quotes. Never append to `DECISIONS.md` without his explicit go for that entry.

## How to work
- **Ask Wrysk one question at a time:** full context, the options, your recommendation. He expects
  questions. He wants the design thought through, not rushed.
- Write the design to `design/behaviour-ai-2026-10-11.md` as it firms up.
- Keep a ledger at `.claude/ledgers/behaviour-ai.md` (current state, at most 60 lines).
- Probes and research go to workers when they'd flood your context.
  - Route by `~/.claude/CLAUDE.md`: opus-medium or opus-high for code reading and probes,
    sonnet for literature on giraffe, elephant, wolf and lion behaviour.
  - Workers never wait more than 5 minutes; runs go through labd (`lab submit`), and you keep one
    waiter.
- **Astra review** (Wrysk asked for it). Once the draft and his main answers are in:
  - Send it to Astra, GPT `gpt-6-astra` via codex: `mcp__codex-lane__dispatch` with
    `model: "gpt-6-astra"`, effort high, and a self-contained review brief.
  - Astra writes `design/7_Research/astra-behaviour-ai-review-2026-10-11.md`. Astra reviews and
    thinks; it never implements.
  - Fold in what holds up. Take any disagreement that changes a decision back to Wrysk.
- Implementation starts only on his go. Route the packages then: new architecture to Opus high,
  specified ports to Codex.

## Other threads
- The ecology-training thread (ledger `.claude/ledgers/ecology-training.md`) owns the tuning runs
  and the training line. It has a combined 8-seed check running on main af0afefc. Don't edit
  presets or fauna defaults from here until implementation starts; the design doc is yours.
- Hard rules: no `unsafe`; no windows on his screen (headless only); cargo at `-j 8` or lower;
  never put worktrees or targets under `/tmp`; EC2 only through `lab approve` on his go; never
  touch AWS credentials.
