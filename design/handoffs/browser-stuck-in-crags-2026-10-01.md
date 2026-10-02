---
status: open
date: 2026-10-01
owner: unassigned; for a new thread (Wrysk)
---

# Browsers stuck in crags: diagnose first

**What Wrysk saw** (live view, 2026-10-01): some browsers get stuck in a crag or on a hill
and spin around forever, looking for a way out. The world was the terrarium, with the
co-training lock step's ecology and brains at generation 12 (`runs/live-view/a-g12`).

The question is which kind of stuck it is. Answer it before fixing anything.

## Hypotheses

1. **Spawned in a pocket.** Founding placed the body in a walk-region that isn't connected
   to the main one.
   - Founding already logs "seed N has no 0.5 m closed walking route (observation only)".
   - Placement may not check which connected component a spot belongs to.
2. **One-way entry.** The body got in some way it can't get back out. Step up and step down
   are equal (frondgrazer 0.375 m both ways), so look at other routes in:
   - falling or sliding;
   - water receding;
   - stands growing across the exit;
   - the body growing (juvenile to adult) until it no longer fits the gap it came through.
3. **Navigation.** The exit exists, but the controller can't find it. The brain's senses may
   not show walls or the exit (cone, occlusion), and turning wins whenever forward motion is
   blocked. Compare the heuristic driver with the brain.
4. **Motor.** Forward effort against a riser turns into rotation only, with no slide along
   the wall.

## Method

Use a headless probe, as in `crates/cubarium/examples/starve_probe.rs`.
- **Detect stuck bodies:** displacement under about 0.5 BL over 5 min while motor effort
  stays above some threshold.
- **For each stuck body, flood-fill the walk graph from its cell,** under its own step,
  wade and clearance rules.
  - A small reachable area (a few m²) means trapped: hypotheses 1 or 2. Record whether the
    body was founded there, or log the event that cut it off.
  - A large reachable area means a navigation or motor failure: hypotheses 3 or 4. Rerun
    with the heuristic driver.
- **Report:** counts by cause, over seeds 1–8, in about 1 h runs.

## Fix directions, after diagnosis

- **(1):** found only in the largest walk component, or in one above some size.
- **(2):** refuse steps that leave the body with no route back; or treat growth into a
  pocket as a design question.
- **(3):** a "stuck" or exit sense, or an escape behaviour.
  - The sense contract is Wrysk's (`design/voxel-senses.md`).
  - Step heights and clearance are digest rows: changing them retrains the brains.
- **(4):** wall-slide motion in the motor.

## Constraints

- Diagnose first. No windows: headless only. Short runs.
- Every new number is a registered knob (WORKING_POLICY).
- Explicit-path commits.
