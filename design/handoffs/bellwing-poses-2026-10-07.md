# Handoff: bellwing pose models (paused 2026-10-07)

**Goal:** give the bellwing a model for each thing it does in the simulation, the way the seedporter has
four (`5e819d74`, `design/animal-body-reimagining-2026-09-24.md` §Seedporter "Poses"). Today a
bellwing wears its one model (wings spread, the artbook front view) whether it flies, lands or hangs.

**Where it stopped:** two rounds of candidate images are generated. **Waiting on Wrysk's picks.** No
model, bake or code work has started.

## The candidates

These are Qwen 2.1 native-reference edits of the artbook pick `art/gen/artbook/bellwing.png`, made by the
sprite-forge lane (local ComfyUI, batch 4).

- **Run directory:** `/home/wrysk/wryskware/cubarium/art/gen/runs/2026-10-02-bellwing-poses/`.
  - `art/gen/runs/` is gitignored, so these images exist **only on the desktop**. Back them up if
    that matters.
- **Generator:** `gen.py`, adapted from the seedporter's. Every image has a `.run.json` with its
  prompt, seed and reference, verdict "unjudged".
- **Round 1** (seed 3001, artbook ref): `sheet-bellwing-poses.png`.
- **Round 2** (seed 3002): `sheet-bellwing-poses-v2.png`. It has strict-profile prompts, wings named as
  folded flat, and two arms:
  - `-v2` used the artbook ref;
  - `-v2b` used `BW-beat-down-side-3001-b0.png`, a true side view, as the ref.
  - The round-1 sip and wing-beat-down rows are repeated at the bottom of the sheet.
- **Per-pose sheets:** `sheet-BW-<pose>-side[-v2].png`.

| Pose (sim state) | Best candidates | Notes |
|---|---|---|
| Landed on a ledge (`Perched`, `PerchKind::Face`) | v2 row | Wings folded but front-on. v2b b0 lies on its side. |
| Landed on a crown top (`CrownTop`) | v2 row | Wings folded, front-on. Could share the ledge model. |
| Clinging to a vine wall (`VineFace`) | v2 row | Side-on, wings folded. **The tendril never grips the vine in any image**, so it has to be modelled by hand. |
| Hanging under a crown or overhang (`Underside`), bell pressed up into the leaves (hang-a) | v2 or v2b | Folded wings, front-on. |
| Hanging from the tendril (hang-b) | v2b | Best profile. Most hang mouth-down rather than upside down. |
| Flying, wing beat up | v2b b0, b2 | True side views, bell tipped forward. Pairs with the beat-down images. |
| Flying, wing beat down | round 1 b0, b1 | True side views. |
| Sipping at a bloom (HoverSip) | round 1 b0, b2 | Tendril into the flower, bell tilted. |

**Front-on is acceptable for the folded poses.** The bell is round and the folded wings wrap it
evenly, so a side model looks much the same from any side. Only the flying poses really need profile.

**Anatomy to keep** (artbook README, `art/gen/artbook/README.md` "Bellwing"): plum bell with a
magenta rim, four lilac wings with cyan leading edges on knobbly roots near the top, and a fine
round tendril with a spindly 3D corkscrew end, no droplet. Its muted peach shifts a little yellower,
never saturated orange or yellow. Wrysk dislikes the images' outer glow; there is only a faint rim
glow here.

## Open questions for Wrysk

1. **Picks:** which image per pose. If a pose has no good image, drop it or describe the fix.
2. **Ledge and crown:** one landed model or two?
3. **Hang:** hang-a (bell up against the leaves), hang-b (from the tendril), or both?
4. **Sip pose and wing beat:** wanted now? Both need presenter work beyond picking a model by mode
   (see below).

## After the picks: the work, in order

1. **Record the picks.** Follow `65da2220` ("Seedporter poses selected"):
   - copy each pick into `art/gen/artbook/` as `bellwing-<pose>.png`;
   - add a section to `art/gen/SIGNOFF.md`;
   - add the artbook README row and `index.html` entry;
   - add a "Poses" paragraph under the bellwing in `design/animal-body-reimagining-2026-09-24.md`.
2. **Rig and bake.** Follow the seedporter's one-rig pattern (`sp_body`, `SP_POSES` in
   `scripts/blender/animal_models.py`):
   - Turn `bellwing()` (`animal_models.py:973`) into one rigged body posed per pick, with wing hinges at the
     roots and a tendril curve that can wrap a stem.
   - Bake in `scripts/blender/bake_voxel_models.py` into `assets/voxel-models/{125,250}/`. A hanging
     pose's z = 0 is the support above it, so it needs the seedporter stand's `BELOW_GROUND` /
     `floor=None` handling, mirrored.
   - Add the poses to the `voxel_specimens` strip, using the bellwing's coat from
     `crates/cubarium/src/voxel/colours.rs`.
   - Iterate in numbered rounds (r1, r2, …) with headless renders for Wrysk; no agent decides the look.
   - Specimens only: nothing simulated changes.
3. **Presenter.** `mode_model` (`crates/cubarium/src/voxel/model.rs:209`) picks by `MobilityMode`
   only. The bellwing's landed, vine and hanging poses are all `Perched`, told apart by
   `PerchKind` (`crates/cubarium-voxel-fauna/src/flight.rs`). So the lookup has to see the perch
   kind: widen the signature, or add a perch-aware lookup beside it. `Fly` keeps the existing `bellwing` model.
   - **Sip:** HoverSip is a heuristic, and the presenter cannot see that an animal is sipping today
     (only ledger counters such as `sips_by_bloom`). A sip pose needs a visible per-animal flag first.
   - **Wing beat:** this is two models alternating on a cycle, so it is an animation, not a mode. Check it against the
     "animation never pops" rule (`animation-never-pops` memory) before building it. The
     simplest acceptable form may be the beat-down model at rest in flight, with no cycle.

## Process notes

- **Image lane:** the `sprite-forge` agent type (Opus, low effort, pinned), driving `gen.py`. Local ComfyUI only;
  check `curl 127.0.0.1:8188/system_stats` first. About 1 min per pose for a batch of 4.
- **Prompt lessons:**
  - The front-on artbook ref pulls every pose front-on.
  - "Strict profile, wings edge-on" helps.
  - A side-view reference image (the v2b arm) pulls harder toward profile, but sometimes lays the body down or spreads the wings.
  - Naming the wings "folded flat down against the sides of the bell" worked.
  - No prompt got the tendril to grip a stem.
- **Rig and bake worker:** Opus at medium effort, iterating rounds with Wrysk. It commits explicit paths only, never merges or
  deploys, and follows the "workers never wait on jobs" rule (Blender `-b` renders are short; anything
  over 5 min launches detached and hands back).

## Related state

- The bellwing has been live since F1 (roster line, `design/handoffs/voxel-roster-2026-10-01.md`).
  It starves within 30 min on defaults; that is for the tuning pass.
- Glassfilm drying is merged (`65043f6a`): residue under 1 cm counts as dry, and dried film dies whole.
  It is unrelated to the poses.
