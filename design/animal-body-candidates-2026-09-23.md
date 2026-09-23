---
design_status: proposal
last_reviewed: 2026-09-23
decision_refs: []
---

# Animal body candidates: a matrix of interpretations

Wrysk's organism design session, 2026-09-23. Sizes and niches are agreed
(`design/organism-large-animals-2026-09-23.md` §1 and §7, the ladder in
`design/art-direction/organism-scale-and-roster-2026-09-21.md` §4). Body plans are
not. Wrysk asked for widely different interpretations of every animal except the
frondgrazer and littershredder (fairly settled) and the bellwing (close), written down
and then blocked out in Blender (`scripts/blender/animal_bodies.py`, renders in
`runs/animal-bodies-2026-09-23/`).

Every candidate keeps the species' size, diet and the job the ecology needs from it.
Everything else is open. Each entry gives the idea, the silhouette at panel scale, how
it does its job, what it senses with, and what it would ask of the model. Codes (LS-A,
LJ-C…) match the Blender matrix. Colours follow the palette: plum/violet bodies,
magenta sense patches, cyan for anything that glows, warm only on fruit the animal
carries.

## Settled or close (one column each, bellwing three)

- **FG · frondgrazer: saddle shield** (dossier D3). Six legs, a dipped shield, a
  folded neck and cropping scoop, magenta shoulder patches. 0.75 × 0.375 m.
- **LT · littershredder: plough segment** (D4). Five overlapping plates, a blunt plough,
  feelers down, no eyes. 0.375 × 0.125 m.
- **BW-A · bellwing: bell with paired vanes** (D14). A hanging bell body, two pairs of
  stiff wings, a coiled siphon. **BW-B fan-tail**: one fan wing over a long counterweight
  tail. **BW-C spinner**: a disc body carried by one helical vane that turns like a
  sycamore seed. Body 0.125 m, span 0.375 m.

## Loftstrider: canopy browser (reach 0.8–3.5 m, hull ~1.5 m)

The ecology needs a ground animal the pack can reach that feeds 2.5–3.5 m up, strips
vaulttree lobes, spills litter and is vulnerable while it feeds.

- **LS-A · Stilt column.** Four stilts in a diamond, a keel hull at 1.4 m, and a
  telescoping feeding column with a six-finger rake that extends to 3.5 m. It always
  stands on three legs and sways as it walks. *Silhouette:* a table with a mast.
  *Vulnerable:* braced, column up, eyes up.
- **LS-B · Tethered float.** The body hangs from a large translucent gas sac that holds it
  up among the lobes, 2.5–3 m high. Six long, thin tether-legs reach down to the ground;
  it walks by re-planting them one at a time, like a spider slung under a balloon. It
  grazes from inside the canopy. *Silhouette:* a lantern in the treetops with lines to
  the ground, the strangest thing in the world. *Vulnerable:* the tethers. A pack drags
  it down by the legs. *Model:* its body occupies the canopy while its feet are on the
  ground; the occupancy is a column, not a box.
- **LS-C · Arch walker.** The body *is* an arch: a long, flexible hoop of tissue 3 m long
  with a cluster of legs at each end. It straddles plants and walks by inching, drawing
  its ends together (the arch rises to 3.5 m) and apart (it lowers to 1.8 m). The mouth
  and sense ring are at the apex. It feeds by planting its ends on either side of a
  vaulttree and rising into the crown. *Silhouette:* a living croquet hoop, and its
  height changes as it moves. *Vulnerable:* when fully arched it is at its narrowest
  base and most top-heavy.
- **LS-D · Tongue caster.** A compact, heavy quadruped (back at 1.3 m) with a long
  ballistic tongue coiled in its chest that it fires 2.2 m straight up into a lobe and
  reels back in. Big upward-facing eye bowls on its back. *Silhouette:* an ordinary
  bulky animal until a thin line flicks into the canopy. *Model:* a fast, reach-only
  bite; the body's own height stays low, so it is the easiest one for the terrain's
  headroom.

## Lanternjaw: mesopredator, ambush with a paid lure (~1.0 × 0.45 m)

Ecology: long stillness, brief strike; eats frondgrazers, shredders, capgnawers,
grounded seedporters and newborn loftstriders; scavenges carrion; its glow costs
reserve.

- **LJ-A · Brow lure stalker.** A long, low quadruped with legs set far back and a heavy
  head that is mostly jaw; a cyan bulb on a brow stalk held dead still. The plain
  reading of the name.
- **LJ-B · Trapdoor.** Lives in a shallow pit with only its lure above the surface: a flat
  disc body 0.8 m across, legs folded under, a huge mouth hinged along the front edge that
  opens *upward*. Prey walks to the light and the ground opens. *Silhouette:* on the
  surface only the light and a rim. *Model:* a burrow or dug pit (terrain edit), or it
  simply lies flush in litter.
- **LJ-C · Tail-lure mantis.** A tall, thin stalker, 0.6 m high, standing among stems.
  Its folded fore-blades are raised in front of it and a jointed tail curls over its back
  like a scorpion's, holding the lure out ahead of the blades. It strikes down with the
  blades. *Silhouette:* vertical and stick-like, hiding in a frond stand.
- **LJ-D · Coil serpent.** Legless, 1.4 m long, lying coiled in an S. The lure is on a
  forked tongue it lets hang out. It strikes by uncoiling. *Silhouette:* a coil on the
  ground, the one animal in the world with no legs.

(Further idea, not built: **false flower**, a lanternjaw whose back grows glowing,
bloom-like lure fronds and which sits in a meadow as a fake bloomcrown clump. Grazers
coming to crop it are caught by long jointed arms folded underneath.)

## Chorister: pack hunter (1.4 m + tail, shoulder 0.6 m, packs of 3–5)

Ecology: runs down frondgrazers alone; as a pack it brings down a feeding loftstrider.
The pack's calls are a visible cyan signal.

- **CH-A · Split-jaw runner.** A four-legged runner with a rigid counterweight tail; two
  lateral jaw plates open sideways; a throat bladder flashes cyan when it calls.
- **CH-B · Relay centipede-hound.** Long and low on six legs with a segmented back. The
  call organ is a row of dorsal vents that light **in sequence from nose to tail**, so
  every call shows which way that runner is heading. *Silhouette:* a long ribbon close
  to the ground.
- **CH-C · Fork-crest strider.** A biped: two long digitigrade legs, a horizontal body
  and tail. In place of a head is a tall **tuning-fork crest**, two blades that
  vibrate to call and glow along their edges. Small grasping forelimbs.
  *Silhouette:* upright, fast, with two antenna-like blades that flash.
- **CH-D · Radial star.** Five legs radiating from a flat disc body, with no front or
  back: it runs in any direction without turning, so a pack encircles by nature. The
  call organ is a central dorsal lens that pulses. The mouth is on the underside and
  it pins prey beneath the disc. *Silhouette:* a starfish running.

## Capgnawer: fungivore of glowcap caps (~0.25 × 0.19 m)

Ecology: eats fungal tissue (weaker fallback: conditioned litter); visits logs; may
carry spores; the biosphere suggests glow attracts it (luminous-fungus research).

- **CG-A · Puffball nibbler.** A round, soft body on four short legs, a rasping mouth
  disc underneath, and bristles on its back that catch spores. The roster's reading.
- **CG-B · Log limpet.** A flat oval that clamps to bark and gnaws caps from beneath.
  Its dorsal shell mimics a glowcap cap, so a colony may have an impostor sitting in
  it. *Silhouette:* one more cap on the log, until it moves.
- **CG-C · Probe snout.** A small body on spindly legs with a long tube snout that
  probes into splits and under caps. It moves like a wading bird over a log.
- **CG-D · Glow moth.** A flying capgnawer drawn to the light: soft broad wings and a
  furred body that carries spores between logs. It lands on caps to feed. This makes
  the glowcap's paid glow an actual trade (spore transport for tissue), and it is the
  second flyer in the air column. *Model:* needs flight, like the bellwing.

## Ripple snail: aquatic film grazer (~0.19 m)

Ecology: scrapes glassfilm off lit, shallow wet surfaces; slow; leaves a cleared trail;
no land foliage.

- **RS-A · Flat spiral.** A disc-coiled shell on a broad foot; two short eye stalks. The
  roster's reading.
- **RS-B · Ribbed dome.** A limpet dome with radial ribs, clamped flat, with a skirt of
  feelers around the rim.
- **RS-C · Ripple ray.** A flat, soft oval that glides over the film by passing waves
  along its margins (the name made literal), with a rasping underside. Nearly flush with
  the surface. *Silhouette:* a moving ripple on the water's edge.
- **RS-D · Case bearer.** A soft grub that lives in a tube case built from grit and
  dead film and drags it along; only its comb mouthparts and front legs show.
  *Silhouette:* a small twig on the move.

## Seedporter: fruit eater and seed carrier (~0.5 × 0.25 m)

Ecology: eats lanternberry fruit pulp and some seeds, climbs trunks and drapes, and
carries seeds between groves; gliding is optional.

- **SP-A · Coil-tail climber.** A slender four-limbed climber with a long prehensile
  tail that coils around drapes, and cheek pouches for fruit. The roster's reading.
- **SP-B · Four-sail glider.** Six limbs with membranes between the front four; it climbs
  a vaulttree and glides down to the next grove. Colonising across gaps is the reason
  the terrarium's levels connect at all.
- **SP-C · Basket crab.** A crab-like climber with a woven dorsal basket in which it
  carries fruit and seeds, visibly: warm lanternberry fruit ride on its back. The seed
  payload is readable on the panel.
- **SP-D · Tri-arm brachiator.** A small body with three long hooked arms spaced around
  it. It swings from drape to drape, rotating as it goes, and never touches the ground
  if it can help it. *Silhouette:* a spinning three-pointed star in the drapes.

## How to read the matrix

`runs/animal-bodies-2026-09-23/`: `matrix.png` stitches one row per species, each row
at its own zoom with its scale noted, so the small animals are visible. The
`row-<species>.png` files are the individual rows. `true-scale.png` shows every
candidate on one floor at true size beside the frondgrazer and a 1.75 m person. The
`.blend` holds them all to orbit. These are rough blocks to judge silhouette, posture
and size; nothing is a finished body. Pick, mix or reject per species.
