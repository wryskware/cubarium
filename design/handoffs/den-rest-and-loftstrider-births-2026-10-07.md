---
status: open
date: 2026-10-07
owner: codex gpt-6.1-sol xhigh, worktree `.claude/worktrees/den-rest`, branch `den-rest` (from main)
---

# Veilhounds rest at their den; loftstrider births

Wrysk, 2026-10-07: "the vielhounds did a hunt and seem to be resting currently. but theyre
like sleeping in the swamp area lol. they should really return to their den to rest, which
should be some overhang or ideally the cave." And: "i saw the pair of loftstriders in my
running world have three offspring within the first 30 minutes or so of running, so that
seems a bit high. although the population seems steady atm."

Read `design/handoffs/worker-rules-2026-10-07.md` first. Veilhound is `chorister` in code.

## 1. Den rest

- Packs already found on covered dens (console `k`), and breeders give birth at their den
  (`a_breeder_conceiving_away_from_its_den_gives_birth_at_its_den`). Rest does not go home.
- Make rest and sleep (after a meal, and at the circadian rest time) a trip home: walk to
  the den and rest there. Rest in the field only when the den is too far for the animal's
  current reserves (a knob) or unreachable.
- Den choice: roofed sites first. A cave or hall (dark, low sky visibility, roof overhead)
  beats an overhang, which beats open ground. Reachable on foot from the pack's range, not
  waterlogged. Choose again when the den floods or becomes unreachable. Reuse the terrain
  measures that exist (sky visibility, the darkness the glowcap habitat uses); don't invent
  new ones.
- Keep the existing "prey watch a sleeping or resting Veilhound" behaviour (afb5f913).

## 2. Loftstrider births

- Find why a founding pair had three young in about 30 sim-minutes. Check t = 0 first:
  founders arriving gravid, litter size, gestation, the wait between births, maturity. The
  world clock and the intended time scales are in `design/ecology-timescales-2026-10-02.md`
  and the configs (`config/desktop/terrarium.toml`).
- The loftstrider is a tall, slow browser. Intent: one calf per birth and a long wait between
  births, measured against the world day. Fix the mechanism or the defaults to match. State
  the visible effect: calves per pair per world day, before and after.
- Balance through roles (WORKING_POLICY): if the change moves the browse budget, name the
  stock that moves and the role that should answer it.
- Loftstriders dying out in a young world is expected (groves come later); don't add
  support for them.

## Check

- Hand-built fixtures for both mechanisms.
- One short detached run: founding, desktop terrarium config, 60 sim-minutes, 4 seeds on
  run-study.py slots, logging each Veilhound's rest positions against its den (distance,
  roofed or not) and each loftstrider birth. The batch writes the finished table, then
  `DONE`.

## Return

Commits, the den rule and loftstrider cause, the table path, the before/after calves per
pair per world day.
