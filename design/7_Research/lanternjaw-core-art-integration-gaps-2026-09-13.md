---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw core/art adapter: remaining API gaps

Astra bounded review of the current native-core work against the
[animation/contact contract](lanternjaw-ecology-animation-contract-2026-09-13.md).
Core and Fable art are actively edited. The
[progress report](fixed-hunter-core-progress-2026-09-13.md) explicitly marks tick
behavior unfinished; no capture-behavior acceptance is claimed here. Preserve
Fable's selected silhouette and the useful existing member/profile API.

## Already available: do not invent another phase clock

`HunterView` currently publishes only `phase_progress: Option<f32>`, but
`World::hunters().member(full_id)` already exposes persisted
`phase_started_tick`, `phase_ends_tick` and `attack_counter`. Root can use these
NOW, joined by the same generation-bearing ID. The renderer need not guess
entry time from when it first sees a view, and no persisted schema addition is
required to expose this existing information.

Minimal observer improvement: copy those three fields into `HunterView` (or a
separate transient presentation record). Keep the current phase progress as a
convenience, not the source for fractional playback. Compute progress from the
actual entry/end boundary and `present_seconds`; untimed phases still need entry
time for one-time settling/recoil. Document which completed-tick boundary a
phase's start/end refers to when the behavior pass lands.

`attack_counter` already has a purpose as the persisted paid-attempt RNG counter.
Specify whether an emitted event carries the value used for that attempt before
or after increment. Reuse it with the full hunter ID as the paid attempt key;
do not create a second unpersisted serial in the host. Windup is not yet a paid
attempt: its animation identity can be `(full_id, phase_started_tick, phase)`.
Unpaid refusals should not consume paid-counter RNG keys; if they need duplicate
suppression, distinguish them by a refusal-domain key such as hunter/tick.

## Actual current geometry defect, not merely a missing view field

`world.rs::mouth_point` currently returns
`travel(hunter.pos, hunter.heading * profile.jaw_offset_px).end` without inspecting
reflection or fallback. `HunterView.mouth` therefore reports a reflected mouth
as if it were visible. Exact fixture: root Front `(32,63)`, heading `(0,1)`,
offset 6 returns a mouth near Front `(32,59)`, even though the intended
attachment is below the open rim at y=69. The root-owned renderer clips that
attachment; it does not fold it back above the body.

The helper is already implemented; its use in final capture settlement is still
in progress. Fix it BEFORE using it as contact authority. Merely adding a contact
position to events while leaving this helper unchanged would record a precise
but visually false contact.

Minimal shared geometry surface, names illustrative:

```text
HunterContactGeometry {
    scale,
    capture_offset_body: Vec2,   // explicitly scaled or explicitly unscaled
    capture_reach_px,
    capture_center: Option<SurfacePoint>
}
contact_in_root_chart(hunter_root, heading, geometry, prey_position) -> ...
```

Core owns one pure size/contact helper and uses it in contact resolution AND the
observer. Unfold target/prey from the hunter ROOT, rotate to the same body basis
as `stamp_rig`, and compare to the capture offset there. A separately carried
mouth chart can select another shortest image at a top vertex, so distance from
that chart alone does not prove contact with the drawn claw. If exposing a
physical center, accept it only when a non-reflected, non-fallback transport
round-trips through the root-owned unfolding to its intended body coordinate.
Otherwise expose `None`, not a fabricated reflected point. Keep the bounded
first-policy refusal of off-surface/ambiguous grasp centers unless partial
contact has its own explicit proof.

Changing `mouth` to `Option` is a transient observer change, not a snapshot
change. Alternatively add a new optional capture-geometry record and make the
adapter ignore the old mouth field; do not retain its current false guarantee.
The existing six-pixel placeholder still fails the measured-art check; the
previous contract contains exact jaw/claw coordinates. Capture and ingestion
points should be named separately, even if the first profile retains one
scalar field for trial capture distance.

## Share scale; do not infer it from the juvenile flag

Current view exports `structure`, `extent` and `juvenile`; the latter is merely
`structure < 0.7 * structure_adult`. Current mouth offset and reach are unscaled
profile constants, and founder `phenotype.extent` is overridden to the profile's
fixed extent. Thus `view.extent / profile.body_extent_px` is NOT evidence of a
juvenile scale, and the boolean cannot provide continuous maturation.

Minimal improvement: expose authoritative `body_scale` from the same core helper
that computes scaled contact offset/reach (or expose `structure_adult` plus one
shared documented pure helper). Root can currently obtain adult structure from
the matching live organism phenotype, but must not independently choose a
different scale equation. Pass that scale to Fable's whole-rig scaling API;
scale offsets, head lunge, both claws and query bounds together.

Keep three extents distinct: visual query support, physical crowding extent,
and capture region. The current profile's `body_extent_px=9` is admitted against
the default world's `body_extent_max=9`; it is not the fully extended art's
roughly sixteen-pixel query radius. Do not override the world's limit or claim
the physical shape already equals all filtered artwork. Define and test the
chosen body collision approximation separately. Audit sensing/pursuit range
when replacing the six-pixel contact placeholder, as already requested.

## Capture events must preserve settlement evidence before removal

Current `Capture` contains only tick, hunter/prey IDs and M/Q. `Attempt` contains
outcome and payment but no paid-attempt key or positions. Once the prey is removed,
the later render view cannot recover its exact post-movement position; the prior
view is one movement interval too old. The hunter may also move, change target
or die before a deferred diagnostic consumes the event.

Minimal event additions:

- On both paid `Attempt` and `Capture`: the SAME documented `attack_counter` key.
- On `Capture`: `prey_pos` at settlement, `hunter_pos` and `hunter_heading` used by
  the contact test, plus its actual scaled capture offset/reach or a compact
  immutable contact-geometry snapshot. Include the validated physical capture
  center when one exists; distinguish it from the prey center.
- For geometric failures worth auditing, record the same post-movement contact
  evidence on `Attempt` when the target resolves. Missing/stale targets naturally
  have no prey position; do not fill it from a reused slot.

Produce this record from the COMMON post-movement settlement state before
removing prey or clearing targets. These are transient diagnostic/observer fields;
they need not enlarge persisted `HunterState` or mutate existing `RenderView` /
`OrganismView`. Root can log them once and recover the capture location exactly.
An ephemeral event stream is not restart-replay history: do not claim old captures
will reappear after loading a snapshot unless the host actually journals them.

For exact recoil reconstruction after restart, phase times alone do not reveal
whether Recovering followed a fully extended miss, an unpaid refusal, or a
finished meal. The adapter can conservatively initialize a folded pose without
inventing an attack. If exact restart parity during the short recoil is required,
persist the minimal transition origin/pose fact in the new hunter extension;
do not guess it from durations that happen to differ in the default profile.

## Focused handoff tests

1. View timing matches member timing at each transition, held fractional draws
   and restart; no six-second study attacks during a long real phase.
2. Off-rim `(32,63)` example yields no reflected contact. Root-owned vertex
   coordinate mismatch is rejected/handled by the same policy core and art use.
3. Juvenile and adult capture geometry uses exactly the scale sent to art.
4. Successful event geometry equals the pre-removal settlement snapshot; no
   later view lookup is needed. Capture/Attempt share one stable paid key; stale
   generations and unpaid refusals cannot alias a paid capture.
5. Observer/event enrichment leaves empty-extension continuation, world hashes
   and RNG streams unchanged. These tests do not establish predator balance.

Verified sources: [hunter types](../../crates/cubarium-core/src/hunter.rs),
[world observer/helper](../../crates/cubarium-core/src/world.rs), and current
[progress report](fixed-hunter-core-progress-2026-09-13.md). No source edits or
large test runs during native-worker ownership. This is an integration gap report,
not a second full ecology or art implementation plan.
