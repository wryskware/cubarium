---
design_status: proposal
last_reviewed: 2026-09-23
decision_refs: []
---

# Large animals: the canopy browser, the lanternjaw at full size, and a pack hunter

Wrysk's organism design session, continued 2026-09-23 after the Blender scale lineup
(`scripts/blender/organism_lineup.py`, renders in `runs/scale-lineup-2026-09-22/`).
Companion to `design/organism-anatomy-2026-09-21.md` (plant layers, small animals) and
`design/art-direction/species-dossiers-2026-09-21.md` (pictures). What Wrysk said is
recorded as said; everything else here is a proposal.

## 1. What Wrysk said (2026-09-23)

- **The ladder sizes stand**, having seen them at scale in the world box.
- **The lineup's adjustments are agreed:** seedlings capped in width as well as height;
  tall species grow up from the seedling instead of starting near adult height; thin
  anatomy (vanes, fronds, drapes) is designed to read at sprite scale, because voxels
  turn it into 1-voxel rods.
- **Lanternjaw is bigger.** It is a mesopredator, fox or coyote scale, not an apex.
- **A missing niche: an animal that grazes fully grown vaulttrees.** Something big and
  alien.
- **After that, decide on a second carnivore**, a pack hunter able to bring down the
  big browser. Pack behaviour is probably not a plain per-animal network any more,
  but it is worth attempting.

**Later the same day (Wrysk):** "i agree on the animals and their niches. im not settled on
body design but the scales are correct for me." So the roster additions (loftstrider,
lanternjaw at coyote scale, chorister), their niches and the sizes in §7 are agreed;
the body plans in §3–5 remain proposals.

**Later still (Wrysk, 2026-09-23), on the open questions:**
- **Bodies go to the ladder now** ("rip off the bandaid"), plant growth and ladder sizes
  with them, even though the retrain in flight learned the old sizes. Wrysk notifies the
  trainer thread; retraining may start from the existing checkpoints on the new bodies.
- **Headroom** for the loftstrider was already handed to the landscape thread.
- **Small populations: deferred.** Ideas Wrysk is weighing: the tiny world may simply
  host a different community; or choristers need only one large meal and then rest or
  hibernate for a long time, so one loftstrider kill carries a pack a long way.
- **The growth fix's frond consequence is accepted:** a young umbrellafrond is browsable
  until its lowest tier grows out of the grazer's band.
- Order: plant illustration round and the anatomy tables first, then the plant packages
  (`design/handoffs/voxel-ladder-growth-2026-09-23.md`,
  `voxel-new-plants-2026-09-23.md`, `voxel-organism-models-2026-09-23.md`); predation
  and flight go to an ecology-adjacent thread
  (`design/handoffs/predation-and-flight-2026-09-23.md`); the loftstrider is its own
  package later.

## 2. Growth fix (the agreed adjustments, as numbers)

Today crown height and radius are linear in wood fraction from each range's minimum,
and the seedling stage caps only height. So a vaulttree seedling is a 1.3 m-wide flat
star and a juvenile vaulttree is already 21 of its 28 voxels.

Proposal: **a seedling is a rosette at most 0.125 m tall and 0.125 m in radius.** From the
end of the seedling stage (`wood_fraction_max` of stage 0, `w0`), height and radius
grow linearly to the species maximum at full wood:
`h = h_seed + (h_max − h_seed) · (wf − w0) / (1 − w0)`, same for radius. The ladder's
ranges then describe *adults*; their minimum only matters for species without a seedling
stage (the ground cover).

| Juvenile at wf 0.35 | Today's rule | Proposed |
| --- | --- | --- |
| vaulttree | 21.5 v (2.7 m) | 8.5 v (1.07 m) |
| umbrellafrond | 10.8 v | 4.6 v (0.57 m) |
| bloomcrown | 4.75 v | 2.3 v (0.29 m) |

**A consequence to note:** with this rule a juvenile umbrellafrond's lowest tier
(0.5 of its height) stays inside the frondgrazer's 0.5 m mouth band until the frond is
about 1 m tall. Organism decision 5 had the frond escaping at the seedling→juvenile
transition. The proposed rule restores the anatomy document's first reading: the young
frond is browsable and grows out of reach. Either is defensible; the second gives the
grazer a real effect on frond recruitment.

## 3. D15 · Loftstrider (*Grallator vellicans*, "the loftstrider"): the canopy browser

*Niche.* Eats the foliage between 0.8 m and 3.5 m above the ground it stands on: adult
vaulttree lobes and drapes, umbrellafrond tiers, adult lanternberry. It does **not** eat
ground cover, rosettes or turf, which stay the frondgrazer's. Two browsers, split by
height, is the classic browse partition, and it gives vaulttree and adult umbrellafrond
a consumer for the first time (today nothing eats either once grown).

It is a **gap-maker**: shade already falls with each layer's foliage stock
(`step.rs`, `shade_k_per_m2 · (1 − porosity) · stock`), so stripped lobes let light
through and the understory under a browsed vault grows, with no new rule. It is a **sloppy
feeder**: a share of each bite falls as litter under the tree, which feeds the
littershredder and glowcap. Both are ordinary mass-balance flows, not bonuses.

*Size.* Hull 1.5 m long, 0.55 m wide, 0.45 m deep, carried with its underside at
1.15 m; back at 1.6 m. Feeding column at rest tops out at 2.2 m, extended at 3.5 m.
**In voxels: 12 long, 13 to the back, reach 28.** The largest animal in the world, as
tall as an adult vaulttree when feeding.

### Body plan, primary: "stilt column"

- **[hull]** a keel-shaped body, deeper than wide, deep plum `#2A0E4A` in long plates
  with `#12093A` seams; the keel line underneath is pale lilac `#B99BE6`.
- **[stilts] × 4 in a diamond**: one at the front, one at the rear, two at the sides
  under the middle of the hull. Not a quadruped's pairs: the gait rocks from diamond to
  tripod (the front or rear stilt lifts while the three others hold), so it always
  stands on three and sways as it walks. Each stilt has two thin segments and a splayed
  three-toed pad; the two side stilts carry **hooked spurs** at the ankle, which are its
  defence (it kicks sideways).
- **[column]** its feeding organ, and its silhouette. It rises from the front third of
  the hull as three nested telescoping segments banded lilac on plum. At rest it stands
  about 0.6 m above the back; feeding, it extends to 1.9 m, and it can bend forward 45°
  to take a frond tier down to 0.8 m.
- **[rake]** at the column's top, six flexible comb-fingers that close around a lobe
  and draw back through it. The inner edges are lilac, the only light-coloured part at
  the top.
- **[sense ring]** a ring of magenta `#FF2AFC` patches just under the rake, so it sees
  the canopy it feeds in. Two more patches at the hull's front see the ground route. The
  sensing manifest gets **two eye anchors**: a canopy cone at the column top, pitched
  up, and a route cone at the hull.

*States.* Walking (column at rest, rocking gait); Feeding (legs braced wide, column
extended, rake working); Resting (column retracted into the hull, stilts folded so the
hull sits at 0.6 m). **Feeding is its vulnerable posture**: braced, top-heavy, eyes
turned up.

*Life history.* Long-lived, slow to breed: one young per birth, long gestation. The
young **follows the parent**, and two loftstriders walking together is a visible social
read. A newborn is 0.46 of adult length (0.7 m hull, reach about 1.6 m), so it eats
drapes, lanternberry and the lower frond tiers: the adult's diet at a lower reach, with
no diet switch. Its carcass is the largest carrion in the world, a long feast for the
shredders and scavengers.

*Alternates.* **(B) Hanging climber**: a sloth-like animal that lives in the vault's
limbs and eats lobes from above. This asks nothing of the terrain's headroom, but
nothing on the ground can hunt it, which kills the pack story. **(C) Rearing browser**:
a heavy ground animal that rears onto a tail tripod to reach the crown. More familiar,
less alien.

## 4. D16 · Lanternjaw at full size: the mesopredator

The roster's lanternjaw (a compact ambusher with long stillness and brief strikes,
whose glow is paid for) is kept, at coyote scale: **body 1.0 m long, 0.25 m wide,
0.45 m tall (8 × 2 × 3.5 v)**, up from 5 × 2.

*Prey.* Frondgrazers, littershredders, capgnawers, grounded seedporters and newborn
loftstriders; carrion as a fallback, including a pack's kill (it is a kleptoparasite
at the edge of a carcass).

*Body plan sketch* (a full dossier is owed): a low, long body on four legs, set far
back, and a heavy head that is mostly jaw. The **lure** is a stalk rising from the
brow, carrying a luminous cyan bulb, which it holds dead still. The glow costs reserve.

## 5. D17 · Chorister (*Lycochorus*, "the chorister"): the pack hunter, for decision

*Why it exists.* An adult loftstrider is too tall and too well armed for any one
predator: the side spurs and a 3.5 m column. A pack can take it by working the stilts
while it is braced to feed. Alone, a chorister hunts frondgrazers like the lanternjaw
does; that overlap is deliberate (two predators on the same prey, separated by hunting
style and group size).

*Size.* Body 1.4 m long, 0.35 m wide, shoulder 0.6 m, and a stiff counterweight tail of
0.8 m: **11 × 3 × 5 v plus tail.** Packs of 3 to 5.

*Body plan sketch.* A long, low runner on four legs with a rigid tail held level for
turning. No head in the earthly sense: a **split jaw**, two lateral plates that open
sideways. Under the throat is a translucent **call bladder** that flashes cyan when the
animal calls. This is the point of the design: **the pack's coordination is visible.**
The calls are the pack channel in the model (a broadcast cue), and on the panel you would
watch a hunt as a sequence of flashes passing between runners closing on a braced
loftstrider. The flash is paid glow, like the lanternjaw's.

*Pack behaviour: three ways to build it.*

1. **Scripted tactic, learned bodies.** A pack-level controller picks roles (driver,
   flankers) and target points; each chorister's own network steers, senses and bites
   toward its assigned point. The most controllable option, and it works first.
2. **Learned pack with a call channel.** Each chorister's network gets packmate
   bearings and distances plus a call input and a call output. The pack is trained
   together (multi-agent evolution) on loftstrider episodes. Cooperative pursuit does
   emerge in multi-agent training, but it takes much more training and may settle on a
   degenerate tactic.
3. **One controller for the pack.** Simplest to train, but the pack stops being
   individuals, and a lost member breaks it.

Recommendation: start with 1, and give each network the call inputs and outputs
anyway. Once 1 hunts, try 2 with 1 as the teacher, which is how the trained founders
were bootstrapped from heuristics.

## 6. The hard questions these animals raise

1. **Can populations this small persist in a 20 m ring?** An adult vaulttree is 2 m
   across; the panel ring holds perhaps 4–8 of them, so perhaps 1–3 loftstriders, and a
   pack of 3–5 needs frondgrazers as its staple with a loftstrider kill a rare event
   (wolves on moose). At these numbers one bad month is extinction. Levers, in order of
   preference: (a) long lives and slow metabolism, which the abstract units allow
   (organism decision 7); (b) the predators' staple is the frondgrazer, the loftstrider
   a windfall; (c) the desktop world (32 m) or a larger panel ring for the big-animal
   tier; (d) reintroduction as a user lever (the game angle), since the biosphere rule
   is no spontaneous appearance. **This is Wrysk's call**, and it decides whether the
   pack is ambient-first or a later-world feature.
2. **Terrain must have room for them.** The loftstrider needs a grove floor with about
   4 m of clear headroom, soil deep enough for vaulttrees, and a walkable route between
   groves; the chorister pack needs open runs to chase on. The terrarium generator's
   dense, stacked base is the opposite. This is a request for the landscape thread: at
   least one open grove level, or big animals confined to the top plateau.
3. **Predation does not exist in the voxel line.** The lanternjaw lives only in the
   legacy `cubarium-core`. Attack, damage, defence (the loftstrider's kick), death to
   carrion and carcass feeding are a substrate package before any predator.
4. **Two eye anchors** (the loftstrider's canopy and route cones) extend the sensing
   manifest, which today has one fan per founder.

## 7. Updated ladder rows (proposal)

| Animal | Size (v, at 0.125 m) | Was |
| --- | --- | --- |
| loftstrider | hull 12 × 4.5, back 13, reach 28 (feeding) | new |
| chorister | 11 × 3 × 5 + tail 6.5; packs of 3–5 | new, for decision |
| lanternjaw | 8 × 2 × 3.5 | 5 × 2 |

## 8. Model packages these imply (owed, not dispatched)

1. **Growth fix** (§2): seedling radius cap and growth from the seedling. Goes with the
   ladder geometry package.
2. **Vaulttree** (anatomy §3, with the dead-wood line at death).
3. **Loftstrider**: a founder species with a high mouth band, two eye anchors and bite
   spill to litter.
4. **Predation substrate**, then the **lanternjaw** founder.
5. **Pack channel** (a broadcast call cue) and the **chorister**, if Wrysk adds it.
