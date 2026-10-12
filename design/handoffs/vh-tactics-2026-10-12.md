---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/vh-tactics`, branch `vh-tactics`
---

# VH-TACTICS: a Veilhound pack that tests, cuts off, encircles, harries and gives up

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.

**One exception to the rules file's "don't change an existing default":** this is Wrysk's
decided behaviour (§0 rows 1, 6, 8), so the new Veilhound behaviour ships **on by default**.
Other lineages stay as today.

## Why

This is package 5 of `design/behaviour-ai-2026-10-11.md`. Read:
- §0 (rows 1, 2, 6 and 8, and his ask at the top);
- §2.1 (senses, calls, removed privileged reads) and §2.2 (the intent layer);
- all of §4;
- §7's Veilhound law tests;
- §11.1, §11.2, §11.4, §11.5 and §11.6.

Ethology is in `design/7_Research/behaviour-ai-ethology-2026-10-11.md`, and the code survey in
`design/7_Research/behaviour-ai-code-survey-2026-10-11.md`.

**Today's baseline** (`design/7_Research/engage-baseline-2026-10-12.md`, 450 staged episodes):
- two to four hounds starting 10–14 m from a loftstrider family kill a grown adult in
  78–96 % of encounters, median 17–41 s;
- they never give up, are never hurt, and take an adult rather than the calf.

His target (topic 8) is a rare feast: about one loftstrider a world-year, mostly strayed
calves, with most standoffs ending in the pack giving up. The adults' kick comes in
LS-DEFEND, in parallel; your side is the hunters' choices.

**Already merged:**
- CONTRACTS (fauna schema 81): intent menus and held-intent records, typed subjects and
  candidate rows, call records with hearing, the kick state machine (off by default), the
  `Controller::decide` seam. Read `crates/cubarium-voxel-fauna/src/intent_tests.rs` first.
- The engagement harness: `cubarium_voxel_sim::engagement`, `config/engagement.toml`,
  `scripts/studies/engagement.md`. It reruns the baseline on matched starts.

**Ships:** test, cut off, encircle, harry and give up.

## Do

Commit after each numbered step.

1. **The Veilhound intent layer** (§4.2). Its menu comes from CONTRACTS' provisional one;
   refine it if §4 needs it. Each intent is a skill the heuristic runs and records as its
   held intent: these are the brains' teachers later.
2. **Remove the privileged reads** (§2.1, §11.2): `pack::known`, true-position stragglers,
   the Social target's true pose, "is the prey aware of any pack member", and the true-state
   use of `window_body`.
   - Each becomes the hound's own tracks plus calls it heard.
   - `window_body` splits (§11.4). Whether a bite can take an animal of that size stays
     physical; the hound's sensed estimate goes only into its risk and target choice.
   - Feast travel goes to a carcass the hound senses, remembers or hears a regroup howl
     from, not to `pack.feast`.
   - Audit the remaining global reads, and either localise them or confine them to
     bookkeeping.
3. **Veilhound calls** (§2.1):
   - The hunt cue, on committing to or rushing a target. Hearers get the caller's bearing
     only, and adopt the quarry only with their own track on it, seeing the caller heading at
     it.
   - The regroup howl, after a hunt or at the den.
   - Range, cooldown and who heeds each call are knobs.
4. **Target choice and giving up** (§4.3).
   - `p_kill`:
     - rises with youth (size), lameness and isolation, by the hound's own senses;
     - falls with adults in guard stance near the target and adults facing the hound.
   - Risk: expected kicks times the cost of a wound. Read it from what the hound sees:
     stance, facing, reach. An adult's kick recovery counts only from a kick the hound saw.
   - Give up when the value goes below zero, or when the hound's own wound, stamina, or time
     without progress passes a knob.
   - The test, an approach then a break-off, is where the choice gets made.
   - Stance channels read 0 until LS-DEFEND gives the loftstrider its guard stance. Make the
     choice work from facing, numbers and distance meanwhile.
5. **Tactical memory and positioning** (§11.5):
   - target commitment with an expiry;
   - a per-hound approach-side preference;
   - occupied-sector avoidance;
   - cut-off points checked as reachable and safe, with lead prediction bounded by terrain;
   - a packmate's role read from its recent movement.
   - Encirclement follows Escobedo's two forces (§7's `da`/`dc` test). It's one skill, not
     the whole repertoire.
6. **Strikes keep their target, and bodies block** (§11.4).
   - The chosen target's id flows into strike creation and holds through windup and
     contact. Taking a juvenile that comes into reach instead is an explicit substitution.
   - A lunge contacts the first body along its path, so an adult standing between hound and
     calf takes or blocks it.
7. **Couplings** (§11.6). Frondgrazer prying (`vigil.rs:1486-1489`) and attack accounting
   read physical events or local attack state, not the pack target.
8. **Rerun the encounter baseline** through labd on the same seeds as the baseline job, with
   one arm set per pack size, and report the table beside today's. Expect a change in
   *choices* (more tests, some give-ups, calves and lame animals preferred), not yet the
   adults' survival: they can't kick until LS-DEFEND.

## Boundaries

- **Not this package:**
  - the loftstrider's defence, kick, bond and guard stance (LS-DEFEND, next);
  - rest and loftstrider calls (CALLS-REST, running in parallel on branch `calls-rest`; it
    raises Veilhound `kin_tracks` to 4 and changes rest at the den);
  - brains (TWO-LAYER half 2 is running on `two-layer-wire`).
- CALLS-REST and you both edit the fauna crate. Keep your edits to the hunting, pack, strike
  and targeting code, and avoid perception and rest internals where you can. Merge main
  before your final commit.

## Checks (written into tests as you go; §7, §11.2, §11.4, §11.5)

- N hounds around a standing target settle outside kick reach without overlapping. Test the
  two forces and bounded approach, not a gap equal to `da`.
- A cut-off point lies between the target and its nearest defender, and is reachable.
- With config weights, an isolated calf outscores a guarded one, and a lame adult outscores
  a sound one.
- A hound gives up when its wound passes the configured threshold.
- Focal invariance: moving bodies a hound can't sense, with its percept and call history
  fixed, doesn't change its decision.
- A hunt cue gives a hearer the caller's bearing only; the hearer adopts the quarry only with
  its own track on it.
- A strike keeps its target through windup. An adult between hound and calf takes or blocks
  the lunge.
- Asymmetric, occluded and confined scenes, not only open-ground circles. Feasible and
  failed interceptions are tested apart.

## Return

The rules file's format, plus:
- the rerun table beside the baseline, with the job id;
- every privileged read removed or confined, with file:line;
- every departure from §4 and §11, with why.
