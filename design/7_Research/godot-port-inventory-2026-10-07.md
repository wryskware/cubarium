---
design_status: exploration
last_reviewed: 2026-10-07
decision_refs: [D-0004]
---

# Old voxel renderer → Godot: feature inventory and port classification

Read-only analysis, 2026-10-07, repo at `d18ccde3` (main). Sources: `crates/cubarium-gpu/shaders/voxel.frag`
(2592 lines; the lit desktop tier), `bloom_smooth.frag`, `volumetric.glsl`, `present.frag`,
`crates/cubarium-gpu/src/{voxel,weather,sunvis,bloom,present}.rs`, `crates/cubarium/src/sink/gpu/{voxel,light,sunvis,weather}.rs`,
`crates/cubarium/src/voxel/{present,stand,animal,model,sleep,idle,hunker,candle,vine,appearance,colours,textures}.rs`,
`LightConfig`/`VoxelConfig` (`crates/cubarium/src/voxel/mod.rs:145`, `:352`, defaults `:493`), `config/desktop/terrarium.toml`,
and the Godot side: `game/README.md` "The view", `game/world/{world_view,organisms,weather}.gd`, `crates/cubarium-godot/src/{mesher,models}.rs`,
`crates/cubarium-session/src/feed.rs`.

Skipped as legacy ring/cube only: `render.rs`, `atlas.rs`, `scene.glsl`, `background.frag`, `water.frag`, `sprite.vert/frag`
(the sprite wind **bend** lives only there), `cubarium-gpu/src/palette.rs`, `art_present/`.

## What the classes mean

- **built-in**: Godot 4.7 has it natively (node, Environment setting or material property). Covered by the lighting worker
  unless marked *(not in lighting brief)*, or trivial wiring.
- **done**: already in the Godot path (mesher / models.rs / organisms.gd / weather.gd); listed so nothing is double-counted.
- **port**: Godot has nothing equivalent; must be written. The stage it becomes is named.
- **obsolete**: meaningless with meshes, a real depth buffer, a real camera and free-moving models.
- **ask Wrysk**: a lighting-like or stylistic rule where Godot's built-in is a plausible but not faithful replacement.
  The trade-off is stated in one line.

Desktop default = `config/desktop/terrarium.toml` (`lighting = "lit"`, `vine_tiles = true`, `dither = 0.04`, `haze = 0.55`,
`water_alpha = 0.5`, no `[light]` table, so every `[light]` key is its `LightConfig::default()`).

"Feed" = `take_view()` (`game/README.md` "The view"): chunks (material, free water, pore water), animals (`ids, species, pos,
heading, state, mobility, stage, flags{striking, riding, hunkered, yielding, alert, wounded}, size`), plants (`ids, species, site,
growth, foliage` total), `sky` (`day, day_phase, hour, daylight, sun_elevation, cloud_cover, fog, rain, rain_m_per_s,
temperature_c, rh, strike[tick,x,z,seed]`), plus `take_events()` (born, meal, kill, injured, died, ...).

---

## 1. Terrain faces

| # | Feature | What it does | Where | Desktop | Driven by | Class | Port stage / feed / size |
|---|---|---|---|---|---|---|---|
| T1 | Strata palette faces | Plain coloured tops and fronts, bedrock/rock/soil colours | `present.rs:136-151`, `voxel.frag:304`; Godot `mesher.rs` | on | voxel grid | **done** (mesher, StandardMaterial vertex colour) | — |
| T2 | Pore-water darkening | Soil/rock lean toward deep water by pore fraction (`WET` 0.55) | `present.rs:180`, `voxel.frag:382` `blockBody` | on | voxel grid (pore plane) | **done** (mesher, 8 levels) | — |
| T3 | Depth haze | Every colour mixed toward `HAZE` by `haze·z/(depth−1)`, applied **after** light | `voxel.frag:314` `hazeAt`, `present.rs:1028` | on (0.55) | voxel z, config | **ask Wrysk** | Mesher already hazes per vertex but *before* light (a shadowed back wall goes darker than the haze colour). Options: built-in Environment fog (by camera distance, so height changes it too in a 30° ortho view; double-hazes unless mesher's is removed) vs port as spatial-shader `FOG` output from world z (small, faithful). |
| T4 | Top gain / top tint | Tops brighter than fronts and leaning to the light colour | `present.rs:154-156`, `voxel.frag:750` | flat tier only | config consts | **obsolete** | The lit tier already replaced it with light (sun + ambient); Godot lighting does the same. |
| T5 | Roof shade | Faces under an overhang keep `ROOF_LIGHT` fading over 4 voxels | `present.rs:164-168,304`, `voxel.frag:323-340`, `roofTex` | flat tier only | voxel grid | **obsolete** | Subsumed by shadows + T14. |
| T6 | Rim light row | Front face's top row leans toward the top colour where above is air (`RIM` 0.28) | `present.rs:170`, `voxel.frag:653,688` | on (lit keeps edge treatments) | voxel grid neighbours | **ask Wrysk** (edge group A5) | port: spatial shader + per-face neighbour mask vertex attribute (mesher; merging must split on mask) or a Texture3D occupancy lookup. medium for the whole group. No feed change. |
| T7 | Side bevel column | Exposed ±x side of a front face darkened (`EDGE_DARK` 0.72) so cliffs read | same | on | neighbours | **ask Wrysk** (A5) | same package |
| T8 | Chamfer corner | Top-row/exposed-side corner pixel takes the top colour: staircase reads as slope | same | on | neighbours | **ask Wrysk** (A5) | same |
| T9 | Riser lean | A one-voxel step into depth shaded top-light→front, no rim, haze interpolated | `present.rs:178`, `voxel.frag:653-720` | on | neighbours | **ask Wrysk** (A5) | same |
| T10 | Contour row | Top's back row shades toward front only where the surface ends going back (`TOP_BACK`) | `present.rs:160`, `voxel.frag:727-770` | on | neighbours | **ask Wrysk** (A5) | same |
| T11 | Top bevel at a drop | Top's side shaded only at a real drop, not a 1-step slope (`TOP_EDGE`) | `present.rs:174`, `voxel.frag:750` | on | neighbours | **ask Wrysk** (A5) | same |
| T12 | Face grain | Per-texel hash brightness jitter on solid faces (`dither` 0.04) | `voxel.frag:643` `faceGrain`, `present.rs:314` | on | voxel position, config | **port** | spatial shader on terrain material: hash of `floor(world_pos·N)` per face; small; no feed. |
| T13 | Face textures | Per-material/bark/leaf 48² textures, 4 variants by cell hash, LOD per px | `voxel.frag:221-258`, `voxel/textures.rs` | off (opt-in `textures`) | config + asset files | **port** (deferred) | spatial shader + Texture2DArray, UVs from mesher (merged quads need world-pos UVs); medium. Experimental; plain voxels are the main look. |
| T14 | Sky-visibility ambient | Ambient × core `sky_visibility` fan at each open cell: caves and under-ledge ground go to the floor (0.2) | `voxel.frag:458` `skyOpen`, `sink/gpu/light.rs` (header), `skyTex` | on | voxel grid (core fan, off-thread) | **ask Wrysk** | Built-in SDFGI/VoxelGI sky occlusion (GPU cost, ortho support unverified, not the ecology's number) vs port as per-vertex ambient attribute from the mesher (it holds the grid; reimplement or link `sky_visibility`), used as `AO`/ambient scale in the terrain shader: medium, faithful to light.rs's rule "may shade more finely than the model, never differently". Without either, a cave reads as lit at full ambient. |
| T15 | AO crease line | Thin hard band (~S/8 px) along an occluded edge, corner square for diagonal-only; light × (1−0.5) | `voxel.frag:405-457` | on (`ao` 0.5) | voxel neighbours | **ask Wrysk** | Built-in SSAO is soft and screen-space (lighting worker) vs port the crease (per-face 8-neighbour mask attribute from the mesher + fragment band test): medium, shares A5's mask. |
| T16 | Light ladder | Ambient snapped to `levels` rungs, sun adds one rung | `voxel.frag:513` `ladderLight` | off (`levels` 0 = smooth) | config | **obsolete** | Off by default. If ever wanted: a quantise CompositorEffect. |
| T17 | Ambient gain/floor/tint | Ambient 1.4×, floor 0.2, hue 0.15 toward sky | `voxel.frag:513`, `LightConfig` | on | config, day look | **built-in** | Environment `ambient_light_color/energy`, sky contribution. Floor = ambient minimum under T14. |
| T18 | Sun + cast shadows | Per-texel ray march toward the sun, 128 cells; terrain, trunks, logs, animals block | `voxel.frag:549-598` `sunReaches` | on (`shadows`) | voxel grid, sun dir | **built-in** | DirectionalLight3D shadows. |
| T19 | Sun tint lean | Sunlit texel leans toward the light colour by tint × N·L (0.18) | `voxel.frag:605` `litBy` | on | sun dir, day look | **built-in** | DirectionalLight3D `light_color`. |
| T20 | Dappled crown shadow | A crown cell passes/blocks a sun ray **as a whole cell** by hash under its column transmission | `voxel.frag:486-490,562` `crownLets`; `sink/gpu/sunvis.rs` | on | stand foliage model | **ask Wrysk** (A2) | Built-in: crowns cast fully opaque shadows. Port: plant shader discards in the shadow pass by `hash(cell) > transmission` (instance uniform per stand): small, but transmission needs canopy data (see T21). |
| T21 | Canopy ambient | Ambient under foliage × the flora model's per-layer transmission (`exp(−k(1−porosity)stock/area)`), straight down | `voxel.frag:466-485` `canopyAt`/`plantCanopy`, `sink/gpu/light.rs` Canopy | on | flora foliage layers (sim state) | **ask Wrysk** (A2) | Built-in: SSAO/GI give little under a canopy. Port: per-stand canopy as a small 2D texture (x,z → transmission) sampled in terrain/organism shaders: medium. **Feed lacks** per-layer foliage stock/radius/porosity (only total `foliage`). |

## 2. Water

| # | Feature | What it does | Where | Desktop | Driven by | Class | Port stage / feed / size |
|---|---|---|---|---|---|---|---|
| W1 | Water body + fill geometry | Translucent cell box to `free/255`, surface/top only at water–air, body/surface colours, skin & top alphas | `present.rs:65-82,942`, `voxel.frag:1216` | on | free-water plane | **done** (mesher) | — |
| W2 | One-face-per-pixel ownership | `nearerOwns`, `stop`, row clipping so blends never compound | `voxel.frag:1040`, `present.rs:914` | on | — | **obsolete** | Depth buffer; mesher already only emits faces where the neighbour's water is lower. |
| W3 | Depth absorption + in-scatter | Beer–Lambert per channel along the real path (σ = absorb·−ln deep), in-scatter ramps surface→deep colour, lit at entry | `voxel.frag:1060-1215` `waterAtLit` | on (`water_absorb` 0.1) | free water, light | **port** | water spatial shader: thickness from `DEPTH_TEXTURE` − fragment depth, `exp(−σ·d)` over `SCREEN_TEXTURE`; small–medium; no feed. |
| W4 | Fresnel reflection | Fresnel × 6 with a 64-cell reflection march into the volume | `voxel.frag:1825-1903` | on (`reflections`) | voxel grid | **built-in** | SSR (lighting worker's cheapest reflection) + material specular/roughness; `water_reflect` gain becomes a material knob. |
| W5 | Ripples | Animated, quantised surface normal from ripple lines advected along the flow, plus a breeze; two copies crossfade (never pops) | `voxel.frag:1346-1440` `flowAt`, `rippleLine`, `rippleNormal` | on (`water_ripple` 0.2, smooth, 12 Hz) | free-water grid (flow derived), clock | **port** | water spatial shader `NORMAL` from world xz + TIME; flow vector as a mesher vertex attribute (mesher has the grid; old flow = surface-height gradient, falling flag): medium. No feed change. |
| W6 | Glint | Sunward-tilted ripple lines lean toward light, toward white by `water_highlight` | `voxel.frag:1425`, `LightConfig.water_glint` | on (0.6) | W5, sun | **port** | in W5 shader; small once W5 exists. (Built-in specular gives a different, non-line look.) |
| W7 | Foam | Breathing band along shores (0.4 voxel) and a churning disc where a fall meets a pool | `voxel.frag:1668-1740` | on (`water_foam` 0.6) | free-water + solid grid, clock | **port** | water shader; shore distance and plunge flag as mesher vertex attributes: medium. |
| W8 | Rain rings | Growing/fading rings on open-sky water while it rains | `voxel.frag:1741-1782` | on (0.5) | `sky.rain_m_per_s`, open sky over cell | **port** | water shader + "open to sky" vertex flag (mesher); small. Feed has the rate. |
| W9 | Falling water | Falls (water over non-full, non-solid) have no ripple/reflection, streaks scroll down | `voxel.frag:1116,1904`, `litWaterFinish:1944` | on | free-water grid, clock | **port** | mesher flags falling faces; water shader scrolls streaks: small–medium. |
| W10 | Water darkens with light | In-scatter as a ratio to sunlit open ground, darker at night/under cover | `voxel.frag:532` `ladderRef` | on | light | **built-in** | Lit water material. |
| W11 | Flow debug | Visualises the derived flow field | `voxel.frag:1918` | dev only | — | **obsolete** | Could be a dev toggle in the W5 shader later. |
| W12 | Reflected underside / glyph texels | What a reflection ray hits drawn with its own lighting | `voxel.frag:1783-1823,1868` | on | — | **obsolete** | SSR reflects the rendered scene. |

## 3. Organisms: static look and state display

| # | Feature | What it does | Where | Desktop | Driven by | Class | Port stage / feed / size |
|---|---|---|---|---|---|---|---|
| O1 | Baked models, ramp colours | `.cvm` models, colour by species/part/height band | `voxel/model.rs`, `colours.rs`; Godot `models.rs` | on (`organisms = models`) | plant growth, animal size | **done** | step nearest growth, bin by size, variant by stand hash. |
| O2 | Organism face glyphs | Per-face pixel-art glyphs from `appearance.rs`: eyes, markings, outlines, sense patches; tones (edge-dark 1, rim 2, lit column 3, under-lean 4/5, graded 16–47, emit 6) | `voxel.frag:771-960` `glyphFrontAt/glyphCapAt/plantTexel`, `voxel/appearance.rs` | on | static per model part, state for some glyphs | **port** (anatomy) + **ask Wrysk** for the light-stand-in tones (A6) | models.rs emits UVs into a glyph atlas (Rust can call `appearance` directly) + organism spatial shader for tones: medium–large. No feed for static glyphs. |
| O3 | Plant tone rules | Crown silhouette edge (`CROWN_EDGE` 0.70), crown underside toward wood (`CROWN_UNDER`), plant rim (`PLANT_RIM`), trunk cylinder (`TRUNK_SHADE` 0.62–1.22 at 0.35) | `present.rs:184-201`, `voxel.frag:849-905` | on | model part tags | **ask Wrysk** (A6) | Inside the O2 shader if kept; small on top of O2. |
| O4 | Plant top gain/tint | Plant tops brighter, leaning to light | `present.rs:184-186`, `voxel.frag:370` | flat tier | — | **obsolete** | Light replaces it. |
| O5 | Foliage thinning by layer | Foliage/drape cell of layer i drawn iff `keeps(stand, cell, layer, stock/capacity)`: browsed rosettes empty, regrowth only adds | `model.rs:637` `keeps` | on | per-layer foliage (sim) | **port** | models.rs per-vertex layer + cell hash; organism shader discards vs per-instance layer fractions (instance uniforms); medium. **Feed lacks** per-layer stock/capacity (has total `foliage`). |
| O6 | Wilt tint | Foliage tinted by moisture, `WILT_LEVELS` steps | `model.rs:597-610` | on | plant moisture (sim) | **port** | instance uniform; small. **Feed lacks** moisture/wilt level. |
| O7 | Accents by state | Warm bloom only when the parcel is ripe; lanternberry fruit by parcel fullness | `model.rs` header, `stand_delta.rs` (ripeness in ModelSig) | on | parcel state (sim) | **port** | models.rs tags accent cells; shader/visibility per instance; small–medium. **Feed lacks** ripeness/parcel fullness. |
| O8 | Emission | Dossier-luminous parts (glowcap lip, drapes, bloomcrown core, heart stripes) drawn unlit at full value | `voxel.frag:342-369`, `appearance.rs` (Emission), `colours.rs` | on (`emission`) | model parts, config | **built-in** *(wiring in models.rs)* | Material `EMISSION` per emissive cell: models.rs must mark emissive vertices (it reads `colours::plant_emission`); small. |
| O9 | Glow local light | Emitters light open cells around them (0.5 per voxel, 3 cells of 4³), stopped by rock | `voxel.frag:539` `glowAt`, `sink/gpu/light.rs` Glow | on | emitter list (models + plants feed) | **built-in** *(not in lighting brief)* | OmniLight3D per emitter cluster from GDScript; shadowless omnis leak through rock, shadowed ones cost. Alternative: SDFGI emissive. small–medium. |
| O10 | Bloom | Smooth halo (mip chain, Gaussian), strength 0.3, radius 1.5; emitter itself stays crisp | `bloom_smooth.frag`, `bloom.rs` | on | emission | **built-in** | Environment glow (HDR threshold). |
| O11 | Glowcap candle | Each cap's lip wanders `candle_min^(1−u)` over ~8 s, few near full at once | `voxel/candle.rs:28,77`, `emitK` | on (0.25, 8 s) | stand id, frame clock | **port** | GDScript (port candle.rs math) → per-instance emission energy + its omni; small. Feed has stand ids. |
| O12 | Starving tint | Body falls toward grey/dark (0.5), accent turns `#7A2A78` | `animal.rs:147,265,733` | on | reserve/body (sim) | **port** | instance uniform on the animal; small. **Feed lacks** a starving flag (add a bit). |
| O13 | Wounded tint | Starving tint at half strength | `animal.rs:161,270` | on | `flags.wounded` | **port** | same uniform; small; feed has it. |
| O14 | Meal flash | Eye/sense patch leans toward warm `#FF9B50` up to 0.2 while cropping, eased 0.6 s | `animal.rs:132-146`, `VoxelConfig.meal_flash` | on (0.2) | animal state (feeding), frame clock | **port** | GDScript weight + instance uniform; models.rs marks eye/accent vertices; small. Feed: `state` (feeding) and `meal` events suffice. |
| O15 | Carcasses and remains | Carcass drawn flat at 0.35 of body colour; remains in bone colour | `animal.rs:156-159,1480` `stamp_carcasses` | on | carcass records (sim) | **port** | GDScript nodes using the lineage model, lying flat; medium. **Feed lacks** carcasses. |
| O16 | Ground marks | Logs (rim/front/dark/top), litter with flecks, glassfilm, seed banks | `stand.rs:183-215`, `FLOOR_MARK`/`LOG` parts | on (`seed_film_at_founding`) | flora ground state | **port** | small meshes per mark (models or mesher overlay); medium. **Feed lacks** marks. |
| O17 | Latticevine tile layer | Covered rock faces: flush tile on rock, curtain under overhangs, quarter slivers on ±x, accents (bud/flower/fruit), emissive texels | `voxel/vine.rs`, `voxel.frag:260-300` `vineTexel` | **on** (`vine_tiles = true`) | flora cover faces (sim) | **port** | quads/decals with the tile atlas + cutout; tile choice can stay in Rust (`VineCell::tile`); large. **Feed lacks** covered faces + densities + accents. |
| O18 | Interim glyph box | Lineages without a model drawn as a placeholder block | `animal.rs:119,220` | fallback | — | **done** (grey box) | — |
| O19 | Animal style dedupe, palette slots, `MAX_STYLES` | Style table bookkeeping | `sink/gpu/voxel.rs`, `sink/gpu/palette.rs` | — | — | **obsolete** | — |

## 4. Animation and motion (presenter-side, not sim)

| # | Feature | What it does | Where | Desktop | Driven by | Class | Port stage / feed / size |
|---|---|---|---|---|---|---|---|
| M1 | Smooth motion between ticks | Bodies glide between feed frames | Godot `organisms.gd:228` | on | feed pos | **done** | — |
| M2 | Four quarter-turn headings | Models turned to the nearest of four headings | `model.rs:716-740` | on | heading | **obsolete** | Free rotation in Godot. |
| M3 | Pose model by mobility | Seedporter climb/stand/glide, perched models | `model.rs:205-222` `mode_model` | on | `mobility` | **done** (`POSE_MODELS`) | — |
| M4 | Wall-climb facing | A climbing body faces the wall it is on (up: ahead; down: behind) | `animal.rs:104` `wall_toward` | on | wall climb (sim) | **port** | GDScript rotation in climb mode; small. Feed: heading + mobility may suffice; check heading on a descending climber, else add wall direction. |
| M5 | Rider lift | A rider drawn its host's height above its site | `animal.rs:53` `rider_lift` | on | riding host (sim) | **port** | GDScript; small. **Feed lacks** host id (has `riding` bit only); or the feed puts the rider's `pos.y` on the host. |
| M6 | Sleep pose | Torso settles ≤2 rows, legs fold under, back breathes (4 s, 0.2 voxel), blend 1 s | `voxel/sleep.rs`, `voxel.frag:960-1037` `posedFind`, `gpu/voxel.rs:261` `pose_rows` | on | `state` asleep, frame clock | **port** | vertex shader displacement on the animal mesh by per-vertex cell class (models.rs runs `sleep::classify`) + instance uniforms (settle, breath, phase) from GDScript; medium (one package with M7, M8). Feed sufficient. |
| M7 | Idle pose | Still body breathes (2.5 s, 0.1), head dips 0.3–0.5 voxel every 4–10 s over 0.5 s; hold 0.5 s, blend 0.5 s | `voxel/idle.rs:29-47` | on | state + stillness, id hash | **port** | same vertex mechanism, head class; schedule is a pure hash of id+tick (portable); medium with M6. |
| M8 | Hunker transition | Standing model posed toward the baked hunker over 0.75 s (shield down, neck/scoop/stalks cut from the top) | `voxel/hunker.rs:38,67` | on (`hunker_s` 0.75) | `flags.hunkered` | **port** | same vertex mechanism with hunker classes; **Godot currently swaps the model instantly (a pop)**. medium with M6. |
| M9 | Strike crouch/lunge | Interim glyph one row lower in windup, one cell longer in lunge | `animal.rs:585-593` | glyph path only | strike phase | **obsolete** | Lanternjaw now has a baked model; glyph-only. Feed has `striking` only (no phase) if a modelled strike pose is wanted later. |
| M10 | Fade in/out | Bodies/stands fade on appear/leave, slower on death | Godot `organisms.gd:26-29,250` | on | feed, events | **done** | — |
| M11 | Growth transitions | Model step nearest growth; no cross-fade in the old voxel path either | `model.rs`, `stand_delta.rs` | on | growth | **obsolete** (nothing to port) | A step cross-fade would be new work, not a port. |
| M12 | Wind sway/bend | Only in the legacy sprite renderer (`sprite.vert` `iBend`); no voxel-path wind | `sprite.vert` | n/a | — | **obsolete** (legacy only) | New work if wanted (vertex shader + wind in feed). |
| M13 | Foliage regrowth only adds cells | Thinning hash is monotone in the fraction | `model.rs:637` | on | — | **port** (part of O5) | — |

## 5. Sky, day and weather

| # | Feature | What it does | Where | Desktop | Driven by | Class | Port stage / feed / size |
|---|---|---|---|---|---|---|---|
| S1 | Sky gradient | Screen-space zenith→horizon gradient behind the world | `voxel.frag:1268` `skyAt`, `present.rs:253-270` | on (`sky_gradient`) | config, day look | **port** | sky shader using `SCREEN_UV` (an ortho camera samples one sky direction, so the built-in ProceduralSkyMaterial gives a flat colour); small. |
| S2 | Twilight band | Dawn/dusk colour band, `TWILIGHT_DEG` 12, `BAND_GAIN` | `voxel.frag:1259` `bandAt`, `gpu/weather.rs:333-335` | on | sun elevation | **port** | sky shader; small. Feed has `sun_elevation`. |
| S3 | Day sky look | Warm/mint/lilac variants, `day_sky_brightness`, overcast grey/dim, storm slate | `voxel.frag:1581-1666`, `gpu/weather.rs:346-354` | on (warm, 1.0) | cloud cover, storm, config | **port** | sky shader uniforms from GDScript; small. |
| S4 | Stars | Power-law brightness, temperature colour, twinkle on two rates, soft cross on bright ones, `star_bloom` 0.15 | `voxel.frag:1494-1550` `starAt` | on | night, frame clock | **port** | sky shader (HDR values feed built-in glow); small–medium. |
| S5 | Clouds | Domain-warped fbm, drift, sunlit edges, violet undersides by day, storm darkening, lit from within by a flash | `voxel.frag:1552-1666` `cloudFbm`, `litSky` | on | `cloud_cover`, storm, sun, flash | **port** | sky shader; medium. Feed has cover; storm from rain mode. |
| S6 | Day/night light look | Sun arc (rise az −80°, set 60°, noon 70°), weak cool moon at night (0.3), night ambient 0.34, overcast sun/ambient cuts, warm lean low down, sun fade under 6° | `gpu/weather.rs:308-342,598` `look` | on (`day.mode = cycle`) | `day_phase`, `sun_elevation`, `daylight`, `cloud_cover` | **port** (GDScript driving built-in nodes) | extend `weather.gd` (it has a simpler swing); small–medium. Feed sufficient. |
| S7 | Weather easing | Sky 0.5 s, cloud 12 s, storm 8 s, rain 2.5 s, fog 3 s time constants | `gpu/weather.rs:89-104,208` | on | frame clock | **port** | GDScript (weather.gd already eases one tau); small. |
| S8 | Ground fog | Height-shaped band with falloff, drifting noise, lit by ambient + sun back-scatter + emitter glow, stops at water/surfaces | `voxel.frag:2039-2092`, `volumetric.glsl` `fogDensity` | on when `sky.fog` | `sky.fog`, frame clock | **built-in** | Environment height fog / volumetric fog + FogVolume with a noise texture (lighting worker). |
| S9 | Rain streaks | Three parallax layers (drizzle/shower/downpour), slant, gusts, a curtain at downpour, depth-aware (stop at surfaces) | `voxel.frag:2093-2186` | on when raining | `sky.rain`, rate, frame clock | **port** | GPUParticles3D with collision (built-in system, custom look); weather.gd has basic streaks to replace; medium. |
| S10 | Splashes | Short-lived splashes on open tops while raining | `voxel.frag:2188-2217` | on | rain, open tops | **port** | particle sub-emitter on collision (GPUParticlesCollisionHeightField3D); small–medium. |
| S11 | Lightning bolt | 18-segment bolt + fork, kinks, swing, decays (0.13 s), restrokes | `voxel.frag:2219-2306`, `gpu/weather.rs:111-125` | on in storms | `sky.strike` [tick,x,z,seed] | **port** | GDScript-built line/quad mesh with an emissive unshaded shader; medium. Feed sufficient. |
| S12 | Lightning flash fill | Fill light 0.75 × flash, 25 ms attack, 80 ms decay, restroke | `gpu/weather.rs:111-125`, `flashC` | on | strike | **built-in** (GDScript timing) | Bump DirectionalLight/ambient energy; small. |
| S13 | Light shafts | Sun- and glow-lit air scatters toward the camera (sun-visibility volume, rebake + fade) | `volumetric.glsl`, `sink/gpu/sunvis.rs` | **off** (`volumetric = false`) | grid, sun | **built-in** (opt-in) | Volumetric fog if it works with the ortho camera (lighting worker checks). |
| S14 | Flat-tier mist wisps | Drifting wisps in the flat sky | `voxel.frag:1280-1301` | flat tier | — | **obsolete** | Replaced by S5. |
| S15 | Flat-tier day/night tint | Whole-picture tint | `voxel.frag` `u.tint` | flat tier | — | **obsolete** | — |
| S16 | Founding pulse | Sky-only breathing frame while founding | `present.rs:273-302` | panel founding | — | **obsolete** | The shell's new-world picker owns founding. |

## 6. Pipeline and bookkeeping

| # | Feature | Where | Class |
|---|---|---|---|
| P1 | Slab walk, face ownership, `front_hidden`/`top_hidden`, near-to-far order | `voxel.frag` main `:2412`, `present.rs` header | **obsolete** |
| P2 | `px_per_voxel`, integer `rise`, `auto_px_per_voxel`, `raster_height` | `mod.rs` VoxelConfig | **obsolete** (real camera) |
| P3 | Present pass: nearest ×k upscale, quarter turn, WindowFit box filter | `present.frag`, `gpu/present.rs` | **obsolete** |
| P4 | Blocky bloom style | `bloom.frag` | **obsolete** (non-default) |
| P5 | Change-only pack, persistent style palette, `emitK`/`poseK` uniform tables | `sink/gpu/voxel.rs`, `sink/gpu/palette.rs` | **obsolete** |
| P6 | Per-effect switches (`shadows`, `reflections`, `volumetric`, `glow`, `ao`) as spec constants | `gpu/sunvis.rs` Effects | **built-in** (Environment/light toggles; keep one config switch per effect) |
| P7 | Weather depth in the emission alpha | `voxel.frag` `WEATHER_DEPTH` | **obsolete** (depth buffer) |
| P8 | Roof table, sky worker thread, sun-visibility rebake | `sink/gpu/light.rs`, `sunvis.rs` | **obsolete** (T14's port, if chosen, lives in the mesher) |
| P9 | Ring wrap in x | everywhere | **done** for terrain (two copies); every ported shader that reads neighbours (W5, W7, T14, A5 masks) must wrap x in the mesher. |

---

## Counts

Rows above, by class (M13 is part of O5 and not counted; O2 counts as port, its tones as ask):

- **done**: 9 (T1, T2, W1, O1, O18, M1, M3, M10, P9)
- **built-in**: 12 (T17, T18, T19, W4, W10, O8, O9, O10, S8, S12, S13, P6)
- **port**: 34 (T12, T13, W3, W5, W6, W7, W8, W9, O2, O5, O6, O7, O11, O12, O13, O14, O15, O16, O17, M4, M5, M6, M7, M8, S1, S2, S3, S4, S5, S6, S7, S9, S10, S11)
- **obsolete**: 22 (T4, T5, T16, W2, W11, W12, O4, O19, M2, M9, M11, M12, S14, S15, S16, P1, P2, P3, P4, P5, P7, P8)
- **ask Wrysk**: 12 rows in 6 decisions (T3; T6–T11; T14; T15; T20, T21; O3 + O2's tones)

## Ask Wrysk (six decisions)

- **A1 Cave and overhang darkness (T14)**: built-in SDFGI (GPU cost, ortho support unverified, its own occlusion rather than the ecology's) vs port `sky_visibility` as a per-vertex ambient attribute from the mesher (medium, exact). Without either, caves are lit at full ambient.
- **A2 Canopy shade (T20, T21)**: built-in = crowns cast hard, fully opaque shadows and no ambient dimming under them; port = dappled whole-cell shadow (small) + canopy ambient texture (medium), but the feed must carry per-layer foliage.
- **A3 AO crease line (T15)**: built-in SSAO (soft, screen-space) vs port the thin hard pixel-art crease (medium, shares the mesher's neighbour mask with A5).
- **A4 Haze (T3)**: built-in distance fog (also varies with height in a 30° ortho view; must drop the mesher's pre-light haze) vs port z-linear haze after light via the spatial shader's `FOG` output (small, faithful).
- **A5 Autotile edge rules (T6–T11)**: rim row, side bevel, chamfer, riser lean, contour row and drop bevel were readability aids for flat light. With real sun and SSAO the edges may read alone. Keeping them is one package (a mesher neighbour mask that splits greedy merges + a terrain shader), medium.
- **A6 Organism light-stand-in tones (O2/O3)**: the glyph anatomy (eyes, markings) must be ported. The lit-column, rim, crown-underside, crown-edge and trunk-cylinder tones imitate light. Keep them (small on top of O2) or let Godot's light shape the models.

## Proposed port order

1. **Feed additions** (Rust, `crates/cubarium-session/src/feed.rs`; one small package, unblocks 4–6):
   a starving flag bit; per-plant wilt level, ripeness/parcel fullness and per-layer foliage fractions (layer count is small);
   carcass records; ground marks (logs, litter, film, seeds); latticevine covered faces (cell, kind, tile, density, accent);
   the rider's host id (or the rider's `pos.y` set on its host); optionally a canopy plane if A2 = port.
2. **Mesher vertex attributes** (Rust `mesher.rs`; medium; before any face-rule shader): neighbour mask per face
   (A3/A5, greedy merge splits on it), sky visibility (A1), water flow vector, falling flag, shore distance / plunge flag,
   open-to-sky flag (W5–W9). Ring wrap in x for all of them.
3. **Terrain shader** (spatial shader replacing the StandardMaterial): face grain (T12), haze via `FOG` (A4), sky-visibility
   ambient (A1), crease (A3), edge rules (A5). Depends on 2 and the asks.
4. **Water shader**: absorption (W3), ripples + glint (W5, W6), foam (W7), rain rings (W8), falls (W9). Depends on 2;
   lands on top of the lighting worker's SSR.
5. **Sky + day look**: sky shader (S1–S5) and the `weather.gd` look/easing port (S6, S7, S12). Feed is already sufficient;
   independent of 2–4, can run in parallel with them.
6. **Organism look**: models.rs per-vertex data (glyph UVs, part class, emissive flag, layer + cell hash, accent/eye flag),
   the organism shader (glyph tones O2/A6, foliage thinning O5, wilt O6, accents O7, tints O12/O13, meal flash O14),
   emission wiring (O8) and the glowcap candle (O11). Glow omnis (O9) go here or with the lighting worker. Needs 1 for O5–O7, O12.
7. **Poses**: vertex-shader displacement by cell class for sleep, idle and hunker (M6–M8), with GDScript weights. Removes the
   current hunker pop. Shares models.rs per-vertex class with 6, so it goes after 6 or with it.
8. **Weather particles and lightning**: rain layers + splashes (S9, S10) and the bolt (S11). Needs the depth buffer only;
   independent of everything above.
9. **World content from new feed data**: carcasses (O15), ground marks (O16), rider lift and wall facing (M4, M5),
   then the latticevine tile layer (O17, the largest single item). Needs 1.
10. **Deferred / opt-in**: face textures (T13), dappled crown shadow (T20, if A2 = port).
