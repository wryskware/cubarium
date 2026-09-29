---
status: open
date: 2026-09-28
owner: Fable (organism line); Wrysk said go 2026-09-28
follows: design/7_Research/ecology-parameter-audit-2026-09-28.md (§8); voxel-browser-movement-2026-09-28.md
---

# Audit round: the world we were tuning is mis-scaled

Wrysk's parameter audit shows that parking is the right strategy in this world. One plant
regrows enough to feed 5–8 parked browsers forever, because plants and animals are counted
in org-units roughly 1,000× apart. The es4 search is stopped and its held-out check
skipped. Commit each step when it is tested, and report its SHA in one line; merges land
as they come.

## Steps

1. **Motor cost tied to upkeep (§8.1).**
   - Walking cost = `motor_factor` × `maintenance_per_s`, per lineage, default 1 (the
     shipped ratio), so an upkeep multiplier no longer silently changes the price of
     walking.
   - `motor_factor` is a search parameter, log range [0.5, 4].
   - Retire `config/training/ecology-tuned-2026-09-28.toml`: mark it superseded in its
     header.
2. **Mass-scale test (§1.4, §8.2).**
   - A per-lineage `mass_scale` multiplies every absolute animal quantity the audit lists:
     `body_max`, `body_min`, `birth_body`, `birth_cost`, `surplus_floor`, `egg_organic`,
     `bite_per_s`, `bite_half_stock` and `growth_max_per_s`. Per-unit rates and metres are
     left alone.
   - Heuristic walker vs round 2's parker at ×1, ×30, ×100 and ×300, for browsers; the same
     for shredders against litter.
   - New-defaults world, small, still water, seeds 1–8 × 60 min, plus one 6 h census at the
     winner.
   - Report walked m/h, moving share, intake per body-hour, births, starved, and the
     population at 6 h.
   - Compute R, S and F at each scale, to confirm the audit's arithmetic.
   - Say how the brains' senses (taste saturation and so on) read at the new mass.
   - `mass_scale` becomes a search parameter, log range [1, 1000].
3. **Breeding eligibility (§8.5; Wrysk decided).** An animal may breed after being in
   surplus for at least 80 % of the last 900 s, instead of all of it unbroken, so a
   foraging trip is not a reset. This lands before the mass test's final census.
4. **Defence readable, on the biological clock (§8.4).**
   - The mouth's stand choice reads (1 − defence) × stock.
   - Switch-on within a few minutes of grazing; relaxation over tens of minutes.
   - Taste: change it only if the contract allows it without a new input. Otherwise say so
     and leave it.
5. **Training worlds are the live world (§8.6).** The frozen arenas and landscapes get the
   graze refuge, the defence, the night and the ecology vector.

## Wrysk's decisions, recorded

- **The reward (§8.3):** keep the score as it is, with no new terms. The world, through
  mass scale, must make parking unable to sate.
- **One clock, K_bio (§8.7):** yes, but after the mass test, and agreed with the weather
  thread's night work. Not started here; it gets its own brief.
- **The registry (§8.8):** held until predation P1 is on main.

## Constraints

- CCD1 under `nice -n 10`.
- Tests first, each ≤ 200 ticks.
- Explicit-path commits with the two trailer lines. Merge main in often.
- Block in the foreground, and kill your own wait loops before reporting.

## Return (≤ 40 lines)

End with a recommendation for the next brain training round.

## State at handover (2026-09-28, branch live-train at the commit adding this)

- **Done and tested:** step 1 motor_factor `043a18c2` (brief `55989265`); step 3 breeding window (80 % of the last 900 s) `9e19afd1` + fauna schema 20 `20bce514`. Full suite 2558/2558 before the schema bump; fauna + cubarium 963/963 after.
- **Not started:** step 2 (mass_scale: add `founder.mass_scale` to `LINEAGE_PARAMS`/`PARAMS` in `crates/cubarium-search/src/es/voxel/live/ecology.rs`, log [1, 1000], multiply the nine absolute fields in `fauna_config`), step 4 (defence: `fauna body.rs` mouth choice ~`:1330`, flora `DEFENCE_PER_BITE`/`DEFENCE_HALF_LIFE_S` in `flora lib.rs`; the taste rule text is in the manifest digest, so check `manifest.rs` before touching taste), step 5 (`arena.rs` uses `.without_graze_refuge().without_defence()`; landscapes and arenas build with default configs).
- **Taken from the last package:** step 1/3 results (walker vs parker, herding) are in `runs/step1`, `runs/step3`; es4 was stopped at gen 7 (`runs/es4`), no check.
- **Running checks:** `scripts/live-train/ecology.sh check|search …` wraps `cubarium-search voxel-ecology-{check,search}` at `nice -n 10 taskset -c 8-15,24-31` with round 2's centres. Check arms: `--arm name=default|<ecology.toml>`, `--seeds 1,2,3,4 --hours 6 --water-every 1 --threads N --out DIR`; one CSV per world, one row per 10 min. Brains per lineage via `--centre browser=<file>|heuristic|herding`. `scripts/live-train/walker-vs-parker.sh` is the step-1 grid; `arms.py DIR…` and `check_summary.py DIR` summarise.
- **Gotchas:**
  - Launch long runs with `nohup setsid bash script.sh >/dev/null 2>&1 </dev/null & disown`; wait with `until … ps -eo args | grep -q "^target/release/cubarium-search …"`. Don't use `pgrep -f`: it matches your own shell's command line.
  - The scratchpad can be wiped between sessions; keep scripts in `scripts/live-train/`.
  - `--water-every 4` (coarse water) is for searches and training only. Checks and censuses run `--water-every 1`, and the live app refuses anything else.
  - Round 2's centres: `config/training/centres/r2-{frondgrazer-upd160,littershredder-upd512}.json`. They still load under the current digest (tested in `crates/cubarium-search/tests/round2_centres.rs`).
  - `rustfmt` on a crate's `lib.rs` also reformats its child modules. Format single leaf files only, and check `git diff --stat` before committing.
  - Main moves often and has conflicted in `water.rs` (coarse water) and the examples. Merge, then run the full suite: `cargo nextest run --release --workspace --exclude cubarium-gpu`, `CARGO_TARGET_DIR=$PWD/target`, `RUSTFLAGS="-C target-cpu=native"`.
  - Timing- and deadline-based tests can flake while CCD1 is loaded. Rerun before chasing them.
  - `config/training/ecology-tuned-2026-09-28.toml` is superseded; don't train on it.
