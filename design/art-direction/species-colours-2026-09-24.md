---
design_status: proposal
last_reviewed: 2026-09-24
decision_refs: []
---

# Species colours: the colour source of truth (2026-09-24)

**For colour, this file overrides the species dossiers
(`species-dossiers-2026-09-21.md`, D15) and Astra's prose
(`design/animal-body-reimagining-2026-09-24.md`).** Those documents still bind for
**parts, counts, shapes, surfaces and states**. Only their hexes are replaced. Where a
dossier names a colour for a part, look the part up here instead. The terrain, sky and
water palette stays as the art direction and `design/appearance.md` give it.

Why: Wrysk preferred the 2026-09-23 colour pass to the dossier blues ("I think these
colours are honestly better than the dossier"). He then chose colourway 1 for the
animals ("top row for everything, except bellwing … keep the original").

## Plants: the colour pass

Source: `crates/cubarium/src/voxel/colours.rs` (`recipe`). Keep the two in step: when one
changes, change the other. Ramps run from the **base** (dark, saturated) to the **tip**
(bright), and shadows lean toward deep indigo `#140A3C`, not black.

| species | bark / stem (base → tip) | foliage (base → tip) | glint | other |
| --- | --- | --- | --- | --- |
| bloomcrown | `#4A1250` → `#6E1E6A` | hot pink `#9A1080` → `#FF52D0` | `#FFE0F6` | core `#FFD2F2` (blush); bloom ripe `#FF8A3A` → `#FFE890`, unripe `#FFD2F2` |
| umbrellafrond | `#241650` → `#3E2A7E` | sea-glass `#0A4A5E` → `#46F2D2` | `#C8FFF2` | ripe `#FF9B50` → `#FFE8A8` |
| springturf | `#3A1A5A` → `#5A2E7A` | mint `#106A5A` → `#6AF7BE` | `#D8FFEC` | — |
| stonecushion | root mat `#510B6D` | lilac-white `#7A5AC8` → `#EEE4FF` | — | — |
| velvetpad | `#3A1A5A` → `#5A2E7A` | cobalt `#1C46B0` → `#2E6CE0` | cyan `#7CE8FF` | — |
| glowcap | lilac stalk `#8A5AC0` → `#B99BE6` | luminous aqua `#2AE0B0` → `#7CFFDA` | `#F0FFF8` | lip glow cyan `#42C5F8` (ripe) / `#2A86B8` (fruiting) |
| vaulttree | `#331242` → `#5A2670` | emerald `#0B4A3C` → `#5CE68E` | `#D6FFE2` | drape `#1E8AB8` → `#8CF6FF` (glowing) |
| lanternberry | `#3A1A5A` → `#522A6E` | acid lime `#4E7E16` → `#CCFF52` | `#F2FFC0` | lantern ripe `#FF7A2A` → `#FFE6A0`, unripe `#6A2E72` |
| siphonreed | — | celadon `#244E66` → `#C4EEDC` | `#FFC8F4` | head electric magenta `#FF40D2` |

**Latticevine** keeps its dossier colours (D15; the tile set Wrysk liked):
- foliage `#1E2798` / `#2B6AD0` / `#42C5F8`;
- runner `#3A1A7A` / `#2A0E4A`;
- holdfast `#510B6D`;
- bell `#B99BE6` with a cyan `#42C5F8` mouth;
- beads magenta `#FF2AFC` with cyan seeds.

The umbrellafrond, velvetpad, vaulttree and glowcap designs are still open (SIGNOFF);
these are their current colours, not signed-off ones.

## Animals: colourway 1

Source: `runs/animal-bodies-2026-09-24/README.md`, v2 and v3 sections, and the renders
beside it. The columns are **main / shadow / pale secondary / marking / accent**.

| animal | main | shadow | pale | marking | accent | markings |
| --- | --- | --- | --- | --- | --- | --- |
| chorister (CH-A2 v3), violet stalker | `#6E5CD6` | `#2E2470` | `#D6CCF6` | `#1C1450` | jaw `#42C5F8`, eye `#FF2AFC` | countershading (dark back, pale belly and head) with soft rosettes; **no bands** (v3) |
| ripple-snail (RS-D2), azure dome | `#4A7CF0` | `#141A40` | `#A8C8FF` | `#2A50C0` | lobe tips `#FF5AD8` | two concentric growth rings on the dome |
| capgnawer (CG-D2), rose-mauve | `#9A5A8C` | `#4A2446` | `#F0C8E4` | `#E6A6D8` | eyes `#FF4AC0` | pale spots over the mantle |
| seedporter (SP-AB2), moss | `#5A9A3A` | `#23461E` | `#D8F0A8` | `#162E12` | `#FF2AFC` | dark dorsal stripe head to tail, pale belly, dark outer panel on each membrane |
| frondgrazer, coral | `#FF8A6E` | `#A22A4A` | `#FFE0CC` | `#6A1A30` | sense patches `#FF2AFC` | dark saddles on both shield lobes, pale underside |
| littershredder, gold | `#FFD860` | `#B06A18` | `#FFF4C8` | `#5A3208` | sense dot `#FF2AFC` | alternating dark segment bands, pale underside |
| **bellwing A, original** | bell `#2A0E4A` | — | vanes `#B99BE6` | — | rim `#FF2AFC`, vane leading edge `#42C5F8` | none; Wrysk: keep the original, don't recolour |

### Kept alternates (colourway 2; a second deliverable, backlog §8)

These come after colourway 1 is approved. Wrysk likes the two in bold.

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
