---
status: open
date: 2026-10-02
owner: opus-medium worker, read-only on main (no worktree needed; write only the report)
---

# Time scales: what the world runs on today

## Why

Wrysk, 2026-10-02, on the next ecology search (the full roster):

> look into plant/animal metabolic rates and adjust for day/night cycles … in the case of
> real-time day night … 12hr night in the world would be an unreasonably long time to go with
> no energy input. so maybe … day night is more of a visual than how strongly it affects
> ecology? or plants are able to photosynthesize moonlight, albeit at some percentage, and
> animals still sleep / have nocturnal cycles. also … when does hunting happen and at what
> cadence for predators. that needs to be balanced too with birth rates.

Fable will write the proposal; Astra (GPT via codex) reviews it, and Wrysk decides. This
package gathers the **facts** the proposal needs, from the code and from short
measurements. **Recommend nothing.**

## Deliverable

`design/7_Research/timescales-inventory-2026-10-02.md`. It is evidence, not decisions. Use
tables, and give every number with its registry path or `file:line` and its unit.

### 1. The clock

- The day-clock modes (cycle, local, off).
  - Each mode's day length and its night fraction.
  - What each live preset uses: desktop, voidrunner and tachyon.
  - What co-training worlds use.
- What night changes in the sim today:
  - photosynthesis;
  - any plant respiration or growth from stores;
  - the animal circadian;
  - which lineages are diurnal or nocturnal;
  - the sleep metabolic multiplier;
  - movement;
  - the lanternjaw's dusk hatching;
  - anything else gated on light.
- Moonlight or ambient light at night: does any model term exist?

### 2. Animals: one row per lineage, all nine

Cover the littershredder, frondgrazer, lanternjaw, capgnawer, loftstrider, ripple-snail,
bellwing, seedporter and chorister. At adult size and the **current live defaults**,
including any trained-ecology edits that have landed by the time you run, give:
- basal upkeep;
- reserve capacity;
- **hours from full reserve to starvation**, awake at rest and asleep;
- feeding intake rate against upkeep: the surplus, and the hours of feeding that fund one
  hour asleep;
- age at first breeding;
- incubation or gestation;
- litter or clutch size;
- surplus floor;
- median lifespan;
- motor cost per body length.
- For predators, also give:
  - hunger or strike threshold;
  - strike success;
  - digestion or satiety time;
  - kills per day needed to break even;
  - the chorister's pack provisioning rate.

### 3. Plants: one row per species

- growth rate;
- foliage turnover (leaf life);
- standing biomass per stand;
- seed or fruit or nectar production rate;
- what happens to a plant at night, and over an 8 h and a 12 h dark period.

### 4. The balance numbers

- Per herbivore lineage: the grazed share of plant production at today's founding counts.
- Per predator: the prey births per day needed to sustain one predator at the break-even
  kill rate, against the prey lineages' actual birth rates in the 2026-10-02 gate (CSVs
  under `runs/gate-curio/`).

### 5. Real-world anchors

Cite the existing audit, `design/7_Research/` and the parameter audit's baselines first.
Add short sourced web references only where they are missing:
- overnight fasting tolerance and torpor or sleep metabolic depression, for small
  endotherms and ectotherms;
- night-time growth from starch in plants;
- moonlight's irradiance against the sun's;
- predator kill frequencies for small predators.

### 6. One measurement

- Desktop terrarium on main, two seeds, cycle mode, run for one full day length if that is
  ≤ 2 h of sim; otherwise 2 h, and say so.
- Log, per lineage, energy in and out by day and by night, deaths by cause and phase, and
  births by phase.
- Use `--threads 8` and `nice -n 10`, headless.

## Constraints

- **Read-only on code.** Write only the report. Commit it on main with an explicit path,
  and push nothing else.
- **Shared desktop:** use `nice -n 10` and at most 8 threads. Never put a window on Wrysk's
  screen.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End the commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

The report's path and commit, the five numbers you think most constrain the design (facts
only), and any section you couldn't fill, with the reason.
