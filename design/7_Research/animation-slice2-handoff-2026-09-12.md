---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Animation slice 2: wind and articulation handoff

Wrysk liked the first pass and explicitly authorized continued iteration while away,
including remaining steps in the animation plan. Preserve the look they approved.
This work order is not canon and does not authorize ecology or transport changes.

## Checkpoint

Before this pass root archived tracked and nonignored untracked project source
(excluding user-owned `.vscode`) plus the working release binary at
`/tmp/cubarium-slice1-checkpoint-QYzhsp/`.
`source.tar.gz` SHA256: `30346360e860a2e7b9a5a4ee6c77cbe16cc12f24ab34d072c540596d88a1d077`.
`cubarium` SHA256: `365598d1250f9db7f10b0265dd6d98b6ea6e60d72a747c9ee14d8879fdde1aad`.
Both files are also copied into the gitignored `captures/checkpoints/animation-slice1/`
directory for recovery after `/tmp` cleanup. Extract the source archive into a separate
directory for comparison; do not blindly unpack it over later work.
Do not restore over the worktree or make commits without root coordination. Original
worlds, shim, original runners, and the slice1 preview at port 7396 are out of bounds.
Root owns checkpointing and preview/capture comparison setup.

The checkpoint's old art pack is also unpacked for matched comparisons at
`/tmp/cubarium-slice1-art-npy1F0/assets/atelier`; use the saved executable with that
pack, not the subsequently edited working-tree assets.
Root prepared the new preview's separate state at
`/tmp/cubarium-animation2-preview-ttVUwn` from tick382800, and reserves port7397 for
the verified slice2 build. Do not launch or stop it from a worker; port7396 remains the
approved first-pass preview.

## Ownership and scope

Astra is writing `astra-animation-slice2-2026-09-12.md` now: obtain and incorporate its
direction before implementing the motion math. Fable 5.1 orchestrates sprite/animation
implementation and may use Opus 5 high/medium workers. Prefer a small number of bounded
tasks and await every worker through completion before ending the print session.
Last session's unawaited background workers were killed at parent exit: do not repeat.

Implement a reviewable second slice: coherent gentle wind with species response and
anchored/joined plants, plus a bounded authored articulation improvement if feasible.
Prioritize 64px silhouettes, readable calm intervals, and retaining the approved palette.
Do not turn everything into synchronized jelly. Read source rigs; dense bake sampling
alone did not add tall-plant movement. Authored opening/growth or easing on selected
species is valuable, but do not expand into an engine migration or giant new exporter.
Coverage AA is optional only if a compared capture demonstrates better readability.
Droplets, nibble scars, LCD migration, persistent plant biology, and fauna ecology remain
later ideas. Update the backlog with what actually lands and what remains.

## Invariants

- Read `AGENTS.md`, canon README and ledger before design. Prior docs with future-dated
  filenames are existing context; this pass uses the actual session date 2026-09-12.
- The current working tree IS the approved checkpoint, including untracked tests/docs;
  do not reset it. Preserve `.vscode/`. Use apply_patch for local edits.
- Keep 20Hz ecology, 60fps render interpolation, deterministic simulated presentation
  time, draw purity, pause/rewind behavior, resource-gated fruit and no snapshot changes.
- Reuse current shim/surface geometry. Do not hand-roll independent face wrapping or
  enlarge the nine-pixel per-stamp footprint cap. Roots cannot skate; tall trunks, vines
  and crowns must share one continuous deformation with no seam gaps/double opacity.
- Preserve existing single-source-over premultiplied pose/layer blending and no-art mode.
- Source scenes/SVGs are the art source of truth; one-time author scripts must not
  overwrite hand edits. Any new atlas format must keep old packs supported.
- Keep analytical UI out of normal display. Test/capture tools may expose controls.
- Every write stays in this repo or /tmp. Do not use network/Claude credentials in logs.

## Verification and delivery

Baseline full host/render all-targets tests pass; release dense dry draw ~9.86ms,
dense wet/rainy/all-columns/200 bodies ~10.453ms on this machine (16.7ms target budget).
Short matched 4s60fps capture had 2/236 large pixel-jump events (>0.5 sRGB channel), not
a guarantee of no real state changes. Release has an existing unused AUDIT_TOLERANCE
warning in core; do not claim zero warnings globally.

Add independent regressions for zero-wind identity, roots, column continuity, seams,
wind temporal/spatial continuity, deterministic render-rate-independent behavior,
pause, and any new art contract. Keep existing checks meaningful; changed appearance
expectations may adapt but do not delete or weaken invariant tests. Re-run host/render
all-targets and release worst-case draw benchmark. If new footprint inflation or render
cost threatens 60fps, tune or simplify before handoff.

Make a deterministic sparse 64px wind/articulation capture and a matched live-snapshot
capture using `/tmp/cap-state-base/world-382800.cubw` (read-only seed; copies only).
Inspect actual images and continuous motion evidence, not only tests. Record commands,
numbers, and limitations in `animation-slice2-2026-09-12.md`; use honest uncertainty for
physical cube behavior. Provide final changed-file list, tests, and remaining work.
No commits and no accepted decision-ledger changes.

## Integrator/Astra review while workers implement

Before final delivery, reread Astra's updated slice2 plan and its corrections to the
Fable brief. These are acceptance issues, not optional new features:

- A texel-center-only bend bound plus unchanged filter support is not conservative:
  displacement varies across each texel's bilinear footprint. Use a proven conservative
  bound (including warped support corners or a derivative allowance) before capping the
  unfolded radius at 9. `min(9, extent + amplitude)` alone does not prove no clipping.
- Radial masks must use inverse-warped material x as well as y. The rows-unchanged
  argument only applies to axial/strip masks.
- A stable family amplitude ceiling must include slot variation and actual wind peak;
  ensure wind_strength really stays in [0,1]. Fractional cap tile indices count when
  finding a column's maximum bend height/support, not only integer trunk positions.
- Flutter applied only during the hold introduces jumps at rise/hold/fall boundaries.
  Multiply one continuous active-interval envelope by continuous modulation instead.

Astra will supply regression tests after public APIs land. Root will independently
rerun acceptance checks and compare captures before launching the new preview.

The concrete renderer target is now `cargo test -p cubarium-render --test
astra_wind_regressions -- --nocapture`: identity/root rows pass, but material-space
radial-mask translation and admitted-headroom filter support fail on the initial
implementation. These independent regressions are acceptance gates; fix production,
not the oracle. The support fixture has a valid 1x1 sprite pivot (.5,8.2), admitted
amplitude about 2.39932 and a source-filter point with 4% coverage displaced outside
the nine-pixel radius. See the assertions and Astra's plan for the derivation.

Further integration checks as host APIs land: `plant_bend_budget` must include
`Plant.transitions` now that v5 exists. Also reconcile `PLANT_BEND_ROOT = 1` with the
actual lanternstalk's lowest painted row 14 (center14.5 => height1.5): a root threshold
of1 does not hold that painted contact exactly still. Test shipped root contact rather
than only a synthetic last-row15 texel; choose the still-root threshold from the art.

Astra has now saved host regressions in `crates/cubarium/tests/astra_wind_regressions.rs`:
four pass (gust range/calm, temporal joins, seam transport, final varied amplitude) and
two expose the transition-budget and shipped-row14-contact issues above. The renderer
target also has a fifth test for the remaining square-corner bilinear support gap after
the Htexel+1 fix. Its source sample is inside the original sprite extent, so fixing
admission does not require changing the old zero-wind renderer. Run both targets before
claiming the wind acceptance gates pass.

Root added `crates/cubarium/tests/animation_load.rs`, two ignored release timing studies
for crowded transitions (200 bodies, rain, dry all-field growth including the authored
pilot; wet decline from a mature world). They draw 60fps over12 simulated seconds and
report mean and worst one-second mean. Run alone with `--test-threads=1`; do not include
them in a concurrent benchmark batch. This fills a gap in the mature-world timing fixtures.
Provisional root run after wind wiring, before C2 playback: dry growth mean11.988ms,
worst one-second mean14.363ms; wet decline mean10.244ms, worst10.963ms. Both passed.
Rerun after C2 is integrated before treating these as final slice2 timings.

Root independently ran a fresh Godot4.7.2 bake into
`/tmp/cubarium-slice2-bake-check-6gLhbr/art`; `cmp` matched all five shipped atlas PNGs
and `pack.json` exactly. The actual source scene/baker therefore reproduces the shipped
v5 pack, independently of C1's Python design prototype and its earlier two-bake check.

Root reran the transition timing study after C2 playback landed: dry growth mean
11.385 ms/frame, worst one-second mean 12.972 ms; wet decline mean 10.101 ms, worst
10.763 ms. Both passed. These supersede the provisional pre-C2 timings above, subject
to a final rerun if subsequent production fixes change the drawing path.

Root corrected a small bake-name validation edge case after C1: check each of the two
stage characters as a digit, not only the whole suffix with `is_valid_int()`. The latter
accepts signed strings like `+1`, which would silently become a `0 -> 1` transition.
Valid `grow01` output is unchanged; no atlas content or runtime behavior is altered.

The integrated 36-second review capture is at `/tmp/cubarium-wind-review-Ta95xn`
(1,080 native net frames, every second 60fps draw), encoded by root as
`captures/animation-review/slice2/wind-and-growth.mp4` (30fps, nearest-neighbour4x).
The pilot's blue square behind its early sprout is the synthetic isolated producer field,
not a detached bulb in the authored clip; label that distinction in capture notes.

Root's integrated `cargo test -p cubarium -p cubarium-render --all-targets --quiet`
passed (including C2 unit tests, Astra's eleven wind regressions and the initial seven
Package B renderer tests). Package B's remaining tests were still in progress. Root
built the release executable and started the separate preview at
`http://127.0.0.1:7397/`, session9650, using the reserved copied state. The approved
slice1 preview on7396 and the original runners are unchanged. If later production fixes
land, coordinate with root so only this second-pass preview is refreshed.

Root verified the new preview's frame endpoint: 59.82 submitted frames/second over
a three-second sample, all payload lengths valid; `/note` reports 1× time. This is
submission evidence, not browser or hardware frame pacing. Root updated README's
status paragraph for slice 2; Fable still owns the final report and roadmap update.

Final report accuracy notes: call strength 0.71 an active gust, not the mathematical
peak. Do not attribute the slice-1/slice-2 mean-delta difference solely to black
margins without reproducing the original aggregation; the current script averages
all RGB channels and should only be compared with its own baseline. Larger future
column sway could also use recentered per-part bend bounds, not only narrower art
(Astra's deferred option); no such renderer expansion is authorized for this slice.
Root corrected the capture header: it starts at tick 601 after warm-up, not tick 1.

Validation caveat: root also tried `cargo fmt --all -- --check`; it reports extensive
format differences across the pre-existing workspace and vendored shim code, so it
is not a clean gate here. No bulk formatting was applied. Root formatted only its
new `animation_load.rs`; `git diff --check` passes.

Root ran the initial independent `art_growth_clip` file: six pass, three initial
fixture assertions fail. Review before treating as production defects: the entry
loop can overshoot its requested three samples within an inner three-frame tick;
authored and fallback pictures may coincide at an endpoint (require difference
somewhere in the step, not at every frame); a fixed root location does not promise
identical RGB throughout a cross-fade between stages with different opacities.
Root has not edited the reviewer's file or relaxed any gate.

The reviewer corrected those three fixture assumptions. Root independently reran
`cargo test -p cubarium --test art_growth_clip --quiet`: all nine pass, warning-free.
The corrected root test requires an unchanged painted footprint and exact equality
to the unbent root at each progress value while proving the upper stem really bends.
Root is now running the final full-workspace all-targets suite.

Final gate: `cargo test --workspace --all-targets --quiet` completed with exit 0,
including all nine growth tests, eighteen wind tests, seven renderer bend tests,
both Astra regression sets, and the core/surface/oracle/transport suites. Explicit
capture and timing fixtures remain ignored by this ordinary correctness command;
their separate successful runs are recorded above. No production fixes were needed
after the second-pass preview was launched.
