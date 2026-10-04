---
status: open
date: 2026-10-04
owner: opus-high worker, worktree `.claude/worktrees/spread`, branch `spread` (from main)
---

# Springturf spreads: wind, burrs, runners; velvetpad on waterlogged ground; grass that cures in drought

Wrysk said go on 2026-10-04. He merged the succession work as it stood (bc40be9f): the G
defaults, overtop rate 0.

His direction (verbatim where quoted):
- "velvetpad should stay at this 50% cut" (alpha 2, the thin moss).
- "springturf not spreading is the problem":
  - real grass has runners, and its seeds travel very easily;
  - "radius can increase (call it 'wind' or whatever)";
  - seeds "like burrs that latch on to passing by animals and randomly fall off. pick an
    appropriate probability distribution for that: exponential / gamma, etc." Read
    https://en.wikipedia.org/wiki/Seed_dispersal.
- Velvetpad may sprout on waterlogged ground.
- "i do not think that the world should end up leaner. it should end up the same if not
  slightly richer because grass should have more foliage than moss".
- "grass can turn brown when its dried out and produce a lot of litter in the process. but then
  it regrows from reserves in the roots."
- Standing rules, still in force:
  - springturf is a California wild grass of open hills: drought-resistant, the main grazing
    crop;
  - velvetpad is mossy undergrowth and the early colonizer;
  - too-damp ground pushes turf out only through each species' own water stress;
  - food classes stay as they are.

**Read first:**
- `design/7_Research/succession-study-results-2026-10-04.md`: the last round's numbers and tools.
- `design/7_Research/astra-succession-results-review-2026-10-04.md`: findings 2–5 and 7–9 apply
  here.
- Code:
  - `crates/cubarium-voxel-flora/src/seeds.rs`: `Dispersal::{Drop, Wind}`, `hop`, `clonal_share`,
    `runner_target`, `runner_reach`, `overtop_covers`;
  - vine dormancy in `cover.rs` (`dormant`), as the precedent for curing;
  - `crates/cubarium/examples/succession_study.rs`.

## Deliverable, in this order

Every new number is a serde config field with a registry row (the every-number-a-knob rule).
All matter moved is booked, and every ledger is conserved.

1. **Overtopping on.** Springturf's overtop rate goes to 0.03, the study's pick, restricted to
   velvetpad as now.
2. **Wind.** Springturf disperses farther.
   - Use `Dispersal::Wind` if it fits. Otherwise add a distance kernel.
   - Choose the kernel from the reading: exponential, 2Dt, WALD, or another. State why in a
     doc comment and in the report.
   - Its mean distance is a knob. Arms at mean reach 6, 12 and 24 voxels, which is Astra's
     hop 3/6/12 at 0.125 m.
3. **Burrs (epizoochory).** A springturf stand's seed can latch onto an animal that moves
   through it, and falls off later.
   - Pick-up probability per contact is a knob, scaled by a per-lineage coat factor. Propose
     the factors from each body. Flyers pick up seed only when grounded.
   - A cap on seeds carried.
   - **Detachment:**
     - choose the retention distribution from the literature: exponential, gamma, Weibull or
       lognormal;
     - choose whether the clock runs in time or in distance moved;
     - shape and mean are knobs; justify both.
   - **Where a seed lands:** where the animal is when the seed drops. It joins the seed bank
     there and germinates only through springturf's own gates.
   - **Booking:**
     - a seed's matter comes out of the donor's propagule budget;
     - while carried it sits in a booked pool;
     - when its carrier dies, the seed drops at the carcass.
   - Any snapshot schema bump is fine: fresh worlds only, no migration.
4. **Runners.** A springturf runner may root on a velvetpad face as well as a vacant one.
   - It's paid from the parent's package like any runner, and only where `overtop_covers`
     allows.
   - The turf must pass its own gates there. The displaced pad is a booked cover death.
   - Set springturf's `clonal_share` and `runner_reach`, and say why.
   - This is no free replacement, and moss never overtops grass.
5. **Velvetpad on waterlogged ground.** `establish_saturated_max` goes to 1.0, keeping relaxation
   and depth 0.1 m.
   - Drowning stays a separate limit.
   - Report how many of the vacant wet faces it now fills.
6. **Curing.** A turf stand whose own root-box water stays below its wilt level for a sustained
   time (a knob) cures:
   - it drops most of its foliage to litter (booked), then goes dormant: no photosynthesis,
     reduced maintenance;
   - its root reserve survives. Use the stand's existing structure or wood pool if it can
     serve as that reserve. Otherwise add a booked reserve;
   - it regrows foliage from that reserve when water comes back above a resume level;
   - a long enough drought still kills it.
   - Expose a `cured` flag in the render view. The plain-voxel draw of a cured turf uses a
     colour knob, default a straw brown. Wrysk judges the look; screenshot it headless.
7. **Production.** Velvetpad stays at alpha 2. Set springturf's own knobs (alpha, foliage and
   wood rates, capacities) so that:
   - a mature turf stand carries clearly more edible foliage than a mature pad stand, about
     1.5–2×;
   - at hour 6, world edible foliage and litter come out at **1.0–1.15× the pre-G reference**:
     velvetpad alpha 4 with the old springturf, which was last round's `base` arm.
   - Don't move any other species' numbers.
8. **Measurement repairs** from Astra's review:
   - build the cohort from the untouched loaded flora before `replace_config`, and assert site
     IDs and classes, not just totals (finding 2);
   - labels must say what they measure: deaths with causes versus disappearances; delivery
     receipts; turf→woody continuity, so the persistent `turf_donor_seen` no longer counts an
     unrelated later seedling (finding 3);
   - split failed gates into light, moisture, saturation and flooding;
   - report kills, carcass production, bites and assimilation for the Veilhound (code
     `chorister`) alongside B (finding 9).

9. **The co-training screen.** Since the merge, main fails
   `screen_tests::the_shipped_start_passes_the_screen_where_the_defaults_fail` with
   `S 0.485 outside [0.02, 0.2]`: the G turf changed the supply ratio.
   - After step 7, re-check it.
   - Either update `config/training/search/cotrain-start.toml` or the screen's S band. Say which,
     and why. Don't just delete the test.

## Tests (write them first, check that each fails on main)

- Runners root on velvetpad only when the target list allows it and turf's gates pass. A pad
  under a forbidden cover is never displaced. The displaced pad is booked.
- Burrs conserve: picked = carried + dropped + dropped-at-carcass, per seed and in the ledgers.
- Over many seeds, a sampled retention matches the chosen distribution's mean and variance.
- A sampled wind distance matches its kernel's mean.
- A grounded flyer can pick up seed; one in flight can't.
- Curing:
  - a dry stand drops foliage to litter, booked;
  - while dormant it never photosynthesises;
  - it regrows from its reserve after re-wetting;
  - it dies past the long-drought limit.
- Velvetpad establishes on a saturated, unflooded face, and not on a flooded one.

## Verify

- **Fixtures:** the old saves predate any new state, so found fresh worlds. Run with fauna
  present from the start, because burrs need animals.
  - Seeds 1, 3, 5 and 6 first, 6 h each, at the default speed.
  - Then the selected arm on all eight (1, 3, 5, 6, 8, 9, 12, 14).
- **Arms:**
  - pre-G reference;
  - merged main;
  - + overtop 0.03;
  - + wind (6, 12, 24);
  - + runners;
  - + burrs;
  - all on, at the chosen wind reach.
- **Readouts:**
  - S1: the turf share of the hour-2 cohort at hours 4 and 6, as a geographic coverage map, not
    only nearest-donor subsets;
  - world edible foliage and litter against the pre-G reference;
  - turf and pad edible per stand;
  - wet-face occupancy;
  - burr deliveries: count, distance distribution, and the share that germinated;
  - cured stands through a dry spell, if one occurs;
  - fauna B for every lineage against the reference.
- **No pass gates.** Wrysk merges and iterates. Report the numbers plainly.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/spread -b spread main`.
- **Commits:** explicit paths only, never `-a`. Never merge, deploy, or restart a host. Headless
  only.
- **Scratch:** only `<scratchpad>/spread/`.
  - `<scratchpad>` = `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad`.
  - Delete nothing outside it.
- **Machines:**
  - Desktop: at most 16 threads, `nice -n 10`.
  - Eidolon (`ssh eidolon.local`): all cores free. Its shell is fish, so run bash script files.
    Use renamed binaries under `~/spread/`.
  - No AWS.
- **Long runs: don't sit and wait.** Any batch longer than about 30 min:
  - launch it detached (`setsid nohup`), with every remaining stage chained in one script that
    writes `DONE` markers at `<scratchpad>/spread/DONE` and `eidolon.local:~/spread/DONE`;
  - stop all your own background tasks, then hand back. That ends your turn; I start a fresh
    worker for the analysis.
  - No polling loops, no progress checks.
  - The hand-back must be self-contained enough to brief a fresh worker from:
    - what runs where;
    - the output paths;
    - the analysis scripts and the exact commands left;
    - the decisions already made, with their reasons.
- **Processes:** never `pkill -f` or `pgrep -f` a pattern that appears in your own command line.
  Leave no background shells.
- Say "Veilhound" in prose. Code identifiers stay `chorister`.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

Either the detached hand-back above, or, if everything finished inside 30 min:
- branch and head;
- each mechanism as built, with the chosen distributions and why;
- tests, and whether each failed on main first;
- the arm table;
- the cured-turf screenshot path;
- open questions.
