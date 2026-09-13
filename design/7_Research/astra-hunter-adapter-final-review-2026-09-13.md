---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Delivered hunter adapter: bounded independent review

Reviewed minification `5b24ac4`, adapter `f4420fc`, progress `a6a3b89` and the
current corresponding sources. No production/existing-test/live-process edits.
Fable's selected Lanternjaw body remains the visual direction. This is not a
live-introduction or ecological signoff.

## Minification findings closed; documentation qualifications remain

All four [saved minification fixtures](assets/astra-minification-regressions-2026-09-13.rs)
now pass. The automatic query retains the previously clipped 0.2-scale corner;
the destination-box nonoverlap case is now zero; scale `1−1e-9` matches the adult
center at float precision. The owned `multipart_scale` target passes all 16 tests.

My additional public-API sweep crosses every reciprocal grid boundary 1 through 8
(one-sided at the admitted minimum), with rotated, fractional positioning; it
passes. Source inspection confirms chart-axis sample offsets, continuous adjacent
grid blending, radial reach `1/sqrt(2)` and early rejection below scale 1/8. The
existing `RIG_MARGIN` plus that reach conservatively covers true bilinear support.
The old pathological arbitrarily tiny-scale loops are no longer reachable through
the stamp API.

Three doc claims should be corrected without undoing the useful implementation:

- Two neighboring grids can cost `7²+8² = 113` complete body samples/pixel,
  not the stated maximum 64. Work is nevertheless bounded; the art's minimum
  0.2 limits its neighboring-grid case to 4²+5² = 41 samples/pixel.
- Finite quadrature is not generally an exact box integral even at integer
  reciprocal scale, especially for rotated bilinear fields. "Approximate" applies
  there too; the fixed sampler is a valid continuous reconstruction choice.
- The lingering `scale*(r+1)+0.5` painted-center bound is obsolete. The saved
  centered-texel case has `r=0`, scale 0.2 and paints distance ≈0.8457, beyond
  that claimed 0.7. State the actual conservative support bound instead.

## Blocking host/capability gaps before adapter readiness

**Hunter events are never drained by the owning runner.** Its `advance` closure
calls `drain_events()` for ordinary life records, but has no `drain_hunter_events()`
call. Core appends Attempt/Capture/Reproduction/Death records to a retained vector;
the latter method is the `mem::take` that clears it. A running hunter world therefore
retains its entire event history, in headless mode too. Drain once every completed
tick whether or not logging/rendering is enabled, passing the same owned batch to
any observers that need it. The experiment harness already drains correctly.

**Unsupported scale is substituted after mutation, not rejected beforehand.**
`world.step()` precedes `Show::observe_hunters`; unsupported members are then
removed from the Lanternjaw map, logged once and rendered with their ordinary
atelier rig. The delivered test explicitly asserts this fallback. That is not the
requested capability rejection before mutating a running world. Add preflight of
the saved/admitted profile before starting the visual run or admitting a trial,
including the scale range descendants can reach, not merely today's adult scale.
Do not change the authoritative physical scale or silently substitute a different
body to evade the mismatch.

**Scale-only checking also admits incompatible contact geometry.** A valid core
profile with `capture_offset_body = (0,0)` still has scale 1 and passes the adapter.
It is drawn as a Lanternjaw whose named near claw is `(13.279411764705882,1.1)`.
The new isolated fixture creates this real admitted World and fails the core/art
effector equality assertion. Capability validation must check the role's geometry
contract and profile-supported range, not only `hunter_scale_supported(body_scale)`.
The current same-geometry default trial is sound; arbitrary admitted profiles are
not automatically drawable by this fixed body.

## Concrete observation/cache regressions

Saved in [astra-hunter-adapter-regressions-2026-09-13.rs](assets/astra-hunter-adapter-regressions-2026-09-13.rs):

- Reobserving the same settlement tick calls `HunterMemory::observe` again,
  overwriting `prev` with the current Handling frame. At the SAME fractional time
  0.5, the returned pose changes from late Strike to Handling. Retained prey is
  also cleared by this repeated path. Preserve previous-tick memory on same-tick
  observations, as the established plant presenter already does.
- Rewinding `ArtPresenter` resets plant/body memory but leaves the hunter map.
  A reused presenter restored from later Handling into earlier Windup keeps an
  extended entry reach; a fresh presenter reconstructs folded reach. Clear/rebuild
  hunter state, retained targets and warning state on a rewind/replaced-world reset.

These are synthetic observer-frame fixtures isolating public presentation behavior,
not manufactured ecological capture evidence. Both fail against the delivered
adapter. Full generation-bearing IDs and removal-based pruning otherwise look
correct; stale-generation and dropped-membership tests pass. The cache remains
bounded by listed live membership, apart from the separate runner event leak.

## What the boundary adapter does and does not establish

The default staged certain-capture test correctly keeps the previous phase through
the interval before its post-movement settlement boundary, then enters recoil.
Pre-step Windup/Strike timing is used from the interval's start. Attack elapsed time
comes from persisted boundaries, not a synthetic repeating hunt. A certain miss
does not create a meal/cocoon or restart the strike. Gut/escrow values come from
actual core state, and normal root positioning uses the existing interpolated
movement path/turn followed by one root-owned multipart query.

The retained prey is an authentic **last published view**, not its actual capture
position: the adapter consumes no Capture evidence and has no capture-tick prey
path. It stamps that old view at its endpoint, with current animation time and no
previous body-fade memory. The frozen-prey fixture cannot detect missing final
movement, seam-crossing or a final mode fade. Keep this explicitly approximate;
use the real settlement record if claiming exact prey-to-claw correspondence.
Likewise `cur.gut`, `cur.gestation` and `cur.scale` are used even while the prior
attack phase is being interpolated. There is no test of newly acquired gut before
the capture boundary or of a juvenile whose physiology changes scale after the
capture test; compare actual event geometry in that case before claiming exact
all-boundary effector identity.

Restart reconstruction is conservative, not frame-identical: it lacks retained
prey and observed partial entry reach. The known far-claw lag difference during
the first 200 ms recoil remains documented; the test only establishes near-reach/
hush convergence after recoil, not whole-image equality through it. The repeated-
draw test proves same-instant purity/nonmutation, not distinct 30/60/120 Hz
end-to-end runs or a care-held runner integration scenario.

The adapter seam test establishes an extension onto the neighboring Right face.
It does not exercise moving root passage through all seam orientations, a top
vertex, or open-rim capture/retained prey. Existing multipart geometry tests provide
lower-layer evidence but do not fill these adapter-level scenario gaps.

## Visual and verification evidence

Viewed `captures/lanternjaw/world-attack.png`: the chosen body is coherent and the
meal transition is visible, but the nearby bright stalk/canopy makes the limbs
hard to separate at native 64 px. This is a staged frozen-prey sheet, not hardware
legibility or moving-prey settlement validation. Preserve the silhouette; add an
unobstructed native view before tuning subtle limb details from this crowded one.

- Original isolated minification fixtures: **4 passed**.
- Owned multipart-scale target: **16 passed**.
- Owned hunter-present target: **13 passed, 2 ignored**. Three passing tests are
  pure adapter constructions; ten use staged Worlds. Neither ignored capture nor
  timing function is a passing behavioral test.
- New isolated adapter fixture package `/tmp/cubarium-astra-adapter-RhmpbV`:
  **1 passed (full grid sweep), 3 failed** (duplicate observation, rewind,
  admitted custom geometry). Reproduce with `cargo test --offline --manifest-path
  /tmp/cubarium-astra-adapter-RhmpbV/Cargo.toml -- --nocapture`.
- Read `/tmp/lw-full2.log`: 39 successful summaries total **576 passed, 0 failed,
  13 ignored**, matching Fable's limited `--lib --tests` count.
- `cargo check -p cubarium --examples` succeeds now, with only unrelated
  reproduction-observer dead-code warnings. The progress document's asserted
  exhaustive-HunterEvent example blocker is stale, not a present compile failure.

Do not call the complete adapter ready until the event-drain and capability policy
gaps are resolved. Retain the repaired minification and real-world phase/art work;
the issues above do not justify replacing the selected Fable body or reverting
whole-rig scaling.
