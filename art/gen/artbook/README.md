# Cubarium artbook — concept picks, 2026-09-24–25 (lanternjaw, loftstrider and seedporter poses 2026-10-01)

[Open the visual artbook](index.html).

These are Wrysk's selected visual references, retained alternatives and terrain candidates. They are not finished texture masters or updated voxel models. Later selected appearances override conflicting older concept proposals within visual scope only.

## Remaining cleanup

- Remove painted outer halos/backgrounds where present; they are image artifacts, not creature emission. The gallery reports actual PNG alpha separately from appearance.
- Capgnawer: keep the four sensor patches light pink, with larger upper and smaller lower patches.
- Bellwing: shift the thin tendril a little yellower than the retained muted peach, without returning to vivid yellow/orange; preserve its spindly corkscrew.
- Velvetpad remains provisional. Bloomcrown retains both crown colours, leaning pink. Seedporter retains the exact selected image and the explicitly requested compact-head alternate.
- All terrain stays candidate material for a later detailed rendering pass. Litter needs further arrangements and densities using our plants' foliage. Further animal colourway exploration is deferred.
- Lanternjaw: where the image and the model differ (tail fork, fangs, razor blades), the model is the reference.
- Loftstrider: the image does not keep the sixes (legs, eyes, tusks, fronds) or the mixed frond sizes; the model is the reference.
- Seedporter poses: the images attach the membranes to the flanks or hips and the small arms near the shoulders; the models (membranes wrist to knee, small arms the middle pair) are the reference. The outer glow is unwanted (Wrysk) and needs cleanup.
- Reconcile rigging, exact counts, growth states and material palettes later. These picks do not imply engine changes.

## Plants

| Subject | Pick | Decision |
| --- | --- | --- |
| Bloomcrown | [pink crown](bloomcrown-pink-preferred.png) | Preferred of two retained alternatives: hot-pink upper vanes (more than six), pale blush-lilac pod, orange tip; original stem and basal leaf colours. |
| Bloomcrown | [original blue crown](bloomcrown-blue-alt.png) | Retained alternative; no final choice between the two yet. |
| Lanternberry | [blue foliage](lanternberry-blue.png) | Accepted; return to the original foliage colourway. |
| Springturf | [selected concept](springturf.png) | Accepted. |
| Stonecushion | [selected concept](stonecushion.png) | Accepted; alien mound between the flatter original and the overly succulent rosette. |
| Siphonreed | [pink fan heads](siphonreed.png) | Accepted; four slender stems with swollen upper shafts, rounded pink fan heads, no side nubs. |
| Umbrellafrond | [geometric fronds](umbrellafrond.png) | Accepted; straight, regular three-tier symmetry with teal lacy fronds and cyan ribs. |
| Velvetpad | [moss and buds](velvetpad-provisional.png) | Provisional pick; irregular cobalt moss tufts and smooth round blue-to-cyan young buds on a spreading violet pad. |
| Vaulttree | [layered canopy and drapes](vaulttree.png) | Selected; asymmetric violet trunk, dark teal canopy shelves, and long cyan tendrils. |
| Glowcap on fallen vaulttree | [spotted mushroom and split log](glowcap-on-vaulttree.png) | Selected; purple cap with flat dark spots, cyan brim and underside. Cyan threads in the split are glowing fungal mycelium, not veins in the wood. |

## Animals

| Subject | Pick | Decision |
| --- | --- | --- |
| Chorister CH-A2 v3 | [violet stalker](chorister.png) | Accepted. |
| Ripple-snail RS-D2 | [organic azure dome](ripple-snail-organic.png) | Selected concept; matte grown shell with two shallow growth ridges, pale mantle edge, dark foot and pink-tipped feeding palps. |
| Capgnawer CG-D2 | [rose-mauve bark mantle](capgnawer-rose-bark.png) | Selected concept; rounded twin mantle with layered bark texture, six mandibles in three vertical pairs, and two rows of smooth pink sensor patches on each side (larger upper, smaller lower). Keep the patches light pink. |
| Seedporter | [selected moss-green design](seedporter.png) | Selected exact tall-headed, open-mouthed image; six limbs, a small fruit-handling pair, larger membrane-bearing arms, rear gripping feet and a coiled prehensile tail. Keep the pictured proportions as the selected reference. Its model (`seedporter()`) is the climbing pose (Wrysk 2026-10-01); the three poses below share its body. Outer halo needs cleanup. |
| Seedporter, standing | [perched, eating](seedporter-stand.png) | Selected 2026-10-01 as the standing pose: a Qwen 2.1 edit of the selected image (Wrysk's pick of `SP-stand-3001-b3`; b2 was also circled). Perched upright on a branch, crouched, the big arms down to the perch, the membranes folded, the small arms holding the fruit to its mouth, the tail hanging below the perch. **Anatomy is Wrysk's, not the image's:** the membranes run from the big arms' wrists to the hind knees, and the small arms are the middle pair. The model (`scripts/blender/animal_models.py` `seedporter_stand()`, z = 0 under its feet, its tail below) is the reference. The selected image above is the climbing pose. Opaque background with an outer glow. |
| Seedporter, on the ground | [walking on all fours](seedporter-ground.png) | Selected 2026-10-01 as the ground pose (`SP-ground-side-3001-b2`; b3 and `SP-ground-34-3001-b1` were also circled). On all fours with the big arms as front legs, the membranes hanging along its flanks, the fruit held at the chest, the tail curling up behind. The model (`seedporter_ground()`) is the reference: membranes wrist to knee, a thinner tail, the hind legs set back. Opaque background with an outer glow. |
| Seedporter, gliding | [membranes spread, from behind](seedporter-glide.png) | Selected 2026-10-01 as the gliding pose (`SP-glide-back-3001-b3`; `SP-glide-back-b1`, `SP-glide-top-b1` and `SP-glide-side-b1` were also circled). Level and belly down, the big arms spread forward with the membranes taut, the tail straight behind. The model (`seedporter_glide()`) is the reference: the membranes reach the knees, the knees spread out, the small arms hold the fruit under the chest. Opaque background. |
| Seedporter | [compact-head alternate](seedporter-compact-head-alt.png) | Explicitly retained alternate: broader compact head, substantial muzzle and closed mouth, with lateral skull lobes. Does not replace the selected image. Outer halo needs cleanup. |
| Frondgrazer | [coral saddle shield](frondgrazer.png) | Selected; six short legs, continuous double-raised saddle shield, folded neck and transverse cropping scoop. Two stalk-tip sensors have dark plum lenses and magenta rims; shoulder patches remain. Outer halo needs cleanup. |
| Littershredder | [longer tapered crawler](littershredder.png) | Selected second version (the longer, tapered one); golden overlapping plates, brown bands, pale legs and feelers, single magenta spot. The later less-tapered variant was not selected. Exact segment count needs model reconciliation. Outer halo needs cleanup. |
| Lanternjaw | [mantis scorpid with twin lures](lanternjaw.png) | Selected 2026-10-01 (Wrysk's own GPT-image reference, then a Blender/voxel round). Violet plated scorpid: helmeted head, two large magenta eyes and two small ones, pale fangs, a pair of spined mantis striking arms, six heavy walking legs with dark joints and claw tips, overlapping back plates. Two tails, each holding a cyan lure orb in a claw cup. **The image is not followed for the tail or the face:** the image draws a second tail end beside one tail; the model (`scripts/blender/animal_models.py` `lanternjaw()`) forks the tail at its root into two branches that arch high over the back and reach over the face, each orb held from behind. The model's fangs are larger and vertical, and the striking arms end in pale razor blades. Transparent background. |
| Loftstrider | [radial canopy browser](loftstrider.png) | Selected 2026-10-01 (Wrysk's own GPT-image reference, then a Blender/voxel round). Onion-shaped teal-to-violet body on long legs with pink knee bulbs and splayed teal toes; magenta slit-pupil eyes round its middle; a long violet neck to a pink head with the mouth hole on top, ringed by bone tusks and lilac feeding fronds with frilled cyan edges. **Counts are Wrysk's, not the image's:** six legs; six eyes, each between two legs; six tusks and six fronds round the mouth, alternating and equally spaced. The fronds come in three sizes (100, 75 and 60 %, each twice, no two neighbours alike). The model (`scripts/blender/animal_models.py` `loftstrider()`) is the reference for counts and sizes; its colours are pushed brighter than the sampled ones to match the image. Transparent background. |
| Bellwing | [thin corkscrew tendril](bellwing.png) | Selected; plum bell, four lilac wings with cyan leading edges and magenta bell rim. Fine round tendril with a spindly three-dimensional corkscrew end, without a droplet. Shift its muted peach a little yellower later; avoid saturated orange or yellow. Alpha transparency verified. |

## Terrain candidates

| Subject | Pick | Decision |
| --- | --- | --- |
| Rock | [broad slabs](rock-broad-slabs.png) | Potential terrain source; original fractured-slab study. |
| Rock | [offset plates](rock-offset-plates.png) | Potential dry structural variant with long diagonal plates. |
| Rock | [damp stone](rock-damp.png) | Potential weather variant with uneven wet patches. |
| Rock | [shallow puddle](rock-puddle.png) | Potential water-bearing variant. |
| Rock | [lichen and springturf](rock-lichen-springturf.png) | Potential growth variant; crust-like lichen and a small turf tuft. |
| Bedrock | [dark foundation](bedrock.png) | Terrain candidate; dense near-black violet mass with broad fractured planes. |
| Ground litter | [fallen plant fragments](litter-dead-foliage.png) | Terrain candidate for colour and shape; dried umbrellafrond, vaulttree and springturf pieces in petrol, mauve and lilac. Density and placement still need variants. |
| Soil | [soil bare bank](soil-bare-bank.png) | Bare bank and depth study; terrain candidate, not a final tile. Deeper multi-voxel variants remain part of the rendering pass. |
| Soil / rock | [soil rock transition](soil-rock-transition.png) | Exposed soil-to-rock transition; terrain candidate. |
| Water / rock | [water rock pocket](water-rock-pocket.png) | Water held in a rock pocket; terrain candidate. |
| Water / soil | [water soil shore](water-soil-shore.png) | Shallow water and soil shoreline; terrain candidate. |
| Water | [water deep cutaway](water-deep-cutaway.png) | Deeper water cutaway; terrain candidate for multi-voxel depth. |

## Deferred colour studies

| Subject | Pick | Decision |
| --- | --- | --- |
| Seedporter | [orchid colour study](seedporter-orchid-study.png) | Exploratory alternate; Wrysk said it was fine, then deferred further colourway work. Does not replace either retained green reference. Outer halo and excessive sheen remain. |
