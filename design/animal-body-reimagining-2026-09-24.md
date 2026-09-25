---
design_status: proposal
last_reviewed: 2026-09-24
decision_refs: []
---

# Body reimagining: six animals and four plant revisions

> **Colour note (2026-09-24):** the hexes in this document are superseded by
> `design/art-direction/species-colours-2026-09-24.md`. Parts, counts, shapes and
> states here still bind.

Astra's prose proposals for Wrysk's next Qwen exploration. Every option is open
for rejection, combination or revision. Codes identify alternatives, not selections.
The sizes and niches stand by Wrysk's instruction; the anatomy proposed here does
not establish a decision or describe implemented behaviour.

## Reading basis and shared conventions

The brief is Wrysk's wording in [SIGNOFF](../art/gen/SIGNOFF.md). The earlier
[animal candidates](animal-body-candidates-2026-09-23.md) supply A–D; the
[large animals brief](organism-large-animals-2026-09-23.md), especially §§1, 3–5
and 7, supplies the updated animal sizes and niches. The
[scale ladder](art-direction/organism-scale-and-roster-2026-09-21.md) supplies
the other dimensions. Form, palette and motion follow
[Cubarium Art Direction v0.1](art-direction/Cubarium_Art_Direction_v0.1.md)
and the [species dossiers](art-direction/species-dossiers-2026-09-21.md),
including D1 for glowcap and D5–D14 for the plant context.

The judged evidence is the animal run's
[LOG](../art/gen/runs/2026-09-23-animal-concepts/LOG.md),
[worker picks](../art/gen/runs/2026-09-23-animal-concepts/picks.json) and
[comparison sheet](../art/gen/runs/2026-09-23-animal-concepts/sheet-all.png),
plus the plant run's [comparison](../art/gen/runs/2026-09-23-plant-concepts/compare.png)
and its four relevant Qwen subject prompts. Worker picks identify render examples;
Wrysk's comments supply the preferences. The plant run has no LOG.md.

One naming discrepancy matters: the old text and animal run call LJ-D a coil
serpent; Wrysk calls his preferred D the scorpid. LJ-D2 and LJ-D3 below follow
his current scorpid instruction. They do not silently relabel the old images.

One voxel is 0.125 m. Logical reading is at 6 px per voxel, checked at 4.
Aspect ratios describe the organism, not the canvas; appendages and pose envelopes
are distinguished from body dimensions. For plants, descriptions proceed from the
near attachment through the crown to the rear growth. Counts describe the pictured
adult example; foliage fullness and coverage still govern the living form.
Fixed markings are ornament. Proposed movements illustrate rest, locomotion and
feeding; they do not add ecological powers or claim new simulation inputs.

Each **Qwen** paragraph is a subject suffix for the
[kit v0.4](../art/gen/PROMPT-KIT.md) style block. Use the scene block for these
in-world explorations, with its elevated side view. Velvetpad uses surface framing.
An isolated version uses the kit's organism framing and the same anatomy. Render
clean illustrations; assess small-scale readability after reduction. Tall plants
and feeding loftstriders need a tall canvas. These are prompts, not generated art.

## Lanternjaw

> prefer D, the scorpid version. overall idea is good. however, needs refinement or some reimagining.

**Body need.** The scorpid direction offers a useful separation between a stationary
lure and a sudden capture, but needs a body whose jaw, supports and lure stay
distinct. Keep the 1.0 × 0.25 × 0.45 m body, approximately 8 × 2 × 3.5 voxels:
32–48 px long and 14–21 px high. It waits, strikes briefly, eats the agreed small
prey and newborn loftstriders, and scavenges; the lure consumes reserve. At this
scale a dark capture cleft and one cyan endpoint carry more than many fine claws.

### LJ-D2 · Low scorpid with a sheltered lure

**Silhouette:** a blunt forward cleft, low plated back and one open tail arch.

**Body.** At the right-hand front, two thick jaw leaves frame a vertical dark
cleft, their inner edges pale lilac `#B99BE6`. A short capture arm sits outside
each jaw; open ground separates these two hooked arms from the first walking
pair. Two flush magenta `#FF2AFC` sense patches occupy the shoulders behind
the jaw hinges. The long trunk is deep plum `#2A0E4A`, its three broad dorsal
plates separated by two flexible `#12093A` seams. Six walking legs form three
pairs, with two generous ground windows along either side. Behind the final
plate, one thick tail bends forward over the back; a large opening remains
between tail and armour. Its terminal cyan `#42C5F8` bulb hangs just ahead of
the jaw. The resting envelope is 20:9 length to height, including the folded
tail. Matte plates carry a single violet `#3A1A7A` upper facet. At rest the
arms close against the cheeks and the lure holds still. During a strike the
rear legs push, the arms open and close around prey, and the tail retracts
its bulb into the arch above the shoulders.

**Qwen.** One lanternjaw facing right beneath drooping blue fronds, body aspect
20:9 length to height, slightly longer than a nearby low grazer. At the front,
two blunt jaw leaves with #B99BE6 inner edges enclose a dark vertical cleft;
two short hooked capture arms stand outside them. Two #FF2AFC shoulder patches
precede three broad #2A0E4A back plates separated by two #12093A seams. Six
walking legs in three pairs have open ground between successive pairs. One
thick tail arches forward with open space above the back and a single #42C5F8
bulb hanging before the mouth. Matte #3A1A7A upper facets. Show the resting
ambush posture, capture arms folded, planted feet and still lure.

### LJ-D3 · Scorpid with a lateral lure channel

**Silhouette:** two forward hooks around an empty bite space, with a low cyan tip beside it.

**Body.** The front begins with a broad lilac `#B99BE6` biting lip between two
long capture arms. Each arm bends outward once, leaving a head-width opening
through which approaching prey remains visible. A pair of magenta `#FF2AFC`
sense strips sits on the arms' inner roots, facing this opening. Behind them,
a narrow plum `#2A0E4A` trunk rises into two unequal armour humps separated
by one deep violet `#12093A` notch. Four walking legs emerge as two pairs,
with a broad patch of visible ground between the pairs. The rear tapers into
one flattened, jointed tail that folds forward along the far flank. Its cyan
`#42C5F8` tip rests beside the mouth, across a clear channel from the nearer
arm. Length to resting height is 20:9; the raised armour supplies the height.
Arm faces have broad `#3A1A7A` ridges and soft lilac undersides. At rest the
body crouches over its feet. A strike sweeps the arms inward across the lure
channel while the tail slides backward. Walking opens the central notch as
the front and rear armour rock separately.

**Qwen.** One ground ambusher facing right, aspect 20:9 length to height,
among dark meadow litter. A #B99BE6 biting lip sits between two forward
jointed capture arms; a head-width space separates their hooks. Two #FF2AFC
sense strips mark their roots. Behind them, two unequal #2A0E4A armour humps
have a deep #12093A notch between them. Four planted walking legs form two
pairs with a broad ground gap. One flattened tail folds forward beside the
far flank, its single #42C5F8 tip beside the mouth across an open channel.
Matte #3A1A7A ridges articulate the arms. Show the waiting pose with the
tail channel clearly visible and the capture arms poised around it.

## Chorister

> A, leaning. needs some rework and reimagining. perhaps hexapod, or somewhat raptorial like C? not sure i like the smooth body.

**Body need.** A's sideways jaw repeatedly became a familiar head or an upward
lid in the run; smooth bodies also give little sense of stored running force.
Keep 1.4 × 0.35 m, shoulder 0.6 m, plus a 0.8 m tail: about 11 × 3 × 5
voxels plus 6.5. The body reads at 44–66 × 20–30 px before the tail. It
pursues frondgrazers alone and attacks feeding loftstriders in packs of 3–5.
Both options need directional sensing, traction and a cyan call visible between
animals; anatomy should expose the lateral jaw and break the smooth outline.

### CH-A2 · Six-footed split-jaw runner

**Silhouette:** a forked muzzle, three stepped back masses and a long level tail.

**Body.** At the right-hand front, two deep plum `#2A0E4A` jaw plates diverge
sideways from a short central throat. Their pale lilac `#B99BE6` inner faces
frame a conspicuous V of open air in the elevated view. Immediately behind,
two magenta `#FF2AFC` sense patches sit on raised cheek ridges. One cyan
`#42C5F8` call bladder hangs within a dark opening beneath the throat.
The torso has three sloping muscle plates, each violet `#3A1A7A` above and
plum below, with two deep flexible valleys between them. Six legs attach
in three pairs; their unequal fore-aft spacing leaves two different-sized
ground windows on each side. Broad feet end in two gripping lobes separated
by a notch. The rear plate narrows into the stiff 0.8 m tail. Body length
to shoulder height is 7:3; including the level tail it is 11:3. Short coarse
ridges collect around the shoulders, leaving the flanks quiet. Rest folds
all six knees beneath the torso. Running alternates two tripods, while the
jaws part laterally and the throat patch flashes during a call.

**Qwen.** One chorister running right across a grove floor, viewed slightly
from above. Body aspect 7:3 length to shoulder height, 11:3 including its
level counterweight tail. Two #2A0E4A jaw plates spread sideways, their
#B99BE6 inner edges enclosing an open V. Two #FF2AFC cheek patches sit behind
the hinges; one #42C5F8 call bladder shows beneath the throat. Three raised
#3A1A7A torso plates have two dark valleys between them. Six legs form three
pairs, with two open ground windows per side; three feet support the running
pose and three advance. Matte ridges cluster around the shoulders, with quiet
plum flanks. Keep the whole tail and every limb attachment visible.

### CH-C2 · Raptorial runner with an opening breast

**Silhouette:** horizontal tail, deep folded chest and two forward jaw blades.

**Body.** The front is a pair of upright jaw blades rooted directly into the
forward thorax. An open slit separates their lilac `#B99BE6` cutting edges;
two magenta `#FF2AFC` sensory bars occupy their outer bases. Behind this
split jaw, two small grasping forelimbs fold beneath a deep breast, with
daylight between the wrists and chest. The breast carries two broad violet
`#3A1A7A` folds around one cyan `#42C5F8` throat membrane. Those folds part
during a call, exposing a bright central wedge. Three overlapping plum
`#2A0E4A` back plates make a stepped upper outline, each overlap marked by
a dark seam. Two powerful hind legs support the horizontal torso; a wide
ground opening separates them in the running pose. One stiff tail continues
backward for 0.8 m. Body aspect remains 7:3 length to shoulder height,
11:3 with the tail. Surfaces have broad transverse creases and quiet side
panels. At rest the hind joints fold and the wrists brace against the ground.
In pursuit the wrists lift, the tail steadies the body, and the grasping
arms reach forward only at contact with prey.

**Qwen.** One chorister sprinting right, whole body visible, body aspect 7:3
and 11:3 with its stiff horizontal tail. At the front two upright jaw blades
with #B99BE6 inner edges enclose a dark slit; two #FF2AFC sensory bars mark
their bases. Two short grasping forelimbs fold below the chest with open
air around the wrists. Two #3A1A7A breast folds part around one bright
#42C5F8 call membrane. Three overlapping #2A0E4A back plates form a stepped
outline with dark seams. Two long hind legs propel the body, widely
separated in stride. Broad creases articulate matte tissue. Show the call
membrane open as the forelimbs remain folded during pursuit.

## Loftstrider

> none of these really satisfy. full reimagining.

**Body need.** The old set made the animal a narrow upright, a suspended body,
an arch or a tongue delivery system; none earned a preference. Start again
with a substantial ground animal and make canopy feeding change its whole
posture. Retain the 1.5 × 0.55 × 0.45 m hull, walking underside at 1.15 m
and back near 1.6 m: about 12 voxels long, 13 to the back, reach 28. It
eats foliage at 0.8–3.5 m, opens canopy gaps and drops litter. At 4–6 px per
voxel that is a 48–72 px hull and 112–168 px feeding reach. Route sensing
and canopy sensing remain distinct; feeding leaves ground supports exposed
to the pack. These options propose different feeding mechanics, all within
that reach and diet.

### LS-E · Rearing underside browser

**Silhouette:** a deep upright body on a rear tripod, with a broad biting notch at its top.

**Body.** At the right-hand front, two lilac `#B99BE6` cropping plates close
across a broad mouth on the hull's underside. A short muscular lip extends
beyond them; two magenta `#FF2AFC` canopy patches flank its base. The mouth
occupies the foremost end of the 1.5 m hull. Behind it, three thick plum
`#2A0E4A` body folds overlap, with deep `#12093A` creases separating their
rounded upper edges. Two smaller route patches sit low on the first fold.
Six legs form three pairs, with clear ground windows between the pairs.
The rear ends in one short bracing pad, folded against the belly while
walking. Hull length to depth is 10:3, width to length 11:30. Violet
`#3A1A7A` transverse ridges concentrate around the leg roots. Rest lowers
the folded hull between the feet. Walking carries it horizontally at the
established height. Feeding rotates the hull upright on the rear legs and
bracing pad; the extended mouth reaches 3.5 m while the four forward legs
fold close. Lower feeding pitches the mouth down to 0.8 m. Loose cut
foliage falls through the broad mouth opening.

**Qwen.** One loftstrider feeding beside a tall blue canopy tree, full body
and planted supports visible. Its 1.5 m hull has aspect 10:3 length to
depth and stands upright in this pose; the mouth reaches 3.5 m, level
with the crown. Two #B99BE6 cropping plates form a broad notch at the
upper front, with two #FF2AFC patches beside them. Three rounded #2A0E4A
body folds have deep #12093A creases between them. Six legs total: four
fold against the forward body with gaps around their joints, two rear
legs and one short rear bracing pad support the animal. #3A1A7A ridges
mark the limb roots. Show cut blue foliage falling from the mouth.

### LS-F · Paired harvesting arms

**Silhouette:** a broad low jaw beneath two long angular arms enclosing open canopy space.

**Body.** The right-hand front carries a shallow lilac `#B99BE6` mouth tray,
with two plum `#2A0E4A` biting plates meeting above it. Two magenta
`#FF2AFC` route patches occupy the tray's outer roots. Immediately behind,
two long feeding arms rise from separate shoulder sockets, an open vertical
window between them. Each arm has three muscular lengths and two deep
joint folds, ending in a pad split into three blunt gripping digits with
two visible slots. A magenta canopy patch lies behind each pad. The 1.5 m
hull continues rearward as a heavy violet `#3A1A7A` abdominal mass under
two broad overlapping plum plates. Four walking legs support its front
and rear, with clear ground between the pairs. Hull aspect is 10:3 length
to depth, width to length 11:30. Matte skin bunches at the arm hinges and
stretches smooth across the belly. At rest the arms fold along the flanks.
Walking keeps their tips near the mouth. Feeding braces all four feet
while the arms extend to 3.5 m, grip foliage and bring it into the jaw;
low reaches stop at 0.8 m. Broken foliage spills beside the tray.

**Qwen.** One loftstrider facing right under a canopy, a 1.5 m hull with
aspect 10:3 length to depth, carried horizontally with its back at 1.6 m.
A #B99BE6 mouth tray and two #2A0E4A biting plates project from the front;
two #FF2AFC route patches mark the tray roots. Two long feeding arms rise
from separate shoulders, with open air between them, each with three
lengths, two deep folds and three slotted terminal digits. Two further
magenta patches sit behind the terminal pads. Show one arm reaching the
3.5 m crown and the other lowering foliage into the mouth. Four planted
walking legs have open ground between the pairs. Two overlapping plum
back plates cover a #3A1A7A belly with broad matte creases.

### LS-G · Tall feeding mantle

**Silhouette:** a narrow elevated hull carrying a broad, open-notched sheet of living tissue.

**Body.** At the right-hand front, a transverse lilac `#B99BE6` cropping
cleft sits along the upper edge of a broad muscular mantle. Two short
cutting lobes face inward across one deep central notch. Two magenta
`#FF2AFC` patches below the lobes look into the foliage. Behind that edge,
the mantle descends as a pleated feeding surface to the hull's front;
two thick contractile margins support it, leaving the central notch open
to the surroundings. Its inner surface is violet `#3A1A7A`, with three
broad plum `#2A0E4A` folds divided by two lilac grooves. The hull extends
rearward in a quiet plated mass, 10:3 length to depth and 11:30 width
to length. Four stout legs stand in two separated pairs; two route
patches sit above the front feet. At rest the mantle folds into a thick
collar over the forebody. Walking sways that collar over the planted
limbs. Feeding raises and spreads the mantle to the 3.5 m crown; the
cropping edge closes on foliage and muscular folds convey it downward.
The rim lowers to browse at 0.8 m. Pieces escape beside the central notch,
while the braced ground body remains accessible to hunters.

**Qwen.** One loftstrider feeding beside a 3.5 m canopy, facing right.
Its horizontal 1.5 m hull has aspect 10:3 length to depth and a back
at 1.6 m. Above its front rises one broad #3A1A7A feeding mantle, supported
by two thick muscular margins. Two #B99BE6 cutting lobes at canopy height
face across a deep open central notch; two #FF2AFC sense patches sit below
them. Three plum pleats descend toward the hull, separated by two lilac
grooves. Four stout walking legs in two pairs brace below the #2A0E4A
body, open ground between pairs; two small magenta route patches sit above
the front feet. Show foliage caught in the upper cleft and fragments
falling beside the mantle. Broad matte surfaces and quiet flanks.

## Capgnawer

> torn between A and D. not sure it has any reason to fly though.

**Body need.** A's rendered face and bristles became a familiar small mammal;
D supplied appealing soft surfaces but added flight to a log-feeding niche.
Keep the rounded 0.25 × 0.19 m envelope, about 2 × 1.5 voxels or only
8–12 × 6–9 px. It eats glowcap tissue, takes conditioned litter as weaker
fallback, visits logs and may carry spores. Both options walk and grip bark.
A broad feeding notch, dark foot contact and one soft upper mass must carry
the read; bristles and spores can merge into texture. Cyan fungal light can
be sensed without making the animal's own body luminous.

### CG-A2 · Cleft-front nibbler

**Silhouette:** a soft rounded back above a broad forward bite notch and four low feet.

**Body.** At the right-hand front, a deep notch interrupts the rounded body,
exposing a pale lilac `#B99BE6` rasp pad on its lower inner face. Two short
magenta `#FF2AFC` sensory folds sit on the notch's upper corners, separated
by the dark mouth opening. Behind them, the plum `#2A0E4A` body rises into
one uneven soft hump with a violet `#3A1A7A` upper face. Four squat legs
emerge as two pairs, with a visible bark window between front and rear
feet. Each foot spreads into a blunt gripping pad. Three coarse bristle
tufts occupy the rear half of the back, separated by two smooth strips;
their tips are pale lilac. The body closes behind in a low rounded lobe.
Length to height is 4:3. At rest the feet pull beneath the hump and the
sensory folds flatten against the mouth rim. Walking raises the front
pair over bark ridges. Feeding presses the rasp pad against a cap's edge,
rocking the entire front down while the rear pads hold the log. Bristles
brush the fungal underside during the visit.

**Qwen.** One tiny capgnawer facing right on split dark wood beside a
glowcap, body aspect 4:3 length to height, approximately the size of a
large fungal crown. A broad front notch exposes one #B99BE6 rasp pad;
two #FF2AFC sensory folds mark its upper corners across the dark opening.
One rounded #2A0E4A body hump has a matte #3A1A7A upper face. Four short
gripping legs form two pairs with visible bark between them. Three coarse
#B99BE6-tipped bristle tufts cover the rear back, separated by smooth skin.
Show the front tilted into the cap edge while the rear feet hold the
wood. The whole small animal remains visible above a thin contact shadow.

### CG-D2 · Folded spore mantle

**Silhouette:** a squat body enclosed by two soft upright folds, opening sideways to feed.

**Body.** The front begins with a short lilac `#B99BE6` rasping lip set
between two thick sensory palps. A finger-width gap separates the palps;
their inward surfaces carry magenta `#FF2AFC` patches aimed toward fungal
light. Behind the mouth, two broad skin mantles rise from the flanks and
fold toward one another above the back, leaving a deep central cleft.
Their rounded edges carry dense, short lilac fibres; their outer faces
are plum `#2A0E4A` with broad violet `#3A1A7A` folds. A compact belly
continues rearward beneath them. Six short gripping legs form three pairs,
with two small bark windows along either flank. The closed animal fills
a 4:3 length-to-height envelope. At rest the mantles meet above the belly
and the feet hold the log. Walking lifts alternate leg tripods while the
folds rock together. During feeding the mantles open just far enough to
brace their soft edges against neighbouring caps, exposing the rasping
front. Fibres contact the fungal tissue and can carry adhering spores
during the subsequent walk between feeding patches.

**Qwen.** One grounded capgnawer facing right on a fungal log, closed-body
aspect 4:3 length to height. At its front one #B99BE6 rasping lip lies
between two thick palps with a clear gap and inward #FF2AFC sense patches.
Two soft #2A0E4A skin mantles rise from the flanks and fold over a compact
belly, leaving a central cleft; broad #3A1A7A creases and short lilac
edge fibres texture their surfaces. Six short legs in three pairs grip
the bark, with two gaps along each side. Show the mantles partly opened
against two neighbouring cap edges as the mouth feeds. The body remains
supported by its planted feet, approximately one large crown in size.

## Ripple-snail

> B or D. D is giving canoli squid, while B is giving sand dollar. might need some adjustments.

**Body need.** B's ribbed dome reads well but risks becoming all shell and
ornamental fringe; D's moving case gives a clearer front but can hide the
feeding contact. Keep the approximately 0.19 m, 1.5 × 1.5 voxel footprint,
only 6–9 px across. This slow aquatic grazer scrapes glassfilm from lit,
shallow wet surfaces and leaves a cleared trail. The useful adjustments are
a directional opening, visible attachment to wet rock and a simple contour
that survives fringe loss. Neither option acquires a land-foliage diet.

### RS-B2 · Notched mineral disc

**Silhouette:** a shallow circular disc with one front notch and a darker cleared track behind.

**Body.** At the right-hand front, one broad notch opens through the shell
rim. Two short lilac `#B99BE6` palps project from it with a dark water
gap between their tips; magenta `#FF2AFC` sensory patches sit at their
bases. The rasping mouth lies immediately beneath this opening, pressed
against the rock. Behind it, one shallow round shell expands over a broad
adhesive foot. Three thick violet `#3A1A7A` ridges sweep backward across
the plum `#2A0E4A` shell, separated by two broad dark channels. Its rear
edge stays continuous and low. The footprint is 1:1 length to width;
the side profile is 3:1 length to height. Matte mineral plates carry one
quiet lilac facet near the notch. Four short soft foot lobes show around
the rim, separated by exposed wet-rock intervals. At rest they clamp
down. During crawling a contraction passes from the front notch toward
the rear, carrying the disc slowly forward. The mouth scrapes as it
moves, leaving a clean strip of darker wet rock behind the rounded shell.

**Qwen.** One ripple-snail facing right on film-coated shallow wet rock,
footprint aspect 1:1 and side profile 3:1 length to height. One shallow
#2A0E4A shell has a broad notch in its front rim. Two short #B99BE6 palps
project across a dark water gap, with two #FF2AFC patches at their bases;
the mouth touches rock beneath them. Three broad #3A1A7A ridges sweep
backward over the shell with two dark channels between them. Four small
foot lobes grip the rock at separated points around the rim. Matte
mineral surfaces and one pale front facet. Show the animal crawling with
a clearly cleared strip of dark wet rock extending behind it.

### RS-D2 · Open-front folded case

**Silhouette:** a squat folded tube with two pale feeding lobes projecting from its open end.

**Body.** The right-hand front is an oval opening bordered by a thick,
rolled case edge. Two soft lilac `#B99BE6` feeding lobes extend downward
from the opening, leaving one V-shaped water gap between them. Each lobe
carries a magenta `#FF2AFC` sensory patch near its root; their inner
surfaces guide scraped film into a central mouth. Behind them, the body
is enclosed in a short plum `#2A0E4A` tube assembled from compacted grit
and dead film. Two broad violet `#3A1A7A` case folds overlap across its
top, leaving one deep oblique seam. The rear opening narrows around a
dark muscular foot that continues beneath the entire case. Its adhering
sole is broad enough to hold the tube against wet rock. Footprint aspect
is 1:1; side profile is 3:2 length to height. Grain collects at the two
ends, leaving quiet middle surfaces. At rest the feeding lobes withdraw
into the front opening. Crawling extends them onto the film while a
slow foot contraction draws the case forward, exposing a cleared track
behind the trailing rim.

**Qwen.** One ripple-snail facing right on shallow wet rock, footprint
aspect 1:1 and side profile 3:2 length to height. Its front is an oval
case opening with a thick rolled rim. Two #B99BE6 feeding lobes reach
down onto the film with a V-shaped water gap between them and two
#FF2AFC sensory patches at their roots. Behind them is a short #2A0E4A
grit-and-dead-film tube with two broad #3A1A7A folds and one oblique dark
seam. One continuous dark adhesive foot contacts the rock beneath the
whole tube. Granular texture gathers at the ends. Show the lobes extended
in feeding and a cleared wet-rock trail behind the narrow rear opening.

## Seedporter

> i would combine A and B. keep the alien monkey vibe but also give it skin flaps to glide with

**Body need.** A already supplies dexterous fruit handling and the coiling
tail; B's images repeatedly detached the membranes into familiar wings.
Keep the 0.5 × 0.25 m body, 4 × 2 voxels or 16–24 × 8–12 px, with the
tail and gliding posture shown separately. It climbs trunks and drapes,
eats lanternberry pulp and some seeds, and carries seeds between groves.
Both options combine grasping limbs, a prehensile tail and skin joined
to those limbs. The visible priorities are fruit at the mouth, the tail's
grip and the membrane outline; only carried ripe fruit receives warm colour.

### SP-AB2 · Six-limbed grove glider

**Silhouette:** a compact grasping front, two folded flank sails and a long coiling tail.

**Body.** At the right-hand front, a short lilac `#B99BE6` mouth cleft opens
between two deep plum `#2A0E4A` cheek pouches. Two magenta `#FF2AFC`
sense patches occupy the upper cheek surfaces. Immediately behind them,
six limbs form three pairs: a long forward grasping pair, a long middle
spreading pair and a short rear climbing pair, with clear spaces between
successive pairs. One continuous skin flap joins the front and middle
limbs on each flank, two membranes total; open air separates each membrane's
rear edge from the hind foot. The narrow torso is violet `#3A1A7A` above
and plum underneath, 2:1 length to resting height. Membranes have matte
lilac outer edges and broad plum attachment bands. One long prehensile
tail leaves the rear and coils around a branch. Rest gathers the skin
into two heavy flank folds. Climbing alternates handholds while the rear
feet and tail secure the body. During a descending glide the front and
middle pairs spread the attached skin; the hind feet reach toward the
next grip. One held fruit shows a small `#FF9B50` pulp centre.

**Qwen.** One seedporter facing right on a fruiting branch, body aspect
2:1 length to height, its 0.5 m body smaller than the shrub's width.
A #B99BE6 mouth cleft sits between two #2A0E4A cheek pouches, with two
#FF2AFC patches above. Six limbs form three separated pairs: long front
hands, long middle spreading limbs and short rear gripping feet. Two
continuous #3A1A7A skin membranes join front to middle limbs, one on
each side, with open gaps behind them before the hind feet. #B99BE6
membrane edges and plum attachment folds. One long tail coils around
the branch. Show a climbing rest with folded skin and one hand holding
a small fruit with #FF9B50 pulp at the mouth.

### SP-AB3 · Tail-braced glider with a free carrying pair

**Silhouette:** a forward membrane triangle, two small hands beneath the throat and a curled tail.

**Body.** The right-hand front carries a blunt mouth surrounded by two
lilac `#B99BE6` lip folds and two magenta `#FF2AFC` cheek patches. Just
behind the mouth, a small pair of hands faces inward across a clear
gap, holding fruit against the chest. Farther back, two long support
arms rise from the shoulders; two long hind legs emerge beyond the
belly, leaving broad spaces around the smaller carrying arms. Six limbs
in total serve these three distinct pairs. Two skin membranes connect
the long shoulder arms to the hind ankles along the flanks. Their outer
edges are lilac, their stretched surfaces violet `#3A1A7A`, and their
deep attachment creases plum `#2A0E4A`. The resting body remains 2:1
length to height. One muscular tail coils tightly from the rump. At
rest it takes the animal's weight beneath a branch while the four long
limbs gather the membranes into overlapping folds. During climbing the
long hands and feet take successive grips. Gliding spreads both membranes
while the two small hands retain fruit; ripe pulp supplies one warm
`#FF9B50` patch beside the mouth.

**Qwen.** One seedporter facing right, hanging below a branch by one
coiled prehensile tail, compact body aspect 2:1 length to height. Two
#B99BE6 lip folds and two #FF2AFC cheek patches surround a blunt mouth.
Six limbs total: two small carrying hands beneath the throat, two long
shoulder arms and two long hind legs. Open space separates the small
hands from the long limbs. Two continuous #3A1A7A skin membranes attach
shoulder arms to hind ankles along the #2A0E4A flanks, their #B99BE6
edges folded beside the belly. Show the four long limbs loosely folded
while the two small hands hold one fruit with #FF9B50 pulp to the
mouth. Broad matte creases reveal where the gliding skin attaches.

## Umbrellafrond

> 3/5. i dont love the radial symmetry

**Body need.** Equal fronds around each stem level made the rendered plant
read as repeated circular tiers. Its job is still a rooted, deep-rooted
wet-hollow producer, tolerant of saturation and low light, shading velvetpad
beneath layered drooping foliage. Keep 8–16 voxels tall and 6–12 wide:
32–96 × 24–72 px across the 4–6 px checks. The mature aspect is 4:3
height to width. Unequal attachment positions and large intervening gaps
can change the form while preserving its leafy layers, young browsable
growth and adult reach. Movement is passive sway and moisture-driven droop;
the canopy itself supplies the light-gathering surface.

### UF-R2 · Offset drooping tiers

**Silhouette:** three unequal leafy shelves stepping around a visibly bent stem.

**Body.** At the near base, one thick violet `#3A1A7A` stem emerges from
damp soil, its plum `#2A0E4A` leaf scars grouped into three broad bands
with quiet bark between them. The stem rises through a shallow bend to
the first tier, which extends mainly rightward. Two broad blue `#2B6AD0`
fronds arch from separate attachments, a triangular sky gap between their
drooping ends. The middle tier faces left and carries three narrower
fronds with two uneven slots. The rear upper tier carries two short
ascending fronds divided by one broad opening. Bare stem separates all
three tiers. Height to total width is 4:3 at full growth. Frond undersides
are `#1E2798`; broad raised midribs carry restrained `#42C5F8` facets.
Surfaces are matte and longitudinally folded, with thick rolled tips.
At rest the seven fronds hang at unequal depths and the lowest tier
holds the widest shade. During sway the upper pair leads, the lower
pair lags and the openings alternately widen. Drying lowers the frond
tips while the stem keeps its planted shape.

**Qwen.** One mature umbrellafrond rooted in a wet hollow, aspect 4:3
height to width. One bent #3A1A7A stem has three separated #2A0E4A scar
bands. Three offset tiers carry seven fronds total: two broad fronds
extend right from the lowest tier across a triangular gap, three narrower
fronds extend left from the middle tier with two uneven slots, and two
short upper fronds rise at the rear across one broad opening. Bare stem
shows between tiers. #2B6AD0 upper surfaces, #1E2798 undersides and raised
#42C5F8 midrib facets. Thick rolled tips droop at unequal heights. Show
the resting plant shading a low dark carpet, with its base and the sky
openings visible.

### UF-R3 · Forked curtain crown

**Silhouette:** an off-centre upper fork bearing two broad hanging fronds of unequal depth.

**Body.** The near base is one stout plum `#2A0E4A` stem with a violet
`#3A1A7A` lit face. It rises into a short rightward bend and then divides
into two unequal upper branches, a tall wedge of open sky between them.
The nearer branch holds one broad folded frond hanging down and outward;
the farther branch carries a second, shorter frond above the first.
Each frond divides into three attached terminal straps, their rounded
tips separated by two deep cuts. The two frond bodies remain distinct
across a broad diagonal gap. The full plant has a 4:3 height-to-width
aspect. Upper surfaces are blue `#2B6AD0` with dark `#1E2798` folds and
one cyan `#42C5F8` rib per strap. Thick tissue forms shallow channels
from attachment to tip. At rest the lower frond presents a broad shaded
underside above the soil, while the upper one tilts toward open light.
Sway twists each attachment slowly so the curtain's width changes.
Moisture loss deepens the hanging folds and draws the straps closer;
the fork remains readable through the central opening.

**Qwen.** One mature umbrellafrond in a wet hollow, aspect 4:3 height to
width. A single #2A0E4A stem with a #3A1A7A face bends right and forks
into two unequal branches with a tall sky wedge between them. Two broad
folded #2B6AD0 fronds hang from the branches: one large near frond and
one shorter higher rear frond, separated by a diagonal air gap. Each
frond ends in three rounded straps divided by two deep cuts, six tips
total. #1E2798 folds and one #42C5F8 rib per strap articulate thick matte
tissue. Show the resting crown with the lower curtain shading damp soil,
the upper curtain tilted outward and the planted stem fully visible.

## Velvetpad

> dunno how i feel about it. its a little odd looking. maybe it just looks too regular of a pattern

**Body need.** The old brick arrangement and every-fourth highlight gave
the ground an even manufactured rhythm. Keep a low, broad carpet of thin
damp soil under standing crowns and ledges, with low-light tolerance and
slow turnover. Its height stays 1 voxel, coverage width 3–6: a 4–6 px
edge and 12–36 px patch. It stays a rooted surface, gathering light across
its upper tissue; expansion follows coverage and thinning exposes substrate.
Only small passive edge motion and moisture-driven flattening are needed.
Irregular seams should join across neighbouring tiles into a continuous
carpet, with the thickness visible chiefly at the exposed near edge.

### VP-R2 · Coalescing soft pads

**Silhouette:** a flat scalloped sheet with three unequal bulges along its near lip.

**Body.** At the near edge, a soft teal `#248CA8` lip rises from the
soil in three unequal rounded bulges. Two shallow notches separate them,
revealing the dark contact line beneath. Behind the lip, the carpet
spreads into three joined tissue fields: a broad near field, a narrow
left field and a smaller rear field, their junctions marked by two
branching `#1E2798` seams. The seams widen locally into quiet dark
pockets, while surrounding tissue remains continuous. A mature example
is 6:1 width to height; younger coverage is 3:1 at the same height.
Broad matte surfaces carry two short `#42C5F8` highlight clusters on
raised folds, separated by a large unmarked middle. The far margin
thins into the soil beneath the crown. At rest the fields lie heavy
against the terrain. A slight passive lift travels along the exposed
lip and settles. Moisture loss flattens the bulges; foliage thinning
opens irregular substrate windows along the seams. Adjacent healthy
patches meet through broad soft connections, continuing the carpet's
surface beyond the pictured crop.

**Qwen.** A velvetpad surface spreading across damp shaded ground, seen
slightly from above, mature patch aspect 6:1 width to raised-edge height.
The near #248CA8 lip has three unequal rounded bulges separated by two
shallow notches over a dark contact line. Behind it, three joined soft
tissue fields have two branching #1E2798 seams and broad continuous
connections. Two short #42C5F8 highlight clusters occupy raised folds,
with a large quiet matte area between them. The rear margin thins into
soil beneath a dark ledge. Show the resting carpet following the ground
contour, its near lip visible and open space above the surface; let the
carpet continue beyond the side edges of the crop.

### VP-R3 · Low folded lamina

**Silhouette:** a continuous thin carpet interrupted by two offset rolled lips and one dark inlet.

**Body.** At the near margin, one broad teal `#248CA8` tissue sheet lies
against the soil, its edge interrupted by a deep irregular inlet.
Two thick rounded tongues flank that inlet, one projecting farther
toward the viewer. Behind them, the surface lifts into two low rolled
folds, offset along a diagonal and separated by a broad flat field.
Their exposed undersides are deep blue `#1E2798`; the upper lamina is
muted teal with two short cyan `#42C5F8` facets along the highest rolls.
One branching depression continues behind the second fold into a quiet
rear surface. The whole carpet keeps a 6:1 width-to-height aspect at
full coverage, with rolls contained inside the 1-voxel height. Its
texture is finely fibrous within the broad forms. At rest the folds
overlap loosely and the rear tissue adheres to the ground. Gentle sway
lifts only their free edges. Drying draws both rolls tighter and lowers
their outer surfaces; thinning opens the front inlet farther back.
Neighbouring sheets merge through flat margins, so coverage reads as
one irregular living surface beneath the canopy.

**Qwen.** A continuous velvetpad ground surface under canopy shade,
aspect 6:1 width to height, seen slightly from above. One matte #248CA8
sheet follows damp soil. Its near edge has two unequal rounded tongues
separated by a deep irregular inlet. Behind them two low rolled folds
sit diagonally offset, with a broad flat field between them. Their
undersides are #1E2798; two short #42C5F8 facets mark the upper rolls.
One branching depression continues into the quiet rear field. Fine
fibres stay within these broad tissue masses. Show the resting carpet
with a visible dark contact edge and empty space above; continuous flat
margins join further carpet at both sides of the crop.

## Vaulttree

> 3/5 stars. looks okay but just feels like one of those africa trees in the illustrations. would need improvement.

**Body need.** Qwen flattened the separated crown into familiar spreading
canopies, weakening the vertical vault. Keep the canopy builder's deep,
aerated, adequately watered soil niche, costly long-lived trunk, establishment
gap and eventual dead-wood line. Height stays 18–28 voxels, width 10–16:
72–168 × 40–96 px at the two reading scales. A full adult is 7:4 height
to width. Rooted support, separated edible lobes, hanging growth and sky
gaps matter; slow lobe sway supplies motion. The revisions move the crown
mass vertically and expose its load-bearing branches. Canopy loss still
widens light gaps and exposes wood rather than changing the species' job.

### VT-R2 · Ascending chamber crown

**Silhouette:** a tall crooked trunk with five upright foliage lobes at successively different heights.

**Body.** At the near base, one thick plum `#2A0E4A` trunk enters the
soil through a flared foot. Its violet `#3A1A7A` face is divided into
three long bark plates by two `#12093A` seams. The lower half rises
alone; above it, five limbs leave at staggered heights and curve
outward before turning steeply upward. Clear sky separates successive
limbs. Each ends in one thick, vertically elongated foliage lobe,
five lobes total, with four irregular openings across the crown.
The near-right lobe sits lowest and the rear central lobe highest.
Overall height to width is 7:4. Each lobe is `#2B6AD0` over an
`#1E2798` underside, its outer face divided by one deep longitudinal
fold and a short `#42C5F8` upper facet. Two loose teal `#248CA8`
filament bundles hang from the lowest limbs, with open air between
them. At rest the heavy lobes lean toward their supporting curves.
During sway each lobe turns slightly around its attachment while
the drapes lag below. Thinning contracts the foliage around the
branch ends and enlarges the vertical sky openings.

**Qwen.** One mature vaulttree rooted in a grove gap, whole height visible,
aspect 7:4 height to width. A thick #2A0E4A trunk with three #3A1A7A
bark plates and two #12093A seams rises alone through its lower half.
Five upper limbs depart at staggered heights, curve outward and turn
steeply upward, with sky between them. Each supports one upright
elongated #2B6AD0 foliage lobe, five total, divided by four irregular
sky openings; the rear central lobe stands highest. #1E2798 undersides,
one longitudinal fold and a short #42C5F8 upper facet per lobe. Two
#248CA8 filament bundles hang separately from the lowest limbs. Show
the resting crown with heavy lobes leaning into their curved supports.

### VT-R3 · Inward vault with hanging lobes

**Silhouette:** four rising branch tips enclosing a tall central opening and uneven pendant crown masses.

**Body.** The near base is a single plum `#2A0E4A` trunk with a narrow
violet `#3A1A7A` face, its bark split into broad, overlapping vertical
plates. Halfway up, four thick limbs diverge at different heights,
then curve upward and inward. Two nearer limbs frame a tall central
sky opening; two farther limbs remain visible through it across
separate gaps. Each branch tip carries one rounded foliage lobe on
its inner lower side, four lobes total. The lobes hang at unequal
levels, with open sky separating all four masses. The adult retains
a 7:4 height-to-width aspect. Foliage is deep blue `#1E2798` below
and `#2B6AD0` on its convex outer face, with a short cyan `#42C5F8`
facet beside the attachment. Two teal `#248CA8` drapes hang from
the lowest limbs on opposite sides of the central opening. At rest
the branch curves bear the pendant masses and their tips remain
visible above them. Sway moves the lobes inward and outward at
different rates. Thinning exposes more of each inner branch curve,
gradually widening the view through the vault.

**Qwen.** One mature vaulttree in a grove, rooted base and whole crown
visible, aspect 7:4 height to width. One #2A0E4A trunk with broad
#3A1A7A bark plates rises through the lower half. Four upper limbs
diverge at different heights, then curve up and inward. Two near
limbs frame one tall central sky opening; two rear limbs show through
separate gaps. Each tip supports one rounded hanging #2B6AD0 foliage
lobe on its inner lower side, four lobes at unequal heights with sky
between them. #1E2798 undersides and short #42C5F8 attachment facets.
Two separate #248CA8 drapes hang from the lowest limbs. Show the
resting tree with branch tips visible above the pendant foliage masses.

## Glowcap

> 3/5. the top side is just a fleshy pink.. not very interesting. they just read as regular mushrooms here.

**Body need.** The rendered crowns retained a simple cap-and-stalk contour
and large plain pink tops. Glowcap remains a wood-feeding saprotroph with
local paid luminous tissue, rooted through a split in identifiable dead
wood; its niche includes the existing litter/dead-wood substrate context.
Keep growth in the ladder's 1–2 voxel height band over the wood, ordinarily
1.5 voxels high and wide here. At roughly 6–9 px per crown, shape and
one broad top feature must survive. Feeding occurs through the attached
colony; movement is passive flex and growth or depletion changes fullness.
The revisions use folded tops and recesses, with cyan confined to the
tissue where the crown meets its underside shadow. Wood and colony stay
a composite, and brightness still follows the existing reproductive state.

### GC-R2 · Ridged asymmetric saddle

**Silhouette:** a high left shoulder, deep central hollow and lower right shoulder attached directly to split wood.

**Body.** At the near attachment, one plum `#510B6D` collar spreads through
the log's split, bordered by a short lilac `#B99BE6` lip. A squat neck
continues directly into the mature crown. Its near edge rises into
one high left lobe, descends through a deep central hollow and rises
again into a lower right lobe. Across the top, three broad violet
`#3A1A7A` ridges bend with this saddle, separated by two deep blue
`#1E2798` channels. Crown width to depth in side view is 3:2; including
collar and neck, the fungus is 1:1 width to height. The folded surface
is matte, with one lilac facet on the high shoulder. A narrow cyan
`#42C5F8` strip lies where the crown meets the dark `#2A0E4A` underside.
Behind the mature crown sits one smaller folded crown and one closed
button, separated by visible bark. At rest the collar grips the split
and the two shoulders hold unequal heights. Passive flex raises the
lower shoulder slightly; declining tissue folds both lobes downward.
Two dark mycelial threads continue rearward along the wood seam.

**Qwen.** One glowcap colony growing through a split dark log. The
mature fungus has aspect 1:1 width to total height; its crown alone
is 3:2 width to height. A #510B6D collar with a #B99BE6 lip joins
directly to a thick folded crown: high left lobe, deep central hollow,
lower right lobe. Three broad #3A1A7A top ridges have two deep #1E2798
channels between them; one pale facet marks the high shoulder. One
narrow #42C5F8 strip lies where the crown meets its #2A0E4A underside
shadow. Behind it one smaller folded crown and one button stand
apart across visible bark. Two mycelial threads follow the split.
Show the resting colony firmly attached to identifiable dead wood.

### GC-R3 · Open-seam crown

**Silhouette:** two unequal rolled crown halves enclosing one dark diagonal opening.

**Body.** At the near edge, a thick violet `#510B6D` collar emerges
through torn wood fibres and widens upward into two joined fleshy
lobes. A deep diagonal opening separates their upper halves, exposing
the dark plum `#2A0E4A` interior. The nearer lobe curls outward and
down; the rear lobe rises higher and curls inward. Their outer faces
are blue-violet `#3A1A7A`, divided into two broad facets apiece by
one `#1E2798` crease. Pale lilac `#B99BE6` marks the rolled tips.
The crown is 3:2 width to height; collar included, the full fungus
is 1:1. One continuous cyan `#42C5F8` strip follows the lower-facing
outer fold where the flesh meets underside shadow. Behind the mature
crown, one narrow juvenile fold rises from the same collar, separated
from it by a visible wood notch. Matte ridged tissue and quiet dark
interiors hold the contrast. At rest the unequal lobes enclose the
opening. Passive flex shifts their rolled tips slightly; as tissue
is depleted, the opening narrows and both lobes fold toward the
collar while the attachment remains embedded in the split.

**Qwen.** One glowcap colony attached through the split of a fallen
plum log, total fungus aspect 1:1 width to height, crown aspect 3:2.
One #510B6D collar widens into two joined unequal #3A1A7A crown lobes
with a deep diagonal #2A0E4A opening between their upper halves.
The near lobe rolls outward and down, the rear lobe upward and inward.
Each outer face has two broad facets divided by one #1E2798 crease;
#B99BE6 marks the rolled tips. One narrow #42C5F8 strip follows the
outer fold where it meets underside shadow. One smaller juvenile fold
stands behind across a visible wood notch. Show thick matte folded
tissue, the resting open seam and firm attachment through torn bark.

## How to read and pick

Pick a body and its ordinary resting posture first, then read the feeding or
moving posture to see whether the anatomy earns its place. LJ-D2, CH-A2,
CG-A2, RS-B2/RS-D2 and SP-AB2 are the closer continuations of the named
directions. The lateral lure, opening chorister breast, loftstrider feeding
mantle, grounded spore mantle, seedporter's free carrying hands and the
more folded plant forms offer departures to compare. Surprise is an option,
not a reason to prefer it.

At 4–6 px per voxel, check the silhouette sentence against the image. A
capgnawer's bristles may become one edge texture; its feeding notch still
needs to survive. The tiny ripple-snail can lose individual ridges while
keeping the front opening and cleared trail. At the larger sizes, count
the limb attachments and inspect the gaps, jaw direction, membrane joins
and exposed supporting ground. Those checks distinguish a rendered body
from an attractive image of a different animal.

Use the codes to choose, combine or reject: for example, a body's trunk
from one option and its feeding end from another can become a fresh prose
revision before generation. Judge the animal beside its food and the
plants beside neighbouring growth at the unchanged ladder scale. The Qwen
paragraphs specify one readable pose each; the longer prose supplies the
rest/motion counterpart for a later exploration. A preference here remains
a preference until Wrysk explicitly decides it.
