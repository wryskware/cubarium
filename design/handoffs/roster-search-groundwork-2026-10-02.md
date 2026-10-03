---
status: open
date: 2026-10-02
owner: opus-high worker, worktree `.claude/worktrees/rsearch`, branch `rsearch`
---

# Full-roster ecology search: the groundwork

Step 3 of the order Wrysk accepted on 2026-10-02: Part 3 of
`design/ecology-timescales-2026-10-02.md` (revision 3). Read Part 3 and Astra's [A5], [A6],
[A10] and [A11] in `design/7_Research/astra-ecology-timescales-review-2026-10-02.md` first.
Also read:
- `design/cotraining-plan-2026-10-01.md`, especially the targets and both result sections;
- `crates/cubarium-search/src/es/voxel/live/{search,screen,targets}.rs`;
- `config/training/search/cotrain.toml`.

**Goal:** a search file and harness under which the ecology-only search can run overnight
on the full nine-lineage terrarium, without wasting the night. Two things are out of scope:
- **Running the overnight.** Fable launches it after Astra reviews your screens and
  targets.
- **The night mechanics.** Package `night` (`design/handoffs/night-survival-2026-10-02.md`)
  builds them in parallel. Merge main when it lands and wire its `night_bloom_scale` and
  founders-full knobs in as genes or fixed values.

## Build

1. **Genes**: `config/training/search/roster.toml`, a new file. `cotrain.toml` stays as it
   is.
   - **Per lineage, all nine**, wherever a row exists:
     - upkeep, motor, bite or intake, yield;
     - surplus floor, lifespan, incubation or gestation, growth;
     - reserve per body;
     - `sleep_upkeep`, `wake_hunger`.
   - **Chorister:** the founding pack size.
   - **Plants:** the existing plant groups, plus `night_bloom_scale` once `night` merges.
   - **Ranges** must reach the proposal's targets. For example, lanternjaw and chorister
     upkeep need rows reaching below the registry's 2.5e-4/s. Widen registry search ranges
     where needed.
   - **List every gene with its range and its ties** (derive rules), and say where the
     upkeep tie also moves bite and motor.
2. **Screens**, in the arithmetic pre-screen beside R/S/F. Each refuses with a named reason.
   - **Night endurance (optimistic adult):** `max(0, f0 − (1 − h)) × c / (m × s)` must be
     ≥ 1.2 × the lineage's sleeping phase.
     - `f0` is 0.8.
     - The sleeping phase is computed from the clock and the lineage's activity thresholds:
       0.3 for diurnal and nocturnal, 0.05/0.95 for crepuscular.
     - Compute exactly; don't round.
   - **Food access per food type**, with consumer demand and producer supply in the same
     units:
     - foliage, by mouth band;
     - litter;
     - glowcap caps;
     - glassfilm;
     - nectar and fruit, with the day and night rates and caps (a seedporter needs about
       7.7 harvested stands);
     - carcass and carnivory.

     Each check is a necessary condition, documented as optimistic.
   - **Predator balance:** a **readout only**, not a refusal (Astra [A6]).
3. **Targets, per lineage and per life stage** (proposal Part 3, Astra [A10]):
   - **Require actual births** where one fits the 2 h horizon. For the lanternjaw and
     chorister, use explicit progress criteria instead (a rider attached, a juvenile
     growing, pups provisioned).
   - **Readiness never substitutes for a required birth.**
   - **Replenishment** covers a whole cycle's measured spending.
   - Count egg-bank hatching separately from adult breeding.
   - Count riders separately from established hunters.
   - Chorister: pup survival and provisioning, not a second-half litter.
   - **Keep:** every lineage and every plant persists; the band; browsers moving
     ≥ 0.30; lanternjaw share 5–15 %, scored softly while it can't establish.
4. **Brains:** shredders and browsers on the built-in C1 centres, everyone else on
   heuristics. No brain pairs (ecology only).
5. **Finalist validation** (a subcommand or a flag on `voxel-ecology-check`): run a
   finalist's worlds past the next dusk, pup independence (2 h) and one litter interval.
   About 8 h of sim, finalists only.
6. **Exhibit a passing candidate.** Hand-pick or search a genome that passes every screen,
   and run it once: 2 seeds × 2 h. Report what each lineage did.

## Tests (write first; don't weaken them)

- each new screen refuses a hand-built failing case, passes a passing one, and names its
  reason;
- the night-endurance arithmetic matches the proposal's worked numbers (700 s and 1,167 s);
- a lineage with readiness and no birth fails a birth-required target;
- the decode of `roster.toml` touches every listed path, and nothing outside it.

## Smoke

- λ 2, 2 seeds, 1 h, `--threads 8`, `nice -n 10`, at most 30 min of wall time.
- It must run end to end and write `generations.jsonl` with the new target readouts.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/rsearch -b rsearch main`. Commit there
  with explicit paths only, early and often. Never merge, deploy, or restart a host. Don't
  launch the overnight run.
- **Live defaults stay unchanged**, and code defaults too, except registry search ranges.
- **Shared desktop:** use `nice -n 10` and `-j 8`. Headless only.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- the gene list summary (count per lineage, the widened ranges);
- each screen, with what it refuses;
- the targets as built;
- the exhibited candidate's run;
- the smoke result;
- open questions for Fable.
