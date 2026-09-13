---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Fable animation implementation handoff

Wrysk authorized beginning smoother motion and readable plant growth following a
code/live-feed review, and specifically requested Astra and Fable 5.1 for planning
and sprite/animation work. Claude implementation is preferred because of available
capacity. This handoff is scoped implementation authorization, not canonical design.

Read root AGENTS.md, design/0_Canon/README.md and DECISIONS.md, README.md,
design/animation-roadmap.md, art/README.md and art/PLANTS.md. Astra is independently
writing design/7_Research/astra-animation-plan-2026-09-13.md; read it when available.

## Assignment

Own planning and implementation of a coherent initial animation pass. Implement
smooth temporal presentation and readable progressive plant/tall-plant transitions,
including source art/bake changes where they materially help. Prioritize native
64px clarity. Add gentle anchored sway if it fits cleanly; shared environmental wind
and tiny droplet/nibble detail may remain explicitly deferred. Do not stop at a plan.
Use your judgment for routine choices and record consequential tradeoffs. You may
delegate bounded work to Opus 5 high/medium, retaining the design/art decisions.

Specific evidence:

- crates/cubarium/src/art.rs Clip::at selects discrete frames without blending.
- art/bake.gd exports four plant/tall/ground frames; three-second plant clips hold
  each pose for 750 ms. art/PLANTS.md describes tall sway as brightness only.
- art_present.rs draw_with_fruit uses tick*DT for most layers, ignoring f except
  creature path interpolation and rain. Creature clip frames are also discrete.
- next_stage and next_tall traverse all qualifying thresholds in one call. These
  are biomass-driven visual slots, not individual plants with simulated ages.
- rain_marks floors positions and rain_blink_on is binary. Fruit clip selection
  is immediate. Sprite sampling is already bilinear in premultiplied linear RGBA.
- Both web feeds on 7393/7395 measured roughly 60 fps at 1×. Browser HUD fps is
  browser redraw cadence, not unique received frame rate.

## Boundaries

Preserve unrelated user .vscode/ files. Do not commit, alter canon, change ecological
rules/tick rate, reset saved worlds, stop/restart existing processes, or change the
shim. Reuse existing geometry helpers and honor the nine-pixel stamp extent budget.
All edits stay in this repository or /tmp; use apply_patch for source/document edits.
Generated atlas output through the normal bake is allowed. No generated-image
service is needed for this SVG/cutout rig work. No hardware-resolution migration.

First check the art baking tools and tests. Favor compatible pack changes where
practical. Smooth animation must preserve dark outlines and avoid broad ghosting.
Growth must stay resource-grounded, handle decline/interruptions, and avoid both
instant tall-tree appearances during an ongoing run and replaying a mature world's
entire growth after presenter initialization. Keep animation time deterministic and
respect speed/pause. A pure repeated draw should not advance state from wall time.

## Verification and handoff

Run relevant renderer/host tests and normal bake checks. Add meaningful tests for
fractional-frame changes, temporal continuity, growth pacing, interruption/restart,
and seams where changed behavior warrants them. Do not merely mirror formulas.
Review short native-resolution captures at 1×, and measure representative release
render cost against the 16.7 ms budget. Do not run ecological multi-hour sweeps for
a presentation-only change. Update art contracts/docs and record what landed,
validation results, any captures, and concrete limitations in a research note.
Report changed files and exact test/measurement results to the root orchestrator.

## Root review notes during implementation

Astra's proposal is now available at the path above. Baseline measured before
runtime edits: `cargo test --release -p cubarium --lib plant_and_body_draw_cost --
--ignored --nocapture` passed, reporting 6.855 ms/frame for 622 plants and 200
organisms (41% of the 60 fps frame budget). This is the existing synthetic draw
benchmark, not end-to-end output latency. The existing unused `AUDIT_TOLERANCE`
warning is unrelated to this work.

Astra's independent in-progress renderer review is now in
`design/7_Research/astra-animation-review-2026-09-13.md`. Read it before finalizing
the new masks/crown handling: it identifies a radial reveal startup discontinuity,
potential removal of genuine crown artwork by broad color-equality subtraction,
and the need to validate matching part durations when same-index frames are paired.

Root integration checks to make before finalizing: `present_seconds(0, f)` should
hold zero, otherwise tick 0→1 resets its clock; repeated `observe` at the same tick
must not start an additional growth stage at a just-completed boundary; growth
height/progress should ideally interpolate previous/current state using `f`, since
updating them only in `observe` still leaves those changes at 20 Hz. Distinguish any
remaining tick-stepped growth from continuously sampled sway in the final report.

Astra is now authoring the independent renderer regression file
`crates/cubarium-render/tests/astra_regressions.rs` (and only that file), covering
observable blend opacity, zero-reveal continuity, full-reveal endpoints and seams.
Preserve it and run it with the final renderer checks. Root requested this after the
first Claude parent turn exited and its unfinished background workers were stopped.

Root verification after Fable's review fixes: all 7 `astra_regressions` renderer
tests and all 4 `astra_motion_regressions` host tests pass. Root encoded the 237-frame
new ambient capture as `captures/animation-review/ambient-preview.mp4` (60 fps,
nearest-neighbor 4× enlargement, 3.95 seconds). A separate candidate web runner is
serving port 7396 at 1× and 60 fps from `/tmp/cubarium-animation-preview-My6gKf`,
initialized with a copy of baseline snapshot `world-382800.cubw`. Its process/session
is owned by root; Fable should not change it. Original runners and shim are untouched.

The preview was gracefully refreshed after turn interpolation landed and resumes
its own copied state on port 7396. Root measured 59.93 submitted fps there. The MP4
was refreshed from `/tmp/cap-new-60b`. Full `cargo test -p cubarium -p cubarium-render
--all-targets` passed before the final `art_motion`/`pose_layers` additions; root
removed the now-unused `stamp_sprite` imports in art_plants/art_water tests.

Additional release evidence from root: `cargo test --release -p cubarium --test
art_water everything_on_draw_cost -- --ignored --nocapture` passed at **10.453
ms/frame**, with all wet, raining, rich, all columns tall, and 200 organisms (63%
of the 60 fps budget). Include this wetter-scene measurement in the final record.
