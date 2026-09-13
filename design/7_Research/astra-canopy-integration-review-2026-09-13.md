---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Canopy growth clips: independent integration review

**No concrete blocker found** to root's same-schema canopy update at `3ca510d`.
The package adds authored pictures for the existing growth path, not new plant
biology, care semantics, timing or persistence. Production changes in `art.rs`
and `art_present.rs` are only tests/comments; actual loader and presenter behavior
are unchanged. No live process, state or other worker file was modified here.

Reviewed the authored canopy groups/sub-part construction, pack rows and relevant
host diff after Lore retrieval. Both species now supply 0→1 and 1→2 transitions,
24 samples over four seconds, non-looping, with the existing pack-v5 format and
tile/pivot conventions. The loader uses the existing transition and support
checks; no allowance was enlarged to admit the new art.

The fallback-test correction is valid: it now compares the shipped umbrellafrond
clip against that same plant with transitions explicitly removed. Previously it
lent a different species' clip to a plant assumed to have none. The corrected
test still checks exact unmasked playback and a visibly different radial-mask
fallback, instead of accidentally comparing two authored paths.

## Independently verified preservation

Read the actual atlas visually. Also decoded old and new PNGs to RGBA in memory
with ImageMagick, matching rows by name/stage/from/to rather than row number:
**all 34 pre-existing plant rows are pixel-identical**, and their metadata differs
only by row relocation. All four other atlases (`creatures`, `ground`, `habitat`,
`tall`) are byte-identical; pack metadata outside the plant rows is unchanged.
There are four new growth rows, taking the atlas to 38 rows / 384×608.

This independently confirms preservation without relying on the authoring
script's partition assertions. No image was edited or regenerated during review.
The initial sandbox refused Node subprocess creation; the approved read-only
comparison completed with all assertions passing.

## Foreground tests against the committed files

**64 passed, zero failed, two deliberately ignored**:

- `art_growth_clip`: 9 passed.
- `art_growth_pack`: 12 passed, one ignored capture.
- `art_plants`: 13 passed, one ignored timing test.
- `art_wind`: 18 passed.
- `art_wind_top`: 5 passed.
- Library `art::tests`: 6 passed.
- Corrected `a_radial_slot_with_a_clip_plays_it_unmasked_too`: 1 passed.

Coverage includes every transition's endpoints and silhouette envelope, canopy
centre paint/centering/reach, the existing ≤9-pixel extent and non-narrowed bend
budgets, derived 60-fps continuity, three-layer windy/calm playback, reverse
wilting, fruit exclusion and the four canopy steps straddling a top/side seam.
The seam test compares actual output with a separately assembled stamp and
requires light on both faces and bounded channels. Existing growth thresholds,
hysteresis, render-time independence and top-face spin regressions also pass.

These growth fixtures drive published producer values through `RenderView`;
they verify the presenter's response to world data, not a new ecological growth
law or a long-run population effect. Endpoint alpha planes match the neighboring
stages; the documented neutral-versus-sway centre brightness difference is handled
by existing endpoint blends, not falsely asserted to be full color equality.
This review did not rebake the Godot scenes, run the ignored capture, or establish
room-distance LED legibility. None is a new gate for this bounded integration.

One already-reported documentation follow-up remains root-owned: `art/README.md`
still describes the two canopy species as lacking clips. The actual data and
tests now agree that every shipped plant has both transitions. This is not a
runtime blocker or a reason to hold the verified update.
