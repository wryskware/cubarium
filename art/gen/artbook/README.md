# Cubarium artbook — concept picks, 2026-09-24–25 (lanternjaw 2026-10-01)

[Open the visual artbook](index.html).

These are Wrysk's selected visual references, retained alternatives and terrain candidates. They are not finished texture masters or updated voxel models. Later selected appearances override conflicting older concept proposals within visual scope only.

## Remaining cleanup

- Remove painted outer halos/backgrounds where present; they are image artifacts, not creature emission. The gallery reports actual PNG alpha separately from appearance.
- Capgnawer: keep the four sensor patches light pink, with larger upper and smaller lower patches.
- Bellwing: shift the thin tendril a little yellower than the retained muted peach, without returning to vivid yellow/orange; preserve its spindly corkscrew.
- Velvetpad remains provisional. Bloomcrown retains both crown colours, leaning pink. Seedporter retains the exact selected image and the explicitly requested compact-head alternate.
- All terrain stays candidate material for a later detailed rendering pass. Litter needs further arrangements and densities using our plants' foliage. Further animal colourway exploration is deferred.
- Lanternjaw: where the image and the model differ (tail fork, fangs, razor blades), the model is the reference.
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
| Seedporter | [selected moss-green design](seedporter.png) | Selected exact tall-headed, open-mouthed image; six limbs, a small fruit-handling pair, larger membrane-bearing arms, rear gripping feet and a coiled prehensile tail. Keep the pictured proportions as the selected reference. Outer halo needs cleanup. |
| Seedporter | [compact-head alternate](seedporter-compact-head-alt.png) | Explicitly retained alternate: broader compact head, substantial muzzle and closed mouth, with lateral skull lobes. Does not replace the selected image. Outer halo needs cleanup. |
| Frondgrazer | [coral saddle shield](frondgrazer.png) | Selected; six short legs, continuous double-raised saddle shield, folded neck and transverse cropping scoop. Two stalk-tip sensors have dark plum lenses and magenta rims; shoulder patches remain. Outer halo needs cleanup. |
| Littershredder | [longer tapered crawler](littershredder.png) | Selected second version (the longer, tapered one); golden overlapping plates, brown bands, pale legs and feelers, single magenta spot. The later less-tapered variant was not selected. Exact segment count needs model reconciliation. Outer halo needs cleanup. |
| Lanternjaw | [mantis scorpid with twin lures](lanternjaw.png) | Selected 2026-10-01 (Wrysk's own GPT-image reference, then a Blender/voxel round). Violet plated scorpid: helmeted head, two large magenta eyes and two small ones, pale fangs, a pair of spined mantis striking arms, six heavy walking legs with dark joints and claw tips, overlapping back plates. Two tails, each holding a cyan lure orb in a claw cup. **The image is not followed for the tail or the face:** the image draws a second tail end beside one tail; the model (`scripts/blender/animal_models.py` `lanternjaw()`) forks the tail at its root into two branches that arch high over the back and reach over the face, each orb held from behind. The model's fangs are larger and vertical, and the striking arms end in pale razor blades. Transparent background. |
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
