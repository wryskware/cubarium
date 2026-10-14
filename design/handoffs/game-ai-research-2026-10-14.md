---
status: open
date: 2026-10-14
owner: Opus (high), research only (web + repo reading), no code
---

# GAME-AI-RESEARCH: how games and ML make creature minds, and what they optimise

## Why (Wrysk, 2026-10-14)

"i dont think 'how much food it ate' is the right optimization function, not in itself anyway.
like there are decisions about survival and even about happiness (curiosity/exploration). so if
we want richer brains we need richer scores to account for that, particularly if these are
per-decision rewards. and this is typical of game ais? did you look this up or just from recall
… might be worth doing more research on the subject since this crosses ml and game design."
And: "(general research about game ais, beyond your proposal or our architecture)".

Context (read, don't re-derive): Cubarium is an ambient ecosystem on a cube display; animals
run hand-written rules today, and learned two-layer brains (a top that picks a named intent at
4 Hz, a bottom that steers) are being trained to beat them (`design/behaviour-ai-2026-10-11.md`
§2, §10; `design/7_Research/training-review-2026-10-14.md`). Today's score: net energy incl. the
loss at death, plus kin shares; a novelty (curiosity) term exists, off. Standing principles:
behaviour comes from senses; "shape situations, never reward behaviour" (fix the world so a
behaviour pays, don't score the behaviour); every number a knob.

## Do

Research broadly, with sources (URLs; papers, GDC talks, postmortems, developer writing). Mark
what you verified in a source versus what is your recall.
1. **Game creature AI in practice:** how shipped games build believable animals and agents —
   behaviour trees, utility AI, GOAP/planning, needs/motive systems (The Sims), learning
   creatures (Creatures' neural nets and biochemistry drives, Black & White), ecosystems and
   creature sims (Rain World, Dwarf Fortress, Spore, others you find), and what designers say
   about believability vs optimality, readability, and players watching.
2. **ML agents in games and simulations:** RL in shipped or research games (e.g. OpenAI Five,
   AlphaStar, GT Sophy, Unity ML-Agents, Ubisoft La Forge, EA SEED, multi-agent emergence such as
   hide-and-seek), evolutionary/neuroevolution approaches, artificial-life sims (Polyworld, Avida,
   Framsticks, The Bibites and similar). Which training methods they used, how long, at what
   scale, and what went wrong (reward hacking, degenerate strategies, "sit still" optima).
3. **What to optimise:** reward design for living-seeming agents — fitness proxies (survival,
   reproduction, kin), homeostatic/drive-based reward (needs like hunger, fear, fatigue, social),
   intrinsic motivation (curiosity, novelty, empowerment), evolved or "optimal" reward functions
   (rewards shaped by evolution for fitness), multi-objective trade-offs, and how per-decision
   rewards differ from episode scores. How each fits or fights "never reward a behaviour".
4. **Lessons for Cubarium:** an ambient piece people watch, a small machine budget, rules as
   teachers, kin, a per-species menu. What a richer score could be, what to try first, what to
   avoid.

## Deliverable

`design/7_Research/game-ai-research-2026-10-14.md` (evidence, not decisions): a readable survey
with sources, then 3–6 topics he can decide one at a time (options, what he'd see on the cube,
cost, a recommendation each). Commit with explicit paths on a branch `game-ai-research` in
worktree `.claude/worktrees/game-ai-research` (trailer `Co-Authored-By: Claude Opus 5.5
<noreply@anthropic.com>`); don't merge. Never write "chorister" (the animal is the Veilhound).
No tool call over 5 minutes.

## Return (at most 40 lines)

The doc path and commit, the topic list with recommendations, the five sources that matter most,
and what you could not verify.
