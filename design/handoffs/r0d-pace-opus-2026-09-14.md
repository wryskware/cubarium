---
design_status: exploration
last_reviewed: 2026-09-14
---

# Opus: R0d pace calibration (time-boxed)

Wrysk's direction after R0b: the display is real space. A creature should move the way
an animal of its apparent size moves in a terrarium. The scale of that space is not
fixed (an 8-inch cube or an 8-metre one), so calibrate in **body lengths per second
(BL/s) of the unit adult**, never in pixels. Pixels stay an internal unit only.

Time box: aim for 25 minutes of work, hard stop at 40. If a step will not fit, ship
what is done and say what is left. Do not widen scope to make anything perfect.

## Targets

- Body length `BL = 2 · phenotype.extent` of the unit adult (currently 5 px).
- Cruise `speed_max` = **1.0 BL/s** for the unit adult (today 0.06 BL/s). Keep the
  existing size exponent so larger bodies are faster in absolute terms and slower in BL/s.
- Prey escape multiple and hunter strike speed: keep the same *multiples* of cruise
  they have now; scale the apex profile's `strike_speed_px_s` by the same factor as
  cruise (about 16.7×). Bump the profile version only if `validate` forces it.
- Turning: change nothing. The R0b shared budget already gives a unit adult ≈ 115°/s at
  1 BL/s cruise; the genome ceiling (90°/s) binds again. Report the apex's attainable
  pivot rate at its new cruise and leave it.
- Energy: hold the bill per second at full cruise where it is today, i.e. scale
  `move_cost` by `0.3 / new speed_max` (≈ 0.00036). Rotation price scale unchanged.
  State plainly that travel per distance becomes ~17× cheaper and that the ecology is
  to be rebalanced later by the headless search, not by hand here.
- Express the new defaults in code comments as BL/s with the derivation, so the
  numbers are readable as a calibration rather than magic.

## Persistence fact that keeps this small

`WorldConfig` is persisted inside every snapshot, so resumed worlds and every
continuation fixture keep their old speeds and are unaffected. Only worlds built from
`WorldConfig::default()` change. Do not migrate saved configs and do not touch the live
`state/` directory.

## Do

1. Change the defaults (`config.rs` OrganismConfig, founder kinds if they carry speed,
   `hunter/profile.rs` strike speed). Use Graft to find every consumer of `speed_max`
   and the presentation constants that map speed to gait/stride (e.g. the saturating
   reference speed in `crates/cubarium/src/hunter_present.rs` and any organism
   presenter); scale those presentation references by the same factor so animation does
   not sit permanently saturated. Presentation only, no new visual logic.
2. Run `cargo test -p cubarium-core --release` and the presentation suites that consume
   speed. Fix only failures caused by this change: value pins get the new value; timing
   windows get the measured new value with a one-line reason. Leave unrelated failures
   alone and list them. One full `cargo test --workspace --release` at the end, once.
3. Rerun `examples/mobile_grazing` (≈5 s) at the new pace, same fixture, and report the
   four-arm table next to R0b's.
4. Native demo without touching the current world: stop the running runner
   (pid from `pgrep -af target/release/cubarium`), then launch
   `./scripts/run-cube.sh --fresh` **with `--state` pointed at a new scratch directory**
   `state-pace-demo/` (check how run-cube.sh passes `--state`; pass it after the script's
   own arguments if it overrides, or invoke the binary directly with the same flags and
   the scratch dir). Confirm via `curl localhost:7393/status`. Pull a few mirror frames
   over ~30 s and say whether bodies visibly travel and turn. Leave it running. Do not
   delete or modify `state/`.
5. Commit crates and a short result note
   `design/7_Research/r0d-pace-calibration-2026-09-14.md` (≤ 80 lines: numbers before
   and after, tests, grazing table, what the demo showed, apex pivot rate, what was left).
   Stage only owned paths; never `git add -A`; there is unrelated uncommitted work in
   `.claude/`, `design/handoffs/`, `design/*.md`, `.agents/`.

## Do not

No ecology tuning beyond the one `move_cost` scaling, no regrowth/intake changes, no
sensor/RNN work, no edits to `design/recurrent-interface-contract.md` or the plan, no
world reset of `state/`, no sub-agents, no capture archive, no fixture regeneration
unless a fixture genuinely depends on the default config.

Return: the before/after table, commit SHAs, test counts, the grazing table, the demo
observation, runner status, what was left, and measured usage or "unavailable".
