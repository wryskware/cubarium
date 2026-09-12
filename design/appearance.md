---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# Tiny organisms with readable character

Render the world as a dim living substrate with occasional brighter moving
forms. Negative space, stable color relationships, and localized motion are
composition tools. Population control and behavior remain ecological; the
renderer does not hide most organisms to manufacture a sparse-looking world.

## Visual hierarchy

1. **Substrate:** very dark, broad patches with restrained texture. Food is
   suggested by density and a slight color lift, not glowing heatmap values.
2. **Bodies:** stronger silhouettes, mostly 3–7 pixels long, with a readable
   directional core. Begin with lobe count, body length/aspect, and one head/tail
   appendage; measure whether those distinctions read before adding ornaments.
3. **Action:** feeding briefly fills the core, damage causes a short contraction,
   budding develops a real attached lobe. Chemical signaling pulses appear only
   if a later ecological signal experiment is retained.
4. **Aftereffects:** thin, fading history trails and detritus flecks reveal where
   activity happened without filling the cube with permanent light.

Use a stable inherited accent and body structure to establish related palettes.
Reserve modest inherited hue drift for lineage resemblance; avoid assigning
every newborn a random saturated color. Age, hunger, and later signal phase may
modulate brightness, but they should not erase identity. Maintain legibility
through shape and motion when nearby organisms have similar color.

Body generation follows the same [phenotype](evolution.md) used for physiology.
An elongated propulsive tail gives a different cadence from a broad, slow body.
Procedural gaits make subpixel continuous motion visible as intentional steps,
glides, pauses, or contractions. Interpolate using the actual transported motion
path; naïvely rounding independent positions can make slow creatures shimmer.

## Palette (leaning, 2026-09-11)

Wrysk's room and desktop lean Outrun: deep indigo and violet with electric
blue-cyan light, magenta, and one warm orange accent. The M2 presentation
uses that family (sRGB, converted to linear light in the renderer):

| Role | Color | Use |
| --- | --- | --- |
| Floor | `#12093A` at 0.12 brightness | Uniform night floor under everything; "empty" reads as dark, not off |
| Producer | `#1E2798` → `#42C5F8` by density, saturating at 0.6 of `P_max`; brightness 0.06 + 0.49 · t² | Substrate glow; the ordinary standing crop stays a dim floor and only rich patches turn cyan (the linear ramp made the whole cube a blue slab in the web viewer) |
| Detritus | `#510B6D` | Dim violet flecks |
| Body hue ramp | `#FF2AFC` (hue 0) → `#42C6FF` (hue 1) | Inherited accent; brightness by mode |
| Feeding flash | `#FF9B50` | Core lobe while intake is nonzero; the only warm color |
| Trails | body color × 0.25 | Short renderer-only history |

Judged on the cube; adjust by looking, not by theory.

## Pixel treatment and seams

Draw at native face resolution. Use sparse coverage and stable fractional
brightness to express subpixel motion; evaluate the result on LEDs rather than
assuming desktop antialiasing translates. Avoid a whole-screen blur, dense
sparkles, fast hue cycling, and high-frequency flashing. Small head/core accents
can separate an organism from its local substrate without universal bright halos.

Rasterize partial bodies, appendages, and trails through the shared surface
atlas. Do not draw five independent complete sprites. Diffuse pulse rings and
substrate interpolation must use seam-aware samples as described in
[topology](surface-topology.md). Test an asymmetric body across all Top seams;
symmetrical dots conceal orientation mistakes.

At a top vertex use the specified shortest-path pixel ownership; a local shape
discontinuity is possible at this curved point and must be reviewed explicitly.
Start substrate rendering with nearest-cell samples and optional one-pixel
seam-aware filtering. Do not make initial readability depend on distinguishing
two very dark producer colors. Trails record recent motion in the renderer;
organisms cannot sense these visual histories.

Convert a linear artistic color buffer to the shim's expected RGB values once
under a documented output transfer convention verified during integration.
Leave display calibration, hardware brightness and power enforcement to the
shim. Control scene composition and contrast in Cubarium, not panel-specific
correction tables. Record cube brightness and room lighting during visual review.

## Rhythm without a show director

Organisms rest when well fed or waiting for a patch; feeding and reproduction
take time; hunters pay for bursts. Trail decay lets old activity recede. Weather
changes slowly and usually regionally. These dynamics should supply quiet and
busy areas without a timer triggering synchronized spectacles.

External audio should first change environmental opportunity over seconds or
minutes. Individuals can respond differently based on needs and inherited
sensitivity. If a beat becomes visible, it should be filtered through local
organisms and fields rather than blinking the entire cube.

Visual telemetry can count bright-pixel occupancy and temporal variation to
identify noisy parameter sets. It does not automatically cull or reward
organisms. The final criterion is watching at natural office/party distance.

## Review scenes and questions

E6 begins with twelve samples using the limited geometry controls, hue, and
gait on the physical cube. Compare silhouettes and motion separately; record
related/unrelated pair recognition at actual brightness and viewing distance.
Additional antennae, armor outlines, and fans remain visual/physiological
experiments rather than initial requirements.

Extend the opt-in gallery with actual supported behavior: solitary slow grazer,
scavenger on detritus, budding chain, related descendants, seam/vertex-crossing
body and trail, dense feeding, and sparse post-collapse world. Add a hunter only
when predation exists. The gallery proves vocabulary, not ecological emergence.

Review real-time clips of the ecosystem separately. Can a person distinguish
organisms without labels? Follow an individual over an edge? Notice feeding,
resting, pursuit, and budding without being told? Recognize related descendants
after several minutes? Does a quiet view still appear inhabited? Does a busy
patch leave the rest of the cube breathable? Do snapshots hours apart reveal
changed occupation or habits, not just a shifted background color?

Use fixed-seed captures for regressions and actual cube observation for approval.
Do not judge ambient pacing from accelerated simulation playback. Keep debug
charts, sensor status, IDs, and labels in separate tools; normal presentation
contains only the world.
