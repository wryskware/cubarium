---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# R0d: the pace calibration, in body lengths per second — 2026-09-14

Evidence for Fable against [the R0d brief](../handoffs/r0d-pace-opus-2026-09-14.md), measured
against [R0b](r0b-motor-foraging-result-2026-09-14.md). This decides nothing: it recalibrates one
speed, holds the per-second energy bill, and says plainly what that costs the ecology.

## 1. Before and after

Body length of the unit adult is `BL = 2 · extent` = 5 px. Pixels stay an internal unit.

| | R0b | R0d | note |
| --- | ---: | ---: | --- |
| `organism.speed_max` | 0.3 px/s = 0.06 BL/s | **5.0 px/s = 1.0 BL/s** | ×16.667 |
| `organism.move_cost` | 0.006 | **0.00036** | ×0.3/5.0 |
| bill per second at full cruise | 0.0018 e/s·S | 0.0018 e/s·S | **unchanged** |
| bill per pixel travelled | 0.006 e/px·S | 0.00036 e/px·S | **~17× cheaper** |
| apex `strike_speed_px_s` | 1.0 | **16.667** | same multiple of cruise |
| `HUNTER_FULL_SPEED_PX_S` | 0.25 | **4.1667** | presentation only |
| unit adult pivot, full effort | 6.88 °/s (budget) | 114.59 °/s budget → **90 °/s** | genome ceiling binds again |
| apex (r = 14.83 px) pivot, full effort | 0.97 °/s | **16.28 °/s** (0.2836 rad/s) | |
| apex pivot during a paid strike | 3.87 °/s | **64.5 °/s** (1.126 rad/s) | under the 90 °/s ceiling |

Turning, `ROTATION_COST_SCALE`, regrowth, intake, the `size^-0.25` exponent and the escape/strike
*multiples* are untouched. **Travel per distance is ~17× cheaper, and that is an ecological change
this milestone does not correct** — it belongs to the headless parameter search. `WorldConfig`
rides inside every snapshot, so resumed worlds and the continuation fixtures keep their old speeds.
Nothing was migrated; `state/` was not touched.

## 2. One defect the pace exposed, and fixed

`world/step.rs` decided whether a pursuing member should stop approaching with
`body.x < capture_offset_body.x − tolerance − closing`, `closing = strike_speed · strike_seconds`.
That is the *far* side of the lunge, not the reach envelope the comment beside it names, and it
was false for a prey sitting in the claws. It went unnoticed because `closing` was 1 px, inside
the 2.6 px contact tolerance. At 16.667 px/s the member charged 16.7 px through point-blank prey
and missed — 17 hunter fixtures failed. The predicate is now `body.x < capture_offset_body.x +
tolerance`: the envelope itself.

## 3. Tests

`cargo test -p cubarium-core --release` → **393 passed, 0 failed**.
`cargo test --workspace --release` → **1229 passed, 0 failed, 20 ignored** (one run, at the end).
No unrelated failures were found; nothing was left failing. Each reason is in the source:

- **Value pins** re-recorded: resting sweep 0.02 → 0.24 px/s (measured 0.2350 = 0.047 BL/s); two
  `hunter.rs` observation hashes; `hunter_present.rs` drawn-claw slack 0.5 → 1.4 px (the member
  covers 0.83 px inside the settlement tick, still inside the core's own contact tolerance);
  `hunter_charging.rs`'s 600-tick continuation as `hunter-v3-charge-active-plus600-r0d.cubw` (the
  other seven pairs regenerate byte-identical).
- **Windows** reset to the same *physical distance* the old one bought — depth-band walk 1,600 →
  100 ticks, `redesign_rules` 400 → 24 and 200 → 12 — because the old ones now walk the probe off
  its face. `hunter_geometry`'s juvenile is frozen for the adult-reach half: at 1.0 BL/s it walks
  the 8 px in 200 ticks, which proves nothing about the published scale.
- **Turn ceilings** (`world/tests.rs` sensing, `diagnostic_seam.rs` script) are now
  `min(budget, genome)`: the budget buys 0.1 rad a tick, the genome's 90 °/s only 0.0785, so the
  genome binds again exactly as the brief predicted. Shares 0.8 → 0.4 and 0.9 → 0.5.
- **`motor_foundation.rs`**: during a Strike the apex envelope is the profile's strike speed — a
  *paid* burst, as the escaping-prey fixture already does for the escape multiple. R0b's "10×
  unreachable" guard is retired (the 3.93 px/s addend it removed is no longer an order of
  magnitude above the 4.20 px/s cruise, so the claim is arithmetically unavailable; the per-tick
  envelope assertion carries it). For the prey, a tick whose published path is split or that ends
  on a different face is excluded as unpaid seam transport: `crossed_a_seam` only saw paths
  straddling two faces, and at 1.0 BL/s a body lands its whole tick on the far side, reading as
  47 px/s.

## 4. Mobile grazing at the new pace

Same fixture, four arms, 36,000 ticks (1,800 s), births off, leave rule `P < 0.20`.

| arm | s | eaten_P (m) | grown (m) | upkeep (e) | travel (e) | rotate (e) | px | cells | turns | survival |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| gated-still | 460 | 0.4035 | 4.1338 | 2.8486 | 0 | 0 | 0 | 0 | 0 | died 9190 |
| open-still | 1800 | 0.4237 | 15.8805 | 11.1600 | 0 | 0 | 0 | 0 | 0 | broke 9278 |
| mobile (R0b) | 1800 | 5.6531 | 7.0475 | 11.1600 | 0.94777 | 0.29588 | 157.96 | 52 | 6.28 | broke 28199 |
| **mobile (R0d)** | 1800 | **7.5633** | 9.6902 | 11.1600 | 0.93533 | 0.23641 | **2598.13** | **670** | **83.61** | **funded** |
| no-intake | 369 | 0 | 3.4793 | 2.2887 | 0.00911 | 0.00197 | 25.31 | 7 | 0.70 | died 7384 |

The grazer covers **2,598 px instead of 158 for less motor energy** (1.172 e against 1.244 e), makes
670 cell transitions instead of 52, and is the first arm to finish still funded rather than broke.
Motion is 9.5% of the whole bill against R0b's 10% — the point of holding the per-second price. It
still strips the patch (3.389 → 0.584 m against 9.69 m of gross renewal) while the never-visited
centre cell recovers (0.391 → 0.544 m), so the cropping-floor question R0b raised is untouched.

## 5. Native demo

Old runner (pid 1646454, `state/`) stopped. A **fresh** world on a scratch directory:
`target/release/cubarium run --art assets/atelier --state state-pace-demo --sink shim --mirror-web
--web-port 7393 --fps 60 --speed 1 --care --fresh`, pid 1728299, build `0.1.0+5949002`. `/status`:
`"resumed_from": null`, `"start_tick": 0`, `"state_dir": ".../state-pace-demo"`, `"sink": "shim"`.
Left running; the shim (pid 1446219) untouched. Seven `/frame` pulls 5 s apart over 30 s, decoded
(5 × 64 × 64 RGB): bodies **visibly travel and turn** — the neutral-bright fauna pixels on the Top
face drift their centroid several px per sample (x 33.9 → 26.1 → 19.5 over 15 s) and change shape
as they re-orient. A body crosses a 4 px cell in under a second instead of 13 s; a half turn takes
2 s instead of 26.

## 6. Left undone

No ecology rebalance beyond the one `move_cost` scaling. No profile version bump — `validate` did
not force one. The pursuit fix in §2 is the minimum that makes a strike land; the legacy stalking
deadlock R0b named, the starvation-predicate defect and the cropping floor are all still open.
