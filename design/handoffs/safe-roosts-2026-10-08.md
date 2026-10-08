---
status: open
date: 2026-10-08
parent: roosts-and-rain-2026-10-08.md
---

# Seedporter roosts by safety from Veilhounds

Shared rules: `worker-rules-2026-10-07.md`. Follows the roosts-rain merge (076891b3).

## What happened

The roosts-rain package required a latticevine-covered wall topped by a ledge at least 1 m high
(`glider.roost_ledge_min_m`). On freshly founded worlds no ledge passed: about 140 vine-topped wall
columns per world were too low or failed the lip or footing checks. Vaulttree roosts worked (18 % of
nights).

## Wrysk's correction (2026-10-08)

"it does grow up mesas, it just takes longer. [...] the cliff itself should be the main factor. it
should be a perch that veilhounds cant get to, so whatever their climbing / jumping height is, and
even then its like, idk youd have to do it heuristically. like pick a spot thats fairly high
elevation and maybe has some ledges, and hope thats good enough. go further if the nest is ever
raided by veilhounds and dont come back, lol. [...] in the actually terain [...] there are
defeinitely mesas that fit the bill and have vines grow up there eventually (the vines tend to cover
the whole map) but theres also plenty of elevated groves and meadows that are also perfectly fine and
safe places they should nest in."

## What to build

The heuristic should be coarse; Wrysk said so explicitly.

1. **Safety is the main factor.** A roost site is good when Veilhounds can't get to it. Use their
   real climb and jump reach from the config, and do not add a new copy of it.
   - Score candidates heuristically: high elevation, and a cliff drop on its sides larger than the
     Veilhound's reach.
   - Candidates include elevated groves, meadows and mesa tops, not only vine-topped ledges.
   - A vine is only one way a seedporter climbs up, not a requirement.
   - Vaulttree crowns stay candidates. They are Wrysk's first words: "prefer to roost in vault trees".
     Rate them on the same safety footing.
   - Keep the travel budget and reserve guard.
   - The seedporter must actually be able to get there by its own climbing, walking and gliding.
2. **Raids.**
   - If a Veilhound raids a roosting or nesting seedporter (an attack, or an approach within a knob
     distance while it roosts), the seedporter remembers that site.
   - It never returns to it, and it looks further for its next roost. Read "go further" as a larger
     search radius after a raid.
   - Keep the memory per animal, bounded in size, with sim-path FxHash or dense storage.
   - Say whether "nest" already exists for seedporters (brain-curiosity mentioned high nests), and
     treat the roost as the nest if it does.
3. **Replace the vine-ledge-specific rule.** Remove `roost_ledge_min_m` or repurpose it. Keep the
   climb-over-the-lip support from 076891b3; it is useful on its own.
4. **Knobs and tests.**
   - Every number is a knob with a registry row; regenerate `design/parameters.md`.
   - Mechanism tests on hand-built terrain:
     - a high plateau beyond Veilhound reach beats a low spot;
     - a ledge within Veilhound reach is not "safe";
     - a raided site is never chosen again and the search widens;
     - a vaulttree crown is still chosen when it is the safest site in reach.

## Evidence

- Wrysk: use an established world save if a compatible one exists, otherwise don't worry. Today's
  schema bumps (flora 38, fauna 68) refuse all older saves, so expect none.
- Run the same 4 seeds × 60 min batch as roosts-rain
  (`scripts/studies/roost-rain-batch.sh`, adapted).
- Report where seedporters spend the night:
  - vaulttree;
  - safe elevated site;
  - other.
- Also report raids, raid-driven moves, and seedporter deaths by cause, before and after.
- Launch the batch detached and hand back. No video is needed; Wrysk will watch for it in the world.

## Return

≤40 lines:
- commits (on your branch, explicit paths, never merge);
- the heuristic and its knobs, with each one's visible effect;
- the table, or the detached run's paths and what's left;
- any deviation from Wrysk's words.

## Round 2: back to Wrysk's heuristic (2026-10-08)

Round 1 (branch `worktree-agent-aefeb9d2e7464ac39`, 5a500dec, 29e94f4f, 863277d0) defined "safe" as
"no Veilhound can walk there or reach it". With the seedporter's 0.25 m step against the Veilhound's
0.6 m, every place a seedporter can walk to is Veilhound ground. The rule therefore left only
vaulttrees (2 per world) and vine ledges, and changed nothing in 3 of 4 seeds. That strictness was
the worker's choice, not Wrysk's: "idk youd have to do it heuristically. like pick a spot thats fairly
high elevation and maybe has some ledges, and hope thats good enough. go further if the nest is ever
raided [...] plenty of elevated groves and meadows that are also perfectly fine and safe places".

- **Safety:** a heuristic score:
  - height above the surrounding ground (relief within a radius);
  - a bonus for cliff edges nearby, where an adjacent drop exceeds the Veilhound's step plus jaw band
    from its config.

  Walkability by Veilhounds is not required. Raids catch the mistakes.
- **Candidates:**
  - elevated ground (groves, meadows, mesa tops) the seedporter can walk, climb or glide to;
  - vaulttree crowns, scored on the same height basis;
  - vine ledges, as a way up.

  Ordinary crowns stay as the old fallback, not first-class candidates.
- **Raids:** never come back, and search further.
  - A raided seedporter must leave. It goes to the best non-raided candidate in its widened reach,
    even if that site scores lower than the raided one.
  - With no candidate at all, it falls back to today's rest and still avoids the raided spot.
  - This replaces round 1's "stay put".
- **Keep** round 1's fixes: the vine-foot steering, and the fallback only to real roost sites when
  better ones exist.
