---
status: open
date: 2026-10-10
owner: one worker per species, worktree `.claude/worktrees/feed-<species>`, branch `feed-<species>`
---

# FEED-ROLLOUT: a feeding clip for every eater, one species at a time

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink from main's, explicit-path commits, MP4 recipe, ≤ 40-line
return). Blender runs `-b` only. Worktrees and targets never go under `/tmp`. Put every
video you hand back under the **main checkout's** `art/gen/runs/feed-rollout-2026-10-10/`
(absolute path), not inside the worktree: ignored files die with it. `unsafe` only for the
`ash` FFI calls in `cubarium-gpu` (you should need none).

## Why

Wrysk picked the bone rig with turned cubes (FEED-RIG, `design/handoffs/feeding-motion-rig-
2026-10-09.md`, merged 31db6437) and approved the Veilhound's clip: "veilhound animation
looks great!" (2026-10-10). Now each eater gets its own clip, drafted as a placeholder he
directs from video, one species at a time.

## The pattern (from the Veilhound, merged)

- The rig and clip live in the species' builder in `scripts/blender/animal_models.py`
  (`chorister()`, `ch_feed_armature`, `ch_feed_clip`, `ch_feed_angles`, `rig_armature`,
  `sample_clip`): every built part on one bone, the action sampled into the species' `.rig`
  side file at bake. Every baked size bin carries its bones.
- The engine is done: bone index in `FigureVoxel.bits` 14+, `bonePose` in `figure.vert` and
  `figure_sun.vert`, the clip evaluated on the frame clock, its weight easing with
  `State::Cropping`, each animal on its own loop phase. Touch it only if a species needs
  something it lacks, and say what.
- Evidence comes from `crates/cubarium/examples/voxel_feed_capture.rs`: two grown animals
  feeding, side-on and three-quarter, at the desktop's 6 px and 12 px, lit, real time,
  **today (no clip) | the clip** side by side. Report cells per bone.

## Species sections

Each section is one package. The clip is a believable placeholder; Wrysk directs the real
choreography from the video. Don't invent body parts, colours or new poses beyond it.

### Loftstrider (first; a renderer-worker package)

The tall browser (2.2 × 2.2 × 3.4 m adult, feeding 0.8–3.5 m up, "a sloppy feeder"). Its
builder is `loftstrider()` / `loftstrider_rig(p)` / `build_loftstrider`: a column body, a
neck, the pink head with its mouth on top, six eyes and tusks, **six fronds** rising from
the rim and drooping, **six spider legs** (hip bulb, thigh out and down to a bulbous knee,
shin to the ankle, three toes). The chibi calf is blended in by `ls_blend`; the rig must
hold for the calf bins too.

Wrysk (2026-10-10): "loftstrider feeding will be the hardest since it uses those fronds to
reach and grab foliage and place it into its mouth. we probably want to rig them to be able
to raise and lower their main bodies too, and lean to sides and bend the neck. their
resting state could be them lowering down and touching their core to the ground while the
legs bend like spiders'"

- **Bones**: the body (root); each leg a thigh and a shin (toes on the shin), so the body
  can rise, drop and lean while the feet stay planted (solve it in Blender with IK and
  sample the result; the engine only plays bone transforms); a neck chain of 2–3 that
  bends; the head; each frond a chain of about 3 that can reach and curl. Report the bone
  count and cells per bone, adult and calf, and say if the engine's per-figure bone limit
  or the tile box needs to grow.
- **Feed clip (placeholder; Wrysk directs the real one)**: browsing a crown above and to
  one side.
  1. Reach: the body rises on its legs and leans toward the foliage, the neck bends that
     way, and the two fronds facing it reach up and out into the browse band, 0.7 s,
     eased.
  2. Grab and bring: those fronds curl closed on a spray and arc back over the head to the
     mouth on top, 0.6 s; the head dips a little to meet them.
  3. The next pair round reaches while the first returns to rest, alternating, so the
     fronds are never all still. A small chew bob of the head after each delivery, 0.3 s.
  4. Loop while it crops; the reach and the return come from the clip weight.
- **Rest pose (the first back-apply of sleep to the rig)**: Wrysk's description above. The
  body lowers until its underside touches the ground and the legs fold spider-like, knees
  high, feet where they stood. It eases in and out with the state the sim uses for the
  loftstrider's rest or sleep (find it, `crates/cubarium/src/voxel/sleep.rs` and the
  animal state; say which), replacing the class-path settle for this species only. Keep
  any breathing the sleep path has, on top.
- **Engine**: today a species has one clip whose weight follows `State::Cropping`. This
  needs two (feed, rest), each with its own state-driven weight, easing, never popping,
  and blending sensibly if both are partly on. Build the smallest general form (a clip per
  state in the `.rig` side file) so the other species and idle/hunker can use it later.
- Find whether the sim knows, per bout, how high the cell it crops is, and which side. If
  cheap, say how the reach could aim at it (don't build it); the placeholder reaches a
  fixed height and side, stated.
- **Evidence** as above, adult and calf: the feed loop (at a vaulttree if
  `voxel_feed_capture` can place one cheaply; say if not), and the rest transition
  (standing → lowered → standing). Side-on and three-quarter, 6 px and 12 px.

### Seedporter (second)

Loftstrider merged (e85c4499): the engine now has clips per state (`.rig` CRG2: each clip
names its state, `cropping` or `asleep`, as a loop or a track; `Spec.clips`;
`AnimalRig::pose(bin, &[ClipAt])` layers them), bone limit 64. Build on it; change the
engine only if the seedporter needs something it lacks.

The diurnal frugivore (`seedporter()`, `sp_body`, `SP_POSES` in `animal_models.py`; pose
models `seedporter` = climbing, `-stand`, `-ground`, `-glide`, memory: one Blender rig,
baked per pose). Wrysk's anatomy: the membranes run from the big arms' wrists to the hind
knees; **the small fruit arms are the middle pair**.

- Find which pose model the renderer draws while the seedporter crops (it eats fruit in
  stands and on vine faces) and rig that pose; rig the others too if it is cheap, and say
  which you did.
- **Bones**: root (body), head (jaw if the model has one), each small fruit arm a chain of
  2–3 to the hand; the big arms, legs and tail stay on the root unless the clip needs them.
  Report bones and cells per bone, adult and young.
- **Feed clip (placeholder; Wrysk directs the real one)**: the fruit arms do the work.
  1. Reach: both small arms reach up and forward to a fruit about a body length ahead and
     above, 0.4 s, eased; the head tips up toward it.
  2. Pluck and bring: the hands close and pull back to the mouth, 0.4 s, the head dipping to
     meet them.
  3. Chew: the head bobs twice, 0.5 s, the arms held in.
  4. Loop while it crops; the reach and return from the clip weight.
- No sleep pose this round.
- **Evidence** as above (`voxel_feed_capture`): today | the clip, side-on and three-quarter,
  6 px and 12 px, adult and young, in the pose it feeds in.

Next sections (written when each starts): frondgrazer (its neck under the shield is an
open look question), lanternjaw, bellwing, capgnawer, littershredder,
ripple-snail. Then sleep, idle and hunker move onto the rig.

## Return

Commits; cells per bone; the MP4 paths (absolute); what looks wrong (joint gaps, tile
clipping, shadows, the calf); anything the engine lacked. Stop there: no merge.
