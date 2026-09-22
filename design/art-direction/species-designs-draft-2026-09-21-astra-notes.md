# Astra review, species draft 1

Strong starting identities, but neither sheet is generation-ready. G1’s silhouette remains familiar mushrooms on timber; F1’s upward-feeding mechanism is unresolved. The wording below is proposed for Wrysk’s choice, not an accepted redesign. Pixel dimensions use the sheet’s provisional 8 px/voxel.

## G1 — Glowcap on a fallen log

**1. Ambiguities and exact replacements**

- **“The caps are few and the log is the subject’s body.”** This conflates organism and substrate. Replace with: “Four fungal fruiting bodies share a mycelial attachment on a fallen dead trunk. The trunk supplies the composition’s largest mass; the fungus remains a separately identifiable colony.”
- **“Three or four jagged fibre spikes of unequal length.”** Replace with: “Three blunt fibre splinters project left from the broken end; their exposed lengths are 4, 2 and 3 logical pixels, ordered top to bottom. Their tips taper to single pixels.”
- **“Log 6 voxels long × 1.5 tall… Whole sprite 48 × 22.”** The width’s inclusion of splinters and the height’s padding are unresolved. Replace with: “The complete log silhouette, including splinters, occupies 48 × 12 logical pixels. Its depth is 10 logical pixels. The highest fungal crown rises 8 pixels above the bark; the 48 × 22 frame contains the resulting 20-pixel-high silhouette with one clear row above and below.”
- **“A shallow dome… rim flared and turned slightly up, a soft point at the centre.”** These cues permit umbrella, nipple and bowl shapes. Replace with: “The mature crown is an asymmetric saddle, 8 pixels wide and 4 pixels high, carried by a 3-pixel-wide, 4-pixel-high stalk. Its left lip rises 2 pixels above its right lip; a broad central depression joins the two rounded lobes.”
- **“Two half-size caps flanking cap_1… at the base of the cluster.”** Replace with: “Measure x from the sprite’s left edge. The mature stalk attaches at x=14 on the upper bark ridge; the near juvenile attaches at x=20 on the visible flank; the far juvenile attaches at x=8 behind the upper ridge; the button attaches at x=17 beside the mature stalk. Juvenile crowns measure 4 × 2 pixels; the button measures 2 × 2.”
- **“Six or seven lengthwise ridges… two smooth pale lilac patches.”** Count, contrast and patch extent materially change the result. Replace with: “Three interrupted bark ridges follow the trunk’s long axis. Two exposed-wood patches occupy 5 × 2 pixels near the break and 4 × 2 beneath the stub. Each patch follows the taper of a missing bark plate.”
- **“Three to five single cyan pixels scattered on each cap top.”** Replace with: “Each crown has a continuous cyan strip along its lower-facing lip. The upper crown is a quiet violet mass.” Scattered highlights are both unspecified and expensive at this scale.
- **“Two tones per part” / “half strength” / “darkened toward black.”** These leave colours and alpha to the renderer. Replace with: “Use opaque fills. Bark uses #2A0E4A above and #12093A beneath; exposed wood uses #510B6D with a short #B99BE6 torn-edge highlight. Crowns use #3A1A7A above and #2A0E4A beneath; their luminous lip uses #42C5F8. Mycelial strands use #510B6D.”

**2. Art-direction fit**

The clean ringed end, pointed domes and conventional stems collectively read as Earth mushrooms with cyan trim. The saddle crown above supplies a distinctive silhouette while leaving its unusual folded surface unexplained. Keep its edges broad and fleshy: rounded lobes and blunt wood fibres support curiosity without pushing toward teeth, wounds or mascot faces.

**“The bottom edge is straight”** also under-serves organic form. Replace with: “The underside has two broad contact flats beneath the left and right thirds, joined by a shallow concave bark edge. Each flat carries a short #12093A contact shadow.”

**3. Ecological read at 24 px wide**

The identifiable log already carries the dead-wood niche. The single strongest addition would be **a shared attachment collar spreading from the stalks into a bark split**: specify a 6 × 2 logical-pixel lobed collar, #510B6D with one lilac edge. It makes the fungus visibly grow *through* the wood; the present hairlines will usually disappear at half size. Keep exposed wood darker than the luminous crowns so the attachment supports the colony rather than competing with it.

**4. Rig fitness and states**

- Merge `[stub]` into `[log]`; both remain rigid. Keep `[mycelium]` as a registered overlay for substrate/state changes. Keep each crown and its stalk merged as one interchangeable growth part; static poses provide little reason to articulate their junction.
- Record the four attachment anchors and each part’s depth order. The far juvenile needs its full hidden base painted beneath the bark overlap; the near juvenile renders over the flank.
- **“Bare… mycelium threads only”** should become **`colonised`**. Define `buttons` at the existing cap_1 and cap_4 anchors so growth preserves colony identity.
- **“Spent… flat dark discs… threads brighter and longer.”** Replace with: “Spent crowns retain their attachment positions and fold their two lobes downward to half their fruiting height, coloured #510B6D. Stalks and mycelial extent retain their fruiting geometry.” Brightening and spreading currently imply renewed fungal activity without an identified state.
- Specify the log’s camera angle too. Its oblique end face, upper bark and near/far colony placements require a common projection even though it is static.

**5. Alternative direction — five lines**

A split trunk carries three overlapping fungal shelves projecting from its exposed flank.
Each shelf is a broad, folded fan with a thick rounded attachment and a scalloped outer edge.
The largest fan spans 12 logical pixels; two smaller fans step downward toward the broken end.
Dark plum upper surfaces shelter a narrow cyan underside crescent; lilac tissue joins the fans inside the split.
Growth opens tight pleats into shelves; spent shelves curl inward into persistent dark brackets.

## F1 — Frondgrazer

**1. Ambiguities and exact replacements**

- **“Horseshoe crab crossed with a tortoise: nothing of the deer.”** This invites borrowed anatomy and uses the rejected negative construction. Replace with: “A broad, low browser carries three overlapping dorsal plates above six weight-bearing legs, a short forward feeding wedge and a blunt rear spur.”
- **“Body 6 voxels long × 2 tall… Head adds 1 voxel… raised, it lifts 1 voxel.”** An 8-pixel head rotating 30° raises its tip only 4 pixels. Replace with: “The torso occupies 48 × 24 × 16 logical pixels in length, width and standing height. The head projects 8 pixels beyond its hinge. Cropping lifts the head tip 4 pixels through a 30° upward rotation; the extended palps reach a further 4 pixels above that tip.” Record the hinge height to establish actual reachable foliage.
- **“A blunt wedge, wider than tall.”** Replace with: “The head measures 8 pixels from hinge to tip, 6 across and 4 high. Its upper face slopes gently toward a rounded tip. A shallow feeding cleft opens across the front-upper edge.”
- **“The underside carries the mouth… a vertical slit… beak… darker notch.”** These describe incompatible possible mouth arrangements, and the crop pose needs an opening jaw. Replace with: “The feeding cleft has a fixed upper cutting edge and a separate rounded lower jaw. During cropping the lower jaw rotates downward 20° around its rear hinge, exposing a 2-pixel-deep #12093A mouth interior.”
- **“Each a curled stalk 0.5 voxel long.”** Replace with: “Each palp is a 4-pixel-long, 1-pixel-thick tapered stalk rooted at one mouth corner. At rest its tip folds inward beside the jaw; while cropping it extends upward and forward, with the two tips remaining separate.”
- **“Three pairs of short columnar legs” / “near three show fully.”** Their lengths, placement and exposure remain open. Replace with: “Leg pairs attach 10, 25 and 39 pixels behind the torso’s front edge. Each leg has two stout 3-pixel segments and a 4 × 2-pixel two-toed pad. In side view the rim covers the upper joints; the lower segments and pads form three distinct supports.”
- **“A row of six… spots along the shell rim.”** Is this six total or six per side? Replace with: “Each flank carries six flush sensory pits, grouped into three close pairs above the three leg roots. Each pit occupies 2 × 2 pixels at the working scale; each pair shares a dark inset.”
- **“Three-quarter away (rear skirt, tail, plate seams sweeping toward the viewer).”** This could become a rear portrait that loses the feeding end. Replace with: “Use orthographic projection at 20° camera elevation. With side view facing right defined as yaw 0°, render yaw 45° toward the camera and yaw 45° away. Preserve the same anatomy and world dimensions; the away-facing head remains attached at the far front of the shell.”

**2. Art-direction fit**

The combination of a tortoise dome, straight front lip and continuous neon seams risks a familiar armoured animal decorated like equipment. Replace **“straight vertical lip”** with “a broad arched opening around the head, with rounded shell corners descending beside the hinge.” Let the plates overlap through dark, curved shadow bands.

Replace **“rim band and the two plate seams in electric blue”** with “a short cyan edge on each front-side shoulder; the remaining rim and seams use the shell’s violet ramp.” This restores a quiet body mass. Grouped, flush sensory pits supply unfamiliar anatomy without turning the head into a face or the shell into a string of lights.

**3. Ecological read at 24 px wide**

Low weight and terrestrial support read well; the current underside notch communicates upward browsing poorly. The one feature to prioritise is **a broad, upturned cropping cleft that breaks the front silhouette**, large enough to remain a visible two-pixel opening in the 24-pixel-wide check. Heavy planted pads support the walking niche; inability to swim ultimately needs shoreline behaviour, since a silhouette cannot reliably establish it. The biosphere row permits limited wading.

**4. Rig fitness and poses**

- Merge `[sense_row]` into the shell colour/mask artwork and keep the three shell plates in one rigid `[shell]` part. Split `[head]` into head and lower jaw. Give each palp two hinged sections or a deformable strip; rigid rotation alone cannot uncurl it.
- Split each leg into upper segment, lower segment and pad: 18 leg parts with unique near/far identifiers. Paint joint overlaps and hidden anatomy. Give the tail an explicit rear-root pivot.
- Define the tripods: left-front/right-middle/left-rear, then right-front/left-middle/right-rear. During each stance, those pads remain planted while the opposite group advances. Specify stride, lift and timing in the animation sheet.
- **“All six pads on the ground line”** flattens the three-quarter views. Replace with: “All resting pads contact one horizontal world plane; their projected screen positions follow the shared camera and each foot’s lateral offset.”
- Keep `drink` as a separate future requirement with its own downward hinge range. Generate registered parts for each angle; mirroring can supply the opposite heading within that angle.
- The warm feeding flash is a new visual behaviour. Prefer: “Palps retain pale lilac while guiding foliage; the jaw closes around the contacted leaf.” The mechanical action already makes the bite readable.

**5. Alternative direction — five lines**

A low browser carries a long, shallow saddle shield with a dipped centre and raised shoulder ends.
Six short legs spread outward beneath it, producing a broad footprint and visible gaps between supports.
A thick folded neck rises from the front into a transverse, upward-facing cropping scoop.
Indigo shield masses frame two magenta sensory patches; pale lilac marks the scoop’s inner cutting edge.
At rest the neck folds into the saddle opening; feeding unfolds it upward while all six pads brace.

## Shared sheet and palette

**6. Missing prompt fields:** Add body-axis conventions; complete length/width/height and per-view bounds; camera projection/elevation/yaw; anatomical landmark coordinates; exact counts and bilateral symmetry; light direction and explicit shadow colours; fixed versus variable markings; and the permissible variation between specimens. Separate projected pixel measurements from world dimensions.

Add production fields for assembled reference versus separated-parts sheet, part inventory, pivots, joint overlap, hidden-surface completion, depth order, frame registration, margins, background/alpha treatment and contact-shadow ownership. State whether pixel widths refer to logical artwork or generated output. Each angle needs the same specified pose inventory.

The inherited style block still contains negative instructions despite §1c. Rewrite the outgoing prompt positively: “Crisp stepped silhouettes, opaque flat colour clusters, sharply bounded luminous tissue, and an isolated subject on the specified extraction background.” Treat 8 px/voxel as this sheet’s working scale; use 24 px width as a readability check.

**7. Palette:** Keep pale lilac `#B99BE6` as a small exposed-tissue accent. Reject adding `#C9F6FF` for these two sheets: neither assigns it a necessary role. Consolidate `#2A0E4A` and `#2B1460` to one shared dark support unless an in-scene comparison demonstrates a useful distinction. Likewise choose one of near-identical `#42C5F8` and `#42C6FF` for the shared cyan.

Rename `#3A1A7A` **“mid violet”**, and `#42C6FF` **“electric cyan”** rather than “electric blue.” `#510B6D` is lighter than `#2A0E4A`; it cannot also serve as that plum’s “dark ridge line.” Assign shadow roles explicitly. Reserve the orange accent for a chosen visible tissue or approved event rather than introducing a bite flash by default.