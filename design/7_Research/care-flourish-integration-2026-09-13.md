---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Receipt-driven care flourishes

Implemented a first visual response slice, not the whole interaction backlog.
Astra authored `care_effects.rs` (`e84df49`) and a cleanup-only contrast refinement
(`3112ae1`). Root wires it into `CareRuntime` after actual durable application,
including boundary-correct journal replay. Snapshot history does not retrigger.
The effect is composed after the world presenter and before the single encode:
the cube and shared viewer receive the same pixels, without an analytical overlay.

Feed shows six small warm crumbs entering, settling and fading over2.8 simulation
seconds. Cleanup lifts a few mauve flecks over1.8seconds; opacity scales with
actual material removed, not the requested maximum. Neither is a persistent
resource depiction: the real food/litter remains in core fields. Rain adds no
extra receipt effect because its receipt schedules water, whereas existing rain
presentation is driven by actual delivery. No new RNG, ecology or snapshot state.

One root-owned surface query per event provides seam registration and open-rim
clipping. Motion and coverage are fractional, not whole-pixel stepping. History
is capped at8events with high-water deduplication; expired events do no raster
work. Held simulation time uses the same fraction as bodies. Zero input leaves
the encoded image unchanged. This layer is a brief receipt flourish drawn over
the scene, not a new physical particle/occlusion simulation.

## Verification

- Eight `care_effects` tests pass: zero-input exact bytes; malformed, rejected,
  zero and duplicate receipts; bounded history; smooth onset/end and fractional
  motion; hold/frequency independence; proportional cleanup; all faces, seams,
  top vertices and rims; real receipt without presentation changing ecology.
- Root's runner test covers successful durable acknowledgement, replay at its
  boundary, and uncertain write holding without changing world or painting a
  success. Nine runner tests pass, with one explicit capture test ignored by
  default and run separately below.
- All eight `care_replay` integration tests pass with loopback access, including
  delayed acknowledgement, crash recovery and matched scenario captures.
- `cargo test -p cubarium --lib capture_care_flourishes_on_the_authored_world
  -- --ignored --nocapture` passes. It creates13 native net pairs from the actual
  art presenter: base and flourish share the exact same world, tick and fraction.
  Rendering does not change the state hash. Real receipts apply3material/6energy
  feed at tick400 and approximately2material/4energy clean at470, Front(32,48).

Root and Astra inspected the first captures: feed was restrained but readable;
cleanup blended too closely into the busy background. The latter palette changed
from[183,117,173] to[205,163,195], with size, timing and opacity unchanged.
Root regenerated and inspected the revised native/enlarged captures. Cleanup
remains deliberately quieter than feeding; legibility on the physical cube and
across other backgrounds still needs rollout observation. Static captures do not
prove subjective motion quality, although fractional/held tests cover timing.

First captures: `/tmp/cubarium-care-flourishes-1789278050761668843/`.
Revised captures: `/tmp/cubarium-care-flourishes-1789278268475892685/`.
Isolated Chromium review screenshot: `/tmp/cubarium-care-flourish-review-v2.png`.
The temporary review page crops each actual64px Front face and enlarges it4×;
it does not alter production art. The committed ignored test regenerates images
in a new temporary directory and logs that path.

## Still open

Nearby biological responses, quiet interaction rituals, rain aftermath, persistent
resource-readable deposits, separately tunable input/ambient support, and the paid
predator ecology are not discharged by this slice. Long-run care comparisons also
show form loss that matters before predator balance approval. Lanternjaw rendering
and authored plant growth remain Fable's separate active packages. Nothing here
has been deployed to the live cube yet.
