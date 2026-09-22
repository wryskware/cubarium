---
design_status: proposal
last_reviewed: 2026-09-21
decision_refs: []
---

# Species design sheets (draft 1: glowcap on a log, frondgrazer)

The creative pass for Stage 2 sprites. Written by Fable, critiqued by Astra, decided by
Wrysk; the image generator renders these and imagines nothing. Cites
`Cubarium_Art_Direction_v0.1.md` (pillars 2, 3, 6; §05 organisms) and the ecological
roles in `design/theoretical-biosphere-2026-09-16.md` §4 ("readable form" column).
Everything here is a proposal until Wrysk marks it.

## Sheet format

Each subject: identity · scale · silhouette · parts (named, for the cutout rig) · colour
(hex from the palette family) · surface · poses and angles · ground contact · reads-as
(what a viewer should think, phrased as what to draw). Sizes are given in voxels
(0.125 m) and in logical pixels at 8 px per voxel; 4 px per voxel is half.

Palette family (from `design/appearance.md`, sRGB): floor `#12093A`; producer ramp
`#1E2798` → `#42C5F8`; detritus / dead matter `#510B6D`; body accent ramp `#FF2AFC` →
`#42C6FF`; the one warm accent `#FF9B50`. Working additions for these sheets, to be
confirmed: deep plum `#2A0E4A`, pale lilac `#B99BE6`, dark violet `#2B1460`, violet
`#3A1A7A`, cool white-cyan highlight `#C9F6FF`.

---

## G1 · Glowcap on a fallen log

**Identity.** A wood fungus fruiting on a section of dead trunk. Not a plant, not a
toadstool: the caps are few and the log is the subject's body.

**Scale.** Log 6 voxels long × 1.5 tall (48 × 12 logical px at 8 px/voxel). Tallest cap
rises 1 voxel above the bark (8 px). Whole sprite 48 × 22 logical px.

**Silhouette.** A horizontal, slightly tapered cylinder lying flat, thicker end on the
left. The left end is a torn break: three or four jagged fibre spikes of unequal length
pointing left. The right end is a clean, slightly oval cross-section seen obliquely, with
two concentric rings. One short broken branch stub rises from the top at one third of the
length from the right, angled 30° right. The caps cluster on the upper left half, leaning
toward the break.

**Parts** (rig names in brackets).
- [log] the trunk: bark in long plates. Six or seven lengthwise ridges, each a dark line
  with a lighter plate beside it; two plates have fallen away, exposing two smooth pale
  lilac patches of bare wood, one near the break, one under the branch stub.
- [stub] the branch stub: same bark, blunt torn tip.
- [cap_1] the mature cap: a shallow dome, width 1 voxel (8 px), height 0.5 voxel, rim
  flared and turned slightly up, a soft point at the centre. Stem thick and short, one
  third of the cap width, pale.
- [cap_2], [cap_3] two half-size caps flanking cap_1, one in front (lower on the log
  face, overlapping the bark), one behind and to the left, rim only just curling.
- [cap_4] a button: a closed knob on a stub of stem, at the base of the cluster.
- [mycelium] three or four thin pale threads running along bark cracks from the cluster
  toward the break, visible as 1-px lighter lines.

**Colour.** Bark plates deep plum `#2A0E4A`; ridge lines detritus violet `#510B6D`
darkened toward black in the underside third; exposed wood pale lilac `#B99BE6` with a
`#510B6D` edge; end-grain rings `#510B6D` on `#2A0E4A`. Cap tops violet `#3A1A7A`; a rim
ring one pixel wide in electric cyan `#42C5F8`; three to five single cyan pixels
scattered on each cap top; stems pale lilac `#B99BE6`; mycelium threads `#B99BE6` at half
strength. No warm accent on this subject.

**Surface.** Flat fills; two tones per part (lit upper half, shaded lower half) plus the
one-pixel `#12093A` outline on the whole silhouette. The cyan rim is the only emissive
read; there is no halo and the surrounding bark stays dark.

**Poses and states.** Static. Growth states as separate sprites: `bare` (log, stub,
mycelium threads only), `buttons` (log + cap_4 and one more button), `fruiting` (all
caps, the sheet above), `spent` (caps collapsed to flat dark discs `#510B6D`, threads
brighter and longer). No wind response.

**Ground contact.** The log rests on its full length; the bottom edge is straight and
touches the ground line, with a one-pixel darker contact shadow line under it.

**Reads as.** "A rotting alien trunk with a little colony living on it": dead wood
first, fungus second, light third.

---

## F1 · Frondgrazer

**Identity.** The low, heavy ground browser of the meadow. It crops foliage at head
height, walks slowly between patches, and cannot swim. A shelled grazer, built like a
horseshoe crab crossed with a tortoise: nothing of the deer.

**Scale.** Body 6 voxels long × 2 tall at the hump (48 × 16 logical px), 3 voxels wide
seen from above. Head adds 1 voxel forward when level; raised, it lifts 1 voxel.

**Silhouette (side view, facing right).** A wide, low dome: the shell is a flattened
oval, highest at one third from the front, sloping in a long line to a blunt rear skirt
that nearly touches the ground. The shell edge is a continuous rim all round; the legs
show only as short feet below it. At the front the shell ends in a straight vertical lip;
the head wedge projects from under that lip.

**Parts** (rig names in brackets).
- [shell] the carapace: three overlapping plates front to back, each plate's rear edge
  lying over the next, giving two visible seams that curve down toward the rim. The
  rim is a raised band one pixel wide all round.
- [head] a blunt wedge, wider than tall, flat on top, hinged at the shell lip so it can
  tilt up to 30°. The underside carries the mouth: a vertical slit ending in a small
  horny beak at the tip, drawn as a darker notch.
- [palp_l], [palp_r] two short feeding palps on either side of the mouth, each a curled
  stalk 0.5 voxel long that unfurls forward and up when cropping.
- [leg_f], [leg_m], [leg_r] (×2 sides) three pairs of short columnar legs ending in
  broad two-toed pads. In side view the near three show fully, the far three as darker
  shapes behind them.
- [sense_row] a row of six small round sensory spots along the shell rim, above the
  legs, evenly spaced; the animal has no eyes on the head.
- [tail] a short blunt spur from under the rear skirt, 0.5 voxel, used only in rest
  (tucked) versus walk (lifted a little).

**Colour.** Shell plates dark violet `#2B1460`, lit upper third `#3A1A7A`; the raised
rim band and the two plate seams in electric blue `#42C6FF` at one pixel; the sensory
spots magenta `#FF2AFC` (this is the inherited body accent; it may drift toward cyan in
kin). Head wedge `#2B1460` with a single magenta `#FF2AFC` stripe down the centre of the
top; beak notch `#12093A`; palps pale lilac `#B99BE6`. Legs `#2B1460`, pads `#510B6D`.
Feeding flash: while cropping, the palps and beak notch turn the warm accent `#FF9B50`
for the bite. No other warm colour.

**Surface.** Flat fills, two tones per plate, the one-pixel outline `#12093A`. The rim
and seams are the only bright lines; the sensory spots are single pixels at 4 px/voxel
and 2×2 at 8.

**Poses.** `rest`: head level, palps curled, tail tucked, legs planted. `walk`: a slow
tripod gait, two frames of leg alternation, shell rocks one pixel. `crop`: head raised
30°, palps unfurled toward a crown above, beak open; shell still. `drink` (later): head
lowered below the shell lip.

**Angles.** Side (mirrored for the other heading); three-quarter toward camera (the
shell's front lip and head visible, near legs foreshortened); three-quarter away (rear
skirt, tail, plate seams sweeping toward the viewer). Each angle carries the same three
poses.

**Ground contact.** All six pads on the ground line in rest; the shell's rear skirt
stops one pixel above it.

**Reads as.** "A slow armoured grazer that lifts its face into the plants": low, heavy,
which end is the head, and a mouth that reaches upward.

---

## Next sheets

Bloomcrown, umbrellafrond, springturf, stonecushion, velvetpad (growth states and sway,
no angles) once these two have a verdict, so the plant sheets inherit the surface and
outline decisions made here.
