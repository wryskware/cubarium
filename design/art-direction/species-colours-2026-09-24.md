---
design_status: proposal
last_reviewed: 2026-10-01
decision_refs: []
---

# Species colours: the colour source of truth (2026-09-24)

**For colour, this file overrides the species dossiers
(`species-dossiers-2026-09-21.md`, D15) and Astra's prose
(`design/animal-body-reimagining-2026-09-24.md`).** Those documents still bind for
**parts, counts, shapes, surfaces and states**, subject to the later selected-appearance
notes and linked artbook picks. Where a
dossier names a colour for a part, look the part up here instead. The terrain, sky and
water palette stays as the art direction and `design/appearance.md` give it.

Why: Wrysk preferred the 2026-09-23 colour pass to the dossier blues ("I think these
colours are honestly better than the dossier"). He then chose colourway 1 for the
animals ("top row for everything, except bellwing … keep the original").

## Plants: the colour pass

Source: `crates/cubarium/src/voxel/colours.rs` (`recipe`). Keep the two in step: when one
changes, change the other. Ramps run from the **base** (dark, saturated) to the **tip**
(bright), and shadows lean toward deep indigo `#140A3C`, not black.

**Sampled 2026-10-01 from the artbook pick** (model pass, MP-P;
`design/handoffs/voxel-model-pass-2026-10-01.md`): each part's ramp is eyedropped from
the selected image in `art/gen/artbook/` (bloomcrown pink; velvetpad provisional;
glowcap the mushroom only). The bake material that carries each part is in brackets;
`colours.rs` keys its per-part ramps by that name.

| species | stem / trunk (base → tip) | foliage parts (base → tip) | glint | other |
| --- | --- | --- | --- | --- |
| bloomcrown | `#2E035F` → `#5C1EAF` [violet] | vanes hot pink `#8B0251` → `#FD48D1` [p2]; basal straps periwinkle `#5D55C2` → `#8582F3` [p0] | `#FDC6F2` | pod (core, glows) `#FED9F2` [p1]; orange tip (bloom) ripe `#FEAC42` → `#FEE6EB`, unripe = pod |
| umbrellafrond | stem `#4D1378` → `#7823B4` [violet]; collars `#51176D` → `#7E2C9D` [plum] | fronds teal `#00696F` → `#029894` [p1]; ribs `#4DE0C4` → `#DCFDEE` [p2] | `#DCFDEE` | — |
| springturf | — | tall rank `#037B7E` → `#48F5BE` [p2]; short rank `#023A47` → `#037B7E` [p0]; violet between `#20054E` → `#32166B` [violet] | `#A4FCDB` | — |
| stonecushion | root mat `#390542` → `#561470` [detritus] | lobes `#7A4ED0` → `#DABBFD` [lilac]; pores cyan `#01B8EC` [p2] | `#F4E6FD` | — |
| velvetpad | — | pad `#331D47` → `#48295E` [violet]; lip `#5F3D8D` → `#8F60B8` [plum]; moss cobalt `#1B296C` → `#3866DA` [p1]; buds `#4594F0` → `#75F1F8` [p2]; channels `#75F1F8` [teal] | `#75F1F8` | — |
| glowcap | stalk `#554199` → `#35256D` [detritus] | cap `#4426A1` → `#764BD7` [violet]; dark spots `#291654` [p0]; pale spots `#B078EF` [lilac]; speckles `#F607FC` [magenta]; brim and underside `#2FB0E2` → `#3FF9FC` [p2] | `#B078EF` | lip glow `#3FF9FC` (ripe) / `#2FB0E2` (fruiting) |
| vaulttree | lit plates `#7B46CA` → `#6C3EB9` [plum]; mid plates and limbs `#5F36A9` → `#4A2D8E` [violet]; dark plate `#01261F` [detritus] | shelves dark teal `#014C4D` → `#01696E` [p1]; rims mint `#50C5A9` → `#96F9DC` [p2] | `#96F9DC` | tendrils (drape) `#0197B1` → `#00CEEC` at the tips [teal] |
| lanternberry | `#2E0944` → `#4E178C` [violet] | leaflets blue `#013793` → `#18ADFC` [p1] | `#53F1FD` | lantern ripe `#FDA243` → `#FDEC9D` (glows `#FDA243`), unripe deep plum `#370A49`; seams magenta (not voxelized) |
| siphonreed | — | stems `#014B74` → `#23A6BC` [p1]; swollen shafts celadon `#57C5C9` → `#A5EED3` [lilac]; fan heads pink `#EB2189` → `#FD4FB6` [p2] | `#EEFCE5` | — |

**Latticevine** keeps its dossier colours (D15; the tile set Wrysk liked):
- foliage `#1E2798` / `#2B6AD0` / `#42C5F8`;
- runner `#3A1A7A` / `#2A0E4A`;
- holdfast `#510B6D`;
- bell `#B99BE6` with a cyan `#42C5F8` mouth;
- beads magenta `#FF2AFC` with cyan seeds.

**Later concept picks (2026-09-24):** the [artbook](../../art/gen/artbook/README.md)
supersedes this table where its selected plant images differ. Bloomcrown keeps
both crown alternatives, with hot pink and a pale pod preferred. Lanternberry
returns to blue foliage instead of acid lime. Vaulttree uses dark teal foliage
shelves with mint rims rather than emerald lobes. Glowcap has a purple spotted
cap, cyan brim and underside, and cyan **fungal mycelium in the fallen wood**;
the host wood has no teal veins of its own. Umbrellafrond retains teal foliage
in a different frond form, and velvetpad's blue moss and buds remain provisional.
The plant table above now carries hexes sampled from those picks
(2026-10-01); it replaces the 2026-09-23 colour-pass plant hexes.

## Animals: sampled 2026-10-01 from the artbook pick

Model pass MP-A (`design/handoffs/voxel-model-pass-2026-10-01.md`). Every hex is
**sampled 2026-10-01 from the artbook pick** (`art/gen/artbook/`, the README's selected
image): a median of a 9 x 9 pixel patch, or single pixels on thin parts. Each part is a
ramp **low → high** (belly → back, base → tip) by the cell's height in the model. The
renderer's copy is `crates/cubarium/src/voxel/colours.rs` (`FRONDGRAZER`, `LITTERSHREDDER`,
`LANTERNJAW` for `animal()`; `CHORISTER` … `BELLWING` for `specimen()`), the builders' copy
`scripts/blender/animal_models.py` `COATS`, by the same part slots (`a_main` …).

| animal (pick) | main | shadow / dark | pale | marking | limbs, feet | accents and other parts |
| --- | --- | --- | --- | --- | --- | --- |
| chorister (`chorister.png`) | flank `#6C55D3` → `#6C55D5` | back `#312675` → `#241B56` | countershade `#B9A8E2` | rosettes `#261F5B` | legs `#30256E` → `#6C55D5`, claws `#2A2061` → `#342878` | crest lobes `#B7AAE1` → `#EBDDFC`, eye `#D70FE3` → `#FD22FE`, muzzle `#55C5FB` → `#52C4FC` |
| ripple-snail (`ripple-snail-organic.png`) | shell `#416EE4` → `#5081F2` | foot `#1B234C` → `#252D5A` | mantle edge `#7FA1F4` → `#96B5FC` | ridge grooves `#172B6B` | palps `#5E75CE` → `#5F5BB9` | palp tips `#FC56CE` → `#FC57D0`, front opening `#0A0D2A` |
| capgnawer (`capgnawer-rose-bark.png`) | mantle `#9C607A` → `#A76982` | mouth `#010000` | fringe `#EFA8C1` → `#F8D6EC` | bark streaks `#6B4E63` → `#78596E`, spots `#E3BCF8` → `#ECCAFB` | body, legs `#B4768E` → `#B7778F`, toes `#BD92BF` → `#F0D1F0` | mandibles `#BB95BB` → `#F5E5F4`, sensor patches (light pink) lower `#FAB6E4` → upper `#FEC7EF` |
| seedporter (`seedporter.png`) | skin `#5C6A28` → `#889335` | open mouth `#361D13` | underside, face `#BEB55E` → `#EADE97` | dorsal stripe `#21361D` → `#2D441F` | claws `#191817` → `#30321A` | eyes `#0C0707`, membranes `#DBDA7B` → `#EDEAA5`, fruit `#802B51` → `#D6658A`, cheek dots `#B5376A` → `#C13E7B` |
| frondgrazer (`frondgrazer.png`) | shield `#EE7156` → `#FB8162` | hoof band `#58342D` → `#5D3B31` | belly skirt `#BB7A5E` → `#DF9F7D` | saddles `#872935` → `#902C3A` | legs, neck `#9B4341` → `#D96862`, hooves `#C0927C` → `#CA9B86` | scoop lining `#FED3AC`, stalk-eye lens `#300919` → `#4E162B`, rim `#EE699E` → `#F370A2`, sensor spots `#C83C66` → `#D8456F` |
| lanternjaw (`lanternjaw.png`, selected 2026-10-01; k-means of the pick's opaque pixels) | plates `#5E4070` → `#9168A1` | joints, underside `#1B161B` → `#2F2433` | fangs, razor blades `#B691BF` → `#E5D6ED` | lure holders `#4A4452` → `#695787` | legs, striking arms `#5E4070` → `#865D9C`, claw tips `#2A2530` → `#4A4452` | eyes `#B71984` → `#E437C1`, lure orbs `#63EEFC` → `#ADFAFD` (pigment; they do not glow yet) |
| littershredder (`littershredder.png`) | plates `#F4AD31` → `#FED24E` | bands `#5A2D10` → `#693819` | underside, legs, feelers `#E6C690` → `#FEEEC0` | — | knees `#C06F0E`, claws `#6D3101` → `#E5A537` | sensor spot `#A30E57` |
| **bellwing** (`bellwing.png`), original colours | bell `#5E1764` (mouth) → `#2E0D3E` (crown) | — | wings `#A580F2` → `#C19BFC` | — | wing roots `#9C547D` | rim and underside `#C0099E` → `#E20CAF`, leading edges `#5ABCEF` → `#95E7FC`, tendril (muted peach, not yet shifted yellower) `#EEB4A7` → `#F1C4B9` |

### Colourway 1 (2026-09-24, superseded for colour by the sampled table above)

Source: `runs/animal-bodies-2026-09-24/README.md`, v2 and v3 sections. The columns are
**main / shadow / pale secondary / marking / accent**.

| animal | main | shadow | pale | marking | accent | markings |
| --- | --- | --- | --- | --- | --- | --- |
| chorister (CH-A2 v3), violet stalker | `#6E5CD6` | `#2E2470` | `#D6CCF6` | `#1C1450` | jaw `#42C5F8`, eye `#FF2AFC` | countershading (dark back, pale belly and head) with soft rosettes; **no bands** (v3) |
| ripple-snail (RS-D2), azure dome | `#4A7CF0` | `#141A40` | `#A8C8FF` | `#2A50C0` | lobe tips `#FF5AD8` | two concentric growth rings on the dome |
| capgnawer (CG-D2), rose-mauve | `#9A5A8C` | `#4A2446` | `#F0C8E4` | `#E6A6D8` | eyes `#FF4AC0` | pale spots over the mantle |
| seedporter (SP-AB2), moss | `#5A9A3A` | `#23461E` | `#D8F0A8` | `#162E12` | `#FF2AFC` | dark dorsal stripe head to tail, pale belly, dark outer panel on each membrane |
| frondgrazer, coral | `#FF8A6E` | `#A22A4A` | `#FFE0CC` | `#6A1A30` | sense patches `#FF2AFC` | dark saddles on both shield lobes, pale underside |
| littershredder, gold | `#FFD860` | `#B06A18` | `#FFF4C8` | `#5A3208` | sense dot `#FF2AFC` | alternating dark segment bands, pale underside |
| **bellwing A, original** | bell `#2A0E4A` | — | vanes `#B99BE6` | — | rim `#FF2AFC`, vane leading edge `#42C5F8` | none; Wrysk: keep the original, don't recolour |

### Selected animal exceptions (2026-09-25)

The [artbook](../../art/gen/artbook/index.html) records the exact selected images.
These visual exceptions take precedence over the earlier table where they differ;
they do not mean the renderer palette or body models have been updated.

- **Capgnawer:** rose-mauve with layered bark-like markings and pale spots; all four
  smooth sensor patches should be light pink, not red or dark magenta. Exact revised
  sensor hex remains open.
- **Seedporter:** keep the chosen moss-green image plus the explicitly retained
  compact-head alternate. Orchid is an exploratory study, not a replacement.
- **Frondgrazer:** coral/wine shield and pale underside retained; new stalk-tip
  sensory lenses are dark plum with magenta rims.
- **Littershredder:** gold/brown palette retained on the selected longer tapered body.
- **Bellwing:** original bell and wing colours retained. Only the fine corkscrew
  siphon changes: muted peach, to lean a little more yellow in the next refinement.
  The vivid yellow and saturated orange trials were rejected. Exact target hex is open.
- Painted **outer halos** around the selected assets are cleanup defects, not
  intended body emission. Use actual alpha transparency for the cutouts.

### Kept alternates (colourway 2; deferred backlog §8)

Further alternate generation is deferred by Wrysk (2026-09-25). The existing orchid
seedporter study is retained in the artbook as exploration. Wrysk previously liked
the two proposals in bold; neither is a new primary selection.

| animal | alternate | colours | markings |
| --- | --- | --- | --- |
| **seedporter** | **orchid pink** | `#A8429E` / `#4A1646` / `#FFD0F4` / `#F8A0E8`, cyan `#5FF0FF` | pale spots on back and membranes, cyan cheek patches |
| **frondgrazer** | **periwinkle** | `#6A7AF0` / `#2E2A8A` / `#E0E4FF`, gold spots `#FFD860`, `#5FF0FF` | gold spots over the shield |
| chorister | rust | `#D8662A` / `#6A2410` / `#F4E4CC` / `#3A1408`, `#42C5F8`, eye `#FF2AFC` | dark rosettes on flanks and tail base |
| ripple-snail | jade | `#3AC89A` / `#0E3A30` / `#D8FFF0` / `#0E7A5A`, `#FF5AD8` | eight radial stripes |
| capgnawer | lichen | `#C8B040` / `#5A4A10` / `#F8ECB0` / `#6A5410`, `#FF4AC0` | one dark saddle band |
| littershredder | teal | `#4ACAB8` / `#0E5A5A` / `#D8FFF6` / `#0A2A2A`, orange `#FF8A3A` | dark dorsal stripe |

## Glow

Which parts emit light, and in what colour, is in `colours.rs` (`plant_emission`,
`vine_emission`). Those use the dossiers' luminous hexes for the lighting tier. Painted
art should show the same parts glowing in those colours.
