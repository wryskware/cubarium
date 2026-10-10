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

### Loftstrider round 2 (after Wrysk saw the first clip)

Round 1 merged (e85c4499; MP4s `art/gen/runs/feed-rollout-2026-10-10/loftstrider-feed-rest-px{6,12}.mp4`).
Wrysk: "so the loftstrider fronds should curve in bending the other direction. they can
unfurl, reach upwards, and curl the opposite way to place food in the mouth. for the
sitting, the feet need a separate joint so that they stay flat on the ground. implementing
aiming is a great idea, or at least the framework for it."

- **Fronds**: from their rest droop they unfurl (straighten), reach upward into the browse
  band, and curl the **opposite** way to their droop, inward over the head, to place the
  spray in the mouth on top. Then they relax back to the droop.
- **Rest**: a foot bone per leg (the ankle a joint), so the toes stay flat on the ground
  while the knees fold. Also fix the hip bulbs dipping about 5 cm into the ground: the body
  lowers until its underside touches, no further.
- **Aiming (the framework, and the loftstrider using it)**:
  - Sim: when an animal takes a bite of a stand, keep the bitten cell on the animal
    (`founder_feed`, `cubarium-voxel-fauna/src/step.rs` ~4086, from `body::mouth_meal`'s
    `Meal::Stand` place). **Transient, out of the save** (`#[serde(skip)]` or kept beside
    the animal), so no save schema change and no behaviour change; say how you kept the sim
    bit-for-bit the same in what it decides.
  - Renderer: per frame, the bite's height above the feet and its bearing from the heading,
    eased so a new bite doesn't snap the aim. Carried in the figure `Spec` so any species'
    clips can read it.
  - Loftstrider: the reach follows the height (blend between a low and a high reach, or
    IK the fronds to it; your call) and the bearing (the frond pair nearest it does the
    work, by its six-fold symmetry, and the lean and neck turn toward it). With no bite
    recorded it reaches as today.
- **Raise and lower** (Wrysk left it to us: "do whatever makes sense"): the standing model
  stays as it is (legs nearly straight, so it can't rise above its stand). The body drops
  instead: for a low bite the knees bend and the body crouches toward it; for a high one it
  stands at full height and the fronds and neck do the reaching. The crouch comes from the
  aim's height, eased.
- **Evidence** as round 1, plus an aiming clip: one animal browsing bites placed low, high,
  and to each side (stage them in `voxel_feed_capture`), today | round 2.

### Lanternjaw (third)

Seedporter merged (3e107bd1): rigs are looked up by model name
(`ModelLibrary::rig_in_mode`), so pose models can carry rigs. The Veilhound
(`chorister`, `ch_feed_*`) is the nearest pattern: a carrion tear.

The ambush carnivore (`lanternjaw()` in `animal_models.py`): an armoured body under six
back plates, a head with a central ridge and four magenta eyes, **two large pale fangs
hanging in front of the mouth**, **two striking arms** (a raised femur, a tibia curving
down into a pale razor blade), walking legs, and the tail branches carrying the lure orbs.

- **Bones**: root (body, plates, walking legs, tail), head (eyes, ridge, fangs), each
  striking arm a femur and a tibia/blade. Leave the tail and lure on the root unless the
  clip needs them. Report bones and cells per bone, every bin (the larva is a separate
  model: leave it).
- **Feed clip (placeholder; Wrysk directs the real one)**: at a carcass on the ground
  ahead.
  1. Pin: both striking arms swing down so the blades press the ground about one voxel
     ahead, 0.35 s, eased; the head lowers toward the same spot.
  2. Tear: the head pulls back and up about 4 cm with a small twist, alternating sides,
     0.4 s, the arms pressing down harder as it pulls; then the head dips again, 0.3 s.
  3. Every second tear, a swallow: the head lifts about 6 cm, 0.4 s, the arms easing up.
  4. Loop while it crops; the reach and return from the clip weight.
- If the aim framework lands on main before you finish (branch `feed-loftstrider2`), don't
  wait for it; the lanternjaw's carrion is ahead on the ground either way.
- **Evidence** as above: today | the clip, side-on and three-quarter, 6 px and 12 px, the
  adult and the smallest bin past the larva.

Next sections (written when each starts): frondgrazer (its neck under the shield is an
open look question), bellwing, capgnawer, littershredder,
ripple-snail. Then sleep, idle and hunker move onto the rig.

## Return

Commits; cells per bone; the MP4 paths (absolute); what looks wrong (joint gaps, tile
clipping, shadows, the calf); anything the engine lacked. Stop there: no merge.
