---
status: open
date: 2026-10-02
owner: opus-medium worker, worktree `.claude/worktrees/ljrecruit`, branch `ljrecruit`
---

# Lanternjaw recruitment: where does the chain break?

Lanternjaws hatch but never establish, in every gate arm. Before any predator tuning, find
which link fails. This is step 2 of the order Wrysk accepted on 2026-10-02
(`design/ecology-timescales-2026-10-02.md` revision 3, Part 1c and Astra [A8] and [N1]).

## The chain (code: `crates/cubarium-voxel-fauna/src/step.rs:320-373`, `:2867-2988`)

1. **Egg bank.** Founded as dormant eggs, with a viability of 6–24 h.
2. **Dusk gate.** On the dusk tick, an egg hatches only when all of these hold:
   - distinct prey heard in the last clock-day ≥ the egg's threshold (2–10);
   - no adult within 4 m;
   - **a host within 0.75 m with a free rider slot** (`parasite_max_per_host` 1);
   - the 1/3 draw.
3. **Rider.** It drains its host at 5 % of the host's awake basal, and drops off at 0.24 of
   adult body, or is freed when the host dies.
4. **Free juvenile, then adult.** It ambushes and must reach `birth_body` to breed, about
   176 min of growth.

## Build (readouts only; no behaviour change)

- **Reuse what exists** (`crates/cubarium/examples/voxel_census.rs:986,1184-1187`;
  `fauna/src/lib.rs:2262-2268`): `riders_now`, `riders_attached`, `rider_dropoffs`,
  `riders_freed`.
- **Add, per world:**
  - at each dusk:
    - eggs viable;
    - eggs passing the footfall threshold;
    - eggs blocked by an adult;
    - eggs with a host in reach;
    - eggs drawn;
    - hatches;
  - per rider: attach time, the host's lineage, growth per hour, and detach cause and age;
  - per free lanternjaw: strikes, hits, kills, intake, and reserve over time;
  - death cause and age;
  - adults established: reached `birth_body`;
  - breeding events.
- **One summary line per world** that names the first link with zero throughput.

## Measure

- Desktop terrarium on main's live defaults. Seeds 1–8; each run must span at least two
  dusks: 2 h plus the next dusk, about 3 h.
- `--threads 8` or fewer, `nice -n 10`, headless.
- Eidolon is also free for this (`ssh eidolon.local`; fish shell, so wrap commands in
  `bash -c` or a script file; its binary lives in `~/cubarium-train`; use a new binary name
  and never overwrite a running one).
- Report the chain table: counts at each link, summed and per world, and the first broken
  link per world.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/ljrecruit -b ljrecruit main`. Commit
  there with explicit paths only. Never merge, deploy, or restart a host.
- **Change no sim behaviour and no parameter.** Readouts only; tests must stay green.
- **Shared desktop:** use `nice -n 10` and `-j 8`. Headless only.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 30 lines)

Branch and head, the chain table, the first broken link, and two or three observations
(facts) about why it breaks. Recommend nothing.
