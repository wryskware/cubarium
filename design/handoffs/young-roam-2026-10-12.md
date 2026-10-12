---
status: open
date: 2026-10-12
owner: Opus (medium), worktree `.claude/worktrees/young-roam`, branch `young-roam`
---

# YOUNG-ROAM: young loftstriders and seedporters stray more as they grow

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.
- Never wrap a `scripts/lab/lab` command in `timeout`. Killing it mid-git leaves a stale
  `.git/index.lock` in the main checkout.
- Put jobs under 15 min on the quick lane (`quick = true`). Longer ones queue: submit and hand
  back.

**One exception to the rules file's "don't change an existing default":** this is Wrysk's
addition on topic 7, so it ships on by default for the loftstrider and the seedporter.

## Why

This is package 6 of `design/behaviour-ai-2026-10-11.md`. Read §0 row 7 and §3.5. Wrysk: "i
think young loftstriders and non-infant seedporters should have some level of curiosity to
stray from their parent and explore though, increasing the older they get". Strayed calves
are where the pack's calf kills should come from (topic 8).

**Built already:**
- CALLS-REST: the calf's distress bleat, with the mother answering first.
- LS-DEFEND: the sensed mother-follow primitive and the bond's movement.
- SP-NURSE and SP-FOLLOW (the ecology thread): a dependent seedporter keeps to its mother,
  with a 3 m leash when sated.
- Read their code before adding anything.

**A found bug that belongs here** (VH-FIX, 2026-10-12): a young animal's cruise speed is
body lengths per second times the **adult** length (`manifest.rs:1225`, `body.rs:3129`), so
a loftstrider calf at 16–22 % of adult size flees at about 2.2 m/s, as fast as an adult. Wrysk
on calves: "probably slower than adult veilhounds". His pace rule is to calibrate speed in
body lengths per second (`WORKING_POLICY.md`, memory "pace in body lengths").

## Do

Commit after each numbered step.

1. **Young speed from the young's own size.**
   - Cruise and top speed scale with the young's own length, with a floor knob if a newborn
     would be absurdly slow. Say what you chose and why.
   - This touches every lineage's young. Check that seedporter infants still keep to their
     mothers: SP-FOLLOW's contact and the distance metric, on the ecology thread's check
     (see `.claude/ledgers/ecology-training.md`). If it breaks, scope the change to the
     loftstrider, say so, and leave the rest for the ecology thread.
2. **The leash loosens with age** (§3.5). The follow's comfort distance grows from
   `young.follow_m` at birth to `young.roam_m` at independence. A seedporter infant before
   `weaning_age_s` doesn't stray; after weaning its roam grows.
3. **Excursions.**
   - A young animal sometimes wanders toward something it senses: food, another animal, or
     an unvisited place. The chance per hour rises with age (`young.excursion_per_h` at birth
     and at independence).
   - An excursion ends when the young is alarmed, hears its mother's call, or loses sight of
     its family. Then it bleats, and its mother comes.
   - The seedporter gets a distress call on the substrate too, if it lacks one.
4. **The intent record.** Excursions record an `explore` intent, which the brains copy
   later. The reward stays energy only.
5. **Check it.**
   - Staged encounters: a family with an older calf, through the engagement harness. Show
     that calves stray, that a pack can find a strayed one, and that bleating brings the
     mother.
   - A short wide-world run (quick lane, or queued): how far calves and weaned seedporters
     wander by age, how often they bleat, and how long the mother takes to arrive.
   - Report rates. Don't tune to targets: BALANCE brackets topic 8.

## Boundaries

- Veilhound tactics (branches `vh-tactics`, `vh-fix`, probably merged by the time you
  start), brains, and the guard and kick (LS-DEFEND) aren't yours.
- The ecology thread owns seedporter nursing and following. Change them only through the
  shared primitive, and say exactly what changed.

## Checks (written into tests as you go)

- A young animal's speed scales with its own length (and the floor). An adult is unchanged.
- The leash distance grows with age between the two knobs. A seedporter infant before
  weaning never strays.
- An excursion ends on alarm, on the mother's call, and on losing sight, and then the young
  bleats.
- A mother hearing her own young's bleat goes to it.

## Return

The rules file's format, plus the speed change and its effect on seedporter following, the
straying rates by age, and every departure from §3.5, with why.
