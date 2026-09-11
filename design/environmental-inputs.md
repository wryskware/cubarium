---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# Environmental stimuli, independent of sensors

Expose one versioned environmental event envelope. Audio/vision/touch adapters
interpret their own devices outside the core. The world accepts physical-style
quantities and applies them through the same fields used by autonomous weather.
Silence and absent sensors are normal operating conditions.

Proposed normalized envelope, not an implemented API:

```text
StimulusV1 {
    source_id, sequence,
    target_tick,
    location: Global | SurfacePoint,
    radius_pixels,                 // local footprint along the surface
    attack_ticks, hold_ticks, release_ticks,
    effect: Light | Moisture | Nutrient | Flow,
    amount,                        // effect-specific units; finite and bounded
    direction: optional TangentVector
}
```

`Light` adds irradiance integrated through the normal photosynthesis budget;
`Nutrient` deposits material with an explicit accounting source. `Moisture`
shifts bounded environmental suitability. `Flow` perturbs local drift or
locomotion load; it is not a direct position update. Global flow requires a
spatial vector-field definition, so the first API accepts flow only with a local
tangent frame. Source-specific raw
audio amplitudes and frequencies never enter the core schema.

Stress and chemical Signal effects are future schema/capability extensions,
available only if their ecological mechanisms are implemented and enabled.
Reject unsupported effects explicitly; do not allocate unused fields or silently
accept a signal into a world that cannot respond. Log admitted schema/capability
versions for replay. The sensor contract does not force optional M4 mechanisms.

Define units and normalization per effect before implementation. For example,
a nutrient event's `amount` is total deposited material, independent of footprint
area; a light event's `amount` is peak irradiance above baseline with bounded
duration. Distributing an extensive quantity must conserve its integrated
amount; clipping a local disk at the rim does not lose or multiply it.

## Admission and determinism

The host translates source timestamps to simulation ticks. Reject duplicates
by source/sequence; clamp small lateness to the next tick and journal that chosen
tick; reject events beyond a finite scheduling horizon. Validate locations,
effect units, channel IDs, finite numbers, radius, magnitude, envelope duration,
and direction before admission. Replay bypasses wall-clock mapping and uses the
already accepted normalized envelope.

Bound source count, per-source rate, total pending events (initially 256), and
total external material/energy per simulated minute. Compatible continuous
events may coalesce by source/effect/region without changing the admitted total;
otherwise reject excess with a reason. Apply smooth envelopes and saturation
in environmental fields. Do not allocate one object per audio sample or let an
unattended loud room create unlimited food.

Sources time out to baseline. Their last sample is not held indefinitely.
Accepted events and any capped amounts are observable in logs. Default zero
external input reproduces the autonomous world exactly.

## First adapter and later mappings

Implement one local scripted/replay adapter during the input milestone. It
submits the same envelope a future device would use, including a localized event
straddling Right/Top. No audio or camera dependency is needed to prove the seam,
budget, decay, dropout, and replay behavior.

Later mappings are proposals for adapter code:

| Source feature | Possible environmental influence |
| --- | --- |
| Smoothed audio intensity | Modest additional light, with a slow envelope and saturation |
| Spectral balance | Slowly shifting moisture or producer suitability |
| Rhythm confidence | A weak recurrent energy envelope, not direct creature motion |
| AI sound interpretation | Low-rate bounded weather parameter events with expiry |
| Face touch | Local warmth/stress, food deposit, or attraction signal that diffuses across seams |
| Proximity or surrounding movement | Broad low-frequency disturbance or directional flow |

Initially avoid mapping audio straight to mutation rate. Establish the metabolic
and ecological response first; noisy sensors should not destroy inherited
continuity. No input grants reproduction, selects a winning species, or commands
all creatures to move together.
