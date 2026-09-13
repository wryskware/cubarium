---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw completed-tick continuity: seam-carried prey, boundary-stepped body state

Evidence for root's reconciliation, not a decision. Follows the adapter corrections
(`a559e54`, `15413b6`) and closes the two presentation approximations Astra's hardening
review (`astra-hunter-adapter-hardening-review-2026-09-13.md`, "Settlement and restart")
left open: a captured prey that changed face in its final tick was drawn at the settlement
point for the whole interval with its old chart heading, and the current tick's gut,
gestation and scale were applied before the previous phase's interpolation boundary.

Owned: `crates/cubarium/src/hunter_present.rs`, `crates/cubarium/tests/hunter_present.rs`,
this note and the capture. `art_present.rs` needed no change: its retained-prey block already
consumes `retained_prey_pose(f)` and `living_pose(tick, f, moved)` generically. No core,
runner, example or README edit; nothing live, no state, no deployment.

## What changed (renderer-side only)

**1. Retained prey across a seam.** `HunterMemory::retained_prey_pose(f)` now walks the
shortest valid surface chord from the prey's last published position `p` to the `Capture`
event's settlement position `q`, using the surface's own helpers and nothing new:

- `u = unfold(p, q, MAX_LOCAL_RADIUS)` gives `q`'s image in `p`'s chart;
  `d = u.local − p.chart()`.
- The pose at `f` is `t = travel(p, f · d)`: `t.end` on whichever chart the point falls
  (canonical, seam-transported exactly as a moving body is), heading `t.map.apply(p.heading)`
  (the last published heading carried through the seams the chord crosses).
- Endpoints are exact and unchanged: `f = 0` is `p` with `p.heading`; `f = 1` is `q` itself.
- A valid unfolding never crosses the open rim, so the sweep neither reflects nor falls
  back; both are still checked and, like a missing unfolding, fall back to the previous
  behaviour (settlement point for the whole interval, old heading). The same-face case
  reduces to the straight chart chord it was.

**2. Body state across the interval.** New `HunterMemory::state_at(tick, f) → (gut, cocoon,
scale)`, consumed by `living_pose`:

- scale interpolates linearly from the previous published frame to the current one;
- gut is a boundary fact when the phase changed at this tick's boundary (`cur.started ==
  tick && cur.key() != prev.key()`): the previous value for `f < 1`, the current at `f = 1`,
  so the meal a capture puts in the gut appears on the abdomen at the same settlement
  boundary the strike is held to, not one interval early; a meal that finishes at a boundary
  likewise stays until it. Inside one phase (digestion) it interpolates;
- gestation interpolates while both frames carry one; a start or end (`None ↔ Some`) steps
  at the boundary;
- without a previous frame the current values hold; NaN `f` reads as 0.

## Approximations that remain, explicitly

- The prey's true capture-tick path and final heading are still **not published**. The
  chord is a straight surface geodesic between two published points; the heading is the
  last published one, transported. A prey that curved, or turned before it was taken, is
  drawn straighter and facing its old way. Its animation time and mode are the last
  published ones (no fade history).
- Beyond `MAX_LOCAL_RADIUS` (32 px, never reached by a real capture: the claw is 13.28 px
  from the root at adult scale and the prey moves at most a body-length a tick) the
  documented fallback is the settlement point for the whole interval.
- Restart still lacks the retained prey and the interrupted entry reach (unchanged).
- Body-state interpolation is presentation only; the core's per-tick values are the
  authoritative ones at every boundary and are drawn exactly there.

## Verification (foreground, exit 0)

Forced fixtures, not conditionals: the retained prey's published position and the
settlement position are placed on different faces by construction.

- `a_retained_prey_crossing_a_seam_walks_one_chord_with_its_heading_transported` — three
  forced crossings on hand-built memory over a real world's prey view: Front→Right (no
  turn), Right→Top (quarter turn: Right "up" arrives as Top "−x", the surface crate's own
  Right→Top fixture), Back→Top (half turn: heading negated). Checks: exact endpoints, unit
  headings, 200 samples each moving exactly chord/200 (surface distance through `unfold`),
  exactly one face change, only the two faces, walked length = chord, heading transported
  at and after the seam.
- `a_capture_across_a_seam_carries_the_prey_over_the_seam_on_screen` — a **real** certain
  capture at Front (52, 32) heading +x whose final pre-capture prey view is forced onto
  Front (58, 27) while the world takes it on Right (its real `Capture` event). A twin
  presenter fed identical views minus that final prey view isolates the retained prey's
  light. At f ∈ {0.05, 0.3, 0.5, 0.7, 0.95}: light present, every lit pixel within 8 px of
  the chord point (measured ≈ 6 px, the creature's own stamp), the light's centre within
  2 px of it (measured ≈ 1 px), mostly on Front at f < 0.3 and mostly on Right at f > 0.7;
  identical images at f = 1 (gone at the boundary); `state_hash` unchanged by all of it.
- `a_retained_prey_on_one_face_walks_the_chart_chord_and_the_fallback_is_the_endpoint`,
  `a_retained_prey_chord_along_the_rim_is_never_reflected` (Front (3, 62.5) → Left (60,
  63.5): every sample `v < 64`, on the two faces, monotone on the chord).
- `body_state_steps_at_a_phase_boundary_and_interpolates_inside_a_phase` — pure frames:
  capture boundary (gut 0 for f < 1, 0.6 at 1; scale 0.5→0.52 interpolated), digestion
  interpolation, meal-finished boundary, escrow start/progress/end, no-prev, NaN.
- `a_real_meal_reaches_the_abdomen_at_the_settlement_boundary_not_before` — real capture:
  `prev.gut == 0`, `cur.gut > 0`, `living_pose(tick, 0.5).gut == 0`, `== cur.gut` at 1.
- `a_growing_hunter_is_drawn_at_the_interpolated_scale_and_exactly_at_the_published_ends`
  — through the public presenter with consistent `ContactGeometry` at 0.5 then 0.6: 0.55
  in the memory at f = 0.5; the f = 0.5 image differs from both constant-scale sequences;
  the f = 1 image equals the constant-0.6 sequence's, f = 0 the constant-0.5 one's.

Counts, run in an isolated `git archive HEAD` copy (`/tmp/cubarium-fable-cont-0l8noL`)
with the two owned files copied in, because the shared checkout carried another worker's
in-flight core `CareCommand.dose` edit that breaks `runner.rs` compilation until they
finish; nothing of theirs was touched:

| target | result |
| --- | --- |
| `cargo test -p cubarium --test hunter_present` | 28 passed, 0 failed, 3 ignored |
| `cargo test -p cubarium --lib --tests` (host crate, all targets) | 497 passed, 0 failed, 14 ignored |

Render (`multipart_scale`) and surface crates are untouched by this change.

## Capture

`captures/lanternjaw/seam-crossing.png` (gitignored): the forced Front/Right crossing from
the real world at native 64 px, ×6, tick 32 (strike) and the capture tick 33 at twelve
fractions, then tick 34. Frames and `sheet.py` in the fresh mktemp dir
`/tmp/hunter-seam-pGodeg` (older evidence dirs untouched); ignored test
`capture_a_seam_crossing_capture_as_native_frames` (`HUNTER_SEAM_CAPTURE_DIR`) regenerates
them. What it shows: the prey's light leaves Front (58, 27) and reaches the claws on Right
in one continuous slide over the seam, gone at tick 34 f = 0 as the recoil begins. It is a
staged certain capture with a forced last view, not evidence of natural prey movement.

## Limits of this evidence

- The face-majority and locality checks are containment checks over the isolated prey
  light; the seam-continuity proof is the sampled chord test on memory, not an image
  diff across the seam pixel by pixel.
- Twelve-fraction frames are for reading; the presenter is pure in `f` (existing
  rate-neutrality tests unchanged and green).
- Scale interpolation and boundary stepping are verified on synthetic frames and one real
  capture; a real growth-changing capture tick was not staged.
