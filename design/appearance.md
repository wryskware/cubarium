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
   directional core. A frill, fork, bead-chain, or hooked tail changes the outline.
3. **Action:** feeding briefly fills the core, damage causes a short contraction,
   budding develops a real attached lobe, signals produce a small pulse or trail.
4. **Aftereffects:** thin, fading history trails and detritus flecks reveal where
   activity happened without filling the cube with permanent light.

Use functional pigment and organ allocation to establish related palettes.
Reserve modest inherited hue drift for lineage resemblance; avoid assigning
every newborn a random saturated color. Age, hunger, and signal phase may
modulate brightness, but they should not erase identity. Maintain legibility
through shape and motion when nearby organisms have similar color.

Body generation follows the same [phenotype](evolution.md) used for physiology.
An elongated propulsive tail gives a different cadence from a wide, slow fan.
Procedural gaits make subpixel continuous motion visible as intentional steps,
glides, pauses, or contractions. Interpolate using the actual transported motion
path; naïvely rounding independent positions can make slow creatures shimmer.

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

Use a small opt-in gallery driven by real phenotype/render code: solitary slow
grazer; fast forked hunter; resting fan; budding chain; three related descendants;
seam-crossing body and trail; dense local feeding; sparse post-collapse world.
The gallery proves the visual vocabulary, not ecological emergence.

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
