---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# A candidate community for the voxel strip

Package D of the [voxel first wave](handoffs/voxel-first-wave-briefs-2026-09-16.md).
This sketches one community that the horizontally periodic voxel strip could
carry, and names what terrain would have to expose for it to be simulable. It
proposes; it decides nothing, authorizes no implementation, and sets no
parameters. It follows §5 of the
[terrain and ecosystem proposal](terrain-and-ecosystem-proposal-2026-09-16.md) and
the [niches reconsideration](ecological-niches-reconsideration-2026-09-15.md), and
reuses the mechanisms of the [ecology v1 contract](ecology-v1-contract.md) where
they survive the move off the five-face surface.

## 1. Six organisms

Art names are placeholders for functional roles, not settled biology. "Paid"
means the parent debits its own reserve or escrow; nothing arrives from outside.

| Role | Eats, by identity | Needs from terrain | Reproduction | Death |
| --- | --- | --- | --- | --- |
| **Bloomcrown** — sun producer | light, mineral nutrient | high surface light; shallow rooting over thin ridge and terrace soil; tolerates low pore water, intolerant of standing water at the root voxels | paid propagule package (wood, starter foliage, starter reserve) to a neighbouring bare or establishing surface cell, as ecology v1 §4.8 | unpaid maintenance under new shade, prolonged dry below its wilting floor, foliage stripped past its reserve, roots drowned or buried |
| **Umbrellafrond** — shade/wet producer | light, mineral nutrient | tolerates low light (canopy, overhang, hollow); wants high pore water and several contiguous soil voxels to root into; no free water standing over the crown | same paid propagule, shorter hop and a wetter establishment window than survival window | dry-down of its rooting column, unpaid maintenance in true darkness, browsing past reserve, burial |
| **Glowcap** — decomposer | dead wood `Wd` and plant litter `D` in the voxel it is attached to; never light | an attachment face carrying dead wood or litter; a humidity floor from pore water or overhead cover | paid spore propagule to substrate-bearing voxels a short hop away; no substrate, no establishment | substrate exhausted, humidity below its floor, grazed past reserve |
| **Frondgrazer** — browser | bloomcrown foliage preferred, umbrellafrond foliage at lower yield; no litter, no remains, no wood | foliage reachable by actual 3D geometry from a standable voxel; traversable slope; free water shallow enough to wade | paid offspring through the existing gestation escrow and paid juvenile growth | starvation from unpaid upkeep, predation, deep free water, stranding on a patch with no reachable foliage |
| **Littershredder** — detritivore | litter `D` and animal remains `C`; optionally glowcap tissue as a second channel, which is what couples the decomposer into the animal web | litter-bearing or damp soil voxels to work and shelter in; cover; pore water above a desiccation floor | paid offspring, same escrow | starvation, predation, desiccation on dry bare rock |
| **Lanternjaw** — predator | living frondgrazer and littershredder bodies; carrion only as a fallback its capability actually allows | cover and broken line of sight for an ambush; a standable approach inside its motor budget; prey inside reach at the strike | paid offspring, same escrow | starvation (upkeep and sensing, not charging, looked like the juvenile bottleneck in the hunter charge rerun), failed strikes, prey absence |

Three couplings are the point of the roster, and each should be legible on
screen: bloomcrown shaded out by whatever grows over it; glowcap alive only
where something died and left wood, its glow paid out of its own respiration
rather than granted; grazing that opens a gap and so changes who establishes
next. Removing one should be measurable without granting or removing energy by
decree.

## 2. What terrain would have to expose

`cubarium-voxel` already gives `material`, `free` (fraction of void volume),
`pore` (fraction of pore capacity) and `surface_y(x, z)`. Those cover material
identity and the water state. The community above reads seven things, and five
of them do not exist yet.

| Field | Unit | Per | Read by | Status |
| --- | --- | --- | --- | --- |
| `material_at` | enum | voxel | rooting, attachment, traversal | exists |
| `pore_at` | fraction 0–1 of capacity | voxel | both producers, glowcap humidity, littershredder | exists |
| `free_at` | fraction 0–1 of void | voxel | wading, drowning, shoreline | exists |
| `surface_y` | voxel index | column | skyline only; see the support note below | exists |
| `sky_light` | fraction 0–1 of open-sky irradiance | surface cell, and any voxel a canopy could occupy | producer income; glowcap not at all | **missing** |
| `sky_openness` | fraction 0–1 of the upward hemisphere open (1 = open sky, 0 = fully blocked) | surface cell | light geometry; keep separate from vertical rain exposure, which a roof blocks while lateral light still arrives | **missing** |
| `slope` | rise over run, or degrees | surface cell | soil retention in the generator, traversal cost, propagule retention | **missing** |
| `water_depth` | m of the contiguous wet void interval above the support face | support face | wading, drowning, aquatic margin | **missing as a named field** (derivable from `free` and the support; no accessor) |
| `rooting_depth` | m, or count of contiguous soil voxels below the surface | column | producer access to pore water; groundcover versus tree | **missing** (derivable) |
| `substrate` | m of litter and of dead wood, kept as separate identities | voxel face | glowcap attachment, littershredder food | **missing** (ecology-layer stock, but terrain has to offer it a place to sit) |
| `reachable(from, to, body)` | query: can a body of this radius and step height get there | pair of surface cells | browser foraging, predator approach, everything about patch access | **missing** |
| `visible(from, to)` | query: fraction or boolean | pair of cells | stalking, escape, sensing | **missing** |

Which of these the first coupled experiment (section 4, two producers) actually
needs: a supporting voxel and its exposed face, root-accessible pore water below
it, standing water above it, and geometric light with plant-owned canopy
attenuation. Slope cost, reachability, visibility, concealment and an explicit
substrate attachment wait for the animals and the glowcap; litter and nutrients are
ecology-layer stocks. Two interface needs before coupling water consumption: a
bounded pore-withdrawal command on the core and an explicit transpiration loss in
the ledger, so two readers of the same moisture compete for it instead of both
reading it. A vertical-only light check would make sheltered habitat under an
overhang fully dark; if used first, name that limitation.

**Support, not skyline.** `surface_y` is the highest solid in a column. Under the
requested overhang that is the roof, not the sheltered floor, and summing a column's
free water would count a rooftop pool as drowning the plant below. Ecology locations
should be a supporting voxel plus its exposed top face; water depth is the contiguous
wet void interval above that support, rooting depth the contiguous soil below it,
available water is capacity × pore fraction × voxel volume. No cached field arrays
are needed for that yet.

Two notes on `sky_light`. It is not a terrain-only quantity: terrain can supply
sky visibility through solid voxels, and the ecology layer would attenuate it
further through canopy occupancy, so the sensible split is a geometric
sky-visibility field from the voxel core plus a canopy attenuation the plant
layer owns. And the last two entries are queries, not stored arrays; they are
the ones that make "reachable food by actual 3D geometry" mean something rather
than being a distance threshold in disguise.

## 3. Carry-over

| Mechanism | Carries over? | Note |
| --- | --- | --- |
| Stands: per-cell `P`/`W`/`Q` with structure, reserve, capacity and life stage | yes | per surface column or attachment voxel instead of per face cell; plus species identity |
| Paid local propagules (donor reserve, equal split among bare/establishing neighbours, frozen establishing class, viable package) | yes | neighbourhood becomes voxel adjacency; the "no donor, stays bare" allowance is what makes patches real |
| Growth and conservation ledger (`mass_residual`, `stored_energy`, exactly two light-in entries, every transfer booked twice) | yes | same discipline the voxel water `Ledger` already runs; the two ledgers stay separate |
| Detrital stocks with identity and one-way energy loss (`D`/`De`, `C`/`Ce`, `Wd`) | yes | dead wood gains a geometric location, which is what glowcap needs |
| Digestive capability with a hard exclusion, one mouth rate, trade-off in yield | yes | unchanged; diets by identity extend it to per-species yield |
| Hunter phases (perched, stalking, windup, strike, recovering, handling) and boundary-stepped strike resolution | yes | cover and line of sight become geometric instead of a radius |
| Motor budget `\|v\| + r·\|ω\| ≤ min(speed_cap, affordable)`, pace in body lengths per second | yes | needs a slope and step-height term; wading already fits |
| Paid reproduction, gestation escrow, paid juvenile growth | yes | unchanged |
| Five-face surface fields, the 1,280-cell face graph, seam and `Topology`/`Scale` | **no** | one lattice with a single wrapping axis; hop-based care footprints would be re-expressed as voxel neighbourhoods |
| Cell = 4×4 screen pixels, field resolution tied to the raster | **no** | grid size is world config; `px_per_voxel` is presentation only |
| Noise `light_base` / `moisture_base` and drifting weather blobs as the habitat | **no** | light from geometry, water from the simulation; this is the substantive change |
| `habitat.terrain` height field and `fields.w` surface water | **no** | superseded by material and free water |
| One generic producer pool per cell | **no** | species identity, or the renderer cannot derive what it is drawing |
| Downhill detritus transport on the face graph | **no** | would be re-derived from real slope and free-water flow, if at all |
| Saved worlds and trained policies | **no** | always fresh, never migrate |

## 4. The first coupled experiment

Once the generator and the water solver are trustworthy, the smallest thing
worth running is **the two producers alone**: bloomcrown and umbrellafrond,
seeded as a few founder stands, competing for light and pore water on generated
terrain, with paid propagules and background decomposition of their own litter
returning nutrient. No animals, no glowcap. It is still coupled — through shade
and through the rooting column — and it is the only arm in which a failure can
be attributed to terrain or water rather than to foraging.

The one observation that would show patches follow terrain rather than noise:
**re-draw only the generator's final weak correlated noise with a new seed,
keeping the landform, and the umbrellafrond patch should reoccupy substantially
the same basin, hollow and overhang voxels as before, while bloomcrown stays on
the same ridge and terrace.** If the patches relocate with the noise seed, the
community is reading the noise and the terrain coupling is decorative — which is
exactly the failure the present noise-based habitat would produce.
