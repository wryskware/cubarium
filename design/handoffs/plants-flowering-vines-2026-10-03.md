---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/plants3`, branch `plants3`
---

# Plants: sparse, staggered flowering; bloomcrowns always in flower; vines on open faces; velvetpad's niche

Wrysk's notes, 2026-10-03, with a screenshot of the live world:
`/home/wrysk/Pictures/satty-2026-10-03_03:47:51.png`. The red rings mark large open cliff faces
that no vine has taken.

> "latticevine … basically the entire plant seems to flower and fruit on the same timer. and
> … basically every cell is able to flower and fruit. flowers and fruit should be much
> sparser, and on random time intervals. so if a large face is covered in vine, there should
> be a fair number of flowers and fruits going at any time, but spread out sporadically.
> enough food when a large colony is established."
>
> "lanternberries are mostly okay. they can be a little sparser or more randomly spaced, but
> theyre not really an issue"
>
> "arent bloomcrowns like *always* flowering? … a bloomcrown should provide a constant source
> of nectar at all times."
>
> "vine still isnt establishing itself correctly … big open cliff faces that have no vines at
> all … i dont understand why the founding vines never choose these front facing giant open
> faces."
>
> "most of the world is overrun with velvetpad … basically every other plant should
> out-compete it (… some niche where velvetpad wins, like … soggy places that rarely dry
> maybe)."

## Part A: build (Wrysk's direction is clear)

1. **Latticevine flowering and fruiting is sparse and staggered.**
   - Each site that can flower does so independently, at random intervals: a per-site random
     onset, a flower lifetime, then fruit.
   - Only a small fraction of a colony's sites carry a flower or a fruit at once. Make that
     fraction a knob.
   - A large colony always shows a fair number of flowers and fruits, scattered across it.
   - Use a hash of site and time or a seeded RNG. Don't add global synchrony.
   - **Production per unit of vine area is a knob.** Report the steady-state nectar and fruit
     a colony of N cells supplies per world day, against a bellwing's and a seedporter's
     daily need (`f` in the registry), so we know what size of colony feeds one animal.
2. **Lanternberries:** the same sporadic placement, a little sparser. Keep their totals about
   where they are.
3. **Bloomcrowns are always in flower.**
   - Nectar is produced continuously and capped by stock, so a bloomcrown is never at zero
     because of a phase.
   - At night, the night budget's `night_bloom_scale` still sets the refill rate. That is
     fine: the stock never empties because of the clock.
4. **Rendering:** flowers and fruit are drawn only at the sites that carry them. Change which
   cells show them, not how they look; the look is Wrysk's.
5. **Books close:** flora organic matter, mineral and energy. Vine products keep repair 3's
   mineral, from `Vine::mineral`.

## Part B: diagnose first, then fix

1. **Vines on large open faces.** Why do founding vines, and vines spreading later, never
   take the ringed faces? Trace founding site choice, dispersal, the climb and attachment
   rules, and the light and water rules on walls.
   - Use a generated terrarium world with the plants only (`voxel_census … nofauna`), and
     headless captures (`--gpu-target headless`, `--gpu-capture`). Never put a window on
     screen.
   - **If it's a defect** (a rule that excludes faces it shouldn't, a founding choice that
     never reaches them), fix it, test it, and show before and after captures.
   - **If it's an ecological rule working as designed,** report it with numbers and don't
     change it.
2. **Velvetpad dominance.** Find what lets velvetpad win almost everywhere, using numbers from
   the competition model: establishment, shading, water, grazing and turnover.
   - **Propose,** don't build, a niche where velvetpad wins, such as soggy ground that rarely
     dries, and why the others beat it elsewhere.
   - Fable sends this proposal to Astra before any change.

## Tests (write first; don't weaken them)

- Vine flowering is asynchronous: on a large fixture colony, the share of sites in flower stays
  inside a band over a world day, with no global spike. The flowering sites spread spatially.
- Long-run production per area matches the knob within tolerance.
- A bloomcrown's nectar stock never hits zero from the phase, day or night, with no
  consumers.
- The vine fix, if any, has a fixture: an open face that a vine now takes.
- The books close over a cycle.

## Measure

- Plants only, seeds 1, 3, 5, 6 × 6 h, on main and on the branch:
  - stand counts per species;
  - vine cells on walls;
  - flowers and fruits standing over time.
- Plus one headless capture per seed at the end.
- Long runs are fine tonight (Wrysk: "feel free to run things locally"). Use `nice -n 10` and
  at most 8 threads in total.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/plants3 -b plants3 main`. Commit with
  explicit paths only, early and often. Never merge, deploy, or restart a host.
- **Stay in the flora crate** and the plant rendering. Another worker is on the fauna crate
  (bellwing finding) and another on `cubarium-search` (the grid tool).
- `design/parameters.md` is generated. On a conflict, regenerate it.
- Bump the flora schema if saved state changes.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- Part A as built, with `file:line`;
- the production-per-area numbers against bellwing and seedporter need;
- Part B: the vine diagnosis (and fix, if any, with capture paths) and the velvetpad
  diagnosis and proposal;
- tests;
- the measure table.
