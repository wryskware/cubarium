---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw adapter hardening: independent review

Reviewed `a559e54`, filter documentation `ffa2cf1`, and progress `15413b6` against
the saved `f92e669` findings. Actual adapter/presenter/runner/test sources match
`a559e54`; multipart source matches `ffa2cf1`. Used Lore and checked current canon
and the actual source. No production, concurrent experiment files, live process,
shim, or world state was changed. Fable's selected Lanternjaw silhouette remains
the visual direction.

**Disposition:** the prior event-buffer, capability-policy, duplicate-observation,
and rewind blockers are closed. No new concrete runtime blocker was demonstrated
in this bounded review. The documented reconstruction approximations and several
overstated test/doc claims below must not become broader guarantees.

## Prior blockers: verified closed

- Runner `advance` unconditionally calls `world.drain_hunter_events()` alongside
  the ordinary drain, before the non-headless branch. A rendered run passes that
  owned batch into `observe_hunters`; headless runs drop it. No retained lifetime
  history remains in this path; no hunter journal is claimed.
- Runner opens the presenter and validates the loaded world's profile before
  care initialization/checkpoint, sink initialization, or stepping. Fixed-role
  claw and ingestion offsets must match the named adult effectors; minimum scale
  must be in 0.2..=1 and exponent finite/nonnegative. Core clamps its structure
  ratio to 0..=1, so these conditions bound all derived scales. The ordinary-rig
  substitution and warning maps are removed; per-view mismatches return a named
  error propagated by the runner. This is presentation admission, not narrower
  core admission or permission to mutate a loaded ecological profile.
- Same-tick memory observation preserves `prev`, entry reach and retained prey.
  The public presenter likewise skips advancement of existing same-tick members.
  The preserved fractional-pose regression passes; the owned test additionally
  compares complete memory and images at four fractions.
- Earlier-tick observation clears hunter history with plant/body reset; the
  hunter-only path independently detects earlier ticks. The preserved rewind
  regression and owned fresh-versus-reused image checks pass. This does not detect
  an arbitrary different world with the same IDs and same/later tick: there is no
  world-generation token, so "world replacement" means the documented earlier-
  tick reset path, not every possible replacement of caller-owned data.

New independent profile sweep uses actual `body_scale` and `ContactGeometry::of`,
then compares them with the named art effectors for 4 minimum scales × 5 exponents
× 8 structure ratios, including zero and above-adult structure. All 160 cases pass.
This checks core/art API consistency, not an independent reimplementation of core
scaling or proof of contact during a growth-changing settlement tick.

## Settlement and restart: useful improvement, bounded claims

Matching current-tick Capture evidence is now associated with the retained full
prey ID. For same-face movement the prey's last published position interpolates
to the actual recorded post-movement `prey_pos`; no movement path is invented as
ecological state. At `f < 1` that retained body is drawn, and at `f = 1` it vanishes.
The owned moving-prey fixture verifies the endpoint, a midpoint, and local image
contribution near settlement. Its claw-distance check permits core reach **plus
0.5 px**, not exact coincidence of prey and effector or an exact unrelaxed reach
bound. Capture itself admits prey around the grasp rather than at its centre.

The approximation is explicit and still matters:

- Same-face motion is a straight chart chord, not the missing capture-tick path;
  current animation time and the last published heading/mode are used, without
  the missing final turn or body-fade history.
- If the prey changed face, the adapter uses the settlement point for the entire
  interval, including `f = 0`, while retaining the old chart heading. Therefore
  cross-face retained-prey position/heading continuity is **not** established;
  a visible snap or orientation jump remains possible. The seam fixture does not
  require the prey to cross a face during that final tick: its conditional accepts
  both same-face and changed-face histories. Treat this as the declared endpoint
  fallback, not a claim of recovered seam transport.
- Restart lacks the retained prey and precise interrupted entry reach. The known
  first-200-ms recoil/far-claw discrepancy remains. Rewind matching a fresh
  presenter is different from uninterrupted-versus-restarted image identity.
- `cur.gut`, `cur.gestation` and `cur.scale` still apply before the phase settlement
  boundary. The report acknowledges early gut appearance and untested growth-
  changing contact; there is no new all-boundary semantic or scale guarantee.

Viewed `captures/lanternjaw/world-contact.png`: the quiet background makes the
selected cyan/violet body and paired warm claws substantially easier to read than
the previous plant-obscured sheet. The capture/recoil progression is visible.
This is a staged certain-capture preview, not room-distance LED legibility,
ecological success, or evidence of the untested cross-face final movement.

## Test and documentation precision

- Both executions in the runner-drain test use `--sink none`; adding `--art`
  still leaves them headless. They exercise art preflight, not rendered runner
  observation. State hashes exclude transient event buffers, so equality alone
  cannot prove drainage; the direct unconditional source call supplies that proof.
- Vertex pixel channels ≤1 do not prove unique ownership: compositing can remain
  bounded even with duplicates. Off-Front equality at the rim does not prove no
  same-face reflection. The source delegates the actual drawing to the separately
  tested root-owned multipart renderer; qualify these adapter tests as scenario
  smoke/containment checks, not independent ownership/reflection proofs.
- Held-time repeated draws prove pure rendering at fixed time, not an end-to-end
  care-held runner test or separate 30/60/120 Hz trajectories.
- Filter docs now correctly state 113 blended-grid samples (41 at art minimum),
  approximate quadrature, and conservative radial support. One stale bullet in
  `multipart.rs` at the `stamp_rig_scaled` work bound still says at most
  `MAX_GRID²` samples; update it to agree with the corrected neighboring text.
- `hunter_scale_supported` still documents the removed `unsupported_hunters`
  fallback. The progress document's old "body, not an animal / no presenter"
  remainder and its sweeping "every suite green" wording conflict with its newer
  scoped account. These are documentation leftovers, not the actual code path.

## Exact validation

- `cargo test -p cubarium --test hunter_present`: **21 passed, 2 ignored**, exit 0.
- `cargo test -p cubarium-render --test multipart_scale`: **16 passed**, exit 0.
- New independent API-adapted fixture: **5 passed**, exit 0. Original `f92e669`
  fixture is untouched. New file:
  [`astra-hunter-adapter-hardening-regressions-2026-09-13.rs`](assets/astra-hunter-adapter-hardening-regressions-2026-09-13.rs).
  The three former failures now pass; the geometry fixture requires explicit
  rejection rather than merely permitting absence from the supported map. The
  reciprocal-grid continuity/work-bound sweep and profile-range sweep also pass.
- Read `/tmp/lw-full4.log`: 39 successful summaries, **584 passed, 0 failed,
  13 ignored**. That is recorded `--lib --tests` evidence, not a freshly rerun
  whole repository or examples suite. Root-owned experiment sources are currently
  dirty and outside this review; no present example-build claim is made here.

Reproduce the isolated fixture:

```text
cargo test --offline --manifest-path /tmp/cubarium-astra-adapter-closed-vFQ7gw/Cargo.toml --target-dir /tmp/cubarium-astra-adapter-RhmpbV/target -- --nocapture
```

Nothing here authorizes live predator introduction or signs off the separate
complete-audit ecological experiment. Keep the fixes and selected body; retain
the endpoint/restart limitations as visible follow-up work.
