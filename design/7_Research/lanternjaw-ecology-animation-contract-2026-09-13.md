---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw: real hunting phases and visible contact

Astra source review and integration proposal. Wrysk selected Fable's Lanternjaw,
not a blend with the alternate body. Preserve that silhouette. This document
does not select ecological tuning or authorize live introduction. The core trial
and Fable's art package are still in progress; no production files were changed
for this review. `HunterPhase` / `HunterView` had not yet landed in core when
inspected, so the mapping below names semantic inputs, not an existing ABI.

## Immediate finding: six pixels is not the visible jaw

Current `lanternjaw.rs` adds `(0.5,0.5)` to study coordinates when splatting.
The study's `(8,0)` jaw pixel is therefore centered at body `(8.5,0.5)`, not
`(8,0)`. Body coordinates below are relative to the organism/root anchor,
with +x forward and +y to its clockwise side.

The following values are calculated from the actual `ROWS`, `limb_pose`,
`hunt_state`, column transforms and `CELL_CENTRE` in the current source:

| Pose | Front jaw x / vertical midpoint | Near claw center | Far claw center |
| --- | --- | --- | --- |
| Rest, study t=0 | `8.5 / 0.067941` | `(4.8,2.792778)` | `(4.8,1.792778)` |
| Fully cocked, t=3.22 | `8.5 / -0.037377` | `(4.5,1.270661)` | delayed toward cocked |
| Full strike, t=3.34 | `9.6 / -0.042139` | `(13.279412,1.162368)` | `(12.709881,0.297631)` |
| Recoil complete, t=3.54 | `8.5 / -0.048567` | `(4.8,3.046432)` | still finishing its delay |

There are additional lower/inboard jaw texels around x=7.5 at rest and x=8.444853
at full extension. The outer jaw consists of two pixels, hence its vertical
midpoint above. These are semantic painted centers, not the outermost faint
filter tail; bilinear splatting and sampling spread coverage around them.

Derivation of the full near-claw x is particularly useful for the core worker:

```text
head_dx = compress*(1 - 13/17) + lunge*((13 - 9)/8)
        = -0.3*(4/17) + 1.1*0.5 = 0.479411765
near_claw_x = 12.3 + head_dx + 0.5 = 13.279411765
front_jaw_x = 8 + lunge + 0.5 = 9.6
```

A trial contact disk at `(6,0)` with radius 1.5 is behind both the jaw and the
extended capture claws. It is not visually confirmed. Changing it is also NOT
just cosmetic tuning: the candidate eight-pixel root-centered sense range may
fail to acquire or retain prey near a thirteen-pixel capture effector. Audit
search radius, pursuit stopping distance, windup admission and end-of-strike
contact together; do not silently increase sensory capability in the presenter.

## Distinguish capture from ingestion

Recommended model for preserving the chosen art:

- **Ingestion mouth:** approximately `(8.5,0)` before lunge, `(9.6,0)` during full
  extension. It belongs to the head, and receives food after capture. The small
  decorative vertical wave is not a reason to jitter collision geometry.
- **Capture effectors:** the two raptorial claws. Full-extension near center is
  approximately `(13.2794,1.1)` without its small decorative wave; the far claw
  trails by 45 ms. Either use these named effectors directly, or explicitly define
  one grasp region enclosing them, approximately centered near `(13.0,0.7)` at
  the currently sampled full pose. A radius such as the proposed 1.5 remains a
  trial contact tolerance, not a value proven balanced by this art measurement.

For the first bounded core integration, a fixed full-extension capture anchor is
reasonable ONLY when capture is evaluated once at full extension. There is no
need to make the core depend on every decorative wave. Keep the art's cosmetic
tip displacement within the documented contact tolerance and verify it over all
idle phases. If captures can occur during extension instead, core and art must
share the extension/progress-dependent effector trajectory; a static full-reach
disk during windup would capture visibly untouched prey.

If the core keeps a field named `jaw_forward`, document whether it actually means
capture-claw reach. Prefer a two-component body-local offset (forward and side)
or an explicitly defined grasp region over silently dropping the visible lateral
offset. Do not shorten Fable's arms to make the unconfirmed six-pixel placeholder
look correct. Do not count tail, glow or filter coverage as a mouth.

## Real phases, not the six-second demonstration loop

The current API has `Mode::{Rest,Move,Hunt,Bud}` and `parts(seconds,mode)`.
`Hunt` repeats a complete attack every six seconds; `Bud` always paints a cocoon.
Those remain useful gallery modes but are insufficient for real ecology.

Factor the existing pose construction into explicit channels or a semantic living
pose entry point. Keep the study wrapper as a producer of those channels, so its
palette/silhouette tests stay valid. Independent inputs should include movement
blend, near/far reach, compression/lunge, bounded accent, and optional actual
gestation/gut signals. Ambient wave/pulse/blink reads simulated presentation time;
attack channels read the authoritative attack episode and progress, not modulo
time. Do not freeze every ambient rhythm by feeding a synthetic hunt timestamp
to the whole body.

| Real phase | Visible behavior | What must not be inferred |
| --- | --- | --- |
| Perched | Rest wave, folded arms, quiet lanterns; planted legs when still | No periodic strike or food/cocoon |
| Stalking | Restrained move wave and gait driven by actual movement; arms remain folded | A valid target does not imply a kill |
| Windup | Fold → cock, hull compression and a small chain charge over actual phase progress | No extension/contact/consumption yet |
| Strike | Paid approach if the phase includes it, then one fast extension, reaching full contact pose at settlement | Accent signals an attempt, not success |
| Recovering | Recoil from the actual last reach, then folded stillness for the remaining recovery | No repeat strike while waiting |
| Handling | Initial successful-capture recoil, then quiet closed-jaw/abdomen motion only while actual gut material remains | No recreated prey sprite or fictitious meal from the phase name alone |

Timing recommendation for the candidate core schedule (0.6s windup, 1s Strike,
single contact check at Strike end): stretch the study's 120-ms coil over the real
windup, hold the cocked pose during the early paid approach, and spend the FINAL
approximately 120 ms of Strike on its cubic extension. Keep full extension through
the actual settlement boundary; start the approximately 200-ms recoil only after
entering Recovering/Handling. Otherwise playing the study's complete 440-ms
gesture at Strike start makes the claws retract roughly half a second BEFORE
the core can capture anything. If the core selects a shorter strike duration,
fit the extension to it explicitly; do not change core duration from the renderer.

The far claw's lag is 45 ms of simulated attack time, not a frame count and not
a modulo loop that can replay a previous attack. Clamp before the current episode
begins. Preserve the current head attachment transform for both arms. A failed,
unaffordable or cancelled attempt must not receive a successful-handling flourish.
If a phase is interrupted, recoil from its displayed reach rather than snapping
to an unrelated study keyframe.

## Minimum observer/adapter data

Use full generation-bearing hunter IDs from the explicit core membership view;
never discover hunters by `genome.form` or append a default atelier form. Join
against live organism IDs and skip stale entries.

Needed semantic information: actual phase, elapsed/duration or normalized progress
with phase-entry tick, attack episode identity, current root movement/heading,
physical size/scale, validated target if any, actual gut material/capacity, and
optional escrow progress. Expose capture point/region separately from ingestion
mouth when they differ. A renderer-side state cache may smooth transitions but
must not invent biological outcomes or reset a long phase on restart.

Use the same `present_seconds(tick,f)` and root movement interpolation as the
ordinary presenter. Core outcomes belong to tick boundaries, not the first draw
that happens to notice them. To keep prey disappearance and the grasp consistent
under the presenter's one-tick interpolation interval, retain the previous phase
and settlement boundary when transitioning into Handling/Recovering. Do not add
interpolated lunge to core movement twice: actual translation is the root path;
the small authored head/arm lunge is a local pose offset.

## Juveniles and escrow

The current `Lanternjaw::draw` and `stamp_rig` path has no scale argument. Do not
claim juvenile rendering is already supported or borrow the ordinary creature's
binary juvenile scale without checking core physical scale. Choose one
authoritative size mapping and apply it to the WHOLE rig, capture offsets and
contact tolerances consistently. A possible area-preserving candidate is
`sqrt(actual_structure/adult_structure)`, but the core profile must own the
selected mapping; this note does not select it.

Renderer support can use inverse body coordinates divided by scale and a query
radius multiplied by that same scale, preserving every part's relative position
and shared root ownership. Scale offsets, pivots' effective placement, the
half-pixel lattice convention and local lunge together. Scaling only sprite
images leaves a juvenile's head/arms detached at adult distances. Validate
juvenile contact and anatomy at the smallest admitted size and during maturation.

A cocoon appears only for actual funded escrow. Use its real normalized gestation
to reveal the existing under-tail cocoon gradually; pulse may remain subtle but
must not restart gestation. No escrow means no cocoon, even during satiety,
Handling or recovery. Birth may show only the real new offspring ID; never spawn
a second decorative organism. On birth/miscarriage/cap refusal, remove or briefly
fade the existing cocoon from its current appearance without implying a birth
that did not occur. Keep ordinary ecological settlement and paid escrow in core.

## Seam/rim contact must agree with the root-owned artwork

For an actual target, unfold the prey position from the hunter ROOT using the
same shortest-image ownership as the rig, rotate into the same body basis, then
compare with the scaled capture effector/region. This preserves contact through
ordinary side/top seams without inventing new face mappings. A target-radius
allowance is an explicit ecological choice, not the prey sprite's glow footprint.

Merely computing `travel(root, effector_offset).end` and using that point's own
nearest-distance chart can disagree with the renderer at a top vertex: independently
owned charts were the reason the production rig switched to one root query.
If `HunterView` needs a physical capture anchor, expose it as optional and validate
the root-to-anchor unfolding against the intended body coordinate. A mapped point
whose shortest root image is different is not proof of a visible grasp there.

Never reflect an off-rim mouth/claw offset back onto the world. Static artwork
clips at the open bottom; only actual root travel reflects. The bounded safe
first policy is no capture when the effective grasp center is off-surface or
cannot be mapped consistently, while still charging the attempted strike. If
partial off-rim grasp regions are later allowed, validate their surviving visible
contact region explicitly. At vertices retain the renderer's localized cut, not
a promise that a planar rig can wrap curvature without one.

## Required integration evidence

- Debug-only contact overlays/captures on an isolated world: ingestion point,
  capture region and named claw centers at rest, full coil, release, settlement,
  failed recoil and handling. No markers in normal output.
- Flat +x and diagonals: at settlement the contact region overlaps the visible
  claw, not the thorax. Separate tests for a target near `(6,0)` and one near the
  actual full-extension effector catch the placeholder regression. Audit sensing
  and stopping distance rather than increasing it invisibly in art code.
- Ordinary edges, each top vertex and open rims: compare target/contact geometry
  with the root-owned rendered grasp; no reflected off-rim kills, stale target,
  or conflicting vertex image. Include juvenile sizes and fractional roots.
- Hold each real phase for longer than six seconds: no synthetic attacks. Check
  attack-disabled, failed, aborted, stale-prey and success paths. Success adds
  Handling only from real settlement; gut-zero cannot pretend to chew a meal.
- No escrow versus funded escrow, miscarriage and one actual offspring. Visual
  cocoon state must follow escrow, not a phase loop or reserve threshold.
- 30/60/120-Hz common-instant identity, pause and tick-zero hold, restart during
  windup/strike/handling/gestation, interruption continuity and no RNG/state-hash
  changes from drawing. Native 64px capture and full-scene frame budget remain
  required; desktop evidence is not physical-cube validation.

Sources: current [production art](../../crates/cubarium/src/lanternjaw.rs),
[production geometry plan](astra-lanternjaw-production-plan-2026-09-13.md),
[core work order](fixed-hunter-core-handoff-2026-09-13.md), and
[fixed-hunter proposal](astra-fixed-hunter-implementation-plan-2026-09-13.md).
Lore retrieval was checked against these files. The numeric table was recomputed
from the source template and pose equations; it is not a claim that collision
overlays or an ecological capture run have already been implemented or validated.
