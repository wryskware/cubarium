---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent corrected hunter geometry review

Scope: core commit `22d8dba`, checked against the measured Lanternjaw effectors
and the exploration animation contract. Canon was read first. This is evidence,
not a new decision or permission to deploy hunters. No production files, live
worlds, processes, or display transport were changed.

**Follow-up disposition:** findings 1 and 3 are RESOLVED by `327862c` and
`f36817f`, respectively, with independent regression verification below. Finding
2 remains open pending the completed Fable scale implementation and validation.
The original findings and reproductions are retained as historical evidence.

## Original disposition at `22d8dba`: three integration gates

1. **Post-settlement phase timestamps are one tick early.** In `World::step`,
   successful settlement enters Handling with `(now, now)` and failed settlement
   enters Recovering with `(now, now + recovery_ticks)`, although both the attempt
   and capture events are stamped `now + 1`. `HunterView::phase_started_tick`
   explicitly means the completed-tick boundary at which the phase was entered.
   A deterministic capture fixture reports world/capture tick **33**, Handling
   start/end **32/32**. Interpolating from that published entry starts the recoil
   before contact, and failed-strike recovery loses one tick of its advertised
   duration. Use the actual settlement boundary for post-movement transitions
   and their deadlines; do not indiscriminately shift the legitimate pre-step
   Windup/Strike decision boundaries. Test a successful capture and a paid miss:
   their new phase must start exactly at the settlement event tick, remain fully
   extended through that boundary, and retain the same timing across restart.
   The later `9cf1912` meal-empty transition also runs after settlement and uses
   `now`; apply the same boundary audit there.

2. **The admitted juvenile scale is wider than the current art API.** Core's
   trial has `body_scale_min = 0.2`, exponent `0.5`; Fable's current
   `Lanternjaw::draw_living` asserts a finite scale in **0.5..=1.0**. A valid
   `WorldConfig` with `child_structure_fraction = 0.1` produces child structure
   **0.2**, adult structure **2.0**, authoritative scale
   **0.31622776601683794**. That cannot be passed to the drawing API. Default
   children at fraction 0.4 are about 0.632, so the normal recipe does not itself
   demonstrate a panic. Reconcile the admitted range explicitly before wiring
   arbitrary valid worlds; do not silently clamp only the renderer, which would
   separate its claws from core contact geometry. Include a below-0.5 valid
   juvenile and the smallest admitted scale in the adapter/geometry tests.

3. **NaN scale minimum survives admission and persistence.**
   `FixedHunterProfile::validate` checks `body_scale_min <= 0 || > 1`, but this
   field is absent from the finite-number loops. NaN passes both comparisons.
   The fixture below confirms that profile validation, `start_hunter_trial`, and
   subsequent snapshot decoding all accept it. Require a finite value as well
   as the agreed range; test rejection at both admission and snapshot decode.
   This is a validation defect, not evidence that default trial scales are NaN.

These findings concern `22d8dba`; inspection after `9cf1912` and `15e599d`
landed found the cited expressions unchanged. No ecological balance claim follows
from the tests below.

## What is now supported

- Contact is measured by unfolding the prey from the hunter root and rotating
  into the renderer's forward/clockwise-side basis. Publishing the swept effector
  additionally rejects reflection/fallback/tie paths and requires a root-chart
  round trip. Off-rim grasp centers cannot authorize a capture. The new claw and
  ingestion-mouth fields are separate; physical extent is not the visual query
  extent. Existing seam/rim and new round-trip tests support these corrections.
- Windup's end and Strike's timed span are explicit. Payment increments the
  stable attempt counter before opening Strike; paid Attempt and Capture share
  that key. Unaffordable attempts have no paid key. Actual capture evidence is
  gathered after movement and before removal, retaining full prey ID, prey and
  hunter poses, heading, scaled geometry, and the measured contact. This is
  sufficient for root's local-capture observer without guessing a prior pose.
  Unaffordable evidence is gathered earlier, in the decision pass: do not use
  such refusal records as post-movement capture observations.
- The schema-10 mirror preserves its previous field layout. Migration accepts
  an entirely default hunter extension and refuses nonempty trial **history**,
  including controls/extinct trials, rather than reinterpreting it. The wording
  “active trial” is narrower than that actual policy. Genuine schema-7/8/9
  continuation tests pass; the synthetic schema-10 test below validates its
  decoder branch but is not a genuine old-binary continuation fixture.
- Fable's currently authored `effectors(1.0)` is mouth `(9.6, 0)` and near claw
  `(13.279411764705882, 1.1)`. Core matches mouth/x but still uses claw y
  `1.162368` from the earlier decorated study measurement. This **0.062368 px**
  difference is well inside the proposed 1.5-px reach, not a new gross contact
  failure. Reconcile it in the exact measured-centers assertion; do not call
  the two current constants identical. Fable's source was actively being edited
  during this review, so this comparison is explicitly to its inspected worktree.

## Test provenance

To exclude concurrent source changes, `git archive 22d8dba` was extracted into
`/tmp/cubarium-geometry-review-beG9yI`. Production sources in that archive were
unchanged. Only its test files received the four diagnostic probes below.

Against the archived commit, **45 existing tests passed**: `hunter` 25,
`hunter_migration` 4, `energy_correction` 7, `snapshot_hardening` 8, and
`care::zero_care_reproduces_the_pre_change_binarys_next_600_ticks` 1. The three
bug-demonstration probes and synthetic schema-10 decoder probe also passed;
their success confirms the documented current behavior, not its correctness.

After native core work committed `9cf1912`, the shared tree's separate
`cargo test -p cubarium-core --offline --test hunter_geometry` passed **6/6**,
including metadata, paid-key, root-chart, juvenile-scale and restart checks.
Those are later-commit evidence and are not counted among the 45 above. The
restart test does not assert that post-contact entry equals settlement tick;
the juvenile test does not exercise a scale below the art minimum.

## Preserved diagnostic fixtures

Add these tests to `tests/hunter.rs` in an isolated archive of `22d8dba`; they
reuse that file's existing helpers. Run its `astra_` filter with `--nocapture`.

```rust
#[test]
fn astra_nan_scale_min_is_currently_admitted() {
    let mut world = empty_world();
    let mut p = trial(&world);
    p.body_scale_min = f64::NAN;
    assert!(p.validate().is_ok());
    world.start_hunter_trial(
        p, target_of(SurfacePoint::new(Face::Front, 20.0, 32.0))
    ).unwrap();
    assert!(decode_snapshot(&encode_snapshot(&world.state, "astra-probe")).is_ok());
}

#[test]
fn astra_valid_small_child_scale_is_below_art_minimum() {
    let mut cfg = WorldConfig::default();
    cfg.organism.child_structure_fraction = 0.1;
    let world = World::new(cfg).unwrap();
    let p = trial(&world);
    let adult = decode(&p.genome, &world.config().organism).structure_adult;
    let child = world.config().organism.child_structure_fraction * adult;
    let scale = cubarium_core::hunter::body_scale(&p, child, adult);
    println!("child={child} adult={adult} scale={scale}");
    assert!(scale < 0.5);
}

#[test]
fn astra_post_contact_phase_entry_precedes_the_settlement_tick() {
    let p = certain(trial(&empty_world()));
    let (mut world, hunter, _) = staged(p);
    for _ in 0..400 {
        world.step();
        for event in world.drain_hunter_events() {
            if let HunterEvent::Capture { tick, .. } = event {
                let member = world.hunters().members.iter()
                    .find(|m| m.id == hunter).unwrap();
                println!("capture={tick} world={} start={} end={}",
                    world.tick(), member.phase_started_tick, member.phase_ends_tick);
                assert_eq!(tick, world.tick());
                assert_eq!(member.phase, HunterPhase::Handling);
                assert_eq!(member.phase_started_tick + 1, tick);
                return;
            }
        }
    }
    panic!("no capture");
}
```

Add this fourth probe to the archive's `tests/snapshot_hardening.rs`, reusing
its imports and `stepped_world` helper. It serializes the frozen mirror, not
the current world relabeled with an old header.

```rust
#[test]
fn astra_schema_ten_mirror_loads_empty_and_refuses_trial_history() {
    let world = stepped_world(20);
    let mut old = cubarium_core::snapshot::v10::project(&world.state).unwrap();
    let frame = |old: &cubarium_core::snapshot::v10::WorldStateV10| {
        let payload = postcard::to_allocvec(old).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(b"CUBW");
        out.extend_from_slice(&10u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    };
    let (meta, back) = decode_snapshot(&frame(&old)).unwrap();
    assert_eq!(meta.schema, 10);
    assert_eq!(back, world.state);
    old.hunters.control_deposited = true;
    assert!(matches!(decode_snapshot(&frame(&old)),
        Err(SnapshotError::Invalid(reason)) if reason.contains("schema 10")));
}
```

## Follow-up: committed core corrections verified

Read the actual diffs and regression tests in `f36817f` and `327862c`.
Independently ran `cargo test -p cubarium-core --offline --test hunter --test
hunter_geometry`: **34 passed, 0 failed** (26 hunter, 8 geometry). Core paths were
clean when inspected; later documentation-only `ad512f8` was present at the final
source-status check. No core or art source/test files were modified for this
follow-up.

- **Finding 1 RESOLVED:** successful capture, failed paid attempt, and the
  post-digestion empty-gut pause now enter at `now + 1`, with timed deadlines
  measured from that boundary. The pre-step decision timestamps are unchanged.
  Regression tests check Handling starts at capture tick; paid-miss recovery
  starts at its event tick and lasts its full span, including the contiguous
  next phase; and the meal pause starts on the actual gut-empty tick. The restart
  test now checks settlement-time entry in both worlds, not just eventual hashes.
- **Finding 3 RESOLVED:** profile validation explicitly requires finite scale
  minimum, scale exponent, and escape-speed multiple before their range checks.
  The regression tests NaN and both infinities for each field, rejects both
  initializers, rejects planted invalid state and snapshot payloads by field name,
  and retains valid finite boundaries.
- **Finding 2 still OPEN:** Fable's active worktree extends the drawing range to
  0.2..=1.0 and is adding minification filtering. Those are promising in-progress
  changes, not a completed independently verified rendering disposition. This
  follow-up does not claim that the old tiny-scale panic remains in those edits.

### Scale-test handoff: destination units, not inverse-scale tolerance

In the inspected, uncommitted `tests/lanternjaw_scale.rs`,
`the_named_effectors_are_linear_and_land_on_the_drawn_claw_at_every_admitted_scale`
computes both `b` (destination pixel center minus root) and `effectors(s).near_claw`
in **destination/chart pixels**. Its `(0.5 / s).max(1.0)` acceptance bound is
therefore not the claimed destination-space half-texel bound: inverse scaling
belongs to the source sampling chart, while destination pixel spacing remains
one pixel. At scale 0.2 the test admits a **2.5 chart-pixel** error around a
**0.3 chart-pixel** core capture radius. A nearer painted part of the arm can pass
while the tip is missing. This identifies a weak assertion, not a new measured
failure of the concurrently edited filtered renderer.

For example, with the current +x/mid-pixel fixture the named tiny claw is
`(2.6558823529411764, 0.22)`. A lone painted near-arm pixel centered at `(1, 0)`
is about 1.67 px away and passes 2.5; even that pixel's square footprint stops
about 1.156 px short of the claw, well outside a 0.3-px contact disk. Conversely,
requiring pixel **centers** within 0.3 px would be too strict: the actual tip can
fall between centers while its pixel footprint visibly covers it.

Prefer a destination-space coverage assertion, such as distance from the named
contact disk to a painted pixel's footprint, with any filter support explicitly
included in destination units. Keep the near-limb isolation, assert nonzero
coverage near the actual tip, and vary root subpixel phase and heading. A fixed,
justified pixel-center allowance is also clearer than a tolerance growing as
`1 / scale`, but does not by itself prove coverage of the contact disk. The test's
point-sampling explanation is additionally stale relative to the inspected
`stamp_rig_scaled` edits, which now use `ceil(1 / scale)` squared box-filter samples.
