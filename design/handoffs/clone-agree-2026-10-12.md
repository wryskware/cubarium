---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/clone-agree`, branch `clone-agree`
---

# CLONE-AGREE: why the loftstrider and Veilhound clones disagree with their teachers

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first.
- Never wrap `scripts/lab/lab` in `timeout`.
- The desktop is busy (a lanternjaw training run, an 8-hour A/B, young-roam runs). Run your
  clones on the quick lane: `quick = true`, `hosts = ["desktop"]`, under 15 minutes or the job
  fails at the cap. A `command` job runs in its output folder unless the spec sets `cwd`.
- Build in your own worktree's `target/` (reflink-seed it from the main checkout's) at -j 8 or
  lower. Job binaries go to the main checkout's `runs/bin/<name>-<commit>`, so they outlive the
  worktree.

## Why

The first full-size clones (main d59e44ed binaries, `runs/bin/two_layer_clone-57bf03b5`) copy the
seedporter and lanternjaw well. The loftstrider and Veilhound clones disagree with their own
teachers on most intents. The `agree2` column is the share of a teacher's labels the two-layer
clone picks the same intent for:

| lineage | intent (labels, agree2) |
|---|---|
| loftstrider | graze (10685, 0.56), watch (1432, 0.88), guard (1326, 0.85), move_off (434, 0.18), rest (451, 0.08) |
| Veilhound | rest (1576, 0.13), den (41, 0.41), search (2872, 0.46), stalk (164, 0.35), test (178, 0.22), drive (1266, 0.87), intercept (19, 0.37), give_up (2818, 0.53), feed (567, 0.99) |

For comparison, seedporter forage 0.97, roost 0.97, follow_mother 1.00, flee_to_trees 0.94;
lanternjaw ambush 0.90, patrol 0.88, feed 1.00. Outputs: `runs/lab/1012-154002-brest-clone-
loftstrider--1768/run/` and `runs/lab/1012-154002-brest-clone-veilhound-hu-a7f4/run/` (the
table is in `stdout.log` or `clone.log`). The specs are
`/tmp/claude-1000/-home-wrysk-wryskware-cubarium/7136f1a8-b5d9-47ee-9452-4b410a19291c/scratchpad/brest/clone-loftstrider.toml`
and `clone-veilhound.toml`.

A brain trained from a clone that can't tell graze from rest, or search from give_up, starts
training far from its teacher. Low agreement usually has one of two causes.
1. **The label misnames what the body does.** The precedent is from today: the seedporter's
   `rest` label covered bodies walking through a rest bout, so a clone could never match it.
   It is now "resting and still" (`intent.rs` `teacher_label`, the seedporter arm). The
   loftstrider's `rest` arm (`S::Heuristic if resting`) has the same shape.
2. **The teacher decides from something the brain can't sense.** Examples: a timer or a memory
   inside the defence layer (`fauna.ls_teach`, LS-DEFEND); pack-level state (`fauna.pack_hint`,
   `pack.target`, `fauna.hound_teach`, VH-TACTICS). The worker rules say bookkeeping records may
   feed rewards and census, never behaviour. Wrysk's topic 1: signals, no group mind.

## Do

1. **Measure.** Get a confusion matrix per lineage (label → the clone's pick) on the held-out
   seed. If `two_layer_clone` (`crates/cubarium-search/examples/two_layer_clone.rs`) can't
   print one, add it behind a switch. Find the confused pairs.
2. **Diagnose each confused pair.** Say what decides the teacher's label, and whether the brain's
   senses (the manifest, `manifest.rs`) carry it. Cite file:line.
3. **Fix within your authority:**
   - **A label that misnames what the body does:** relabel it to what the body does, as the
     seedporter fix did. Add a mechanism test like
     `force_tests::a_seedporter_walking_through_a_rest_bout_is_labelled_forage`.
   - **A teacher reading state the animal plausibly has:** its own timers, hunger or fatigue,
     its memory of recent events, a packmate's call or visible posture. Expose it as a sense,
     following the manifest's existing pattern. The policy digest changes and old clones are
     refused; that's fine.
   - **A teacher reading group-mind state that would need a new signal** (a target chosen for
     the whole pack, with no call or sight that carries it): don't build the signal. Report it
     with a proposal: what call or sight would carry it, and what it changes in the pack's play.
4. **Re-measure** with a small quick-lane clone per lineage, before and after your fix, on the
   same seeds. Hand back full-size clone specs (8 workers, like the `brest` ones above) on your
   binaries. Don't submit them.

## Boundaries

- Heuristic behaviour and menus stay as they are. Labels change only where they misname what the
  body does.
- The loftstrider family intents (stay_with_family, go_to_calf, go_to_adults) and explore never
  appear in the teacher worlds, which have no calves. That is a separate package; leave it.
- Plain training stays byte-identical (`a_plain_lineage_trains_exactly_as_today`).
- A save change bumps the fauna schema to the next free number; say which you took.
- BALANCE's knobs and the ecology thread's defaults aren't yours.

## Return (at most 40 lines)

- The confusion matrices, before and after, with the seeds used.
- Each confused pair: its cause (with file:line), what you did, or your proposal.
- Commits; tests added; the schema number, if any.
- The full-size spec paths and binary names.
- Every departure from this brief, with why.
