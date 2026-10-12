---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/calls-rest`, branch `calls-rest`
---

# CALLS-REST: calls on the substrate, and rest that a family survives

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.

**One exception to the rules file's "don't change an existing default":** Wrysk decided this
behaviour (topic 5, §0), so it ships **on by default** for the lineages it names. That's the
loftstrider, plus the Veilhound at its den for "sleeps lighter". Other lineages' behaviour
stays as today, except the frondgrazer's alarm moving onto the substrate unchanged.

## Why

This is package 3 of `design/behaviour-ai-2026-10-11.md`. Read §0 (rows 1, 5, 7), §2.1
(senses and calls), §3.4 (rest), §3.5 (the bond, for the "mine" mark), §5 (species table),
§7's "Calls" and "Sleep" law tests, and §11.2 and §11.4. The probe of 2026-10-11 found that
10 of 11 loftstrider deaths were bodies resting or asleep on open ground near the den.
Asleep bodies don't sense, a rushing hound doesn't wake them, and nobody keeps watch.

CONTRACTS (merged; fauna schema 81) built the substrate: call records, ranged and occluded
hearing awake and asleep (`Fauna::heard`), the call-kind action, the rest request
(`RestAsk`), intent menus and held-intent records, typed subjects. Read its interface first.
Start with `crates/cubarium-voxel-fauna/src/intent_tests.rs`, then `controller.rs`
~180-260 and `subjects.rs`.

**Ships:** a family that wakes on a rush or a call.

## Do

Commit after each numbered step.

1. **Loftstrider calls** (§2.1 table):
   - an alarm snort from an adult or juvenile that sees a pressing threat;
   - a distress bleat from a calf that is chased, struck, or alone with a threat.
   - Hearers react through their heuristics: a snort raises vigilance and gathers the
     family; a bleat draws its mother first, then the other adults. Keep the reaction
     minimal. The full defence (guarding, kicking) is LS-DEFEND's.
   - Range, cooldown and who heeds each call are knobs per lineage and kind.
2. **The frondgrazer's bolt alarm onto the substrate, unchanged.** Its observation input
   stays byte-identical (plain manifest). Check that its alarm behaviour matches today's on
   a fixture: who hears it, when, and the response.
3. **Rest physiology** (§3.4, topic 5; Wrysk: "would you recommend B? it sounds more
   interesting to watch"):
   - mostly light rest, sensing at a reduced range;
   - deep sleep in short bouts of about 1–2 min, and only deep sleep is blind;
   - a minimum light-rest interval between deep bouts;
   - a body past its sleep-need ceiling still falls asleep (today's rhythm stays the
     backstop);
   - **"sleeps lighter":** deep bouts are rare while the animal has no recent sight of a
     relative awake (knob `rest.deep_alone_scale`). It reads visual evidence only (§11.4),
     never an unseen animal's true state.
   - The light-or-deep choice is the body's. The rest request (`RestAsk`) stays the way
     brains and heuristics ask.
4. **Waking.** A rushing hound's footfalls and a heard snort or bleat wake sleepers above a
   loudness threshold, with occlusion, through CONTRACTS' asleep hearing. Replace or route
   the current wake path (`rhythm.rs:371-408`) so there's one rule, and say how.
5. **Rest sites read hunter scent.** A loftstrider choosing where to rest avoids places that
   smell of Veilhounds, through its own smell sense.
6. **Kin 4 with the "mine" mark** (§2.1, §3.5):
   - loftstrider and Veilhound `kin_tracks` go from 1 to 4;
   - a mother and her calf recognise each other, a mark on the kin slot: the
     relation channel CONTRACTS added.
   - Nothing else about the bond: following and guarding are LS-DEFEND's and YOUNG-ROAM's.
7. **Cost.**
   - Measure the per-tick cost on a live mixed world, main against your branch: the
     desktop d96 world, about 10 sim-minutes, the same seed, through a labd job.
   - CONTRACTS now builds subject candidates for every menu-lineage body (seedporters
     included) at each decision sample, and nobody has measured that yet. Report its share
     separately if you can.
   - If either costs more than a few percent, make it cheaper (for example, build
     candidates only for brain-driven bodies or at the intent rate) and say what you did.

## Boundaries

- **Not this package:**
  - guarding, kicking, retiring `group_min` flight, the mother-follow extraction and the
    bond's movement (LS-DEFEND);
  - Veilhound calls and tactics (VH-TACTICS);
  - straying (YOUNG-ROAM);
  - brains (TWO-LAYER half 2 runs in parallel in `crates/cubarium-search/`; touch it only
    for compile fixes).
- TWO-LAYER half 2 may add a small `brain` table under the fauna founders' config. Keep
  clear of it.

## Checks (written into tests as you go; §7)

- A call is heard within range, muffled behind rock, and unheard beyond it. CONTRACTS has
  this already; extend it to each new kind.
- Sleep:
  - a deep sleeper misses a stalking hound and wakes to a rushing one within hearing;
  - a light rester senses at the reduced range;
  - deep bouts are rarer with no relative seen awake than with one seen.
- A calf's bleat draws its own mother before another adult, all else equal.
- A resting loftstrider picks a site without hound scent over an otherwise equal one that
  has it.
- The frondgrazer's alarm matches today's on a fixture.

## Return

The rules file's format, plus the cost numbers with how you measured them, and every
departure from §2.1 and §3.4, with why.
