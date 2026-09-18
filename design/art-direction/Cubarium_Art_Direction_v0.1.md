WORKING CREATIVE BRIEF

# Cubarium

## Art direction

Version 0.1  |  17 September 2026

*Visual reference: R1 / Existing 2D-world mockup: the strongest current reference for the pixel treatment, clustered ecology, and atmosphere. Its former 2D simulation is not a constraint on the new world model.*

> A lush, visibly pixelated alien ecology: a living cutaway diorama whose forms invite curiosity and whose behavior rewards attention.

Cubarium is a 3D voxel-based ecology simulation, initially experienced as an ambient living world and eventually as a game about ecology. Its production art should feel organic, psychedelic, and science-fictional, not magical and not visibly assembled from voxel blocks.

This brief records the agreed direction, the nine visual pillars, and practical guidance for judging new work. It establishes a coherent destination without fixing the renderer, exact pixel scale, final asset pipeline, or a future artist's interpretation.

## The working agreement

*01 / Direction at a glance*

The visual identity is stable enough to begin production. The approach is deliberately staged: use the simplest useful diagnostic view, establish a strong graphic pixel-art baseline, then develop richer execution as resources allow.

| Area | Agreed direction |
| --- | --- |
| **Subject** | An alien ecology with recognizable niches and evolutionary constraints. Organisms can be strange, ornamental, and partly unexplained. |
| **Presentation** | The existing tilted, side-on cutaway diorama, with visible depth. The camera concept has already been chosen. |
| **Simulation and art** | 3D voxels describe the world. Production graphics should not expose a cube-per-cell construction aesthetic. |
| **Visual target** | Lush but still visibly pixelated. Graphic pixel ecology develops toward a painterly pixel diorama, rather than changing identity. |
| **Rendering freedom** | 2D pixel art and sprite-sheet organisms are the leading approach. 3D or hybrid techniques remain available wherever they serve the same look. |
| **Primary display** | Author for normal screens at 1080p and above. The phone-sized AMOLED is itself 1080p, not a low-resolution art target. |
| **Cube adaptation** | Develop after the screen version looks good. The P2.5 LED cube may use adapted, lower-resolution, or different artwork. |
| **Production reality** | Near-term assets are AI-generated and selected by the creator. A future artist or director may refine the detailed visual language. |

### How to read this brief

The decisions above and the nine pillars are the working commitments. The later production guidance translates them into useful defaults, not a locked technical specification. Examples describe visual possibilities; they do not add mandatory species, mechanics, biomes, or simulation features.

The aim is not to make every scene maximally dense or every organism immediately understandable. It is to make the world feel inhabited, coherent, and worth observing, at the level of finish the project can currently support.

## Life as the subject

*02 / Visual pillars*

### 1. Ecology first

The subject is the living system, not a decorative environment with creatures placed on top. Terrain, organisms, water, light, and motion all matter because together they make that system perceptible. No single one needs to dominate every composition.

Art should help a viewer notice occupation, interaction, growth, movement, and change. Beauty and readability should reinforce that attention rather than compete for a permanent winner. In the game, this becomes a foundation for understanding the ecology; in the ambient presentation, it gives the scene substance beyond its first impression.

When visuals explicitly depict a simulated interaction or state, they should agree with it. Decorative texture and atmosphere are still allowed; this is not a requirement to make every visible fleck a simulated entity.

### 2. Alien, but evolutionarily suggestive

Organisms should feel as though they evolved on another planet under recognizable pressures, occupying familiar kinds of ecological niches through unfamiliar bodies. The result is science fiction and psychedelic biology, not a fantasy bestiary or a collection of magical effects.

Plausibility is an invitation to imagine a living organism, not a demand for a scientific defense of every frill. Ornament, variation, extravagant anatomy, and unresolved features belong here. Animals may mix recognizable and unfamiliar forms; vegetation may blur visual categories such as plants, fungi, coral, and colonies.

### 3. Forms invite questions; behavior sometimes answers them

A creature can be legible as a creature while remaining mysterious in its particulars. A strange appendage should invite “what does it use that for?” rather than require an immediate explanation or a functional label.

Occasional behavior may change how the viewer understands a familiar organism. A rare animation might reveal an unexpected feeding method, a sheltering posture, or a use for that conspicuous appendage during courtship or reproduction. These are opportunities for discovery, not a requirement to explain every feature eventually.

> Biologically suggestive, not anatomically justified down to the last frill.

## A world, not an asset shelf

*02 / Visual pillars*

### 4. A habitat, not a collection of specimens

Organisms should appear to occupy a place together. Uneven colonies, overlapping canopies, gaps, mixed sizes, and partial concealment can make a scene feel inhabited. Growth should relate visually to surfaces, water, available space, and neighboring organisms where the simulation supports those relationships.

Composition is therefore partly about relationships between assets. A simple sprite can contribute to a rich patch through variation, grouping, and placement. Avoid solving lushness by spacing an assortment of impressive individual specimens evenly across the terrain.

### 5. Organic, not blocky

Production terrain should read as slopes, ledges, banks, strata, cavities, pools, and continuous masses. The underlying voxel grid is a simulation structure, not the visual grammar. Raw cubes are acceptable in dev mode, not the intended finished style.

Pixel-stepped outlines are compatible with this principle. They are a property of the image, not evidence that the world is built from identical blocks. Organic does not mean uniformly smooth: crags, angular rock, exposed layers, and abrupt cliffs can remain distinctive.

Use whatever combination of sprites, surface treatments, meshes, and overlays gives coherent forms. Removing the visible grid should not invent misleading openings, change a water boundary, or obscure where inhabitants actually stand or move.

### 6. Readable at a glance

At the intended viewing scale, viewers should be able to separate major terrain masses, water, living patches, and moving inhabitants. This does not mean every tiny organism or obscure adaptation must be identifiable from across the room.

Give silhouettes and larger value groupings priority over fine surface marks. Use detail in clusters with quieter spaces between them. Strong color is welcome, but texture, glow, and particles should not dissolve the scene into equally bright noise.

Readability is judged first on the primary screen presentation, including the physical size of the AMOLED. The later LED adaptation may solve it differently, with redesigned assets rather than a compromise imposed on the main version.

## Richness through time

*02 / Visual pillars*

### 7. Beauty includes the whole life cycle

A world should not need to be permanently lush, young, and healthy to be visually rewarding. Growth, maturity, decline, decay, and renewal can each have their own forms, colors, and rhythms. Sparse or recently disturbed areas can still make strong compositions.

Where those states exist in the simulation, their art can offer more than a healthy sprite with its brightness turned down. An aging canopy might become translucent; a persistent shell might remain a striking silhouette; a decomposer colony might introduce a new pattern into a fading patch.

This is not a requirement to make ecological trouble cheerful or to disguise failure. Give change visual character without making its meaning dishonest.

### 8. Calm at a glance, rewarding to watch

The ambient world should have a coherent resting rhythm. Saturated color and lush detail do not require constant visual agitation. Large forms hold the composition while small movements reveal the lives within it.

Routine activity can be modest: feeding, exploring, drifting, tending, responding to nearby movement. More conspicuous behaviors or ecological events occasionally interrupt that baseline. The reward for sustained attention is discovering life, not simply receiving a higher density of effects.

Calm does not mean static, and rare does not mean invisible. A significant behavior can briefly earn attention through motion, posture, or local contrast. The eventual game may need stronger feedback, but that does not have to become the world's permanent ambient state.

### 9. Lushness scales with resources

The graphic and painterly looks are points on a continuum. Begin with the simplest production art that establishes Cubarium's identity, then enrich it through better textures, surface shading, shaders, particles, material transitions, animation, and lighting.

Shape variation and layering matter too: hanging growth, secondary forms, overlaps, and different colony structures contribute richness that a texture upgrade alone cannot supply. A habitat, organism family, or material can improve independently; the whole project need not be replaced at once.

> A simple version should already feel like Cubarium. A richer version should feel like more of the same world, not a different game.

## One direction, increasing richness

*03 / Staged execution*

*Visual reference: R2 / Selected progression from the exploration board: 1. Dev mode; 2. Graphic pixel ecology; 3. Painterly pixel diorama. The board is concept art, not proof of a finished renderer or asset set.*

### 1 / Dev mode: minimum effort, maximum diagnostic value

Get something useful on screen quickly. Flat colors, crude blocks, simple markers, visible voxel cells, and utilitarian overlays are acceptable. This representation exists to inspect progress and changes in the simulation, not to demonstrate the final art style. Keep it available as a diagnostic option after production rendering improves.

### 2 / Graphic pixel ecology: the production foundation

Establish organic terrain contours, coherent pixel treatment, recognizable material groups, distinctive organism silhouettes, and the palette relationships. Use relatively simple textures, modest animation, and selective effects. The habitat should already feel alive and specific to Cubarium, without depending on elaborate shaders to supply its identity.

### 3 / Painterly pixel diorama: the developed form

Increase richness through better surfaces, colony variation, layered vegetation, finer material transitions, more expressive animation, atmosphere, water treatment, and controlled light. Retain visible pixel character and readable large forms. This is the desired direction as resources and budget permit, not a requirement to achieve the full concept image immediately.

The numbers describe a development path, not three separate art identities. Dev mode remains available; the production look can occupy any useful point between 2 and 3. More detailed execution need not mean raising every sprite's resolution.

The other exploration-board alternatives were not selected as separate destinations. In particular, leaving 3D techniques available does not commit the project to the distinct hybrid look shown in option 4.

## Keep the model and image separate

*04 / World and displays*

**R3 / Wide terrain view.** Existing tilted cutaway presentation, including depth, strata, and water.

**R4 / Closer terrain view.** The same presentation at a closer scale. Reference for framing, not final surface quality.

### The presentation is already chosen

Use the existing tilted, predominantly side-on cutaway diorama with visible depth. It can reveal surface habitats, water volumes, cavities, and exposed terrain layers. It is not a mandate to turn the new simulation back into a flat 2D world, nor a request to redesign the camera around the landscape references.

### Voxels describe occupancy; art describes appearance

An organism may occupy a 3D volume while being represented by a 2D sprite or sprite-sheet animation. Terrain appearance may be derived from underlying voxel content through sprite-based surface treatment, materials, or other methods. These are leading approaches, not fixed implementation requirements.

3D geometry, lighting, water, or other 3D-assisted elements can be used for some or much of the image, provided they support the agreed pixel-art identity and coherent depth relationships. Choose techniques by the result, not by a purity rule about 2D versus 3D.

### Screen first; cube adaptation later

The normal rendering target begins at **1080p**. The phone-sized AMOLED has that resolution, so it should not be treated as a low-resolution display. Output resolution does not determine the resolution of each sprite or the eventual internal pixel grid; those choices remain open.

First make the screen version look good. Then author the **P2.5 LED cube** presentation as an adaptation, potentially with different art, simpler silhouettes, lower-resolution sprites, altered detail density, or adjusted effects. Preserve the world's identity, not necessarily its exact assets. Cube constraints should not dictate the primary art from day one.

## Color, surfaces, and living detail

*05 / Working visual language*

These are practical starting defaults derived from the references and pillars. They guide near-term asset selection without fixing a palette table, pixel size, shader stack, or final director-level treatment.

### Color and light

Use deep indigo, blue-violet, and dark purple masses to support electric cyan, blue, violet, and magenta. Selective warm accents can draw attention or enrich a colony. Other reference colors may appear where they fit; this is a relationship between colors, not a requirement that every frame use the same swatches.

Bioluminescent and psychedelic color can be intense without making every surface emissive. Reserve contrast so inhabitants, water edges, and important movements remain distinguishable. Glow should support the underlying forms rather than replace them with haze. No numerical color budget or simulated light chemistry is required by this brief.

### Pixel treatment and terrain

Aim for lush images that still read as pixel art. Avoid treating pixelation as a filter applied to otherwise unrelated assets. Silhouette edges, texture marks, shading, and animation should feel compatible within the scene, while the exact degree of refinement remains a later artistic choice.

Let terrain materials differ through contour, texture, edge treatment, and visible layering. Growth can soften boundaries or occupy cracks without covering every surface. Pixel texture should articulate form rather than hide it beneath random detail. Pixel outlines do not need to track individual simulation cells.

### Organisms and colonies

Use a mixture of recognizable-but-strange and more unfamiliar bodies. Silhouettes can suggest ways of moving, occupying space, feeding, or sensing without fully explaining every feature. Do not default to either mascot-like cuteness or grotesque horror; neither has been chosen as the universal creature language.

Design related forms and variants that can belong to the same habitat. Differences in size, posture, age, or growth pattern can make a colony less repetitive, but should correspond to actual state when the image specifically claims to show that state. Not every decorative variation needs its own simulation variable.

### Motion and effects

Use ordinary activity to give scenes continuity, and more distinctive behavior to create occasional discoveries. Particles, sway, shimmer, water effects, and glow should add atmosphere without becoming the main subject. Quiet spaces and pauses are part of the composition.

Keep a distinction between decoration and a depicted ecological event. An atmospheric mote need not represent anything simulated. A visible reproductive release, feeding interaction, or other meaningful behavior should be driven by the corresponding event rather than played as unrelated wallpaper.

## What to borrow from each image

*06 / Visual references*

*Visual reference: R5 / Alien river scene: the primary reference for lush organic form, branching and draping growth, biological ambiguity, layered detail, and channels of negative space. Translate its qualities into the chosen pixel-art diorama rather than copying its camera.*

**R6 / Owl: palette only.** Borrow the indigo, cyan, violet, magenta, and contrasting warm light. Do not borrow the fantasy creature, cosmic setting, glossy feathers, or action composition.

**R7 / Alien landscape.** Borrow bold umbrella-like silhouettes, repetition at different scales, atmospheric depth, and psychedelic color relationships.

**R8 / Tower landscape.** Borrow strong massing, coherent repeated forms, depth, and luminous accents. The architecture is not a requirement to add buildings.

R1 is the closest existing pixel-art mockup, not a requirement that the new 3D model inherit its former topology. R3 and R4 establish the actual world presentation. R2 supplies the selected progression. Together these references have different jobs; none is a specification to copy every visible feature.

## AI-generated assets, coherently directed

*07 / Near-term production*

Near-term art will be AI-generated and curated by the creator. The brief should make it easier to select and adapt compatible results, not force premature commitments about tools, models, vendors, budgets, or production formats.

### Generate for a role in the world

Give an asset a clear visual role, habitat context, approximate in-scene scale, and relationship to neighboring forms. Its ecological niche can guide the design while leaving room for ornament and uncertainty. A concept can be compelling without carrying an explanation of every appendage.

Use the approved references deliberately: palette from the owl, organic richness from the river, silhouette and depth from the landscapes, and pixel character from the existing mockup. Do not feed the entire reference collection into an instruction that treats all its subjects and styles as equally binding.

### Judge in context, not only in isolation

Test assets against representative terrain, water, vegetation, and the intended viewing scale. A beautiful standalone image is not enough if its lighting, perspective, pixel character, or detail density makes it look pasted into the scene. Inspect both the full composition and the closer view where behaviors will be noticed.

For related sprites and animations, keep silhouettes, proportions, scale, contact with the ground or water, and visual treatment consistent enough to read as the same organism. Prevent accidental asset changes from masquerading as growth, mutation, or a different life stage. Exact sheet layouts, pivots, and export requirements belong in the implementation specification.

### Develop families and states where useful

Prioritize related assets that can form a habitat, rather than an ever-growing collection of unrelated showcase images. Add variants or life-stage treatments where they improve the scene and match supported systems. Do not require a complete bestiary or fully animated life cycle before the graphic baseline is useful.

### Preserve a useful reference set

Keep a small collection of accepted in-scene examples and the direction notes that produced them. Update that set as the project improves. The next generation should be judged against the evolving Cubarium identity, not automatically against the most elaborate output a tool happens to produce.

> Generate a [subject] for a lush, visibly pixelated science-fiction ecology. Use the established cutaway view, coherent silhouettes, dark indigo-violet support, and selective luminous color. Make its biology suggestive and inventive, not magical. Match the current scene's pixel treatment and level of finish.

The text above is an adaptable direction seed, not a complete production prompt or a fixed requirement for every asset.

## A brief that supports iteration

*08 / Review and open decisions*

### Questions to ask of the next build

**Does it belong to this world?** The scene should carry Cubarium's color relationships, pixel character, organic forms, and alien ecological tone even before advanced effects are added.

**Can the habitat be read?** Major masses, water, growth, and moving inhabitants should separate at the intended screen scale. Fine anatomy may remain mysterious. Contrast and texture should not make all parts compete equally.

**Does it feel inhabited?** Look for convincing groupings, relationships to terrain, variation, and room for movement. More individual assets are not automatically a richer ecosystem.

**Does watching reveal more?** Ordinary behavior should sustain the world. Distinctive actions can offer occasional surprise without needing constant spectacle or an answer for every unusual feature.

**Is it faithful where it makes a claim?** Visible positions, contacts, events, and ecological states should match the simulation when explicitly represented. Decorative atmosphere is allowed, but it should not fabricate a meaningful interaction.

**Does the extra detail improve the whole?** Richness should deepen the established image, not erase silhouettes, overwhelm the composition, expose the voxel grid, or silently change the art identity.

### Deliberately open

Exact sprite resolutions and pixel scale; internal rendering resolution; the balance of 2D and 3D; terrain rendering techniques; lighting and shader implementation; animation density; asset-generation tools and workflow; final biome and organism lists; game UI and feedback language; and the eventual cube-specific art treatment remain open.

A future artist or director may refine the relationship between painterly surfaces and visible pixels. For now, “lush but still pixelated” is sufficient. The 1080p screen target, screen-first authoring order, and later freedom to adapt cube assets remain the relevant display commitments.

### Suggested next visual milestone

Use one representative screen habitat to establish the graphic baseline: coherent terrain and water, a related set of organisms, readable ordinary activity, and the agreed palette. Improve that same scene toward the richer look in manageable steps while retaining the diagnostic view. This is a production suggestion, not a new feature requirement or a rigid completion gate.

This v0.1 brief is ready to guide work. Revise it when actual scenes, generated assets, or future art direction reveal a useful change, rather than trying to settle every downstream choice in advance.

## Reference inventory

The illustrated Word edition embeds these references. Filenames below identify the original images supplied or generated during the direction discussion; they are not required runtime assets.

| Reference | Source image | Role |
| --- | --- | --- |
| R1 | `e5543915-1f8f-4e5c-97f2-adf25d921940.png` | Existing pixel-art ecology mockup; visual treatment, not the new simulation topology. |
| R2 | `a_wide_high_resolution_concept_art_montage_image.png` | Exploration board; selected progression is options 1, 2, and 3. The illustrated edition shows the selected row. |
| R3 | `tilt35_d32_px4.png` | Existing world presentation at a wide scale. |
| R4 | `zoom_pool_tilt30_d24_px4.png` | Existing world presentation at a closer scale. |
| R5 | `peragwin_psychedelic_primordial_soup_504eae41-63b6-4caa-93f4-b7eb01697226.png` | Primary lush organic-form and atmosphere reference. |
| R6 | `60a3a9a5-35cd-4a6d-9638-2546e8a67442.png` | Color palette only; not creature or fantasy art direction. |
| R7 | `5ec3ee6f-baec-44d6-af58-48b6c65e8e11.png` | Alien silhouettes, depth, and psychedelic color. |
| R8 | `b3964b22-9c84-4170-82db-856c46a8bfd9.png` | Massing, repeated forms, depth, and luminous accents; not a building requirement. |
