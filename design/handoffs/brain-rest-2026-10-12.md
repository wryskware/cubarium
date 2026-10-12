---
status: open
date: 2026-10-12
owner: Opus (medium), worktree `.claude/worktrees/brain-rest`, branch `brain-rest`
---

# BRAIN-REST: a brain that rests can wake, its rest survives a save; first loftstrider and Veilhound clones

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first.
- Never wrap `scripts/lab/lab` in `timeout`.
- The desktop and eidolon are full until about 20:10 (two 8-hour live checks). Only the quick
  lane runs before then: `quick = true`, `hosts = ["desktop"]`, under 15 minutes, or the job
  fails at the cap. A `command` job runs in its output folder unless the spec sets `cwd`.
- Build in your own worktree's `target/` (seed it by reflink from the main checkout's) at -j 8
  or lower. Expect slow builds: the desktop is loaded. Never run anything over 5 minutes in the
  foreground; build detached and hand back if you must.
- Job binaries: copy them to the main checkout's `runs/bin/<name>-<commit>`, so they outlive
  your worktree.

## Why

Two gaps in the brain's rest, found while merging BRAIN-FIX (main 16f89de3).

1. **A brain that asks to rest can't wake.**
   - `step.rs` `controllers` (~line 2388) stores a brain's rest head as `fauna.rest_asks[id]`.
   - The circadian stage (`circadian`, ~line 994) grants sleep while the ask is `Rest` and the
     body is free and not in an emergency.
   - An asleep body's controller is never sampled (the `due` filter, ~line 1637). So the ask
     never changes, and the body sleeps until a wake (footfall, call, sight) or hunger
     emergency.
   - A brain that says "rest" once at noon sleeps until it starves or something walks by.
     Every brain run trains against that trap.
2. **`rest_asks` is `#[serde(skip)]`** (lib.rs ~4321). A world saved with a brain asleep on its
   own ask loads with the ask cleared, so the body wakes at once. The saved and unsaved worlds
   diverge.

## Do

Commit after each numbered step.

1. **A granted brain rest lasts one bout.** My call (Wrysk left action fidelity to me): when
   the circadian stage grants sleep on a brain's ask, the body sleeps for one bout, then the
   ask clears and the brain is sampled again on its next cadence tick. If it asks again, it
   sleeps another bout.
   - The bout length is a knob per lineage in the rest table (`sleep.rs` `RestTable`;
     `rest.brain_bout_s` or a better name), with a doc comment and a registry row.
     Pick each lineage's value from what it already does: the heuristic's own pause or light
     rest bout where one exists, else the top layer's intent hold. Say what you picked and
     why.
   - Wakes and emergencies still end the bout early, as today.
   - The rhythm's own sleep (night, the den) is unchanged. A heuristic body is unchanged.
   - Keep sleeping bodies cheap. Don't sample an asleep brain every cadence tick.
2. **Save the ask and the bout.** Save whatever the bout needs (the ask, its end tick).
   Take the next free fauna schema number when you commit, and say which you took.
   - The existing save-mid-hold test pattern (`fauna_tests::a_fauna_save_mid_hold_continues_
     the_brains_identically`) shows how to prove a save continues identically; write the same
     test for a body asleep on its own ask.
3. **The seedporter's rest competence.** My call: a seedporter sleeps only perched, which is
   `roost`. Its `rest` intent is the daytime pause of the stop-and-go gait.
   - Today the rest scene reads "the share of body-seconds resting or asleep"
     (`two_layer/scenes.rs` `competence`, ~line 144). The rules score 0.16 on it because a
     glider never sleeps on the ground.
   - Change it to the pause: the share of rest-intent body-seconds the body holds still
     (`teachers.forage_stay_forward`), and not asleep. Check the rules score high on it;
     if they don't, find out why before you change anything else.
4. **Loftstrider and Veilhound clones: prove the path.** Both lineages have teacher labels
   (`intent.rs` `teacher_label`: LS-DEFEND's `ls_teach`, VH-TACTICS' `hound_teach`), but
   neither has been cloned.
   - Run `two_layer_clone founder=loftstrider` and `founder=chorister` small enough for the
     quick lane: short `record_s`, few seeds. Copy the spec pattern from
     `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/7136f1a8-b5d9-47ee-9452-4b410a19291c/scratchpad/bf2/`
     and the clone jobs in `runs/lab/1012-122230-*`.
   - Fix whatever stops them. A teacher world where the label you need never appears is the
     same trap BRAIN-FIX-2 hit with sated lanternjaws: report the label counts.
   - Hand back full-size clone specs ready to submit (8 workers, as `bf2/`), using your
     binaries. Don't submit them: the desktop is full.

## Boundaries

- The two 8-hour live checks and the queued lanternjaw training run use older binaries. Leave
  them alone.
- Plain training stays byte-identical (`a_plain_lineage_trains_exactly_as_today`).
- Menus, teacher labels and heuristic behaviour stay as they are, except step 3's read.
- BALANCE's knobs and the ecology thread's defaults aren't yours.

## Checks (tests you write as you go)

- A brain that asks to rest once sleeps one bout, then is sampled again; asking again sleeps
  again. Use the knob, not its value.
- A wake or an emergency ends a brain's bout early.
- A save mid-bout continues identically.
- A heuristic body's sleep is unchanged by any of this.
- No test pins a tuned number.

## Return

The rules file's format, plus:
- the bout values per lineage and where each came from;
- the schema number you took;
- the seedporter rest read for rules, two-layer and plain, if you could run it on the quick
  lane;
- the loftstrider and Veilhound smoke clones' label counts, what you fixed, and the full specs'
  paths;
- every departure, with why.
